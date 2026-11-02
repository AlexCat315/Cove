//! Translation of `Primitives/Data/Chart/Plot.swift`.
//!
//! A chart resolved against one node size: the frame the axis chrome leaves
//! behind, plus the marks drawn inside it. The session keeps press/pin/reading
//! state across recomposition.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use cove_core::scene::{
    AccessibilityActions, AccessibilityRole, AccessibilitySemantics, Animation, Color,
    FocusChainHolder, LayoutNode, Node, NodeResource, Point, PointerCaptureHolder, Rect, Size,
};

use crate::theme::Theme;
use crate::{AnyView, Layout, PrimitiveView, impl_primitive_view};

use super::axes::ChartValueAxes;
use super::axis::{ChartFrame, ChartText};
use super::cache::ChartResolutionCache;
use super::coordinates::{ChartGeometry, Mark, PieGeometry};
use super::drawing::ChartPaint;
use super::model::{
    Accessibility as AccessibilityConfig, Axis as AxisConfig, ChartAnnotation, ChartBand,
    ChartCallout, ChartCalloutRow, ChartMarker, ChartOrientation, ChartPoint, ChartRange,
    ChartSeries, ChartSlice, ChartStyle, ChartThreshold, ChartXDomain, Pie as PieConfig,
    Viewport as ViewportConfig,
};
use super::morph::ChartTransition;
use super::overlay::{
    ChartOverlayItem, ChartOverlayKind, ChartOverlayLabel, ChartOverlayLayout,
    ChartOverlayPlacement, ChartOverlaySlot,
};
use super::scale::ChartScale;
use super::viewport::ChartWindow;

#[derive(Clone, Default)]
pub struct ChartGuides {
    pub thresholds: Vec<ChartThreshold>,
    pub bands: Vec<ChartBand>,
    pub annotations: Vec<ChartAnnotation>,
    pub markers: Vec<ChartMarker>,
}

#[derive(Clone)]
pub enum ChartContent {
    Cartesian {
        series: Vec<ChartSeries>,
        guides: ChartGuides,
        x: ChartXDomain,
    },
    Pie(Vec<ChartSlice>),
}
impl ChartContent {
    pub fn count(&self) -> usize {
        match self {
            Self::Cartesian { series, .. } => series.len(),
            Self::Pie(slices) => slices.len(),
        }
    }
    pub fn title(&self, index: usize) -> String {
        match self {
            Self::Cartesian { series, .. } => series
                .get(index)
                .map(|item| item.title.clone())
                .unwrap_or_default(),
            Self::Pie(slices) => slices
                .get(index)
                .map(|slice| slice.title.clone())
                .unwrap_or_default(),
        }
    }
}

/// Selection writes coming from input; the chart owns the binding.
#[derive(Clone)]
pub struct ChartPlotInput {
    /// Interactive only when the caller supplied a selection binding.
    pub is_enabled: bool,
    pub select: Rc<dyn Fn(Option<ChartPoint>)>,
}
impl Default for ChartPlotInput {
    fn default() -> Self {
        Self {
            is_enabled: false,
            select: Rc::new(|_| {}),
        }
    }
}

#[derive(Clone)]
pub struct ChartResolution {
    pub frame: ChartFrame,
    pub cartesian: Option<ChartGeometry>,
    pub pie: Option<PieGeometry>,
}

#[derive(Clone)]
pub struct ChartPlot {
    pub content: ChartContent,
    pub visible: Vec<bool>,
    pub style: ChartStyle,
    pub axis: AxisConfig,
    pub pie: PieConfig,
    /// Formats a value for data labels and slice shares.
    pub format: Rc<dyn Fn(f32) -> String>,
    pub selection: Option<ChartPoint>,
    pub summary: (String, String),
    /// Callout for any point, so the accessibility cursor can read a value the
    /// pointer never visited.
    pub callout: Rc<dyn Fn(ChartPoint) -> Option<ChartCallout>>,
    pub accessibility: AccessibilityConfig,
    pub viewport: ViewportConfig,
    pub visible_range: Option<ChartRange>,
    pub brush: Option<cove_core::Binding<Option<ChartRange>>>,
    pub brushed_range: Option<ChartRange>,
    pub animation: Option<Animation>,
    pub input: ChartPlotInput,
    pub theme: Option<Theme>,
}

impl ChartPlot {
    pub fn new(content: ChartContent) -> Self {
        Self {
            content,
            visible: Vec::new(),
            style: ChartStyle::default(),
            axis: AxisConfig::default(),
            pie: PieConfig::default(),
            format: Rc::new(|value| format!("{value:.2}")),
            selection: None,
            summary: (String::new(), String::new()),
            callout: Rc::new(|_| None),
            accessibility: AccessibilityConfig::default(),
            viewport: ViewportConfig::default(),
            visible_range: None,
            brush: None,
            brushed_range: None,
            animation: None,
            input: ChartPlotInput::default(),
            theme: None,
        }
    }

    fn is_interactive(&self) -> bool {
        self.input.is_enabled
    }

    pub(crate) fn theme(&self, node: &Node) -> Theme {
        self.theme
            .clone()
            .unwrap_or_else(|| crate::theme::theme_for_node(node))
    }
}

impl PrimitiveView for ChartPlot {
    fn make_node(&self) -> Node {
        let node = Node::new();
        let session = ChartSession::new();
        node.set_attachment(ChartSession::KEY, session.clone());
        node.add_resource(session);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        if layout.height().unwrap_or(0.0) <= 0.0 {
            layout.set_height(Some(64.0));
        }
        layout.set_min_height(Some(24.0));
        layout.set_min_width(Some(0.0));
    }

    fn update_node(&self, node: &Node) {
        let Some(session) = node.attachment::<Rc<ChartSession>>(ChartSession::KEY) else {
            return;
        };
        // The session holds the live selection: several events can arrive
        // before the chart recomposes with the binding's new value.
        session.current.set(self.selection);
        if self.input.is_enabled {
            session.reading.set(self.selection);
        }
        node.set_is_hit_testable(self.is_interactive());
        node.set_is_focusable(self.is_interactive());
        node.set_cursor(Some(if self.input.is_enabled {
            cove_platform::SystemCursor::Crosshair
        } else {
            cove_platform::SystemCursor::Arrow
        }));
        self.publish_accessibility(node, &session);

        let theme = self.theme(node);
        // Only data and text metrics move a mark; selection changes must not
        // throw the cached geometry away.
        let signature = ChartGeometrySignature::new(self, &theme);
        if session.signature.borrow().as_ref() != Some(&signature) {
            *session.signature.borrow_mut() = Some(signature);
            session.cache.borrow_mut().invalidate();
        }
        session.navigation.borrow_mut().window = None;
        session.transition.borrow_mut().update(
            &self.content,
            self.animation.clone(),
            node,
            session.cache.clone(),
        );

        let snapshot = self.clone();
        let cache = session.cache.clone();
        let weak_node = node.downgrade();
        let identity = ChartPaintIdentity::new(self, &theme);
        node.update_draw(identity, move |list, origin: Point| {
            let Some(node) = Node::upgrade(&weak_node) else {
                return;
            };
            snapshot.draw(&node, origin, list, &cache);
        });
        self.install_input(node, &session);
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let text = ChartText::new(&self.theme(node));
        let Some(session) = node.attachment::<Rc<ChartSession>>(ChartSession::KEY) else {
            return Vec::new();
        };
        let slots = self.overlay_slots(None, &text, Some(session.cache.clone()));
        if slots.is_empty() {
            return Vec::new();
        }
        vec![AnyView::new(ChartOverlayHost {
            plot: self.clone(),
            theme: self.theme(node),
            cache: Some(session.cache.clone()),
        })]
    }
}
impl_primitive_view!(ChartPlot);

/// Absolutely positioned overlay plane that hosts every label.
pub(crate) struct ChartOverlayHost {
    pub plot: ChartPlot,
    pub theme: Theme,
    pub cache: Option<Rc<RefCell<ChartResolutionCache>>>,
}
impl PrimitiveView for ChartOverlayHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, _node: &Node) {}
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_position_type(cove_core::scene::PositionType::Absolute);
        layout.set_position(0.0, cove_core::scene::Edge::Left);
        layout.set_position(0.0, cove_core::scene::Edge::Top);
        layout.set_width_percent(100.0);
        layout.set_height_percent(100.0);
        Some(layout)
    }
    fn update_layout(&self, layout: &LayoutNode) {
        let algorithm = ChartOverlayLayout {
            plot: self.plot.clone(),
            theme: self.theme.clone(),
            cache: self.cache.clone(),
        };
        let measure = algorithm.clone();
        let place = algorithm;
        layout.set_custom_layout(
            Rc::new(move |width, height, children| {
                let subviews: Vec<crate::LayoutSubview> = children
                    .iter()
                    .map(crate::LayoutSubview::from_layout_node)
                    .collect();
                measure.size_that_fits(crate::ProposedViewSize::new(width, height), &subviews)
            }),
            Rc::new(move |bounds, children| {
                let subviews: Vec<crate::LayoutSubview> = children
                    .iter()
                    .map(crate::LayoutSubview::from_layout_node)
                    .collect();
                place.place_subviews(
                    bounds,
                    crate::ProposedViewSize::new(Some(bounds.width), Some(bounds.height)),
                    &subviews,
                );
            }),
        );
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let text = ChartText::new(&self.theme);
        self.plot
            .overlay_slots(
                Some(Size {
                    width: 0.0,
                    height: 0.0,
                }),
                &text,
                self.cache.clone(),
            )
            .into_iter()
            .map(|slot| AnyView::new(ChartOverlayLabel { item: slot.item }))
            .collect()
    }
}
impl_primitive_view!(ChartOverlayHost);

impl ChartPlot {
    /// Every overlay label for one size, back to front.
    pub fn overlay_slots(
        &self,
        size: Option<Size>,
        text: &ChartText,
        cache: Option<Rc<RefCell<ChartResolutionCache>>>,
    ) -> Vec<ChartOverlaySlot> {
        let resolution = size.map(|size| self.resolve(size, Point::new(0.0, 0.0), text, cache));
        let mut slots: Vec<ChartOverlaySlot> = Vec::new();
        let mut taken: Vec<Rect> = Vec::new();
        let mut add =
            |item: ChartOverlayItem, placement: Option<ChartOverlayPlacement>, reserving: bool| {
                let Some(placement) = placement else {
                    slots.push(ChartOverlaySlot {
                        item,
                        placement: None,
                    });
                    return;
                };
                if reserving {
                    let measured = text.size(if item.title.is_empty() {
                        &item.subtitle
                    } else {
                        &item.title
                    });
                    let box_rect = placement.box_rect(measured.width, measured.height);
                    let padded = Rect::new(
                        box_rect.x - 2.0,
                        box_rect.y - 2.0,
                        box_rect.width + 4.0,
                        box_rect.height + 4.0,
                    );
                    if taken.iter().any(|other| Self::intersects(*other, padded)) {
                        slots.push(ChartOverlaySlot {
                            item,
                            placement: None,
                        });
                        return;
                    }
                    taken.push(box_rect);
                }
                slots.push(ChartOverlaySlot {
                    item,
                    placement: Some(placement),
                });
            };

        if let ChartContent::Cartesian { series, guides, .. } = &self.content {
            let geometry = resolution
                .as_ref()
                .and_then(|resolution| resolution.cartesian.as_ref());
            for band in guides.bands.iter().filter(|band| {
                !band.label.is_empty() && band.from.is_finite() && band.to.is_finite()
            }) {
                let placement = geometry.map(|geometry| {
                    let along = geometry.along_value(band.to, band.axis);
                    if geometry.orientation == ChartOrientation::Vertical {
                        ChartOverlayPlacement::new(
                            Point::new(geometry.plot.x + 4.0, along),
                            0.0,
                            0.0,
                        )
                        .gap(2.0)
                    } else {
                        ChartOverlayPlacement::new(
                            Point::new(along, geometry.plot.y + 4.0),
                            0.0,
                            0.0,
                        )
                        .gap(2.0)
                    }
                });
                add(
                    ChartOverlayItem::new(ChartOverlayKind::Band, band.label.clone()),
                    placement,
                    true,
                );
            }
            for threshold in guides
                .thresholds
                .iter()
                .filter(|threshold| !threshold.label.is_empty() && threshold.value.is_finite())
            {
                let placement = geometry.map(|geometry| {
                    let along = geometry.along_value(threshold.value, threshold.axis);
                    if geometry.orientation == ChartOrientation::Vertical {
                        ChartOverlayPlacement::new(
                            Point::new(geometry.plot.x + geometry.plot.width - 2.0, along),
                            1.0,
                            1.0,
                        )
                        .gap(2.0)
                    } else {
                        ChartOverlayPlacement::new(Point::new(along, geometry.plot.y), 0.0, 0.0)
                            .gap(2.0)
                    }
                });
                add(
                    ChartOverlayItem::new(ChartOverlayKind::Threshold, threshold.label.clone()),
                    placement,
                    true,
                );
            }
            for note in &guides.annotations {
                let Some(item) = series.get(note.series) else {
                    continue;
                };
                if !item
                    .data
                    .get(note.index)
                    .map(|datum| datum.y.is_finite())
                    .unwrap_or(false)
                {
                    continue;
                }
                let placement = geometry.and_then(|geometry| {
                    let mark = geometry.mark(ChartPoint::new(note.series, note.index))?;
                    if !mark.y.is_finite() {
                        return None;
                    }
                    self.end_placement(mark, geometry, 5.0)
                });
                add(
                    ChartOverlayItem::new(ChartOverlayKind::Annotation, note.text.clone())
                        .tint(note.color.clone()),
                    placement,
                    true,
                );
            }
            for (index, item) in series
                .iter()
                .enumerate()
                .filter(|(_, item)| item.style.shows_values)
            {
                for i in 0..item.data.len() {
                    let datum = item.data[i];
                    if !datum.y.is_finite() {
                        continue;
                    }
                    let placement = geometry.and_then(|geometry| {
                        let mark = geometry.mark(ChartPoint::new(index, i))?;
                        if !mark.y.is_finite() {
                            return None;
                        }
                        self.end_placement(mark, geometry, 3.0)
                    });
                    add(
                        ChartOverlayItem::new(ChartOverlayKind::Value, (self.format)(datum.y)),
                        placement,
                        true,
                    );
                }
            }
        }

        if let ChartContent::Pie(slices) = &self.content {
            let pie = resolution
                .as_ref()
                .and_then(|resolution| resolution.pie.as_ref());
            if let Some(center) = &self.pie.center {
                let placement = pie.and_then(|geometry| {
                    if geometry.inner_radius > 4.0 {
                        Some(ChartOverlayPlacement::new(geometry.center, 0.5, 0.5))
                    } else {
                        None
                    }
                });
                add(
                    ChartOverlayItem::new(ChartOverlayKind::Center, center.value.clone())
                        .subtitle(center.label.clone()),
                    placement,
                    false,
                );
            }
            if self.pie.shows_labels {
                let values: Vec<f32> = slices
                    .iter()
                    .enumerate()
                    .map(|(index, slice)| {
                        if self.visible.get(index).copied().unwrap_or(false)
                            && slice.value.is_finite()
                        {
                            slice.value.max(0.0)
                        } else {
                            0.0
                        }
                    })
                    .collect();
                let total: f32 = values.iter().sum();
                for (index, slice) in slices.iter().enumerate() {
                    if slice.title.is_empty() || total <= 0.0 || values[index] <= 0.0 {
                        continue;
                    }
                    let share = (values[index] / total * 100.0).round() as i32;
                    let placement = pie.and_then(|geometry| {
                        let wedge = geometry.wedges.iter().find(|wedge| wedge.index == index)?;
                        let anchor = geometry.label_anchor(wedge);
                        Some(ChartOverlayPlacement::new(
                            anchor,
                            if anchor.x >= geometry.center.x {
                                0.0
                            } else {
                                1.0
                            },
                            0.5,
                        ))
                    });
                    add(
                        ChartOverlayItem::new(ChartOverlayKind::Slice, slice.title.clone())
                            .subtitle(format!("{share}%")),
                        placement,
                        true,
                    );
                }
            }
        }

        if let Some(selection) = self.selection
            && let Some(callout) = (self.callout)(selection)
        {
            let mut placement = None;
            if let Some(geometry) = resolution
                .as_ref()
                .and_then(|resolution| resolution.cartesian.as_ref())
            {
                if let Some(mark) = geometry.mark(callout.point) {
                    placement = self.end_placement(mark, geometry, 10.0);
                }
            } else if let Some(pie) = resolution
                .as_ref()
                .and_then(|resolution| resolution.pie.as_ref())
                && let Some(wedge) = pie
                    .wedges
                    .iter()
                    .find(|wedge| wedge.index == callout.point.index)
            {
                let anchor = pie.anchor(wedge);
                placement = Some(
                    ChartOverlayPlacement::new(anchor, 0.5, 1.0)
                        .gap(10.0)
                        .flips(true),
                );
            }
            add(
                ChartOverlayItem::new(ChartOverlayKind::Callout, callout.title.clone())
                    .rows(callout.rows.clone()),
                placement,
                false,
            );
        }
        slots
    }

    fn intersects(a: Rect, b: Rect) -> bool {
        a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
    }

    /// Where a label sits at the far end of a mark.
    fn end_placement(
        &self,
        mark: &Mark,
        geometry: &ChartGeometry,
        gap: f32,
    ) -> Option<ChartOverlayPlacement> {
        let across = geometry.across_mark(mark);
        let start = geometry.orientation.across_origin(geometry.plot);
        if !across.is_finite()
            || across < start
            || across > start + geometry.orientation.breadth(geometry.plot)
        {
            return None;
        }
        match geometry.orientation {
            ChartOrientation::Vertical => {
                let below = mark.y > mark.base;
                let y = mark
                    .bar
                    .map(|bar| if below { bar.y + bar.height } else { bar.y })
                    .unwrap_or(mark.y);
                Some(
                    ChartOverlayPlacement::new(
                        Point::new(mark.x, y),
                        0.5,
                        if below { 0.0 } else { 1.0 },
                    )
                    .gap(gap)
                    .flips(true),
                )
            }
            ChartOrientation::Horizontal => {
                let negative = mark.x < mark.base;
                let x = mark
                    .bar
                    .map(|bar| if negative { bar.x } else { bar.x + bar.width })
                    .unwrap_or(mark.x);
                Some(
                    ChartOverlayPlacement::new(
                        Point::new(x, mark.y),
                        if negative { 1.0 } else { 0.0 },
                        0.5,
                    )
                    .gap(gap)
                    .flips(true),
                )
            }
        }
    }

    pub fn resolve(
        &self,
        size: Size,
        origin: Point,
        text: &ChartText,
        cache: Option<Rc<RefCell<ChartResolutionCache>>>,
    ) -> ChartResolution {
        let Some(cache) = cache else {
            return self.build_resolution(size, origin, text, &self.content);
        };
        let token = text.token.clone();
        // Sample before taking the cache borrow. `resolution` holds `&mut self`
        // for the whole call, so re-borrowing the same `RefCell` inside `build`
        // panicked with "already mutably borrowed" on the first paint of any
        // chart that reached the cached path — every chart interaction behaviour
        // was unreachable. Reading `samples` first keeps the borrow flat.
        let content = cache
            .borrow()
            .samples
            .as_ref()
            .map(|samples| samples.applying(&self.content))
            .unwrap_or_else(|| self.content.clone());
        cache.borrow_mut().resolution(size, origin, &token, || {
            self.build_resolution(size, origin, text, &content)
        })
    }

    fn build_resolution(
        &self,
        size: Size,
        origin: Point,
        text: &ChartText,
        content: &ChartContent,
    ) -> ChartResolution {
        match content {
            ChartContent::Cartesian { series, guides, x } => {
                let axes =
                    ChartValueAxes::resolve(series, &self.visible, &self.style, guides, &self.axis);
                let full_scale = if x.is_numeric() {
                    let extent: Vec<f32> = axes
                        .shown()
                        .iter()
                        .flat_map(|index| series[*index].data.iter().map(|datum| datum.x))
                        .filter(|value| value.is_finite())
                        .collect();
                    Some(ChartScale::fit_extent(
                        &extent,
                        None,
                        None,
                        self.axis.x.tick_count,
                        self.axis.x.kind,
                    ))
                } else {
                    None
                };
                let full = full_scale
                    .as_ref()
                    .map(|scale| scale.domain)
                    .unwrap_or_else(|| {
                        if axes.has_bars || axes.sample_count < 2 {
                            ChartRange::new(-0.5, axes.sample_count.max(1) as f32 - 0.5)
                        } else {
                            ChartRange::new(0.0, (axes.sample_count - 1) as f32)
                        }
                    });
                let minimum = if x.is_numeric() {
                    self.viewport.minimum_fraction
                } else {
                    self.viewport
                        .minimum_fraction
                        .max(1.0 / axes.sample_count.max(1) as f32)
                };
                let window = ChartWindow::new(
                    full,
                    self.visible_range,
                    if x.is_numeric() {
                        self.axis.x.kind
                    } else {
                        super::model::ChartScaleKind::Linear
                    },
                    minimum,
                    !x.is_numeric(),
                );
                let x_scale = full_scale.as_ref().map(|_| {
                    ChartScale::fit_extent(
                        &[],
                        Some(window.range.lower),
                        Some(window.range.upper),
                        self.axis.x.tick_count,
                        self.axis.x.kind,
                    )
                });
                let frame = ChartFrame::cartesian(
                    &axes,
                    x,
                    x_scale.as_ref(),
                    &self.style,
                    &self.axis,
                    size,
                    origin,
                    text,
                    Some(&window),
                );
                let geometry = ChartGeometry::new(
                    series,
                    axes,
                    x,
                    x_scale.as_ref(),
                    self.style.bar_spacing,
                    self.style.orientation,
                    frame.plot,
                    Some(window),
                );
                ChartResolution {
                    frame,
                    cartesian: Some(geometry),
                    pie: None,
                }
            }
            ChartContent::Pie(slices) => {
                let frame = ChartFrame::pie(&self.style, size, origin);
                let geometry = PieGeometry::new(
                    slices,
                    &self.visible,
                    self.style.inner_radius_ratio,
                    self.pie.start_angle,
                    self.pie.sorts_by_value,
                    frame.plot,
                );
                ChartResolution {
                    frame,
                    cartesian: None,
                    pie: Some(geometry),
                }
            }
        }
    }

    pub fn hit(&self, x: f32, y: f32, node: &Node) -> Option<ChartPoint> {
        let text = ChartText::new(&self.theme(node));
        let session = node.attachment::<Rc<ChartSession>>(ChartSession::KEY);
        let frame = node.frame();
        let resolution = self.resolve(
            Size {
                width: frame.width,
                height: frame.height,
            },
            node.absolute_origin(),
            &text,
            session.map(|session| session.cache.clone()),
        );
        if let Some(geometry) = resolution.cartesian {
            return geometry
                .nearest(x, y)
                .map(|mark| ChartPoint::new(mark.series, mark.index));
        }
        resolution.pie.and_then(|pie| {
            pie.wedge_at(x, y)
                .map(|wedge| ChartPoint::new(0, wedge.index))
        })
    }

    /// Visible (series, count) pairs in order; a pie is one series of slices.
    fn tracks(&self) -> Vec<(usize, usize)> {
        match &self.content {
            ChartContent::Cartesian { series, .. } => series
                .iter()
                .enumerate()
                .filter(|(index, item)| {
                    self.visible.get(*index).copied().unwrap_or(false) && !item.data.is_empty()
                })
                .map(|(index, item)| (index, item.data.len()))
                .collect(),
            ChartContent::Pie(slices) => {
                if slices.is_empty() {
                    Vec::new()
                } else {
                    vec![(0, slices.len())]
                }
            }
        }
    }

    fn is_visible_point(&self, point: ChartPoint) -> bool {
        match &self.content {
            ChartContent::Cartesian { series, .. } => series
                .get(point.series)
                .and_then(|item| item.data.get(point.index))
                .map(|datum| datum.y.is_finite())
                .unwrap_or(false),
            ChartContent::Pie(slices) => slices
                .get(point.index)
                .map(|slice| {
                    self.visible.get(point.index).copied().unwrap_or(false) && slice.value > 0.0
                })
                .unwrap_or(false),
        }
    }

    pub fn stepped_index(&self, delta: i32, from: Option<ChartPoint>) -> Option<ChartPoint> {
        let tracks = self.tracks();
        let Some(first) = tracks.first().copied() else {
            return from;
        };
        let start = from.unwrap_or(ChartPoint::new(
            first.0,
            if delta > 0 { 0 } else { first.1 },
        ));
        // A negative starting index cannot be represented; clamp to the track.
        let track = tracks
            .iter()
            .find(|(series, _)| *series == start.series)
            .copied()
            .unwrap_or(first);
        let mut next = ChartPoint::new(track.0, start.index);
        loop {
            next.index = (next.index as i64 + delta as i64)
                .clamp(0, track.1.saturating_sub(1) as i64) as usize;
            if self.is_visible_point(next) || next.index == 0 || next.index >= track.1 - 1 {
                break;
            }
        }
        if self.is_visible_point(next) {
            Some(next)
        } else {
            from
        }
    }

    pub fn stepped_series(&self, delta: i32, from: Option<ChartPoint>) -> Option<ChartPoint> {
        if !matches!(self.content, ChartContent::Cartesian { .. }) {
            return from;
        }
        let tracks = self.tracks();
        if tracks.is_empty() {
            return from;
        }
        let position = from
            .and_then(|point| {
                tracks
                    .iter()
                    .position(|(series, _)| *series == point.series)
            })
            .map(|index| index as i64)
            .unwrap_or(if delta > 0 { -1 } else { tracks.len() as i64 });
        let index = (position + delta as i64).clamp(0, tracks.len() as i64 - 1) as usize;
        let track = tracks[index];
        Some(ChartPoint::new(
            track.0,
            from.map(|point| point.index.min(track.1 - 1)).unwrap_or(0),
        ))
    }

    pub fn jumped(&self, to_end: bool, from: Option<ChartPoint>) -> Option<ChartPoint> {
        let tracks = self.tracks();
        let track = from
            .and_then(|point| tracks.iter().find(|(series, _)| *series == point.series))
            .or_else(|| tracks.first())
            .copied()?;
        Some(ChartPoint::new(
            track.0,
            if to_end { track.1 - 1 } else { 0 },
        ))
    }

    fn can_step(&self) -> bool {
        self.input.is_enabled
            || (self.accessibility.exposes_values
                && self.tracks().iter().any(|(_, count)| *count > 0))
    }

    fn publish_accessibility(&self, node: &Node, session: &ChartSession) {
        let cursor = session.reading.get();
        let steps = self.can_step();
        let mut semantics = AccessibilitySemantics::new(AccessibilityRole::Image);
        semantics.label = self.summary.0.clone();
        semantics.value = cursor
            .and_then(|point| self.readout(point))
            .unwrap_or_else(|| self.summary.1.clone());
        if self.input.is_enabled {
            semantics.help = "Use Increase and Decrease to read each value, or Left and Right to move, Up and Down between series, Escape to clear".to_string();
        } else if steps {
            semantics.help = "Use Increase and Decrease to read each value".to_string();
        }
        node.set_accessibility(Some(semantics));
        let mut actions = AccessibilityActions::default();
        if steps {
            let weak_node = node.downgrade();
            let increment = self.clone();
            actions.increment = Some(Rc::new(move || {
                if let Some(node) = Node::upgrade(&weak_node) {
                    increment.step(1, &node);
                }
            }));
            let weak_node = node.downgrade();
            let decrement = self.clone();
            actions.decrement = Some(Rc::new(move || {
                if let Some(node) = Node::upgrade(&weak_node) {
                    decrement.step(-1, &node);
                }
            }));
        }
        node.set_accessibility_actions(actions);
    }

    fn step(&self, delta: i32, node: &Node) {
        let Some(session) = node.attachment::<Rc<ChartSession>>(ChartSession::KEY) else {
            return;
        };
        let next = self.stepped_index(delta, session.reading.get());
        session.reading.set(next);
        if self.input.is_enabled && next != session.current.get() {
            session.current.set(next);
            (self.input.select)(next);
        }
        self.publish_accessibility(node, &session);
    }

    /// One accessible sentence for a point.
    pub fn readout(&self, point: ChartPoint) -> Option<String> {
        let callout = (self.callout)(point)?;
        if callout.rows.is_empty() {
            return None;
        }
        let values = callout
            .rows
            .iter()
            .map(|row| {
                if row.title.is_empty() {
                    row.value.clone()
                } else {
                    format!("{} {}", row.title, row.value)
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        let head = if callout.title.is_empty() {
            values
        } else {
            format!("{}: {}", callout.title, values)
        };
        let count = self.track_count(point);
        if count <= 1 || self.names_position_in_title(point) {
            return Some(head);
        }
        Some(format!(
            "{head}. {} {} of {count}",
            self.position_noun(),
            point.index + 1
        ))
    }

    fn names_position_in_title(&self, point: ChartPoint) -> bool {
        match &self.content {
            ChartContent::Cartesian {
                x: ChartXDomain::Categorical(labels),
                ..
            } => labels.is_empty(),
            ChartContent::Pie(slices) => slices
                .get(point.index)
                .map(|slice| slice.title.is_empty())
                .unwrap_or(false),
            _ => false,
        }
    }

    fn track_count(&self, point: ChartPoint) -> usize {
        match &self.content {
            ChartContent::Cartesian { series, .. } => series
                .get(point.series)
                .map(|item| item.data.len())
                .unwrap_or(1),
            ChartContent::Pie(slices) => slices.len(),
        }
    }

    fn position_noun(&self) -> &'static str {
        if matches!(self.content, ChartContent::Pie(_)) {
            "Slice"
        } else {
            "Point"
        }
    }

    fn draw(
        &self,
        node: &Node,
        origin: Point,
        list: &mut cove_core::scene::DrawList,
        cache: &Rc<RefCell<ChartResolutionCache>>,
    ) {
        let frame = node.frame();
        let size = Size {
            width: frame.width,
            height: frame.height,
        };
        if size.width <= 0.0 || size.height <= 0.0 {
            return;
        }
        let theme = self.theme(node);
        let text = ChartText::new(&theme);
        let resolution = self.resolve(size, origin, &text, Some(cache.clone()));
        let paint = ChartPaint {
            theme: &theme,
            style: &self.style,
            selection: self.selection,
            frame: &resolution.frame,
            text: &text,
        };
        paint.background(Rect::new(origin.x, origin.y, size.width, size.height), list);
        let marks = resolution
            .cartesian
            .as_ref()
            .filter(|geometry| geometry.plot.width > 0.0 && geometry.plot.height > 0.0);
        if let Some(geometry) = marks {
            list.push_clip_rect(geometry.plot);
        }
        if let ChartContent::Cartesian { series, guides, .. } = &self.content
            && let Some(geometry) = marks
        {
            paint.bands(&guides.bands, geometry, list);
            paint.grid(list);
            if let (Some(range), Some(window)) = (self.brushed_range, geometry.window.as_ref()) {
                paint.brush(range, window, list);
            }
            paint.thresholds(&guides.thresholds, geometry, list);
            paint.series(series, geometry, list);
            paint.markers(&guides.markers, geometry, list);
            paint.selection(series, geometry, list);
        }
        if marks.is_some() {
            list.pop_clip_rect();
        }
        if let (Some(geometry), ChartContent::Pie(slices)) =
            (resolution.pie.as_ref(), &self.content)
        {
            if self.pie.shows_labels {
                paint.pie_leaders(slices, geometry, list);
            }
            paint.pie(slices, geometry, list);
        }
        // Axis text is materialised by the overlay host (Text primitive gap).
        if let Some(focus) = FocusChainHolder::current()
            && focus
                .focused()
                .map(|focused| focused.id() == node.id())
                .unwrap_or(false)
            && focus.is_focus_visible()
        {
            let _ = list.add_rounded_rect_stroke(
                Rect::new(origin.x, origin.y, size.width, size.height),
                theme.radius.sm,
                2.0,
                theme.colors.focus_ring,
            );
        }
    }
}

/// Input state that must survive recomposition: the press and the pin.
pub struct ChartSession {
    pub is_pressed: Cell<bool>,
    /// A clicked, tapped or keyboard selection stays after the pointer leaves.
    pub is_pinned: Cell<bool>,
    pub releases_pin: Cell<bool>,
    /// Selection as of the latest event, ahead of the binding's recomposition.
    pub current: Cell<Option<ChartPoint>>,
    /// Where assistive stepping has read to.
    pub reading: Cell<Option<ChartPoint>>,
    pub signature: RefCell<Option<ChartGeometrySignature>>,
    pub cache: Rc<RefCell<ChartResolutionCache>>,
    pub navigation: RefCell<super::interaction::ChartNavigation>,
    pub transition: RefCell<ChartTransition>,
}
impl ChartSession {
    pub const KEY: &'static str = "chart.session";
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            is_pressed: Cell::new(false),
            is_pinned: Cell::new(false),
            releases_pin: Cell::new(false),
            current: Cell::new(None),
            reading: Cell::new(None),
            signature: RefCell::new(None),
            cache: Rc::new(RefCell::new(ChartResolutionCache::new())),
            navigation: RefCell::new(super::interaction::ChartNavigation::default()),
            transition: RefCell::new(ChartTransition::default()),
        })
    }
}
impl ChartSession {
    /// Claims pointer capture so a drag survives recomposition.
    pub fn press(&self, node: &Node) {
        self.is_pressed.set(true);
        if let Some(capture) = PointerCaptureHolder::current() {
            capture.acquire(node, false);
        }
    }

    pub fn release(&self, node: &Node) {
        self.is_pressed.set(false);
        if let Some(capture) = PointerCaptureHolder::current()
            && capture.target().is_some_and(|target| target == *node)
        {
            capture.release();
        }
    }
}

impl NodeResource for ChartSession {
    fn mount(&self, _node: &Node) {}
    fn unmount(&self, node: &Node) {
        node.cancel_animation("chart.samples");
        self.navigation.borrow_mut().cancel();
    }
    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}

/// Repaint identity: what is painted, and how. Geometry is split out because a
/// pointer move repaints without moving a mark.
struct ChartPaintIdentity {
    geometry: ChartGeometrySignature,
    colors: Vec<u32>,
    selection: Option<ChartPoint>,
    brush: Option<ChartRange>,
}
impl ChartPaintIdentity {
    fn new(plot: &ChartPlot, theme: &Theme) -> Self {
        let mut colors = Vec::new();
        match &plot.content {
            ChartContent::Cartesian { series, guides, .. } => {
                colors.extend(series.iter().map(|item| item.color.resolve(theme).rgba8()));
                colors.extend(
                    guides
                        .thresholds
                        .iter()
                        .map(|item| item.color.resolve(theme).rgba8()),
                );
                colors.extend(
                    guides
                        .bands
                        .iter()
                        .map(|item| item.color.resolve(theme).rgba8()),
                );
                colors.extend(
                    guides
                        .markers
                        .iter()
                        .map(|item| item.color.resolve(theme).rgba8()),
                );
            }
            ChartContent::Pie(slices) => {
                colors.extend(
                    slices
                        .iter()
                        .enumerate()
                        .map(|(index, slice)| slice.resolved_color(index).resolve(theme).rgba8()),
                );
            }
        }
        colors.push(
            plot.style
                .background
                .as_ref()
                .map(|color| color.resolve(theme).rgba8())
                .unwrap_or(Color::CLEAR.rgba8()),
        );
        colors.push(plot.style.grid_color.resolve(theme).rgba8());
        Self {
            geometry: ChartGeometrySignature::new(plot, theme),
            colors,
            selection: plot.selection,
            brush: plot.brushed_range,
        }
    }
}
impl PartialEq for ChartPaintIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.geometry == other.geometry
            && self.colors == other.colors
            && self.selection == other.selection
            && self
                .brush
                .map(|range| (range.lower.to_bits(), range.upper.to_bits()))
                == other
                    .brush
                    .map(|range| (range.lower.to_bits(), range.upper.to_bits()))
    }
}

/// Everything the resolved geometry depends on; colors and selection are
/// deliberately absent.
#[derive(Clone)]
pub struct ChartGeometrySignature {
    data: Vec<Vec<(u32, u32)>>,
    pie_values: Vec<u32>,
    marks: Vec<u8>,
    visible: Vec<bool>,
    x_domain: Option<u8>,
    range: Option<(u32, u32)>,
    style: Vec<u32>,
    axis: Vec<String>,
    pie: [u32; 3],
}
impl ChartGeometrySignature {
    fn new(plot: &ChartPlot, theme: &Theme) -> Self {
        let style = [
            plot.style.min_value.map(f32::to_bits).unwrap_or(u32::MAX),
            plot.style.max_value.map(f32::to_bits).unwrap_or(u32::MAX),
            u32::from(plot.style.shows_grid),
            plot.style.line_width.to_bits(),
            plot.style.bar_spacing.to_bits(),
            plot.style.content_inset.to_bits(),
            plot.style.inner_radius_ratio.to_bits(),
            match plot.style.stacking {
                super::model::ChartStacking::None => 0,
                super::model::ChartStacking::Stacked => 1,
                super::model::ChartStacking::Percent => 2,
            },
            u32::from(plot.style.orientation == ChartOrientation::Horizontal),
            plot.viewport.minimum_fraction.to_bits(),
        ]
        .to_vec();
        let axis = vec![
            plot.axis.y.title.clone(),
            plot.axis.x.title.clone(),
            plot.axis.y.tick_count.to_string(),
            plot.axis.x.tick_count.to_string(),
            u32::from(plot.axis.y.is_labeled).to_string(),
            u32::from(plot.axis.x.is_labeled).to_string(),
            plot.axis.secondary.title.clone(),
            plot.axis.secondary.tick_count.to_string(),
            u32::from(plot.axis.secondary.is_labeled).to_string(),
            format!("{:?}", plot.axis.y.kind),
            format!("{:?}", plot.axis.x.kind),
            format!("{:?}", plot.axis.secondary.kind),
        ];
        let mut signature = Self {
            data: Vec::new(),
            pie_values: Vec::new(),
            marks: Vec::new(),
            visible: plot.visible.clone(),
            x_domain: None,
            range: plot
                .visible_range
                .map(|range| (range.lower.to_bits(), range.upper.to_bits())),
            style,
            axis,
            pie: [
                plot.pie.start_angle.to_bits(),
                u32::from(plot.pie.sorts_by_value),
                u32::from(plot.pie.shows_labels),
            ],
        };
        match &plot.content {
            ChartContent::Cartesian { series, guides, x } => {
                signature.data = series
                    .iter()
                    .map(|item| {
                        item.data
                            .iter()
                            .map(|datum| (datum.x.to_bits(), datum.y.to_bits()))
                            .collect()
                    })
                    .collect();
                signature.marks = series
                    .iter()
                    .map(|item| match item.mark {
                        super::model::ChartMark::Line => 0,
                        super::model::ChartMark::Area => 1,
                        super::model::ChartMark::Bar => 2,
                        super::model::ChartMark::Point => 3,
                    })
                    .collect();
                signature.x_domain = Some(match x {
                    ChartXDomain::Categorical(labels) => {
                        if labels.is_empty() {
                            1
                        } else {
                            2
                        }
                    }
                    ChartXDomain::Numeric => 3,
                });
                let _ = guides;
            }
            ChartContent::Pie(slices) => {
                signature.pie_values = slices.iter().map(|slice| slice.value.to_bits()).collect();
                signature.marks = vec![4];
            }
        }
        let _ = theme;
        signature
    }
}
impl PartialEq for ChartGeometrySignature {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
            && self.pie_values == other.pie_values
            && self.marks == other.marks
            && self.visible == other.visible
            && self.x_domain == other.x_domain
            && self.range == other.range
            && self.style == other.style
            && self.axis == other.axis
            && self.pie == other.pie
    }
}

impl Clone for ChartPaintIdentity {
    fn clone(&self) -> Self {
        Self {
            geometry: self.geometry.clone(),
            colors: self.colors.clone(),
            selection: self.selection,
            brush: self.brush,
        }
    }
}

/// Re-exported so `Chart.swift` can build the standard value formatter.
pub fn default_value_text(value: f32) -> String {
    super::model::Selection::default_text(value)
}

/// Builds the row list shared by the callout and accessibility readout.
pub fn callout_rows(
    items: impl IntoIterator<Item = (String, String, Option<Color>)>,
) -> Vec<ChartCalloutRow> {
    items
        .into_iter()
        .map(|(title, value, _color)| ChartCalloutRow::new(title, value))
        .collect()
}
