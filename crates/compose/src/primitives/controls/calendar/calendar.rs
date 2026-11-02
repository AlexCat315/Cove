//! Translation of Primitives/Controls/Calendar/Calendar.swift.
//!
//! Caller-owned selection, with transient browsing and one focusable cell per
//! grid. The title drills up from days to a year of months to twelve years;
//! picking a cell drills back down to the selection granularity.
use crate::Scope;
use cove_core::Binding;
use cove_core::Signal;
use std::rc::Rc;

use crate::foundation::{Calendar as FoundationCalendar, CalendarUnit, Date};
use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::button::{Button, ButtonIconSource, ButtonRole};
use crate::primitives::layout::{ScrollAxis, ScrollView, ScrollbarGutter};
use crate::primitives::text::Text;
use crate::theme::styles::buttons::{ButtonStyleViewExt, GhostButtonStyle};
use crate::theme::{SemanticColorRef, SemanticFontRef, theme_for_node};
use crate::{AnyView, BoxView, PrimitiveView, Row, TupleView, VerticalAlignment, View};
use cove_composition::adaptive::SurfaceEnvironment;
use cove_core::scene::{AccessibilityRole, AccessibilitySemantics};
use cove_core::scene::{Align, EventResult, FlexDirection, Justify, Node, Wrap};

use super::super::date_values::DateAvailability;
use super::navigation::{CalendarCells, CalendarNavigation};
use super::period::{
    CalendarGranularity, CalendarPanel, granularity_start, interval_start, year_page,
};

/// Width reserved for the ISO week number column.
pub const WEEK_NUMBER_WIDTH: f32 = 24.0;

/// Cells per week.
const DAYS_PER_WEEK: usize = 7;
/// Rows in a month grid; six always fits a 31-day month.
const WEEKS_PER_MONTH: usize = 6;

/// Presentation values for a calendar.
#[derive(Clone, Debug, PartialEq)]
pub struct CalendarPresentation {
    pub cell_size: f32,
    pub number_of_months: usize,
    pub is_enabled: bool,
}

impl Default for CalendarPresentation {
    fn default() -> Self {
        Self {
            cell_size: 34.0,
            number_of_months: 1,
            is_enabled: true,
        }
    }
}

impl CalendarPresentation {
    pub fn validate(&mut self) {
        self.cell_size = if self.cell_size.is_finite() {
            self.cell_size.max(26.0)
        } else {
            34.0
        };
        self.number_of_months = self.number_of_months.clamp(1, 12);
    }

    pub fn validated(mut self) -> Self {
        self.validate();
        self
    }
}

/// Authored calendar options.
#[derive(Clone)]
pub struct CalendarOptions {
    pub calendar: FoundationCalendar,
    pub availability: DateAvailability,
    pub presentation: CalendarPresentation,
    /// Whether a selection is a day, a week, a month or a year.
    pub granularity: CalendarGranularity,
    pub today: Date,
}

impl Default for CalendarOptions {
    fn default() -> Self {
        Self {
            calendar: FoundationCalendar::new(),
            availability: DateAvailability::default(),
            presentation: CalendarPresentation::default(),
            granularity: CalendarGranularity::Day,
            today: Date::from_seconds(0),
        }
    }
}

impl CalendarOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validated(mut self) -> Self {
        self.presentation.validate();
        self
    }
}

/// Transient browsing state: which month is shown, which cell holds the tab stop,
/// and the current panel.
#[derive(Clone, Debug)]
struct CalendarSession {
    month: Date,
    focused_day: Date,
    focus_request: u32,
    panel: CalendarPanel,
}

struct CalendarState {
    selection: Binding<Option<Date>>,
    options: CalendarOptions,
    session: Signal<CalendarSession>,
}

/// A month, year-of-months or twelve-year grid with one Tab stop per grid.
pub struct CalendarView {
    selection: Binding<Option<Date>>,
    options: CalendarOptions,
}
impl CalendarView {
    pub fn new(selection: Binding<Option<Date>>) -> Self {
        Self {
            selection,
            options: CalendarOptions::default().validated(),
        }
    }

    pub fn configure(mut self, configure: impl FnOnce(&mut CalendarOptions)) -> Self {
        configure(&mut self.options);
        self.options = self.options.validated();
        self
    }
    /// Width of the open calendar for popover hosts.
    pub fn width(cell_size: f32, options: &CalendarOptions) -> f32 {
        let months = if options.granularity.base_panel() == CalendarPanel::Days {
            options.presentation.number_of_months as f32
        } else {
            1.0
        };
        months * (Self::panel_width(cell_size, options.granularity) + 24.0)
    }

    fn panel_width(cell_size: f32, granularity: CalendarGranularity) -> f32 {
        cell_size * 7.0
            + 12.0
            + if granularity == CalendarGranularity::Week {
                WEEK_NUMBER_WIDTH + 2.0
            } else {
                0.0
            }
    }
}

impl CalendarState {
    fn set_month(&self, next: Date) {
        self.session.update(|session| session.month = next);
    }

    /// A pick on the base panel selects; a pick above it drills down into it.
    fn choose(&self, date: Date) {
        if !self.options.presentation.is_enabled {
            return;
        }
        let calendar = &self.options.calendar;
        let panel = self.session.get().panel;
        if panel == self.options.granularity.base_panel() {
            let start = granularity_start(self.options.granularity, date, calendar);
            self.selection.set(Some(start));
            let focused = CalendarCells::new(panel, self.options.granularity).start(date, calendar);
            self.session.update(|session| session.focused_day = focused);
            return;
        }
        let Some(child) = panel.child() else { return };
        self.show(child, date);
    }

    fn drill_up(&self) {
        let panel = self.session.get().panel;
        let Some(parent) = panel.parent() else { return };
        let focus = self.selection.get().unwrap_or_else(|| {
            let calendar = &self.options.calendar;
            CalendarCells::new(parent, self.options.granularity)
                .start(self.session.get().month, calendar)
        });
        self.show(parent, focus);
    }

    fn show(&self, panel: CalendarPanel, date: Date) {
        let calendar = &self.options.calendar;
        let focused = CalendarCells::new(panel, self.options.granularity).start(date, calendar);
        let month =
            crate::primitives::controls::date_values::CalendarGrid::month_start(date, calendar);
        self.session.update(|session| {
            session.panel = panel;
            session.focused_day = focused;
            session.month = month;
            session.focus_request = session.focus_request.saturating_add(1);
        });
    }

    fn browse(&self, months: i32) {
        let calendar = &self.options.calendar;
        let month = self.session.get().month;
        if let Some(next) = calendar.date_by_adding(CalendarUnit::Month, months, month) {
            self.set_month(next);
        }
    }

    /// Selected when the cell lies in the selected period, and only at the
    /// granularity's own panel.
    fn is_selected(&self, date: Date) -> bool {
        let Some(selected) = self.selection.get() else {
            return false;
        };
        let panel = self.session.get().panel;
        if panel != self.options.granularity.base_panel() {
            return false;
        }
        let unit = match panel {
            CalendarPanel::Days => self.options.granularity.component(),
            CalendarPanel::Months => CalendarUnit::Month,
            CalendarPanel::Years => CalendarUnit::Year,
        };
        interval_start(unit, date, &self.options.calendar)
            == interval_start(unit, selected, &self.options.calendar)
    }

    fn is_same_day(&self, date: Date, other: Date) -> bool {
        let calendar = &self.options.calendar;
        calendar.start_of_day(date) == calendar.start_of_day(other)
    }
}

impl View for CalendarView {
    fn body(&self, cx: &Scope) -> AnyView {
        let session = cx.state(|| {
            let initial = self.selection.get().unwrap_or(self.options.today);
            let panel = self.options.granularity.base_panel();
            CalendarSession {
                month: crate::primitives::controls::date_values::CalendarGrid::month_start(
                    initial,
                    &self.options.calendar,
                ),
                focused_day: CalendarCells::new(panel, self.options.granularity)
                    .start(initial, &self.options.calendar),
                focus_request: 0,
                panel,
            }
        });
        session.track();
        AnyView::new(CalendarBody {
            view: CalendarState {
                selection: self.selection.clone(),

                options: self.options.clone(),
                session,
            },
        })
    }
}
impl CalendarState {
    fn snapshot(&self) -> Self {
        Self {
            selection: self.selection.clone(),

            options: self.options.clone(),
            session: self.session,
        }
    }
}

/// The whole calendar: header, panels and the key host.
struct CalendarBody {
    view: CalendarState,
}

impl PrimitiveView for CalendarBody {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Group,
            label: crate::i18n::tr("Calendar"),
            ..Default::default()
        }));
        let state = self.view.session;
        let enabled = self.view.options.presentation.is_enabled;
        let weak = node.downgrade();
        node.set_layout_did_update(Some(Rc::new(move |owner: &Node| {
            let Some(chain) = cove_core::scene::FocusChainHolder::current() else {
                return;
            };
            if !enabled {
                if chain
                    .focused()
                    .is_some_and(|focused| chain.contains(&focused, owner))
                {
                    chain.clear();
                }
                return;
            }
            let session = state.get_untracked();
            if session.focus_request == 0
                || owner.attachment::<u32>("calendar.focusApplied") == Some(session.focus_request)
            {
                return;
            }
            let mut pending = owner.children();
            while let Some(candidate) = pending.pop() {
                if candidate.attachment::<Date>("calendar.day") == Some(session.focused_day)
                    && candidate.is_focusable()
                {
                    chain.focus(Some(&candidate), true);
                    if let Some(owner) = Node::upgrade(&weak) {
                        owner.set_attachment("calendar.focusApplied", session.focus_request);
                    }
                    break;
                }
                pending.extend(candidate.children());
            }
        })));
        let Some(registry) = cove_core::scene::InteractionRegistryHolder::current() else {
            return;
        };
        let view = self.view.snapshot();
        registry.set_key(
            node,
            cove_core::scene::InputHandlerRoute::control(),
            Rc::new(move |event, phase| {
                if phase != cove_core::scene::EventPhase::Bubble || !event.modifiers.is_empty() {
                    return EventResult::Ignored;
                }
                let Some(focused) =
                    cove_core::scene::FocusChainHolder::current().and_then(|chain| chain.focused())
                else {
                    return EventResult::Ignored;
                };
                let Some(date) = focused.attachment::<Date>("calendar.day") else {
                    return EventResult::Ignored;
                };
                if !view.options.presentation.is_enabled {
                    return EventResult::Ignored;
                }
                if matches!(
                    event.scancode,
                    crate::Scancode::RETURN
                        | crate::Scancode::KEYPAD_ENTER
                        | crate::Scancode::SPACE
                ) && !event.is_repeat
                {
                    view.choose(date);
                    return EventResult::Handled;
                }
                if view.navigate(date, event.scancode) {
                    EventResult::Handled
                } else {
                    EventResult::Ignored
                }
            }),
        );
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let view = self.view.snapshot();
        let panel = view.session.get().panel;
        let content = if panel == CalendarPanel::Days {
            let months = view.options.presentation.number_of_months;
            let mut columns: Vec<AnyView> = Vec::new();
            let base = view.session.get().month;
            // A touch surface raises every cell to the minimum control extent, so
            // the grid stays pointer- and finger-reachable. Swift does this once
            // per body, in `SurfaceReader`.
            let surface = node.composition_value(&SurfaceEnvironment::key());
            let authored = view.options.presentation.cell_size;
            let cell_size = if surface.input.contains(cove_core::InputCapabilities::TOUCH) {
                authored.max(surface.minimum_control_extent())
            } else {
                authored
            };
            for index in 0..months {
                let month = view
                    .options
                    .calendar
                    .date_by_adding(CalendarUnit::Month, index as i32, base)
                    .unwrap_or(base);
                columns.push(AnyView::new(CalendarMonth {
                    view: view.snapshot(),
                    month,
                    index,
                    count: months,
                    cell_size,
                }));
            }
            AnyView::new(BoxView::new(
                FlexDirection::Row,
                Align::FlexStart,
                Justify::FlexStart,
                Wrap::Wrap,
                16.0,
                AnyView::new(TupleView::new(columns)),
            ))
        } else {
            AnyView::new(CalendarPeriodPanel {
                view: view.snapshot(),
            })
        };
        vec![AnyView::new(
            ScrollView::new(ScrollAxis::Horizontal, content)
                .scrollbar_gutter(ScrollbarGutter::Stable),
        )]
    }
}
crate::impl_primitive_view!(CalendarBody);

impl CalendarState {
    fn navigate(&self, date: Date, scancode: u32) -> bool {
        let panel = self.session.get().panel;
        let cells = CalendarCells::new(panel, self.options.granularity);
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
            self.session.update(|session| {
                session.focused_day = next;
                session.focus_request = session.focus_request.saturating_add(1);
            });
        }
        self.reveal(next);
        true
    }

    /// Browses so the focused cell is on screen.
    fn reveal(&self, cell: Date) {
        let calendar = &self.options.calendar;
        let panel = self.session.get().panel;
        let cells = CalendarCells::new(panel, self.options.granularity);
        match panel {
            CalendarPanel::Days => {
                let month = self.session.get().month;
                let first = crate::primitives::controls::date_values::CalendarGrid::month_start(
                    month, calendar,
                );
                let Some(end) = calendar.date_by_adding(
                    CalendarUnit::Month,
                    self.options.presentation.number_of_months as i32,
                    first,
                ) else {
                    return;
                };
                let extent_start = interval_start(cells.period, cell, calendar);
                let extent_end = calendar
                    .date_interval_end(cells.period, cell)
                    .unwrap_or(extent_start);
                // A week starting before the 1st is visible in this month's grid.
                let anchor = if extent_start < first {
                    extent_end.adding_seconds(-1)
                } else {
                    extent_start
                };
                if anchor >= first && anchor < end {
                    return;
                }
                let target = crate::primitives::controls::date_values::CalendarGrid::month_start(
                    anchor, calendar,
                );
                if anchor < first {
                    self.set_month(target);
                } else {
                    let shifted = calendar.date_by_adding(
                        CalendarUnit::Month,
                        1 - self.options.presentation.number_of_months as i32,
                        target,
                    );
                    if let Some(shifted) = shifted {
                        self.set_month(shifted);
                    }
                }
            }
            CalendarPanel::Months => {
                let month = self.session.get().month;
                let same_year = calendar.component(CalendarUnit::Year, cell)
                    == calendar.component(CalendarUnit::Year, month);
                if !same_year {
                    self.set_month(cell);
                }
            }
            CalendarPanel::Years => {
                let month = self.session.get().month;
                let (start, end) = year_page(month, calendar);
                if !(cell >= start && cell < end) {
                    self.set_month(cell);
                }
            }
        }
    }
}

/// One month page: header, weekday row and six week rows.
struct CalendarMonth {
    view: CalendarState,
    month: Date,
    index: usize,
    count: usize,
    /// Resolved once for the whole page: a touch surface raises the authored
    /// cell size to `minimum_control_extent`, matching Swift's
    /// `max(cellSize, surface.input.contains(.touch) ? surface.minimumControlExtent : 0)`.
    cell_size: f32,
}

impl PrimitiveView for CalendarMonth {
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
        let view = &self.view;
        let calendar = &view.options.calendar;
        let cell_size = self.cell_size;
        let is_week = view.options.granularity == CalendarGranularity::Week;
        let title = month_title(self.month, calendar);
        let days = crate::primitives::controls::date_values::CalendarGrid::days_in_month(
            self.month, calendar,
        );
        let tab_cell = self.tab_stop(&days);
        let can_previous = self.index == 0;
        let can_next = self.index + 1 == self.count;

        let mut column: Vec<AnyView> = vec![AnyView::new(CalendarHeader {
            title: title.clone(),
            can_previous,
            can_next,
            show_drill_up: true,
            is_enabled: view.options.presentation.is_enabled,
            on_previous: {
                let view = view.snapshot();
                Rc::new(move || view.browse(-1))
            },
            on_next: {
                let view = view.snapshot();
                Rc::new(move || view.browse(1))
            },
            on_drill_up: {
                let view = view.snapshot();
                Rc::new(move || view.drill_up())
            },
        })];

        // Weekday row, rotated to the calendar's first weekday.
        let mut weekday_row: Vec<AnyView> = Vec::new();
        if is_week {
            weekday_row.push(AnyView::new(
                Text::new("Wk")
                    .semantic_font(SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::on_surface_muted())
                    .frame_size(WEEK_NUMBER_WIDTH, 24.0),
            ));
        }
        for weekday in 0..DAYS_PER_WEEK {
            let symbol = weekday_symbol((weekday as u32 + calendar.first_weekday() - 1) % 7);
            weekday_row.push(AnyView::new(
                Text::new(symbol)
                    .semantic_font(SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::on_surface_muted())
                    .frame_size(cell_size, 24.0),
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
            let row: Vec<Date> = days
                .iter()
                .skip(week * DAYS_PER_WEEK)
                .take(DAYS_PER_WEEK)
                .copied()
                .collect();
            if is_week {
                column.push(AnyView::new(CalendarWeekRow {
                    view: view.snapshot(),
                    days: row,
                    month: self.month,
                    tab_cell,
                    cell_size,
                }));
            } else {
                let mut cells: Vec<AnyView> = Vec::new();
                for day in row {
                    cells.push(AnyView::new(CalendarDayCell {
                        view: view.snapshot(),
                        day,
                        month: self.month,
                        is_tab_stop: tab_cell == Some(day),
                        cell_size,
                    }));
                }
                column.push(AnyView::new(BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    2.0,
                    AnyView::new(TupleView::new(cells)),
                )));
            }
        }

        let min_width = CalendarView::panel_width(cell_size, view.options.granularity) + 20.0;
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                6.0,
                AnyView::new(TupleView::new(column)),
            )
            .padding(10.0)
            .frame_min_width(min_width)
            .semantic_background(SemanticColorRef::background())
            .corner_radius_metric(crate::theme::ThemeMetric::control_radius()),
        )]
    }
}
crate::impl_primitive_view!(CalendarMonth);

impl CalendarMonth {
    /// The focused cell when it is in this month, else the first available one.
    fn tab_stop(&self, days: &[Date]) -> Option<Date> {
        let calendar = &self.view.options.calendar;
        let availability = &self.view.options.availability;
        let granularity = self.view.options.granularity;
        let enabled = self.view.options.presentation.is_enabled;
        let mut candidates: Vec<Date> = days
            .iter()
            .filter(|day| {
                interval_start(CalendarUnit::Month, **day, calendar)
                    == interval_start(CalendarUnit::Month, self.month, calendar)
            })
            .map(|day| CalendarCells::new(CalendarPanel::Days, granularity).start(*day, calendar))
            .filter(|candidate| {
                enabled
                    && CalendarCells::new(CalendarPanel::Days, granularity).is_available(
                        *candidate,
                        calendar,
                        availability,
                    )
            })
            .collect();
        candidates.sort();
        candidates.dedup();
        let focused = CalendarCells::new(CalendarPanel::Days, granularity)
            .start(self.view.session.get().focused_day, calendar);
        if candidates.contains(&focused) {
            Some(focused)
        } else {
            candidates.first().copied()
        }
    }
}

/// The month name for a one-based month number.
pub fn month_name(month: u32) -> String {
    crate::i18n::tr(&format!("calendar.month.{}", month.clamp(1, 12)))
}

/// The abbreviated month name for a one-based month number.
pub fn month_abbreviation(month: u32) -> String {
    crate::i18n::tr(&format!("calendar.month_short.{}", month.clamp(1, 12)))
}

/// The very short weekday symbol for a Sunday-based index.
pub fn weekday_symbol(index: u32) -> String {
    crate::i18n::tr(&format!(
        "calendar.weekday.{}",
        index % DAYS_PER_WEEK as u32
    ))
}

/// The month title: the localized month and the year.
pub fn month_title(date: Date, calendar: &FoundationCalendar) -> String {
    let parts = calendar.parts(date);
    crate::i18n::format_template(
        &crate::i18n::tr("calendar.month_title"),
        &[
            ("month", &month_name(parts.month)),
            ("year", &parts.year.to_string()),
        ],
    )
}

/// A long date: `January 5, 2026` in English, `2026年1月5日` in Chinese.
pub fn long_date(date: Date, calendar: &FoundationCalendar) -> String {
    let parts = calendar.parts(date);
    crate::i18n::format_template(
        &crate::i18n::tr("calendar.date_long"),
        &[
            ("month", &month_name(parts.month)),
            ("day", &parts.day.to_string()),
            ("year", &parts.year.to_string()),
        ],
    )
}

/// A medium date: `Jan 5, 2026` in English, `2026年1月5日` in Chinese.
pub fn medium_date(date: Date, calendar: &FoundationCalendar) -> String {
    let parts = calendar.parts(date);
    crate::i18n::format_template(
        &crate::i18n::tr("calendar.date_medium"),
        &[
            ("month_short", &month_abbreviation(parts.month)),
            ("day", &parts.day.to_string()),
            ("year", &parts.year.to_string()),
        ],
    )
}

/// Panel header: browse arrows, a title, and the drill-up target above the base
/// panel.
struct CalendarHeader {
    title: String,
    can_previous: bool,
    can_next: bool,
    show_drill_up: bool,
    is_enabled: bool,
    on_previous: Rc<dyn Fn()>,
    on_next: Rc<dyn Fn()>,
    on_drill_up: Rc<dyn Fn()>,
}

impl PrimitiveView for CalendarHeader {
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
        let previous = Button::icon_with_options(
            ButtonIconSource::resource(
                crate::primitives::media::ui_icon_catalog::navigation::chevron_left(),
            ),
            Some(12.0),
            None,
            ButtonRole::Normal,
            self.is_enabled && self.can_previous,
            false,
            false,
            Some("Previous month".to_string()),
            {
                let on_previous = self.on_previous.clone();
                move || on_previous()
            },
        )
        .button_style(GhostButtonStyle);
        let next = Button::icon_with_options(
            ButtonIconSource::resource(
                crate::primitives::media::ui_icon_catalog::navigation::chevron_right(),
            ),
            Some(12.0),
            None,
            ButtonRole::Normal,
            self.is_enabled && self.can_next,
            false,
            false,
            Some("Next month".to_string()),
            {
                let on_next = self.on_next.clone();
                move || on_next()
            },
        )
        .button_style(GhostButtonStyle);
        let title: AnyView = if self.show_drill_up {
            let on_drill_up = self.on_drill_up.clone();
            AnyView::new(
                Button::with_options(
                    AnyView::new(
                        Text::new(self.title.clone()).semantic_font(SemanticFontRef::body_strong()),
                    ),
                    move || on_drill_up(),
                    ButtonRole::Normal,
                    self.is_enabled,
                    false,
                    false,
                    None,
                )
                .button_style(GhostButtonStyle),
            )
        } else {
            AnyView::new(
                Text::new(self.title.clone())
                    .semantic_font(SemanticFontRef::body_strong())
                    .padding_edges(12.0, 0.0)
                    .flex_with(1.0, 1.0, None),
            )
        };
        vec![AnyView::new(Row::new(
            VerticalAlignment::Center,
            4.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(previous),
                AnyView::new(title),
                AnyView::new(crate::Spacer::new(0.0)),
                AnyView::new(next),
            ])),
        ))]
    }
}
crate::impl_primitive_view!(CalendarHeader);

/// One day cell.
struct CalendarDayCell {
    view: CalendarState,
    day: Date,
    month: Date,
    is_tab_stop: bool,
    cell_size: f32,
}

impl PrimitiveView for CalendarDayCell {
    fn make_node(&self) -> Node {
        Node::new()
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
        let calendar = &self.view.options.calendar;
        let in_month = interval_start(CalendarUnit::Month, self.day, calendar)
            == interval_start(CalendarUnit::Month, self.month, calendar);
        let is_enabled = in_month && self.is_enabled(self.day);
        node.set_attachment("calendar.day", self.day);
        node.set_is_focusable(is_enabled);
        node.set_is_tab_stop(self.is_tab_stop && is_enabled);
        node.set_is_hit_testable(is_enabled);
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Button,
            label: match calendar.parts(self.day).month {
                1 => format!(
                    "January {}, {}",
                    calendar.parts(self.day).day,
                    calendar.parts(self.day).year
                ),
                2 => format!(
                    "February {}, {}",
                    calendar.parts(self.day).day,
                    calendar.parts(self.day).year
                ),
                3 => format!(
                    "March {}, {}",
                    calendar.parts(self.day).day,
                    calendar.parts(self.day).year
                ),
                4 => format!(
                    "April {}, {}",
                    calendar.parts(self.day).day,
                    calendar.parts(self.day).year
                ),
                5 => format!(
                    "May {}, {}",
                    calendar.parts(self.day).day,
                    calendar.parts(self.day).year
                ),
                6 => format!(
                    "June {}, {}",
                    calendar.parts(self.day).day,
                    calendar.parts(self.day).year
                ),
                7 => format!(
                    "July {}, {}",
                    calendar.parts(self.day).day,
                    calendar.parts(self.day).year
                ),
                8 => format!(
                    "August {}, {}",
                    calendar.parts(self.day).day,
                    calendar.parts(self.day).year
                ),
                9 => format!(
                    "September {}, {}",
                    calendar.parts(self.day).day,
                    calendar.parts(self.day).year
                ),
                10 => format!(
                    "October {}, {}",
                    calendar.parts(self.day).day,
                    calendar.parts(self.day).year
                ),
                11 => format!(
                    "November {}, {}",
                    calendar.parts(self.day).day,
                    calendar.parts(self.day).year
                ),
                _ => format!(
                    "December {}, {}",
                    calendar.parts(self.day).day,
                    calendar.parts(self.day).year
                ),
            },
            state: cove_core::scene::AccessibilityState {
                is_enabled,
                is_selected: self.view.is_selected(self.day),
                ..Default::default()
            },
            ..Default::default()
        }));

        let calendar = &self.view.options.calendar;
        let is_selected = self.view.is_selected(self.day);
        let is_today = self.view.is_same_day(self.day, self.view.options.today);
        let in_month = interval_start(CalendarUnit::Month, self.day, calendar)
            == interval_start(CalendarUnit::Month, self.month, calendar);
        let theme = theme_for_node(node);
        node.set_corner_radius(theme.radius.md);
        node.set_opacity(if in_month { 1.0 } else { 0.35 });
        node.set_background_color(if is_selected {
            Some(theme.colors.accent)
        } else {
            None
        });
        node.set_border_color(Some(if is_today {
            theme.colors.accent
        } else {
            cove_core::Color::CLEAR
        }));
        node.set_border_width(if is_today { 1.0 } else { 0.0 });

        let Some(registry) = cove_core::scene::InteractionRegistryHolder::current() else {
            return;
        };
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
                    return EventResult::Ignored;
                }
                view.choose(day);
                EventResult::Handled
            }),
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let calendar = &self.view.options.calendar;
        let is_selected = self.view.is_selected(self.day);
        let in_month = interval_start(CalendarUnit::Month, self.day, calendar)
            == interval_start(CalendarUnit::Month, self.month, calendar);
        let day = calendar.parts(self.day).day;
        let tint = if is_selected {
            SemanticColorRef::on_accent()
        } else if in_month {
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
                    Text::new(day.to_string())
                        .semantic_font(SemanticFontRef::label())
                        .semantic_foreground(tint),
                ),
            )
            .frame_size(self.cell_size, self.cell_size),
        )]
    }
}
crate::impl_primitive_view!(CalendarDayCell);

impl CalendarDayCell {
    fn is_enabled(&self, day: Date) -> bool {
        self.view.options.presentation.is_enabled
            && self
                .view
                .options
                .availability
                .contains(day, &self.view.options.calendar)
    }
}

/// A week row: one button for the whole week, led by its number.
struct CalendarWeekRow {
    view: CalendarState,
    days: Vec<Date>,
    month: Date,
    tab_cell: Option<Date>,
    cell_size: f32,
}

impl PrimitiveView for CalendarWeekRow {
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
        let calendar = &self.view.options.calendar;
        let Some(first) = self.days.first().copied() else {
            return Vec::new();
        };
        let start = CalendarCells::new(CalendarPanel::Days, self.view.options.granularity)
            .start(first, calendar);
        let in_month = self.days.iter().any(|day| {
            interval_start(CalendarUnit::Month, *day, calendar)
                == interval_start(CalendarUnit::Month, self.month, calendar)
        });
        let available = in_month
            && self.view.options.presentation.is_enabled
            && self.view.options.availability.contains(start, calendar);
        let is_selected = self.view.is_selected(start);
        let week = calendar.component(CalendarUnit::WeekOfYear, start);

        let mut row: Vec<AnyView> = vec![AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::Center,
                Wrap::NoWrap,
                0.0,
                AnyView::new(
                    Text::new(week.to_string())
                        .semantic_font(SemanticFontRef::caption())
                        .opacity(0.75),
                ),
            )
            .frame_size(WEEK_NUMBER_WIDTH, self.cell_size),
        )];
        for day in &self.days {
            let in_month_cell = interval_start(CalendarUnit::Month, *day, calendar)
                == interval_start(CalendarUnit::Month, self.month, calendar);
            row.push(AnyView::new(
                BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::Center,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(
                        Text::new(calendar.parts(*day).day.to_string())
                            .semantic_font(SemanticFontRef::label()),
                    ),
                )
                .opacity(if in_month_cell { 1.0 } else { 0.4 })
                .frame_size(self.cell_size, self.cell_size),
            ));
        }

        vec![AnyView::new(CalendarWeekButton {
            view: self.view.snapshot(),
            start,
            is_tab_stop: self.tab_cell == Some(start),
            available,
            is_selected,
            content: AnyView::new(BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                2.0,
                AnyView::new(TupleView::new(row)),
            )),
        })]
    }
}
crate::impl_primitive_view!(CalendarWeekRow);

/// The pressable week row.
struct CalendarWeekButton {
    view: CalendarState,
    start: Date,
    is_tab_stop: bool,
    available: bool,
    is_selected: bool,
    content: AnyView,
}

impl PrimitiveView for CalendarWeekButton {
    fn make_node(&self) -> Node {
        Node::new()
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        node.set_attachment("calendar.day", self.start);
        node.set_is_focusable(self.available);
        node.set_is_tab_stop(self.is_tab_stop && self.available);
        node.set_is_hit_testable(self.available);
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Button,
            label: format!(
                "Week {}",
                self.view
                    .options
                    .calendar
                    .component(CalendarUnit::WeekOfYear, self.start)
            ),
            state: cove_core::scene::AccessibilityState {
                is_enabled: self.available,
                is_selected: self.is_selected,
                ..Default::default()
            },
            ..Default::default()
        }));

        let theme = theme_for_node(node);
        node.set_corner_radius(theme.radius.md);
        node.set_background_color(if self.is_selected {
            Some(theme.colors.accent)
        } else {
            None
        });
        let Some(registry) = cove_core::scene::InteractionRegistryHolder::current() else {
            return;
        };
        if !self.available {
            registry.remove(node);
            return;
        }
        let view = self.view.snapshot();
        let start = self.start;
        registry.set_pointer(
            node,
            cove_core::scene::InputHandlerRoute::control(),
            Rc::new(move |event, phase, event_phase| {
                if event.button != cove_platform::MouseButton::Left
                    || phase != cove_core::scene::PointerPhase::Up
                    || event_phase != cove_core::scene::EventPhase::Target
                {
                    return EventResult::Ignored;
                }
                view.choose(start);
                EventResult::Handled
            }),
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
crate::impl_primitive_view!(CalendarWeekButton);

/// The months/years panel: twelve cells in a four-by-three page.
struct CalendarPeriodPanel {
    view: CalendarState,
}

impl PrimitiveView for CalendarPeriodPanel {
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
        let view = &self.view;
        let calendar = &view.options.calendar;
        let panel = view.session.get().panel;
        let is_months = panel == CalendarPanel::Months;
        let month = view.session.get().month;
        let (page_start, page_end) = if is_months {
            let start = interval_start(CalendarUnit::Year, month, calendar);
            let end = calendar
                .date_interval_end(CalendarUnit::Year, month)
                .unwrap_or(start);
            (start, end)
        } else {
            year_page(month, calendar)
        };
        let unit = if is_months {
            CalendarUnit::Month
        } else {
            CalendarUnit::Year
        };
        let dates: Vec<Date> = (0..12)
            .filter_map(|offset| calendar.date_by_adding(unit, offset, page_start))
            .collect();
        let cell_size = view.options.presentation.cell_size;
        let width = CalendarView::panel_width(cell_size, view.options.granularity);
        let cell_width = (width - 8.0) / 3.0;
        let cell_height = (cell_size + 8.0).max(40.0);
        let title = if is_months {
            calendar.parts(page_start).year.to_string()
        } else {
            format!(
                "{} – {}",
                calendar.parts(page_start).year,
                calendar.parts(page_end).year - 1
            )
        };
        let available: Vec<Date> = dates
            .iter()
            .copied()
            .filter(|date| {
                view.options.presentation.is_enabled
                    && CalendarCells::new(panel, view.options.granularity).is_available(
                        *date,
                        calendar,
                        &view.options.availability,
                    )
            })
            .collect();
        let focused = CalendarCells::new(panel, view.options.granularity)
            .start(view.session.get().focused_day, calendar);
        let tab_cell = if available.contains(&focused) {
            Some(focused)
        } else {
            available.first().copied()
        };

        let mut column: Vec<AnyView> = vec![AnyView::new(CalendarHeader {
            title,
            can_previous: true,
            can_next: true,
            show_drill_up: !is_months || panel != CalendarPanel::Years,
            is_enabled: view.options.presentation.is_enabled,
            on_previous: {
                let view = view.snapshot();
                let step = if is_months { 12 } else { 144 };
                Rc::new(move || {
                    view.set_month(
                        view.options
                            .calendar
                            .date_by_adding(CalendarUnit::Month, -step, view.session.get().month)
                            .unwrap_or_else(|| view.session.get().month),
                    )
                })
            },
            on_next: {
                let view = view.snapshot();
                let step = if is_months { 12 } else { 144 };
                Rc::new(move || {
                    view.set_month(
                        view.options
                            .calendar
                            .date_by_adding(CalendarUnit::Month, step, view.session.get().month)
                            .unwrap_or_else(|| view.session.get().month),
                    )
                })
            },
            on_drill_up: {
                let view = view.snapshot();
                Rc::new(move || view.drill_up())
            },
        })];
        for row in 0..4 {
            let mut cells: Vec<AnyView> = Vec::new();
            for date in dates.iter().skip(row * 3).take(3) {
                cells.push(AnyView::new(CalendarPeriodCell {
                    view: view.snapshot(),
                    date: *date,
                    is_months,
                    is_available: available.contains(date),
                    is_tab_stop: tab_cell == Some(*date),
                    width: cell_width,
                    height: cell_height,
                }));
            }
            column.push(AnyView::new(BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                4.0,
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
            .padding(10.0)
            .frame_min_width(width + 20.0)
            .semantic_background(SemanticColorRef::background())
            .corner_radius_metric(crate::theme::ThemeMetric::control_radius()),
        )]
    }
}
crate::impl_primitive_view!(CalendarPeriodPanel);

/// One month or year cell.
struct CalendarPeriodCell {
    view: CalendarState,
    date: Date,
    is_months: bool,
    is_available: bool,
    is_tab_stop: bool,
    width: f32,
    height: f32,
}

impl PrimitiveView for CalendarPeriodCell {
    fn make_node(&self) -> Node {
        Node::new()
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        let layout = cove_core::scene::LayoutNode::new();
        layout.set_width(Some(self.width));
        layout.set_height(Some(self.height));
        layout.set_flex_shrink(0.0);
        Some(layout)
    }

    fn update_layout(&self, layout: &cove_core::scene::LayoutNode) {
        layout.set_width(Some(self.width));
        layout.set_height(Some(self.height));
        layout.set_flex_shrink(0.0);
    }

    fn update_node(&self, node: &Node) {
        node.set_attachment("calendar.day", self.date);
        node.set_is_focusable(self.is_available);
        node.set_is_tab_stop(self.is_tab_stop && self.is_available);
        node.set_is_hit_testable(self.is_available);
        let label = period_label(self.date, self.is_months, &self.view.options.calendar);
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Button,
            label,
            state: cove_core::scene::AccessibilityState {
                is_enabled: self.is_available,
                is_selected: self.view.is_selected(self.date),
                ..Default::default()
            },
            ..Default::default()
        }));

        let theme = theme_for_node(node);
        node.set_corner_radius(theme.radius.md);
        node.set_background_color(if self.view.is_selected(self.date) {
            Some(theme.colors.accent)
        } else {
            None
        });
        let Some(registry) = cove_core::scene::InteractionRegistryHolder::current() else {
            return;
        };
        if !self.is_available {
            registry.remove(node);
            return;
        }
        let view = self.view.snapshot();
        let date = self.date;
        registry.set_pointer(
            node,
            cove_core::scene::InputHandlerRoute::control(),
            Rc::new(move |event, phase, event_phase| {
                if event.button != cove_platform::MouseButton::Left
                    || phase != cove_core::scene::PointerPhase::Up
                    || event_phase != cove_core::scene::EventPhase::Target
                {
                    return EventResult::Ignored;
                }
                view.choose(date);
                EventResult::Handled
            }),
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let text: String = if self.is_months {
            short_month_name(self.view.options.calendar.parts(self.date).month)
        } else {
            self.view.options.calendar.parts(self.date).year.to_string()
        };
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::Center,
                Wrap::NoWrap,
                0.0,
                AnyView::new(Text::new(text).semantic_font(SemanticFontRef::label())),
            )
            .frame_size(self.width, self.height),
        )]
    }
}
crate::impl_primitive_view!(CalendarPeriodCell);

fn short_month_name(month: u32) -> String {
    month_abbreviation(month)
}

fn period_label(date: Date, is_months: bool, calendar: &FoundationCalendar) -> String {
    if is_months {
        month_title(date, calendar)
    } else {
        calendar.parts(date).year.to_string()
    }
}
