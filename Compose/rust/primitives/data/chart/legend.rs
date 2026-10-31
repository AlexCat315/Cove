//! Translation of `Primitives/Data/Chart/Legend.swift`.
//!
//! Wrapping legend whose items toggle series visibility. Items stay on one
//! line when there is room and wrap whole; a hidden item keeps its place,
//! dims, and shows a hollow swatch.
use std::collections::HashSet;
use std::rc::Rc;

use crate::{Layout, LayoutSubview, PrimitiveView, ProposedViewSize, View, impl_primitive_view};
use cove_core::Binding;
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, Direction, EventPhase, EventResult, LayoutNode,
    Node, Point, PointerPhase, Rect, Size,
};

use crate::theme::theme_for_node;
use crate::theme::tokens::SemanticColorRef;

#[derive(Clone)]
pub struct ChartLegendItem {
    pub title: String,
    pub color: SemanticColorRef,
}

pub struct ChartLegend {
    pub items: Vec<ChartLegendItem>,
    pub hidden: Binding<HashSet<String>>,
}
impl PrimitiveView for ChartLegend {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, _node: &Node) {}
    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }
    fn update_layout(&self, layout: &LayoutNode) {
        let algorithm = ChartLegendLayout {
            horizontal_spacing: 4.0,
            vertical_spacing: 2.0,
        };
        let measure = algorithm.clone();
        let place = algorithm;
        layout.set_custom_layout(
            Rc::new(move |width, height, children| {
                let subviews: Vec<LayoutSubview> = children
                    .iter()
                    .map(LayoutSubview::from_layout_node)
                    .collect();
                let result =
                    measure.size_that_fits(ProposedViewSize::new(width, height), &subviews);
                Size {
                    width: result.width.max(0.0),
                    height: result.height.max(0.0),
                }
            }),
            Rc::new(move |bounds, children| {
                let subviews: Vec<LayoutSubview> = children
                    .iter()
                    .map(LayoutSubview::from_layout_node)
                    .collect();
                place.place_subviews(
                    bounds,
                    ProposedViewSize::new(Some(bounds.width), Some(bounds.height)),
                    &subviews,
                );
            }),
        );
        layout.set_min_width(Some(0.0));
    }
    fn children(&self, _node: &Node) -> Vec<crate::AnyView> {
        self.items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                crate::AnyView::new(ChartLegendToggle {
                    item: item.clone(),
                    index,
                    is_hidden: self.hidden.get().contains(&item.title),
                    hidden: self.hidden.clone(),
                })
            })
            .collect()
    }
}
impl_primitive_view!(ChartLegend);

struct ChartLegendToggle {
    item: ChartLegendItem,
    index: usize,
    is_hidden: bool,
    hidden: Binding<HashSet<String>>,
}
impl PrimitiveView for ChartLegendToggle {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node.set_is_focusable(true);
        node
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_min_height(Some(22.0));
        layout.set_padding(6.0, cove_core::scene::Edge::Horizontal);
        Some(layout)
    }
    fn update_node(&self, node: &Node) {
        let mut semantics = AccessibilitySemantics::new(AccessibilityRole::Toggle);
        semantics.label = self.item.title.clone();
        semantics.value = if self.is_hidden {
            "Hidden".to_string()
        } else {
            "Shown".to_string()
        };
        semantics.state.is_selected = !self.is_hidden;
        node.set_accessibility(Some(semantics));
        node.set_attachment("chart.legend.title", self.item.title.clone());
        node.set_attachment("chart.legend.hidden", self.is_hidden);
        node.set_attachment("chart.legend.index", self.index);
        let theme = theme_for_node(node);
        let swatch = if self.is_hidden {
            theme.colors.background
        } else {
            self.item.color.resolve(&theme)
        };
        let stroke = if self.is_hidden {
            theme.colors.on_surface_muted
        } else {
            self.item.color.resolve(&theme)
        };
        node.update_overlay_draw(
            (swatch.rgba8(), stroke.rgba8(), self.is_hidden),
            move |list, origin| {
                let rect = Rect::new(origin.x, origin.y, 8.0, 8.0);
                let _ = list.add_rounded_rect(rect, 4.0, swatch);
                let _ = list.add_rounded_rect_stroke(rect, 4.0, 1.5, stroke);
            },
        );
        let hidden = self.hidden.clone();
        let title = self.item.title.clone();
        if let Some(registry) = cove_core::scene::InteractionRegistryHolder::current() {
            registry.set_pointer(
                node,
                cove_core::scene::InputHandlerRoute::control(),
                Rc::new(move |_event, phase, event_phase| {
                    if phase == PointerPhase::Up && event_phase == EventPhase::Target {
                        let mut next = hidden.get();
                        if !next.remove(&title) {
                            next.insert(title.clone());
                        }
                        hidden.set(next);
                        return EventResult::Handled;
                    }
                    EventResult::Ignored
                }),
            );
        }
        node.set_cursor(Some(cove_platform::SystemCursor::Pointer));
    }
}
impl_primitive_view!(ChartLegendToggle);

/// Places whole items left to right and wraps; RTL starts at the trailing edge.
#[derive(Clone, Copy)]
pub struct ChartLegendLayout {
    pub horizontal_spacing: f32,
    pub vertical_spacing: f32,
}
impl Layout for ChartLegendLayout {
    fn size_that_fits(&self, proposal: ProposedViewSize, subviews: &[LayoutSubview]) -> Size {
        if subviews.is_empty() {
            return Size {
                width: 0.0,
                height: 0.0,
            };
        }
        let sizes: Vec<Size> = subviews
            .iter()
            .map(|subview| subview.size_that_fits(ProposedViewSize::new(Some(10_000.0), None)))
            .collect();
        let Some(width) = proposal.width else {
            let content_width = sizes.iter().map(|size| size.width).sum::<f32>()
                + self.horizontal_spacing * sizes.len().saturating_sub(1) as f32;
            return Size {
                width: content_width,
                height: sizes.iter().map(|size| size.height).fold(0.0, f32::max),
            };
        };
        let (_, height) = self.rows(&sizes, width.max(0.0));
        Size {
            width: width.max(0.0),
            height,
        }
    }

    fn place_subviews(
        &self,
        bounds: Rect,
        _proposal: ProposedViewSize,
        subviews: &[LayoutSubview],
    ) {
        if subviews.is_empty() {
            return;
        }
        let sizes: Vec<Size> = subviews
            .iter()
            .map(|subview| subview.size_that_fits(ProposedViewSize::new(Some(10_000.0), None)))
            .collect();
        let (positions, _) = self.rows(&sizes, bounds.width);
        let rtl = subviews
            .first()
            .map(|subview| subview.layout_direction == Direction::Rtl)
            .unwrap_or(false);
        for (index, subview) in subviews.iter().enumerate() {
            let size = sizes[index];
            let x = if rtl {
                bounds.x + bounds.width - positions[index].0 - size.width
            } else {
                bounds.x + positions[index].0
            };
            subview.place(
                Point::new(x, bounds.y + positions[index].1),
                ProposedViewSize::new(Some(size.width), Some(size.height)),
                true,
            );
        }
    }
}
impl ChartLegendLayout {
    fn rows(&self, sizes: &[Size], width: f32) -> (Vec<(f32, f32)>, f32) {
        let mut positions = Vec::with_capacity(sizes.len());
        let mut x = 0.0;
        let mut y = 0.0;
        let mut row_height = 0.0;
        for size in sizes {
            if x > 0.0 && x + size.width > width {
                y += row_height + self.vertical_spacing;
                x = 0.0;
                row_height = 0.0;
            }
            positions.push((x, y));
            row_height = row_height.max(size.height);
            x += size.width + self.horizontal_spacing;
        }
        (positions, y + row_height)
    }
}
