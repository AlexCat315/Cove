//! Rich text: shaped flow, cross-run selection, paragraphs and link anchors.
mod adapter;
pub(crate) mod display;
pub mod flow;
pub mod link_anchor;
pub mod paragraph;
pub mod run;
pub mod text_selection;

pub use link_anchor::RichTextLinkAnchor;
pub use paragraph::{RichTextPalette, RichTextSurface, TextRole};
