//! Translation of Primitives/Text/Rich/RichText.swift.
//!
//! A paragraph of styled runs that wrap together: bold, italic, inline code,
//! strikethrough and links in one flow, with the surrounding text style,
//! foreground and text scaler applied once.
//!
//! ```ignore
//! RichText::new(vec![
//!     RichTextRun::new("Run "),
//!     RichTextRun::with_traits("cove build", RichTextTraits::CODE),
//!     RichTextRun::new(" or read the "),
//!     RichTextRun::new("guide").link("https://example.com/guide"),
//!     RichTextRun::new("."),
//! ])
//! ```
//!
//! Links are underlined body-copy links: they take the pointer cursor and
//! activate on click; the Markdown views build whole documents from these
//! paragraphs.
use crate::Scope;
use std::rc::Rc;

use crate::modifiers::style::{text_style_font, text_style_line_height};
use crate::primitives::text::rich::flow::{RichParagraph, RichParagraphSpan};
use crate::primitives::text::rich::run::RichTextRun;
use crate::{AnyView, PrimitiveView, View};
use cove_core::scene::Node;
use cove_core::{Font, TextAlignment};

use super::paragraph::{RichTextRole, RichTextSurface, link_anchors};

/// A paragraph of styled runs that wrap together.
pub struct RichText {
    runs: Vec<RichTextRun>,
    alignment: TextAlignment,
    on_open_link: Option<Rc<dyn Fn(&str)>>,
    role: RichTextRole,
    /// Display-only paragraphs (menu rows) neither select nor take focus, so the
    /// enclosing control keeps every click.
    is_selectable: bool,
}

impl RichText {
    pub fn new(runs: Vec<RichTextRun>) -> Self {
        Self {
            runs,
            alignment: TextAlignment::Leading,
            on_open_link: None,
            role: RichTextRole::Paragraph,
            is_selectable: true,
        }
    }

    pub fn text(text: impl Into<String>) -> Self {
        Self::new(vec![RichTextRun::new(text)])
    }

    pub fn alignment(mut self, alignment: TextAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub fn on_open_link(mut self, handler: impl Fn(&str) + 'static) -> Self {
        self.on_open_link = Some(Rc::new(handler));
        self
    }

    pub fn role(mut self, role: RichTextRole) -> Self {
        self.role = role;
        self
    }

    /// Display-only paragraphs neither select nor take focus.
    pub fn display_only(mut self, is_selectable: bool) -> Self {
        self.is_selectable = !is_selectable;
        self
    }
}

impl Default for RichText {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl View for RichText {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(RichTextReader {
            runs: self.runs.clone(),
            alignment: self.alignment,
            on_open_link: self.on_open_link.clone(),
            role: self.role.clone(),
            is_selectable: self.is_selectable,
        })
    }
}

/// Resolves the run list into a shaped paragraph against the node's text style.
struct RichTextReader {
    runs: Vec<RichTextRun>,
    alignment: TextAlignment,
    on_open_link: Option<Rc<dyn Fn(&str)>>,
    role: RichTextRole,
    is_selectable: bool,
}

impl PrimitiveView for RichTextReader {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let paragraph = self.resolve(node);
        let mut views: Vec<AnyView> = vec![AnyView::new(RichTextSurface {
            paragraph: paragraph.clone(),
            role: self.role.clone(),
            destinations: destinations_of(&self.runs),
            on_open_link: self.on_open_link.clone(),
        })];
        // Links are the keyboard targets inside text; the surface itself is not
        // a Tab stop.
        views.extend(link_anchors(
            &paragraph,
            &destinations_of(&self.runs),
            self.on_open_link.clone(),
            None,
        ));
        let _ = self.is_selectable;
        vec![AnyView::new(crate::TupleView::new(views))]
    }
}
crate::impl_primitive_view!(RichTextReader);

/// Font and line height for the paragraph, read from the node's resolved style.
fn resolve_style(node: &Node) -> (Font, f32) {
    let theme = crate::theme::theme_for_node(node);
    let body = crate::theme::SemanticFontRef::body().resolve(&theme);
    let scaler = crate::theme::node_text_scaler(node);
    let font = text_style_font(node).unwrap_or_else(|| scaler.scale_font(&body.font));
    let line_height = text_style_line_height(node)
        .unwrap_or_else(|| scaler.scale(body.line_height_for(Some(&font))));
    (font, line_height)
}

/// Distinct link destinations, in first-appearance order, matching each span's
/// `link` index.
fn destinations_of(runs: &[RichTextRun]) -> Vec<String> {
    let mut destinations: Vec<String> = Vec::new();
    for run in runs {
        if let Some(link) = run.link.as_ref()
            && !destinations.contains(link)
        {
            destinations.push(link.clone());
        }
    }
    destinations
}

impl RichTextReader {
    fn resolve(&self, node: &Node) -> RichParagraph {
        let (font, line_height) = resolve_style(node);
        let destinations = destinations_of(&self.runs);
        let spans: Vec<RichParagraphSpan> =
            self.runs
                .iter()
                .map(|run| RichParagraphSpan {
                    text: run.text.clone(),
                    font: font.clone(),
                    line_height,
                    traits: run.traits,
                    link: run.link.as_ref().and_then(|link| {
                        destinations.iter().position(|candidate| candidate == link)
                    }),
                })
                .collect();
        RichParagraph {
            spans,
            alignment: self.alignment,
            wraps: self.role != RichTextRole::CodeBlock,
        }
    }
}
