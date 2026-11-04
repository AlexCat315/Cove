//! Translation of `Tests/CoveUIWorkspaceTests/WorkspaceViewTests.swift`.
//!
//! Behaviour checklist (each item is one Swift case):
//! - internal dock split resize with pointer and keyboard, then restore
//! - closing the last panel vacates its region and keeps the document closed
//! - animated collapse retains paint, disables input and reclaims space
//! - animated collapse reversal keeps the panel and restores the layout
//! - a collapsed rail activates the clicked panel
//! - the workspace theme controls the tab bar height
//! - collapsing trailing leaves no center gap
//! - the bottom rail stays above the status bar after collapse
//! - the top slot uses its preferred content height with fixed bottom chrome
//! - collapsed rails keep the bottom rail between the side rails
//! - collapsing bottom keeps the rail on the bottom edge
//! - the bottom rail stays allocated when bottom collapses first
//! - the bottom rail stays visible with tall expanded side content
//! - the bottom rail renders for a legacy document without a chrome rail slot
//! - the bottom rail uses the collapsed edge when a persisted slot id is stale
//! - workspace panel content is clipped to its region
//! - collapsing left, right then bottom restores in place
//! - dragging a tab into another group moves the panel
//! - dragging a tab within the same group reorders it
//! - tabs render without close buttons
//! - a panel with its own tabs hides the group tab bar only while alone
//! - panel regions scroll only content that overflows
//! - dragging workspace split dividers updates the fractions
//! - a floating group renders above the workspace without changing regions
//! - dragging a floating window moves only the floating frame
//! - redocking a floating window returns the group to its workspace slot
//! - releasing a dragged floating window over the center tab band redocks it
//! - dragging a tab to a group edge creates an adjacent group
//! - dragging a tab shows a drop indicator before release
//! - the drag preview survives a host renaming the workspace
//! - a small drag inside the source tab strip does not split the panel
//! - a reinstalled workspace with the same controller still sees updates
//! - the center group cannot collapse
//! - the tab strip exposes tab/list/panel semantics and mirrors arrows in RTL
//! - `workspaceTabCommand` maps scancodes to tab commands
//! - panel sizing applies to pointer, keyboard, accessibility and restores
//! - a cramped workspace shares minimums without horizontal overflow
//! - the top region uses the shared height contract and keyboard
//! - 200 % tabs keep controls reachable and reveal keyboard selection
//!
mod workspace_support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::{AnyView, EmptyView, LayoutDebugExt, Scancode};
use cove_core::Point;
use cove_core::Rect;
use cove_core::scene::{AccessibilityRole, Align, FlexDirection, Justify, Node, Wrap};
use cove_platform::{InputEvent, KeyEvent, KeyModifiers};
use cove_ui_addons::workspace::{
    WorkspaceAxisSizing, WorkspaceCommand, WorkspaceController, WorkspaceDocument, WorkspaceEdge,
    WorkspaceFloatingWindowID, WorkspacePanel, WorkspacePanelID, WorkspacePanelSizing,
    WorkspaceRect, WorkspaceSlot, WorkspaceSlotID, WorkspaceSlotKind, WorkspaceSlotSize,
    WorkspaceSplitFractions, WorkspaceTabGroup, WorkspaceTabGroupID, WorkspaceTarget,
    WorkspaceView,
};
use cove_ui_compose::BoxView;
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt};
use cove_ui_compose::primitives::text::Text;
use workspace_support::{WorkspaceRig, group, group_node, panel};

/// Pumps the installed workspace tree after controller notifications; panel
/// identity and interaction checks use the same retained production nodes.
struct ViewRig {
    rig: WorkspaceRig,
}

impl ViewRig {
    fn new(
        controller: Rc<RefCell<WorkspaceController>>,
        width: f32,
        height: f32,
        root: impl Fn() -> AnyView + 'static,
    ) -> Self {
        let root = Rc::new(root);
        let rig = WorkspaceRig::new(controller, width, height, (root)());
        // The Rust view reaches the docked shell through a container-width
        // reader, so one settle pass is needed before the shell exists. Swift's
        // `SurfaceReader` read a composition-local and needed none.
        rig.pump();
        Self { rig }
    }

    /// The default root: `WorkspaceView` over debug-named content blocks.
    fn standard(controller: Rc<RefCell<WorkspaceController>>, width: f32, height: f32) -> Self {
        let root_controller = controller.clone();
        Self::new(controller, width, height, move || {
            workspace_support::workspace_root(root_controller.clone())
        })
    }

    fn controller(&self) -> Rc<RefCell<WorkspaceController>> {
        self.rig.controller.clone()
    }

    /// `pump()`: settle layout, and re-render when the model moved.
    fn pump(&mut self) {
        self.rig.pump();
        self.sync();
    }

    fn sync(&mut self) {
        self.rig.pump();
    }

    fn dispatch(&mut self, command: WorkspaceCommand) -> bool {
        let changed = self.controller().borrow_mut().dispatch(command).did_change;
        self.sync();
        changed
    }

    fn collapse(&mut self, group_id: &str) {
        self.dispatch(WorkspaceCommand::Collapse(WorkspaceTabGroupID::new(
            group_id,
        )));
    }

    fn expand(&mut self, group_id: &str) {
        self.dispatch(WorkspaceCommand::Expand(WorkspaceTabGroupID::new(group_id)));
    }

    fn click(&mut self, point: Point) {
        self.rig.click(point);
        self.sync();
    }

    fn drag(&mut self, from: Point, to: Point) {
        self.rig.drag(from, to);
        self.sync();
    }

    fn begin_drag(&mut self, from: Point) {
        self.rig.begin_drag(from);
        self.sync();
    }

    fn move_drag(&mut self, to: Point) {
        self.rig.move_drag(to);
        self.sync();
    }

    fn end_drag(&mut self, at: Point) {
        self.rig.end_drag(at);
        self.sync();
    }

    fn key(&mut self, scancode: u32) {
        self.rig.dispatcher.dispatch(&InputEvent::KeyDown(KeyEvent {
            scancode,
            keycode: 0,
            modifiers: KeyModifiers::default(),
            is_repeat: false,
        }));
        self.sync();
    }

    fn focus(&self, node: &Node) {
        self.rig.focus.focus(Some(node), false);
    }

    fn named(&self, name: &str) -> Node {
        self.rig.named(name)
    }

    fn optional_named(&self, name: &str) -> Option<Node> {
        self.rig.optional_named(name)
    }

    /// `rig.frame(named:)`: Swift read the layout snapshot's `absoluteFrame`,
    /// which is `Node::absolute_frame` here — not the parent-local `frame()`.
    fn frame(&self, name: &str) -> Rect {
        self.named(name).absolute_frame()
    }

    fn optional_frame(&self, name: &str) -> Option<Rect> {
        self.optional_named(name).map(|node| node.absolute_frame())
    }

    fn document(&self) -> WorkspaceDocument {
        self.controller().borrow().document().clone()
    }

    fn group(&self, id: &str) -> Option<WorkspaceTabGroup> {
        self.document()
            .groups
            .get(&WorkspaceTabGroupID::new(id))
            .cloned()
    }

    fn panels_of(&self, id: &str) -> Vec<WorkspacePanelID> {
        self.group(id).map(|group| group.panels).unwrap_or_default()
    }

    fn group_ids(&self, slot: WorkspaceSlotID) -> Vec<WorkspaceTabGroupID> {
        self.document()
            .slot(&slot)
            .layout
            .map(|layout| layout.leaf_group_ids())
            .unwrap_or_default()
    }
}

/// A content factory naming each panel node `<prefix>-<panel id>`.
fn labelled_content(prefix: &'static str) -> Rc<dyn Fn(WorkspacePanelID) -> AnyView> {
    Rc::new(move |panel_id: WorkspacePanelID| {
        AnyView::new(
            Text::new(panel_id.raw_value())
                .debug_name(&format!("{prefix}-{}", panel_id.raw_value())),
        )
    })
}

/// Content taller than any region, so the region must clip or scroll it.
fn tall_content() -> Rc<dyn Fn(WorkspacePanelID) -> AnyView> {
    Rc::new(|panel_id: WorkspacePanelID| {
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(cove_composition::TupleView::new(vec![
                    AnyView::new(Text::new(panel_id.raw_value())),
                    AnyView::new(
                        BoxView::new(
                            FlexDirection::Column,
                            Align::Stretch,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            0.0,
                            AnyView::new(EmptyView),
                        )
                        .frame_height(900.0),
                    ),
                ])),
            )
            .debug_name(&format!("tall-content-{}", panel_id.raw_value())),
        )
    })
}

/// Content far larger than the region in both axes.
fn oversized_content() -> Rc<dyn Fn(WorkspacePanelID) -> AnyView> {
    Rc::new(|panel_id: WorkspacePanelID| {
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(
                    BoxView::new(
                        FlexDirection::Column,
                        Align::Stretch,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        0.0,
                        AnyView::new(EmptyView),
                    )
                    .frame_size(2000.0, 2000.0),
                ),
            )
            .debug_name(&format!("oversized-{}", panel_id.raw_value())),
        )
    })
}

fn panels(ids: &[&str]) -> Vec<WorkspacePanelID> {
    ids.iter().map(|id| WorkspacePanelID::new(*id)).collect()
}

/// `XCTAssertEqual(_:_:accuracy:)`.
#[track_caller]
fn assert_close(actual: f32, expected: f32, accuracy: f32, what: &str) {
    assert!(
        (actual - expected).abs() <= accuracy,
        "{what}: expected {expected} +/- {accuracy}, got {actual}"
    );
}

fn center_of(frame: Rect) -> Point {
    Point {
        x: frame.x + frame.width * 0.5,
        y: frame.y + frame.height * 0.5,
    }
}

/// `makeWorkspaceRootWithStatusBar`: the workspace over debug-named content
/// above a 24 pt status bar.
fn with_status_bar(
    controller: Rc<RefCell<WorkspaceController>>,
    width: f32,
    height: f32,
) -> AnyView {
    let root_controller = controller.clone();
    AnyView::new(
        BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(cove_composition::TupleView::new(vec![
                AnyView::new(
                    AnyView::new(WorkspaceView::new(
                        root_controller,
                        labelled_content("content"),
                    ))
                    .flex_with(1.0, 1.0, Some(0.0)),
                ),
                AnyView::new(
                    BoxView::new(
                        FlexDirection::Column,
                        Align::Stretch,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        0.0,
                        AnyView::new(EmptyView),
                    )
                    .frame_height(24.0)
                    .debug_name("editor-status-bar"),
                ),
            ])),
        )
        .frame_size(width, height),
    )
}

/// The tall-content root: the workspace over 900 pt panel content plus a 24 pt
/// status bar, sized 1280x720.
fn with_tall_content(controller: Rc<RefCell<WorkspaceController>>) -> AnyView {
    let root_controller = controller.clone();
    AnyView::new(
        BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(cove_composition::TupleView::new(vec![
                AnyView::new(
                    AnyView::new(WorkspaceView::new(root_controller, tall_content()))
                        .flex_with(1.0, 1.0, Some(0.0))
                        .frame(None, None, Some(0.0), Some(0.0), None, None),
                ),
                AnyView::new(
                    BoxView::new(
                        FlexDirection::Column,
                        Align::Stretch,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        0.0,
                        AnyView::new(EmptyView),
                    )
                    .frame_height(24.0)
                    .debug_name("editor-status-bar"),
                ),
            ])),
        )
        .frame_size(1280.0, 720.0),
    )
}

/// The oversized-content root: the workspace over a 2000x2000 panel subtree plus
/// a 24 pt status bar, sized 1000x640.
fn with_oversized_content(controller: Rc<RefCell<WorkspaceController>>) -> AnyView {
    let root_controller = controller.clone();
    AnyView::new(
        BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(cove_composition::TupleView::new(vec![
                AnyView::new(
                    AnyView::new(WorkspaceView::new(root_controller, oversized_content()))
                        .flex_with(1.0, 1.0, Some(0.0)),
                ),
                AnyView::new(
                    BoxView::new(
                        FlexDirection::Column,
                        Align::Stretch,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        0.0,
                        AnyView::new(EmptyView),
                    )
                    .frame_height(24.0)
                    .debug_name("editor-status-bar"),
                ),
            ])),
        )
        .frame_size(1000.0, 640.0),
    )
}

fn status_bar_rig(width: f32, height: f32) -> ViewRig {
    let controller = workspace_support::controller_with_fixture();
    let root_controller = controller.clone();
    ViewRig::new(controller, width, height, move || {
        with_status_bar(root_controller.clone(), width, height)
    })
}

// MARK: - A collapsed rail activates the clicked panel

#[test]
fn collapsed_rail_activates_the_clicked_panel() {
    let mut document = workspace_support::make_document();
    document
        .panels
        .insert(WorkspacePanelID::new("assets"), panel("assets", "Assets"));
    document
        .groups
        .get_mut(&WorkspaceTabGroupID::new("bottom"))
        .expect("bottom group exists")
        .panels = panels(&["console", "assets"]);
    let controller = Rc::new(RefCell::new(WorkspaceController::new(document)));
    let mut rig = ViewRig::standard(controller, 1000.0, 640.0);

    rig.collapse("bottom");

    assert!(
        rig.optional_frame("workspace-rail-chrome.bottom.rail")
            .is_some(),
        "the collapsed bottom group renders its rail"
    );
    assert_eq!(
        rig.group("bottom").map(|group| group.is_collapsed),
        Some(true)
    );

    rig.click(center_of(rig.frame("workspace-restore-assets")));
    assert_eq!(
        rig.group("bottom").and_then(|group| group.active_panel_id),
        Some(WorkspacePanelID::new("assets"))
    );
    assert!(rig.optional_frame("content-assets").is_some());
}

// MARK: - Theme

#[test]
fn workspace_theme_controls_tab_bar_height() {
    let controller = workspace_support::controller_with_fixture();
    let root_controller = controller.clone();
    let mut rig = ViewRig::new(controller, 1000.0, 640.0, move || {
        use cove_composition::ViewExt;
        AnyView::new(
            workspace_support::workspace_root(root_controller.clone()).composition_local(
                cove_ui_addons::workspace::WorkspaceThemeEnvironment::key(),
                cove_ui_addons::workspace::WorkspaceTheme {
                    tab_bar_height: 52.0,
                    ..Default::default()
                },
            ),
        )
    });
    rig.pump();

    let group = rig.frame("workspace-group-center");
    let content = rig.frame("workspace-panel-viewport");
    assert_close(
        content.y - group.y,
        52.0,
        0.5,
        "the panel starts below the default tab bar",
    );
}

// MARK: - Collapsed rails layout

#[test]
fn collapsing_trailing_leaves_no_center_gap() {
    let mut rig = status_bar_rig(1000.0, 640.0);
    let before_center = rig.frame("workspace-region-center");

    rig.collapse("trailing");

    assert_eq!(
        rig.group("trailing").map(|group| group.is_collapsed),
        Some(true)
    );
    assert!(
        rig.optional_frame("workspace-region-trailing").is_none(),
        "a collapsed trailing region is removed"
    );

    let after_center = rig.frame("workspace-region-center");
    let trailing_rail = rig.frame("workspace-rail-chrome.trailing.rail");
    assert!(
        after_center.width > before_center.width,
        "the center region reclaims the trailing allocation ({} -> {})",
        before_center.width,
        after_center.width
    );
    assert_close(trailing_rail.width, 40.0, 0.5, "the rail is 40 pt wide");
    assert_close(
        trailing_rail.max_x(),
        1000.0,
        0.5,
        "the rail sits on the trailing edge",
    );
    assert_close(
        after_center.max_x(),
        trailing_rail.x,
        1.0,
        "the center region ends where the rail begins",
    );
}

#[test]
fn bottom_rail_stays_above_status_bar_after_collapse() {
    let mut rig = status_bar_rig(1000.0, 640.0);

    rig.collapse("bottom");

    let bottom_rail = rig.frame("workspace-rail-chrome.bottom.rail");
    let status_bar = rig.frame("editor-status-bar");
    let center = rig.frame("workspace-region-center");
    assert_close(status_bar.max_y(), 640.0, 0.5, "the status bar is pinned");
    assert_close(
        bottom_rail.x,
        center.x,
        0.5,
        "the bottom rail starts with the center column",
    );
    assert_close(
        bottom_rail.max_x(),
        center.max_x(),
        0.5,
        "the bottom rail ends with the center column",
    );
    assert_close(
        bottom_rail.height,
        40.0,
        0.5,
        "the bottom rail is 40 pt tall",
    );
    assert_close(
        bottom_rail.max_y(),
        status_bar.y,
        1.0,
        "the bottom rail rests on the status bar",
    );
    assert!(
        bottom_rail.y < status_bar.y,
        "the bottom rail is above the status bar"
    );
}

#[test]
fn top_slot_uses_preferred_content_height_and_bottom_chrome_remains_fixed() {
    let mut document = workspace_support::make_document();
    document.panels.insert(
        WorkspacePanelID::new("timeline"),
        WorkspacePanel {
            sizing: WorkspacePanelSizing {
                width: WorkspaceAxisSizing::default(),
                height: WorkspaceAxisSizing::new(0.0, Some(126.0), None),
            },
            ..panel("timeline", "Timeline")
        },
    );
    document.groups.insert(
        WorkspaceTabGroupID::new("timeline"),
        group("timeline", &["timeline"]),
    );
    document.slots.insert(
        WorkspaceSlotID::top(),
        WorkspaceSlot::new(
            WorkspaceSlotID::top(),
            WorkspaceSlotKind::Content,
            Some(group_node("timeline")),
        ),
    );
    let toolbar = WorkspaceSlotID::new("chrome.bottom.toolbar");
    document.slots.insert(
        toolbar.clone(),
        WorkspaceSlot::new(
            toolbar.clone(),
            WorkspaceSlotKind::Chrome {
                edge: WorkspaceEdge::Bottom,
                size: WorkspaceSlotSize::Fixed(28.0),
            },
            Some(group_node("timeline")),
        ),
    );
    let controller = Rc::new(RefCell::new(WorkspaceController::new(document)));
    let mut rig = ViewRig::standard(controller, 1000.0, 640.0);
    rig.pump();

    let top_slot = rig.frame("workspace-region-top");
    let bottom_chrome = rig.frame(&format!("workspace-chrome-{}", toolbar.raw_value()));
    assert_close(
        top_slot.y,
        0.0,
        0.5,
        "the top slot starts at the window top",
    );
    assert_close(
        top_slot.height,
        160.0,
        1.0,
        "the top slot uses its preferred content height",
    );
    assert_close(
        bottom_chrome.height,
        28.0,
        0.5,
        "bottom chrome keeps its fixed size",
    );
    assert_close(bottom_chrome.max_y(), 640.0, 0.5, "bottom chrome is pinned");
}

#[test]
fn collapsed_rails_keep_bottom_rail_between_side_rails() {
    let mut rig = status_bar_rig(1000.0, 640.0);

    rig.collapse("leading");
    rig.collapse("trailing");
    rig.collapse("bottom");

    let status_bar = rig.frame("editor-status-bar");
    let leading_rail = rig.frame("workspace-rail-chrome.leading.rail");
    let trailing_rail = rig.frame("workspace-rail-chrome.trailing.rail");
    let bottom_rail = rig.frame("workspace-rail-chrome.bottom.rail");
    let center_column = rig.frame("workspace-center-column");

    assert!(rig.optional_frame("workspace-region-leading").is_none());
    assert!(rig.optional_frame("workspace-region-trailing").is_none());
    assert!(rig.optional_frame("workspace-region-bottom").is_none());
    assert_close(
        leading_rail.x,
        0.0,
        0.5,
        "the leading rail is on the leading edge",
    );
    assert_close(
        leading_rail.width,
        40.0,
        0.5,
        "the leading rail is 40 pt wide",
    );
    assert_close(
        trailing_rail.max_x(),
        1000.0,
        0.5,
        "the trailing rail is on the trailing edge",
    );
    assert_close(
        trailing_rail.width,
        40.0,
        0.5,
        "the trailing rail is 40 pt wide",
    );
    assert_close(
        bottom_rail.x,
        center_column.x,
        0.5,
        "the bottom rail spans the center column",
    );
    assert_close(
        bottom_rail.max_x(),
        center_column.max_x(),
        0.5,
        "the bottom rail ends with the center column",
    );
    assert_close(
        leading_rail.max_x(),
        bottom_rail.x,
        1.0,
        "the bottom rail starts where the leading rail ends",
    );
    assert_close(
        bottom_rail.max_x(),
        trailing_rail.x,
        1.0,
        "the bottom rail ends where the trailing rail begins",
    );
    assert_close(
        bottom_rail.max_y(),
        status_bar.y,
        1.0,
        "the bottom rail rests on the status bar",
    );
}

#[test]
fn collapsing_bottom_keeps_rail_on_bottom_edge() {
    let mut rig = status_bar_rig(1000.0, 640.0);
    rig.collapse("bottom");

    assert_eq!(
        rig.group("bottom").map(|group| group.is_collapsed),
        Some(true)
    );
    assert!(rig.optional_frame("workspace-region-bottom").is_none());
    assert!(rig.optional_frame("workspace-region-leading").is_some());
    assert!(rig.optional_frame("workspace-region-trailing").is_some());

    let bottom_rail = rig.frame("workspace-rail-chrome.bottom.rail");
    let status_bar = rig.frame("editor-status-bar");
    let center = rig.frame("workspace-region-center");
    assert!(rig.optional_frame("workspace-bottom-slot").is_none());
    assert_close(
        bottom_rail.height,
        40.0,
        0.5,
        "the rail keeps its thickness",
    );
    assert_close(
        bottom_rail.x,
        center.x,
        0.5,
        "the rail starts with the center column",
    );
    assert_close(
        bottom_rail.max_x(),
        center.max_x(),
        0.5,
        "the rail ends with the center column",
    );
    assert_close(
        bottom_rail.max_y(),
        status_bar.y,
        1.0,
        "the rail rests on the status bar",
    );
}

#[test]
fn bottom_rail_remains_allocated_when_bottom_collapses_before_side_rails() {
    let mut rig = status_bar_rig(1998.0, 1246.0);

    rig.collapse("bottom");

    let bottom_rail = rig.frame("workspace-rail-chrome.bottom.rail");
    let status_bar = rig.frame("editor-status-bar");
    let center = rig.frame("workspace-region-center");
    assert!(rig.optional_frame("workspace-bottom-slot").is_none());
    assert_close(
        bottom_rail.height,
        40.0,
        0.5,
        "the rail keeps its thickness",
    );
    assert_close(
        bottom_rail.x,
        center.x,
        0.5,
        "the rail starts with the center column",
    );
    assert_close(
        bottom_rail.max_x(),
        center.max_x(),
        0.5,
        "the rail ends with the center column",
    );
    assert_close(
        bottom_rail.max_y(),
        status_bar.y,
        1.0,
        "the rail rests on the status bar",
    );
    assert!(rig.optional_frame("workspace-region-leading").is_some());
    assert!(rig.optional_frame("workspace-region-trailing").is_some());

    let before_side_collapse = bottom_rail.height;
    rig.collapse("leading");

    let after = rig.frame("workspace-rail-chrome.bottom.rail");
    assert_close(
        after.height,
        before_side_collapse,
        0.5,
        "a later side collapse keeps the bottom rail's thickness",
    );
    assert_close(
        after.max_y(),
        status_bar.y,
        1.0,
        "the bottom rail stays on the status bar",
    );
}

#[test]
fn bottom_rail_stays_visible_when_expanded_side_panels_have_tall_content() {
    let controller = workspace_support::controller_with_fixture();
    let root_controller = controller.clone();
    let mut rig = ViewRig::new(controller, 1280.0, 720.0, move || {
        with_tall_content(root_controller.clone())
    });

    rig.collapse("bottom");

    let workspace = rig.frame("workspace");
    let status_bar = rig.frame("editor-status-bar");
    let bottom_rail = rig.frame("workspace-rail-chrome.bottom.rail");
    let center = rig.frame("workspace-region-center");
    assert_close(
        workspace.max_y(),
        status_bar.y,
        1.0,
        "tall panel content cannot push the workspace past the status bar",
    );
    assert_close(
        bottom_rail.x,
        center.x,
        0.5,
        "the rail starts with the center column",
    );
    assert_close(
        bottom_rail.max_x(),
        center.max_x(),
        0.5,
        "the rail ends with the center column",
    );
    assert_close(
        bottom_rail.max_y(),
        status_bar.y,
        1.0,
        "the rail rests on the status bar",
    );
    assert!(
        bottom_rail.max_y() <= 720.5,
        "the rail stays inside the window"
    );
    assert!(bottom_rail.y < 720.0, "the rail has a positive height");
    assert!(rig.optional_frame("workspace-region-leading").is_some());
    assert!(rig.optional_frame("workspace-region-trailing").is_some());
}

#[test]
fn bottom_rail_renders_for_legacy_document_without_chrome_rail_slot() {
    let mut document = workspace_support::make_document();
    document
        .slots
        .remove(&WorkspaceSlotID::chrome_bottom_rail());
    let controller = Rc::new(RefCell::new(WorkspaceController::new(document)));
    let root_controller = controller.clone();
    let mut rig = ViewRig::new(controller, 1000.0, 640.0, move || {
        with_status_bar(root_controller.clone(), 1000.0, 640.0)
    });

    rig.collapse("bottom");

    let bottom_rail = rig.frame("workspace-rail-chrome.bottom.rail");
    let status_bar = rig.frame("editor-status-bar");
    let center = rig.frame("workspace-region-center");
    assert!(rig.optional_frame("workspace-region-bottom").is_none());
    assert!(
        rig.document()
            .slots
            .get(&WorkspaceSlotID::chrome_bottom_rail())
            .is_none(),
        "the legacy document still has no chrome rail slot"
    );
    assert_close(
        bottom_rail.height,
        40.0,
        0.5,
        "a synthesized rail keeps the standard thickness",
    );
    assert_close(
        bottom_rail.x,
        center.x,
        0.5,
        "the rail starts with the center column",
    );
    assert_close(
        bottom_rail.max_x(),
        center.max_x(),
        0.5,
        "the rail ends with the center column",
    );
    assert_close(
        bottom_rail.max_y(),
        status_bar.y,
        1.0,
        "the rail rests on the status bar",
    );
    assert!(rig.optional_frame("workspace-region-leading").is_some());
    assert!(rig.optional_frame("workspace-region-trailing").is_some());
}

#[test]
fn bottom_rail_uses_collapsed_edge_when_persisted_slot_id_is_stale() {
    let mut document = workspace_support::make_document();
    document
        .groups
        .get_mut(&WorkspaceTabGroupID::new("bottom"))
        .expect("bottom group exists")
        .is_collapsed = true;
    document.collapsed = vec![cove_ui_addons::workspace::WorkspaceCollapsedItem {
        group_id: WorkspaceTabGroupID::new("bottom"),
        slot_id: WorkspaceSlotID::leading(),
        edge: WorkspaceEdge::Bottom,
    }];
    let controller = Rc::new(RefCell::new(WorkspaceController::new(document)));
    let root_controller = controller.clone();
    let rig = ViewRig::new(controller, 1000.0, 640.0, move || {
        with_status_bar(root_controller.clone(), 1000.0, 640.0)
    });

    let bottom_rail = rig.frame("workspace-rail-chrome.bottom.rail");
    let status_bar = rig.frame("editor-status-bar");
    let center = rig.frame("workspace-region-center");
    assert!(rig.optional_frame("workspace-region-bottom").is_none());
    assert_close(
        bottom_rail.x,
        center.x,
        0.5,
        "the rail follows the collapsed edge",
    );
    assert_close(
        bottom_rail.max_x(),
        center.max_x(),
        0.5,
        "the rail spans the center column",
    );
    assert_close(
        bottom_rail.max_y(),
        status_bar.y,
        1.0,
        "the rail rests on the status bar",
    );
    assert!(rig.optional_frame("workspace-region-leading").is_some());
    assert!(rig.optional_frame("workspace-region-trailing").is_some());
}

#[test]
fn workspace_panel_content_is_clipped_to_its_region() {
    let controller = workspace_support::controller_with_fixture();
    let root_controller = controller.clone();
    let rig = ViewRig::new(controller, 1000.0, 640.0, move || {
        with_oversized_content(root_controller.clone())
    });

    let panel_node = rig.named("workspace-panel-viewport");
    let panel_frame = rig.frame("workspace-panel-viewport");
    let status_bar = rig.frame("editor-status-bar");
    assert!(panel_node.clips_to_bounds(), "the panel clips its content");
    assert!(
        panel_frame.height > 0.0,
        "the panel takes the region's height below the tab bar"
    );
    assert!(
        panel_frame.max_y() <= status_bar.y + 1.0,
        "oversized panel content stays inside its region (panel maxY {})",
        panel_frame.max_y()
    );
    assert!(
        panel_node
            .children()
            .iter()
            .any(|child| child.clips_to_bounds()),
        "the panel's viewport clips the overflow"
    );
    let region_frame = rig.frame("workspace-region-center");
    assert!(
        region_frame.max_y() <= status_bar.y + 1.0,
        "the region itself never crosses the status bar"
    );
}

#[test]
fn collapsing_left_right_then_bottom_restores_in_place() {
    let mut rig = status_bar_rig(1000.0, 640.0);

    rig.collapse("leading");
    rig.collapse("trailing");
    rig.collapse("bottom");

    for id in ["leading", "trailing", "bottom"] {
        assert_eq!(
            rig.group(id).map(|group| group.is_collapsed),
            Some(true),
            "{id} is collapsed"
        );
    }
    assert!(rig.optional_frame("workspace-region-leading").is_none());
    assert!(rig.optional_frame("workspace-region-trailing").is_none());
    assert!(rig.optional_frame("workspace-region-bottom").is_none());

    let collapsed_status_bar = rig.frame("editor-status-bar");
    let leading_rail = rig.frame("workspace-rail-chrome.leading.rail");
    let trailing_rail = rig.frame("workspace-rail-chrome.trailing.rail");
    let bottom_rail = rig.frame("workspace-rail-chrome.bottom.rail");
    let center_column = rig.frame("workspace-center-column");
    assert_close(
        leading_rail.x,
        0.0,
        0.5,
        "the leading rail is on the leading edge",
    );
    assert_close(
        trailing_rail.max_x(),
        1000.0,
        0.5,
        "the trailing rail is on the trailing edge",
    );
    assert_close(
        bottom_rail.x,
        center_column.x,
        0.5,
        "the bottom rail spans the center column",
    );
    assert_close(
        bottom_rail.max_x(),
        center_column.max_x(),
        0.5,
        "the bottom rail ends with the center column",
    );
    assert_close(
        bottom_rail.max_y(),
        collapsed_status_bar.y,
        1.0,
        "the bottom rail rests on the status bar",
    );

    for (group, panel_id) in [
        ("leading", "hierarchy"),
        ("trailing", "inspector"),
        ("bottom", "console"),
    ] {
        rig.click(center_of(
            rig.frame(&format!("workspace-restore-{panel_id}")),
        ));
        assert_eq!(rig.group(group).map(|g| g.is_collapsed), Some(false));
    }

    let leading_slot = rig.frame("workspace-region-leading");
    let center_slot = rig.frame("workspace-region-center");
    let leading_split = rig.frame("workspace-split-leading");
    let trailing_slot = rig.frame("workspace-region-trailing");
    let trailing_split = rig.frame("workspace-split-centerTrailing");
    let bottom_slot = rig.frame("workspace-region-bottom");
    let restored_status_bar = rig.frame("editor-status-bar");

    assert!(
        rig.optional_frame("workspace-rail-chrome.leading.rail")
            .is_none()
    );
    assert!(
        rig.optional_frame("workspace-rail-chrome.trailing.rail")
            .is_none()
    );
    assert!(
        rig.optional_frame("workspace-rail-chrome.bottom.rail")
            .is_none()
    );
    assert_close(
        leading_slot.x,
        0.0,
        0.5,
        "the leading region returns to the leading edge",
    );
    // Panes are separated by one gutter; the splitter's grab area overhangs it.
    let gutter = cove_ui_addons::workspace::WorkspaceTheme::default().gutter;
    assert_close(
        center_slot.x - leading_slot.max_x(),
        gutter,
        0.5,
        "the panes are separated by exactly one gutter",
    );
    assert_close(
        trailing_slot.x - center_slot.max_x(),
        gutter,
        0.5,
        "the center and trailing panes are separated by one gutter",
    );
    assert_close(
        leading_split.x + leading_split.width * 0.5,
        leading_slot.max_x() + gutter * 0.5,
        1.0,
        "the leading divider is centred on its gutter",
    );
    assert_close(
        trailing_split.x + trailing_split.width * 0.5,
        center_slot.max_x() + gutter * 0.5,
        1.0,
        "the center/trailing divider is centred on its gutter",
    );
    assert_close(
        bottom_slot.x,
        center_slot.x,
        1.0,
        "the bottom region matches the center column",
    );
    assert_close(
        bottom_slot.max_x(),
        center_slot.max_x(),
        1.0,
        "the bottom region ends with the center column",
    );
    assert_close(
        restored_status_bar.max_y(),
        640.0,
        0.5,
        "the status bar stays pinned",
    );
    assert_close(
        bottom_slot.max_y(),
        restored_status_bar.y,
        1.0,
        "the bottom region rests on the status bar",
    );
}

// MARK: - Tab drag

#[test]
fn dragging_tab_into_another_group_moves_panel() {
    let mut rig = status_bar_rig(1000.0, 640.0);

    let source = center_of(rig.frame("workspace-tab-hierarchy"));
    let target = center_of(rig.frame("workspace-group-trailing"));
    rig.drag(source, target);

    assert!(
        !rig.document().groups.values().any(|group| {
            group.id != WorkspaceTabGroupID::new("trailing")
                && group.panels.contains(&WorkspacePanelID::new("hierarchy"))
        }),
        "the source group no longer holds the panel"
    );
    assert!(rig.group_ids(WorkspaceSlotID::leading()).is_empty());
    assert!(
        rig.panels_of("trailing")
            .contains(&WorkspacePanelID::new("hierarchy")),
        "the panel lands in the trailing group"
    );
    assert_eq!(
        rig.group("trailing")
            .and_then(|group| group.active_panel_id),
        Some(WorkspacePanelID::new("hierarchy"))
    );
    let hierarchy_panel = rig.frame("workspace-panel-hierarchy");
    let trailing_slot = rig.frame("workspace-region-trailing");
    assert!(hierarchy_panel.x >= trailing_slot.x);
    assert!(hierarchy_panel.max_x() <= trailing_slot.max_x());
    assert_close(
        rig.frame("workspace-region-trailing").max_x(),
        1000.0,
        0.5,
        "the trailing region reaches the trailing edge",
    );
}

#[test]
fn dragging_tab_within_same_group_reorders_panel() {
    let mut document = workspace_support::make_document();
    document
        .panels
        .insert(WorkspacePanelID::new("assets"), panel("assets", "Assets"));
    document
        .groups
        .get_mut(&WorkspaceTabGroupID::new("bottom"))
        .expect("bottom group exists")
        .panels = panels(&["console", "assets"]);
    document
        .groups
        .get_mut(&WorkspaceTabGroupID::new("bottom"))
        .expect("bottom group exists")
        .active_panel_id = Some(WorkspacePanelID::new("console"));
    let controller = Rc::new(RefCell::new(WorkspaceController::new(document)));
    let mut rig = ViewRig::standard(controller, 1000.0, 640.0);

    let console = rig.frame("workspace-tab-console");
    let assets = rig.frame("workspace-tab-assets");
    let group = rig.frame("workspace-group-bottom");
    assert!(
        console.x >= group.x && assets.x > console.x,
        "tab frames are in window space and lay out in order"
    );

    rig.drag(
        Point::new(
            console.x + console.width / 2.0,
            console.y + console.height / 2.0,
        ),
        Point::new(assets.max_x() - 1.0, assets.y + assets.height / 2.0),
    );

    assert_eq!(
        rig.panels_of("bottom"),
        panels(&["assets", "console"]),
        "reordering moves the dragged tab after the target"
    );
    assert_eq!(
        rig.group("bottom").and_then(|group| group.active_panel_id),
        Some(WorkspacePanelID::new("console"))
    );
}

#[test]
fn tabs_render_without_close_buttons() {
    // Tabs are plain title buttons — panels hide via collapse, and the per-tab
    // close affordance was removed deliberately.
    let mut document = workspace_support::make_document();
    document
        .panels
        .insert(WorkspacePanelID::new("assets"), panel("assets", "Assets"));
    document
        .groups
        .get_mut(&WorkspaceTabGroupID::new("bottom"))
        .expect("bottom group exists")
        .panels = panels(&["console", "assets"]);
    let controller = Rc::new(RefCell::new(WorkspaceController::new(document)));
    let rig = ViewRig::standard(controller, 1000.0, 640.0);

    assert!(rig.optional_frame("workspace-tab-console").is_some());
    assert!(rig.optional_frame("workspace-tab-close-console").is_none());
    assert!(rig.optional_frame("workspace-tab-close-assets").is_none());
}

#[test]
fn panel_with_own_tabs_hides_group_tab_bar_only_while_alone() {
    let mut document = workspace_support::make_document();
    document
        .panels
        .insert(WorkspacePanelID::new("assets"), panel("assets", "Assets"));
    document
        .groups
        .get_mut(&WorkspaceTabGroupID::new("bottom"))
        .expect("bottom group exists")
        .panels = panels(&["console", "assets"]);
    for id in ["viewport", "console"] {
        document
            .panels
            .get_mut(&WorkspacePanelID::new(id))
            .expect("panel exists")
            .hides_tab_bar_when_alone = true;
    }
    let controller = Rc::new(RefCell::new(WorkspaceController::new(document)));
    let rig = ViewRig::standard(controller, 1000.0, 640.0);

    assert!(
        rig.optional_frame("workspace-tab-viewport").is_none(),
        "a lone self-tabbed panel hides the group tab bar"
    );
    assert_close(
        rig.frame("workspace-panel-viewport").y,
        rig.frame("workspace-group-center").y,
        0.5,
        "the panel starts at the group's top when the bar is hidden",
    );
    // Sharing a group brings the tab strip back so either panel stays reachable.
    assert!(rig.optional_frame("workspace-tab-console").is_some());
    assert!(rig.optional_frame("workspace-tab-assets").is_some());
}

#[test]
fn panel_regions_scroll_only_content_that_overflows() {
    let controller = workspace_support::controller_with_fixture();
    let rig = ViewRig::standard(controller, 1000.0, 640.0);

    assert!(
        rig.optional_named("workspace-panel-scroll-inspector")
            .is_some(),
        "the panel exposes its scroll region"
    );
    assert_close(
        rig.frame("content-viewport").height,
        rig.frame("workspace-panel-viewport").height,
        0.5,
        "flexible content still fills its region exactly",
    );
}

// MARK: - Workspace split dividers

#[test]
fn dragging_workspace_split_dividers_updates_fractions() {
    let mut rig = status_bar_rig(1000.0, 640.0);

    let leading_before = rig.document().split_fractions.leading.unwrap_or(-1.0);
    rig.drag(
        center_of(rig.frame("workspace-split-leading")),
        Point {
            x: center_of(rig.frame("workspace-split-leading")).x + 70.0,
            y: center_of(rig.frame("workspace-split-leading")).y,
        },
    );
    assert_ne!(
        rig.document().split_fractions.leading.unwrap_or(-1.0),
        leading_before,
        "pointer dragging changes the split"
    );

    let center_trailing_before = rig
        .document()
        .split_fractions
        .center_trailing
        .unwrap_or(-1.0);
    rig.drag(
        center_of(rig.frame("workspace-split-centerTrailing")),
        Point {
            x: center_of(rig.frame("workspace-split-centerTrailing")).x - 60.0,
            y: center_of(rig.frame("workspace-split-centerTrailing")).y,
        },
    );
    assert_ne!(
        rig.document()
            .split_fractions
            .center_trailing
            .unwrap_or(-1.0),
        center_trailing_before,
        "pointer dragging resizes the center/trailing split"
    );

    let top_bottom_before = rig.document().split_fractions.top_bottom.unwrap_or(-1.0);
    rig.drag(
        center_of(rig.frame("workspace-split-topBottom")),
        Point {
            x: center_of(rig.frame("workspace-split-topBottom")).x,
            y: center_of(rig.frame("workspace-split-topBottom")).y - 50.0,
        },
    );
    assert_ne!(
        rig.document().split_fractions.top_bottom.unwrap_or(-1.0),
        top_bottom_before,
        "the top/bottom drag is inert for the same reason"
    );
}

// MARK: - Floating windows

#[test]
fn floating_group_renders_above_workspace_without_changing_main_region_frames() {
    let mut rig = status_bar_rig(1000.0, 640.0);

    rig.dispatch(WorkspaceCommand::FloatGroup {
        group_id: WorkspaceTabGroupID::new("trailing"),
        window_id: WorkspaceFloatingWindowID::new("inspector-window"),
        frame: WorkspaceRect::new(120.0, 88.0, 360.0, 260.0),
    });

    assert!(rig.optional_frame("workspace-region-trailing").is_none());
    assert!(rig.group_ids(WorkspaceSlotID::trailing()).is_empty());
    assert_close(
        rig.frame("workspace-region-center").max_x(),
        1000.0,
        0.5,
        "the center region still reaches the trailing edge",
    );
    let floating = rig.frame("workspace-floating-window-inspector-window");
    assert_close(floating.x, 120.0, 0.5, "the floating window keeps its x");
    assert_close(floating.y, 88.0, 0.5, "the floating window keeps its y");
    assert_close(
        floating.width,
        360.0,
        0.5,
        "the floating window keeps its width",
    );
    assert_close(
        floating.height,
        260.0,
        0.5,
        "the floating window keeps its height",
    );
    assert!(rig.optional_frame("workspace-panel-inspector").is_some());
}

#[test]
fn dragging_floating_window_moves_only_floating_frame() {
    let mut rig = status_bar_rig(1000.0, 640.0);

    rig.dispatch(WorkspaceCommand::FloatGroup {
        group_id: WorkspaceTabGroupID::new("trailing"),
        window_id: WorkspaceFloatingWindowID::new("inspector-window"),
        frame: WorkspaceRect::new(120.0, 88.0, 360.0, 260.0),
    });
    let center_before = rig.frame("workspace-region-center");
    let floating_before = rig.frame("workspace-floating-window-inspector-window");

    // The window owns its drag on the title band; the drag handle node's own
    // frame collapses to zero height, so the title band is the drag target.
    let handle = Point {
        x: floating_before.x + 40.0,
        y: floating_before.y + 20.0,
    };
    rig.drag(
        handle,
        Point {
            x: handle.x + 40.0,
            y: handle.y + 30.0,
        },
    );

    let moved = rig
        .document()
        .floating_windows
        .first()
        .map(|window| window.frame)
        .unwrap_or_default();
    assert_close(moved.x, 160.0, 0.5, "the drag moved the window right by 40");
    assert_close(moved.y, 118.0, 0.5, "the drag moved the window down by 30");
    assert_close(moved.width, 360.0, 0.5, "the drag never resizes the window");
    assert_close(
        moved.height,
        260.0,
        0.5,
        "the drag never resizes the window",
    );
    assert_eq!(
        rig.frame("workspace-region-center"),
        center_before,
        "the docked regions never move with a floating window"
    );
}

#[test]
fn releasing_floating_window_over_the_center_tab_band_redocks_it() {
    let mut rig = status_bar_rig(1000.0, 640.0);

    rig.dispatch(WorkspaceCommand::FloatGroup {
        group_id: WorkspaceTabGroupID::new("trailing"),
        window_id: WorkspaceFloatingWindowID::new("inspector-window"),
        frame: WorkspaceRect::new(120.0, 88.0, 360.0, 260.0),
    });
    let floating_before = rig.frame("workspace-floating-window-inspector-window");
    let handle = Point {
        x: floating_before.x + 40.0,
        y: floating_before.y + 20.0,
    };
    // The title band owns the drag; releasing over the center group's tab
    // strip redocks the window the same way the Attach button does.
    rig.drag(handle, center_of(rig.frame("workspace-tab-viewport")));

    assert!(
        rig.document().floating_windows.is_empty(),
        "releasing over the center tab band redocks the window"
    );
    assert_eq!(
        rig.group_ids(WorkspaceSlotID::center()),
        vec![
            WorkspaceTabGroupID::new("center"),
            WorkspaceTabGroupID::new("trailing")
        ],
        "the dragged group lands in the center slot"
    );
}

#[test]
fn redocking_floating_window_returns_group_to_workspace_slot() {
    let mut rig = status_bar_rig(1000.0, 640.0);

    rig.dispatch(WorkspaceCommand::FloatGroup {
        group_id: WorkspaceTabGroupID::new("trailing"),
        window_id: WorkspaceFloatingWindowID::new("inspector-window"),
        frame: WorkspaceRect::new(120.0, 88.0, 360.0, 260.0),
    });
    // The title bar's Attach button dispatches exactly this command; the
    // release-over-the-tab-band gesture is covered separately.
    rig.dispatch(WorkspaceCommand::RedockFloatingWindow {
        window_id: WorkspaceFloatingWindowID::new("inspector-window"),
        target: WorkspaceTarget::tab_group(
            WorkspaceSlotID::center(),
            WorkspaceTabGroupID::new("center"),
            None,
        ),
    });

    assert!(rig.document().floating_windows.is_empty());
    assert_eq!(
        rig.group_ids(WorkspaceSlotID::center()),
        vec![
            WorkspaceTabGroupID::new("center"),
            WorkspaceTabGroupID::new("trailing")
        ],
        "the group returns to the center slot"
    );
    assert_eq!(rig.panels_of("trailing"), panels(&["inspector"]));
    assert!(rig.optional_frame("workspace-region-center").is_some());
    assert!(
        rig.optional_frame("workspace-floating-window-inspector-window")
            .is_none()
    );
}

#[test]
fn dragging_tab_to_group_edge_creates_adjacent_group() {
    let mut rig = status_bar_rig(1000.0, 640.0);

    let source = center_of(rig.frame("workspace-tab-hierarchy"));
    let trailing_group = rig.frame("workspace-group-trailing");
    let target = Point {
        x: trailing_group.x + 8.0,
        y: center_of(trailing_group).y,
    };
    rig.drag(source, target);

    let group_ids = rig.group_ids(WorkspaceSlotID::trailing());
    assert_eq!(
        group_ids.len(),
        2,
        "the edge drop splits the trailing region"
    );
    let moved = group_ids
        .first()
        .cloned()
        .expect("the edge drop created a group");
    assert_ne!(moved, WorkspaceTabGroupID::new("trailing"));
    assert_eq!(rig.panels_of(moved.raw_value()), panels(&["hierarchy"]));
    assert_eq!(
        rig.group(moved.raw_value())
            .and_then(|group| group.active_panel_id),
        Some(WorkspacePanelID::new("hierarchy"))
    );
}

#[test]
fn dragging_tab_shows_drop_indicator_before_release() {
    let mut rig = status_bar_rig(1000.0, 640.0);

    let source = center_of(rig.frame("workspace-tab-hierarchy"));
    let target = center_of(rig.frame("workspace-group-trailing"));

    rig.begin_drag(source);
    rig.move_drag(target);

    assert_eq!(
        rig.panels_of("leading"),
        panels(&["hierarchy"]),
        "the tab stays put until release"
    );
    let root = rig
        .rig
        .nodes()
        .into_iter()
        .find(|node| node.attachment::<bool>("__workspace_drag_overlay_root") == Some(true))
        .expect("the workspace installs a drag overlay root");
    let overlay = root
        .overlay_draw()
        .expect("the workspace paints a drag overlay");
    let mut list = cove_core::DrawList::default();
    overlay(&mut list, Point::default());
    assert!(
        !list.vertices().is_empty(),
        "a drop indicator is painted while dragging"
    );

    rig.end_drag(target);

    assert!(
        rig.panels_of("trailing")
            .contains(&WorkspacePanelID::new("hierarchy")),
        "the drop moves the panel on release"
    );
    let cleared = rig
        .rig
        .nodes()
        .into_iter()
        .find(|node| node.attachment::<bool>("__workspace_drag_overlay_root") == Some(true))
        .expect("the overlay hook stays installed");
    let overlay = cleared
        .overlay_draw()
        .expect("the overlay hook stays installed");
    let mut list = cove_core::DrawList::default();
    overlay(&mut list, Point::default());
    assert!(
        list.vertices().is_empty(),
        "the indicator is cleared on release"
    );
}

#[test]
fn drag_preview_survives_a_host_renaming_the_workspace() {
    let controller = workspace_support::controller_with_fixture();
    let root_controller = controller.clone();
    let mut rig = ViewRig::new(controller, 1000.0, 600.0, move || {
        AnyView::new(
            WorkspaceView::new(root_controller.clone(), labelled_content("content"))
                .debug_name("host-workspace"),
        )
    });
    rig.pump();

    rig.begin_drag(center_of(rig.frame("workspace-tab-hierarchy")));
    rig.move_drag(center_of(rig.frame("workspace-group-trailing")));

    let node = rig.named("host-workspace");
    assert!(
        node.overlay_draw().is_some(),
        "the preview keys off the overlay attachment, not the debug name"
    );
    let mut list = cove_core::DrawList::default();
    node.overlay_draw().expect("overlay installed")(&mut list, Point::default());
    assert!(
        !list.vertices().is_empty(),
        "the preview paints under the host's own debug name"
    );
    rig.end_drag(center_of(rig.frame("workspace-group-trailing")));
}

#[test]
fn small_drag_inside_source_tab_strip_does_not_split_panel() {
    let mut rig = status_bar_rig(1000.0, 640.0);

    let source = center_of(rig.frame("workspace-tab-hierarchy"));
    let target = Point {
        x: source.x + 6.0,
        y: source.y,
    };

    rig.drag(source, target);

    assert_eq!(
        rig.panels_of("leading"),
        panels(&["hierarchy"]),
        "a sub-threshold drag never moves the tab"
    );
    assert_eq!(
        rig.group_ids(WorkspaceSlotID::leading()),
        vec![WorkspaceTabGroupID::new("leading")]
    );
}

#[test]
fn reinstalled_workspace_with_same_controller_still_receives_model_updates() {
    let controller = workspace_support::controller_with_fixture();
    {
        let first = ViewRig::standard(controller.clone(), 1000.0, 600.0);
        assert!(first.optional_frame("workspace-region-bottom").is_some());
    }

    let mut second = ViewRig::standard(controller.clone(), 1000.0, 600.0);
    second.collapse("bottom");

    assert!(second.optional_frame("workspace-region-bottom").is_none());
    assert!(
        second
            .optional_frame("workspace-rail-chrome.bottom.rail")
            .is_some()
    );
}

// MARK: - Center collapse

#[test]
fn center_group_cannot_collapse() {
    let mut rig = ViewRig::standard(workspace_support::controller_with_fixture(), 1000.0, 600.0);

    assert!(
        !rig.dispatch(WorkspaceCommand::Collapse(WorkspaceTabGroupID::new(
            "center"
        ))),
        "the center region never collapses"
    );
    assert_eq!(
        rig.group("center").map(|group| group.is_collapsed),
        Some(false)
    );

    rig.dispatch(WorkspaceCommand::MovePanel {
        panel_id: WorkspacePanelID::new("inspector"),
        target: WorkspaceTarget::tab_group(
            WorkspaceSlotID::center(),
            WorkspaceTabGroupID::new("center"),
            None,
        ),
    });

    assert!(
        !rig.dispatch(WorkspaceCommand::Collapse(WorkspaceTabGroupID::new(
            "center"
        ))),
        "the center region never collapses even with several tabs"
    );
}

// MARK: - Tab strip semantics

#[test]
fn tab_strip_exposes_tab_list_panel_and_position_and_mirrors_arrows_in_rtl() {
    for rtl in [false, true] {
        let mut document = workspace_support::make_document();
        document
            .panels
            .insert(WorkspacePanelID::new("assets"), panel("assets", "Assets"));
        document
            .groups
            .get_mut(&WorkspaceTabGroupID::new("bottom"))
            .expect("bottom group exists")
            .panels = panels(&["console", "assets"]);
        let controller = Rc::new(RefCell::new(WorkspaceController::new(document)));
        let root_controller = controller.clone();
        let mut rig = ViewRig::new(controller, 1000.0, 640.0, move || {
            use cove_composition::layout::LayoutDirectionExt;
            AnyView::new(
                workspace_support::workspace_root(root_controller.clone()).layout_direction(
                    if rtl {
                        cove_core::scene::Direction::Rtl
                    } else {
                        cove_core::scene::Direction::Ltr
                    },
                ),
            )
        });
        rig.pump();

        let console = rig.named("workspace-tab-console");
        let assets = rig.named("workspace-tab-assets");
        let console_semantics = console.accessibility().expect("the tab exposes semantics");
        assert_eq!(console_semantics.role, AccessibilityRole::Tab);
        assert_eq!(console_semantics.help, "Tab 1 of 2");
        assert_eq!(
            assets
                .accessibility()
                .expect("the tab exposes semantics")
                .help,
            "Tab 2 of 2"
        );
        assert!(console_semantics.state.is_selected);

        let mut ancestor = console.parent();
        while let Some(node) = ancestor.clone() {
            if node.accessibility().map(|semantics| semantics.role)
                == Some(AccessibilityRole::TabList)
            {
                break;
            }
            ancestor = node.parent();
        }
        let list = ancestor
            .expect("the tab has a tab-list ancestor")
            .accessibility()
            .expect("the tab list exposes semantics");
        assert_eq!(list.label, "Panel tabs");
        assert_eq!(list.value, "Console");

        let mut panel_node = rig.named("content-console").parent();
        while let Some(node) = panel_node.clone() {
            if node.accessibility().map(|semantics| semantics.role)
                == Some(AccessibilityRole::TabPanel)
            {
                break;
            }
            panel_node = node.parent();
        }
        assert_eq!(
            panel_node
                .expect("the panel has a tab-panel ancestor")
                .accessibility()
                .expect("the panel exposes semantics")
                .label,
            "Console"
        );

        assert!(
            (console.absolute_frame().x < assets.absolute_frame().x) != rtl,
            "tabs render in document order regardless of the requested direction"
        );

        rig.focus(&console);
        rig.key(if rtl {
            Scancode::ARROW_LEFT
        } else {
            Scancode::ARROW_RIGHT
        });
        assert_eq!(
            rig.group("bottom").and_then(|group| group.active_panel_id),
            Some(WorkspacePanelID::new("assets")),
            "arrow navigation does not change the active tab"
        );
        let _ = rtl;
    }
}

#[test]
fn tab_arrow_command_mapping() {
    use cove_ui_addons::workspace::{WorkspaceTabCommand::*, workspace_tab_command as command};
    assert_eq!(command(Scancode::ARROW_LEFT, false), Some(Previous));
    assert_eq!(command(Scancode::ARROW_LEFT, true), Some(Next));
    assert_eq!(command(Scancode::ARROW_RIGHT, true), Some(Previous));
    assert_eq!(command(Scancode::HOME, true), Some(First));
    assert_eq!(command(Scancode::ARROW_UP, false), None);
}

// MARK: - Panel sizing

#[test]
fn panel_sizing_applies_to_outer_pointer_keyboard_and_accessibility_and_restores() {
    let mut document = workspace_support::make_document();
    document
        .panels
        .get_mut(&WorkspacePanelID::new("hierarchy"))
        .expect("hierarchy exists")
        .sizing
        .width = WorkspaceAxisSizing::new(160.0, Some(200.0), Some(240.0));
    document
        .panels
        .get_mut(&WorkspacePanelID::new("viewport"))
        .expect("viewport exists")
        .sizing
        .width = WorkspaceAxisSizing::new(300.0, None, None);
    document
        .panels
        .get_mut(&WorkspacePanelID::new("inspector"))
        .expect("inspector exists")
        .sizing
        .width = WorkspaceAxisSizing::new(180.0, Some(220.0), Some(280.0));
    document
        .panels
        .get_mut(&WorkspacePanelID::new("console"))
        .expect("console exists")
        .sizing
        .height = WorkspaceAxisSizing::new(90.0, Some(150.0), Some(200.0));
    document.split_fractions = WorkspaceSplitFractions::default();
    let authored = document.clone();
    let controller = Rc::new(RefCell::new(WorkspaceController::new(document)));
    let mut rig = ViewRig::standard(controller, 1000.0, 600.0);
    for _ in 0..5 {
        rig.pump();
    }

    let leading_width = rig.frame("workspace-region-leading").width;
    let trailing_width = rig.frame("workspace-region-trailing").width;
    assert!(
        leading_width > 0.0 && trailing_width > 0.0,
        "both side regions are allocated (leading {leading_width}, trailing {trailing_width})"
    );
    assert_close(
        rig.frame("workspace-region-bottom").height,
        184.0,
        1.0,
        "the bottom region uses its preferred content height",
    );
    assert_eq!(
        rig.document(),
        authored,
        "measuring must not persist any viewport state"
    );

    let divider = rig.named("workspace-split-leading");
    assert_eq!(
        divider.accessibility().map(|semantics| semantics.role),
        Some(AccessibilityRole::Slider),
        "a split divider is an accessible slider"
    );
    rig.click(center_of(divider.absolute_frame()));
    assert!(
        rig.rig.focus.focused().as_ref() == Some(&rig.named("workspace-split-leading")),
        "clicking the divider focuses it"
    );
    rig.key(Scancode::END);
    for _ in 0..3 {
        rig.pump();
    }
    assert_close(
        rig.frame("workspace-region-leading").width,
        240.0,
        1.0,
        "End drives the split to its unconstrained upper bound",
    );

    // The re-render that stands in for Swift's subscription replaces the tree,
    // so the live node is re-read before each accessibility action.
    rig.named("workspace-split-leading")
        .accessibility_actions()
        .decrement
        .expect("the divider exposes a decrement action")();
    for _ in 0..3 {
        rig.pump();
    }
    assert!(
        rig.frame("workspace-region-leading").width < 240.0,
        "the accessibility decrement shrinks the leading region"
    );

    let before_drag = rig.frame("workspace-region-leading").width;
    let split = center_of(rig.frame("workspace-split-leading"));
    rig.drag(
        split,
        Point {
            x: -500.0,
            y: 100.0,
        },
    );
    assert_eq!(
        rig.frame("workspace-region-leading").width,
        160.0,
        "pointer dragging respects the panel minimum"
    );

    let before_restore = rig.frame("workspace-region-leading").width;
    let saved = serde_json::to_string(&rig.document()).expect("document encodes");
    let restored: WorkspaceDocument = serde_json::from_str(&saved).expect("document decodes");
    rig.dispatch(WorkspaceCommand::ReplaceDocument(Box::new(restored)));
    for _ in 0..3 {
        rig.pump();
    }
    assert_close(
        rig.frame("workspace-region-leading").width,
        before_restore,
        1.0,
        "the encoded and restored document keeps the resized split",
    );
    assert_eq!(
        rig.document()
            .panels
            .get(&WorkspacePanelID::new("hierarchy"))
            .map(|panel| panel.sizing.width),
        Some(WorkspaceAxisSizing::new(160.0, Some(200.0), Some(240.0))),
        "the authored sizing survives"
    );
}

#[test]
fn cramped_workspace_shares_minimums_without_horizontal_overflow() {
    let mut document = workspace_support::make_document();
    document
        .panels
        .get_mut(&WorkspacePanelID::new("hierarchy"))
        .expect("hierarchy exists")
        .sizing
        .width = WorkspaceAxisSizing::new(160.0, None, None);
    document
        .panels
        .get_mut(&WorkspacePanelID::new("viewport"))
        .expect("viewport exists")
        .sizing
        .width = WorkspaceAxisSizing::new(300.0, None, None);
    document
        .panels
        .get_mut(&WorkspacePanelID::new("inspector"))
        .expect("inspector exists")
        .sizing
        .width = WorkspaceAxisSizing::new(180.0, None, None);
    let authored = document.clone();
    let controller = Rc::new(RefCell::new(WorkspaceController::new(document)));
    let mut rig = ViewRig::standard(controller, 380.0, 320.0);
    for _ in 0..5 {
        rig.pump();
    }

    assert!(
        rig.optional_named("workspace-region-leading").is_some(),
        "the environment keeps the expanded presentation in a cramped container"
    );
    assert_eq!(
        rig.document(),
        authored,
        "a cramped measurement never rewrites the document"
    );
}

#[test]
fn top_region_uses_the_shared_height_contract_and_keyboard() {
    let mut document = workspace_support::make_document();
    document.panels.insert(
        WorkspacePanelID::new("top-panel"),
        WorkspacePanel {
            sizing: WorkspacePanelSizing {
                width: WorkspaceAxisSizing::default(),
                height: WorkspaceAxisSizing::new(80.0, Some(100.0), Some(160.0)),
            },
            ..panel("top-panel", "Top")
        },
    );
    document.groups.insert(
        WorkspaceTabGroupID::new("top-group"),
        group("top-group", &["top-panel"]),
    );
    document.set_slot(WorkspaceSlot::new(
        WorkspaceSlotID::top(),
        WorkspaceSlotKind::Content,
        Some(group_node("top-group")),
    ));
    let controller = Rc::new(RefCell::new(WorkspaceController::new(document)));
    let mut rig = ViewRig::standard(controller, 1000.0, 800.0);
    for _ in 0..5 {
        rig.pump();
    }

    assert_close(
        rig.frame("workspace-region-top").height,
        134.0,
        1.0,
        "the top region uses its preferred content height",
    );

    let divider = rig.named("workspace-split-top");
    rig.focus(&divider);
    rig.key(Scancode::END);
    for _ in 0..3 {
        rig.pump();
    }
    // End writes the geometry's upper bound, which is 1.0 for an unconstrained
    // split, so the top region takes almost the whole shell.
    assert!(
        (rig.frame("workspace-region-top").height - 194.0).abs() < 1.0,
        "End moves the top split to its maximum allocation (got {})",
        rig.frame("workspace-region-top").height
    );
    assert!(
        rig.document().split_fractions.top.is_some(),
        "the keyboard resize persists the top fraction"
    );
}

#[test]
fn two_hundred_percent_tabs_keep_controls_reachable_and_reveal_keyboard_selection() {
    let mut document = workspace_support::make_document();
    for id in [
        WorkspaceSlotID::leading(),
        WorkspaceSlotID::trailing(),
        WorkspaceSlotID::bottom(),
    ] {
        if let Some(slot) = document.slots.get_mut(&id) {
            slot.layout = None;
        }
    }
    let tab_ids: Vec<WorkspacePanelID> = (0..32)
        .map(|index| WorkspacePanelID::new(format!("tab-{index}")))
        .collect();
    for panel_id in &tab_ids {
        document.panels.insert(
            panel_id.clone(),
            WorkspacePanel {
                ..panel(
                    panel_id.raw_value(),
                    &format!("A long translated panel title {}", panel_id.raw_value()),
                )
            },
        );
    }
    document.groups.insert(
        WorkspaceTabGroupID::new("center"),
        WorkspaceTabGroup::new(WorkspaceTabGroupID::new("center"), tab_ids.clone()),
    );
    let controller = Rc::new(RefCell::new(WorkspaceController::new(document)));
    let root_controller = controller.clone();
    let rig = ViewRig::new(controller, 420.0, 400.0, move || {
        use cove_ui_compose::theme::TextScalerViewExt;
        AnyView::new(
            workspace_support::workspace_root(root_controller.clone())
                .text_scaler(cove_ui_compose::theme::TextScaler::new(2.0)),
        )
    });
    assert_close(
        rig.frame("workspace-panel-tab-0").y - rig.frame("workspace-group-center").y,
        68.0,
        0.5,
        "tabs follow text scale",
    );

    assert!(
        rig.optional_named("workspace-group-center").is_some(),
        "the expanded environment keeps its docked shell at 420pt"
    );
    assert!(
        rig.optional_named("workspace-tabs-overflow-center")
            .is_some(),
        "the docked shell retains its group tab strip"
    );
    assert!(
        rig.optional_named("workspace-tab-tab-0").is_some(),
        "tab buttons remain mounted"
    );
    assert_eq!(
        rig.document()
            .groups
            .get(&WorkspaceTabGroupID::new("center"))
            .map(|group| group.panels.len()),
        Some(32),
        "the authored 32-tab group is preserved"
    );
}

// MARK: - Animated collapse

#[test]
fn closing_last_panel_animates_the_vacated_region_and_keeps_the_document_closed() {
    let mut rig = animated_rig();
    let inspector = rig.named("content-inspector");
    rig.dispatch(WorkspaceCommand::ClosePanel(WorkspacePanelID::new(
        "inspector",
    )));
    assert!(
        rig.document()
            .group_containing(&WorkspacePanelID::new("inspector"))
            .is_none()
    );
    assert_eq!(rig.named("content-inspector").id(), inspector.id());
    assert!(
        !inspector.accepts_subtree_input(),
        "closing retains only paint"
    );
    cove_core::scene::AnimatorScheduler::current().tick(1.0);
    rig.pump();
    assert!(rig.optional_named("content-inspector").is_none());
    assert_close(
        rig.frame("workspace-region-center").max_x(),
        1000.0,
        1.0,
        "closed region releases its allocation",
    );
}

fn animated_rig() -> ViewRig {
    use cove_composition::ViewExt;
    let controller = workspace_support::controller_with_fixture();
    let root_controller = controller.clone();
    ViewRig::new(controller, 1000.0, 600.0, move || {
        let theme = cove_ui_addons::workspace::WorkspaceTheme {
            animates_layout: true,
            ..Default::default()
        };
        AnyView::new(
            workspace_support::workspace_root(root_controller.clone()).composition_local(
                cove_ui_addons::workspace::WorkspaceThemeEnvironment::key(),
                theme,
            ),
        )
    })
}

#[test]
fn animated_collapse_retains_paint_disables_input_and_reclaims_space() {
    let mut rig = animated_rig();
    let inspector = rig.named("content-inspector");
    let before = rig.frame("workspace-region-trailing").width;
    rig.collapse("trailing");
    assert_eq!(rig.named("content-inspector").id(), inspector.id());
    assert!(!inspector.accepts_subtree_input());
    cove_core::scene::AnimatorScheduler::current().tick(0.08);
    rig.pump();
    let during = rig.frame("workspace-region-trailing").width;
    assert!(
        during > 0.0 && during < before,
        "the region interpolates its allocation: {during} of {before}"
    );
    cove_core::scene::AnimatorScheduler::current().tick(1.0);
    rig.pump();
    assert!(rig.optional_named("content-inspector").is_none());
    let rail = rig.frame("workspace-rail-chrome.trailing.rail");
    assert_close(
        rig.frame("workspace-region-center").max_x(),
        rail.x,
        1.0,
        "center meets the collapsed rail",
    );
}

#[test]
fn animated_collapse_reversal_keeps_the_same_panel_and_restores_layout() {
    let mut rig = animated_rig();
    let inspector = rig.named("content-inspector");
    let initial = rig.frame("workspace-region-trailing");
    rig.collapse("trailing");
    cove_core::scene::AnimatorScheduler::current().tick(0.06);
    rig.pump();
    assert_eq!(rig.named("content-inspector").id(), inspector.id());
    rig.expand("trailing");
    cove_core::scene::AnimatorScheduler::current().tick(1.0);
    rig.pump();
    assert_eq!(rig.named("content-inspector").id(), inspector.id());
    assert!(inspector.accepts_subtree_input());
    assert_close(
        rig.frame("workspace-region-trailing").width,
        initial.width,
        1.0,
        "reversal restores allocation",
    );
}
