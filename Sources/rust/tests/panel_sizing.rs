//! Translation of `Tests/CoveUIWorkspaceTests/PanelSizingTests.swift`.
//!
//! Behaviour checklist:
//! - invalid authored sizing normalises instead of reaching the layout engine
//! - a decoded panel sizing re-normalises and round-trips through the document
//! - inactive tabs, nested splits and collapsed groups still own their bounds
//! - a floating frame clamps to the panel bounds and persists the clamp
//! - split maximums resolve deterministically, including impossible bounds

mod workspace_support;

use std::collections::BTreeMap;

use cove_ui_addons::workspace::{
    SplitConstraints, WorkspaceAxisSizing, WorkspaceCommand, WorkspaceController,
    WorkspaceDocument, WorkspaceLayoutMetrics, WorkspaceLayoutNode, WorkspacePanel,
    WorkspacePanelID, WorkspacePanelSizing, WorkspaceRect, WorkspaceSizingResolver, WorkspaceSlot,
    WorkspaceSlotID, WorkspaceSlotKind, WorkspaceSplitAxis, WorkspaceTabGroup, WorkspaceTabGroupID,
    WorkspaceTheme,
};
use workspace_support::{group_node, panel};

fn panel_with(id: &str, sizing: WorkspacePanelSizing) -> WorkspacePanel {
    WorkspacePanel {
        sizing,
        ..panel(id, id)
    }
}

#[test]
fn sizing_defaults_normalisation_and_document_round_trip() {
    let invalid = WorkspaceAxisSizing::new(-10.0, Some(f32::INFINITY), Some(f32::NAN));
    assert_eq!(
        invalid,
        WorkspaceAxisSizing::default(),
        "non-finite values fall back"
    );

    let clamped = WorkspaceAxisSizing::new(100.0, Some(10.0), Some(80.0));
    assert_eq!(
        clamped,
        WorkspaceAxisSizing::new(100.0, Some(100.0), Some(100.0))
    );

    let decoded: WorkspacePanelSizing =
        serde_json::from_str(r#"{"width":{"minimum":180,"preferred":500,"maximum":320}}"#)
            .expect("panel sizing decodes");
    assert_eq!(
        decoded.width,
        WorkspaceAxisSizing::new(180.0, Some(320.0), Some(320.0))
    );
    assert_eq!(decoded.height, WorkspaceAxisSizing::default());

    let inspector = WorkspacePanel {
        sizing: decoded,
        ..panel("inspector", "Inspector")
    };
    let descriptor = cove_ui_addons::workspace::WorkspacePanelDescriptor::new(
        WorkspacePanelID::new("inspector"),
        "Inspector",
    )
    .configure(|panel| panel.sizing = decoded);
    assert_eq!(descriptor.panel, inspector);

    let mut panels = BTreeMap::new();
    panels.insert(inspector.id.clone(), inspector);
    let mut groups = BTreeMap::new();
    groups.insert(WorkspaceTabGroupID::new("inspector"), group_node_group());
    let mut slots = WorkspaceSlot::standard_editor_slots();
    slots.insert(
        WorkspaceSlotID::trailing(),
        WorkspaceSlot::new(
            WorkspaceSlotID::trailing(),
            WorkspaceSlotKind::Content,
            Some(group_node("inspector")),
        ),
    );
    let document = WorkspaceDocument::new(panels, groups, slots);

    let encoded = serde_json::to_string(&document).expect("document encodes");
    let round_tripped: WorkspaceDocument =
        serde_json::from_str(&encoded).expect("document decodes");
    assert_eq!(round_tripped, document);
    assert_eq!(document.split_fractions.leading, None);
    assert!(
        !encoded.contains("measuredExtent"),
        "a measured extent is not authored state and must not persist"
    );

    // Missing optional fields decode to their documented defaults.
    let sparse = r#"{"id":"inspector","title":"Inspector"}"#;
    let decoded: WorkspacePanel = serde_json::from_str(sparse).expect("panel decodes");
    assert_eq!(decoded.sizing, WorkspacePanelSizing::default());
    assert!(decoded.is_closable);
}

fn group_node_group() -> WorkspaceTabGroup {
    WorkspaceTabGroup::new(
        WorkspaceTabGroupID::new("inspector"),
        vec![WorkspacePanelID::new("inspector")],
    )
}

#[test]
fn inactive_tabs_nested_splits_and_collapsed_groups_own_their_bounds() {
    let panels = BTreeMap::from([
        (
            WorkspacePanelID::new("a"),
            panel_with(
                "a",
                WorkspacePanelSizing::new(
                    WorkspaceAxisSizing::new(100.0, None, Some(500.0)),
                    WorkspaceAxisSizing::default(),
                ),
            ),
        ),
        (
            WorkspacePanelID::new("b"),
            panel_with(
                "b",
                WorkspacePanelSizing::new(
                    WorkspaceAxisSizing::new(220.0, None, Some(350.0)),
                    WorkspaceAxisSizing::default(),
                ),
            ),
        ),
        (
            WorkspacePanelID::new("c"),
            panel_with(
                "c",
                WorkspacePanelSizing::new(
                    WorkspaceAxisSizing::new(80.0, None, Some(200.0)),
                    WorkspaceAxisSizing::default(),
                ),
            ),
        ),
    ]);
    let groups = BTreeMap::from([
        (
            WorkspaceTabGroupID::new("ab"),
            WorkspaceTabGroup::new(
                WorkspaceTabGroupID::new("ab"),
                vec![WorkspacePanelID::new("a"), WorkspacePanelID::new("b")],
            ),
        ),
        (
            WorkspaceTabGroupID::new("c"),
            WorkspaceTabGroup::new(
                WorkspaceTabGroupID::new("c"),
                vec![WorkspacePanelID::new("c")],
            ),
        ),
    ]);
    let mut slots = WorkspaceSlot::standard_editor_slots();
    slots.insert(
        WorkspaceSlotID::center(),
        WorkspaceSlot::new(
            WorkspaceSlotID::center(),
            WorkspaceSlotKind::Content,
            Some(WorkspaceLayoutNode::split(
                WorkspaceSplitAxis::Horizontal,
                0.5,
                group_node("ab"),
                group_node("c"),
            )),
        ),
    );
    let mut document = WorkspaceDocument::new(panels, groups, slots);

    let sizing = WorkspaceSizingResolver::new(&document);
    assert_eq!(
        sizing.group(
            &WorkspaceTabGroupID::new("ab"),
            WorkspaceSplitAxis::Horizontal
        ),
        WorkspaceAxisSizing::new(220.0, None, Some(350.0)),
        "a group takes the largest bound of every panel it holds"
    );

    // Stacked siblings share one gutter; the grab area overhangs the panes
    // instead of reserving more margin.
    let metrics = WorkspaceLayoutMetrics::default();
    let gutter = metrics.divider;
    assert_eq!(gutter, WorkspaceTheme::default().gutter);
    assert_eq!(
        sizing.slot(&WorkspaceSlotID::center(), WorkspaceSplitAxis::Horizontal),
        WorkspaceAxisSizing::new(220.0 + 80.0 + gutter, None, Some(350.0 + 200.0 + gutter))
    );
    assert_eq!(
        sizing
            .slot(&WorkspaceSlotID::center(), WorkspaceSplitAxis::Vertical)
            .minimum,
        34.0,
        "an empty group still reserves the tab bar height"
    );

    document
        .groups
        .get_mut(&WorkspaceTabGroupID::new("c"))
        .expect("group c")
        .is_collapsed = true;
    let sizing = WorkspaceSizingResolver::new(&document);
    assert_eq!(
        sizing.slot(&WorkspaceSlotID::center(), WorkspaceSplitAxis::Horizontal),
        WorkspaceAxisSizing::new(220.0, None, Some(350.0)),
        "a collapsed group contributes nothing"
    );
}

#[test]
fn floating_frames_use_the_panel_bounds_and_persist_the_clamped_size() {
    let inspector = panel_with(
        "inspector",
        WorkspacePanelSizing::new(
            WorkspaceAxisSizing::new(200.0, Some(260.0), Some(320.0)),
            WorkspaceAxisSizing::new(120.0, Some(180.0), Some(240.0)),
        ),
    );
    let mut panels = BTreeMap::new();
    panels.insert(inspector.id.clone(), inspector);
    let mut groups = BTreeMap::new();
    groups.insert(
        WorkspaceTabGroupID::new("inspector"),
        WorkspaceTabGroup::new(
            WorkspaceTabGroupID::new("inspector"),
            vec![WorkspacePanelID::new("inspector")],
        ),
    );
    let mut slots = WorkspaceSlot::standard_editor_slots();
    slots.insert(
        WorkspaceSlotID::trailing(),
        WorkspaceSlot::new(
            WorkspaceSlotID::trailing(),
            WorkspaceSlotKind::Content,
            Some(group_node("inspector")),
        ),
    );
    let mut controller = WorkspaceController::new(WorkspaceDocument::new(panels, groups, slots));

    controller.dispatch(WorkspaceCommand::FloatGroup {
        group_id: WorkspaceTabGroupID::new("inspector"),
        window_id: cove_ui_addons::workspace::WorkspaceFloatingWindowID::new("float"),
        frame: WorkspaceRect::new(0.0, 0.0, 900.0, 900.0),
    });
    let window = controller
        .document()
        .floating_windows
        .first()
        .expect("floating window");
    assert_eq!(
        window.frame.width, 320.0,
        "the width clamps to the panel maximum"
    );
    assert_eq!(
        window.frame.height, 304.0,
        "the height maximum plus the title bar"
    );

    controller.dispatch(WorkspaceCommand::MoveFloatingWindow {
        window_id: cove_ui_addons::workspace::WorkspaceFloatingWindowID::new("float"),
        frame: WorkspaceRect::new(0.0, 0.0, 160.0, 120.0),
    });
    let window = controller
        .document()
        .floating_windows
        .first()
        .expect("floating window");
    assert_eq!(
        window.frame.width, 200.0,
        "the width floors at the panel minimum"
    );
    assert_eq!(
        window.frame.height, 184.0,
        "the height minimum plus the title bar"
    );
}

#[test]
fn split_maximums_and_impossible_bounds_are_deterministic() {
    let constraints = SplitConstraints {
        fraction: (0.0, 1.0),
        minimum_first: 100.0,
        minimum_second: 150.0,
        maximum_first: Some(260.0),
        maximum_second: Some(360.0),
    };

    let feasible = constraints.range(500.0);
    assert!(
        (feasible.0 - 0.28).abs() < 0.00001,
        "100 / 500 is the lower bound, got {}",
        feasible.0
    );
    assert!(
        (feasible.1 - 0.52).abs() < 0.00001,
        "260 / 500 is the upper bound, got {}",
        feasible.1
    );

    // Impossible bounds collapse to a single legal point rather than inverting.
    let cramped = constraints.range(100.0);
    assert!((cramped.0 - 0.4).abs() < 0.00001);
    assert!((cramped.1 - 0.4).abs() < 0.00001);

    let extra = constraints.range(1000.0);
    assert!((extra.0 - 260.0 / 620.0).abs() < 0.00001);
    assert!(
        (extra.0 - extra.1).abs() < 0.00001,
        "the two maxima pin one fraction"
    );

    let mut empty = WorkspaceController::new(WorkspaceDocument::new(
        BTreeMap::new(),
        BTreeMap::new(),
        BTreeMap::new(),
    ));
    assert!(
        !empty
            .dispatch(WorkspaceCommand::ResizeSplit {
                split_id: cove_ui_addons::workspace::WorkspaceSplitID::new("leading"),
                fraction: f32::NAN,
            })
            .did_change,
        "a non-finite resize is rejected, not applied"
    );
}
