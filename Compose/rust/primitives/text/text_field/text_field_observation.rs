//! Translation of Primitives/Text/TextField/TextFieldObservation.swift.
//!
//! Where the pointer is resting inside a text field, in editor terms, plus the
//! caret/selection snapshot reported whenever either moves.

/// Where the pointer is resting inside a text field, in editor terms.
///
/// A text input measures offsets in character units; higher layers (language
/// servers, for example) need line/UTF-16 columns, and a screen position has to
/// be known before anything can anchor a popup. Both are reported here so each
/// consumer converts once rather than guessing at another layer's unit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextFieldHoverAnchor {
    /// Offset in character units from the start of the bound text.
    pub character_index: usize,
    /// Window-space pointer position, matching the coordinate space used for
    /// overlay drawing.
    pub window_x: f32,
    pub window_y: f32,
}

impl TextFieldHoverAnchor {
    pub fn new(character_index: usize, window_x: f32, window_y: f32) -> Self {
        Self {
            character_index,
            window_x,
            window_y,
        }
    }
}

/// Current caret and selection, reported whenever either moves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextFieldCaretState {
    /// Caret offset in character units; for a selection this is the moving end.
    pub caret_index: usize,
    /// Half-open `[low, high)` selection, or `None` when collapsed.
    pub selected_range: Option<(usize, usize)>,
}

impl TextFieldCaretState {
    pub fn new(caret_index: usize, selected_range: Option<(usize, usize)>) -> Self {
        Self {
            caret_index,
            selected_range,
        }
    }

    pub const INITIAL: Self = Self {
        caret_index: 0,
        selected_range: None,
    };
}
