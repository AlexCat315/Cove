//! Translation of Primitives/Text/Markdown/ListBlock.swift.
//!
//! Bulleted, numbered and task lists. Markers sit in a fixed-width gutter on the
//! first line of their item, so wrapped lines and nested blocks align to the
//! item text rather than to the marker.
use crate::Scope;
use std::rc::Rc;

use crate::modifiers::layout::{FlexExt, LayoutFrameExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::button::{ButtonIcon, ButtonIconSource};
use crate::primitives::text::Text;
use crate::theme::{SemanticColorRef, SemanticFontRef, theme_for_node};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View, impl_primitive_view};
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, Align, FlexDirection, Justify, Node, Wrap,
};

use super::document::{MarkdownList, MarkdownListItem};

/// Gutter width for a bulleted list.
const BULLET_GUTTER: f32 = 18.0;
/// Per-digit gutter cost for an ordered list.
const DIGIT_WIDTH: f32 = 8.0;
/// Extra room so the widest number still clears the text.
const NUMBER_PADDING: f32 = 12.0;
const TASK_MARKER: f32 = 15.0;

/// Renders one list with its markers and nested blocks.
pub struct MarkdownListView {
    pub list: MarkdownList,
    pub depth: usize,
    /// Renderer for a nested block; the caller owns block dispatch.
    pub render_block: Option<Rc<dyn Fn(&super::document::MarkdownBlock, usize) -> AnyView>>,
}

impl MarkdownListView {
    pub fn new(list: MarkdownList, depth: usize) -> Self {
        Self {
            list,
            depth,
            render_block: None,
        }
    }

    pub fn render_block(
        mut self,
        render: impl Fn(&super::document::MarkdownBlock, usize) -> AnyView + 'static,
    ) -> Self {
        self.render_block = Some(Rc::new(render));
        self
    }

    /// Numbers are right-aligned in a gutter sized for the widest one.
    fn gutter(&self) -> f32 {
        match self.list.start {
            None => BULLET_GUTTER,
            Some(start) => {
                let last = start + self.list.items.len().saturating_sub(1);
                last.to_string().len() as f32 * DIGIT_WIDTH + NUMBER_PADDING
            }
        }
    }
}

impl View for MarkdownListView {
    fn body(&self, _cx: &Scope) -> AnyView {
        let gutter = self.gutter();
        let start = self.list.start.unwrap_or(1);
        let depth = self.depth;
        let render_block = self.render_block.clone();
        let mut rows: Vec<AnyView> = Vec::new();
        for (index, item) in self.list.items.iter().enumerate() {
            rows.push(AnyView::new(MarkdownListRow {
                item: item.clone(),
                number: start + index,
                depth,
                gutter,
                render_block: render_block.clone(),
            }));
        }
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                6.0,
                AnyView::new(TupleView::new(rows)),
            )
            .frame_min_width(0.0),
        )
    }
}

/// One list item: marker gutter plus the item's own blocks.
struct MarkdownListRow {
    item: MarkdownListItem,
    number: usize,
    depth: usize,
    gutter: f32,
    render_block: Option<Rc<dyn Fn(&super::document::MarkdownBlock, usize) -> AnyView>>,
}

impl PrimitiveView for MarkdownListRow {
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
        let marker = match self.item.is_checked {
            Some(checked) => AnyView::new(MarkdownTaskMarker {
                is_checked: checked,
            }),
            None => match self.render_block {
                _ if self.number > 0 && self.gutter > BULLET_GUTTER => AnyView::new(
                    Text::new(format!("{}.", self.number))
                        .alignment(cove_core::TextAlignment::Trailing)
                        .semantic_font(SemanticFontRef::body())
                        .semantic_foreground(SemanticColorRef::on_surface_muted()),
                ),
                _ => AnyView::new(MarkdownBullet { depth: self.depth }),
            },
        };
        // The marker centers on the item's first line, not on the whole item:
        // the row top-aligns its columns, so the marker box takes the first
        // line's height and centers within it.
        let theme = theme_for_node(node);
        let scaler = crate::theme::node_text_scaler(node);
        let first_line = scaler.scale(theme.typography.resolved_body().line_height);
        let marker = AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                marker,
            )
            .frame(None, None, None, Some(first_line), None, None),
        );
        let mut blocks: Vec<AnyView> = Vec::new();
        for block in &self.item.blocks {
            blocks.push(match self.render_block.as_ref() {
                Some(render) => render(block, self.depth + 1),
                None => AnyView::new(crate::EmptyView),
            });
        }
        vec![AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::FlexStart,
            Justify::FlexStart,
            Wrap::NoWrap,
            6.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(
                    BoxView::new(
                        FlexDirection::Column,
                        Align::Stretch,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        0.0,
                        marker,
                    )
                    .frame_width(self.gutter),
                ),
                AnyView::new(
                    BoxView::new(
                        FlexDirection::Column,
                        Align::Stretch,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        6.0,
                        AnyView::new(TupleView::new(blocks)),
                    )
                    .flex_with(1.0, 1.0, Some(0.0)),
                ),
            ])),
        ))]
    }
}
impl_primitive_view!(MarkdownListRow);

/// Drawn rather than typeset: a font's bullet glyph is often too small to read
/// as a marker. Nesting alternates dot, ring and square.
struct MarkdownBullet {
    depth: usize,
}

impl PrimitiveView for MarkdownBullet {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        let layout = cove_core::scene::LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Row);
        layout.set_align_items(Align::Center);
        layout.set_justify_content(Justify::Center);
        Some(layout)
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, node: &Node) -> Vec<AnyView> {
        // The marker paints only its own small box; the gutter around it stays
        // clear. Styling the gutter node would draw a bar across the column.
        let theme = theme_for_node(node);
        let color = theme.colors.on_surface_muted;
        let dot = |size: f32| {
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(crate::EmptyView),
            )
            .frame_size(size, size)
        };
        let marker = match self.depth % 3 {
            0 => AnyView::new(dot(6.0).background_color(color).corner_radius(3.0)),
            1 => AnyView::new(dot(6.0).border_color(color, 1.5).corner_radius(3.0)),
            _ => AnyView::new(dot(5.0).background_color(color).corner_radius(1.0)),
        };
        vec![marker]
    }
}
impl_primitive_view!(MarkdownBullet);

/// A read-only task checkbox, centred on the item's first line.
struct MarkdownTaskMarker {
    is_checked: bool,
}

impl PrimitiveView for MarkdownTaskMarker {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Checkbox,
            label: if self.is_checked {
                "Completed"
            } else {
                "Not completed"
            }
            .to_string(),
            state: cove_core::scene::AccessibilityState {
                is_selected: self.is_checked,
                is_read_only: true,
                ..Default::default()
            },
            ..Default::default()
        }));
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        let layout = cove_core::scene::LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Row);
        layout.set_align_items(Align::Center);
        layout.set_justify_content(Justify::Center);
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        let theme = theme_for_node(node);
        if self.is_checked {
            node.set_background_color(Some(theme.colors.accent));
            node.set_corner_radius(4.0);
            node.set_border_width(0.0);
        } else {
            node.set_background_color(Some(cove_core::Color::CLEAR));
            node.set_corner_radius(4.0);
            node.set_border_color(Some(theme.colors.border_strong));
            node.set_border_width(1.5);
        }
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let glyph = if self.is_checked {
            AnyView::new(ButtonIcon::new(
                ButtonIconSource::resource(
                    crate::primitives::media::ui_icon_catalog::action::check(),
                ),
                Some(12.0),
                Some(cove_core::Color::WHITE),
            ))
        } else {
            AnyView::new(crate::EmptyView)
        };
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::Center,
                Wrap::NoWrap,
                0.0,
                glyph,
            )
            .frame_size(TASK_MARKER, TASK_MARKER),
        )]
    }
}
impl_primitive_view!(MarkdownTaskMarker);
