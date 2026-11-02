//! Translation of `Tests/CoveUIComposeTests/TestSupport.swift`.
//!
//! Compose tests build view graphs and mutate the process-wide holders
//! (`InteractionRegistryHolder`, `FocusChainHolder`, `TextEnvironmentHolder`),
//! so they must not run concurrently with each other. Rust runs `#[test]`
//! functions on separate threads by default, so every such test takes
//! [`GlobalTestLock`] for the whole body and the crate's serialized suites are
//! driven through it.
//!
//! The Swift version also scopes a fresh `AnimatorScheduler` per lock scope;
//! [`GlobalTestLock::with_scene`] reproduces that.

use std::path::Path;
use std::sync::{Mutex, MutexGuard, OnceLock};

use cove_composition::{LayoutDebugAttachmentKey, View, ViewGraph};
use cove_core::scene::{
    AccessibilityRole, AnimatorScheduler, EventDispatcher, FocusChain, FocusChainHolder,
    InteractionRegistry, InteractionRegistryHolder, Node, NodeTree, PointerCapture,
    PointerCaptureHolder, Recomposer,
};
use std::rc::Rc;

use cove_core::{Font, FontWeight, Point};
use cove_platform::{
    InputEvent, KeyEvent, KeyModifiers, MouseButton, MouseButtonEvent, MouseMotionEvent, WindowId,
};
use cove_text::TextCacheLimits;
use cove_text::{FontAtlas, TextEnvironment, TextEnvironmentHolder, TextFontResolver, TextShaper};
use cove_ui_compose::primitives::overlays::portal::{PortalStore, PortalStoreHolder};

/// Cross-platform test font resolved from the host's system fonts. CoveUI no
/// longer bundles a font, so the assertions below are font-agnostic.
pub fn system_test_font_path() -> Option<&'static str> {
    static PATH: OnceLock<Option<&'static str>> = OnceLock::new();
    *PATH.get_or_init(|| {
        const CANDIDATES: [&str; 8] = [
            r"C:\Windows\Fonts\segoeui.ttf",
            r"C:\Windows\Fonts\arial.ttf",
            "/System/Library/Fonts/Helvetica.ttc",
            "/Library/Fonts/Arial.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
        ];
        CANDIDATES
            .into_iter()
            .find(|candidate| Path::new(candidate).is_file())
    })
}

/// Process-wide serialization for tests that touch shared holders or caches.
pub struct GlobalTestLock;

impl GlobalTestLock {
    fn mutex() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    /// Runs `body` with exclusive access to the shared holders.
    ///
    /// A poisoned lock is recovered: a panicking test must not cascade into
    /// every later test, and the holders are restored by the guard regardless.
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

    /// Runs `body` with exclusive access and a fresh animation scheduler
    /// installed for the duration, matching `$current.withValue`.
    pub fn with_scheduler<T>(body: impl FnOnce() -> T) -> T {
        Self::locked(|| AnimatorScheduler::default().with_current(body))
    }
}

/// The scene handles a test needs to dispatch input, mirroring the Swift
/// `withScene` helper's arguments.
pub struct TestScene {
    pub graph: Rc<ViewGraph>,
    pub interactions: std::rc::Rc<InteractionRegistry>,
    pub focus_chain: std::rc::Rc<FocusChain>,
    pub pointer_capture: std::rc::Rc<PointerCapture>,
}

impl TestScene {
    /// Builds a graph with private holders installed for the closure's scope.
    ///
    /// The holders are process-wide thread-locals, so the caller must already
    /// hold [`GlobalTestLock`] — [`Self::run`] does that.
    pub fn new() -> Self {
        let interactions = std::rc::Rc::new(InteractionRegistry::new());
        let focus_chain = std::rc::Rc::new(FocusChain::new());
        let pointer_capture = std::rc::Rc::new(PointerCapture::new());
        InteractionRegistryHolder::set_current(Some(interactions.clone()));
        FocusChainHolder::set_current(Some(focus_chain.clone()));
        PointerCaptureHolder::set_current(Some(pointer_capture.clone()));
        let graph = std::rc::Rc::new(ViewGraph::new(
            std::rc::Rc::new(std::cell::RefCell::new(NodeTree::new())),
            std::rc::Rc::new(Recomposer::new()),
        ));
        Self {
            graph,
            interactions,
            focus_chain,
            pointer_capture,
        }
    }

    /// Runs `body` under the global lock with fresh holders and a fresh
    /// scheduler, then tears the graph down and ticks once like Swift does.
    pub fn run(body: impl FnOnce(&mut TestScene)) {
        GlobalTestLock::with_scheduler(|| {
            let mut scene = Self::new();
            body(&mut scene);
            scene
                .graph
                .install(cove_composition::EmptyView, Default::default());
            AnimatorScheduler::current().tick(1.0);
            InteractionRegistryHolder::set_current(None);
            FocusChainHolder::set_current(None);
            PointerCaptureHolder::set_current(None);
        });
    }

    /// Every node in the graph, depth first.
    pub fn nodes(&self) -> Vec<Node> {
        let mut collected = Vec::new();
        if let Some(root) = self.graph.tree.borrow().root() {
            collect(&root, &mut collected);
        }
        collected
    }

    /// The node carrying `name` as its layout debug name.
    pub fn named(&self, name: &str) -> Option<Node> {
        self.nodes()
            .into_iter()
            .find(|node| debug_name(node).as_deref() == Some(name))
    }
}

/// A graph with private holders installed, for tests that do not dispatch.
pub fn graph() -> std::rc::Rc<ViewGraph> {
    std::rc::Rc::new(ViewGraph::new(
        std::rc::Rc::new(std::cell::RefCell::new(NodeTree::new())),
        std::rc::Rc::new(Recomposer::new()),
    ))
}

/// The layout debug name recorded by `.debug_name(_)`.
pub fn debug_name(node: &Node) -> Option<String> {
    node.attachment::<String>(LayoutDebugAttachmentKey::DEBUG_NAME)
}

fn collect(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        collect(&child, into);
    }
}

/// Builds a text environment backed by a host font, matching
/// `TestTextEnvironmentFactory.make`.
pub fn test_text_environment(size: f32, line_height: f32) -> TextEnvironment {
    let atlas = FontAtlas::new(512, 512);
    if let Some(path) = system_test_font_path() {
        atlas.load_font_path(Path::new(path), size, 1.0);
    }
    let mut shaper = TextShaper::default();
    if let Some(face) = atlas.font_face() {
        shaper.set_face(face, size);
    }
    let resolver = TextFontResolver::new("Arial".to_string(), atlas.clone(), 1.0);
    TextEnvironment::new(
        atlas,
        shaper,
        1,
        line_height,
        cove_core::Color::WHITE,
        Some(Font::system(size, FontWeight::Regular)),
        Some(resolver),
        TextCacheLimits::default(),
    )
}

/// Installs a test text environment for the duration of `body`.
pub fn with_test_text_environment<T>(body: impl FnOnce() -> T) -> T {
    let previous = TextEnvironmentHolder::current();
    TextEnvironmentHolder::set_current(Some(test_text_environment(16.0, 20.0)));
    let result = body();
    TextEnvironmentHolder::set_current(previous);
    result
}

/// A primitive with no layout of its own that records a label, the Swift
/// `_DebugNode` helper from `ViewBuilderTests.swift`.
///
/// The label is stashed as the node's `view_tag` so tests can identify a
/// specific materialised node without inventing a new `Node` property.
#[derive(Clone)]
pub struct DebugNode {
    pub label: String,
}

impl DebugNode {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
        }
    }
}

impl cove_composition::PrimitiveView for DebugNode {
    fn make_node(&self) -> Node {
        Node::new()
    }
    fn update_node(&self, node: &Node) {
        node.set_view_tag(Some(self.label.clone()));
    }
    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }
}

impl cove_composition::View for DebugNode {
    fn kind(&self) -> cove_composition::ViewKind<'_> {
        cove_composition::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// One mounted graph with its own interaction, focus, capture, portal and text
/// holders, plus real pointer and keyboard dispatch.
///
/// Translation of `Tests/CoveUIComposeTests/InteractiveScene.swift`. The Swift
/// suite installed the holders inside a scoped lock and tore the graph down on
/// scope exit; [`InteractiveScene::run`] keeps that shape, so a test body reads
/// exactly the geometry the Swift suite read.
pub struct InteractiveScene {
    pub graph: Rc<ViewGraph>,
    pub registry: Rc<InteractionRegistry>,
    pub focus: Rc<FocusChain>,
    pub capture: Rc<PointerCapture>,
    pub portal: Rc<PortalStore>,
    pub width: f32,
    pub height: f32,
}

impl InteractiveScene {
    /// Runs `body` under the global lock with private holders installed, then
    /// reinstalls an empty root, ticks once and restores the holders.
    pub fn run(width: f32, height: f32, body: impl FnOnce(&mut InteractiveScene)) {
        GlobalTestLock::with_scheduler(|| {
            let previous_registry = InteractionRegistryHolder::current();
            let previous_focus = FocusChainHolder::current();
            let previous_capture = PointerCaptureHolder::current();
            let previous_portal = PortalStoreHolder::current();
            let previous_text = TextEnvironmentHolder::current();

            let mut scene = Self::new(width, height);
            body(&mut scene);

            scene
                .graph
                .install(cove_composition::EmptyView, Default::default());
            AnimatorScheduler::current().tick(1.0);

            InteractionRegistryHolder::set_current(previous_registry);
            FocusChainHolder::set_current(previous_focus);
            PointerCaptureHolder::set_current(previous_capture);
            PortalStoreHolder::set_current(Some(previous_portal));
            TextEnvironmentHolder::set_current(previous_text);
        });
    }

    fn new(width: f32, height: f32) -> Self {
        let registry = Rc::new(InteractionRegistry::new());
        let focus = Rc::new(FocusChain::new());
        let capture = Rc::new(PointerCapture::new());
        let portal = Rc::new(PortalStore::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));
        FocusChainHolder::set_current(Some(focus.clone()));
        PointerCaptureHolder::set_current(Some(capture.clone()));
        PortalStoreHolder::set_current(Some(portal.clone()));
        TextEnvironmentHolder::set_current(Some(test_text_environment(16.0, 20.0)));
        let graph = Rc::new(ViewGraph::new(
            Rc::new(std::cell::RefCell::new(NodeTree::new())),
            Rc::new(Recomposer::new()),
        ));
        Self {
            graph,
            registry,
            focus,
            capture,
            portal,
            width,
            height,
        }
    }

    pub fn install<V: View + 'static>(&mut self, view: V) {
        self.graph.install(view, Default::default());
        self.settle(5, 1.0);
    }

    /// Recomposes, lays out and ticks `ticks` times, then commits and lays out
    /// once more — the Swift `settle` loop.
    pub fn settle(&self, ticks: usize, delta: f64) {
        for _ in 0..ticks {
            // Like the host loop: finish background work and apply what it
            // posted back before recomposing.
            cove_core::work::wait_idle(std::time::Duration::from_secs(5));
            cove_core::scene::drain_scene_invalidations();
            self.graph.recomposer.commit_all();
            self.graph.compute_layout(self.width, self.height);
            AnimatorScheduler::current().tick(delta);
        }
        self.graph.recomposer.commit_all();
        self.graph.compute_layout(self.width, self.height);
    }

    pub fn dispatcher(&self) -> EventDispatcher {
        EventDispatcher::new(
            self.graph.tree.clone(),
            self.registry.clone(),
            self.capture.clone(),
            self.focus.clone(),
            WindowId::MAIN,
        )
    }

    pub fn nodes(&self) -> Vec<Node> {
        let mut collected = Vec::new();
        if let Some(root) = self.graph.tree.borrow().root() {
            collect(&root, &mut collected);
        }
        collected
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

    pub fn labelled(&self, label: &str, role: Option<AccessibilityRole>) -> Option<Node> {
        self.first(|node| {
            node.accessibility().is_some_and(|semantics| {
                semantics.label == label && role.is_none_or(|role| semantics.role == role)
            })
        })
    }

    pub fn named(&self, name: &str) -> Option<Node> {
        self.first(|node| debug_name(node).as_deref() == Some(name))
    }

    fn click_at(&self, x: f32, y: f32, clicks: u8) {
        let event = MouseButtonEvent {
            button: MouseButton::Left,
            x,
            y,
            clicks,
            modifiers: Default::default(),
            kind: Default::default(),
        };
        let dispatcher = self.dispatcher();
        dispatcher.dispatch(&InputEvent::MouseButtonDown(event.clone()));
        self.graph.recomposer.commit_all();
        dispatcher.dispatch(&InputEvent::MouseButtonUp(event));
        self.settle(5, 1.0);
    }

    pub fn click(&self, node: &Node, clicks: u8) {
        let frame = node.absolute_frame();
        self.click_at(
            frame.x + frame.width / 2.0,
            frame.y + frame.height / 2.0,
            clicks,
        );
    }

    pub fn click_at_point(&self, point: Point, clicks: u8) {
        self.click_at(point.x, point.y, clicks);
    }

    pub fn drag(&self, start: Point, end: Point, steps: usize) {
        let dispatcher = self.dispatcher();
        dispatcher.dispatch(&InputEvent::MouseButtonDown(MouseButtonEvent {
            button: MouseButton::Left,
            x: start.x,
            y: start.y,
            clicks: 1,
            modifiers: Default::default(),
            kind: Default::default(),
        }));
        self.graph.recomposer.commit_all();
        for step in 1..=steps {
            let t = step as f32 / steps as f32;
            dispatcher.dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
                x: start.x + (end.x - start.x) * t,
                y: start.y + (end.y - start.y) * t,
                delta_x: 0.0,
                delta_y: 0.0,
                kind: Default::default(),
            }));
            self.graph.recomposer.commit_all();
            self.graph.compute_layout(self.width, self.height);
        }
        dispatcher.dispatch(&InputEvent::MouseButtonUp(MouseButtonEvent {
            button: MouseButton::Left,
            x: end.x,
            y: end.y,
            clicks: 1,
            modifiers: Default::default(),
            kind: Default::default(),
        }));
        self.settle(5, 1.0);
    }

    pub fn key(&self, scancode: u32, modifiers: KeyModifiers) {
        self.dispatcher().dispatch(&InputEvent::KeyDown(KeyEvent {
            scancode,
            keycode: 0,
            modifiers,
            is_repeat: false,
        }));
        self.settle(5, 1.0);
    }
}

/// The deepest node below `root`, following first children.
pub fn deepest_node(root: &Node) -> Node {
    let mut cursor = root.clone();
    while let Some(next) = cursor.children().first().cloned() {
        cursor = next;
    }
    cursor
}

/// Finds a node by its layout debug name below `root`.
pub fn find_named(root: &Node, name: &str) -> Option<Node> {
    if debug_name(root).as_deref() == Some(name) {
        return Some(root.clone());
    }
    for child in root.children() {
        if let Some(found) = find_named(&child, name) {
            return Some(found);
        }
    }
    None
}
