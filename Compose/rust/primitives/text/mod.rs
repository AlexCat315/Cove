//! Text primitives.
pub mod code;
pub mod markdown;
pub mod rope;
pub mod rich;
pub mod single_line;
pub mod text_buffer;
pub mod text_completion_rebase;
pub mod text_diagnostic;
pub mod text_document_layout;
pub mod text_edit_history;
pub mod text_editing_commands;
pub mod text_field;
pub mod text;

pub use text::{Divider, DividerAxis, Text};
pub use text_field::{FieldState, TextField, TextFieldEvents};
pub use cove_core::TextAlignment;
