//! Translation of Primitives/Text/TextField/TextFieldOptions.swift.
//!
//! Authored layout, behavior and decoration settings. Cursor, selection, IME and
//! scrolling remain node-local.
use cove_core::Color;
use cove_imaging::BundleImageResource;

use crate::primitives::text::text_diagnostic::TextDiagnostics;
use crate::primitives::text::text_edit_history::TextEditHistory;
use crate::primitives::text::code::folding_options::TextEditorFolding;
use crate::primitives::text::code::gutter_options::TextEditorGutter;
use crate::primitives::text::text_completion_rebase::TextCompletionItem;
use crate::primitives::text::text_field::text_field_observation::TextFieldHoverAnchor;
use crate::primitives::text::text_buffer::TextBuffer;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextFieldAxis {
    #[default]
    Horizontal,
    Vertical,
}

/// Authored layout settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextFieldLayout {
    pub axis: TextFieldAxis,
    pub max_visible_lines: usize,
    pub ideal_width: f32,
    pub wraps_lines: bool,
}

impl Default for TextFieldLayout {
    fn default() -> Self {
        Self {
            axis: TextFieldAxis::Horizontal,
            max_visible_lines: 6,
            ideal_width: 160.0,
            wraps_lines: true,
        }
    }
}

impl TextFieldLayout {
    pub fn new() -> Self {
        Self::default()
    }

    fn validate(&mut self) {
        self.max_visible_lines = self.max_visible_lines.max(1);
    }

    pub fn validated(mut self) -> Self {
        self.validate();
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextFieldSize {
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
}

impl Default for TextFieldSize {
    fn default() -> Self {
        Self::Automatic
    }
}

impl TextFieldDecoration {
    pub fn new() -> Self {
        Self::default()
    }
}

/// One-shot navigation requests, separate from the live caret/selection state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextFieldNavigationRequest {
    pub focus_request_id: Option<u64>,
    pub caret_request_id: Option<u64>,
    pub caret_request_index: Option<usize>,
}

impl TextFieldNavigationRequest {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Code-editing payload carried alongside a plain text field.
#[derive(Clone, Default)]
pub struct TextFieldCodeEditing {
    pub gutter: TextEditorGutter,
    pub folding: TextEditorFolding,
    pub diagnostics: TextDiagnostics,
    pub on_request_completion: Option<std::rc::Rc<dyn Fn(&TextFieldCompletionRequest)>>,
    /// Asked once per highlighted item that arrived without documentation.
    pub on_resolve_completion: Option<std::rc::Rc<dyn Fn(TextCompletionItem)>>,
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
    pub syntax_color_at_utf8_offset: Option<std::rc::Rc<dyn Fn(&TextBuffer, usize) -> Option<Color>>>,
}

/// What the field asks a completion provider for at the caret.
#[derive(Clone, Debug, PartialEq)]
pub struct TextFieldCompletionRequest {
    pub buffer: TextBuffer,
    pub caret: usize,
    pub anchor: Option<cove_core::Point>,
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
}

/// Node-owned mutable payload for the code-editing settings.
pub struct TextFieldCodeStorage {
    pub value: TextFieldCodeEditing,
}

impl TextFieldCodeStorage {
    pub fn new(value: TextFieldCodeEditing) -> Self {
        Self { value }
    }
}

impl Default for TextFieldCodeStorage {
    fn default() -> Self {
        Self { value: TextFieldCodeEditing::default() }
    }
}
