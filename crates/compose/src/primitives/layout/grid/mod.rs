//! Table-shaped grid containers: column policies, named columns, row windows
//! and span bookkeeping.
//!
//! Layer split — there is exactly one grid geometry engine, Taffy's CSS Grid:
//! - authored CSS Grid ([`crate::modifiers::layout::GridExt`], `GridSpec`)
//!   declares tracks, gaps, named areas and per-item placement directly;
//! - [`GridLayout`] / [`Grid`] / [`LazyGrid`] are the *table policy* layer: they
//!   translate a column policy (fixed, flexible, adaptive, named-plus-visibility)
//!   into a `GridStyle`, let Taffy solve the tracks and cells, and add what CSS
//!   has no equivalent for (column visibility with slot preservation, row
//!   windows over a demand-measured viewport).
//!
//! Nothing here re-implements track sizing. [`GridTrackResolver`] and
//! [`GridTracks`] are the policy preview (which columns exist and how wide the
//! policy wants them) used by the table containers and their tests; the final
//! geometry always comes from the engine.
pub mod columns;
pub mod grid;
pub mod lazy_grid;
pub mod named_columns;
pub mod row_heights;
pub mod span;

pub use columns::{
    ColumnVisibility, GridColumn, GridTrackResolver, GridTracks, GridTracksGroup,
    GridTracksIterator,
};
pub use grid::{Grid, GridLayout, named_column_plan};
pub use lazy_grid::{LazyGrid, LazyGridError, LazyGridOptions, LazyGridRows, LazyGridView};
pub use named_columns::{
    ColumnVisibilityPlan, ColumnVisibilityTrack, GridColumnError, GridNamedColumns,
};
pub use row_heights::GridRowHeights;
pub use span::{GridSpan, GridSpanCell, GridSpanPlacement};
