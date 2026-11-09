//! Editable text field: layout indexing, options, editing and navigation.
pub mod bracket_matching;
pub mod indent_guides;
pub mod platform_input;
pub mod snippet;
pub mod text_completion_session;
pub mod text_field;
pub mod text_field_code_editing;
pub mod text_field_editing;
pub mod text_field_horizontal_scroll;
pub mod text_field_layout_engine;
pub mod text_field_navigation;
pub mod text_field_navigation_options;
pub mod text_field_observation;
pub mod text_field_options;
pub mod text_visual_line_index;

pub use bracket_matching::{BracketPair, TextBracketMatching};
pub use indent_guides::{IndentGuideSegment, TextIndentGuides};
pub use platform_input::{character_index, platform_input_client, utf16_offset};
pub use snippet::{TextInsertFormat, TextSnippet, TextSnippetSession, TextSnippetStop};
pub use text_field::{FieldState, TextField, TextFieldEvents};
pub use text_field_code_editing::{IndentResult, TextEditKind, indent_lines, indented_newline};
pub use text_field_editing::{
    caret_state, delete_selection, insert_replacing_selection, move_cursor, normalize_indices,
    record_caret_activity,
};
pub use text_field_horizontal_scroll::HorizontalScrollState;
pub use text_field_layout_engine::{
    FieldCompositionState, FieldLayoutCache, FieldLayoutInputs, FieldScrollState,
    FieldSelectionState, RenderCacheEntry, RenderCacheKey, RenderDraftState, RenderState,
    ScrollbarMetrics, ViewportMetrics, document_geometry, horizontal_scrollbar_metrics,
    make_render_state, scrollbar_metrics, update_scroll, visible_layout,
};
pub use text_field_navigation::{
    CaretMove, character_index_at_point, line_boundary, move_cursor_by_page, move_cursor_vertically,
};
pub use text_field_navigation_options::TextFieldNavigation;
pub use text_field_observation::{TextFieldCaretState, TextFieldHoverAnchor};
pub use text_field_options::{
    TextFieldAxis, TextFieldBehavior, TextFieldCodeEditing, TextFieldCodeStorage,
    TextFieldContentPadding, TextFieldDecoration, TextFieldLayout, TextFieldNavigationRequest,
    TextFieldSize,
};
