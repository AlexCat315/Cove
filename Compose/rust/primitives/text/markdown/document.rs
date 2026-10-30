//! Translation of Primitives/Text/Markdown/Document.swift.
//!
//! The rendered subset of a Markdown document, independent of the parser.
use crate::primitives::text::rich::run::RichTextRun;

/// Alignment of one table column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MarkdownAlignment {
    #[default]
    Leading,
    Center,
    Trailing,
}

/// One list item: a plain bullet or a GFM task item.
#[derive(Clone, Debug, PartialEq)]
pub struct MarkdownListItem {
    /// `None` for a plain item; `Some(true)`/`Some(false)` for a GFM task item.
    pub is_checked: Option<bool>,
    pub blocks: Vec<MarkdownBlock>,
}

/// A bulleted (`start: None`) or ordered list.
#[derive(Clone, Debug, PartialEq)]
pub struct MarkdownList {
    /// `None` for a bulleted list.
    pub start: Option<usize>,
    pub items: Vec<MarkdownListItem>,
}

/// A GFM table.
#[derive(Clone, Debug, PartialEq)]
pub struct MarkdownTable {
    pub alignments: Vec<MarkdownAlignment>,
    pub header: Vec<Vec<RichTextRun>>,
    pub rows: Vec<Vec<Vec<RichTextRun>>>,
}

/// The rendered subset of a Markdown document.
#[derive(Clone, Debug, PartialEq)]
pub enum MarkdownBlock {
    Paragraph(Vec<RichTextRun>),
    Heading { level: usize, runs: Vec<RichTextRun> },
    Quote(Vec<MarkdownBlock>),
    List(MarkdownList),
    Code { language: Option<String>, text: String },
    Table(MarkdownTable),
    Rule,
}

impl MarkdownBlock {
    pub fn paragraph(text: impl Into<String>) -> Self {
        Self::Paragraph(vec![RichTextRun::new(text)])
    }
}
