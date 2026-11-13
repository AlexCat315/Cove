//! Translation of `BrowserPrototype` from
//! `Browser/Sources/CoveUIBrowserPrototype/main.swift`.
//!
//! The reactor owns the single scene graph, the text stack, the draw list and
//! the DevTools surface for the Wasm host. Swift marks the global `app`
//! `nonisolated(unsafe)` because JavaScript only ever calls in on the browser
//! main thread; here the same invariant is carried by `thread_local!` storage
//! in the parent module, so the fields below need no locking.
//!
//! The root content comes from the shared example crate, exactly as the Swift
//! prototype imports `CounterExample`. It is factored behind
//! [`PrototypeContent`] so the host ABI, frame encoding and DevTools plumbing
//! stay independent of any particular demo.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

use cove_composition::ViewGraph;
use cove_core::scene::{
    AnimatorScheduler, EventDispatcher, NodeRenderer, NodeTree, PlatformInputContext, Recomposer,
    TextInputAreaResolver, TextInputAttachmentKey, scale_animation_delta,
};
use cove_core::{DrawList, Point, TextLayoutOptions, layout_text};
use cove_devtools::frame_analysis::{FrameWindow, Timeline};
use cove_devtools::inspection::DiagnosticStateSource;
use cove_devtools::runtime_probe::RuntimeProbe;
use cove_devtools_protocol::{
    TimingFrame,
    control::{Fault, ProbeCommand, Registration, VERSION, Viewport},
};
use cove_platform::{
    InputEvent as PlatformInputEvent, KeyEvent, KeyModifiers, MouseButton, MouseButtonEvent,
    MouseMotionEvent, MouseWheelEvent, PointerKind, ScrollInput, ScrollUnit, TextEditingEvent,
    TextInputArea,
};
use cove_text::{DrawListText, FontCollection};

#[cfg(target_arch = "wasm32")]
use super::gpu::BrowserGpu;
use crate::export_buffer::ExportBuffer;
use crate::prototype::frame::{Batch, Frame, Label, RasterAsset, viewport_size};

/// Atlas edge in texels, matching the Swift prototype.
const ATLAS_SIZE: usize = 2048;

/// Maximum bytes accepted for one font or one text event.
const MAX_PAYLOAD: usize = 32 * 1024 * 1024;

/// The root demo a browser host renders.
///
/// Swift hard-codes `CounterView`; keeping the small surface behind a trait
/// lets the same host drive any shared example that supplies a view tree.
pub trait PrototypeContent {
    /// The `View` this content installs as the graph root.
    fn install(self: &Rc<Self>, graph: &Rc<ViewGraph>);
    /// The three values the frame payload publishes.
    fn summary(&self) -> (i64, bool, String);
}

/// The single-threaded browser reactor.
pub struct BrowserPrototype<C: PrototypeContent> {
    content: Rc<C>,
    tree: Rc<RefCell<NodeTree>>,
    context: PlatformInputContext,
    dispatcher: Rc<EventDispatcher>,
    probe: RuntimeProbe,
    probing: bool,
    graph: Rc<ViewGraph>,
    recomposer: Rc<Recomposer>,
    animations: AnimatorScheduler,
    last_animation_tick: Instant,
    draw_list: DrawList,
    fonts: FontCollection,
    raster_scale: f32,
    frame: Option<Frame>,
    recomposition_ms: f64,
    layout_ms: f64,
    render_ms: f64,
    timeline: Timeline,
    window: FrameWindow,
    pub vertex_buffer: ExportBuffer,
    pub index_buffer: ExportBuffer,
    pub snapshot_buffer: ExportBuffer,
    pub response_buffer: ExportBuffer,
    pub atlas_buffer: ExportBuffer,
    input_anchor: [f32; 3],
    /// The canvas's top-left in the page viewport, in CSS pixels, as last
    /// reported by the JavaScript host. `None` until the host has measured it.
    content_origin: Rc<Cell<Option<(f32, f32)>>>,
    /// The logical size the last frame laid out at.
    content_size: Rc<Cell<(f32, f32)>>,
    #[cfg(target_arch = "wasm32")]
    webgpu: Option<BrowserGpu>,
    #[cfg(target_arch = "wasm32")]
    webgpu_failed: bool,
}

impl<C: PrototypeContent> BrowserPrototype<C> {
    /// Builds the reactor and installs `content` as the graph root.
    pub fn new(content: C) -> Self {
        // The browser has no persistent filesystem: WASI's `std::env::temp_dir()`
        // panics, so file-backed logging stays off and records go to the console.
        let configuration = cove_logging::LoggingConfiguration::new(|value| {
            value.storage.enabled = false;
        });
        let _ = cove_logging::LogSystem::bootstrap(Some("com.cove.browser"), configuration, None);
        let content = Rc::new(content);
        let tree = Rc::new(RefCell::new(NodeTree::new()));
        let context = PlatformInputContext::default();
        let recomposer = Rc::new(Recomposer::new());
        let graph = Rc::new(ViewGraph::new(tree.clone(), recomposer.clone()));
        let dispatcher = Rc::new(EventDispatcher::new(
            tree.clone(),
            context.interactions.clone(),
            context.pointer_capture.clone(),
            context.focus_chain.clone(),
            cove_platform::WindowId(0),
        ));
        // One portal store and tooltip arbiter for this surface, installed before
        // the graph mounts so presenters and the portal host resolve the same
        // store. The browser drives a single window, so the thread-local animator
        // scheduler is captured once and ticked each frame.
        cove_hosts::wasm::install_overlay_services(&context);
        let animations = AnimatorScheduler::current();
        context.with_current(|| content.install(&graph));
        let content_origin = Rc::new(Cell::new(None));
        let content_size = Rc::new(Cell::new((0.0, 0.0)));
        let timeline: Timeline = tree.borrow().timeline.clone();
        let window = FrameWindow::new(&timeline);
        let mut probe = RuntimeProbe::new(
            tree.clone(),
            Some(context.invalidation_log.clone()),
            Some(graph.render_tree().clone()),
            Some(Rc::new(GraphState(graph.clone()))),
        );
        // The browser hosts the same DevTools extension framework as the desktop:
        // packages register panels into this shared registry at runtime.
        probe.diagnostics().set_extension_source(Rc::new(
            cove_devtools::inspection::builtin_extension_registry(),
        ));
        // A wasm reactor cannot read its own place in the page: the JavaScript
        // host reports the canvas origin. Until it does, point mapping stays
        // unavailable instead of assuming the viewport origin.
        probe.content_geometry = Some(Rc::new({
            let origin = content_origin.clone();
            let size = content_size.clone();
            move || {
                let (x, y) = origin
                    .get()
                    .ok_or("The browser host has not reported the canvas position yet.")?;
                let (width, height) = size.get();
                Ok(cove_devtools_protocol::ContentGeometry {
                    origin_x: f64::from(x),
                    origin_y: f64::from(y),
                    scale: 1.0,
                    width: f64::from(width),
                    height: f64::from(height),
                })
            }
        }));
        probe.viewport = Some(Rc::new({
            let size = content_size.clone();
            move || {
                let (width, height) = size.get();
                Viewport { width, height }
            }
        }));
        probe.input = Some(Rc::new({
            let dispatcher = dispatcher.clone();
            move |event| {
                dispatcher.dispatch_local(&event);
            }
        }));
        let mut fonts = FontCollection::new(ATLAS_SIZE, ATLAS_SIZE);
        fonts.configure(16.0, 1.0);
        // Compose `Text` primitives draw through the ambient TextEnvironment.
        // Wasm has no system font resolver: share the reactor's atlas (fed by
        // `cove_font_load`) and seed the shaper from the first loaded face.
        let environment = cove_text::TextEnvironment::new(
            fonts.atlas.clone(),
            cove_text::TextShaper::default(),
            1,
            22.0,
            cove_text::Color::WHITE,
            None,
            None,
            cove_text::TextCacheLimits::default(),
        );
        cove_text::TextEnvironmentHolder::set_current(Some(environment));
        Self {
            content,
            tree,
            context,
            dispatcher,
            probe,
            probing: false,
            graph,
            recomposer,
            animations,
            last_animation_tick: Instant::now(),
            draw_list: DrawList::default(),
            fonts,
            raster_scale: 1.0,
            frame: None,
            recomposition_ms: 0.0,
            layout_ms: 0.0,
            render_ms: 0.0,
            timeline,
            window,
            vertex_buffer: ExportBuffer::new(),
            index_buffer: ExportBuffer::new(),
            snapshot_buffer: ExportBuffer::new(),
            response_buffer: ExportBuffer::new(),
            atlas_buffer: ExportBuffer::new(),
            input_anchor: [0.0; 3],
            content_origin,
            content_size,
            #[cfg(target_arch = "wasm32")]
            webgpu: None,
            #[cfg(target_arch = "wasm32")]
            webgpu_failed: false,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn enable_webgpu(&mut self, format: cove_graphics::TextureFormat) -> bool {
        match BrowserGpu::new(format) {
            Ok(gpu) => {
                self.webgpu = Some(gpu);
                self.webgpu_failed = false;
                true
            }
            Err(error) => {
                eprintln!("WebGPU initialization: {error}");
                false
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn disable_webgpu(&mut self) {
        self.webgpu = None;
        self.webgpu_failed = false;
        self.fonts.atlas.reset();
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn render_web(&mut self, width: f32, height: f32) -> i32 {
        if let Some(gpu) = &self.webgpu {
            match gpu.ready() {
                Ok(()) => {}
                Err(cove_graphics::RhiError::GpuBusy) => return 1,
                Err(error) => {
                    eprintln!("WebGPU frame: {error}");
                    return -1;
                }
            }
        }
        self.render(width, height);
        if self.webgpu_failed { -1 } else { 0 }
    }

    pub fn set_raster_scale(&mut self, scale: f32) {
        if scale.is_finite() {
            self.raster_scale = scale.clamp(1.0, 4.0);
        }
    }

    /// Records the canvas origin the page host measured, in CSS pixels.
    pub fn set_content_origin(&self, x: f32, y: f32) {
        if x.is_finite() && y.is_finite() {
            self.content_origin.set(Some((x, y)));
        }
    }

    pub(super) fn raster_asset_pixels(&self, id: u32) -> *const u8 {
        let mut pixels = std::ptr::null();
        self.draw_list
            .resources
            .for_each::<cove_imaging::ImageAsset>(|asset| {
                if asset.texture_id == id {
                    pixels = asset.image.pixels().as_ptr();
                }
            });
        pixels
    }

    /// Whether the JS host should request another animation frame: an overlay
    /// animation is running, composition is queued, or the tree has unflushed
    /// render updates. `render` flushes the tree, so this settles to `false`
    /// once the surface is idle.
    pub fn needs_frame(&self) -> bool {
        self.animations.has_active_animations()
            || self.recomposer.has_pending()
            || self.tree.borrow().has_render_updates()
    }

    /// Appends a font face; returns its id, or `None` when rejected.
    pub fn append_font(&mut self, bytes: &[u8]) -> Option<i64> {
        if bytes.is_empty() || bytes.len() > MAX_PAYLOAD || self.fonts.font_count() >= 8 {
            return None;
        }
        let id = self.fonts.append(bytes.to_vec())?;
        // `TextShaper` emits glyphs with font id 0. Register the first loaded
        // face under that id too and give it to the ambient shaper, so compose
        // text rasterizes into the same atlas the host exports to JavaScript.
        if id == 1
            && let Some(face) = self.fonts.last_face()
        {
            self.fonts.atlas.register_face(face.clone(), 0);
            if let Some(mut environment) = cove_text::TextEnvironmentHolder::current() {
                environment.shaper.set_face(face, self.fonts.size);
                cove_text::TextEnvironmentHolder::set_current(Some(environment));
            }
        }
        Some(id)
    }

    /// Rebuilds the frame snapshot for the requested viewport.
    pub fn render(&mut self, raw_width: f32, raw_height: f32) {
        let started = Instant::now();
        let (width, height) = viewport_size(raw_width, raw_height);
        self.content_size.set((width, height));

        // Advance overlay/menu animations and commit composition inside the
        // surface's ambient scope, so a controller registered while composing
        // resolves this window's portal store and tooltip arbiter. The delta is
        // capped so a long idle gap cannot skip an animation outright.
        let delta = started
            .duration_since(self.last_animation_tick)
            .as_secs_f64()
            .min(0.1);
        self.last_animation_tick = started;
        let animations = self.animations.clone();
        let context = &self.context;
        let recomposer = &self.recomposer;
        animations.with_current(|| {
            context.with_current(|| {
                animations.tick(scale_animation_delta(delta));
                recomposer.commit_all();
            });
        });
        self.recomposition_ms = started.elapsed().as_secs_f64() * 1000.0;

        let layout_started = Instant::now();
        self.graph.compute_layout_if_needed(width, height);
        self.layout_ms = layout_started.elapsed().as_secs_f64() * 1000.0;

        let mut labels: Vec<Label> = Vec::new();
        let mut note_width = 0.0_f32;
        // `with_current` needs `&self.context` while the pass needs mutable
        // access to the draw list and fonts, so both halves are taken as
        // separate locals rather than through `self`.
        let (context, tree) = (&self.context, &self.tree);
        let (draw_list, fonts) = (&mut self.draw_list, &mut self.fonts);
        context.with_current(|| {
            encode_frame(tree, draw_list, fonts, &mut labels, &mut note_width);
        });

        if fonts.atlas.is_full() {
            // Resetting the atlas invalidates every UV, so re-encode the whole
            // frame instead of patching the swapped glyphs.
            fonts.atlas.reset();
            labels.clear();
            note_width = 0.0;
            context.with_current(|| {
                encode_frame(tree, draw_list, fonts, &mut labels, &mut note_width);
            });
        }

        if self.probing {
            self.probe.scene.editor.draw_overlay(&mut self.draw_list);
        }
        // Close the frame's timeline window even when nothing is recorded, so
        // later frames cannot inherit this one's spans.
        let attribution = self.window.close(&self.timeline);
        let upload = self.fonts.atlas.dirty_upload_payload();
        #[cfg(target_arch = "wasm32")]
        let mut pipeline_creation = None;
        #[cfg(not(target_arch = "wasm32"))]
        let pipeline_creation = None;
        let mut gpu_measurement: Option<cove_devtools::runtime_probe::GpuMeasurement> = None;
        #[cfg(target_arch = "wasm32")]
        if let Some(gpu) = &mut self.webgpu {
            match gpu.draw(
                &self.draw_list,
                &self.fonts.atlas,
                [width, height],
                self.raster_scale,
                self.probing,
            ) {
                Ok(timing) => {
                    if let Some(timing) = timing {
                        let (count, nanos) = timing.pipeline_creation();
                        pipeline_creation = Some(cove_devtools_protocol::PipelineCreation {
                            count,
                            cpu_ms: nanos as f64 / 1_000_000.0,
                        });
                        gpu_measurement = Some(Box::new(move || timing.result()));
                    }
                }
                Err(error) => {
                    self.webgpu_failed = true;
                    eprintln!("WebGPU frame: {error}");
                }
            }
        }
        match &upload {
            Some(payload) => self.atlas_buffer.replace(&payload.pixels),
            None => self.atlas_buffer.replace(&[]),
        }
        self.fonts.atlas.mark_clean();

        let (count, dark, note) = self.content.summary();
        let batches = self
            .draw_list
            .batches()
            .iter()
            .map(|batch| Batch {
                offset: batch.index_offset,
                count: batch.index_count,
                texture_id: batch.texture_id,
                clip: batch
                    .scissor
                    .map(|rect| [rect.x, rect.y, rect.width, rect.height]),
            })
            .collect();
        let vertices = self.draw_list.vertices().len();
        let indices = self.draw_list.indices().len();
        let mut raster_assets = Vec::new();
        self.draw_list
            .resources
            .for_each::<cove_imaging::ImageAsset>(|asset| {
                raster_assets.push(RasterAsset {
                    id: asset.texture_id,
                    width: asset.image.width(),
                    height: asset.image.height(),
                });
            });
        self.frame = Some(Frame {
            count,
            dark,
            note,
            note_width,
            focused: self
                .context
                .focus_chain
                .focused()
                .and_then(|node| crate::prototype::node_debug_name(&node)),
            width,
            height,
            vertices,
            indices,
            batches,
            raster_assets,
            labels,
            font_count: self.fonts.font_count(),
            atlas_width: self.fonts.atlas.width(),
            atlas_height: self.fonts.atlas.height(),
            atlas_update: upload.as_ref().map(|payload| {
                [
                    payload.region.x,
                    payload.region.y,
                    payload.region.width,
                    payload.region.height,
                ]
            }),
            atlas_full: self.fonts.atlas.is_full(),
        });

        self.vertex_buffer
            .replace(vertex_bytes(self.draw_list.vertices()));
        self.index_buffer
            .replace(index_bytes(self.draw_list.indices()));
        if let Some(frame) = &self.frame {
            self.snapshot_buffer.encode(frame);
        }
        self.render_ms =
            (started.elapsed().as_secs_f64() * 1000.0) - self.recomposition_ms - self.layout_ms;
        if self.probing {
            let mut frame = TimingFrame {
                frame: 0,
                recomposition_ms: Some(self.recomposition_ms),
                layout_ms: self.layout_ms,
                draw_ms: self.render_ms,
                render_thread_ms: None,
                gpu_ms: None,
                pipeline_creation,
                submitted: true,
                presented: None,
                total_ms: self.recomposition_ms + self.layout_ms + self.render_ms,
                node_count: self.tree.borrow_mut().node_count(),
                batch_count: self.draw_list.batches().len(),
                timeline_start: 0,
                timeline_end: 0,
            };
            attribution.apply_window(&mut frame);
            self.probe.record_measured(frame, gpu_measurement);
        }
    }

    pub fn pointer(&mut self, kind: i32, x: f32, y: f32) {
        if !x.is_finite() || !y.is_finite() {
            return;
        }
        let event = match kind {
            0 => PlatformInputEvent::MouseMotion(MouseMotionEvent {
                x,
                y,
                delta_x: 0.0,
                delta_y: 0.0,
                kind: PointerKind::default(),
            }),
            1 => PlatformInputEvent::MouseButtonDown(MouseButtonEvent {
                button: MouseButton::Left,
                x,
                y,
                clicks: 1,
                modifiers: KeyModifiers::default(),
                kind: PointerKind::default(),
            }),
            2 => PlatformInputEvent::MouseButtonUp(MouseButtonEvent {
                button: MouseButton::Left,
                x,
                y,
                clicks: 1,
                modifiers: KeyModifiers::default(),
                kind: PointerKind::default(),
            }),
            3 => {
                let press = |down: bool| {
                    let inner = MouseButtonEvent {
                        button: MouseButton::Left,
                        x,
                        y,
                        clicks: 1,
                        modifiers: KeyModifiers::default(),
                        kind: PointerKind::default(),
                    };
                    if down {
                        PlatformInputEvent::MouseButtonDown(inner)
                    } else {
                        PlatformInputEvent::MouseButtonUp(inner)
                    }
                };
                self.context.with_current(|| {
                    self.dispatcher.dispatch(&press(true));
                    self.dispatcher.dispatch(&press(false));
                });
                return;
            }
            _ => return,
        };
        self.context.with_current(|| {
            self.dispatcher.dispatch(&event);
        });
    }

    pub fn key(&mut self, code: u32, modifiers: u16, down: bool, repeated: bool) {
        let event = KeyEvent {
            scancode: code,
            keycode: 0,
            modifiers: KeyModifiers(modifiers),
            is_repeat: repeated,
        };
        let event = if down {
            PlatformInputEvent::KeyDown(event)
        } else {
            PlatformInputEvent::KeyUp(event)
        };
        self.context.with_current(|| {
            self.dispatcher.dispatch(&event);
        });
    }

    pub(super) fn wheel(&self, unit: ScrollUnit, dx: f32, dy: f32, x: f32, y: f32, time: f64) {
        // DOM deltas describe viewport motion, while Cove's wheel convention
        // describes content motion. DOM does not expose gesture phases.
        let event = PlatformInputEvent::MouseWheel(MouseWheelEvent {
            x: -dx,
            y: -dy,
            mouse_x: Some(x),
            mouse_y: Some(y),
            input: ScrollInput {
                unit,
                timestamp: time / 1000.0,
                ..ScrollInput::default()
            },
        });
        self.context
            .with_current(|| self.dispatcher.dispatch(&event));
    }

    pub(super) fn text_input_area(&mut self) -> *const f32 {
        let anchor = self.context.with_current(|| {
            let node = self.context.focus_chain.focused()?;
            if !node.accepts_subtree_input() || !self.context.focus_chain.permits_input(&node) {
                return None;
            }
            let origin = node.absolute_origin();
            match node.attachment::<TextInputAreaResolver>(TextInputAttachmentKey::AREA_RESOLVER) {
                Some(resolve) => resolve(&node, origin),
                None => node.attachment(TextInputAttachmentKey::AREA).or_else(|| {
                    // The prototype's custom note predates the shared editor.
                    (crate::prototype::node_debug_name(&node).as_deref()
                        == Some(cove_examples::CounterDebugName::NOTE))
                    .then(|| TextInputArea {
                        x: origin.x + 16.0,
                        y: origin.y + 12.0,
                        height: 24.0,
                        cursor_x: self.frame.as_ref().map_or(0.0, |frame| {
                            if frame.note.is_empty() {
                                0.0
                            } else {
                                frame.note_width.min((node.frame().width - 36.0).max(0.0))
                            }
                        }),
                        ..TextInputArea::default()
                    })
                }),
            }
        });
        let Some(area) = anchor else {
            return std::ptr::null();
        };
        self.input_anchor = [area.x + area.cursor_x, area.y, area.height];
        if !self.input_anchor.iter().all(|value| value.is_finite()) {
            return std::ptr::null();
        }
        self.input_anchor.as_ptr()
    }

    pub fn text(&mut self, text: &str, editing: bool) {
        if text.len() > 32768 {
            return;
        }
        let event = if editing {
            PlatformInputEvent::TextEditing(TextEditingEvent::from_utf16(
                text,
                text.encode_utf16().count() as i32,
                0,
            ))
        } else {
            PlatformInputEvent::TextInput(text.to_owned())
        };
        self.context.with_current(|| {
            self.dispatcher.dispatch(&event);
        });
    }

    pub fn register_probe(&mut self) {
        self.probing = true;
        let capabilities = self.probe.capabilities();
        #[cfg(target_arch = "wasm32")]
        let capabilities = capabilities
            .into_iter()
            .chain(["capture".to_owned()])
            .collect();
        self.response_buffer.encode(&Registration {
            version: VERSION,
            title: "Cove Web".to_owned(),
            platform: "web".to_owned(),
            pid: None,
            window: None,
            capabilities,
        });
    }

    pub fn disconnect_probe(&mut self) {
        self.probing = false;
        self.probe.disconnect();
    }

    pub fn dispatch(&mut self, bytes: &[u8]) {
        let result = serde_json::from_slice::<ProbeCommand>(bytes)
            .map_err(|error| Fault::new("invalid_request", error.to_string()))
            .and_then(|command| self.context.with_current(|| self.probe.execute(command)));
        self.response_buffer.encode(&result);
    }

    pub fn counter_action(&mut self, action: u32) {
        let name = match action {
            0 => cove_examples::CounterDebugName::INCREMENT,
            1 => cove_examples::CounterDebugName::RESET,
            2 => cove_examples::CounterDebugName::THEME,
            _ => return,
        };
        fn find(node: cove_core::scene::Node, name: &str) -> Option<cove_core::scene::Node> {
            if crate::prototype::node_debug_name(&node).as_deref() == Some(name) {
                return Some(node);
            }
            node.children()
                .into_iter()
                .find_map(|child| find(child, name))
        }
        let root = self.tree.borrow().root();
        if let Some(node) = root.and_then(|root| find(root, name)) {
            let rect = node.absolute_frame();
            self.pointer(3, rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
        }
    }

    pub fn frame(&self) -> Option<&Frame> {
        self.frame.as_ref()
    }

    pub fn font_count(&self) -> usize {
        self.fonts.font_count()
    }

    pub fn draw_list(&self) -> &DrawList {
        &self.draw_list
    }
}

/// Projects `Vertex` storage as bytes for the JavaScript upload.
fn vertex_bytes(vertices: &[cove_core::Vertex]) -> &[u8] {
    // SAFETY: `Vertex` is `#[repr(C)]` with the asserted 20-byte layout and no
    // padding, so the slice covers exactly `len * STRIDE` initialised bytes.
    unsafe {
        std::slice::from_raw_parts(
            vertices.as_ptr().cast::<u8>(),
            std::mem::size_of_val(vertices),
        )
    }
}

/// Projects the index buffer as bytes for the JavaScript upload.
fn index_bytes(indices: &[u32]) -> &[u8] {
    // SAFETY: `u32` is plain old data; the slice covers exactly the live range.
    unsafe {
        std::slice::from_raw_parts(
            indices.as_ptr().cast::<u8>(),
            std::mem::size_of_val(indices),
        )
    }
}

/// Shapes every text run and re-encodes the draw list for one frame.
fn encode_frame(
    tree: &Rc<RefCell<NodeTree>>,
    draw_list: &mut DrawList,
    fonts: &mut FontCollection,
    labels: &mut Vec<Label>,
    note_width: &mut f32,
) {
    let Some(root) = tree.borrow().root() else {
        return;
    };
    draw_list.reset();
    collect_labels(fonts, &root, labels, note_width);
    NodeRenderer.render(&root, draw_list);
    tree.borrow_mut().flush();
}

/// Walks the tree in paint order, reshaping every text attachment.
///
/// The Swift prototype installs a `node.draw` hook for each `CounterTextRun`
/// here, so glyphs inherit the render tree's clipping and ordering. Doing the
/// shaping during the walk (rather than emitting quads directly) is what keeps
/// text above the backgrounds `NodeRenderer` paints afterwards.
fn collect_labels(
    fonts: &mut FontCollection,
    node: &cove_core::scene::Node,
    labels: &mut Vec<Label>,
    note_width: &mut f32,
) {
    if let Some(run) =
        node.attachment::<cove_examples::CounterTextRun>(cove_examples::CounterText::ATTACHMENT)
    {
        let frame = node.absolute_frame();
        let padding = node
            .layout_node()
            .map(|layout| layout.resolved_padding())
            .unwrap_or_default();
        let color = node.inherited_foreground_color().unwrap_or(run.color);
        let scale = fonts.raster_scale;
        fonts.configure(run.size, scale);
        let glyphs = fonts.shape(&run.text);
        let layout = layout_text(
            &glyphs,
            &run.text,
            &fonts.atlas,
            TextLayoutOptions {
                line_height: run.size * 1.4,
                ..Default::default()
            },
        );
        let x = frame.x + run.inset + padding.left;
        let y = frame.y
            + padding.top
            + (frame.height - padding.top - padding.bottom - run.size * 1.4) * 0.5;
        labels.push(Label {
            text: run.text.clone(),
            x,
            y,
            size: run.size,
            color: format!(
                "rgba({}, {}, {}, {})",
                (color.r * 255.0) as u8,
                (color.g * 255.0) as u8,
                (color.b * 255.0) as u8,
                color.a
            ),
            width: layout.total_width,
            glyphs: glyphs.iter().map(|glyph| glyph.glyph_id).collect(),
            clusters: glyphs.iter().map(|glyph| glyph.cluster).collect(),
            font_ids: glyphs.iter().map(|glyph| glyph.font_id).collect(),
        });
        if crate::prototype::node_debug_name(node).as_deref()
            == Some(cove_examples::CounterDebugName::NOTE)
        {
            *note_width = layout.total_width;
        }
        let atlas = fonts.atlas.clone();
        let origin = Point::new(x, y);
        let tint = color;
        node.set_draw(Some(Rc::new(move |list, _| {
            let _ = list.add_text(&layout, origin, tint, 1, Some(&atlas), None);
        })));
    }
    for child in node.children() {
        collect_labels(fonts, &child, labels, note_width);
    }
}

struct GraphState(Rc<ViewGraph>);

impl DiagnosticStateSource for GraphState {
    fn descriptors(&self) -> Vec<cove_core::scene::StateDescriptor> {
        self.0.state_descriptors()
    }

    fn values(&self, ids: &[String]) -> Vec<cove_core::scene::StateValue> {
        self.0.state_values(ids)
    }
}
