//! Code-editor chrome: gutters, folding, minimap and occurrences.
pub mod folding_options;
pub mod gutter_options;

pub use folding_options::{TextEditorFolding, TextFoldRegion, TextFoldingLabels, TextLineRebase};
pub use gutter_options::{
    TextBreakpoint, TextBreakpointState, TextEditorAnnotations, TextEditorGutter,
    TextGutterLabels, TextGutterMarker,
};
