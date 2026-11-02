//! Translation of Desktop/Sources/CoveUI/Window/Auxiliary.swift.
//!
//! An auxiliary window owns its own tree, view graph, portal store and
//! presentation surface, sharing only the text/image services and draw-list
//! renderer of the application.

use crate::desktop::application::composition::MainComposition;
use crate::desktop::application::configuration::{
    AppWindowRendering, AppWindowTitleBarStyle, ApplicationConfiguration,
};
use crate::desktop::application::memory::AppWindowMemorySnapshot;
use crate::desktop::application::typography::AdaptiveApplicationContent;
use crate::desktop::rendering::context::AppRenderingContext;
use crate::desktop::window::chrome::{
    self, AppWindowChromeContextHolder, WindowChromeHitTestCollector,
};
use crate::desktop::window::native_renderer::AppWindowRenderer;
use cove_composition::{AnyView, ViewGraph};
use cove_core::scene::{LayerAwareNodeRenderer, NodeRenderer, NodeTree, Recomposer};
use cove_core::{DrawList, Environment, State};
use cove_platform::WindowId;
use cove_runtime::{NativeAccessibility, PlatformWindowSession, Sdl3PlatformHost};
use cove_shell::{NativeRenderSurface, WindowChromeHitTest};
use cove_ui_compose::primitives::overlays::portal::portal_registry::{
    PortalStore, PortalStoreAmbient,
};
use std::cell::RefCell;
use std::rc::Rc;

/// The platform style for an authored title-bar style.
pub fn platform_style(style: AppWindowTitleBarStyle) -> cove_shell::WindowTitleBarStyle {
    style.platform_style()
}

pub struct AuxiliaryAppWindow {
    session: Rc<PlatformWindowSession>,
    graph: Rc<ViewGraph>,
    root_view: AnyView,
    portal_store: Rc<PortalStore>,
    rendering: Rc<AppRenderingContext>,
    window_renderer: Option<Box<dyn AppWindowRenderer>>,
    config: ApplicationConfiguration,
    window_rendering: AppWindowRendering,
    use_legacy_renderer: bool,
    draw_list: RefCell<DrawList>,
    layer_renderer: RefCell<LayerAwareNodeRenderer>,
    node_renderer: NodeRenderer,
    is_vsync_enabled: bool,
    logical_w: u32,
    logical_h: u32,
    did_install_root: bool,
    window_chrome_hit_test_collector: RefCell<WindowChromeHitTestCollector>,
    last_window_chrome_hit_test: RefCell<Option<WindowChromeHitTest>>,
    set_window_chrome_hit_test: Rc<dyn Fn(WindowId, Option<&WindowChromeHitTest>)>,
    environment: State<Environment>,
}

impl AuxiliaryAppWindow {
    #[allow(
        clippy::too_many_arguments,
        reason = "translated window constructor carries the shared services and per-window rendering overrides"
    )]
    pub fn new(
        session: Rc<PlatformWindowSession>,
        root_view: AnyView,
        portal_store: Rc<PortalStore>,
        rendering: Rc<AppRenderingContext>,
        config: ApplicationConfiguration,
        window_rendering: AppWindowRendering,
        is_vsync_enabled: bool,
        use_legacy_renderer: bool,
        set_window_chrome_hit_test: Rc<dyn Fn(WindowId, Option<&WindowChromeHitTest>)>,
    ) -> Self {
        let mut draw_list = DrawList::default();
        draw_list.geometry_antialiasing = window_rendering
            .geometry_antialiasing
            .unwrap_or(config.rendering.geometry_antialiasing);
        let graph = Rc::new(ViewGraph::new(
            session.tree.clone(),
            session.recomposer.clone(),
        ));
        Self {
            session,
            graph,
            root_view,
            portal_store,
            rendering,
            window_renderer: None,
            window_rendering,
            use_legacy_renderer,
            config,
            draw_list: RefCell::new(draw_list),
            layer_renderer: RefCell::new(LayerAwareNodeRenderer::new()),
            node_renderer: NodeRenderer::new(),
            is_vsync_enabled,
            logical_w: 0,
            logical_h: 0,
            did_install_root: false,
            window_chrome_hit_test_collector: RefCell::new(WindowChromeHitTestCollector::new()),
            last_window_chrome_hit_test: RefCell::new(None),
            set_window_chrome_hit_test,
            environment: State::new(Environment::default()),
        }
    }

    pub fn window_id(&self) -> WindowId {
        self.session.id
    }

    pub fn session(&self) -> &Rc<PlatformWindowSession> {
        &self.session
    }

    pub fn handle_init(
        &mut self,
        native: NativeRenderSurface,
        width_px: u32,
        height_px: u32,
        configure_text_environment: &dyn Fn(f32),
        upload_atlas_if_needed: &dyn Fn(bool) -> Result<(), String>,
    ) -> Result<(), String> {
        let logical = self.session.logical_size();
        self.logical_w = logical.0;
        self.logical_h = logical.1;

        let settings = crate::desktop::window::native_renderer::AppWindowRenderSettings::resolve(
            &self.config,
            self.window_rendering,
        );
        let mut renderer = self
            .rendering
            .make_window(settings)
            .map_err(|error| error.to_string())?;
        renderer
            .configure(native, (width_px, height_px), self.is_vsync_enabled)
            .map_err(|error| error.to_string())?;
        self.window_renderer = Some(renderer);

        let window_id = self.session.id;
        let graph = self.graph.clone();
        let root_view = self.root_view.clone();
        let environment = self.environment.clone();
        let logical_w = self.logical_w;
        let logical_h = self.logical_h;
        let did_install_root = self.did_install_root;
        let session = self.session.clone();
        // Scope this window's overlay store for every consumer that reads the
        // process ambient, so one window's popover cannot paint into another's.
        let ambient: Rc<dyn cove_core::scene::ScopedAmbient> =
            Rc::new(PortalStoreScope::new(self.portal_store.clone()));
        session.input_context.add_scoped_ambient(ambient);
        session.with_current(|| {
            configure_text_environment(session.content_scale_factor());
            if !did_install_root {
                chrome::with_window_chrome_context(Some(window_id), || {
                    let content = AdaptiveApplicationContent::new(root_view, environment);
                    graph.install_view(
                        AnyView::new(content),
                        cove_core::scene::ComponentSourceLocation::new(
                            "cove-ui",
                            "crates/hosts/src/desktop/window/auxiliary.rs",
                            0,
                        ),
                    );
                });
                graph.compute_layout(logical_w as f32, logical_h as f32);
                self.sync_window_chrome_hit_test();
                self.did_install_root = true;
            }
            let _ = upload_atlas_if_needed(false);
        });

        session.request_display();
        Ok(())
    }

    /// Releases this window's GPU surface before the shell destroys its native
    /// window.
    pub fn release_gpu_surface(&mut self) {
        if let Some(mut renderer) = self.window_renderer.take() {
            let _ = renderer.close();
        }
    }

    pub fn set_presentation_active(&mut self, active: bool) -> Result<(), String> {
        match &mut self.window_renderer {
            Some(renderer) => renderer
                .set_presentation_active(active)
                .map_err(|e| e.to_string()),
            None => Ok(()),
        }
    }

    pub fn memory_snapshot(&self) -> Option<AppWindowMemorySnapshot> {
        self.window_renderer
            .as_ref()
            .map(|renderer| renderer.memory_snapshot())
    }

    pub fn trim_memory(&self) {
        self.graph.render_tree().trim_caches();
        self.draw_list.borrow_mut().reset();
    }

    pub fn handle_resize(
        &mut self,
        width_px: u32,
        height_px: u32,
        configure_text_environment: &dyn Fn(f32),
        upload_atlas_if_needed: &dyn Fn(bool) -> Result<(), String>,
    ) -> Result<(), String> {
        let logical = self.session.logical_size();
        self.logical_w = logical.0;
        self.logical_h = logical.1;
        if let Some(renderer) = &mut self.window_renderer {
            renderer
                .resize((width_px, height_px), self.is_vsync_enabled)
                .map_err(|error| error.to_string())?;
        }
        let session = self.session.clone();
        session.with_current(|| {
            configure_text_environment(session.content_scale_factor());
            let _ = upload_atlas_if_needed(false);
        });
        Ok(())
    }

    pub fn set_vsync_enabled(&mut self, enabled: bool) {
        self.is_vsync_enabled = enabled;
        let size = self.session.drawable_size();
        if let Some(renderer) = &mut self.window_renderer {
            let _ = renderer.resize(size, enabled);
        }
        self.session.request_display();
    }

    fn sync_window_chrome_hit_test(&self) {
        let tree = self.session.tree.borrow();
        let Some(root) = tree.root() else { return };
        let next = self
            .window_chrome_hit_test_collector
            .borrow_mut()
            .collect(&root);
        let mut last = self.last_window_chrome_hit_test.borrow_mut();
        match next {
            Some(next) => {
                if last.as_ref() == Some(&next) {
                    return;
                }
                (self.set_window_chrome_hit_test)(self.session.id, Some(&next));
                *last = Some(next);
            }
            None => {
                if last.is_some() {
                    (self.set_window_chrome_hit_test)(self.session.id, None);
                    *last = None;
                }
            }
        }
    }

    pub fn handle_frame(
        &mut self,
        host: &Sdl3PlatformHost,
        configure_text_environment: &dyn Fn(f32),
        upload_atlas_if_needed: &dyn Fn(bool) -> Result<(), String>,
    ) -> bool {
        let Some(renderer) = &self.window_renderer else {
            return false;
        };
        if !renderer.is_configured() {
            return false;
        }
        let (root, timeline) = {
            let tree = self.session.tree.borrow();
            let Some(root) = tree.root() else {
                return false;
            };
            (root, tree.timeline.clone())
        };

        let logical = (self.logical_w, self.logical_h);
        let session = self.session.clone();
        let graph = self.graph.clone();
        let draw_list = &self.draw_list;
        let layer_renderer = &self.layer_renderer;
        let node_renderer = &self.node_renderer;
        let use_legacy = self.use_legacy_renderer;
        session.with_current(|| {
            configure_text_environment(session.content_scale_factor());
            let _ = graph.compute_layout_if_needed(logical.0 as f32, logical.1 as f32);
            self.sync_window_chrome_hit_test();
            NativeAccessibility::synchronize(host, &session, &root, &timeline);
            let mut list = draw_list.borrow_mut();
            list.reset();
            list.pixel_scale = session.content_scale_factor();
            if use_legacy {
                node_renderer.render(&root, &mut list);
            } else {
                layer_renderer
                    .borrow_mut()
                    .render(graph.render_tree(), &mut list);
            }
        });

        if upload_atlas_if_needed(false).is_err() {
            return false;
        }

        let mut list = self.draw_list.borrow_mut();
        let Some(renderer) = &mut self.window_renderer else {
            return false;
        };
        match renderer.draw(&mut list, (logical.0 as f32, logical.1 as f32)) {
            Ok(presented) => {
                if !presented {
                    self.session.request_display();
                }
                presented
            }
            Err(_) => false,
        }
    }
}

/// Adapts `PortalStoreAmbient` to the runtime's scoped-ambient hook.
///
/// `PortalStoreAmbient` exposes `activate`; the runtime trait needs that closure
/// boxed as `FnOnce`, so this host-side adapter supplies it without editing
/// Compose.
pub struct PortalStoreScope {
    store: Rc<PortalStore>,
}

impl PortalStoreScope {
    pub fn new(store: Rc<PortalStore>) -> Self {
        Self { store }
    }
}

impl cove_core::scene::ScopedAmbient for PortalStoreScope {
    fn activate(&self) -> Box<dyn FnOnce()> {
        let restore = PortalStoreAmbient::new(self.store.clone()).activate();
        Box::new(move || restore())
    }
}

/// Shared composition state an auxiliary window needs from the application.
pub struct AuxiliaryWindowServices {
    pub composition: Rc<MainComposition>,
    pub rendering: Rc<AppRenderingContext>,
    pub config: ApplicationConfiguration,
}

impl AuxiliaryWindowServices {
    pub fn new(
        composition: Rc<MainComposition>,
        rendering: Rc<AppRenderingContext>,
        config: ApplicationConfiguration,
    ) -> Self {
        Self {
            composition,
            rendering,
            config,
        }
    }

    /// Builds a fresh tree/recomposer/portal-store trio for a new window, as
    /// `openAuxiliaryWindow` did.
    pub fn make_window_state(&self) -> (Rc<RefCell<NodeTree>>, Rc<Recomposer>, Rc<PortalStore>) {
        let _ = &self.composition;
        (
            Rc::new(RefCell::new(NodeTree::new())),
            Rc::new(Recomposer::new()),
            Rc::new(PortalStore::new()),
        )
    }
}

/// Runs `body` with the auxiliary window's chrome context installed.
pub fn with_chrome<R>(window_id: WindowId, body: impl FnOnce() -> R) -> R {
    chrome::with_window_chrome_context(Some(window_id), body)
}

/// Clears the chrome context after a window closes.
pub fn clear_chrome_context() {
    AppWindowChromeContextHolder::set_current(None);
}
