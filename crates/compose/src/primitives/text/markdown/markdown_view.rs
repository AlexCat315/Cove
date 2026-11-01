//! Translation of Primitives/Text/Markdown/MarkdownView.swift.
//!
//! Renders CommonMark with the GitHub extensions (tables, strikethrough, task
//! lists): headings, paragraphs with inline styles and links, quotes, nested
//! lists, code blocks that scroll sideways instead of wrapping, tables and
//! rules. It takes the surrounding text style and width and grows to its
//! content; place it in a scroll view for long documents.
//!
//! Links are reported to `on_open_link`; nothing is opened on the caller's
//! behalf. Images render as their linked alternative text.
use std::rc::Rc;

use crate::modifiers::layout::PaddingExt;
use crate::modifiers::style::SemanticStyleViewExt;
use crate::primitives::text::rich::run::RichTextRun;
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View};
use cove_core::TextAlignment;
use cove_core::scene::{Align, FlexDirection, Justify, Node, Wrap};

use super::code_block::MarkdownCodeBlock;
use super::document::{MarkdownBlock, MarkdownList};
use super::list_block::MarkdownListView;
use super::table_block::MarkdownTableView;

/// A rendered Markdown document.
pub struct MarkdownView {
    pub source: String,
    pub on_open_link: Option<Rc<dyn Fn(&str)>>,
}

impl MarkdownView {
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            on_open_link: None,
        }
    }

    pub fn on_open_link(mut self, handler: impl Fn(&str) + 'static) -> Self {
        self.on_open_link = Some(Rc::new(handler));
        self
    }
}

impl View for MarkdownView {
    fn body(&self) -> AnyView {
        AnyView::new(MarkdownBlocks {
            blocks: super::parse::blocks(&self.source),
            depth: 0,
            on_open_link: self.on_open_link.clone(),
        })
    }
}

/// A vertical run of blocks with document rhythm: headings get room above them,
/// everything else is evenly spaced.
pub struct MarkdownBlocks {
    pub blocks: Vec<MarkdownBlock>,
    /// List nesting, which picks the bullet glyph.
    pub depth: usize,
    pub on_open_link: Option<Rc<dyn Fn(&str)>>,
}

impl PrimitiveView for MarkdownBlocks {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let mut views: Vec<AnyView> = Vec::new();
        for (index, block) in self.blocks.iter().enumerate() {
            let is_heading = matches!(block, MarkdownBlock::Heading { .. });
            let view = markdown_block(block, self.depth, self.on_open_link.clone());
            if index > 0 && is_heading {
                views.push(AnyView::new(view.padding_insets(
                    crate::modifiers::layout::EdgeInsets::new(8.0, 0.0, 0.0, 0.0),
                )));
            } else {
                views.push(view);
            }
        }
        vec![AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            12.0,
            AnyView::new(TupleView::new(views)),
        ))]
    }
}
crate::impl_primitive_view!(MarkdownBlocks);

/// Dispatch one block to its renderer.
pub fn markdown_block(
    block: &MarkdownBlock,
    depth: usize,
    on_open_link: Option<Rc<dyn Fn(&str)>>,
) -> AnyView {
    match block {
        MarkdownBlock::Paragraph(runs) => AnyView::new(ParagraphText { runs: runs.clone() }),
        MarkdownBlock::Heading { level, runs } => AnyView::new(MarkdownHeading {
            level: *level,
            runs: runs.clone(),
        }),
        MarkdownBlock::Quote(blocks) => AnyView::new(MarkdownQuote {
            blocks: blocks.clone(),
            depth,
            on_open_link,
        }),
        MarkdownBlock::List(list) => AnyView::new(markdown_list_view(list, depth, on_open_link)),
        MarkdownBlock::Code { language, text } => {
            AnyView::new(MarkdownCodeBlock::new(language.clone(), text.clone()))
        }
        MarkdownBlock::Table(table) => AnyView::new(match on_open_link.clone() {
            Some(handler) => {
                MarkdownTableView::new(table.clone()).on_open_link(move |link| handler(link))
            }
            None => MarkdownTableView::new(table.clone()),
        }),
        MarkdownBlock::Rule => AnyView::new(
            crate::primitives::text::Divider::new(
                None,
                1.0,
                crate::primitives::text::DividerAxis::Horizontal,
            )
            .padding_edges(0.0, 4.0),
        ),
    }
}

fn markdown_list_view(
    list: &MarkdownList,
    depth: usize,
    on_open_link: Option<Rc<dyn Fn(&str)>>,
) -> MarkdownListView {
    let view = MarkdownListView::new(list.clone(), depth);
    match on_open_link {
        Some(handler) => view.render_block(move |block, nested_depth| {
            markdown_block(block, nested_depth, Some(handler.clone()))
        }),
        None => view.render_block(|block, nested_depth| markdown_block(block, nested_depth, None)),
    }
}

/// A plain paragraph: its runs flattened to text with the inline traits that
/// affect the whole paragraph preserved.
struct ParagraphText {
    runs: Vec<RichTextRun>,
}

impl PrimitiveView for ParagraphText {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        // Real runs, so inline bold, code and links survive.
        vec![AnyView::new(
            crate::primitives::text::rich::RichText::new(self.runs.clone())
                .alignment(TextAlignment::Leading),
        )]
    }
}
crate::impl_primitive_view!(ParagraphText);

/// A heading: its type token widens with level, and the top two levels rule off
/// their section.
struct MarkdownHeading {
    level: usize,
    runs: Vec<RichTextRun>,
}

impl PrimitiveView for MarkdownHeading {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.set_accessibility(Some(cove_core::scene::AccessibilitySemantics {
            role: cove_core::scene::AccessibilityRole::Heading,
            label: self.runs.iter().map(|run| run.text.as_str()).collect(),
            ..Default::default()
        }));
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let level = self.level;
        let mut column: Vec<AnyView> = vec![AnyView::new(
            crate::primitives::text::rich::RichText::new(self.runs.clone())
                .role(crate::primitives::text::rich::RichTextRole::Heading(level)),
        )];
        // The two top levels rule off their section, as in most readers.
        if self.level <= 2 {
            column.push(AnyView::new(crate::primitives::text::Divider::new(
                None,
                1.0,
                crate::primitives::text::DividerAxis::Horizontal,
            )));
        }
        vec![AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            8.0,
            AnyView::new(TupleView::new(column)),
        ))]
    }
}
crate::impl_primitive_view!(MarkdownHeading);

/// Type token for a heading level, derived from the theme's scale.
pub fn heading_token(level: usize, theme: &crate::theme::Theme) -> crate::theme::TextStyleToken {
    let typography = &theme.typography;
    match level {
        1 => typography.title.clone(),
        2 => {
            let size =
                ((typography.title.font.size() + typography.headline.font.size()) / 2.0).round();
            crate::theme::TextStyleToken::new(
                typography
                    .headline
                    .font
                    .with_weight(cove_core::FontWeight::Bold)
                    .resized(size),
                (size * 1.3).round(),
                typography.headline.letter_spacing,
            )
        }
        3 => typography.headline.clone(),
        _ => {
            let size = if level == 4 {
                typography.body.font.size() + 1.0
            } else {
                typography.body.font.size()
            };
            crate::theme::TextStyleToken::new(
                typography.body_strong.font.resized(size),
                typography.body.line_height + 2.0,
                typography.body.letter_spacing,
            )
        }
    }
}

trait ResizedFont {
    fn resized(&self, size: f32) -> cove_core::Font;
    fn with_weight(&self, weight: cove_core::FontWeight) -> cove_core::Font;
}

impl ResizedFont for cove_core::Font {
    fn resized(&self, size: f32) -> cove_core::Font {
        cove_core::Font::new(
            size,
            self.weight(),
            self.design(),
            self.family().map(str::to_string),
            self.is_italic(),
        )
    }

    fn with_weight(&self, weight: cove_core::FontWeight) -> cove_core::Font {
        cove_core::Font::new(
            self.size(),
            weight,
            self.design(),
            self.family().map(str::to_string),
            self.is_italic(),
        )
    }
}

/// A block quote: a strong rule in the leading gutter and muted content.
struct MarkdownQuote {
    blocks: Vec<MarkdownBlock>,
    depth: usize,
    on_open_link: Option<Rc<dyn Fn(&str)>>,
}

impl PrimitiveView for MarkdownQuote {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let rule = AnyView::new(QuoteRule);
        let content = AnyView::new(MarkdownBlocks {
            blocks: self.blocks.clone(),
            depth: self.depth,
            on_open_link: self.on_open_link.clone(),
        });
        vec![AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            12.0,
            AnyView::new(TupleView::new(vec![
                rule,
                AnyView::new(
                    content.semantic_foreground(crate::theme::SemanticColorRef::on_surface_muted()),
                ),
            ])),
        ))]
    }
}
crate::impl_primitive_view!(MarkdownQuote);

/// The quote's leading rule.
struct QuoteRule;

impl PrimitiveView for QuoteRule {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        let layout = cove_core::scene::LayoutNode::new();
        layout.set_width(Some(3.0));
        Some(layout)
    }

    fn update_layout(&self, layout: &cove_core::scene::LayoutNode) {
        layout.set_width(Some(3.0));
    }

    fn update_node(&self, node: &Node) {
        node.set_background_color(Some(
            crate::theme::theme_for_node(node).colors.border_strong,
        ));
        node.set_corner_radius(1.5);
    }
}
crate::impl_primitive_view!(QuoteRule);
