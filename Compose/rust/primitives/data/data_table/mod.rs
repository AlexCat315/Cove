//! Data table geometry, interaction state and the two-axis virtualized view.
pub mod column_order;
pub mod data_table_geometry;
pub mod editing_model;
pub mod hosts_model;
pub mod model;
pub mod row_metrics;
pub mod selection_model;
pub mod view;

pub use column_order::DataTableColumnOrder;
pub use data_table_geometry::DataTableColumnGeometry;
pub use editing_model::{
    CellDraft, CellEditing, CommitAnswer, CommitOutcome, DataTableCellDraft,
    DataTableEditingState, EditableColumn,
};
pub use hosts_model::{
    drop_indicator_x, editor_blur_outcome, CellHostUpdate, EditorBlurOutcome, RowAccessibilityRole,
    RowHostUpdate,
};
pub use model::{DataTableModel, DataTableSortingStatus};
pub use row_metrics::{
    accepts_projection, row_height_key_matches, variable_row_heights, DataTableProjection,
    DataTableRowMetrics, DataTableSortRequest, RowHeightKey,
};
pub use selection_model::{
    DataTableNavigation, DataTableSelection, DataTableSelectionMode,
};
pub use view::{DataTable, DataTableOptions, DataTableWindows};
