//! Translation of Primitives/Data/Lists/InsetGroup.swift.
//!
//! Inset grouped rows: an optional caption, then the rows on one rounded
//! surface, separated by hairlines that start at the row content. The shape
//! phones use for settings and detail screens; touch surfaces get taller rows
//! and rounder corners.
use crate::Scope;
use std::rc::Rc;

use crate::modifiers::layout::{LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::text::Text;
use crate::theme::{SemanticColorRef, SemanticFontRef, Theme, ThemeReader, theme_for_node};
use crate::{AnyView, BoxView, PrimitiveView, View, impl_primitive_view};
use cove_core::Color;
use cove_core::scene::{Align, FlexDirection, Justify, LayoutNode, Node, Rect, Wrap};

/// Hairline inset from the row content edges.
const ROW_INSET: f32 = 16.0;

/// Grouped rows on one rounded surface with separators between them.
pub struct ListGroup {
    pub title: Option<String>,
    pub footer: Option<String>,
    content: AnyView,
}

impl ListGroup {
    pub fn new(content: impl View + 'static) -> Self {
        Self {
            title: None,
            footer: None,
            content: AnyView::new(content),
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn footer(mut self, footer: impl Into<String>) -> Self {
        self.footer = Some(footer.into());
        self
    }
}

impl View for ListGroup {
    fn body(&self, _cx: &Scope) -> AnyView {
        let title = self.title.clone();
        let footer = self.footer.clone();
        let content = self.content.clone();
        AnyView::new(ThemeReader::new(move |theme: &Theme| {
            let mut column: Vec<AnyView> = Vec::new();
            if let Some(title) = title.clone() {
                column.push(AnyView::new(
                    Text::new(title)
                        .semantic_font(SemanticFontRef::caption())
                        .semantic_foreground(SemanticColorRef::on_surface_muted())
                        .padding_insets(crate::modifiers::layout::EdgeInsets::new(
                            0.0, ROW_INSET, 0.0, ROW_INSET,
                        )),
                ));
            }
            // The surface sits on its own node: the rows node paints the
            // separators.
            column.push(AnyView::new(
                GroupRows {
                    content: content.clone(),
                }
                .frame(None, None, Some(0.0), None, None, None)
                .background_color(theme.colors.surface_raised)
                .corner_radius(theme.radius.md),
            ));
            if let Some(footer) = footer.clone() {
                column.push(AnyView::new(
                    Text::new(footer)
                        .semantic_font(SemanticFontRef::caption())
                        .semantic_foreground(SemanticColorRef::on_surface_muted())
                        .padding_insets(crate::modifiers::layout::EdgeInsets::new(
                            0.0, ROW_INSET, 0.0, ROW_INSET,
                        )),
                ));
            }
            AnyView::new(
                BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    6.0,
                    AnyView::new(crate::TupleView::new(column)),
                )
                .frame(None, None, Some(0.0), None, None, None),
            )
        }))
    }
}

/// Stacks rows and paints the separators between them.
struct GroupRows {
    content: AnyView,
}

impl PrimitiveView for GroupRows {
    fn make_node(&self) -> Node {
        Node::new()
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
        layout.set_width_percent(100.0);
        layout.set_padding(ROW_INSET, cove_core::scene::Edge::Start);
        layout.set_padding(ROW_INSET, cove_core::scene::Edge::End);
        layout.set_min_width(Some(0.0));
    }

    fn update_node(&self, node: &Node) {
        // Separator positions come from the finished layout; keying the paint on
        // them repaints when rows move, appear or change height.
        let weak = node.downgrade();
        node.set_layout_did_update(Some(Rc::new(move |owner: &Node| {
            let Some(node) = Node::upgrade(&weak) else {
                return;
            };
            let origin = node.absolute_frame();
            let children: Vec<Node> = owner.children();
            let visible: Vec<Node> = children
                .into_iter()
                .filter(|child| child.is_layout_visible())
                .collect();
            let bottoms: Vec<u32> = visible
                .iter()
                .take(visible.len().saturating_sub(1))
                .map(|child| {
                    let frame = child.absolute_frame();
                    (frame.y + frame.height - origin.y).to_bits()
                })
                .collect();
            let draw_weak = weak.clone();
            let identity = bottoms.clone();
            node.update_overlay_draw(identity, move |list, draw_origin| {
                let Some(inner) = Node::upgrade(&draw_weak) else {
                    return;
                };
                let color = theme_for_node(&inner)
                    .colors
                    .divider
                    .multiplied_alpha(inner.opacity());
                let width = (inner.frame().width - ROW_INSET).max(0.0);
                let _ = draw_bottoms(list, draw_origin, &bottoms, ROW_INSET, width, color);
            });
        }) as cove_core::scene::LayoutUpdateHook));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(GroupRows);

fn draw_bottoms(
    list: &mut cove_core::DrawList,
    origin: cove_core::Point,
    bottoms: &[u32],
    inset: f32,
    width: f32,
    color: Color,
) -> Result<(), cove_core::DrawError> {
    for bottom in bottoms {
        let y = f32::from_bits(*bottom);
        list.add_rect(
            Rect::new(origin.x + inset, origin.y + y - 0.5, width, 0.5),
            color,
        )?;
    }
    Ok(())
}
