//! Translation of `Primitives/Controls/Fields/DateRange/RangePicker.swift`.
//!
//! Range picker backed by a real calendar. First click starts a range, second
//! click completes it; the earlier day becomes the start when clicked second.
//!
//! Preset resolution, maximum-length validation and day counting live in
//! [`DateRangeRules`] and [`DateRangePreset`].
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::foundation::{CalendarUnit, Date};
use cove_core::scene::{Align, FlexDirection, Justify, Node, Wrap};
use cove_core::{Binding, Color};

use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::SemanticStyleViewExt;
use crate::primitives::controls::button::{Button, ButtonIconSource, ButtonRole};
use crate::primitives::controls::calendar::{
    CalendarCells, CalendarNavigation, CalendarOptions, CalendarPanel,
};
use crate::primitives::controls::date_values::{CalendarGrid, DateAvailability};
use crate::primitives::controls::fields::date_range_rules::{DateRangePreset, DateRangeRules};
use crate::primitives::overlays::anchored::{PopoverOverflow, PopoverSurface};
use crate::primitives::overlays::popover::Popover;
use crate::primitives::text::Text;
use crate::theme::styles::buttons::{ButtonStyleViewExt, GhostButtonStyle, ToggleButtonStyle};
use crate::theme::{SemanticColorRef, SemanticFontRef, theme_for_node};
use crate::{
    AnyView, BoxView, PrimitiveView, Row, TupleView, VerticalAlignment, View, impl_primitive_view,
};

/// Trigger padding and floor.
const TRIGGER_PADDING_HORIZONTAL: f32 = 10.0;
const TRIGGER_PADDING_VERTICAL: f32 = 6.0;
const TRIGGER_MIN_HEIGHT: f32 = 34.0;
/// Month card padding and gap between months.
const MONTH_PADDING: f32 = 10.0;
const MONTH_GAP: f32 = 16.0;
/// Weekday label height.
const WEEKDAY_HEIGHT: f32 = 24.0;
/// Cells per week and rows per month.
const DAYS_PER_WEEK: usize = 7;
const WEEKS_PER_MONTH: usize = 6;

/// A closed day range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DateRange {
    pub lower: Date,
    pub upper: Date,
}

impl DateRange {
    pub fn new(lower: Date, upper: Date) -> Self {
        if lower <= upper {
            Self { lower, upper }
        } else {
            Self {
                lower: upper,
                upper: lower,
            }
        }
    }

    pub fn contains(self, day: Date) -> bool {
        day >= self.lower && day <= self.upper
    }
}

/// Trigger presentation for a range picker.
#[derive(Clone, Debug, PartialEq)]
pub struct DateRangePickerPresentation {
    pub placeholder: String,
    pub is_clearable: bool,
    pub is_enabled: bool,
    pub width: f32,
}

impl Default for DateRangePickerPresentation {
    fn default() -> Self {
        Self {
            placeholder: "Choose dates".to_string(),
            is_clearable: true,
            is_enabled: true,
            width: 260.0,
        }
    }
}

impl DateRangePickerPresentation {
    pub fn validate(&mut self) {
        self.width = self.width.max(160.0);
    }

    pub fn validated(mut self) -> Self {
        self.validate();
        self
    }
}

/// Authored range-picker options.
#[derive(Clone, Default)]
pub struct DateRangePickerOptions {
    pub calendar: CalendarOptions,
    pub rules: DateRangeRules,
    pub presentation: DateRangePickerPresentation,
}

impl DateRangePickerOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validated(mut self) -> Self {
        self.presentation.validate();
        self.calendar = self.calendar.validated();
        self.rules.validate();
        self
    }
}

/// Calendar with range highlighting and two-click range selection.
pub struct DateRangeCalendar {
    pub selection: Binding<Option<DateRange>>,
    pub options: CalendarOptions,
    pub rules: DateRangeRules,
    session: Rc<RefCell<RangeSession>>,
}

/// Transient browsing state plus the half-finished range.
#[derive(Clone, Debug)]
struct RangeSession {
    month: Date,
    focused_day: Date,
    focus_request: u32,
    pending_start: Option<Date>,
}

impl DateRangeCalendar {
    pub fn new(
        selection: Binding<Option<DateRange>>,
        options: CalendarOptions,
        rules: DateRangeRules,
    ) -> Self {
        let mut options = options.validated();
        options.presentation.cell_size = options.presentation.cell_size.max(26.0);
        let initial = selection
            .get()
            .map(|range| range.lower)
            .unwrap_or(options.today);
        let session = RangeSession {
            month: CalendarGrid::month_start(initial, &options.calendar),
            focused_day: options.calendar.start_of_day(initial),
            focus_request: 0,
            pending_start: None,
        };
        let mut rules = rules;
        rules.validate();
        Self {
            selection,
            options,
            rules,
            session: Rc::new(RefCell::new(session)),
        }
    }

    fn snapshot(&self) -> DateRangeCalendar {
        DateRangeCalendar {
            selection: self.selection.clone(),
            options: self.options.clone(),
            rules: self.rules.clone(),
            session: self.session.clone(),
        }
    }

    fn month(&self) -> Date {
        self.session.borrow().month
    }

    fn set_month(&self, month: Date) {
        self.session.borrow_mut().month = month;
    }

    fn browse(&self, months: i32) {
        let month = self.month();
        if let Some(next) = self
            .options
            .calendar
            .date_by_adding(CalendarUnit::Month, months, month)
        {
            self.set_month(next);
        }
    }

    /// Whether a day may be pressed at all.
    fn is_enabled(&self, day: Date) -> bool {
        self.options.presentation.is_enabled
            && self
                .options
                .availability
                .contains(day, &self.options.calendar)
    }

    /// Availability plus the maximum length measured from a pending start.
    fn is_selectable(&self, day: Date) -> bool {
        if !self.is_enabled(day) {
            return false;
        }
        let session = self.session.borrow();
        let Some(start) = session.pending_start else {
            return true;
        };
        self.rules.can_complete(
            start,
            self.options.calendar.start_of_day(day),
            &self.options.calendar,
        )
    }

    /// First press starts a range, second completes it.
    fn choose(&self, day: Date) {
        let calendar = &self.options.calendar;
        let value = calendar.start_of_day(day);
        let mut session = self.session.borrow_mut();
        match session.pending_start {
            Some(start) => {
                if !self.rules.can_complete(start, value, calendar) {
                    return;
                }
                let range =
                    DateRange::new(calendar.start_of_day(start), calendar.start_of_day(value));
                session.pending_start = None;
                session.focused_day = value;
                drop(session);
                self.selection.set(Some(range));
            }
            None => {
                session.pending_start = Some(value);
                session.focused_day = value;
                drop(session);
                self.selection.set(Some(DateRange::new(value, value)));
            }
        }
    }

    /// Presets resolve against start-of-day today.
    fn apply_preset(&self, preset: &DateRangePreset) {
        let calendar = &self.options.calendar;
        let today = calendar.start_of_day(self.options.today);
        let Some((lower, upper)) = preset.range(today, calendar) else {
            return;
        };
        {
            let mut session = self.session.borrow_mut();
            session.pending_start = None;
            session.focused_day = lower;
        }
        self.selection.set(Some(DateRange::new(lower, upper)));
    }

    fn preset_allowed(&self, lower: Date, upper: Date) -> bool {
        let calendar = &self.options.calendar;
        self.rules.allows((lower, upper), calendar)
            && self.options.availability.contains(lower, calendar)
            && self.options.availability.contains(upper, calendar)
    }

    fn navigate(&self, date: Date, scancode: u32) -> bool {
        if !self.options.presentation.is_enabled {
            return false;
        }
        let cells = CalendarCells::new(CalendarPanel::Days, self.options.granularity);
        let Some(navigation) = CalendarNavigation::new(scancode, cells.columns) else {
            return false;
        };
        let Some(next) = navigation.destination(
            date,
            cells,
            &self.options.calendar,
            &self.options.availability,
        ) else {
            return true;
        };
        {
            let mut session = self.session.borrow_mut();
            session.focused_day = next;
            session.focus_request = session.focus_request.saturating_add(1);
        }
        self.reveal(next);
        true
    }

    /// Browses so `day` is on screen.
    fn reveal(&self, day: Date) {
        let calendar = &self.options.calendar;
        let first = CalendarGrid::month_start(self.month(), calendar);
        let Some(end) = calendar.date_by_adding(
            CalendarUnit::Month,
            self.options.presentation.number_of_months as i32,
            first,
        ) else {
            return;
        };
        if day >= first && day < end {
            return;
        }
        let target = CalendarGrid::month_start(day, calendar);
        let next = if day < first {
            target
        } else {
            calendar
                .date_by_adding(
                    CalendarUnit::Month,
                    1 - self.options.presentation.number_of_months as i32,
                    target,
                )
                .unwrap_or(target)
        };
        self.set_month(next);
    }

    /// The day that carries the grid's Tab stop.
    fn tab_day(&self, days: &[Date], month: Date) -> Option<Date> {
        let calendar = &self.options.calendar;
        let focused = self.session.borrow().focused_day;
        let in_month: Vec<Date> = days
            .iter()
            .copied()
            .filter(|day| {
                calendar.date_interval_start(CalendarUnit::Month, *day)
                    == calendar.date_interval_start(CalendarUnit::Month, month)
            })
            .filter(|day| self.is_enabled(*day))
            .collect();
        in_month
            .iter()
            .copied()
            .find(|day| calendar.start_of_day(*day) == calendar.start_of_day(focused))
            .or_else(|| in_month.first().copied())
    }
}

impl View for DateRangeCalendar {
    fn body(&self) -> AnyView {
        AnyView::new(DateRangeCalendarBody {
            view: self.snapshot(),
        })
    }
}

impl Clone for DateRangeCalendar {
    fn clone(&self) -> Self {
        self.snapshot()
    }
}

/// The scrollable row of month cards behind a key host.
struct DateRangeCalendarBody {
    view: DateRangeCalendar,
}

impl PrimitiveView for DateRangeCalendarBody {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        node.set_accessibility(Some(cove_core::scene::AccessibilitySemantics {
            role: cove_core::scene::AccessibilityRole::Group,
            label: "Date range".to_string(),
            ..Default::default()
        }));
        let Some(registry) = cove_core::scene::InteractionRegistryHolder::current() else {
            return;
        };
        let view = self.view.snapshot();
        registry.set_key(
            node,
            cove_core::scene::InputHandlerRoute::control(),
            Rc::new(move |event, phase| {
                if phase != cove_core::scene::EventPhase::Bubble || !event.modifiers.is_empty() {
                    return cove_core::scene::EventResult::Ignored;
                }
                let Some(focused) =
                    cove_core::scene::FocusChainHolder::current().and_then(|chain| chain.focused())
                else {
                    return cove_core::scene::EventResult::Ignored;
                };
                let Some(date) = focused.attachment::<Date>("calendar.day") else {
                    return cove_core::scene::EventResult::Ignored;
                };
                if view.navigate(date, event.scancode) {
                    cove_core::scene::EventResult::Handled
                } else {
                    cove_core::scene::EventResult::Ignored
                }
            }),
        );
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let view = self.view.snapshot();
        let cell_size = self.resolved_cell_size(node);
        let mut column: Vec<AnyView> = Vec::new();
        if !view.rules.presets.is_empty() {
            column.push(AnyView::new(PresetBar { view: view.clone() }));
        }
        let months = view.options.presentation.number_of_months.max(1);
        let month = view.month();
        let cards: Vec<AnyView> = (0..months)
            .map(|index| {
                let month = view
                    .options
                    .calendar
                    .date_by_adding(CalendarUnit::Month, index as i32, month)
                    .unwrap_or(month);
                AnyView::new(MonthCard {
                    view: view.clone(),
                    month,
                    index,
                    count: months,
                    cell_size,
                })
            })
            .collect();
        column.push(AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::FlexStart,
            Justify::FlexStart,
            Wrap::Wrap,
            MONTH_GAP,
            AnyView::new(TupleView::new(cards)),
        )));
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::FlexStart,
                Justify::FlexStart,
                Wrap::NoWrap,
                10.0,
                AnyView::new(TupleView::new(column)),
            )
            .frame_min_width(self.minimum_width(cell_size, months)),
        )]
    }
}
impl_primitive_view!(DateRangeCalendarBody);

impl DateRangeCalendarBody {
    /// Touch surfaces raise the cell to the minimum control extent.
    fn resolved_cell_size(&self, node: &Node) -> f32 {
        let environment = node.composition_value(&crate::adaptive::SurfaceEnvironment::key());
        let floor = if environment
            .input
            .contains(cove_core::InputCapabilities::TOUCH)
        {
            environment.minimum_control_extent()
        } else {
            0.0
        };
        self.view.options.presentation.cell_size.max(floor)
    }

    fn minimum_width(&self, cell_size: f32, months: usize) -> f32 {
        months as f32 * (cell_size * DAYS_PER_WEEK as f32 + 32.0)
    }
}

/// The preset shortcut row.
struct PresetBar {
    view: DateRangeCalendar,
}

impl PrimitiveView for PresetBar {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        node.set_accessibility(Some(cove_core::scene::AccessibilitySemantics {
            role: cove_core::scene::AccessibilityRole::Group,
            label: "Presets".to_string(),
            ..Default::default()
        }));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let view = &self.view;
        let calendar = &view.options.calendar;
        let today = calendar.start_of_day(view.options.today);
        let selected = view.selection.get();
        let pending = view.session.borrow().pending_start;

        view.rules
            .presets
            .iter()
            .map(|preset| {
                let range = preset.range(today, calendar);
                let allowed = range.is_some_and(|(lower, upper)| view.preset_allowed(lower, upper));
                let is_current = range.is_some()
                    && pending.is_none()
                    && selected == range.map(|(lower, upper)| DateRange::new(lower, upper));
                let target = view.snapshot();
                let preset = preset.clone();
                AnyView::new(
                    Button::with_options(
                        AnyView::new(
                            Text::new(preset.title.clone()).semantic_font(SemanticFontRef::label()),
                        ),
                        move || target.apply_preset(&preset),
                        ButtonRole::Normal,
                        view.options.presentation.is_enabled && allowed,
                        false,
                        is_current,
                        None,
                    )
                    .button_style(ToggleButtonStyle::new(0.0, 24.0)),
                )
            })
            .collect()
    }
}
impl_primitive_view!(PresetBar);

/// One month page: header, weekday row, six week rows.
struct MonthCard {
    view: DateRangeCalendar,
    month: Date,
    index: usize,
    count: usize,
    cell_size: f32,
}

impl PrimitiveView for MonthCard {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        let theme = theme_for_node(node);
        node.set_background_color(Some(theme.colors.surface));
        node.set_corner_radius(theme.radius.md);
        node.set_border_color(Some(theme.colors.border));
        node.set_border_width(1.0);
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let view = &self.view;
        let calendar = &view.options.calendar;
        let days = CalendarGrid::days_in_month(self.month, calendar);
        let tab_day = view.tab_day(&days, self.month);
        let enabled = view.options.presentation.is_enabled;

        let mut column: Vec<AnyView> = vec![AnyView::new(Row::new(
            VerticalAlignment::Center,
            4.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(
                    Button::icon_with_options(
                        ButtonIconSource::resource(
                            crate::primitives::media::ui_icon_catalog::navigation::chevron_left(),
                        ),
                        Some(12.0),
                        None,
                        ButtonRole::Normal,
                        enabled && self.index == 0,
                        false,
                        false,
                        Some("Previous month".to_string()),
                        {
                            let view = view.snapshot();
                            move || view.browse(-1)
                        },
                    )
                    .button_style(GhostButtonStyle),
                ),
                AnyView::new(
                    Text::new(month_title(self.month, calendar))
                        .semantic_font(SemanticFontRef::body_strong())
                        .flex(1.0)
                        .padding_edges(4.0, 0.0),
                ),
                AnyView::new(
                    Button::icon_with_options(
                        ButtonIconSource::resource(
                            crate::primitives::media::ui_icon_catalog::navigation::chevron_right(),
                        ),
                        Some(12.0),
                        None,
                        ButtonRole::Normal,
                        enabled && self.index + 1 == self.count,
                        false,
                        false,
                        Some("Next month".to_string()),
                        {
                            let view = view.snapshot();
                            move || view.browse(1)
                        },
                    )
                    .button_style(GhostButtonStyle),
                ),
            ])),
        ))];

        let mut weekday_row: Vec<AnyView> = Vec::new();
        for weekday in 0..DAYS_PER_WEEK {
            weekday_row.push(AnyView::new(
                Text::new(weekday_symbol(
                    (weekday as u32 + calendar.first_weekday() - 1) % 7,
                ))
                .semantic_font(SemanticFontRef::caption())
                .semantic_foreground(SemanticColorRef::on_surface_muted())
                .frame_size(self.cell_size, WEEKDAY_HEIGHT)
                .accessibility_hidden(true),
            ));
        }
        column.push(AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::NoWrap,
            2.0,
            AnyView::new(TupleView::new(weekday_row)),
        )));

        for week in 0..WEEKS_PER_MONTH {
            let cells: Vec<AnyView> = days
                .iter()
                .skip(week * DAYS_PER_WEEK)
                .take(DAYS_PER_WEEK)
                .map(|day| {
                    AnyView::new(DayCell {
                        view: view.snapshot(),
                        day: *day,
                        month: self.month,
                        tab_day,
                        cell_size: self.cell_size,
                    })
                })
                .collect();
            column.push(AnyView::new(BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                2.0,
                AnyView::new(TupleView::new(cells)),
            )));
        }

        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                6.0,
                AnyView::new(TupleView::new(column)),
            )
            .padding(MONTH_PADDING)
            .frame_min_width(self.cell_size * DAYS_PER_WEEK as f32 + 32.0),
        )]
    }
}
impl_primitive_view!(MonthCard);

fn month_title(date: Date, calendar: &crate::foundation::Calendar) -> String {
    const NAMES: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let parts = calendar.parts(date);
    let name = NAMES
        .get(parts.month.saturating_sub(1) as usize)
        .copied()
        .unwrap_or("January");
    format!("{name} {}", parts.year)
}

fn weekday_symbol(index: u32) -> &'static str {
    match index {
        0 => "S",
        1 => "M",
        2 => "T",
        3 => "W",
        4 => "T",
        5 => "F",
        _ => "S",
    }
}

/// One day, with range fill and start/end selection.
struct DayCell {
    view: DateRangeCalendar,
    day: Date,
    month: Date,
    tab_day: Option<Date>,
    cell_size: f32,
}

impl DayCell {
    fn is_start(&self) -> bool {
        self.view.selection.get().is_some_and(|range| {
            self.view.options.calendar.start_of_day(range.lower)
                == self.view.options.calendar.start_of_day(self.day)
        })
    }

    fn is_end(&self) -> bool {
        self.view.selection.get().is_some_and(|range| {
            self.view.options.calendar.start_of_day(range.upper)
                == self.view.options.calendar.start_of_day(self.day)
        })
    }

    fn in_range(&self) -> bool {
        self.view
            .selection
            .get()
            .is_some_and(|range| range.contains(self.day))
    }

    fn in_month(&self) -> bool {
        self.view
            .options
            .calendar
            .date_interval_start(CalendarUnit::Month, self.day)
            == self
                .view
                .options
                .calendar
                .date_interval_start(CalendarUnit::Month, self.month)
    }
}

impl PrimitiveView for DayCell {
    fn make_node(&self) -> Node {
        let enabled = self.in_month() && self.view.is_selectable(self.day);
        let selected = self.is_start() || self.is_end();
        let node = Node::new();
        node.set_attachment("calendar.day", self.day);
        node.set_is_focusable(enabled);
        node.set_is_tab_stop(self.tab_day == Some(self.day) && enabled);
        node.set_is_hit_testable(enabled);
        node.set_accessibility(Some(cove_core::scene::AccessibilitySemantics {
            role: cove_core::scene::AccessibilityRole::Button,
            label: full_date_label(self.day, &self.view.options.calendar),
            state: cove_core::scene::AccessibilityState {
                is_enabled: enabled,
                is_selected: selected,
                ..Default::default()
            },
            ..Default::default()
        }));
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        let layout = cove_core::scene::LayoutNode::new();
        layout.set_width(Some(self.cell_size));
        layout.set_height(Some(self.cell_size));
        layout.set_flex_shrink(0.0);
        Some(layout)
    }

    fn update_layout(&self, layout: &cove_core::scene::LayoutNode) {
        layout.set_width(Some(self.cell_size));
        layout.set_height(Some(self.cell_size));
        layout.set_flex_shrink(0.0);
    }

    fn update_node(&self, node: &Node) {
        let theme = theme_for_node(node);
        let in_month = self.in_month();
        let is_today = self.view.options.calendar.start_of_day(self.day)
            == self
                .view
                .options
                .calendar
                .start_of_day(self.view.options.today);
        node.set_corner_radius(theme.radius.sm);
        node.set_opacity(if in_month { 1.0 } else { 0.35 });
        node.set_background_color(if self.in_range() {
            Some(theme.colors.state_layer_selected)
        } else {
            None
        });
        node.set_border_color(Some(if is_today {
            theme.colors.accent
        } else {
            Color::CLEAR
        }));
        node.set_border_width(if is_today { 1.0 } else { 0.0 });

        let Some(registry) = cove_core::scene::InteractionRegistryHolder::current() else {
            return;
        };
        if !self.in_month() || !self.view.is_selectable(self.day) {
            registry.remove(node);
            return;
        }
        let view = self.view.snapshot();
        let day = self.day;
        registry.set_pointer(
            node,
            cove_core::scene::InputHandlerRoute::control(),
            Rc::new(move |event, phase, event_phase| {
                if event.button != cove_platform::MouseButton::Left
                    || phase != cove_core::scene::PointerPhase::Up
                    || event_phase != cove_core::scene::EventPhase::Target
                {
                    return cove_core::scene::EventResult::Ignored;
                }
                view.choose(day);
                cove_core::scene::EventResult::Handled
            }),
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let selected = self.is_start() || self.is_end();
        let tint = if selected {
            SemanticColorRef::on_accent()
        } else if self.in_month() {
            SemanticColorRef::on_surface()
        } else {
            SemanticColorRef::on_surface_muted()
        };
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::Center,
                Wrap::NoWrap,
                0.0,
                AnyView::new(
                    Text::new(self.view.options.calendar.parts(self.day).day.to_string())
                        .semantic_font(SemanticFontRef::label())
                        .semantic_foreground(tint),
                ),
            )
            .frame_size(self.cell_size, self.cell_size),
        )]
    }
}
impl_primitive_view!(DayCell);

fn full_date_label(day: Date, calendar: &crate::foundation::Calendar) -> String {
    const NAMES: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let parts = calendar.parts(day);
    let month = NAMES
        .get(parts.month.saturating_sub(1) as usize)
        .copied()
        .unwrap_or("January");
    format!("{month} {}, {}", parts.day, parts.year)
}

/// A trigger that opens the range calendar.
pub struct DateRangePicker {
    pub selection: Binding<Option<DateRange>>,
    pub options: DateRangePickerOptions,
    is_presented: Arc<Mutex<bool>>,
}

impl DateRangePicker {
    pub fn new(selection: Binding<Option<DateRange>>, options: DateRangePickerOptions) -> Self {
        Self {
            selection,
            options: options.validated(),
            is_presented: Arc::new(Mutex::new(false)),
        }
    }

    fn snapshot(&self) -> DateRangePicker {
        DateRangePicker {
            selection: self.selection.clone(),
            options: self.options.clone(),
            is_presented: self.is_presented.clone(),
        }
    }

    /// `Lower – Upper` for the selection, or the placeholder.
    pub fn label(&self) -> String {
        match self.selection.get() {
            Some(range) => format!(
                "{} – {}",
                medium_date(range.lower, &self.options.calendar),
                medium_date(range.upper, &self.options.calendar)
            ),
            None => self.options.presentation.placeholder.clone(),
        }
    }
}

fn medium_date(date: Date, options: &CalendarOptions) -> String {
    let parts = options.calendar.parts(date);
    format!(
        "{} {}, {}",
        month_title(date, &options.calendar)
            .split(' ')
            .next()
            .unwrap_or("Jan"),
        parts.day,
        parts.year
    )
}

impl View for DateRangePicker {
    fn body(&self) -> AnyView {
        let label = self.label();
        let has_selection = self.selection.get().is_some();

        let mut row: Vec<AnyView> = vec![AnyView::new(
            Text::new(label.clone())
                .truncated()
                .semantic_font(SemanticFontRef::label())
                .flex_with(1.0, 1.0, Some(0.0))
                .frame_min_width(0.0),
        )];
        if self.options.presentation.is_clearable && has_selection {
            let selection = self.selection.clone();
            row.push(AnyView::new(
                Button::icon_with_options(
                    ButtonIconSource::resource(
                        crate::primitives::media::ui_icon_catalog::action::x(),
                    ),
                    Some(12.0),
                    None,
                    ButtonRole::Normal,
                    self.options.presentation.is_enabled,
                    false,
                    false,
                    Some("Clear dates".to_string()),
                    move || selection.set(None),
                )
                .button_style(GhostButtonStyle),
            ));
        }
        row.push(AnyView::new(
            crate::primitives::controls::button::ButtonIcon::new(
                ButtonIconSource::resource(
                    crate::primitives::media::ui_icon_catalog::navigation::dropdown_chevron_down(),
                ),
                Some(12.0),
                None,
            ),
        ));

        let trigger = AnyView::new(
            Row::new(
                VerticalAlignment::Center,
                8.0,
                AnyView::new(TupleView::new(row)),
            )
            .padding_edges(TRIGGER_PADDING_HORIZONTAL, TRIGGER_PADDING_VERTICAL)
            .frame_height(TRIGGER_MIN_HEIGHT)
            .semantic_background(SemanticColorRef::surface())
            .corner_radius_metric(crate::theme::ThemeMetric::input_radius())
            .semantic_border(SemanticColorRef::border(), 1.0),
        );

        let presented = Binding::new(
            {
                let state = self.is_presented.clone();
                move || {
                    *state
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                }
            },
            {
                let state = self.is_presented.clone();
                move |value: bool| {
                    *state
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner()) = value
                }
            },
        );

        let calendar = DateRangeCalendar::new(
            self.selection.clone(),
            self.options.calendar.clone(),
            self.options.rules.clone(),
        );
        let popover = Popover::new(trigger, AnyView::new(calendar))
            .is_presented(presented)
            .enabled(self.options.presentation.is_enabled)
            .width(self.options.presentation.width)
            .surface(PopoverSurface::Content)
            .overflow(PopoverOverflow::Flip)
            .trigger_description(
                crate::primitives::overlays::popover::PopoverTriggerDescription {
                    label: self.options.presentation.placeholder.clone(),
                    value: label,
                    help: None,
                },
            );

        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(popover),
            )
            .frame_width(self.options.presentation.width),
        )
    }
}

impl Clone for DateRangePicker {
    fn clone(&self) -> Self {
        self.snapshot()
    }
}

impl DateRangePicker {
    /// Whether the popover is open, for hosts mirroring the state.
    pub fn is_presented(&self) -> bool {
        *self
            .is_presented
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Availability the calendar applies, exposed for hosts that pre-filter days.
    pub fn availability(&self) -> &DateAvailability {
        &self.options.calendar.availability
    }
}
