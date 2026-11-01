//! Rich text: shaped flow, cross-run selection, paragraphs and link anchors.
pub mod flow;
pub mod link_anchor;
pub mod paragraph;
pub mod rich_text;
pub mod run;
pub mod text_selection;

pub use link_anchor::RichTextLinkAnchor;
pub use paragraph::{RichTextPalette, RichTextRole, RichTextSurface};
pub use rich_text::RichText;
