//! Translation of Desktop/Sources/CoveUI/Application/Composition.swift.

use cove_composition::{AnyView, ViewGraph};
use cove_core::scene::{LayerAwareNodeRenderer, NodeRenderer, NodeTree, Recomposer};
use cove_core::{DrawList, Environment, State};
use cove_imaging::ImageAssetRegistry;
use cove_ui_compose::primitives::overlays::portal::portal_registry::PortalStore;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Scoped asset drop registry.
///
/// The Swift type tracked drag-and-drop targets per scene. `cove-core` does not
/// expose a drop registry, so the host keeps the per-window slot the pieces are
/// registered into; a drag session asks this registry which node accepted it.
#[derive(Default)]
pub struct AssetDropRegistry {
    active_target: RefCell<Option<String>>,
}

impl AssetDropRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_active_target(&self, target: Option<String>) {
        *self.active_target.borrow_mut() = target;
    }

    pub fn active_target(&self) -> Option<String> {
        self.active_target.borrow().clone()
    }
}

/// The process-global drop registry slot, mirroring `AssetDropRegistryHolder`.
pub struct AssetDropRegistryHolder;

thread_local! {
    static CURRENT_DROP_REGISTRY: RefCell<Option<Rc<AssetDropRegistry>>> = const { RefCell::new(None) };
    static CURRENT_DISPLAY_HANDLE: RefCell<Option<crate::desktop::display::AppDisplayHandle>> =
        const { RefCell::new(None) };
}

impl AssetDropRegistryHolder {
    pub fn current() -> Option<Rc<AssetDropRegistry>> {
        CURRENT_DROP_REGISTRY.with(|slot| slot.borrow().clone())
    }

    pub fn set_current(registry: Option<Rc<AssetDropRegistry>>) {
        CURRENT_DROP_REGISTRY.with(|slot| *slot.borrow_mut() = registry);
    }
}

/// Holds the display handle application content drives its window through.
pub struct AppDisplayHandleHolder;

impl AppDisplayHandleHolder {
    pub fn current() -> Option<crate::desktop::display::AppDisplayHandle> {
        CURRENT_DISPLAY_HANDLE.with(|slot| slot.borrow().clone())
    }

    pub fn set_current(handle: Option<crate::desktop::display::AppDisplayHandle>) {
        CURRENT_DISPLAY_HANDLE.with(|slot| *slot.borrow_mut() = handle);
    }
}

/// UI resources shared by the main window and every auxiliary window.
pub struct MainComposition {
    pub tree: Rc<RefCell<NodeTree>>,
    pub environment: State<Environment>,
    pub portal_store: Rc<PortalStore>,
    pub graph: Rc<ViewGraph>,
    pub image_assets: std::sync::Arc<ImageAssetRegistry>,
    pub asset_drop_registry: Rc<AssetDropRegistry>,
    pub draw_list: RefCell<DrawList>,
    pub layer_renderer: RefCell<LayerAwareNodeRenderer>,
    pub node_renderer: NodeRenderer,
    pub use_legacy_renderer: bool,
    /// Set whenever the composition produced a new draw list.
    pub draw_revision: Cell<u64>,
}

impl MainComposition {
    pub fn new(recomposer: Rc<Recomposer>, portal_store: Rc<PortalStore>) -> Self {
        let tree = Rc::new(RefCell::new(NodeTree::new()));
        let graph = Rc::new(ViewGraph::new(tree.clone(), recomposer));
        let use_legacy_renderer = std::env::var("COVEUI_LEGACY_RENDERER").as_deref() == Ok("1");
        Self {
            tree,
            environment: State::new(Environment::default()),
            portal_store,
            graph,
            image_assets: std::sync::Arc::new(ImageAssetRegistry::default()),
            asset_drop_registry: Rc::new(AssetDropRegistry::new()),
            draw_list: RefCell::new(DrawList::default()),
            layer_renderer: RefCell::new(LayerAwareNodeRenderer::new()),
            node_renderer: NodeRenderer::new(),
            use_legacy_renderer,
            draw_revision: Cell::new(0),
        }
    }

    /// Applies the application's image cache limits to the shared registry.
    pub fn apply_image_limits(&self, limits: cove_imaging::ImageCacheLimits) {
        self.image_assets.set_limits(limits);
    }

    /// Installs the root view and gives it one layout pass.
    pub fn install_root(
        &self,
        view: impl cove_composition::View + 'static,
        width: f32,
        height: f32,
    ) {
        self.graph.install(
            view,
            cove_core::scene::ComponentSourceLocation::new(
                "cove-ui",
                "Hosts/rust/desktop/application/composition.rs",
                0,
            ),
        );
        self.graph.compute_layout(width, height);
    }

    /// Installs a pre-erased root view, used by auxiliary windows.
    pub fn install_view(&self, view: AnyView, width: f32, height: f32) {
        self.graph.install_view(
            view,
            cove_core::scene::ComponentSourceLocation::new(
                "cove-ui",
                "Hosts/rust/desktop/application/composition.rs",
                0,
            ),
        );
        self.graph.compute_layout(width, height);
    }

    /// Drops heavy CPU caches so a backgrounded application actually reclaims
    /// memory.
    pub fn trim_memory(&self) {
        self.image_assets.clear();
        self.graph.render_tree().trim_caches();
        self.draw_list.borrow_mut().reset();
    }
}
