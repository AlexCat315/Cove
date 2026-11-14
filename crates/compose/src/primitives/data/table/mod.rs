//! Data table geometry, interaction state and the two-axis virtualized view.
pub mod column_order;
pub mod editing_model;
pub mod hosts_model;
pub mod model;
pub mod row_metrics;
pub mod selection_model;
pub mod table_column;
pub mod table_geometry;
pub mod view;

pub use column_order::TableColumnOrder;
pub use editing_model::{
    CellDraft, CellEditing, CommitAnswer, CommitOutcome, EditableColumn, TableCellDraft,
    TableEditingState,
};
pub use hosts_model::{
    CellHostUpdate, EditorBlurOutcome, RowAccessibilityRole, RowHostUpdate, drop_indicator_x,
    editor_blur_outcome,
};
pub use model::{TableModel, TableSortingStatus};
pub use row_metrics::{
    RowHeightKey, TableProjection, TableRowMetrics, TableSortRequest, accepts_projection,
    row_height_key_matches, variable_row_heights,
};
pub use selection_model::{TableNavigation, TableSelection, TableSelectionMode};
pub use table_column::{
    ColumnVisibility, ComparisonResult, TableColumn, TableColumnLayout, TableColumnPlan,
    TableLayout, TableSort, TableSortDirection, TableTextEditing,
};
pub use table_geometry::TableColumnGeometry;
pub use view::{Table, TableOptions, TableRowHover, TableWindows};
