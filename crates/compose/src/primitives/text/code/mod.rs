//! Code-editor chrome: gutters, folding, minimap and occurrences.
pub mod folding_options;
pub mod gutter_controls;
pub mod gutter_geometry;
pub mod gutter_layout;
pub mod gutter_options;
pub mod gutter_session;
pub mod minimap;
pub mod occurrences;

pub use folding_options::{TextEditorFolding, TextFoldRegion, TextFoldingLabels, TextLineRebase};
pub use gutter_controls::TextGutterControls;
pub use gutter_geometry::{TextGutterControl, TextGutterControlKind, TextGutterGeometry};
pub use gutter_layout::{
    GutterRow, TextGutterInputs, distinct_lines, folded_breakpoint_is_active, folded_breakpoints,
    gutter_geometry, update_gutter_controls,
};
pub use gutter_options::{
    TextBreakpoint, TextBreakpointState, TextEditorAnnotations, TextEditorGutter, TextGutterLabels,
    TextGutterMarker,
};
pub use gutter_session::{TextFoldSession, TextGutterSession};
pub use minimap::{MINIMAP_WIDTH, MinimapGeometry, ink_runs};
pub use occurrences::draw_occurrences;
