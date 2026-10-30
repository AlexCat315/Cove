//! Translation of Primitives/Text/Markdown/Parse.swift.
//!
//! Swift delegates to swift-markdown (cmark-gfm) and converts its AST. Rust uses
//! `pulldown-cmark`, the same CommonMark/GFM dialect, and converts its event
//! stream into the same rendered subset. The parser stays an implementation
//! detail: `pulldown_cmark` types never cross this module.
use std::cell::RefCell;
use std::collections::HashMap;

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::primitives::text::rich::run::{RichTextRun, RichTextTraits};

use super::document::{
    MarkdownAlignment, MarkdownBlock, MarkdownList, MarkdownListItem, MarkdownTable,
};

/// Documents cached by source, so recomposition re-reads the same text.
thread_local! {
    static CACHE: RefCell<HashMap<String, Vec<MarkdownBlock>>> = RefCell::new(HashMap::new());
}

const CACHE_LIMIT: usize = 64;

/// CommonMark with the GFM tables, strikethrough and task-list extensions —
/// the same set the Swift conversion reads.
fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS
}

/// Parse a document into the rendered block subset.
pub fn blocks(source: &str) -> Vec<MarkdownBlock> {
    if let Some(hit) = CACHE.with(|cache| cache.borrow().get(source).cloned()) {
        return hit;
    }
    let parsed = convert(Parser::new_ext(source, options()));
    CACHE.with(|cache| {
        let mut table = cache.borrow_mut();
        if table.len() >= CACHE_LIMIT {
            table.clear();
        }
        table.insert(source.to_string(), parsed.clone());
    });
    parsed
}

/// Inline runs for one span of text, parsed as an inline-only document.
pub fn runs(source: &str) -> Vec<RichTextRun> {
    let mut inline = InlineState::default();
    for event in Parser::new_ext(source, options()) {
        inline.event(event);
    }
    inline.finish()
}

// MARK: - Block conversion

fn heading_level(level: HeadingLevel) -> usize {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// A block frame that collects children until its end tag.
enum Frame {
    Root,
    Quote(Vec<MarkdownBlock>),
    List(ListFrame),
    Item(ListItemFrame),
    Table(TableFrame),
    Heading { level: usize, inline: InlineState },
}

struct ListFrame {
    start: Option<usize>,
    items: Vec<MarkdownListItem>,
    next: usize,
}

struct ListItemFrame {
    is_checked: Option<bool>,
    blocks: Vec<MarkdownBlock>,
}

struct TableFrame {
    alignments: Vec<MarkdownAlignment>,
    header: Vec<Vec<RichTextRun>>,
    rows: Vec<Vec<Vec<RichTextRun>>>,
    row: Vec<Vec<RichTextRun>>,
    inline: InlineState,
    in_head: bool,
}

#[derive(Default)]
struct InlineState {
    runs: Vec<RichTextRun>,
    traits: RichTextTraits,
    link: Option<String>,
    /// One entry per open emphasis/strong/strike/link span.
    stack: Vec<(RichTextTraits, Option<String>)>,
    literal: String,
}

impl InlineState {
    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Text(text) => self.push(&text),
            Event::Code(code) => self.push_traits(&code, RichTextTraits::CODE),
            Event::SoftBreak => self.push(" "),
            Event::HardBreak => {
                self.flush();
                self.runs.push(RichTextRun {
                    text: "\n".to_string(),
                    traits: self.traits,
                    link: self.link.clone(),
                });
            }
            Event::Rule => {}
            Event::Html(html) => self.push_traits(&html, RichTextTraits::CODE),
            Event::InlineHtml(html) => self.push_traits(&html, RichTextTraits::CODE),
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Emphasis => self.open(RichTextTraits::ITALIC),
            Tag::Strong => self.open(RichTextTraits::BOLD),
            Tag::Strikethrough => self.open(RichTextTraits::STRIKETHROUGH),
            Tag::Link { dest_url, .. } => {
                self.flush();
                self.stack.push((self.traits, self.link.clone()));
                self.link = Some(dest_url.to_string());
            }
            Tag::Image { dest_url, .. } => {
                // Images render as their alternative text, linked to the source.
                self.flush();
                self.stack.push((self.traits.union(RichTextTraits::ITALIC), Some(dest_url.to_string())));
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough | TagEnd::Image => self.close(),
            TagEnd::Link => self.close(),
            _ => {}
        }
    }

    fn open(&mut self, trait_bit: RichTextTraits) {
        self.flush();
        self.stack.push((self.traits, self.link.clone()));
        self.traits = self.traits.union(trait_bit);
    }

    fn close(&mut self) {
        self.flush();
        if let Some((traits, link)) = self.stack.pop() {
            self.traits = traits;
            self.link = link;
        }
    }

    fn push(&mut self, text: &str) {
        self.literal.push_str(text);
    }

    fn push_traits(&mut self, text: &str, trait_bit: RichTextTraits) {
        self.flush();
        if text.is_empty() {
            return;
        }
        self.runs.push(RichTextRun {
            text: text.to_string(),
            traits: self.traits.union(trait_bit),
            link: self.link.clone(),
        });
    }

    fn flush(&mut self) {
        if self.literal.is_empty() {
            return;
        }
        let text = std::mem::take(&mut self.literal);
        self.runs.push(RichTextRun { text, traits: self.traits, link: self.link.clone() });
    }

    fn finish(mut self) -> Vec<RichTextRun> {
        self.flush();
        // Adjacent runs with the same style become one, so text shapes in fewer
        // pieces.
        let mut merged: Vec<RichTextRun> = Vec::new();
        for run in self.runs {
            if run.text.is_empty() {
                continue;
            }
            match merged.last_mut() {
                Some(last) if last.traits == run.traits && last.link == run.link => {
                    last.text.push_str(&run.text);
                }
                _ => merged.push(run),
            }
        }
        merged
    }
}

fn convert<'a>(events: impl Iterator<Item = Event<'a>>) -> Vec<MarkdownBlock> {
    let mut stack: Vec<Frame> = vec![Frame::Root];
    let mut blocks: Vec<MarkdownBlock> = Vec::new();
    let mut inline = InlineState::default();
    let mut code: Option<(Option<String>, String)> = None;

    fn emit(stack: &mut [Frame], blocks: &mut Vec<MarkdownBlock>, block: MarkdownBlock) {
        match stack.last_mut() {
            Some(Frame::Quote(list)) => list.push(block),
            Some(Frame::Item(item)) => item.blocks.push(block),
            _ => blocks.push(block),
        }
    }

    for event in events {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => inline = InlineState::default(),
                Tag::Heading { level, .. } => {
                    stack.push(Frame::Heading { level: heading_level(level), inline: InlineState::default() });
                }
                Tag::BlockQuote(_) => stack.push(Frame::Quote(Vec::new())),
                Tag::List(start) => stack.push(Frame::List(ListFrame {
                    start: start.map(|value| value as usize),
                    items: Vec::new(),
                    next: start.map_or(1, |value| value as usize),
                })),
                Tag::Item => {
                    let is_checked = match stack.last() {
                        Some(Frame::List(list)) => Some(list.next),
                        _ => None,
                    };
                    let _ = is_checked;
                    stack.push(Frame::Item(ListItemFrame { is_checked: None, blocks: Vec::new() }));
                }
                Tag::Table(alignments) => stack.push(Frame::Table(TableFrame {
                    alignments: alignments
                        .into_iter()
                        .map(|alignment| match alignment {
                            pulldown_cmark::Alignment::Center => MarkdownAlignment::Center,
                            pulldown_cmark::Alignment::Right => MarkdownAlignment::Trailing,
                            _ => MarkdownAlignment::Leading,
                        })
                        .collect(),
                    header: Vec::new(),
                    rows: Vec::new(),
                    row: Vec::new(),
                    inline: InlineState::default(),
                    in_head: false,
                })),
                Tag::TableHead => {
                    if let Some(Frame::Table(table)) = stack.last_mut() {
                        table.in_head = true;
                        table.inline = InlineState::default();
                    }
                }
                Tag::TableRow => {
                    if let Some(Frame::Table(table)) = stack.last_mut() {
                        table.row.clear();
                    }
                }
                Tag::TableCell => {
                    if let Some(Frame::Table(table)) = stack.last_mut() {
                        table.inline = InlineState::default();
                    }
                }
                Tag::CodeBlock(kind) => {
                    let language = match &kind {
                        CodeBlockKind::Fenced(info) => info
                            .split_whitespace()
                            .next()
                            .map(|value| value.to_string())
                            .filter(|value| !value.is_empty()),
                        CodeBlockKind::Indented => None,
                    };
                    code = Some((language, String::new()));
                }
                Tag::FootnoteDefinition(_) => {}
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Paragraph => {
                    let runs = std::mem::take(&mut inline).finish();
                    if !runs.is_empty() {
                        emit(&mut stack, &mut blocks, MarkdownBlock::Paragraph(runs));
                    }
                }
                TagEnd::Heading(_) => {
                    if let Some(Frame::Heading { level, inline }) = stack.pop() {
                        let runs = inline.finish();
                        emit(&mut stack, &mut blocks, MarkdownBlock::Heading { level, runs });
                    }
                }
                TagEnd::BlockQuote(_) => {
                    if let Some(Frame::Quote(children)) = stack.pop() {
                        emit(&mut stack, &mut blocks, MarkdownBlock::Quote(children));
                    }
                }
                TagEnd::Item => {
                    if let Some(Frame::Item(item)) = stack.pop() {
                        if let Some(Frame::List(list)) = stack.last_mut() {
                            list.items.push(MarkdownListItem {
                                is_checked: item.is_checked,
                                blocks: item.blocks,
                            });
                            list.next += 1;
                        }
                    }
                }
                TagEnd::List(_) => {
                    if let Some(Frame::List(list)) = stack.pop() {
                        emit(
                            &mut stack,
                            &mut blocks,
                            MarkdownBlock::List(MarkdownList { start: list.start, items: list.items }),
                        );
                    }
                }
                TagEnd::Table => {
                    if let Some(Frame::Table(table)) = stack.pop() {
                        emit(
                            &mut stack,
                            &mut blocks,
                            MarkdownBlock::Table(MarkdownTable {
                                alignments: table.alignments,
                                header: table.header,
                                rows: table.rows,
                            }),
                        );
                    }
                }
                TagEnd::TableHead => {
                    if let Some(Frame::Table(table)) = stack.last_mut() {
                        table.in_head = false;
                    }
                }
                TagEnd::TableRow => {
                    if let Some(Frame::Table(table)) = stack.last_mut() {
                        let row = std::mem::take(&mut table.row);
                        table.rows.push(row);
                    }
                }
                TagEnd::TableCell => {
                    if let Some(Frame::Table(table)) = stack.last_mut() {
                        let runs = std::mem::take(&mut table.inline).finish();
                        if table.in_head {
                            table.header.push(runs);
                        } else {
                            table.row.push(runs);
                        }
                    }
                }
                TagEnd::CodeBlock => {
                    if let Some((language, text)) = code.take() {
                        let text = text.strip_suffix('\n').map(str::to_string).unwrap_or(text);
                        emit(&mut stack, &mut blocks, MarkdownBlock::Code { language, text });
                    }
                }
                _ => {}
            },
            Event::Text(text) => {
                if let Some((_, body)) = code.as_mut() {
                    body.push_str(&text);
                } else if let Some(Frame::Heading { inline, .. }) = stack.last_mut() {
                    inline.push(&text);
                } else if let Some(Frame::Table(table)) = stack.last_mut() {
                    table.inline.push(&text);
                } else {
                    inline.push(&text);
                }
            }
            Event::Code(text) => {
                if let Some(Frame::Heading { inline, .. }) = stack.last_mut() {
                    inline.push_traits(&text, RichTextTraits::CODE);
                } else if let Some(Frame::Table(table)) = stack.last_mut() {
                    table.inline.push_traits(&text, RichTextTraits::CODE);
                } else {
                    inline.push_traits(&text, RichTextTraits::CODE);
                }
            }
            Event::SoftBreak => inline.push(" "),
            Event::HardBreak => inline.event(Event::HardBreak),
            Event::TaskListMarker(checked) => {
                if let Some(Frame::Item(item)) = stack.last_mut() {
                    item.is_checked = Some(checked);
                }
            }
            Event::Html(html) => {
                let trimmed = html.trim_matches('\n');
                if !trimmed.is_empty() {
                    emit(
                        &mut stack,
                        &mut blocks,
                        MarkdownBlock::Code { language: Some("html".to_string()), text: trimmed.to_string() },
                    );
                }
            }
            Event::InlineHtml(html) => inline.push_traits(&html, RichTextTraits::CODE),
            Event::Rule => emit(&mut stack, &mut blocks, MarkdownBlock::Rule),
            Event::FootnoteReference(name) => inline.push(&name),
            Event::InlineMath(math) | Event::DisplayMath(math) => {
                inline.push_traits(&math, RichTextTraits::CODE)
            }
            Event::Start(_) | Event::End(_) => {}
        }
    }
    blocks
}
