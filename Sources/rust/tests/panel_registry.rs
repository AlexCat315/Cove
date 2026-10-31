//! Translation of `Tests/CoveUIWorkspaceTests/PanelRegistryTests.swift`.
//!
//! Behaviour checklist:
//! - registering keeps insertion order and resolves by id
//! - re-registering an existing id replaces it without moving it
//! - updating a descriptor changes the panel but keeps the order and factory
//! - unregistering drops both the entry and its order slot
//! - `make` on an unknown id returns an empty view instead of panicking

mod workspace_support;

use cove_ui_addons::workspace::{
    WorkspacePanelDescriptor, WorkspacePanelID, WorkspacePanelRegistry, WorkspaceSlotID,
};

fn descriptor(id: &str, title: &str) -> WorkspacePanelDescriptor {
    WorkspacePanelDescriptor::new(WorkspacePanelID::new(id), title)
        .configure(|panel| panel.is_collapsible = false)
        .default_slot(WorkspaceSlotID::center())
}

#[test]
fn register_and_resolve() {
    let mut registry = WorkspacePanelRegistry::default();
    assert!(registry.is_empty());

    registry.register(descriptor("hierarchy", "Scene"));
    registry.register(descriptor("inspector", "Inspector"));

    assert_eq!(registry.count(), 2);
    assert_eq!(
        registry.ids(),
        [WorkspacePanelID::new("hierarchy"), WorkspacePanelID::new("inspector")]
    );
    assert_eq!(
        registry.descriptor(&WorkspacePanelID::new("hierarchy")).map(|d| d.panel.title.as_str()),
        Some("Scene")
    );
    assert_eq!(
        registry.descriptor(&WorkspacePanelID::new("missing")).map(|d| d.panel.title.as_str()),
        None
    );
}

#[test]
fn register_overwrite_keeps_order() {
    let mut registry = WorkspacePanelRegistry::new([
        descriptor("a", "A"),
        descriptor("b", "B"),
        descriptor("c", "C"),
    ]);

    registry.register(descriptor("b", "B'"));

    assert_eq!(
        registry.ids(),
        [
            WorkspacePanelID::new("a"),
            WorkspacePanelID::new("b"),
            WorkspacePanelID::new("c")
        ],
        "replacing a descriptor must not move it"
    );
    assert_eq!(
        registry.descriptor(&WorkspacePanelID::new("b")).map(|d| d.panel.title.as_str()),
        Some("B'")
    );
}

#[test]
fn update_descriptor_keeps_factory_and_order() {
    let mut registry = WorkspacePanelRegistry::new([
        descriptor("a", "A").content(|| {
            cove_composition::AnyView::new(cove_ui_compose::primitives::text::Text::new("A"))
        }),
        descriptor("b", "B").content(|| {
            cove_composition::AnyView::new(cove_ui_compose::primitives::text::Text::new("B"))
        }),
    ]);

    registry.update_descriptor(&WorkspacePanelID::new("b"), |descriptor| {
        descriptor.panel.title = "Localized B".to_string();
    });

    assert_eq!(
        registry.ids(),
        [WorkspacePanelID::new("a"), WorkspacePanelID::new("b")]
    );
    assert_eq!(
        registry.descriptor(&WorkspacePanelID::new("b")).map(|d| d.panel.title.as_str()),
        Some("Localized B")
    );
    // The factory still produces a view after the metadata update.
    let _ = registry.make(&WorkspacePanelID::new("b"));
}

#[test]
fn unregister_drops_the_id_and_its_order_slot() {
    let mut registry = WorkspacePanelRegistry::new([descriptor("a", "A"), descriptor("b", "B")]);

    registry.unregister(&WorkspacePanelID::new("a"));

    assert_eq!(registry.ids(), [WorkspacePanelID::new("b")]);
    assert!(registry.descriptor(&WorkspacePanelID::new("a")).is_none());
}

#[test]
fn make_returns_empty_for_unknown_id() {
    let registry = WorkspacePanelRegistry::default();
    // A missing tab user key renders an empty view in the workspace, so the
    // failure mode for a dock that outlives its panel is blank, not a crash.
    let _ = registry.make(&WorkspacePanelID::new("nope"));
}

#[test]
fn a_descriptor_update_cannot_change_panel_identity() {
    let mut registry = WorkspacePanelRegistry::new([descriptor("a", "A")]);

    registry.update_descriptor(&WorkspacePanelID::new("a"), |descriptor| {
        descriptor.panel.id = WorkspacePanelID::new("hijacked");
    });

    // The identity is the map key, so a mismatching descriptor replaces nothing.
    assert_eq!(registry.ids(), [WorkspacePanelID::new("a")]);
    assert_eq!(
        registry.descriptor(&WorkspacePanelID::new("a")).map(|d| d.panel.id.clone()),
        Some(WorkspacePanelID::new("a"))
    );
}
