//! Translation of `Primitives/Data/Chart/ChartOverlay.swift`.
//!
//! Text drawn over the plot: callout, guide labels, notes, value labels and
//! pie labels. Text placement is resolved at layout time when the plot rect is
//! known; the label primitive carries its strings for the text host (the
//! `Text` primitive has no Rust counterpart yet).
use crate::LayoutDebugExt;
use crate::modifiers::layout::{LayoutFrameExt, PaddingExt};
use crate::modifiers::style::AppearanceExt;
use crate::primitives::text::Text;
use crate::theme::tokens::{SemanticColorRef, SemanticFontRef, SemanticStyleViewExt};
use crate::theme::{Theme, ThemeMetric};
use crate::{AnyView, BoxView, Layout, LayoutSubview, ProposedViewSize, TupleView, View};
use cove_core::scene::{Align, FlexDirection, Justify, Point, Rect, Size, Wrap};

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
        Self {
            anchor,
            horizontal,
            vertical,
            gap: 0.0,
            flips: false,
        }
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
#[derive(Clone)]
pub struct ChartOverlayLayout {
    pub plot: ChartPlot,
    pub theme: Theme,
    pub cache: Option<std::rc::Rc<std::cell::RefCell<ChartResolutionCache>>>,
}
impl Layout for ChartOverlayLayout {
    fn size_that_fits(&self, proposal: ProposedViewSize, _subviews: &[LayoutSubview]) -> Size {
        Size {
            width: proposal.width.unwrap_or(0.0),
            height: proposal.height.unwrap_or(0.0),
        }
    }

    fn place_subviews(
        &self,
        bounds: Rect,
        _proposal: ProposedViewSize,
        subviews: &[LayoutSubview],
    ) {
        let text = ChartText::new(&self.theme);
        let slots = self.plot.overlay_slots(
            Some(Size {
                width: bounds.width,
                height: bounds.height,
            }),
            &text,
            self.cache.clone(),
        );
        for (index, subview) in subviews.iter().enumerate() {
            let Some(slot) = slots.get(index) else {
                subview.place(
                    Point::new(bounds.x, bounds.y),
                    ProposedViewSize::ZERO,
                    false,
                );
                continue;
            };
            let Some(placement) = slot.placement else {
                subview.place(
                    Point::new(bounds.x, bounds.y),
                    ProposedViewSize::ZERO,
                    false,
                );
                continue;
            };
            let size = subview.size_that_fits(ProposedViewSize::UNSPECIFIED);
            let mut rect = placement.box_rect(size.width, size.height);
            rect.x += bounds.x;
            rect.y += bounds.y;
            if placement.flips
                && (rect.y < bounds.y || rect.y + rect.height > bounds.y + bounds.height)
            {
                rect.y = bounds.y + placement.flipped_y(size.height);
            }
            rect.x = rect
                .x
                .max(bounds.x)
                .min(bounds.x + bounds.width - size.width);
            rect.y = rect
                .y
                .max(bounds.y)
                .min(bounds.y + bounds.height - size.height);
            subview.place(
                Point::new(rect.x, rect.y),
                ProposedViewSize::new(Some(size.width), Some(size.height)),
                true,
            );
        }
    }
}

/// The text of one overlay item. Every kind shares one structure so only the
/// styling differs: a callout reads as a card, an annotation as a tinted pill,
/// the donut center as headline text and everything else as a caption.
#[derive(Clone)]
pub struct ChartOverlayLabel {
    pub item: ChartOverlayItem,
}
impl View for ChartOverlayLabel {
    fn body(&self, _cx: &crate::Scope) -> AnyView {
        let item = &self.item;
        let label = match item.kind {
            ChartOverlayKind::Callout => callout_label(item),
            ChartOverlayKind::Annotation => annotation_label(item),
            ChartOverlayKind::Center => center_label(item),
            _ => caption_label(item),
        };
        AnyView::new(AnyView::new(label).debug_name("chart-overlay-label"))
    }

    fn dynamic_properties(&self) -> Vec<crate::DynamicField> {
        Vec::new()
    }
}

/// One line of overlay text.
fn line(text: impl Into<String>, font: SemanticFontRef, color: SemanticColorRef) -> AnyView {
    AnyView::new(
        AnyView::new(Text::new(text))
            .semantic_font(font)
            .semantic_foreground(color),
    )
}

fn column(align: Align, spacing: f32, lines: Vec<AnyView>) -> BoxView {
    BoxView::new(
        FlexDirection::Column,
        align,
        Justify::FlexStart,
        Wrap::NoWrap,
        spacing,
        AnyView::new(TupleView::new(lines)),
    )
}

/// The series swatch a callout row opens with.
fn swatch(color: SemanticColorRef) -> AnyView {
    AnyView::new(
        BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(crate::EmptyView),
        )
        .frame_size(6.0, 6.0)
        .semantic_background(color)
        .corner_radius(3.0),
    )
}

/// One row of a multi-series callout: the series swatch, its name and value.
fn callout_rows(rows: &[ChartCalloutRow]) -> AnyView {
    let mut lines: Vec<AnyView> = Vec::new();
    for row in rows {
        lines.push(AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::NoWrap,
            5.0,
            AnyView::new(TupleView::new(vec![
                swatch(row.color.clone().unwrap_or(SemanticColorRef::accent())),
                line(
                    row.title.clone(),
                    SemanticFontRef::caption(),
                    SemanticColorRef::on_surface_muted(),
                ),
                line(
                    row.value.clone(),
                    SemanticFontRef::body_strong(),
                    SemanticColorRef::on_surface(),
                ),
            ])),
        )));
    }
    AnyView::new(BoxView::new(
        FlexDirection::Column,
        Align::FlexStart,
        Justify::FlexStart,
        Wrap::NoWrap,
        2.0,
        AnyView::new(TupleView::new(lines)),
    ))
}

/// The multi-row tooltip for the selected point: a floating card.
fn callout_label(item: &ChartOverlayItem) -> AnyView {
    let mut lines = vec![line(
        item.title.clone(),
        SemanticFontRef::caption(),
        SemanticColorRef::on_surface_muted(),
    )];
    lines.push(callout_rows(&item.rows));
    AnyView::new(
        AnyView::new(column(Align::FlexStart, 2.0, lines))
            .padding_edges(8.0, 5.0)
            .semantic_background(SemanticColorRef::surface_floating())
            .corner_radius_metric(ThemeMetric::input_radius())
            .semantic_border(SemanticColorRef::border(), 1.0)
            .debug_name("chart-callout"),
    )
}

/// A note pinned to one value: a transparent pill outlined in its own tint.
fn annotation_label(item: &ChartOverlayItem) -> AnyView {
    let border = item.tint.clone().unwrap_or(SemanticColorRef::accent());
    AnyView::new(
        AnyView::new(line(
            item.title.clone(),
            SemanticFontRef::caption(),
            SemanticColorRef::on_surface(),
        ))
        .padding_edges(5.0, 2.0)
        .semantic_background(SemanticColorRef::surface_floating())
        .corner_radius(4.0)
        .semantic_border(border, 1.0)
        .debug_name("chart-annotation"),
    )
}

/// Text in the donut hole: a headline value and an optional label under it.
fn center_label(item: &ChartOverlayItem) -> AnyView {
    let mut lines = vec![line(
        item.title.clone(),
        SemanticFontRef::headline(),
        SemanticColorRef::on_surface(),
    )];
    lines.extend(subtitle_line(item));
    AnyView::new(AnyView::new(column(Align::Center, 0.0, lines)).debug_name("chart-center"))
}

/// A guide, value or slice label: caption text, centred on its anchor.
fn caption_label(item: &ChartOverlayItem) -> AnyView {
    let mut lines = vec![line(
        item.title.clone(),
        SemanticFontRef::caption(),
        SemanticColorRef::on_surface_variant(),
    )];
    lines.extend(subtitle_line(item));
    AnyView::new(AnyView::new(column(Align::Center, 0.0, lines)).debug_name("chart-label"))
}

fn subtitle_line(item: &ChartOverlayItem) -> Option<AnyView> {
    if item.subtitle.is_empty() {
        return None;
    }
    Some(line(
        item.subtitle.clone(),
        SemanticFontRef::caption(),
        SemanticColorRef::on_surface_muted(),
    ))
}
