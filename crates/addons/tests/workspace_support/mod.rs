//! Translation of `Tests/CoveUIWorkspaceTests/WorkspaceViewTests.swift` support
//! (`WorkspaceViewRig`, `makeDocument`, `makeWorkspaceRoot`).
//!
//! The Swift rig installed a `ViewGraph` with private input holders and pumped
//! layout between events. This module keeps that shape: [`WorkspaceRig`] owns the
//! graph, the dispatcher and the private holders, so a test reads exactly the
//! geometry the Swift suite read.
//!
//! The holders are process-wide thread-locals, so every test that builds a rig
//! must hold [`GlobalTestLock`].

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::{Mutex, MutexGuard, OnceLock};

use cove_composition::{AnyView, LayoutDebugAttachmentKey, View, ViewGraph};
use cove_core::scene::{
    EventDispatcher, FocusChain, FocusChainHolder, InteractionRegistry, InteractionRegistryHolder,
    Node, NodeTree, PointerCapture, PointerCaptureHolder, Recomposer,
};
use cove_core::{Point, Rect};
use cove_platform::{InputEvent, MouseButton, MouseButtonEvent, MouseMotionEvent};
use cove_ui_addons::workspace::{
    WorkspaceController, WorkspaceDocument, WorkspaceLayoutNode, WorkspacePanel, WorkspacePanelID,
    WorkspaceSlot, WorkspaceSlotID, WorkspaceSlotKind, WorkspaceSplitAxis, WorkspaceSplitFractions,
    WorkspaceTabGroup, WorkspaceTabGroupID, WorkspaceView,
};

/// Process-wide serialization for tests that install the scene holders.
pub struct GlobalTestLock;

impl GlobalTestLock {
    fn mutex() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

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
}

/// The layout debug name recorded by `.debug_name(_)`.
pub fn debug_name(node: &Node) -> Option<String> {
    node.attachment::<String>(LayoutDebugAttachmentKey::DEBUG_NAME)
}

/// Every node in the graph, depth first.
pub fn collect(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        collect(&child, into);
    }
}

pub fn panel(id: &str, title: &str) -> WorkspacePanel {
    WorkspacePanel::new(WorkspacePanelID::new(id), title)
}

pub fn group(id: &str, panels: &[&str]) -> WorkspaceTabGroup {
    WorkspaceTabGroup::new(
        WorkspaceTabGroupID::new(id),
        panels.iter().map(|id| WorkspacePanelID::new(*id)).collect(),
    )
}

pub fn group_node(id: &str) -> WorkspaceLayoutNode {
    WorkspaceLayoutNode::Group(WorkspaceTabGroupID::new(id))
}

/// The Swift suite's shared fixture: four panels in four regions with the
/// authored split fractions.
pub fn make_document() -> WorkspaceDocument {
    let panels = BTreeMap::from([
        (
            WorkspacePanelID::new("hierarchy"),
            panel("hierarchy", "Hierarchy"),
        ),
        (
            WorkspacePanelID::new("viewport"),
            WorkspacePanel {
                is_closable: false,
                is_collapsible: false,
                ..panel("viewport", "Viewport")
            },
        ),
        (
            WorkspacePanelID::new("inspector"),
            panel("inspector", "Inspector"),
        ),
        (
            WorkspacePanelID::new("console"),
            panel("console", "Console"),
        ),
    ]);
    let groups = BTreeMap::from([
        (
            WorkspaceTabGroupID::new("leading"),
            group("leading", &["hierarchy"]),
        ),
        (
            WorkspaceTabGroupID::new("center"),
            group("center", &["viewport"]),
        ),
        (
            WorkspaceTabGroupID::new("trailing"),
            group("trailing", &["inspector"]),
        ),
        (
            WorkspaceTabGroupID::new("bottom"),
            group("bottom", &["console"]),
        ),
    ]);
    let mut slots = WorkspaceSlot::standard_editor_slots();
    for (slot_id, group_id) in [
        (WorkspaceSlotID::leading(), "leading"),
        (WorkspaceSlotID::center(), "center"),
        (WorkspaceSlotID::trailing(), "trailing"),
        (WorkspaceSlotID::bottom(), "bottom"),
    ] {
        slots.insert(
            slot_id.clone(),
            WorkspaceSlot::new(
                slot_id,
                WorkspaceSlotKind::Content,
                Some(group_node(group_id)),
            ),
        );
    }
    WorkspaceDocument {
        panels,
        groups,
        slots,
        split_fractions: WorkspaceSplitFractions::new(Some(0.22), Some(0.78), Some(0.74), None),
        ..WorkspaceDocument::new(BTreeMap::new(), BTreeMap::new(), BTreeMap::new())
    }
}

/// A document with no region chrome at all, so a slot render can be observed
/// without neighbouring rails affecting the frame.
pub fn make_bare_document(
    group_ids: &[(&str, &[&str])],
    slot_id: WorkspaceSlotID,
) -> WorkspaceDocument {
    let mut panels = BTreeMap::new();
    let mut groups = BTreeMap::new();
    for (id, panel_ids) in group_ids {
        for panel_id in *panel_ids {
            panels.insert(WorkspacePanelID::new(*panel_id), panel(panel_id, panel_id));
        }
        groups.insert(WorkspaceTabGroupID::new(*id), group(id, panel_ids));
    }
    let mut slots = WorkspaceSlot::standard_editor_slots();
    for slot in slots.values_mut() {
        slot.layout = None;
    }
    let layout = WorkspaceLayoutNode::stacked(
        &group_ids
            .iter()
            .map(|(id, _)| WorkspaceTabGroupID::new(*id))
            .collect::<Vec<_>>(),
        WorkspaceSplitAxis::Horizontal,
        0.5,
    );
    slots.insert(
        slot_id.clone(),
        WorkspaceSlot::new(slot_id, WorkspaceSlotKind::Content, layout),
    );
    WorkspaceDocument::new(panels, groups, slots)
}

/// The scene handles a workspace test dispatches input through.
pub struct WorkspaceRig {
    pub controller: Rc<RefCell<WorkspaceController>>,
    pub graph: Rc<ViewGraph>,
    pub interactions: Rc<InteractionRegistry>,
    pub focus: Rc<FocusChain>,
    pub capture: Rc<PointerCapture>,
    pub dispatcher: EventDispatcher,
    pub width: f32,
    pub height: f32,
}

impl WorkspaceRig {
    /// `WorkspaceViewRig(controller:width:height:root:)`.
    pub fn new(
        controller: Rc<RefCell<WorkspaceController>>,
        width: f32,
        height: f32,
        root: impl View + 'static,
    ) -> Self {
        let interactions = Rc::new(InteractionRegistry::new());
        let focus = Rc::new(FocusChain::new());
        let capture = Rc::new(PointerCapture::new());
        InteractionRegistryHolder::set_current(Some(interactions.clone()));
        FocusChainHolder::set_current(Some(focus.clone()));
        PointerCaptureHolder::set_current(Some(capture.clone()));

        let tree = Rc::new(RefCell::new(NodeTree::new()));
        let graph = Rc::new(ViewGraph::new(tree.clone(), Rc::new(Recomposer::new())));
        let dispatcher = EventDispatcher::new(
            tree,
            interactions.clone(),
            capture.clone(),
            focus.clone(),
            cove_platform::WindowId::MAIN,
        );
        graph.install(root, Default::default());
        graph.compute_layout(width, height);
        Self {
            controller,
            graph,
            interactions,
            focus,
            capture,
            dispatcher,
            width,
            height,
        }
    }

    /// The default workspace root: `WorkspaceView` over a debug-named content
    /// block per panel.
    pub fn standard(controller: Rc<RefCell<WorkspaceController>>, width: f32, height: f32) -> Self {
        let view = workspace_root(controller.clone());
        Self::new(controller, width, height, view)
    }

    /// `pump()`: settle recomposition and layout until the tree stops changing.
    pub fn pump(&self) {
        for _ in 0..4 {
            self.graph.recomposer.commit_all();
            self.graph.compute_layout(self.width, self.height);
        }
    }

    pub fn nodes(&self) -> Vec<Node> {
        let mut collected = Vec::new();
        if let Some(root) = self.graph.tree.borrow().root() {
            collect(&root, &mut collected);
        }
        collected
    }

    pub fn named(&self, name: &str) -> Node {
        self.optional_named(name)
            .unwrap_or_else(|| panic!("no node named {name}"))
    }

    pub fn optional_named(&self, name: &str) -> Option<Node> {
        self.nodes()
            .into_iter()
            .find(|node| debug_name(node).as_deref() == Some(name))
    }

    pub fn frame(&self, name: &str) -> Rect {
        self.named(name).frame()
    }

    /// A press/release at `point`, pumped between phases.
    pub fn click(&self, point: Point) {
        self.dispatch_pointer(point, true);
        self.pump();
        self.dispatch_pointer(point, false);
        self.pump();
    }

    fn dispatch_pointer(&self, point: Point, is_down: bool) {
        let event = MouseButtonEvent {
            button: MouseButton::Left,
            x: point.x,
            y: point.y,
            clicks: 1,
            ..Default::default()
        };
        let input = if is_down {
            InputEvent::MouseButtonDown(event)
        } else {
            InputEvent::MouseButtonUp(event)
        };
        self.dispatcher.dispatch(&input);
    }

    pub fn begin_drag(&self, from: Point) {
        self.dispatch_pointer(from, true);
        self.pump();
    }

    pub fn move_drag(&self, to: Point) {
        self.dispatcher
            .dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
                x: to.x,
                y: to.y,
                delta_x: 0.0,
                delta_y: 0.0,
                kind: Default::default(),
            }));
        self.pump();
    }

    pub fn end_drag(&self, at: Point) {
        self.dispatch_pointer(at, false);
        self.pump();
    }

    /// `drag(from:to:)`: press, two motion samples, release.
    pub fn drag(&self, from: Point, to: Point) {
        self.begin_drag(from);
        let mid = Point {
            x: (from.x + to.x) * 0.5,
            y: (from.y + to.y) * 0.5,
        };
        self.move_drag(mid);
        self.move_drag(to);
        self.end_drag(to);
    }

    pub fn collapse(&self, group_id: &str) {
        self.controller.borrow_mut().dispatch(
            cove_ui_addons::workspace::WorkspaceCommand::Collapse(WorkspaceTabGroupID::new(
                group_id,
            )),
        );
        self.pump();
    }

    pub fn expand(&self, group_id: &str) {
        self.controller
            .borrow_mut()
            .dispatch(cove_ui_addons::workspace::WorkspaceCommand::Expand(
                WorkspaceTabGroupID::new(group_id),
            ));
        self.pump();
    }

    pub fn document(&self) -> WorkspaceDocument {
        self.controller.borrow().document().clone()
    }
}

impl Drop for WorkspaceRig {
    fn drop(&mut self) {
        if InteractionRegistryHolder::current()
            .is_some_and(|current| Rc::ptr_eq(&current, &self.interactions))
        {
            InteractionRegistryHolder::set_current(None);
        }
        if FocusChainHolder::current().is_some_and(|current| Rc::ptr_eq(&current, &self.focus)) {
            FocusChainHolder::set_current(None);
        }
        if PointerCaptureHolder::current()
            .is_some_and(|current| Rc::ptr_eq(&current, &self.capture))
        {
            PointerCaptureHolder::set_current(None);
        }
    }
}

/// `makeWorkspaceRoot(controller:)`: the workspace over debug-named content.
pub fn workspace_root(controller: Rc<RefCell<WorkspaceController>>) -> AnyView {
    use cove_composition::{EmptyView, LayoutDebugExt};
    use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
    use cove_ui_compose::BoxView;

    AnyView::new(WorkspaceView::new(
        controller,
        Rc::new(|panel_id: WorkspacePanelID| {
            AnyView::new(
                BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(EmptyView),
                )
                .debug_name(&format!("content-{}", panel_id.raw_value())),
            )
        }),
    ))
}

/// A controller over `make_document`.
pub fn controller_with_fixture() -> Rc<RefCell<WorkspaceController>> {
    Rc::new(RefCell::new(WorkspaceController::new(make_document())))
}
