//! Translation of `Tests/CoveUIWorkspaceTests/PanelWorkspaceIntegrationTests.swift`.
//!
//! Behaviour checklist:
//! - a panel registry exports typed workspace descriptors with slot and icon
//! - installing a `WorkspaceView` over a registry renders the workspace shell
//!
//! GAP (structural, documented): Swift's `SurfaceReader` reads the
//! composition-local `UIEnvironment`, whose default viewport is 1280x720 and
//! therefore always `expanded`, so the docked shell rendered at any layout size.
//! The Rust `WorkspaceSurfaceReader` measures the container instead, so the
//! docked shell appears only at an expanded width. The second case lays out at
//! 1280x720 — the Swift default viewport — to assert the same shell nodes.

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::{AnyView, EmptyView, ViewGraph};
use cove_core::scene::{NodeTree, Recomposer};
use cove_ui_addons::workspace::{
    WorkspaceController, WorkspaceDocument, WorkspacePanel, WorkspacePanelDescriptor,
    WorkspacePanelID, WorkspacePanelRegistry, WorkspaceSlot, WorkspaceSlotID, WorkspaceTabGroup,
    WorkspaceTabGroupID, WorkspaceView,
};
use cove_ui_compose::primitives::text::Text;

fn descriptor(id: &str, title: &str, slot: WorkspaceSlotID) -> WorkspacePanelDescriptor {
    WorkspacePanelDescriptor::new(WorkspacePanelID::new(id), title).default_slot(slot)
}

#[test]
fn panel_registry_exports_typed_workspace_descriptors() {
    let registry = WorkspacePanelRegistry::new([
        descriptor("hierarchy", "Hierarchy", WorkspaceSlotID::leading())
            .configure(|panel| {
                panel.is_collapsible = true;
                panel.icon_asset_key = Some("hierarchy".to_string());
            })
            .content(|| AnyView::new(EmptyView)),
        descriptor("inspector", "Inspector", WorkspaceSlotID::trailing())
            .configure(|panel| {
                panel.is_collapsible = true;
                panel.icon_asset_key = Some("inspector".to_string());
            })
            .content(|| AnyView::new(EmptyView)),
    ]);

    let hierarchy = registry
        .descriptor(&WorkspacePanelID::new("hierarchy"))
        .expect("hierarchy is registered");
    assert_eq!(hierarchy.default_slot, WorkspaceSlotID::leading());
    let inspector = registry
        .descriptor(&WorkspacePanelID::new("inspector"))
        .expect("inspector is registered");
    assert_eq!(inspector.default_slot, WorkspaceSlotID::trailing());
    assert_eq!(
        hierarchy.panel.icon_asset_key.as_deref(),
        Some("hierarchy")
    );
}

#[test]
fn panel_workspace_renders_new_workspace_controller() {
    let registry = WorkspacePanelRegistry::new([
        descriptor("viewport", "Viewport", WorkspaceSlotID::center())
            .configure(|panel| {
                panel.is_collapsible = false;
                panel.is_closable = false;
            })
            .content(|| AnyView::new(Text::new("Viewport"))),
        descriptor("console", "Console", WorkspaceSlotID::bottom())
            .configure(|panel| panel.is_collapsible = true)
            .content(|| AnyView::new(Text::new("Console"))),
    ]);

    let panels = std::collections::BTreeMap::from([
        (
            WorkspacePanelID::new("viewport"),
            WorkspacePanel {
                is_closable: false,
                ..WorkspacePanel::new(WorkspacePanelID::new("viewport"), "Viewport")
            },
        ),
        (
            WorkspacePanelID::new("console"),
            WorkspacePanel::new(WorkspacePanelID::new("console"), "Console"),
        ),
    ]);
    let groups = std::collections::BTreeMap::from([
        (
            WorkspaceTabGroupID::new("center"),
            WorkspaceTabGroup::new(
                WorkspaceTabGroupID::new("center"),
                vec![WorkspacePanelID::new("viewport")],
            ),
        ),
        (
            WorkspaceTabGroupID::new("bottom"),
            WorkspaceTabGroup::new(
                WorkspaceTabGroupID::new("bottom"),
                vec![WorkspacePanelID::new("console")],
            ),
        ),
    ]);
    let mut slots = WorkspaceSlot::standard_editor_slots();
    slots.insert(
        WorkspaceSlotID::center(),
        WorkspaceSlot::new(
            WorkspaceSlotID::center(),
            cove_ui_addons::workspace::WorkspaceSlotKind::Content,
            Some(cove_ui_addons::workspace::WorkspaceLayoutNode::group(
                WorkspaceTabGroupID::new("center"),
            )),
        ),
    );
    slots.insert(
        WorkspaceSlotID::bottom(),
        WorkspaceSlot::new(
            WorkspaceSlotID::bottom(),
            cove_ui_addons::workspace::WorkspaceSlotKind::Content,
            Some(cove_ui_addons::workspace::WorkspaceLayoutNode::group(
                WorkspaceTabGroupID::new("bottom"),
            )),
        ),
    );
    let controller = Rc::new(RefCell::new(WorkspaceController::new(
        WorkspaceDocument::new(panels, groups, slots),
    )));

    let graph = Rc::new(ViewGraph::new(
        Rc::new(RefCell::new(NodeTree::new())),
        Rc::new(Recomposer::new()),
    ));
    graph.install(
        WorkspaceView::with_registry(controller, &registry),
        Default::default(),
    );
    for _ in 0..4 {
        graph.recomposer.commit_all();
        graph.compute_layout(1280.0, 720.0);
    }

    let snapshot = graph.layout_snapshot();
    for name in ["workspace", "workspace-region-center", "workspace-region-bottom"] {
        assert!(
            snapshot
                .iter()
                .any(|entry| entry.debug_name.as_deref() == Some(name)),
            "the snapshot must contain {name}"
        );
    }
}
