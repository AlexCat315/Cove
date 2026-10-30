//! Markdown parsing and rendering: the block subset, the parser and the views.
pub mod code_block;
pub mod document;
pub mod list_block;
pub mod markdown_view;
pub mod parse;
pub mod table_block;

pub use code_block::{DelayedReset, MarkdownCodeBlock};
pub use document::{
    MarkdownAlignment, MarkdownBlock, MarkdownList, MarkdownListItem, MarkdownTable,
};
pub use list_block::MarkdownListView;
pub use markdown_view::{markdown_block, MarkdownBlocks, MarkdownView};
pub use parse::{blocks, runs};
pub use table_block::{MarkdownTableLayout, MarkdownTableView};
