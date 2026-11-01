//! Shared scene harness for the gallery and counter suites.
//!
//! The Swift suites under `examples/gallery/Tests/GalleryContentTests` and
//! `examples/gallery/Counter/Tests` each opened with the same boilerplate: install a
//! bootstrapped text environment, scope a fresh `AnimatorScheduler`, install a
//! fresh interaction/focus/capture/portal ambient, build a `ViewGraph`, settle
//! it, and tear the graph down on exit. This module is that boilerplate once.
//!
//! `GlobalTestLock` serializes every suite, because the holders behind
//! `TextEnvironmentHolder`, `InteractionRegistryHolder`, `FocusChainHolder`,
//! `PointerCaptureHolder` and `PortalStoreHolder` are process-wide thread-locals
//! and Rust runs `#[test]` functions on separate threads. It is the same lock
//! the Compose suites use, and it is what makes the Swift `.serialized` trait a
//! no-op here.

use std::rc::Rc;
use std::sync::{Mutex, MutexGuard, OnceLock};

use cove_composition::{LayoutDebugAttachmentKey, PrimitiveView, View, ViewGraph, ViewKind};
use cove_core::scene::{
    AnimatorScheduler, EventDispatcher, FocusChain, InteractionRegistry, Node, NodeTree,
    PlatformInputContext, PointerCapture, Recomposer, ScopedAmbient,
};
use cove_core::{Environment, Point};
use cove_platform::{InputEvent, KeyEvent, KeyModifiers, MouseButton, MouseButtonEvent, WindowId};
use cove_text::{SystemFontDefaults, TextBootstrapOptions, TextEnvironment, TextEnvironmentHolder};
use cove_ui_compose::primitives::overlays::portal::PortalStore;

/// Scopes a portal store with the window context.
///
/// `PortalStoreAmbient` now implements `ScopedAmbient` and is re-exported, so
/// this is a thin alias rather than a reimplementation.
type ScopedPortalStore = cove_ui_compose::primitives::overlays::portal::PortalStoreAmbient;

/// Process-wide serialization for every suite that touches a shared holder.
pub struct GlobalTestLock;

impl GlobalTestLock {
    fn mutex() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    /// Runs `body` with exclusive access to the shared holders.
    ///
    /// A poisoned lock is recovered: one failing test must not cascade into
    /// every later one, and the scoped guard restores the holders regardless.
    ///
    /// Re-entrant on one thread: a helper that takes the lock may run inside a
    /// test that already holds it, instead of deadlocking.
    pub fn locked<T>(body: impl FnOnce() -> T) -> T {
        thread_local! {
            static HELD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        }
        if HELD.get() {
            return body();
        }
        let _guard: MutexGuard<'_, ()> = Self::mutex()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        struct Release;
        impl Drop for Release {
            fn drop(&mut self) {
                HELD.set(false);
            }
        }
        HELD.set(true);
        let _release = Release;
        body()
    }

    /// Runs `body` with exclusive access and a fresh scheduler installed,
    /// matching `AnimatorScheduler.$current.withValue(AnimatorScheduler())`.
    pub fn with_scheduler<T>(body: impl FnOnce() -> T) -> T {
        Self::locked(|| AnimatorScheduler::default().with_current(body))
    }
}

/// Installs a bootstrapped text environment for the duration of `body`.
///
/// Swift wrote this as
/// `TextEnvironment.bootstrapped(atlasTextureID: 1, primaryFontName: SystemFontDefaults.primaryFontName)`.
pub fn with_text_environment<T>(body: impl FnOnce() -> T) -> T {
    let previous = TextEnvironmentHolder::current();
    TextEnvironmentHolder::set_current(Some(TextEnvironment::bootstrapped(TextBootstrapOptions {
        atlas_texture_id: 1,
        primary_font_name: SystemFontDefaults::primary_font_name(),
        ..TextBootstrapOptions::default()
    })));
    let result = body();
    TextEnvironmentHolder::set_current(previous);
    result
}

/// One mounted gallery scene with private, scoped runtime ambients.
///
/// Mirrors the Swift `scene { gallery, graph, context, input in ... }` helper:
/// a fresh `PlatformInputContext` and `PortalStore`, a fresh `AnimatorScheduler`,
/// and a fresh `ViewGraph` torn down on exit.
pub struct GalleryScene {
    pub graph: Rc<ViewGraph>,
    pub interactions: Rc<InteractionRegistry>,
    pub focus: Rc<FocusChain>,
    pub capture: Rc<PointerCapture>,
    pub scheduler: AnimatorScheduler,
    pub width: f32,
    pub height: f32,
}

impl GalleryScene {
    /// Runs `body` inside a fully-installed scene and tears it down afterwards.
    ///
    /// `install` receives the scene so the caller can mount its own root; every
    /// path runs the Swift `defer { graph.install(EmptyView); scheduler.reset() }`
    /// cleanup.
    pub fn run(
        width: f32,
        height: f32,
        install: impl FnOnce(&GalleryScene) -> cove_composition::AnyView,
        body: impl FnOnce(&GalleryScene),
    ) {
        GlobalTestLock::with_scheduler(|| {
            with_text_environment(|| {
                let context = PlatformInputContext::new();
                let scheduler = AnimatorScheduler::default();
                let portals = Rc::new(PortalStore::new());
                context.add_scoped_ambient(Rc::new(ScopedPortalStore::new(portals.clone())));
                let (interactions, focus, capture) = (
                    context.interactions.clone(),
                    context.focus_chain.clone(),
                    context.pointer_capture.clone(),
                );
                context.with_current(|| {
                    scheduler.with_current(|| {
                        let graph = Rc::new(ViewGraph::new(
                            Rc::new(std::cell::RefCell::new(NodeTree::new())),
                            Rc::new(Recomposer::new()),
                        ));
                        let scene = GalleryScene {
                            graph: graph.clone(),
                            interactions,
                            focus,
                            capture,
                            scheduler: scheduler.clone(),
                            width,
                            height,
                        };
                        scene
                            .graph
                            .install_view(install(&scene), Default::default());
                        scene.settle();
                        body(&scene);
                        scene
                            .graph
                            .install(cove_composition::EmptyView, Default::default());
                        scene.scheduler.reset();
                    });
                });
            });
        });
    }

    /// Recomposes, lays out and commits `count` times at the scene size.
    pub fn settle_n(&self, count: usize) {
        for _ in 0..count {
            self.graph.recomposer.commit_all();
            self.graph.compute_layout(self.width, self.height);
        }
    }

    /// The 8-iteration settle most gallery suites used.
    pub fn settle(&self) {
        self.settle_n(8);
    }

    pub fn dispatcher(&self) -> EventDispatcher {
        EventDispatcher::new(
            self.graph.tree.clone(),
            self.interactions.clone(),
            self.capture.clone(),
            self.focus.clone(),
            WindowId::MAIN,
        )
    }

    pub fn nodes(&self) -> Vec<Node> {
        self.graph
            .tree
            .borrow()
            .root()
            .map(|root| collect(&root))
            .unwrap_or_default()
    }

    pub fn named(&self, name: &str) -> Option<Node> {
        self.nodes()
            .into_iter()
            .find(|node| debug_name(node).as_deref() == Some(name))
    }

    /// The node named `name`, or a panic quoting which names did exist.
    pub fn require_named(&self, name: &str) -> Node {
        self.named(name).unwrap_or_else(|| {
            let mut available: Vec<String> = self.nodes().iter().filter_map(debug_name).collect();
            available.sort();
            available.dedup();
            panic!("no node named {name:?}; present debug names: {available:?}")
        })
    }

    pub fn first(&self, predicate: impl Fn(&Node) -> bool) -> Option<Node> {
        self.nodes().into_iter().find(|node| predicate(node))
    }

    pub fn all(&self, predicate: impl Fn(&Node) -> bool) -> Vec<Node> {
        self.nodes()
            .into_iter()
            .filter(|node| predicate(node))
            .collect()
    }

    pub fn click(&self, node: &Node) {
        let frame = node.absolute_frame();
        let event = MouseButtonEvent {
            button: MouseButton::Left,
            x: frame.x + frame.width / 2.0,
            y: frame.y + frame.height / 2.0,
            clicks: 1,
            modifiers: Default::default(),
            kind: Default::default(),
        };
        let dispatcher = self.dispatcher();
        dispatcher.dispatch(&InputEvent::MouseButtonDown(event.clone()));
        dispatcher.dispatch(&InputEvent::MouseButtonUp(event));
    }

    pub fn click_at(&self, point: Point) {
        let event = MouseButtonEvent {
            button: MouseButton::Left,
            x: point.x,
            y: point.y,
            clicks: 1,
            modifiers: Default::default(),
            kind: Default::default(),
        };
        let dispatcher = self.dispatcher();
        dispatcher.dispatch(&InputEvent::MouseButtonDown(event.clone()));
        dispatcher.dispatch(&InputEvent::MouseButtonUp(event));
    }

    pub fn key(&self, scancode: u32) {
        self.key_with(scancode, KeyModifiers::default());
    }

    pub fn key_with(&self, scancode: u32, modifiers: KeyModifiers) {
        self.dispatcher().dispatch(&InputEvent::KeyDown(KeyEvent {
            scancode,
            keycode: 0,
            modifiers,
            is_repeat: false,
        }));
    }

    pub fn text_input(&self, text: &str) {
        self.dispatcher()
            .dispatch(&InputEvent::TextInput(text.to_owned()));
    }

    /// Every text run the layout nodes carry, in tree order.
    pub fn texts(&self) -> Vec<String> {
        self.nodes()
            .into_iter()
            .filter_map(|node| self.graph.layout_node(&node))
            .filter_map(|layout| layout.text_inputs().map(|inputs| inputs.text))
            .collect()
    }

    /// Whether a recomposition is still queued.
    pub fn recomposer_has_pending(&self) -> bool {
        self.graph.recomposer.has_pending()
    }

    /// The debug name of the node carrying `text` as its layout text run.
    pub fn has_text(&self, text: &str) -> bool {
        self.texts().iter().any(|value| value == text)
    }
}

/// Depth-first node walk, root first.
pub fn collect(node: &Node) -> Vec<Node> {
    let mut collected = vec![node.clone()];
    for child in node.children() {
        collected.extend(collect(&child));
    }
    collected
}

/// The layout debug name recorded by `.debug_name(_)`.
pub fn debug_name(node: &Node) -> Option<String> {
    node.attachment::<String>(LayoutDebugAttachmentKey::DEBUG_NAME)
}

/// Every accessibility label in the tree, in tree order.
pub fn labels(scene: &GalleryScene) -> Vec<String> {
    scene
        .nodes()
        .into_iter()
        .filter_map(|node| node.accessibility())
        .map(|semantics| semantics.label)
        .filter(|label| !label.is_empty())
        .collect()
}

/// A primitive leaf with a fixed size and an optional debug name.
///
/// The Swift suites defined local `_PrimitiveView` leaves for their diagnostic
/// tests; this is the one shape they all needed.
pub struct FixedLeaf {
    pub name: Option<&'static str>,
    pub width: f32,
    pub height: f32,
}

impl FixedLeaf {
    pub fn new(name: Option<&'static str>, width: f32, height: f32) -> Self {
        Self {
            name,
            width,
            height,
        }
    }
}

impl PrimitiveView for FixedLeaf {
    fn make_node(&self) -> Node {
        Node::new()
    }
    fn update_node(&self, node: &Node) {
        node.set_is_hit_testable(false);
        if let Some(name) = self.name {
            node.set_attachment(LayoutDebugAttachmentKey::DEBUG_NAME, name.to_owned());
        }
    }
    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        let layout = cove_core::scene::LayoutNode::new();
        layout.set_width(Some(self.width));
        layout.set_height(Some(self.height));
        Some(layout)
    }
}

impl View for FixedLeaf {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// The default `Environment` with a viewport and input capabilities set.
pub fn environment_with(
    width: f32,
    height: f32,
    input: cove_core::InputCapabilities,
) -> Environment {
    let mut environment = Environment::default();
    environment.viewport.width = width;
    environment.viewport.height = height;
    environment.input = input;
    environment
}

/// Placeholder so `ScopedAmbient` stays exercised by the harness's contract.
#[allow(dead_code)]
fn _assert_scoped_ambient_is_used(_: &dyn ScopedAmbient) {}
