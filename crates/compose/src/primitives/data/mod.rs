//! Data display primitives.
pub mod avatar;
pub mod chart;
pub mod columns;
pub mod data_table;
pub mod lists;
pub(crate) mod property_grid;
pub(crate) mod timeline;
pub(crate) mod tree_table;

pub use api::*;

pub(crate) mod api {
    pub use super::avatar::{
        Avatar, AvatarAppearance, AvatarColors, AvatarGroup, AvatarGroupGeometry,
        AvatarGroupOptions, AvatarMemberList, AvatarOverflow, AvatarShape, AvatarStatus,
    };
    pub use super::chart::{
        CHART_DATA_GAP, Chart, ChartConfiguration, ChartDataTable, ChartDataTableEntry, ChartDatum,
        ChartGuides, ChartMark, ChartSeries, ChartSlice, ChartStyle, ChartXDomain,
    };
    pub use super::data_table::{
        DataTable, DataTableModel, DataTableOptions, TableColumn, TableColumnLayout, TableLayout,
        TableSort, TableSortDirection, TableTextEditing,
    };
    pub use super::lists::{
        CollectionId, List, ListGroup, ListRow, ListRowOptions, ListSection, RefreshOptions,
        ReorderSession, ReorderableList, RowState, SectionedEntryId, SectionedList, Selection,
        StickyHeader, SwipeAction, SwipeList, Table, TableOptions, Tree, TreeEntry,
        TreeSearchMetadata, flatten_tree,
    };
    pub use super::property_grid::{
        PropertyGrid, PropertyGridRow, PropertyGridRowLayout, PropertyGridRowSizing,
        PropertyGridScrollAxes, PropertyGridSection,
    };
    pub use super::timeline::{
        Timeline, TimelineEvent, TimelineGroup, TimelineSpacing, TimelineZoom,
    };
    pub use super::tree_table::{
        TreeTable, TreeTableColumn, TreeTableOptions, TreeTableRow, flatten_tree_table,
    };
}
