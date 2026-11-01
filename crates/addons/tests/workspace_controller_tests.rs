//! Translation of `Tests/CoveUIWorkspaceTests/WorkspaceControllerTests.swift`.
//!
//! Behaviour checklist:
//! - a nested split resize persists, reports its dirty slot and rejects stale
//!   or non-finite addresses while keeping hidden branches authored
//! - collapse/expand in any order preserves slots and restores the fractions
//! - a document supports application-defined content and chrome slots
//! - the standard editor slot schema backfills missing chrome rail slots
//! - moving a panel into another group does not rebuild slots
//! - edge drops create nested splits in the target region
//! - an edge drop inside a region creates an adjacent group instead of merging
//! - an arbitrary placement tree is preserved as authored
//! - a document rejects unattached groups from an obsolete persisted shape
//! - collapse/restore keeps the active tab and its order
//! - the center region cannot collapse even with collapsible panels
//! - reordering respects the pinned boundary
//! - close/reopen restores a panel at its original group and index
//! - reopening a specific panel does not consume newer closed panels
//! - close-others keeps pinned tabs and the selected tab
//! - close-to-the-right skips pinned tabs
//! - an unclosable panel cannot close
//! - floating a group removes it from its region without destroying tabs
//! - redocking restores the group to the target region
//! - moving a panel out of a floating group never duplicates it
//! - moving the last panel out of a floating window closes the window record
//! - a floating move and focus update the frame and the z-order
//!

use std::collections::{BTreeMap, BTreeSet};

use cove_ui_addons::workspace::{
    WorkspaceCommand, WorkspaceController, WorkspaceDocument, WorkspaceEdge, WorkspaceLayoutNode,
    WorkspaceLayoutSplit, WorkspacePanel, WorkspacePanelID, WorkspaceRect,
    WorkspaceSlot, WorkspaceSlotID, WorkspaceSlotKind, WorkspaceSlotSize, WorkspaceSplitAxis,
    WorkspaceSplitFractions, WorkspaceTabGroup, WorkspaceTabGroupID, WorkspaceTarget,
};

fn group(id: &str, panels: &[&str]) -> WorkspaceTabGroup {
    WorkspaceTabGroup::new(
        WorkspaceTabGroupID::new(id),
        panels.iter().map(|id| WorkspacePanelID::new(*id)).collect(),
    )
}

fn panel(id: &str, title: &str) -> WorkspacePanel {
    WorkspacePanel::new(WorkspacePanelID::new(id), title)
}

/// `makeDocument()`: the controller suite's own fixture, with its own channels.
fn make_document() -> WorkspaceDocument {
    let panels = BTreeMap::from([
        (
            WorkspacePanelID::new("hierarchy"),
            panel("hierarchy", "Hierarchy"),
        ),
        (
            WorkspacePanelID::new("viewport"),
            WorkspacePanel {
                is_closable: false,
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
                Some(WorkspaceLayoutNode::group(WorkspaceTabGroupID::new(
                    group_id,
                ))),
            ),
        );
    }
    WorkspaceDocument {
        panels,
        groups,
        slots,
        split_fractions: WorkspaceSplitFractions::new(Some(0.24), Some(0.72), Some(0.68), None),
        ..WorkspaceDocument::new(BTreeMap::new(), BTreeMap::new(), BTreeMap::new())
    }
}

fn group_ids(controller: &WorkspaceController, slot: WorkspaceSlotID) -> Vec<WorkspaceTabGroupID> {
    controller
        .document()
        .slot(&slot)
        .layout
        .map(|layout| layout.leaf_group_ids())
        .unwrap_or_default()
}

fn panels_of(controller: &WorkspaceController, group_id: &str) -> Vec<WorkspacePanelID> {
    controller
        .document()
        .groups
        .get(&WorkspaceTabGroupID::new(group_id))
        .map(|group| group.panels.clone())
        .unwrap_or_default()
}

fn panels(ids: &[&str]) -> Vec<WorkspacePanelID> {
    ids.iter().map(|id| WorkspacePanelID::new(*id)).collect()
}

fn active_of(controller: &WorkspaceController, group_id: &str) -> Option<WorkspacePanelID> {
    controller
        .document()
        .groups
        .get(&WorkspaceTabGroupID::new(group_id))
        .and_then(|group| group.active_panel_id.clone())
}

#[test]
fn nested_split_resize_persists_and_rejects_stale_addresses() {
    let mut document = make_document();
    let mut center = document.slot(&WorkspaceSlotID::center());
    center.layout = Some(WorkspaceLayoutNode::split(
        WorkspaceSplitAxis::Vertical,
        0.4,
        WorkspaceLayoutNode::group(WorkspaceTabGroupID::new("center")),
        WorkspaceLayoutNode::split(
            WorkspaceSplitAxis::Horizontal,
            0.6,
            WorkspaceLayoutNode::group(WorkspaceTabGroupID::new("leading")),
            WorkspaceLayoutNode::group(WorkspaceTabGroupID::new("trailing")),
        ),
    ));
    document.set_slot(center);
    let mut controller = WorkspaceController::new(document);
    let address = WorkspaceLayoutSplit::new(
        WorkspaceSlotID::center(),
        WorkspaceTabGroupID::new("leading"),
        WorkspaceTabGroupID::new("trailing"),
    );

    let result = controller.dispatch(WorkspaceCommand::ResizeLayoutSplit {
        address: address.clone(),
        fraction: 0.7,
    });
    assert!(result.persistence_dirty);
    assert_eq!(
        result.changed_slots,
        BTreeSet::from([WorkspaceSlotID::center()])
    );

    let Some(WorkspaceLayoutNode::Split {
        fraction: outer,
        second,
        ..
    }) = controller
        .document()
        .slot(&WorkspaceSlotID::center())
        .layout
    else {
        panic!("nested layout was lost");
    };
    assert_eq!(outer, 0.4);
    let WorkspaceLayoutNode::Split {
        fraction: inner, ..
    } = *second
    else {
        panic!("nested layout was lost");
    };
    assert_eq!(inner, 0.7);

    let encoded = serde_json::to_string(&controller.document()).expect("document encodes");
    assert!(
        !encoded.contains("layoutTree"),
        "the persisted shape has no derived layout tree"
    );
    let decoded: WorkspaceDocument = serde_json::from_str(&encoded).expect("document decodes");
    assert_eq!(
        &decoded,
        controller.document(),
        "the document round trips"
    );

    assert!(
        !controller
            .dispatch(WorkspaceCommand::ResizeLayoutSplit {
                address: address.clone(),
                fraction: 0.7,
            })
            .did_change,
        "the same fraction is a no-op"
    );
    let stale = WorkspaceLayoutSplit::new(
        WorkspaceSlotID::center(),
        WorkspaceTabGroupID::new("missing"),
        WorkspaceTabGroupID::new("trailing"),
    );
    assert!(
        !controller
            .dispatch(WorkspaceCommand::ResizeLayoutSplit {
                address: stale,
                fraction: 0.2,
            })
            .did_change,
        "a stale address matches no split"
    );
    assert!(
        !controller
            .dispatch(WorkspaceCommand::ResizeLayoutSplit {
                address: address.clone(),
                fraction: f32::NAN,
            })
            .did_change,
        "a non-finite fraction is rejected"
    );

    controller.dispatch(WorkspaceCommand::Collapse(WorkspaceTabGroupID::new(
        "leading",
    )));
    let outer_address = WorkspaceLayoutSplit::new(
        WorkspaceSlotID::center(),
        WorkspaceTabGroupID::new("center"),
        WorkspaceTabGroupID::new("trailing"),
    );
    assert!(
        controller
            .dispatch(WorkspaceCommand::ResizeLayoutSplit {
                address: outer_address,
                fraction: 0.55,
            })
            .did_change,
        "an outer split still resizes while a branch is collapsed"
    );
    let Some(WorkspaceLayoutNode::Split {
        fraction: next_outer,
        second,
        ..
    }) = controller
        .document()
        .slot(&WorkspaceSlotID::center())
        .layout
    else {
        panic!("hidden branches must remain authored");
    };
    assert_eq!(next_outer, 0.55);
    let WorkspaceLayoutNode::Split {
        fraction: next_inner,
        ..
    } = *second
    else {
        panic!("hidden branches must remain authored");
    };
    assert_eq!(next_inner, 0.7);
}

#[test]
fn collapse_expand_left_right_bottom_in_any_order_preserves_slots() {
    let mut controller = WorkspaceController::new(make_document());

    let left = controller.dispatch(WorkspaceCommand::Collapse(WorkspaceTabGroupID::new(
        "leading",
    )));
    let right = controller.dispatch(WorkspaceCommand::Collapse(WorkspaceTabGroupID::new(
        "trailing",
    )));
    let bottom = controller.dispatch(WorkspaceCommand::Collapse(WorkspaceTabGroupID::new(
        "bottom",
    )));

    assert!(left.persistence_dirty);
    assert!(right.persistence_dirty);
    assert!(bottom.persistence_dirty);
    for id in ["leading", "trailing", "bottom"] {
        assert!(
            controller
                .document()
                .groups
                .get(&WorkspaceTabGroupID::new(id))
                .is_some_and(|group| group.is_collapsed),
            "{id} collapses"
        );
    }
    assert_eq!(
        group_ids(&controller, WorkspaceSlotID::leading()),
        vec![WorkspaceTabGroupID::new("leading")]
    );
    assert_eq!(
        group_ids(&controller, WorkspaceSlotID::trailing()),
        vec![WorkspaceTabGroupID::new("trailing")]
    );
    assert_eq!(
        group_ids(&controller, WorkspaceSlotID::bottom()),
        vec![WorkspaceTabGroupID::new("bottom")]
    );

    controller.dispatch(WorkspaceCommand::Expand(WorkspaceTabGroupID::new("bottom")));
    controller.dispatch(WorkspaceCommand::Expand(WorkspaceTabGroupID::new(
        "trailing",
    )));
    controller.dispatch(WorkspaceCommand::Expand(WorkspaceTabGroupID::new(
        "leading",
    )));

    for id in ["leading", "trailing", "bottom"] {
        assert!(
            !controller
                .document()
                .groups
                .get(&WorkspaceTabGroupID::new(id))
                .is_some_and(|group| group.is_collapsed),
            "{id} expands"
        );
    }
    assert_eq!(
        controller.document().split_fractions.leading,
        Some(0.24),
        "expanding never rewrites the authored leading fraction"
    );
    assert_eq!(
        controller.document().split_fractions.top_bottom,
        Some(0.68),
        "expanding never rewrites the authored top/bottom fraction"
    );
    assert!(controller.document().collapsed.is_empty());
}

#[test]
fn document_supports_application_defined_slots_and_chrome_policies() {
    let drawer_slot = WorkspaceSlotID::new("drawer.properties");
    let bottom_toolbar = WorkspaceSlotID::new("chrome.bottom.timeline-toolbar");
    let mut document = make_document();
    document.panels.insert(
        WorkspacePanelID::new("properties"),
        panel("properties", "Properties"),
    );
    document.groups.insert(
        WorkspaceTabGroupID::new("properties"),
        group("properties", &["properties"]),
    );
    document.slots.insert(
        drawer_slot.clone(),
        WorkspaceSlot::new(
            drawer_slot.clone(),
            WorkspaceSlotKind::Content,
            Some(WorkspaceLayoutNode::group(WorkspaceTabGroupID::new(
                "properties",
            ))),
        ),
    );
    document.slots.insert(
        bottom_toolbar.clone(),
        WorkspaceSlot::new(
            bottom_toolbar.clone(),
            WorkspaceSlotKind::Chrome {
                edge: WorkspaceEdge::Bottom,
                size: WorkspaceSlotSize::Fixed(28.0),
            },
            None,
        ),
    );

    let mut controller = WorkspaceController::new(document);

    assert_eq!(
        group_ids(&controller, drawer_slot.clone()),
        vec![WorkspaceTabGroupID::new("properties")]
    );
    assert_eq!(
        controller.document().slot(&bottom_toolbar).kind,
        WorkspaceSlotKind::Chrome {
            edge: WorkspaceEdge::Bottom,
            size: WorkspaceSlotSize::Fixed(28.0),
        }
    );

    controller.dispatch(WorkspaceCommand::MovePanel {
        panel_id: WorkspacePanelID::new("properties"),
        target: WorkspaceTarget::split(
            WorkspaceSlotID::center(),
            WorkspaceTabGroupID::new("center"),
            WorkspaceEdge::Trailing,
            0.35,
        ),
    });

    assert!(
        group_ids(&controller, drawer_slot).is_empty(),
        "the drawer slot is emptied by the move"
    );
    let center_groups = group_ids(&controller, WorkspaceSlotID::center());
    assert!(
        center_groups
            .iter()
            .any(|id| panels_of(&controller, id.raw_value()) == panels(&["properties"])),
        "the panel lands in a center group"
    );
}

#[test]
fn standard_editor_slot_schema_backfills_missing_rail_chrome_slots() {
    let mut document = make_document();
    document
        .slots
        .remove(&WorkspaceSlotID::chrome_bottom_rail());
    document
        .slots
        .remove(&WorkspaceSlotID::chrome_trailing_rail());

    document.ensure_standard_editor_slot_schema();

    assert_eq!(
        document.slot(&WorkspaceSlotID::chrome_bottom_rail()).kind,
        WorkspaceSlotKind::Chrome {
            edge: WorkspaceEdge::Bottom,
            size: WorkspaceSlotSize::Fixed(40.0),
        }
    );
    assert_eq!(
        document.slot(&WorkspaceSlotID::chrome_trailing_rail()).kind,
        WorkspaceSlotKind::Chrome {
            edge: WorkspaceEdge::Trailing,
            size: WorkspaceSlotSize::Fixed(40.0),
        }
    );
    assert_eq!(
        document
            .slot(&WorkspaceSlotID::leading())
            .layout
            .map(|layout| layout.leaf_group_ids()),
        Some(vec![WorkspaceTabGroupID::new("leading")])
    );
    assert_eq!(
        document
            .slot(&WorkspaceSlotID::bottom())
            .layout
            .map(|layout| layout.leaf_group_ids()),
        Some(vec![WorkspaceTabGroupID::new("bottom")])
    );
}

#[test]
fn move_panel_into_another_group_does_not_rebuild_slots() {
    let mut controller = WorkspaceController::new(make_document());

    let result = controller.dispatch(WorkspaceCommand::MovePanel {
        panel_id: WorkspacePanelID::new("inspector"),
        target: WorkspaceTarget::tab_group(
            WorkspaceSlotID::center(),
            WorkspaceTabGroupID::new("center"),
            None,
        ),
    });

    assert!(result.persistence_dirty);
    assert_eq!(
        panels_of(&controller, "center"),
        panels(&["viewport", "inspector"])
    );
    assert_eq!(
        active_of(&controller, "center"),
        Some(WorkspacePanelID::new("inspector"))
    );
    assert!(
        !controller
            .document()
            .groups
            .contains_key(&WorkspaceTabGroupID::new("trailing")),
        "the vacated group is dropped"
    );
    assert!(group_ids(&controller, WorkspaceSlotID::trailing()).is_empty());
    assert_eq!(
        group_ids(&controller, WorkspaceSlotID::bottom()),
        vec![WorkspaceTabGroupID::new("bottom")],
        "neighbouring slots are untouched"
    );

    controller.dispatch(WorkspaceCommand::MovePanel {
        panel_id: WorkspacePanelID::new("inspector"),
        target: WorkspaceTarget::tab_group(
            WorkspaceSlotID::trailing(),
            WorkspaceTabGroupID::new("trailing"),
            None,
        ),
    });

    assert_eq!(
        group_ids(&controller, WorkspaceSlotID::trailing()),
        vec![WorkspaceTabGroupID::new("trailing")]
    );
    assert_eq!(panels_of(&controller, "trailing"), panels(&["inspector"]));
}

#[test]
fn center_edge_drops_create_nested_splits_in_center_region() {
    let mut controller = WorkspaceController::new(make_document());

    controller.dispatch(WorkspaceCommand::MovePanel {
        panel_id: WorkspacePanelID::new("inspector"),
        target: WorkspaceTarget {
            edge: Some(WorkspaceEdge::Trailing),
            ..WorkspaceTarget::split(
                WorkspaceSlotID::center(),
                WorkspaceTabGroupID::new("center"),
                WorkspaceEdge::Trailing,
                0.5,
            )
        },
    });
    assert!(group_ids(&controller, WorkspaceSlotID::trailing()).is_empty());
    assert_eq!(panels_of(&controller, "center"), panels(&["viewport"]));
    let inspector_group_id = group_ids(&controller, WorkspaceSlotID::center())
        .last()
        .cloned()
        .expect("the drop created a group");
    assert_eq!(
        panels_of(&controller, inspector_group_id.raw_value()),
        panels(&["inspector"])
    );

    controller.dispatch(WorkspaceCommand::MovePanel {
        panel_id: WorkspacePanelID::new("hierarchy"),
        target: WorkspaceTarget::split(
            WorkspaceSlotID::center(),
            WorkspaceTabGroupID::new("center"),
            WorkspaceEdge::Leading,
            0.5,
        ),
    });
    assert!(group_ids(&controller, WorkspaceSlotID::leading()).is_empty());
    assert_eq!(group_ids(&controller, WorkspaceSlotID::center()).len(), 3);
    assert_eq!(panels_of(&controller, "center"), panels(&["viewport"]));

    controller.dispatch(WorkspaceCommand::MovePanel {
        panel_id: WorkspacePanelID::new("console"),
        target: WorkspaceTarget::split(
            WorkspaceSlotID::center(),
            WorkspaceTabGroupID::new("center"),
            WorkspaceEdge::Bottom,
            0.5,
        ),
    });
    assert!(group_ids(&controller, WorkspaceSlotID::bottom()).is_empty());
    assert_eq!(group_ids(&controller, WorkspaceSlotID::center()).len(), 4);
    assert_eq!(panels_of(&controller, "center"), panels(&["viewport"]));
}

#[test]
fn edge_drop_inside_region_creates_adjacent_group_instead_of_merging() {
    let mut controller = WorkspaceController::new(make_document());

    controller.dispatch(WorkspaceCommand::MovePanel {
        panel_id: WorkspacePanelID::new("inspector"),
        target: WorkspaceTarget::split(
            WorkspaceSlotID::leading(),
            WorkspaceTabGroupID::new("leading"),
            WorkspaceEdge::Trailing,
            0.5,
        ),
    });

    let leading_group_ids = group_ids(&controller, WorkspaceSlotID::leading());
    assert_eq!(leading_group_ids.len(), 2);
    assert_eq!(
        leading_group_ids.first(),
        Some(&WorkspaceTabGroupID::new("leading"))
    );
    let inserted = leading_group_ids[1].clone();
    assert_eq!(panels_of(&controller, "leading"), panels(&["hierarchy"]));
    assert_eq!(
        panels_of(&controller, inserted.raw_value()),
        panels(&["inspector"])
    );
    assert!(group_ids(&controller, WorkspaceSlotID::trailing()).is_empty());
}

#[test]
fn nested_split_tree_preserves_arbitrary_panel_placement() {
    let mut document = make_document();
    document.panels.insert(
        WorkspacePanelID::new("timeline"),
        panel("timeline", "Timeline"),
    );
    document
        .groups
        .get_mut(&WorkspaceTabGroupID::new("bottom"))
        .expect("bottom group exists")
        .panels = panels(&["console", "timeline"]);
    let mut controller = WorkspaceController::new(document);

    controller.dispatch(WorkspaceCommand::MovePanel {
        panel_id: WorkspacePanelID::new("inspector"),
        target: WorkspaceTarget::split(
            WorkspaceSlotID::center(),
            WorkspaceTabGroupID::new("center"),
            WorkspaceEdge::Trailing,
            0.35,
        ),
    });
    let inspector_group_id = group_ids(&controller, WorkspaceSlotID::center())
        .last()
        .cloned()
        .expect("inspector group exists");
    controller.dispatch(WorkspaceCommand::MovePanel {
        panel_id: WorkspacePanelID::new("console"),
        target: WorkspaceTarget::split(
            WorkspaceSlotID::center(),
            inspector_group_id.clone(),
            WorkspaceEdge::Bottom,
            0.4,
        ),
    });
    controller.dispatch(WorkspaceCommand::MovePanel {
        panel_id: WorkspacePanelID::new("hierarchy"),
        target: WorkspaceTarget::split(
            WorkspaceSlotID::center(),
            WorkspaceTabGroupID::new("center"),
            WorkspaceEdge::Top,
            0.3,
        ),
    });

    let Some(WorkspaceLayoutNode::Split {
        axis,
        first,
        second,
        ..
    }) = controller
        .document()
        .slot(&WorkspaceSlotID::center())
        .layout
    else {
        panic!("expected the center region to be backed by a split tree");
    };
    assert_eq!(axis, WorkspaceSplitAxis::Horizontal);
    assert_eq!(group_ids(&controller, WorkspaceSlotID::center()).len(), 4);
    assert!(
        first.contains_group(&WorkspaceTabGroupID::new("center")),
        "the center group stays on the first side"
    );
    assert!(
        first.leaf_group_ids().iter().any(|id| {
            controller
                .document()
                .groups
                .get(id)
                .is_some_and(|group| group.panels == panels(&["hierarchy"]))
        }),
        "hierarchy lands on the first side"
    );
    assert!(second.contains_group(&inspector_group_id));
    assert!(
        second.leaf_group_ids().iter().any(|id| {
            controller
                .document()
                .groups
                .get(id)
                .is_some_and(|group| group.panels == panels(&["console"]))
        }),
        "console lands on the second side"
    );
    assert!(group_ids(&controller, WorkspaceSlotID::leading()).is_empty());
    assert_eq!(
        group_ids(&controller, WorkspaceSlotID::bottom()),
        vec![WorkspaceTabGroupID::new("bottom")]
    );
}

#[test]
fn document_rejects_unattached_groups_from_obsolete_persisted_shape() {
    let panels = BTreeMap::from([
        (
            WorkspacePanelID::new("viewport"),
            WorkspacePanel {
                is_closable: false,
                ..panel("viewport", "Viewport")
            },
        ),
        (
            WorkspacePanelID::new("inspector"),
            panel("inspector", "Inspector"),
        ),
    ]);
    let groups = BTreeMap::from([
        (
            WorkspaceTabGroupID::new("center"),
            group("center", &["viewport"]),
        ),
        (
            WorkspaceTabGroupID::new("trailing"),
            group("trailing", &["inspector"]),
        ),
    ]);
    let obsolete = WorkspaceDocument::new(panels, groups, WorkspaceSlot::standard_editor_slots());

    assert!(
        !obsolete.has_valid_layout_references(),
        "an unreferenced non-empty group invalidates the shape"
    );
    assert!(make_document().has_valid_layout_references());
}

#[test]
fn collapse_restore_keeps_active_tab_and_order() {
    let mut document = make_document();
    document
        .groups
        .get_mut(&WorkspaceTabGroupID::new("bottom"))
        .expect("bottom group exists")
        .panels = panels(&["console", "assets"]);
    document
        .groups
        .get_mut(&WorkspaceTabGroupID::new("bottom"))
        .expect("bottom group exists")
        .active_panel_id = Some(WorkspacePanelID::new("assets"));
    document
        .panels
        .insert(WorkspacePanelID::new("assets"), panel("assets", "Assets"));
    let mut controller = WorkspaceController::new(document);

    controller.dispatch(WorkspaceCommand::Collapse(WorkspaceTabGroupID::new(
        "bottom",
    )));
    controller.dispatch(WorkspaceCommand::Expand(WorkspaceTabGroupID::new("bottom")));

    assert_eq!(
        panels_of(&controller, "bottom"),
        panels(&["console", "assets"]),
        "collapsing never reorders tabs"
    );
    assert_eq!(
        active_of(&controller, "bottom"),
        Some(WorkspacePanelID::new("assets"))
    );
    assert_eq!(controller.document().split_fractions.top_bottom, Some(0.68));
}

#[test]
fn center_region_cannot_collapse_even_with_collapsible_panels() {
    let mut controller = WorkspaceController::new(make_document());

    controller.dispatch(WorkspaceCommand::MovePanel {
        panel_id: WorkspacePanelID::new("inspector"),
        target: WorkspaceTarget::tab_group(
            WorkspaceSlotID::center(),
            WorkspaceTabGroupID::new("center"),
            None,
        ),
    });
    let result = controller.dispatch(WorkspaceCommand::Collapse(WorkspaceTabGroupID::new(
        "center",
    )));

    assert!(!result.did_change, "the center region never collapses");
    assert_eq!(
        controller
            .document()
            .groups
            .get(&WorkspaceTabGroupID::new("center"))
            .map(|group| group.is_collapsed),
        Some(false)
    );
    assert_eq!(
        panels_of(&controller, "center"),
        panels(&["viewport", "inspector"])
    );
}

#[test]
fn reorder_panel_keeps_pinned_boundary() {
    let mut document = make_document();
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
    let mut controller = WorkspaceController::new(document);

    controller.dispatch(WorkspaceCommand::SetPinned {
        panel_id: WorkspacePanelID::new("assets"),
        is_pinned: true,
    });
    controller.dispatch(WorkspaceCommand::ReorderPanel {
        panel_id: WorkspacePanelID::new("console"),
        group_id: WorkspaceTabGroupID::new("bottom"),
        index: 0,
    });

    assert_eq!(
        controller
            .document()
            .groups
            .get(&WorkspaceTabGroupID::new("bottom"))
            .map(|group| group.pinned_panel_ids.clone()),
        Some(panels(&["assets"]))
    );
    assert_eq!(
        panels_of(&controller, "bottom"),
        panels(&["assets", "console"]),
        "an unpinned tab cannot move ahead of a pinned one"
    );

    controller.dispatch(WorkspaceCommand::ReorderPanel {
        panel_id: WorkspacePanelID::new("assets"),
        group_id: WorkspaceTabGroupID::new("bottom"),
        index: 2,
    });
    assert_eq!(
        panels_of(&controller, "bottom"),
        panels(&["assets", "console"]),
        "a pinned tab cannot leave the pinned band"
    );
}

#[test]
fn close_reopen_restores_panel_at_original_group_and_index() {
    let mut document = make_document();
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
        .active_panel_id = Some(WorkspacePanelID::new("assets"));
    let mut controller = WorkspaceController::new(document);

    controller.dispatch(WorkspaceCommand::ClosePanel(WorkspacePanelID::new(
        "console",
    )));

    assert_eq!(panels_of(&controller, "bottom"), panels(&["assets"]));
    assert_eq!(
        controller
            .document()
            .closed_history
            .last()
            .map(|closed| closed.panel_id.clone()),
        Some(WorkspacePanelID::new("console"))
    );

    controller.dispatch(WorkspaceCommand::ReopenLastClosed);

    assert_eq!(
        panels_of(&controller, "bottom"),
        panels(&["console", "assets"]),
        "reopen restores the original index"
    );
    assert_eq!(
        active_of(&controller, "bottom"),
        Some(WorkspacePanelID::new("console"))
    );
    assert!(controller.document().closed_history.is_empty());
}

#[test]
fn reopen_specific_panel_does_not_consume_newer_closed_panels() {
    let mut document = make_document();
    for (id, title) in [("assets", "Assets"), ("timeline", "Timeline")] {
        document
            .panels
            .insert(WorkspacePanelID::new(id), panel(id, title));
    }
    document
        .groups
        .get_mut(&WorkspaceTabGroupID::new("bottom"))
        .expect("bottom group exists")
        .panels = panels(&["console", "assets", "timeline"]);
    let mut controller = WorkspaceController::new(document);

    controller.dispatch(WorkspaceCommand::ClosePanel(WorkspacePanelID::new(
        "assets",
    )));
    controller.dispatch(WorkspaceCommand::ClosePanel(WorkspacePanelID::new(
        "timeline",
    )));
    controller.dispatch(WorkspaceCommand::ReopenPanel(WorkspacePanelID::new(
        "assets",
    )));

    assert_eq!(
        panels_of(&controller, "bottom"),
        panels(&["console", "assets"])
    );
    assert_eq!(
        active_of(&controller, "bottom"),
        Some(WorkspacePanelID::new("assets"))
    );
    assert_eq!(
        controller
            .document()
            .closed_history
            .iter()
            .map(|closed| closed.panel_id.clone())
            .collect::<Vec<_>>(),
        panels(&["timeline"]),
        "the newer closed panel stays in the history"
    );
}

#[test]
fn close_others_keeps_pinned_tabs_and_selected_tab() {
    let mut document = make_document();
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
        .pinned_panel_ids = panels(&["assets"]);
    let mut controller = WorkspaceController::new(document);

    controller.dispatch(WorkspaceCommand::CloseOthers {
        group_id: WorkspaceTabGroupID::new("bottom"),
        keeping: WorkspacePanelID::new("console"),
    });

    assert_eq!(
        panels_of(&controller, "bottom"),
        panels(&["console", "assets"]),
        "a pinned tab survives close-others"
    );
    assert_eq!(
        active_of(&controller, "bottom"),
        Some(WorkspacePanelID::new("console"))
    );
    assert!(controller.document().closed_history.is_empty());
}

#[test]
fn close_to_right_skips_pinned_tabs() {
    let mut document = make_document();
    for (id, title) in [("assets", "Assets"), ("timeline", "Timeline")] {
        document
            .panels
            .insert(WorkspacePanelID::new(id), panel(id, title));
    }
    document
        .groups
        .get_mut(&WorkspaceTabGroupID::new("bottom"))
        .expect("bottom group exists")
        .panels = panels(&["console", "assets", "timeline"]);
    document
        .groups
        .get_mut(&WorkspaceTabGroupID::new("bottom"))
        .expect("bottom group exists")
        .pinned_panel_ids = panels(&["timeline"]);
    let mut controller = WorkspaceController::new(document);

    controller.dispatch(WorkspaceCommand::CloseToTheRight {
        group_id: WorkspaceTabGroupID::new("bottom"),
        of: WorkspacePanelID::new("console"),
    });

    assert_eq!(
        panels_of(&controller, "bottom"),
        panels(&["console", "timeline"])
    );
    assert_eq!(
        controller
            .document()
            .closed_history
            .iter()
            .map(|closed| closed.panel_id.clone())
            .collect::<Vec<_>>(),
        panels(&["assets"])
    );
}

#[test]
fn unclosable_panel_cannot_close() {
    let mut controller = WorkspaceController::new(make_document());

    let result = controller.dispatch(WorkspaceCommand::ClosePanel(WorkspacePanelID::new(
        "viewport",
    )));

    assert!(!result.did_change, "an unclosable panel never closes");
    assert_eq!(panels_of(&controller, "center"), panels(&["viewport"]));
}

#[test]
fn float_group_removes_it_from_region_without_destroying_tabs() {
    let mut controller = WorkspaceController::new(make_document());
    let frame = WorkspaceRect::new(160.0, 120.0, 420.0, 320.0);

    let result = controller.dispatch(WorkspaceCommand::FloatGroup {
        group_id: WorkspaceTabGroupID::new("trailing"),
        window_id: cove_ui_addons::workspace::WorkspaceFloatingWindowID::new("inspector-window"),
        frame,
    });

    assert!(result.did_change);
    assert!(group_ids(&controller, WorkspaceSlotID::trailing()).is_empty());
    assert_eq!(panels_of(&controller, "trailing"), panels(&["inspector"]));
    let window = controller
        .document()
        .floating_windows
        .first()
        .cloned()
        .expect("the window record exists");
    assert_eq!(
        window.id,
        cove_ui_addons::workspace::WorkspaceFloatingWindowID::new("inspector-window")
    );
    assert_eq!(window.group_id, WorkspaceTabGroupID::new("trailing"));
    assert_eq!(window.frame, frame);
}

#[test]
fn redock_floating_window_restores_group_to_target_region() {
    let mut controller = WorkspaceController::new(make_document());

    controller.dispatch(WorkspaceCommand::FloatGroup {
        group_id: WorkspaceTabGroupID::new("trailing"),
        window_id: cove_ui_addons::workspace::WorkspaceFloatingWindowID::new("inspector-window"),
        frame: WorkspaceRect::new(160.0, 120.0, 420.0, 320.0),
    });
    let result = controller.dispatch(WorkspaceCommand::RedockFloatingWindow {
        window_id: cove_ui_addons::workspace::WorkspaceFloatingWindowID::new("inspector-window"),
        target: WorkspaceTarget::split(
            WorkspaceSlotID::leading(),
            WorkspaceTabGroupID::new("leading"),
            WorkspaceEdge::Trailing,
            0.5,
        ),
    });

    assert!(result.persistence_dirty);
    assert!(controller.document().floating_windows.is_empty());
    let leading = group_ids(&controller, WorkspaceSlotID::leading());
    assert_eq!(leading.len(), 2);
    assert_eq!(leading.last(), Some(&WorkspaceTabGroupID::new("trailing")));
    assert_eq!(panels_of(&controller, "trailing"), panels(&["inspector"]));
}

#[test]
fn move_panel_out_of_floating_group_does_not_duplicate_panel() {
    let mut document = make_document();
    document.panels.insert(
        WorkspacePanelID::new("debugger"),
        panel("debugger", "Debugger"),
    );
    document
        .groups
        .get_mut(&WorkspaceTabGroupID::new("trailing"))
        .expect("trailing group exists")
        .panels = panels(&["inspector", "debugger"]);
    let mut controller = WorkspaceController::new(document);

    controller.dispatch(WorkspaceCommand::FloatGroup {
        group_id: WorkspaceTabGroupID::new("trailing"),
        window_id: cove_ui_addons::workspace::WorkspaceFloatingWindowID::new("inspector-window"),
        frame: WorkspaceRect::default(),
    });
    controller.dispatch(WorkspaceCommand::MovePanel {
        panel_id: WorkspacePanelID::new("inspector"),
        target: WorkspaceTarget::tab_group(
            WorkspaceSlotID::center(),
            WorkspaceTabGroupID::new("center"),
            None,
        ),
    });

    assert_eq!(
        panels_of(&controller, "center"),
        panels(&["viewport", "inspector"])
    );
    assert_eq!(panels_of(&controller, "trailing"), panels(&["debugger"]));
    assert_eq!(
        controller
            .document()
            .floating_windows
            .first()
            .map(|window| window.group_id.clone()),
        Some(WorkspaceTabGroupID::new("trailing"))
    );
    let holders = controller
        .document()
        .groups
        .values()
        .filter(|group| group.panels.contains(&WorkspacePanelID::new("inspector")))
        .count();
    assert_eq!(holders, 1, "the panel exists in exactly one group");
}

#[test]
fn moving_last_panel_out_of_floating_window_closes_the_window_record() {
    let mut controller = WorkspaceController::new(make_document());

    controller.dispatch(WorkspaceCommand::FloatGroup {
        group_id: WorkspaceTabGroupID::new("trailing"),
        window_id: cove_ui_addons::workspace::WorkspaceFloatingWindowID::new("inspector-window"),
        frame: WorkspaceRect::default(),
    });
    controller.dispatch(WorkspaceCommand::MovePanel {
        panel_id: WorkspacePanelID::new("inspector"),
        target: WorkspaceTarget::tab_group(
            WorkspaceSlotID::center(),
            WorkspaceTabGroupID::new("center"),
            None,
        ),
    });

    assert!(controller.document().floating_windows.is_empty());
    assert!(
        !controller
            .document()
            .groups
            .contains_key(&WorkspaceTabGroupID::new("trailing"))
    );
    assert_eq!(
        panels_of(&controller, "center"),
        panels(&["viewport", "inspector"])
    );
}

#[test]
fn floating_window_move_and_focus_update_frame_and_z_order() {
    let mut controller = WorkspaceController::new(make_document());

    controller.dispatch(WorkspaceCommand::FloatGroup {
        group_id: WorkspaceTabGroupID::new("leading"),
        window_id: cove_ui_addons::workspace::WorkspaceFloatingWindowID::new("hierarchy-window"),
        frame: WorkspaceRect::new(20.0, 20.0, 300.0, 240.0),
    });
    controller.dispatch(WorkspaceCommand::FloatGroup {
        group_id: WorkspaceTabGroupID::new("trailing"),
        window_id: cove_ui_addons::workspace::WorkspaceFloatingWindowID::new("inspector-window"),
        frame: WorkspaceRect::new(40.0, 40.0, 320.0, 260.0),
    });
    controller.dispatch(WorkspaceCommand::MoveFloatingWindow {
        window_id: cove_ui_addons::workspace::WorkspaceFloatingWindowID::new("hierarchy-window"),
        frame: WorkspaceRect::new(80.0, 90.0, 300.0, 240.0),
    });
    controller.dispatch(WorkspaceCommand::FocusFloatingWindow(
        cove_ui_addons::workspace::WorkspaceFloatingWindowID::new("hierarchy-window"),
    ));

    let windows = &controller.document().floating_windows;
    let hierarchy = windows
        .iter()
        .find(|window| {
            window.id
                == cove_ui_addons::workspace::WorkspaceFloatingWindowID::new("hierarchy-window")
        })
        .expect("hierarchy window exists");
    let inspector = windows
        .iter()
        .find(|window| {
            window.id
                == cove_ui_addons::workspace::WorkspaceFloatingWindowID::new("inspector-window")
        })
        .expect("inspector window exists");
    assert_eq!(
        hierarchy.frame,
        WorkspaceRect::new(80.0, 90.0, 300.0, 240.0),
        "the move updates the frame"
    );
    assert!(
        hierarchy.z_index > inspector.z_index,
        "focus raises the window above the other one"
    );
}
