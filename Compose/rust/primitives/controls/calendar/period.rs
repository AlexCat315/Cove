//! Translation of Primitives/Controls/Calendar/Period.swift.
//!
//! What one selection covers, and the browsing level above it.
use crate::foundation::{Calendar, CalendarUnit, Date};

use super::super::date_values::DateAvailability;

/// What one selection covers. The bound date is the period's first day, and any
/// date inside the period shows as selected, so callers may store either.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CalendarGranularity {
    #[default]
    Day,
    Week,
    Month,
    Year,
}

impl CalendarGranularity {
    pub fn component(self) -> CalendarUnit {
        match self {
            Self::Day => CalendarUnit::Day,
            Self::Week => CalendarUnit::WeekOfYear,
            Self::Month => CalendarUnit::Month,
            Self::Year => CalendarUnit::Year,
        }
    }

    /// The most detailed panel this granularity can browse.
    pub fn base_panel(self) -> CalendarPanel {
        match self {
            Self::Day | Self::Week => CalendarPanel::Days,
            Self::Month => CalendarPanel::Months,
            Self::Year => CalendarPanel::Years,
        }
    }
}

/// The browsing level: a month of days, a year of months, or twelve years.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CalendarPanel {
    #[default]
    Days,
    Months,
    Years,
}

impl CalendarPanel {
    pub fn parent(self) -> Option<Self> {
        match self {
            Self::Days => Some(Self::Months),
            Self::Months => Some(Self::Years),
            Self::Years => None,
        }
    }

    pub fn child(self) -> Option<Self> {
        match self {
            Self::Days => None,
            Self::Months => Some(Self::Days),
            Self::Years => Some(Self::Months),
        }
    }
}

/// Start of the unit containing `date`.
pub fn interval_start(of: CalendarUnit, date: Date, calendar: &Calendar) -> Date {
    calendar
        .date_interval_start(of, date)
        .unwrap_or_else(|| calendar.start_of_day(date))
}

/// Start of the period a granularity selects.
pub fn granularity_start(of: CalendarGranularity, date: Date, calendar: &Calendar) -> Date {
    interval_start(of.component(), date, calendar)
}

/// The twelve-year page containing `date`, aligned so pages never overlap.
pub fn year_page(date: Date, calendar: &Calendar) -> (Date, Date) {
    let year = calendar.component(CalendarUnit::Year, date) as i64;
    let first = year - year.rem_euclid(12);
    let start = calendar
        .date(crate::foundation::DateParts::new(first as i32, 1, 1));
    let end = calendar
        .date_by_adding(CalendarUnit::Year, 12, start)
        .unwrap_or(start);
    (start, end)
}

/// True when any day of the interval is available.
pub fn has_available_day(
    start: Date,
    end: Date,
    calendar: &Calendar,
    availability: &DateAvailability,
) -> bool {
    if let Some(minimum) = availability.minimum
        && end <= calendar.start_of_day(minimum)
    {
        return false;
    }
    if let Some(maximum) = availability.maximum
        && start > calendar.start_of_day(maximum)
    {
        return false;
    }
    let mut day = start;
    while day < end {
        if availability.contains(day, calendar) {
            return true;
        }
        let Some(next) = calendar.date_by_adding(CalendarUnit::Day, 1, day) else { return false };
        if next <= day {
            return false;
        }
        day = next;
    }
    false
}

/// Whether `date` falls inside the interval, inclusive of the start.
pub fn contains(start: Date, end: Date, date: Date) -> bool {
    date >= start && date < end
}
