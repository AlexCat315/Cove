//! Video surface: presents the latest frame of a `cove_media::VideoOutput`.
//!
//! Frames arrive on media threads. The surface's change listener only flips an
//! atomic and, when it was clear, posts one wake to the UI thread, which marks
//! the node render-dirty. Drawing samples whatever frame is newest by then, so a
//! slow UI thread drops frames instead of queueing them.
//!
//! Every frame is retained under one texture id per surface; the renderer treats a
//! new owner under a known id as a streamed texture and updates one GPU slot in
//! place. RGBA8 frames whose storage lends its plane are shown without a CPU copy.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::{PrimitiveView, impl_primitive_view};
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, InvalidationSource, LayoutNode, MeasureMode, Node,
    NodeResource, NodeWeak, Point, Rect,
};
use cove_core::{Bitmap, Color, Size, TextureAsset, TextureId, ThreadPoster};
use cove_media::{ChangeListener, VideoFrame, VideoOutput};

use super::image_geometry::{ImageContentMode, ImageGeometry};

/// Presents a [`VideoOutput`]. Without an output it paints only its background.
#[derive(Clone)]
pub struct VideoView {
    pub output: Option<VideoOutput>,
    pub content_mode: ImageContentMode,
    pub background: Option<Color>,
    /// Width over height used for layout before the first frame arrives.
    pub placeholder_aspect: f32,
}

impl VideoView {
    pub fn new(output: Option<VideoOutput>) -> Self {
        Self {
            output,
            content_mode: ImageContentMode::Fit,
            background: Some(Color::BLACK),
            placeholder_aspect: 16.0 / 9.0,
        }
    }

    pub fn content_mode(mut self, mode: ImageContentMode) -> Self {
        self.content_mode = mode;
        self
    }

    /// `None` leaves letterbox areas transparent.
    pub fn background(mut self, color: Option<Color>) -> Self {
        self.background = color;
        self
    }

    pub fn placeholder_aspect(mut self, aspect: f32) -> Self {
        if aspect.is_finite() && aspect > 0.0 {
            self.placeholder_aspect = aspect;
        }
        self
    }
}

#[derive(Clone, PartialEq)]
struct PaintIdentity {
    output: Option<u64>,
    content_mode: ImageContentMode,
    background: Option<Color>,
}

impl PrimitiveView for VideoView {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        let surface: Rc<dyn NodeResource> = Rc::new(VideoSurface::new());
        node.add_resource(surface);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_node(&self, node: &Node) {
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Image,
            label: crate::i18n::tr("Video"),
            ..Default::default()
        }));
        let Some(surface) = node.first_resource::<VideoSurface>() else {
            return;
        };
        surface.bind(self.output.clone());

        if let Some(layout) = node.layout_node() {
            let measured = surface.clone();
            let placeholder = self.placeholder_aspect;
            layout.set_measure_func(Some(Rc::new(
                move |width, width_mode, height, height_mode| {
                    let aspect = measured.aspect().unwrap_or(placeholder);
                    match (width_mode, height_mode) {
                        (MeasureMode::Undefined, MeasureMode::Undefined) => Size {
                            width: 320.0,
                            height: 320.0 / aspect,
                        },
                        (MeasureMode::Undefined, _) => Size {
                            width: height * aspect,
                            height,
                        },
                        _ => Size {
                            width,
                            height: width / aspect,
                        },
                    }
                },
            )));
        }

        let identity = PaintIdentity {
            output: self.output.as_ref().map(VideoOutput::id),
            content_mode: self.content_mode,
            background: self.background,
        };
        let weak = node.downgrade();
        let content_mode = self.content_mode;
        let background = self.background;
        node.update_draw(identity, move |list, origin| {
            let Some(owner) = Node::upgrade(&weak) else {
                return;
            };
            let frame = owner.frame();
            let container = Rect::new(origin.x, origin.y, frame.width, frame.height);
            if container.width <= 0.0 || container.height <= 0.0 {
                return;
            }
            let opacity = owner.opacity();
            let radius = owner.corner_radius();
            if let Some(color) = background {
                let color = color.multiplied_alpha(opacity);
                let _ = if radius > 0.0 {
                    list.add_rounded_rect(container, radius, color)
                } else {
                    list.add_rect(container, color)
                };
            }
            let Some(surface) = owner.first_resource::<VideoSurface>() else {
                return;
            };
            let Some(texture) = surface.texture() else {
                return;
            };
            let geometry = ImageGeometry::new(
                container,
                Some((texture.display_width, texture.display_height)),
                content_mode,
            );
            let tint = Color::WHITE.multiplied_alpha(opacity);
            let uv_min = Point::new(geometry.uv_min.x, geometry.uv_min.y);
            let uv_max = Point::new(geometry.uv_max.x, geometry.uv_max.y);
            let id = texture.id;
            list.resources
                .retain_texture_asset(texture as Arc<dyn TextureAsset>);
            let _ = if radius > 0.0 {
                list.add_rounded_image_quad(geometry.rect, radius, id, tint, uv_min, uv_max)
            } else {
                list.add_image_quad(geometry.rect, id, tint, uv_min, uv_max)
            };
            surface.presented.set(surface.presented.get() + 1);
        });
    }
}
impl_primitive_view!(VideoView);

/// One video frame lent to the renderer under the surface's stable texture id.
struct FrameTexture {
    id: TextureId,
    bitmap: Bitmap,
    display_width: f32,
    display_height: f32,
    key: (u64, u64, u64),
}

impl TextureAsset for FrameTexture {
    fn texture_id(&self) -> TextureId {
        self.id
    }
    fn image(&self) -> &Bitmap {
        &self.bitmap
    }
}

thread_local! {
    static SURFACES: RefCell<Vec<Weak<VideoSurface>>> = const { RefCell::new(Vec::new()) };
}

/// Repaints every surface whose output changed since the last wake.
fn flush_surfaces() {
    let surfaces: Vec<Rc<VideoSurface>> = SURFACES.with(|surfaces| {
        let mut surfaces = surfaces.borrow_mut();
        surfaces.retain(|surface| surface.strong_count() > 0);
        surfaces.iter().filter_map(Weak::upgrade).collect()
    });
    for surface in surfaces {
        let was = surface.dirty.swap(false, Ordering::AcqRel);
        let node = Node::upgrade(&surface.node.borrow());
        // TEMP-DIAG
        cove_logging::LogSystem::logger("com.cove.media.diag").log(&cove_logging::LogEvent::new(
            cove_logging::Level::Info,
            format!("flush dirty={was} node={}", node.is_some()),
        ));
        if was && let Some(node) = node {
            node.mark_render_dirty(InvalidationSource::style_set("video.frame"));
        }
    }
}

/// Node-owned binding between a [`VideoOutput`] and the renderer.
pub struct VideoSurface {
    node: RefCell<NodeWeak>,
    texture_id: Option<TextureId>,
    output: RefCell<Option<VideoOutput>>,
    listener: RefCell<Option<ChangeListener>>,
    dirty: Arc<AtomicBool>,
    current: RefCell<Option<Arc<FrameTexture>>>,
    /// Draws that sampled a frame; diagnostics and tests read it.
    presented: Cell<u64>,
    mounted: Cell<bool>,
}

impl VideoSurface {
    fn new() -> Self {
        Self {
            node: RefCell::new(Weak::new()),
            texture_id: cove_imaging::allocate_texture_id().ok(),
            output: RefCell::new(None),
            listener: RefCell::new(None),
            dirty: Arc::new(AtomicBool::new(false)),
            current: RefCell::new(None),
            presented: Cell::new(0),
            mounted: Cell::new(false),
        }
    }

    /// Draws that sampled a frame since the surface was created.
    pub fn presented_draws(&self) -> u64 {
        self.presented.get()
    }

    fn bind(&self, output: Option<VideoOutput>) {
        let same = match (&*self.output.borrow(), &output) {
            (Some(lhs), Some(rhs)) => lhs.id() == rhs.id(),
            (None, None) => true,
            _ => false,
        };
        if same {
            return;
        }
        *self.output.borrow_mut() = output;
        *self.current.borrow_mut() = None;
        self.listen();
    }

    fn listen(&self) {
        let listener = self.output.borrow().as_ref().and_then(|output| {
            if !self.mounted.get() {
                return None;
            }
            let poster = ThreadPoster::current();
            let dirty = self.dirty.clone();
            Some(output.on_change(Arc::new(move || {
                if !dirty.swap(true, Ordering::AcqRel) {
                    poster.post(flush_surfaces);
                }
            })))
        });
        *self.listener.borrow_mut() = listener;
    }

    /// Width over height of the newest frame, honouring its pixel aspect ratio.
    fn aspect(&self) -> Option<f32> {
        let frame = self.output.borrow().as_ref()?.latest()?;
        let (width, height) = display_size(&frame);
        (height > 0.0).then_some(width / height)
    }

    /// The texture for the newest frame, rebuilt only when the frame changed.
    fn texture(&self) -> Option<Arc<FrameTexture>> {
        let id = self.texture_id?;
        let frame = self.output.borrow().as_ref()?.latest()?;
        let key = (frame.generation, frame.sequence, frame.format.width as u64);
        if let Some(current) = self.current.borrow().as_ref()
            && current.key == key
        {
            return Some(current.clone());
        }
        // TEMP-DIAG
        cove_logging::LogSystem::logger("com.cove.media.diag").log(&cove_logging::LogEvent::new(
            cove_logging::Level::Info,
            format!("texture pts={:?} seq={}", frame.pts.map(cove_media::MediaTime::seconds), frame.sequence),
        ));
        let pixels = frame.rgba_pixels().ok()?;
        let bitmap = Bitmap::new(frame.format.width, frame.format.height, pixels).ok()?;
        let (display_width, display_height) = display_size(&frame);
        let texture = Arc::new(FrameTexture {
            id,
            bitmap,
            display_width,
            display_height,
            key,
        });
        *self.current.borrow_mut() = Some(texture.clone());
        Some(texture)
    }
}

fn display_size(frame: &VideoFrame) -> (f32, f32) {
    let (num, den) = frame.format.pixel_aspect_ratio;
    let ratio = if num > 0 && den > 0 {
        num as f32 / den as f32
    } else {
        1.0
    };
    (
        frame.format.width as f32 * ratio,
        frame.format.height as f32,
    )
}

impl NodeResource for VideoSurface {
    fn mount(&self, node: &Node) {
        *self.node.borrow_mut() = node.downgrade();
        self.mounted.set(true);
        SURFACES.with(|surfaces| {
            if let Some(this) = node.first_resource::<VideoSurface>() {
                surfaces.borrow_mut().push(Rc::downgrade(&this));
            }
        });
        self.listen();
    }

    fn unmount(&self, _node: &Node) {
        self.mounted.set(false);
        *self.listener.borrow_mut() = None;
        *self.current.borrow_mut() = None;
        *self.node.borrow_mut() = Weak::new();
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}
