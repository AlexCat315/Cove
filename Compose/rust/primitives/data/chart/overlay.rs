//! Translation of `Primitives/Data/Chart/ChartOverlay.swift`.
//!
//! Text drawn over the plot: callout, guide labels, notes, value labels and
//! pie labels. Text placement is resolved at layout time when the plot rect is
//! known; the label primitive carries its strings for the text host (the
//! `Text` primitive has no Rust counterpart yet).
use cove_core::scene::{Align, FlexDirection, LayoutNode, Node, Point, Rect, Size};
use crate::{impl_primitive_view, Layout, LayoutSubview, PrimitiveView, ProposedViewSize};

use crate::theme::tokens::SemanticColorRef;
use crate::theme::{theme_for_node, Theme};

use super::axis::ChartText;
use super::cache::ChartResolutionCache;
use super::model::ChartCalloutRow;
use super::plot::ChartPlot;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChartOverlayKind {
    Callout,
    Annotation,
    Threshold,
    Band,
    Value,
    Slice,
    Center,
}

#[derive(Clone)]
pub struct ChartOverlayItem {
    pub kind: ChartOverlayKind,
    pub title: String,
    pub rows: Vec<ChartCalloutRow>,
    pub tint: Option<SemanticColorRef>,
    pub subtitle: String,
}
impl ChartOverlayItem {
    pub fn new(kind: ChartOverlayKind, title: impl Into<String>) -> Self {
        Self {
            kind,
            title: title.into(),
            rows: Vec::new(),
            tint: None,
            subtitle: String::new(),
        }
    }
    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = subtitle.into();
        self
    }
    pub fn rows(mut self, rows: Vec<ChartCalloutRow>) -> Self {
        self.rows = rows;
        self
    }
    pub fn tint(mut self, tint: SemanticColorRef) -> Self {
        self.tint = Some(tint);
        self
    }
    /// A callout reads as a card; everything else is caption text.
    pub fn is_card(&self) -> bool {
        self.kind == ChartOverlayKind::Callout
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChartOverlayPlacement {
    pub anchor: Point,
    pub horizontal: f32,
    pub vertical: f32,
    /// Space kept from the anchor.
    pub gap: f32,
    /// Moves to the other side of the anchor when there is no room.
    pub flips: bool,
}
impl ChartOverlayPlacement {
    pub fn new(anchor: Point, horizontal: f32, vertical: f32) -> Self {
        Self { anchor, horizontal, vertical, gap: 0.0, flips: false }
    }
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }
    pub fn flips(mut self, flips: bool) -> Self {
        self.flips = flips;
        self
    }

    /// The rect the item occupies at a measured size, before bounds clamping.
    pub fn box_rect(&self, width: f32, height: f32) -> Rect {
        Rect::new(
            self.anchor.x - width * self.horizontal,
            self.origin_y(height),
            width,
            height,
        )
    }

    /// The same box mirrored about the anchor.
    pub fn flipped_y(&self, height: f32) -> f32 {
        if self.vertical >= 1.0 {
            self.anchor.y + self.gap
        } else {
            self.anchor.y - self.gap - height
        }
    }

    fn origin_y(&self, height: f32) -> f32 {
        if self.vertical >= 1.0 {
            self.anchor.y - self.gap - height
        } else if self.vertical <= 0.0 {
            self.anchor.y + self.gap
        } else {
            self.anchor.y - height / 2.0
        }
    }
}

#[derive(Clone)]
pub struct ChartOverlaySlot {
    pub item: ChartOverlayItem,
    /// `None` means dropped: it would overlap one already placed.
    pub placement: Option<ChartOverlayPlacement>,
}

/// Places every overlay item against its anchor, keeps it inside the chart,
/// flips it when it does not fit and hides dropped labels.
pub struct ChartOverlayLayout {
    pub plot: ChartPlot,
    pub theme: Theme,
    pub cache: Option<std::rc::Rc<std::cell::RefCell<ChartResolutionCache>>>,
}
impl Layout for ChartOverlayLayout {
    fn size_that_fits(&self, proposal: ProposedViewSize, _subviews: &[LayoutSubview]) -> Size {
        Size { width: proposal.width.unwrap_or(0.0), height: proposal.height.unwrap_or(0.0) }
    }

    fn place_subviews(&self, bounds: Rect, _proposal: ProposedViewSize, subviews: &[LayoutSubview]) {
        let text = ChartText::new(&self.theme);
        let slots = self.plot.overlay_slots(
            Some(Size { width: bounds.width, height: bounds.height }),
            &text,
            self.cache.clone(),
        );
        for (index, subview) in subviews.iter().enumerate() {
            let Some(slot) = slots.get(index) else {
                subview.place(Point::new(bounds.x, bounds.y), ProposedViewSize::ZERO, false);
                continue;
            };
            let Some(placement) = slot.placement else {
                subview.place(Point::new(bounds.x, bounds.y), ProposedViewSize::ZERO, false);
                continue;
            };
            let size = subview.size_that_fits(ProposedViewSize::UNSPECIFIED);
            let mut rect = placement.box_rect(size.width, size.height);
            rect.x += bounds.x;
            rect.y += bounds.y;
            if placement.flips && (rect.y < bounds.y || rect.y + rect.height > bounds.y + bounds.height) {
                rect.y = bounds.y + placement.flipped_y(size.height);
            }
            rect.x = rect.x.max(bounds.x).min(bounds.x + bounds.width - size.width);
            rect.y = rect.y.max(bounds.y).min(bounds.y + bounds.height - size.height);
            subview.place(
                Point::new(rect.x, rect.y),
                ProposedViewSize::new(Some(size.width), Some(size.height)),
                true,
            );
        }
    }
}

/// The text of one overlay item. Every kind shares one structure so only the
/// styling differs; strings are exposed through attachments until `Text` lands.
#[derive(Clone)]
pub struct ChartOverlayLabel {
    pub item: ChartOverlayItem,
}
impl PrimitiveView for ChartOverlayLabel {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(if self.item.kind == ChartOverlayKind::Callout {
            Align::FlexStart
        } else {
            Align::Center
        });
        layout.set_gap(2.0, cove_core::scene::Gutter::Row);
        if self.item.is_card() {
            layout.set_padding(8.0, cove_core::scene::Edge::Horizontal);
            layout.set_padding(5.0, cove_core::scene::Edge::Vertical);
        } else if self.item.kind == ChartOverlayKind::Annotation {
            layout.set_padding(5.0, cove_core::scene::Edge::Horizontal);
            layout.set_padding(2.0, cove_core::scene::Edge::Vertical);
        }
        Some(layout)
    }
    fn update_node(&self, node: &Node) {
        node.set_attachment("chart.label.title", self.item.title.clone());
        node.set_attachment("chart.label.subtitle", self.item.subtitle.clone());
        node.set_attachment(
            "chart.label.rows",
            self.item.rows.iter().map(|row| (row.title.clone(), row.value.clone())).collect::<Vec<_>>(),
        );
        let theme = theme_for_node(node);
        match self.item.kind {
            ChartOverlayKind::Callout => {
                node.set_background_color(Some(theme.colors.surface_floating));
                node.set_corner_radius(theme.radius.md);
                node.set_border_color(Some(theme.colors.border));
                node.set_border_width(1.0);
            }
            ChartOverlayKind::Annotation => {
                node.set_background_color(Some(theme.colors.surface_floating));
                node.set_corner_radius(4.0);
                node.set_border_color(Some(
                    self.item
                        .tint
                        .as_ref()
                        .map(|tint| tint.resolve(&theme))
                        .unwrap_or(theme.colors.accent),
                ));
                node.set_border_width(1.0);
            }
            _ => {}
        }
    }
}
impl_primitive_view!(ChartOverlayLabel);
