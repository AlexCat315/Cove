//! Translation of Android/Integration/Scene.swift.

use super::diagnostics::{AndroidDiagnostics, AndroidDiagnosticsHooks};
use super::environment::SceneEnvironment;
use super::surface::SurfaceState;
use super::touch_input::TouchInput;
use crate::desktop::diagnostics::integration::{
    DevToolsConfiguration, DiagnosticMemorySnapshot, DiagnosticsFrameSource,
};
use cove_composition::{
    AnyView, LayoutDebugAttachmentKey, PrimitiveView, View, ViewGraph, ViewKind,
};
use cove_core::scene::ComponentSourceLocation;
use cove_core::scene::{
    AnimatorScheduler, EventDispatcher, NodeRenderer, NodeTree, PlatformInputContext, Recomposer,
    RenderTree, TextEditActions, TextEditCommand, TextInputAttachmentKey, TextInputClient,
    TextInputSnapshot,
};
use cove_core::{DrawList, Environment, GeometryAntialiasing, Point, State};
use cove_platform::{
    InputEvent, KeyEvent, KeyModifiers, MouseButton, MouseButtonEvent, MouseMotionEvent,
    MouseWheelEvent, ScrollDevice, ScrollInput, ScrollUnit, TextEditingEvent,
};
use cove_ui_compose::theme::{TextScalerViewExt, surface_theme};
use std::cell::{Cell, OnceCell, RefCell};
use std::rc::Rc;
use std::time::Instant;

/// Provides the surface environment read by adaptive content.
struct AndroidAdaptiveSurface {
    content: AnyView,
    environment: State<Environment>,
}

impl PrimitiveView for AndroidAdaptiveSurface {
    fn make_node(&self) -> cove_core::scene::Node {
        let node = cove_core::scene::Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn update_node(&self, _node: &cove_core::scene::Node) {}

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn children(&self, _node: &cove_core::scene::Node) -> Vec<AnyView> {
        let environment = self.environment.get();
        vec![AnyView::new(
            cove_ui_compose::primitives::layout::SurfaceRoot::new(
                environment,
                self.content.clone(),
            ),
        )]
    }
}

impl View for AndroidAdaptiveSurface {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
    /// `children()` reads the surface environment from `State`, so the scope must
    /// declare that dependency. Without it nothing subscribes and a repaint never
    /// re-runs `children()`, leaving idiom, keyboard height and text scale frozen
    /// at the first frame — which is what broke Android safe-area avoidance.
    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![cove_composition::DynamicField::state(
            "environment",
            &self.environment,
        )]
    }
}

/// Compose, input and font ownership for a native Android surface host.
/// Application content supplies a View; the platform adapter supplies surface frames.
pub struct AndroidContentSession {
    pub(crate) tree: Rc<RefCell<NodeTree>>,
    context: Rc<PlatformInputContext>,
    recomposer: Rc<Recomposer>,
    environment: SceneEnvironment,
    animations: AnimatorScheduler,
    rendering: Rendering,
    touch: RefCell<TouchInput>,
    layers: RefCell<cove_core::scene::LayerAwareNodeRenderer>,
    nodes: NodeRenderer,
    graph: OnceCell<Rc<ViewGraph>>,
    dispatcher: OnceCell<Rc<EventDispatcher>>,
    needs_update: Cell<bool>,
    diagnostics: RefCell<Option<AndroidDiagnostics>>,
    /// A mirror session needs a steady frame stream even when the UI is idle.
    mirroring: Rc<Cell<bool>>,
    draw_list: DrawList,
    render_tree: RenderTree,
    render_root: Option<cove_core::scene::Node>,
    render_structure_version: u64,
    surface: SurfaceState,
    performance_hud: Option<crate::desktop::diagnostics::PerformanceHud>,
}

/// How the content session turns the node tree into a draw list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rendering {
    Layers,
    Direct,
}

impl AndroidContentSession {
    pub fn new(scale: f32, rendering: Rendering, content: AnyView) -> Self {
        Self::configured(scale, rendering, content, None)
    }

    pub fn with_configuration(
        scale: f32,
        rendering: Rendering,
        content: AnyView,
        configuration: crate::ApplicationConfiguration,
    ) -> Self {
        Self::configured(scale, rendering, content, Some(configuration))
    }

    fn configured(
        scale: f32,
        rendering: Rendering,
        content: AnyView,
        configuration: Option<crate::ApplicationConfiguration>,
    ) -> Self {
        let tree = Rc::new(RefCell::new(NodeTree::new()));
        let recomposer = Rc::new(Recomposer::new());
        let context = Rc::new(PlatformInputContext::new());
        let surface = SurfaceState::new(scale);
        let mut environment = SceneEnvironment::new();
        if let Some(configuration) = &configuration {
            environment.configure_application(configuration);
        }
        environment.configure(surface.scale());
        let mut draw_list = DrawList::default();
        draw_list.geometry_antialiasing =
            configuration
                .as_ref()
                .map_or(GeometryAntialiasing::Analytic, |value| {
                    value
                        .window
                        .rendering
                        .geometry_antialiasing
                        .unwrap_or(value.rendering.geometry_antialiasing)
                });
        let performance_hud = configuration
            .as_ref()
            .filter(|value| value.diagnostics.shows_performance_hud)
            .map(|_| crate::desktop::diagnostics::PerformanceHud::default());
        let content = match &configuration {
            Some(configuration) => {
                AnyView::new(content.text_scaler(configuration.typography.scaler.clone()))
            }
            None => content,
        };

        let session = Self {
            tree,
            context,
            recomposer,
            environment,
            animations: AnimatorScheduler::current(),
            rendering,
            touch: RefCell::new(TouchInput::new()),
            layers: RefCell::new(cove_core::scene::LayerAwareNodeRenderer::new()),
            nodes: NodeRenderer::new(),
            graph: OnceCell::new(),
            dispatcher: OnceCell::new(),
            needs_update: Cell::new(true),
            diagnostics: RefCell::new(None),
            mirroring: Rc::new(Cell::new(false)),
            draw_list,
            render_tree: RenderTree::new(),
            render_root: None,
            render_structure_version: 0,
            surface,
            performance_hud,
        };
        session.with_current(|| {
            let graph = session.graph.get_or_init(|| {
                Rc::new(ViewGraph::new(
                    session.tree.clone(),
                    session.recomposer.clone(),
                ))
            });
            let view = AnyView::new(AndroidAdaptiveSurface {
                content,
                environment: session.surface.value(),
            });
            graph.install_view(
                view,
                ComponentSourceLocation::new(
                    "android",
                    "crates/hosts/src/android/integration/scene.rs",
                    0,
                ),
            );
        });
        session
    }

    fn dispatcher(&self) -> Rc<EventDispatcher> {
        self.dispatcher
            .get_or_init(|| {
                Rc::new(EventDispatcher::new(
                    self.tree.clone(),
                    self.context.interactions.clone(),
                    self.context.pointer_capture.clone(),
                    self.context.focus_chain.clone(),
                    cove_platform::WindowId::MAIN,
                ))
            })
            .clone()
    }

    pub fn logical_size(&self) -> (f32, f32) {
        self.surface.logical_size()
    }

    pub fn scale(&self) -> f32 {
        self.surface.scale()
    }

    /// 1 = dark icons over a light surface, 0 = light icons over a dark
    /// surface, -1 = unknown. The Java host applies it to the system bars.
    pub fn system_bar_icon_style(&self) -> i32 {
        self.surface.system_bar_icon_style()
    }

    /// Reports device facts the window itself cannot derive: display cutout and
    /// system-bar insets, the software keyboard height, the accessibility text
    /// size and whether this is a phone or a tablet.
    pub fn update_surface(
        &self,
        idiom: cove_core::Idiom,
        safe_area: cove_core::Insets,
        keyboard: cove_core::Keyboard,
        text_scale: cove_core::TextScale,
    ) {
        if self.surface.update(idiom, safe_area, keyboard, text_scale) {
            self.needs_update.set(true);
        }
    }

    pub fn configure_diagnostics(
        &self,
        config: DevToolsConfiguration,
        schedule: Rc<dyn Fn()>,
        frame_source: Option<DiagnosticsFrameSource>,
    ) -> Result<(), String> {
        if let Some(previous) = self.diagnostics.borrow_mut().as_mut() {
            previous.stop();
        }
        let graph = self
            .graph
            .get()
            .cloned()
            .ok_or("scene graph is not installed")?;
        // The image registry is shared, so the closure can own it instead of
        // borrowing the scene environment for its whole lifetime.
        let images = self.environment.images().clone();
        let shaping = self
            .environment
            .text()
            .map(|text| text.shaping_cache_snapshot());
        let layout = self
            .environment
            .text()
            .map(|text| text.layout_cache_snapshot());
        let memory = {
            let graph = graph.clone();
            Rc::new(move || DiagnosticMemorySnapshot {
                counters: [
                    (
                        "shapingEntries".to_owned(),
                        shaping.map_or(0, |cache| cache.entries as u64),
                    ),
                    (
                        "shapingBytes".to_owned(),
                        shaping.map_or(0, |cache| cache.estimated_bytes as u64),
                    ),
                    (
                        "layoutEntries".to_owned(),
                        layout.map_or(0, |cache| cache.entries as u64),
                    ),
                    (
                        "layoutBytes".to_owned(),
                        layout.map_or(0, |cache| cache.estimated_bytes as u64),
                    ),
                    ("imageEntries".to_owned(), images.cached_count() as u64),
                    ("imageBytes".to_owned(), images.cached_bytes() as u64),
                    (
                        "skippedParentUpdates".to_owned(),
                        graph.skipped_parent_updates() as u64,
                    ),
                ]
                .into_iter()
                .collect(),
                windows: Default::default(),
            }) as Rc<dyn Fn() -> DiagnosticMemorySnapshot>
        };
        let invalidate = {
            let tree = self.tree.clone();
            Rc::new(move || {
                if let Some(root) = tree.borrow().root() {
                    root.mark_render_dirty(cove_core::scene::InvalidationSource::Unknown);
                }
            })
        };
        let diagnostics = AndroidDiagnostics::new(
            config,
            self.tree.clone(),
            graph,
            schedule,
            invalidate,
            memory,
            AndroidDiagnosticsHooks {
                invalidation_log: self.context.invalidation_log.clone(),
                frame_source,
                mirror_active: {
                    let mirroring = self.mirroring.clone();
                    Rc::new(move |active| mirroring.set(active))
                },
                inject: {
                    let dispatcher = self.dispatcher();
                    Rc::new(move |event| {
                        dispatcher.dispatch(&event);
                    })
                },
            },
        )?;
        *self.diagnostics.borrow_mut() = Some(diagnostics);
        Ok(())
    }

    pub fn pump_diagnostics(&self) {
        self.with_current(|| {
            if let Some(diagnostics) = self.diagnostics.borrow_mut().as_mut() {
                diagnostics.drain();
            }
        });
    }

    pub fn trim_memory(&mut self) {
        if let Some(text) = self.environment.text() {
            text.trim_text_caches();
        }
        self.environment.clear_images();
        // The host owns no ViewGraph render mirror accessor (cove-composition
        // keeps `render_tree` private), so the local mirror is rebuilt lazily
        // on the next frame and its stale caches are dropped here.
        self.render_tree = RenderTree::new();
        self.render_root = None;
        self.draw_list.reset();
        self.needs_update.set(true);
    }

    pub fn needs_frame(&self) -> bool {
        self.needs_update.get()
            || self.mirroring.get()
            || self.recomposer.has_pending()
            || self.tree.borrow().has_render_updates()
            || self.animations.has_active_animations()
            || self.environment.has_work()
    }

    pub fn font_atlas(&self) -> cove_text::FontAtlas {
        self.environment
            .text()
            .expect("Android scene environment has no text environment")
            .atlas
            .clone()
    }

    pub fn focused_id(&self) -> Option<String> {
        self.context.focus_chain.focused().map(|node| {
            node.attachment::<String>(LayoutDebugAttachmentKey::DEBUG_NAME)
                .unwrap_or_else(|| format!("field.{}", node.id()))
        })
    }

    pub fn preedit(&self) -> String {
        self.context
            .focus_chain
            .focused()
            .and_then(|node| node.attachment::<String>("preedit"))
            .unwrap_or_default()
    }

    pub fn editor_snapshot(&self) -> Option<TextInputSnapshot> {
        self.with_current(|| self.text_client().map(|client| (client.snapshot)()))
    }

    fn text_client(&self) -> Option<TextInputClient> {
        self.context
            .focus_chain
            .focused()
            .and_then(|node| node.attachment::<TextInputClient>(TextInputAttachmentKey::CLIENT))
    }

    fn with_current<R>(&self, body: impl FnOnce() -> R) -> R {
        let animations = self.animations.clone();
        animations.with_current(|| {
            self.environment
                .with_current(self.surface.scale(), || self.context.with_current(body))
        })
    }

    /// Prepares one frame. The returned list is an independent copy so the
    /// caller can submit it while the next frame overwrites the owned list.
    pub fn prepare(&mut self, pixels: (i32, i32), scale: f32, delta_time: f64) -> DrawList {
        self.surface.prepare(pixels, scale);
        let scale = self.surface.scale();
        self.environment.configure(scale);

        let tree = self.tree.clone();
        let recomposer = self.recomposer.clone();
        let graph = self
            .graph
            .get()
            .cloned()
            .expect("Android scene graph is not installed");
        let animations = self.animations.clone();
        let environment = &self.environment;
        let context = &self.context;
        let surface = &mut self.surface;
        let layers = &mut self.layers;
        let nodes = &self.nodes;
        let rendering = self.rendering;
        let draw_list = &mut self.draw_list;
        let render_tree = &mut self.render_tree;
        let render_root = &mut self.render_root;
        let render_structure_version = &mut self.render_structure_version;
        let diagnostics = &mut self.diagnostics;
        let needs_update = &self.needs_update;
        let performance_hud = &mut self.performance_hud;
        let (width, height) = (surface.logical_size().0, surface.logical_size().1);

        let frame_start = Instant::now();
        let mut layout_ms = 0.0_f64;
        let mut draw_ms = 0.0_f64;

        animations.with_current(|| {
            environment.with_current(scale, || {
                context.with_current(|| {
                    if let Some(diagnostics) = diagnostics.borrow_mut().as_mut() {
                        diagnostics.drain();
                        diagnostics.begin_frame();
                    }
                    let layout_start = Instant::now();
                    environment.drain();
                    animations.tick(delta_time);
                    recomposer.commit_all();
                    graph.compute_layout_if_needed(width, height);
                    let theme = tree.borrow().root().and_then(|root| surface_theme(&root));
                    surface.update_system_bars(theme.as_ref());
                    layout_ms = layout_start.elapsed().as_secs_f64() * 1000.0;

                    let draw_start = Instant::now();
                    {
                        let tree = tree.borrow();
                        let trace = tree.timeline.borrow().begin();
                        draw_list.reset();
                        draw_list.pixel_scale = scale;
                        if rendering == Rendering::Direct {
                            if let Some(root) = tree.root() {
                                nodes.render(&root, draw_list);
                            }
                        } else if let Some(root) = tree.root() {
                            let version = root.subtree_structure_version();
                            if render_root.as_ref() != Some(&root)
                                || *render_structure_version != version
                            {
                                render_tree.install(&root);
                                *render_root = Some(root.clone());
                                *render_structure_version = version;
                            }
                            layers.borrow_mut().render(render_tree, draw_list);
                        }
                        if let Some(hud) = performance_hud {
                            hud.tick();
                            hud.draw(draw_list);
                        }
                        tree.timeline.borrow_mut().end(
                            trace,
                            "draw",
                            "Encode draw list",
                            None,
                            &[],
                        );
                    }
                    if let Some(diagnostics) = diagnostics.borrow_mut().as_mut() {
                        diagnostics.overlay(draw_list);
                    }
                    draw_ms = draw_start.elapsed().as_secs_f64() * 1000.0;
                    if let Some(diagnostics) = diagnostics.borrow_mut().as_mut() {
                        diagnostics.prepared(layout_ms, draw_ms);
                    }
                });
            });
        });
        let _ = (frame_start, needs_update);

        let mut drawing = DrawList::default();
        let _ = drawing.load(
            self.draw_list.vertices().to_vec(),
            self.draw_list.indices().to_vec(),
            self.draw_list.batches().to_vec(),
            self.draw_list.resources.clone(),
        );
        drawing.pixel_scale = self.draw_list.pixel_scale;
        drawing.set_viewport_bounds(self.draw_list.viewport_bounds);
        drawing
    }

    pub fn did_present(&mut self) {
        if let Some(hud) = &mut self.performance_hud {
            hud.presented();
        }
        let mut tree = self.tree.borrow_mut();
        if let Some(diagnostics) = self.diagnostics.borrow_mut().as_mut() {
            diagnostics.presented(&mut tree, &self.draw_list);
            if diagnostics.wants_mirror_frame() {
                let logical = self.surface.logical_size();
                let scale = self.surface.scale();
                diagnostics.mirror_capture(
                    &self.draw_list,
                    (
                        (logical.0 * scale).round() as u32,
                        (logical.1 * scale).round() as u32,
                    ),
                    logical,
                );
            }
        }
        self.needs_update.set(false);
        tree.flush();
    }

    pub fn invalidate(&self) {
        self.needs_update.set(true);
    }

    pub fn suspend(&self) {
        self.with_current(|| {
            self.context.pointer_capture.release();
            self.context.focus_chain.focus(None, false);
        });
    }

    pub fn pointer(&self, kind: i32, x: f32, y: f32) {
        if !x.is_finite() || !y.is_finite() {
            return;
        }
        let scale = self.surface.scale();
        let point = Point::new(x / scale, y / scale);
        let dispatcher = self.dispatcher();
        self.with_current(|| {
            let button = MouseButtonEvent {
                button: MouseButton::Left,
                x: point.x,
                y: point.y,
                clicks: 1,
                ..MouseButtonEvent::default()
            };
            let event = match kind {
                0 => InputEvent::MouseButtonDown(button.clone()),
                1 => InputEvent::MouseButtonUp(button),
                _ => InputEvent::MouseMotion(MouseMotionEvent {
                    x: point.x,
                    y: point.y,
                    ..MouseMotionEvent::default()
                }),
            };
            let intercepted = self
                .diagnostics
                .borrow_mut()
                .as_mut()
                .is_some_and(|diagnostics| diagnostics.intercept(&event));
            if intercepted {
                self.needs_update.set(true);
                return;
            }
            if kind == 4 {
                self.context.pointer_capture.release();
                let button = MouseButtonEvent {
                    button: MouseButton::Right,
                    x: point.x,
                    y: point.y,
                    clicks: 1,
                    ..MouseButtonEvent::default()
                };
                dispatcher.dispatch(&InputEvent::MouseButtonDown(button.clone()));
                dispatcher.dispatch(&InputEvent::MouseButtonUp(button));
            } else if kind == 5 {
                dispatcher.dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
                    x: point.x,
                    y: point.y,
                    ..MouseMotionEvent::default()
                }));
            } else {
                self.touch.borrow_mut().route(
                    kind,
                    point,
                    &self.tree.borrow(),
                    &self.context,
                    &dispatcher,
                );
            }
        });
        self.needs_update.set(true);
    }

    pub fn text(&self, string: &str, composing: bool) {
        if string.len() > 32768 {
            return;
        }
        let dispatcher = self.dispatcher();
        self.with_current(|| {
            if composing {
                dispatcher.dispatch(&InputEvent::TextEditing(TextEditingEvent::from_utf16(
                    string,
                    0,
                    string.encode_utf16().count() as i32,
                )));
            } else {
                dispatcher.dispatch(&InputEvent::TextInput(string.to_owned()));
            }
        });
        self.needs_update.set(true);
    }

    pub fn key(&self, scancode: u32, modifiers: KeyModifiers) {
        let dispatcher = self.dispatcher();
        self.with_current(|| {
            let event = KeyEvent {
                scancode,
                keycode: 0,
                modifiers,
                is_repeat: false,
            };
            dispatcher.dispatch(&InputEvent::KeyDown(event.clone()));
            dispatcher.dispatch(&InputEvent::KeyUp(event));
        });
        self.needs_update.set(true);
    }

    pub fn edit(&self, kind: i32, start: i32, end: i32) {
        let dispatcher = self.dispatcher();
        self.with_current(|| {
            if kind == 2 || kind == 3 {
                if let Some(actions) = self.context.focus_chain.focused().and_then(|node| {
                    node.attachment::<TextEditActions>(TextInputAttachmentKey::EDIT_ACTIONS)
                }) {
                    (actions.perform)(if kind == 2 {
                        TextEditCommand::Undo
                    } else {
                        TextEditCommand::Redo
                    });
                }
            } else if let Some(client) = self.text_client() {
                if kind == 0 {
                    (client.select)(start, end);
                } else {
                    (client.delete_surrounding)(start, end);
                }
            } else if kind == 1 {
                for _ in 0..start.min(32768).max(0) {
                    let event = KeyEvent {
                        scancode: 42,
                        keycode: 0,
                        modifiers: KeyModifiers::default(),
                        is_repeat: false,
                    };
                    dispatcher.dispatch(&InputEvent::KeyDown(event.clone()));
                    dispatcher.dispatch(&InputEvent::KeyUp(event));
                }
            }
        });
        self.needs_update.set(true);
    }

    pub fn scroll(&self, x: f32, y: f32, delta_x: f32, delta_y: f32) {
        if !x.is_finite() || !y.is_finite() || !delta_x.is_finite() || !delta_y.is_finite() {
            return;
        }
        let scale = self.surface.scale();
        let dispatcher = self.dispatcher();
        self.with_current(|| {
            dispatcher.dispatch(&InputEvent::MouseWheel(MouseWheelEvent {
                x: delta_x,
                y: delta_y,
                mouse_x: Some(x / scale),
                mouse_y: Some(y / scale),
                input: ScrollInput {
                    unit: ScrollUnit::Lines,
                    device: ScrollDevice::Wheel,
                    ..ScrollInput::default()
                },
            }));
        });
        self.needs_update.set(true);
    }
    // `inspect()` — the explicit diagnostic snapshot — lives in `inspection.rs`,
    // mirroring the Swift extension on `HostedScene`.
}
