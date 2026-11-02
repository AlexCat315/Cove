//! Text primitives.
pub mod code;
pub mod markdown;
pub mod rich;
pub mod rope;
mod selection_input;
pub mod single_line;
pub(crate) mod text;
pub mod text_buffer;
pub mod text_completion_rebase;
pub mod text_diagnostic;
pub mod text_document_layout;
pub mod text_edit_history;
pub mod text_editing_commands;
pub mod text_field;

pub use api::*;

pub(crate) mod api {
    pub use super::markdown::MarkdownView;
    pub use super::rich::RichText;
    pub use super::rich::text_selection::TextSelectionRoot;
    pub use super::text::{Divider, DividerAxis, Text};
    pub use super::text_field::{
        FieldState, TextField, TextFieldAxis, TextFieldBehavior, TextFieldEvents, TextFieldLayout,
        TextFieldSize,
    };
    pub use cove_core::TextAlignment;
}
