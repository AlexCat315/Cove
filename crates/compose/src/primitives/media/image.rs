//! Translation of Compose/Sources/CoveUICompose/Primitives/Media/Image.swift.
//!
//! Bitmap image primitive backed by an owned CPU asset or a registered GPU
//! texture.
//!
//! File images retain their decoded asset with the draw list. NativeRHI
//! renderers upload it on first use. Hosts can also supply a manually
//! registered `TextureId`. Layout uses the explicit logical width and height.
//!
//! `tint` multiplies the sampled RGBA. Pass white (default) for an untouched
//! bitmap, or any other color to recolour an opaque shape (a multiplicative
//! tint matching the shader's `color * texture` path).

use std::path::PathBuf;
use std::sync::Arc;

use crate::{PrimitiveView, impl_primitive_view};
use cove_core::scene::{Point, Rect, Size};
use cove_core::{Color, TextureId};
use cove_imaging::{ImageAsset, TextureImage};

use super::image_geometry::{ImageContentMode, ImageGeometry, ImageRenderingMode};
use super::vector_image_raster::VectorImageRaster;

/// The drawable density published by the host. Swift reads the same value from
/// `ContentScaleHolder`; the Rust core keeps the process-wide value in
/// `cove_core::content_scale`.
pub struct ContentScaleHolder;
impl ContentScaleHolder {
    pub fn current() -> f32 {
        cove_core::content_scale()
    }
    pub fn set_current(scale: f32) {
        cove_core::set_content_scale(scale);
    }
}

/// Where an `Image` samples from.
#[derive(Clone)]
pub enum ImageSource {
    Texture {
        texture_id: TextureId,
        size: Option<Size>,
    },
    Asset(Arc<ImageAsset>),
    Rendered(Arc<TextureImage>),
}

impl PartialEq for ImageSource {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Texture {
                    texture_id: lhs,
                    size: left,
                },
                Self::Texture {
                    texture_id: rhs,
                    size: right,
                },
            ) => lhs == rhs && left == right,
            (Self::Asset(lhs), Self::Asset(rhs)) => Arc::ptr_eq(lhs, rhs),
            (Self::Rendered(lhs), Self::Rendered(rhs)) => Arc::ptr_eq(lhs, rhs),
            _ => false,
        }
    }
}
impl Eq for ImageSource {}

impl ImageSource {
    pub fn texture_id(&self) -> TextureId {
        match self {
            Self::Texture { texture_id, .. } => *texture_id,
            Self::Asset(asset) => asset.texture_id,
            Self::Rendered(image) => image.texture_id,
        }
    }

    pub fn source_pixel_size(&self) -> Option<(f32, f32)> {
        match self {
            Self::Texture { size, .. } => size.map(|size| (size.width, size.height)),
            Self::Asset(asset) => Some((asset.image.width() as f32, asset.image.height() as f32)),
            Self::Rendered(image) => Some((
                image.pixel_size.width as f32,
                image.pixel_size.height as f32,
            )),
        }
    }

    pub fn retain(&self, list: &mut cove_core::DrawList) {
        match self {
            Self::Asset(asset) => list.resources.retain(asset.clone()),
            Self::Rendered(image) => list.resources.retain(image.clone()),
            Self::Texture { .. } => {}
        }
    }

    /// Pick a mip/resolution that covers `target` pixels out of the fraction of
    /// the source that is actually sampled.
    pub fn resolution(&self, target: Size, fraction: Size) -> Self {
        match self {
            Self::Asset(asset) => Self::Asset(asset.resolution(target, fraction)),
            Self::Rendered(image) => Self::Rendered(image.resolution(
                target,
                Some(Size {
                    width: image.pixel_size.width as f32 * fraction.width,
                    height: image.pixel_size.height as f32 * fraction.height,
                }),
            )),
            Self::Texture { .. } => self.clone(),
        }
    }
}

/// Bitmap image primitive.
#[derive(Clone)]
pub struct Image {
    pub source: ImageSource,
    pub width: f32,
    pub height: f32,
    pub tint: Color,
    pub content_mode: ImageContentMode,
    pub rendering_mode: ImageRenderingMode,
    /// Explicit fill for alpha-mask sources, used by `Icon`/`Badge`.
    pub foreground: Option<Color>,
    /// Set for SVG/PDF sources so the raster follows the draw size and density.
    pub vector_source_url: Option<PathBuf>,
}

#[derive(Clone, PartialEq)]
struct PaintIdentity {
    source: ImageSource,
    width: f32,
    height: f32,
    tint: Color,
    content_mode: ImageContentMode,
    rendering_mode: ImageRenderingMode,
    foreground: Option<Color>,
    vector_source_url: Option<PathBuf>,
}

impl Image {
    pub fn from_texture(texture_id: TextureId, size: Size) -> Self {
        Self::new(
            ImageSource::Texture {
                texture_id,
                size: None,
            },
            size,
        )
    }

    /// Draw the source at the given logical size.
    pub fn new(source: ImageSource, size: Size) -> Self {
        Self {
            source,
            width: size.width,
            height: size.height,
            tint: Color::WHITE,
            content_mode: ImageContentMode::Stretch,
            rendering_mode: ImageRenderingMode::Color,
            foreground: None,
            vector_source_url: None,
        }
    }

    pub fn tint(mut self, tint: Color) -> Self {
        self.tint = tint;
        self
    }

    pub fn content_mode(mut self, mode: ImageContentMode) -> Self {
        self.content_mode = mode;
        self
    }

    pub fn rendering_mode(mut self, mode: ImageRenderingMode) -> Self {
        self.rendering_mode = mode;
        self
    }

    /// Explicit fill for alpha-mask glyphs.
    pub fn foreground(mut self, color: Color) -> Self {
        self.foreground = Some(color);
        self
    }

    pub fn with_vector_source(mut self, path: impl Into<PathBuf>) -> Self {
        self.vector_source_url = Some(path.into());
        self
    }
}

impl PrimitiveView for Image {
    fn make_node(&self) -> cove_core::scene::Node {
        let node = cove_core::scene::Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn update_node(&self, node: &cove_core::scene::Node) {
        let snapshot = self.clone();
        let vector_raster = self.vector_source_url.clone().map(|path| {
            std::rc::Rc::new(VectorImageRaster::new(
                path,
                self.source.clone(),
                self.width,
                self.height,
            ))
        });
        let identity = PaintIdentity {
            source: self.source.clone(),
            width: self.width,
            height: self.height,
            tint: self.tint,
            content_mode: self.content_mode,
            rendering_mode: self.rendering_mode,
            foreground: self.foreground,
            vector_source_url: self.vector_source_url.clone(),
        };
        let draw_node = node.clone();
        node.update_draw(identity, move |list, origin| {
            let node = &draw_node;
            let frame = node.frame();
            let draw_width = if frame.width > 0.0 {
                frame.width
            } else {
                snapshot.width
            };
            let draw_height = if frame.height > 0.0 {
                frame.height
            } else {
                snapshot.height
            };
            let modifier_tint = if snapshot.rendering_mode == ImageRenderingMode::AlphaMask {
                snapshot
                    .foreground
                    .or_else(|| node.inherited_foreground_color())
                    .unwrap_or_else(|| crate::theme::theme_for_node(node).colors.on_surface)
            } else {
                node.foreground_color().unwrap_or(Color::WHITE)
            };
            let base_tint = Color {
                r: snapshot.tint.r * modifier_tint.r,
                g: snapshot.tint.g * modifier_tint.g,
                b: snapshot.tint.b * modifier_tint.b,
                a: snapshot.tint.a * modifier_tint.a,
            }
            .multiplied_alpha(node.opacity());
            let container = Rect::new(origin.x, origin.y, draw_width, draw_height);
            let resolved = vector_raster
                .as_ref()
                .map(|raster| {
                    let repaint = node.downgrade();
                    raster.resolve_async(draw_width, draw_height, move || {
                        if let Some(node) = cove_core::scene::Node::upgrade(&repaint) {
                            node.mark_render_dirty(
                                cove_core::scene::InvalidationSource::style_set("image.raster"),
                            );
                        }
                    })
                })
                .unwrap_or_else(|| snapshot.source.clone());
            let geometry = ImageGeometry::new(
                container,
                resolved.source_pixel_size(),
                snapshot.content_mode,
            );
            let mut rect = geometry.rect;
            let pixel_scale = list.pixel_scale.max(1.0);
            let selected = resolved.resolution(
                Size {
                    width: rect.width * pixel_scale,
                    height: rect.height * pixel_scale,
                },
                Size {
                    width: geometry.uv_max.x - geometry.uv_min.x,
                    height: geometry.uv_max.y - geometry.uv_min.y,
                },
            );
            selected.retain(list);
            let texture = selected.texture_id();
            if snapshot.rendering_mode == ImageRenderingMode::AlphaMask {
                let scale = ContentScaleHolder::current();
                if vector_raster.is_some() && scale.is_finite() && scale > 0.0 {
                    rect = Rect::new(
                        (rect.x * scale).round() / scale,
                        (rect.y * scale).round() / scale,
                        rect.width,
                        rect.height,
                    );
                }
                let _ = list.add_image_mask_quad(
                    rect,
                    texture,
                    base_tint,
                    Point::new(geometry.uv_min.x, geometry.uv_min.y),
                    Point::new(geometry.uv_max.x, geometry.uv_max.y),
                );
            } else if node.corner_radius() > 0.0 {
                let _ = list.add_rounded_image_quad(
                    rect,
                    node.corner_radius(),
                    texture,
                    base_tint,
                    Point::new(geometry.uv_min.x, geometry.uv_min.y),
                    Point::new(geometry.uv_max.x, geometry.uv_max.y),
                );
            } else {
                let _ = list.add_image_quad(
                    rect,
                    texture,
                    base_tint,
                    Point::new(geometry.uv_min.x, geometry.uv_min.y),
                    Point::new(geometry.uv_max.x, geometry.uv_max.y),
                );
            }
        });
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        let layout = cove_core::scene::LayoutNode::new();
        layout.set_width(Some(self.width));
        layout.set_height(Some(self.height));
        Some(layout)
    }

    fn update_layout(&self, layout: &cove_core::scene::LayoutNode) {
        layout.set_width(Some(self.width));
        layout.set_height(Some(self.height));
    }
}
impl_primitive_view!(Image);

/// Erased source convenience for callers that build a texture id by hand.
pub fn texture_source(texture_id: TextureId, size: Option<Size>) -> ImageSource {
    ImageSource::Texture { texture_id, size }
}

/// Identity helper used by host galleries.
pub fn image_is_same_source(lhs: &ImageSource, rhs: &ImageSource) -> bool {
    lhs == rhs
}
