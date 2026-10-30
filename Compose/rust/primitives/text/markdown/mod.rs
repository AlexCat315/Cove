//! Markdown parsing and the rendered block subset.
pub mod code_block;
pub mod document;
pub mod parse;

pub use document::{
    MarkdownAlignment, MarkdownBlock, MarkdownList, MarkdownListItem, MarkdownTable,
};
pub use code_block::{DelayedReset, MarkdownCodeBlock};
pub use parse::{blocks, runs};
