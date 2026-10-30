//! Translation of `Primitives/Data/Timeline.swift`.
//!
//! Ordered list of events on a vertical rail. The rail is the content: the
//! connector line is what turns a stack of rows into a sequence, and the glyph
//! on each node is what lets the reader scan by outcome before reading a title.

use std::cell::RefCell;
use std::rc::Rc;

use cove_core::scene::{
    AccessibilityRole, Align, FlexDirection, Justify, Node, Wrap,
};
use cove_core::{Binding, Color, Point, Rect, Size};
use cove_imaging::BundleImageResource;
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{EdgeInsets, FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::button::{ActionControlAccessibility, Button, ButtonIconSource, ButtonRole};
use crate::primitives::feedback::{Badge, StatusTone};
use crate::{Layout, LayoutContainer, LayoutSubview, ProposedViewSize};
use crate::primitives::media::{ui_icon_catalog, Icon};
use crate::primitives::text::Text;
use crate::theme::styles::buttons::{ButtonStyleViewExt, GhostButtonStyle};
use crate::theme::{
    node_text_scaler, theme_for_node, IconMetrics, SemanticColorRef, SemanticFontRef,
    SemanticIconSize, Theme,
};
use crate::{AnyView, BoxView, PrimitiveView, Row, Spacer, TupleView, VerticalAlignment, View, ViewExt};

pub mod heading_layout;

use heading_layout::TimelineHeadingLayout;

/// One entry in a `Timeline`.
#[derive(Clone)]
pub struct TimelineEvent {
    pub id: String,
    pub title: String,
    pub detail: String,
    /// Trailing metadata: a timestamp, duration or actor. Moves below the title
    /// when this timeline's content width is compact or both labels cannot fit.
    pub trailing: String,
    pub tone: StatusTone,
    /// Glyph override. `None` uses the tone's glyph; set `shows_icon = false`
    /// for a plain dot.
    pub icon: Option<BundleImageResource>,
    pub shows_icon: bool,
    /// Draws a plain dot instead of a glyph, for dense or low-signal entries.
    pub is_compact: bool,
    /// When the event happened. Proportional spacing uses the gaps between
    /// consecutive dates; events without one keep uniform spacing.
    pub date: Option<f64>,
}

impl TimelineEvent {
    pub fn new(title: impl Into<String>) -> Self {
        let title = title.into();
        Self {
            id: title.clone(),
            title,
            detail: String::new(),
            trailing: String::new(),
            tone: StatusTone::Neutral,
            icon: None,
            shows_icon: true,
            is_compact: false,
            date: None,
        }
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    pub fn trailing(mut self, trailing: impl Into<String>) -> Self {
        self.trailing = trailing.into();
        self
    }

    pub fn tone(mut self, tone: StatusTone) -> Self {
        self.tone = tone;
        self
    }

    pub fn icon(mut self, icon: BundleImageResource) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn shows_icon(mut self, shows_icon: bool) -> Self {
        self.shows_icon = shows_icon;
        self
    }

    pub fn is_compact(mut self, is_compact: bool) -> Self {
        self.is_compact = is_compact;
        self
    }

    /// Seconds since the Unix epoch, matching Swift's `Date.timeIntervalSince1970`.
    pub fn date(mut self, seconds_since_epoch: f64) -> Self {
        self.date = Some(seconds_since_epoch);
        self
    }
}

/// A titled run of events that can collapse to its header.
#[derive(Clone)]
pub struct TimelineGroup {
    pub id: String,
    pub title: String,
    pub events: Vec<TimelineEvent>,
    pub is_collapsible: bool,
}

impl TimelineGroup {
    pub fn new(title: impl Into<String>, events: Vec<TimelineEvent>) -> Self {
        let title = title.into();
        Self { id: title.clone(), title, events, is_collapsible: true }
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    pub fn is_collapsible(mut self, is_collapsible: bool) -> Self {
        self.is_collapsible = is_collapsible;
        self
    }
}

/// Vertical distance between consecutive events.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TimelineSpacing {
    /// One theme gap between entries: a sequence, not a time scale.
    Uniform,
    /// Extra gap proportional to the time between dated events, clamped to
    /// `minimum..maximum` points, then multiplied by the zoom factor.
    Proportional { points_per_hour: f32, minimum: f32, maximum: f32 },
}

impl TimelineSpacing {
    pub fn proportional(points_per_hour: f32) -> Self {
        Self::Proportional { points_per_hour, minimum: 0.0, maximum: 240.0 }
    }

    fn gap(self, earlier: Option<f64>, later: Option<f64>, zoom: f32) -> f32 {
        let Self::Proportional { points_per_hour, minimum, maximum } = self else { return 0.0 };
        let (Some(earlier), Some(later)) = (earlier, later) else { return 0.0 };
        let hours = ((later - earlier).abs() / 3600.0) as f32;
        (hours * points_per_hour).clamp(minimum, maximum) * zoom
    }
}

impl Default for TimelineSpacing {
    fn default() -> Self {
        Self::Uniform
    }
}

/// Zoom range for proportional spacing. The binding is the caller's view state;
/// Timeline adds zoom buttons only when one is supplied.
#[derive(Clone)]
pub struct TimelineZoom {
    pub factor: Binding<f32>,
    pub range: (f32, f32),
}

impl TimelineZoom {
    pub fn new(factor: Binding<f32>) -> Self {
        Self { factor, range: (0.25, 8.0) }
    }

    pub fn range(mut self, lower: f32, upper: f32) -> Self {
        self.range = (lower, upper);
        self
    }

    fn clamped(&self, value: f32) -> f32 {
        let (lower, upper) = self.range;
        let value = if value.is_finite() { value } else { 1.0 };
        value.min(upper).max(lower)
    }
}

/// Ordered list of events on a vertical rail.
pub struct Timeline {
    pub groups: Vec<TimelineGroup>,
    /// Hides the connector line on the final entry, which is the default: the
    /// sequence ends where the list ends.
    pub shows_trailing_connector: bool,
    pub spacing: TimelineSpacing,
    pub zoom: Option<TimelineZoom>,
    /// Collapsed group IDs; session-local when `None`.
    pub collapsed_groups: Option<Binding<Vec<String>>>,
    local_collapsed: Rc<RefCell<Vec<String>>>,
}

impl Timeline {
    pub fn new(events: Vec<TimelineEvent>) -> Self {
        Self::groups(vec![TimelineGroup::new("", events).id("timeline.events")])
    }

    pub fn groups(groups: Vec<TimelineGroup>) -> Self {
        Self {
            groups,
            shows_trailing_connector: false,
            spacing: TimelineSpacing::Uniform,
            zoom: None,
            collapsed_groups: None,
            local_collapsed: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn configure(mut self, configure: impl FnOnce(&mut Self)) -> Self {
        configure(&mut self);
        self
    }

    /// Every event in display order, across groups.
    pub fn events(&self) -> Vec<&TimelineEvent> {
        self.groups.iter().flat_map(|group| group.events.iter()).collect()
    }

    fn is_collapsed(&self, id: &str) -> bool {
        match &self.collapsed_groups {
            Some(binding) => binding.get().iter().any(|value| value == id),
            None => self.local_collapsed.borrow().iter().any(|value| value == id),
        }
    }

    fn toggle_collapsed(&self, id: &str) {
        match &self.collapsed_groups {
            Some(binding) => {
                let mut next = binding.get();
                if let Some(index) = next.iter().position(|value| value == id) {
                    next.remove(index);
                } else {
                    next.push(id.to_string());
                }
                binding.set(next);
            }
            None => {
                let mut local = self.local_collapsed.borrow_mut();
                if let Some(index) = local.iter().position(|value| value == id) {
                    local.remove(index);
                } else {
                    local.push(id.to_string());
                }
            }
        }
    }

    fn zoom_factor(&self) -> f32 {
        self.zoom
            .as_ref()
            .map(|zoom| zoom.clamped(zoom.factor.get()))
            .unwrap_or(1.0)
    }
}

impl View for Timeline {
    fn body(&self) -> AnyView {
        let groups = self.groups.clone();
        let shows_trailing_connector = self.shows_trailing_connector;
        let spacing = self.spacing;
        let zoom = self.zoom.clone();
        let collapsed_binding = self.collapsed_groups.clone();
        let local_collapsed = self.local_collapsed.clone();
        let group_count = groups.len();

        let mut column: Vec<AnyView> = Vec::new();
        if let Some(zoom) = zoom.clone() {
            column.push(zoom_controls(zoom));
        }
        for (group_index, group) in groups.iter().enumerate() {
            let is_last_group = group_index + 1 == group_count;
            let is_collapsed = !group.title.is_empty()
                && group.is_collapsible
                && match &collapsed_binding {
                    Some(binding) => binding.get().iter().any(|value| value == &group.id),
                    None => local_collapsed.borrow().iter().any(|value| value == &group.id),
                };
            let factor = zoom
                .as_ref()
                .map(|zoom| zoom.clamped(zoom.factor.get()))
                .unwrap_or(1.0);

            let mut group_column: Vec<AnyView> = Vec::new();
            if !group.title.is_empty() {
                let toggle_id = group.id.clone();
                let binding = collapsed_binding.clone();
                let local = local_collapsed.clone();
                let toggle = Rc::new(move || match &binding {
                    Some(binding) => {
                        let mut next = binding.get();
                        if let Some(index) = next.iter().position(|value| value == &toggle_id) {
                            next.remove(index);
                        } else {
                            next.push(toggle_id.clone());
                        }
                        binding.set(next);
                    }
                    None => {
                        let mut next = local.borrow_mut();
                        if let Some(index) = next.iter().position(|value| value == &toggle_id) {
                            next.remove(index);
                        } else {
                            next.push(toggle_id.clone());
                        }
                    }
                });
                group_column.push(AnyView::new(group_header(group, is_collapsed, toggle, is_last_group)));
            }
            if !is_collapsed {
                let event_count = group.events.len();
                for (index, event) in group.events.iter().enumerate() {
                    let is_last = is_last_group && index + 1 == event_count;
                    let next_date = group.events.get(index + 1).and_then(|next| next.date);
                    let gap = if index + 1 == event_count {
                        if is_last_group { 0.0 } else { 0.0 }
                    } else {
                        0.0 + spacing.gap(event.date, next_date, factor)
                    };
                    let row = TimelineRow {
                        event: event.clone(),
                        is_last,
                        shows_trailing_connector,
                    };
                    group_column.push(AnyView::new(
                        AnyView::new(row)
                            .padding_insets(EdgeInsets::new(0.0, 0.0, gap, 0.0)),
                    ));
                }
            }
            column.push(AnyView::new(
                AnyView::new(AnyView::new(BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(TupleView::new(group_column)),
                )))
                .id(group.id.clone()),
            ));
        }

        AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(column)),
            ))
            .accessibility(|semantics| semantics.role = AccessibilityRole::List),
        )
    }
}

fn zoom_controls(zoom: TimelineZoom) -> AnyView {
    let factor = zoom.clamped(zoom.factor.get());
    let (lower, upper) = zoom.range;
    let percent = format!("{}%", (factor * 100.0).round() as i32);

    let zoom_out_binding = zoom.factor.clone();
    let zoom_out = {
        let zoom = zoom.clone();
        Button::icon(
            ButtonIconSource::resource(ui_icon_catalog::action::minus()),
            12.0,
            move || zoom_out_binding.set(zoom.clamped(factor / 2.0)),
        )
        .enabled(factor > lower)
        .tooltip("Zoom out")
        .button_style(GhostButtonStyle)
        .accessibility_label("Zoom out")
    };
    let zoom_in_binding = zoom.factor.clone();
    let zoom_in = {
        let zoom = zoom.clone();
        Button::icon(
            ButtonIconSource::resource(ui_icon_catalog::action::plus()),
            12.0,
            move || zoom_in_binding.set(zoom.clamped(factor * 2.0)),
        )
        .enabled(factor < upper)
        .tooltip("Zoom in")
        .button_style(GhostButtonStyle)
        .accessibility_label("Zoom in")
    };
    let readout = Text::new(percent.clone())
        .semantic_font(SemanticFontRef::caption())
        .semantic_foreground(SemanticColorRef::on_surface_muted())
        .accessibility(move |semantics| {
            semantics.role = AccessibilityRole::Status;
            semantics.label = "Zoom".to_string();
            semantics.value = format!("{} percent", percent);
        });

    AnyView::new(
        AnyView::new(Row::new(
            VerticalAlignment::Center,
            4.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(Spacer::new(0.0)),
                AnyView::new(zoom_out),
                AnyView::new(readout),
                AnyView::new(zoom_in),
            ])),
        ))
        .padding_insets(EdgeInsets::new(0.0, 0.0, 4.0, 0.0))
        .accessibility(|semantics| {
            semantics.role = AccessibilityRole::Toolbar;
            semantics.label = "Timeline zoom".to_string();
        }),
    )
}

fn group_header(
    group: &TimelineGroup,
    is_collapsed: bool,
    toggle: Rc<dyn Fn()>,
    _is_last_group: bool,
) -> AnyView {
    let count = group.events.len().to_string();
    let title = group.title.clone();
    let mut row: Vec<AnyView> = Vec::new();
    if group.is_collapsible {
        let chevron = if is_collapsed {
            ui_icon_catalog::navigation::chevron_right()
        } else {
            ui_icon_catalog::navigation::chevron_down()
        };
        row.push(AnyView::new(
            Icon::semantic(chevron, SemanticIconSize::small()).accessibility_hidden(true),
        ));
    }
    row.push(AnyView::new(
        Text::new(group.title.clone())
            .semantic_font(SemanticFontRef::body_strong())
            .semantic_foreground(SemanticColorRef::on_surface())
            .flex(1.0)
            .shrink(1.0)
            .frame_min_width(0.0),
    ));
    row.push(AnyView::new(
        Badge::new(count.clone()).tone(StatusTone::Neutral).accessibility_hidden(true),
    ));
    let label = AnyView::new(
        AnyView::new(Row::new(
            VerticalAlignment::Center,
            6.0,
            AnyView::new(TupleView::new(row)),
        ))
        .frame(None, None, None, Some(28.0), None, None),
    );

    if !group.is_collapsible {
        let heading = title.clone();
        let value = format!("{count} events");
        return AnyView::new(label.accessibility(move |semantics| {
            semantics.role = AccessibilityRole::Heading;
            semantics.label = heading.clone();
            semantics.value = value.clone();
            semantics.combines_children = true;
        }));
    }

    let action_label = title.clone();
    let action_value = format!("{count} events");
    AnyView::new(
        Button::with_options(
            label,
            move || toggle(),
            ButtonRole::Normal,
            true,
            false,
            false,
            None,
        )
        .button_style(GhostButtonStyle)
        .accessibility(move |semantics| {
            semantics.label = action_label.clone();
            semantics.value = action_value.clone();
            semantics.state.is_expanded = Some(!is_collapsed);
        }),
    )
}

/// One event row: rail node, connector, and the heading/body text.
struct TimelineRow {
    event: TimelineEvent,
    is_last: bool,
    shows_trailing_connector: bool,
}

impl PrimitiveView for TimelineRow {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, _node: &Node) {}
    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }
    fn children(&self, node: &Node) -> Vec<AnyView> {
        let theme = theme_for_node(node);
        let metrics = IconMetrics::new(node);
        let rail_width = metrics.size(theme.icons.medium).max(22.0);

        let mut rail: Vec<AnyView> = vec![AnyView::new(self.node(&theme, &metrics))];
        if !self.is_last || self.shows_trailing_connector {
            rail.push(AnyView::new(
                AnyView::new(BoxView::new(
                    FlexDirection::Row,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(crate::EmptyView),
                ))
                .frame_width(1.0)
                .flex_with(1.0, 1.0, Some(0.0))
                .background_color(theme.colors.border),
            ));
        }

        let mut heading: Vec<AnyView> = vec![AnyView::new(
            Text::new(self.event.title.clone())
                .semantic_font(SemanticFontRef::body_strong())
                .semantic_foreground(SemanticColorRef::on_surface()),
        )];
        if !self.event.trailing.is_empty() {
            heading.push(AnyView::new(
                Text::new(self.event.trailing.clone())
                    .semantic_font(SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
            ));
        }
        let heading = LayoutContainer::new(
            TimelineHeadingLayout::new(theme.spacing.sm),
            AnyView::new(TupleView::new(heading)),
        );

        let mut body: Vec<AnyView> = vec![AnyView::new(heading)];
        if !self.event.detail.is_empty() {
            body.push(AnyView::new(
                Text::new(self.event.detail.clone())
                    .semantic_font(SemanticFontRef::body())
                    .semantic_foreground(SemanticColorRef::on_surface_variant()),
            ));
        }

        let label = self.event.title.clone();
        let value = self.event.trailing.clone();
        let help = self.event.detail.clone();
        vec![AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Row,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                theme.spacing.sm,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(
                        AnyView::new(BoxView::new(
                            FlexDirection::Column,
                            Align::Center,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            4.0,
                            AnyView::new(TupleView::new(rail)),
                        ))
                        .frame_width(rail_width),
                    ),
                    AnyView::new(
                        AnyView::new(BoxView::new(
                            FlexDirection::Column,
                            Align::Stretch,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            2.0,
                            AnyView::new(TupleView::new(body)),
                        ))
                        .flex_with(1.0, 1.0, Some(0.0))
                        .frame_min_width(0.0),
                    ),
                ])),
            ))
            .accessibility(move |semantics| {
                semantics.role = AccessibilityRole::ListItem;
                semantics.label = label.clone();
                semantics.value = value.clone();
                semantics.help = help.clone();
            }),
        )]
    }
}

impl TimelineRow {
    fn node(&self, theme: &Theme, metrics: &IconMetrics) -> AnyView {
        if self.event.is_compact || !self.event.shows_icon {
            return AnyView::new(
                AnyView::new(BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::Center,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(crate::EmptyView),
                ))
                .frame_size(7.0, 7.0)
                .background_color(self.event.tone.color().resolve(theme))
                .corner_radius(3.5)
                .accessibility_hidden(true),
            );
        }
        let resource = self.event.icon.clone().unwrap_or_else(|| self.event.tone.icon());
        let size = metrics.size(theme.icons.medium);
        AnyView::new(
            Icon::with_semantic(resource, size, self.event.tone.color())
                .accessibility_hidden(true),
        )
    }
}

impl View for TimelineRow {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}
