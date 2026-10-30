//! Translation of `Primitives/Controls/Fields/DateRange/Rules.swift`.
//!
//! Range-only rules layered on the shared calendar options: shortcut presets
//! and an inclusive maximum length in days.
//!
//! Behaviour checklist:
//! - a preset result is normalized to start-of-day bounds, in either order
//! - `yesterday`, `last_days`, `this_month` and `last_month` resolve against the
//!   calendar, not against a fixed month length
//! - `last_days(0)` clamps to one day
//! - the day count includes both endpoints
//! - an unlimited rule allows any range
//! - `can_complete` measures the range the two days would form, in either order

use std::rc::Rc;

use crate::foundation::{Calendar, CalendarUnit, Date};

/// A shortcut that resolves to a closed day range relative to `today`.
#[derive(Clone)]
pub struct DateRangePreset {
    pub id: String,
    pub title: String,
    resolve: Rc<dyn Fn(Date, &Calendar) -> Option<(Date, Date)>>,
}

impl std::fmt::Debug for DateRangePreset {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DateRangePreset")
            .field("id", &self.id)
            .field("title", &self.title)
            .finish_non_exhaustive()
    }
}

impl DateRangePreset {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        resolve: impl Fn(Date, &Calendar) -> Option<(Date, Date)> + 'static,
    ) -> Self {
        Self { id: id.into(), title: title.into(), resolve: Rc::new(resolve) }
    }

    /// The normalized range for `today`, or `None` when the resolver cannot
    /// answer. Bounds are ordered and snapped to start-of-day, so a preset that
    /// returns them backwards still yields a valid range.
    pub fn range(&self, today: Date, calendar: &Calendar) -> Option<(Date, Date)> {
        let (first, second) = (self.resolve)(today, calendar)?;
        let (low, high) = order((first, second));
        Some((calendar.start_of_day(low), calendar.start_of_day(high)))
    }

    pub fn today_range(title: impl Into<String>) -> Self {
        Self::new("today", title, |today, _| Some((today, today)))
    }

    pub fn yesterday(title: impl Into<String>) -> Self {
        Self::new("yesterday", title, |today, calendar| {
            calendar.date_by_adding(CalendarUnit::Day, -1, today).map(|day| (day, day))
        })
    }

    /// The last `count` days including today. A count below one becomes one.
    pub fn last_days(count: i32, title: Option<String>) -> Self {
        let count = count.max(1);
        let title = title.unwrap_or_else(|| format!("Last {count} days"));
        Self::new(format!("last-{count}-days"), title, move |today, calendar| {
            calendar
                .date_by_adding(CalendarUnit::Day, 1 - count, today)
                .map(|start| (start, today))
        })
    }

    pub fn this_month(title: impl Into<String>) -> Self {
        Self::new("this-month", title, |today, calendar| month_containing(today, calendar))
    }

    pub fn last_month(title: impl Into<String>) -> Self {
        Self::new("last-month", title, |today, calendar| {
            calendar
                .date_by_adding(CalendarUnit::Month, -1, today)
                .and_then(|previous| month_containing(previous, calendar))
        })
    }
}

/// The first and last day of the month containing `day`.
fn month_containing(day: Date, calendar: &Calendar) -> Option<(Date, Date)> {
    let start = calendar.date_interval_start(CalendarUnit::Month, day)?;
    let next = calendar.date_by_adding(CalendarUnit::Month, 1, start)?;
    let last = calendar.date_by_adding(CalendarUnit::Day, -1, next)?;
    Some((start, last))
}

/// Range-only validation rules for a date-range picker.
#[derive(Clone, Debug, Default)]
pub struct DateRangeRules {
    pub presets: Vec<DateRangePreset>,
    /// Longest selectable range, counting both endpoints. `None` is unlimited.
    pub maximum_days: Option<i32>,
}

impl DateRangeRules {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_presets(mut self, presets: Vec<DateRangePreset>) -> Self {
        self.presets = presets;
        self
    }

    pub fn with_maximum_days(mut self, maximum_days: i32) -> Self {
        self.maximum_days = Some(maximum_days);
        self.validate();
        self
    }

    /// A maximum below one becomes one, matching `max(1, $0)`.
    pub fn validate(&mut self) {
        self.maximum_days = self.maximum_days.map(|value| value.max(1));
    }

    /// Inclusive day count of a start-of-day range.
    pub fn day_count(range: (Date, Date), calendar: &Calendar) -> i32 {
        let (lower, upper) = order(range);
        let start = calendar.start_of_day(lower);
        let end = calendar.start_of_day(upper);
        (end.days_between(start) + 1) as i32
    }

    /// Whether the range fits the maximum, when one is set.
    pub fn allows(&self, range: (Date, Date), calendar: &Calendar) -> bool {
        match self.maximum_days {
            None => true,
            Some(maximum) => Self::day_count(range, calendar) <= maximum,
        }
    }

    /// Whether `day` can complete a range started at `start`.
    ///
    /// The candidate range is measured in either direction, so dragging upwards
    /// is validated the same way as dragging down.
    pub fn can_complete(&self, start: Date, day: Date, calendar: &Calendar) -> bool {
        self.allows(order((start, day)), calendar)
    }
}

fn order(range: (Date, Date)) -> (Date, Date) {
    if range.0 <= range.1 {
        range
    } else {
        (range.1, range.0)
    }
}
