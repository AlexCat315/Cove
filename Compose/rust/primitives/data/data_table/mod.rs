//! Data table geometry and interaction state.
pub mod column_order;
pub mod data_table_geometry;
pub mod hosts_model;
pub mod editing_model;
pub mod row_metrics;

pub use column_order::DataTableColumnOrder;
pub use data_table_geometry::DataTableColumnGeometry;
pub use hosts_model::{
    drop_indicator_x, editor_blur_outcome, CellHostUpdate, EditorBlurOutcome, RowAccessibilityRole,
    RowHostUpdate,
};
pub use editing_model::{
    CommitAnswer, CommitOutcome, DataTableCellDraft, DataTableEditingState, EditableColumn,
};
pub use row_metrics::{
    accepts_projection, row_height_key_matches, variable_row_heights, DataTableProjection,
    DataTableRowMetrics, DataTableSortRequest, RowHeightKey,
};
