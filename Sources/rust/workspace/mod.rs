//! Workspace panels: authored document, command controller, layout sizing,
//! theme metrics, icons and the composed view.
//!
//! The document is pure data; every mutation goes through `WorkspaceController`
//! so observers, maximize/focus reconciliation and persistence dirtiness stay in
//! one place. The view layer reads only the document and the controller.
mod controller;
mod drag;
mod icons;
pub mod ids;
mod model;
mod sizing;
mod tab_style;
mod theme;
mod view;
mod workspace_view;

pub use controller::{
    SubscriptionToken, WorkspaceCommand, WorkspaceController, WorkspaceHitTarget, WorkspaceTarget,
    WorkspaceTransactionResult,
};
pub use drag::{
    WorkspaceDragResolution, WorkspaceDropPreview, WorkspaceReorderPreview,
    WorkspaceTabDragOverlayState, drag_overlay, first_node, resolve_drag, set_drag_overlay,
    tab_band, tab_frames, workspace_root,
};
pub use icons::{WORKSPACE_RESOURCE_BUNDLE_NAME, WorkspacePanelIconCatalog};
pub use model::{
    WorkspaceClosedPanel, WorkspaceCollapsedItem, WorkspaceDocument, WorkspaceFloatingWindow,
    WorkspaceLayoutNode, WorkspaceLayoutSplit, WorkspacePanel, WorkspacePanelDescriptor,
    WorkspacePanelRegistry, WorkspaceSlot, WorkspaceSlotKind, WorkspaceSlotSize, WorkspaceTabGroup,
};
pub use sizing::{
    SplitConstraints, WorkspaceAxisSizing, WorkspaceLayoutMetrics, WorkspacePanelSizing,
    WorkspaceRect, WorkspaceResolvedSplit, WorkspaceSizingResolver, WorkspaceSplitFractions,
};
pub use tab_style::WorkspaceTabStyle;
pub use theme::{
    WorkspaceRegionSizing, WorkspaceTheme, WorkspaceThemeEnvironment, resolve_workspace_theme,
};
pub use view::{
    WorkspaceBox, WorkspaceLabel, WorkspacePane, WorkspaceRegionTransition,
    WorkspaceResizeGeometry, WorkspaceVerticalTitle, chrome_rail_slots, chrome_thickness,
    collapsed_groups_on, compact_layout, rail_edge, rail_groups, rail_plan, renderable_layout,
    visible_chrome_rail_slots, visible_groups,
};
pub use workspace_view::{
    WorkspaceContent, WorkspaceLayoutNodeView, WorkspaceRailReserve, WorkspaceSplitDivider,
    WorkspaceTabGroupView, WorkspaceView,
};

pub use ids::{
    WorkspaceEdge, WorkspaceFloatingWindowID, WorkspacePanelID, WorkspacePlacementKind,
    WorkspaceSlotID, WorkspaceSplitAxis, WorkspaceSplitID, WorkspaceTabGroupID,
};
