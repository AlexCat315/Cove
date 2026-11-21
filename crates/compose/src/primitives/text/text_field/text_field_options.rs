//! Translation of Primitives/Text/TextField/TextFieldOptions.swift.
//!
//! Authored layout, behavior and decoration settings. Cursor, selection, IME and
//! scrolling remain node-local.
use cove_core::Color;
use cove_imaging::BundleImageResource;

use crate::primitives::text::code::folding_options::TextEditorFolding;
use crate::primitives::text::code::gutter_options::TextEditorGutter;
use crate::primitives::text::text_buffer::TextBuffer;
use crate::primitives::text::text_completion_rebase::{
    TextCompletionItem, TextCompletionProvider, TextCompletionResolver,
};
use crate::primitives::text::text_diagnostic::TextDiagnostics;
use crate::primitives::text::text_edit_history::TextEditHistory;
use crate::primitives::text::text_field::text_field_observation::TextFieldHoverAnchor;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextFieldAxis {
    #[default]
    Horizontal,
    Vertical,
}

/// Optional per-line opt-out for the paragraph indent. The editor decides
/// which lines are body paragraphs; everything else (chapter titles, a book
/// title) stays flush.
#[derive(Clone, Copy, Debug, Default)]
pub struct TextFieldIndentSkip(Option<fn(&str) -> bool>);

impl TextFieldIndentSkip {
    pub fn new(skip: fn(&str) -> bool) -> Self {
        Self(Some(skip))
    }

    pub fn skips(&self, text: &str) -> bool {
        self.0.is_some_and(|skip| skip(text))
    }
}

impl PartialEq for TextFieldIndentSkip {
    fn eq(&self, other: &Self) -> bool {
        match (self.0, other.0) {
            (None, None) => true,
            (Some(left), Some(right)) => std::ptr::fn_addr_eq(left, right),
            _ => false,
        }
    }
}

/// Authored layout settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextFieldLayout {
    pub axis: TextFieldAxis,
    pub max_visible_lines: usize,
    pub ideal_width: f32,
    pub wraps_lines: bool,
    /// Paragraph indent applied to the first visual row of each wrapped line,
    /// in points. Zero keeps the old flush-left layout.
    pub first_line_indent: f32,
    /// Lines this filter accepts stay flush, even with an indent configured.
    pub first_line_indent_skip: TextFieldIndentSkip,
    /// Keep the caret row near the vertical middle of the viewport while the
    /// document scrolls, instead of only revealing it at the bottom edge.
    pub typewriter_scrolling: bool,
}

impl Default for TextFieldLayout {
    fn default() -> Self {
        Self {
            axis: TextFieldAxis::Horizontal,
            max_visible_lines: 6,
            ideal_width: 160.0,
            wraps_lines: true,
            first_line_indent: 0.0,
            first_line_indent_skip: TextFieldIndentSkip::default(),
            typewriter_scrolling: false,
        }
    }
}

impl TextFieldLayout {
    pub fn new() -> Self {
        Self::default()
    }

    fn validate(&mut self) {
        self.max_visible_lines = self.max_visible_lines.max(1);
        self.first_line_indent = if self.first_line_indent.is_finite() {
            self.first_line_indent.max(0.0)
        } else {
            0.0
        };
    }

    pub fn validated(mut self) -> Self {
        self.validate();
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum TextFieldSize {
    #[default]
    Automatic,
    Mini,
    Small,
    Regular,
    Large,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextFieldBehavior {
    pub disabled: bool,
    pub read_only: bool,
    pub secure: bool,
    pub clearable: bool,
    pub max_length: Option<usize>,
}

impl TextFieldBehavior {
    pub fn new() -> Self {
        Self::default()
    }

    fn validate(&mut self) {
        // `usize` cannot be negative; the Swift clamp only guards authored
        // negatives, which the type system already excludes.
    }

    pub fn validated(mut self) -> Self {
        self.validate();
        self
    }
}

/// Decoration is independent of editing behavior and can be copied as a group.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextFieldDecoration {
    pub size: TextFieldSize,
    pub show_word_limit: bool,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
    pub prepend: Option<String>,
    pub append: Option<String>,
    /// Decorative leading glyph rendered inside the field chrome. It reserves
    /// text inset but is not hit-testable; use `prefix` for authored text.
    pub leading_icon: Option<BundleImageResource>,
    pub text_color: Option<Color>,
    pub placeholder_color: Option<Color>,
    pub cursor_color: Option<Color>,
    pub selection_color: Option<Color>,
    /// Space between the field edge and its text, replacing the size-derived
    /// insets. The padding lies inside the scrolled region, so a wide editor
    /// can center a readable column and still scroll and take clicks across
    /// its whole width.
    pub content_padding: Option<TextFieldContentPadding>,
}

impl TextFieldDecoration {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Authored text insets for [`TextFieldDecoration::content_padding`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextFieldContentPadding {
    /// Applied on both the leading and trailing edges.
    pub horizontal: f32,
    /// Applied above the first line and below the last.
    pub vertical: f32,
}

impl TextFieldContentPadding {
    pub fn new(horizontal: f32, vertical: f32) -> Self {
        Self {
            horizontal: horizontal.max(0.0),
            vertical: vertical.max(0.0),
        }
    }
}

/// One-shot navigation requests, separate from the live caret/selection state.
#[derive(Clone, Debug, PartialEq)]
pub struct TextFieldNavigationRequest {
    pub focus_request_id: Option<u64>,
    pub caret_request_id: Option<u64>,
    pub caret_request_index: Option<usize>,
    /// Selection anchor for the caret request; the caret sits at the other
    /// end. `None` (or equal to the index) leaves a plain caret.
    pub caret_request_anchor: Option<usize>,
    /// Whether the caret request also focuses the field. Find-style navigation
    /// moves the caret and selection while another field keeps the keyboard.
    pub caret_request_focus: bool,
}

impl Default for TextFieldNavigationRequest {
    fn default() -> Self {
        Self {
            focus_request_id: None,
            caret_request_id: None,
            caret_request_index: None,
            caret_request_anchor: None,
            caret_request_focus: true,
        }
    }
}

impl TextFieldNavigationRequest {
    pub fn new() -> Self {
        Self::default()
    }
}

/// A one-shot programmatic edit. Unlike rewriting the bound text, the request
/// is applied inside the field's undo bracket, so the step can be undone and
/// the caret/selection land where the caller asks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextFieldEditRequest {
    /// Monotonic id; a request applies once per id.
    pub id: u64,
    /// Character range to replace.
    pub range: std::ops::Range<usize>,
    pub replacement: String,
    pub caret: usize,
    /// Selection anchor; `None` leaves a plain caret at `caret`.
    pub anchor: Option<usize>,
}

/// Code-editing payload carried alongside a plain text field.
#[derive(Clone, Default)]
pub struct TextFieldCodeEditing {
    pub gutter: TextEditorGutter,
    pub folding: TextEditorFolding,
    pub diagnostics: TextDiagnostics,
    /// Asked for candidates once typing pauses, or at once on Ctrl+Space.
    pub on_request_completion: Option<TextCompletionProvider>,
    /// Asked once per highlighted item that arrived without documentation.
    pub on_resolve_completion: Option<TextCompletionResolver>,
    pub on_accept_completion: Option<std::rc::Rc<dyn Fn(TextCompletionItem)>>,
    /// F1 requests documentation at the visible caret, also in read-only code.
    pub on_request_hover: Option<std::rc::Rc<dyn Fn(TextFieldHoverAnchor)>>,
    pub shows_line_numbers: bool,
    /// A scaled overview of the whole document on the trailing edge, with the
    /// visible region marked; clicking or dragging it scrolls. Vertical fields
    /// only.
    pub shows_minimap: bool,
    /// UTF-8 ranges marked as the same symbol as the one at the caret.
    pub occurrences: Vec<std::ops::Range<usize>>,
    pub indentation_width: Option<usize>,
    pub edit_history: Option<TextEditHistory>,
    pub line_number_color: Option<Color>,
    pub line_number_gutter_color: Option<Color>,
    pub syntax_revision: Option<u64>,
    pub syntax_color_at_utf8_offset:
        Option<std::rc::Rc<dyn Fn(&TextBuffer, usize) -> Option<Color>>>,
}

impl TextFieldCodeEditing {
    pub fn new() -> Self {
        Self::default()
    }

    fn validate(&mut self) {
        self.indentation_width = self.indentation_width.map(|width| width.clamp(1, 8));
    }

    pub fn validated(mut self) -> Self {
        self.validate();
        self
    }

    /// Comparable projection of the settings the surface paints, for the draw
    /// identity. Closures cannot be compared, so a highlighter whose output
    /// changes without its presence changing must bump `syntax_revision`.
    pub(crate) fn draw_identity(
        &self,
    ) -> (
        Vec<std::ops::Range<usize>>,
        bool,
        Option<u64>,
        bool,
        bool,
        Option<usize>,
        Option<Color>,
        Option<Color>,
    ) {
        (
            self.occurrences.clone(),
            self.syntax_color_at_utf8_offset.is_some(),
            self.syntax_revision,
            self.shows_line_numbers,
            self.shows_minimap,
            self.indentation_width,
            self.line_number_color,
            self.line_number_gutter_color,
        )
    }
}

/// Node-owned mutable payload for the code-editing settings.
#[derive(Default)]
pub struct TextFieldCodeStorage {
    pub value: TextFieldCodeEditing,
}

impl TextFieldCodeStorage {
    pub fn new(value: TextFieldCodeEditing) -> Self {
        Self { value }
    }
}
