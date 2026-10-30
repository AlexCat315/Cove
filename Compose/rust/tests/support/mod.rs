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

use cove_core::scene::{
    AnimatorScheduler, FocusChain, FocusChainHolder, InteractionRegistry, InteractionRegistryHolder,
    Node, NodeTree, PointerCapture, PointerCaptureHolder, Recomposer,
};
use cove_composition::{LayoutDebugAttachmentKey, ViewGraph};
use std::rc::Rc;

use cove_core::{Font, FontWeight};
use cove_text::TextCacheLimits;
use cove_text::{FontAtlas, TextEnvironment, TextEnvironmentHolder, TextFontResolver, TextShaper};

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
    pub fn locked<T>(body: impl FnOnce() -> T) -> T {
        let _guard: MutexGuard<'_, ()> = Self::mutex()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        Self { graph, interactions, focus_chain, pointer_capture }
    }

    /// Runs `body` under the global lock with fresh holders and a fresh
    /// scheduler, then tears the graph down and ticks once like Swift does.
    pub fn run(body: impl FnOnce(&mut TestScene) -> ()) {
        GlobalTestLock::with_scheduler(|| {
            let mut scene = Self::new();
            body(&mut scene);
            scene.graph.install(cove_composition::EmptyView, Default::default());
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
        self.nodes().into_iter().find(|node| debug_name(node).as_deref() == Some(name))
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
    let resolver = TextFontResolver::new("Arial".to_string(), atlas.clone(), 256, 1.0);
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
        Self { label: label.into() }
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
