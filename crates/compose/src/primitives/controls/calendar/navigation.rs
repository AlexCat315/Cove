//! Translation of Primitives/Controls/Calendar/Navigation.swift.
//!
//! The cells one panel's keys move between, and where a key press lands.
//! Keyboard destinations are transient; only activation writes the selection.
use crate::foundation::{Calendar, CalendarUnit, Date};

use super::super::date_values::DateAvailability;
use super::period::{
    CalendarGranularity, CalendarPanel, has_available_day, interval_start, year_page,
};

/// The cells one panel's keys move between: their extent, one step, how many sit
/// in a row, and the page that PageUp/PageDown and Home/End use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CalendarCells {
    pub period: CalendarUnit,
    pub unit: CalendarUnit,
    pub unit_count: i32,
    pub columns: i32,
    pub page: CalendarUnit,
    pub page_count: i32,
}

impl CalendarCells {
    pub fn new(panel: CalendarPanel, granularity: CalendarGranularity) -> Self {
        match (panel, granularity) {
            (CalendarPanel::Days, CalendarGranularity::Week) => Self {
                period: CalendarUnit::WeekOfYear,
                unit: CalendarUnit::Day,
                unit_count: 7,
                columns: 1,
                page: CalendarUnit::Month,
                page_count: 1,
            },
            (CalendarPanel::Days, _) => Self {
                period: CalendarUnit::Day,
                unit: CalendarUnit::Day,
                unit_count: 1,
                columns: 7,
                page: CalendarUnit::Month,
                page_count: 1,
            },
            (CalendarPanel::Months, _) => Self {
                period: CalendarUnit::Month,
                unit: CalendarUnit::Month,
                unit_count: 1,
                columns: 3,
                page: CalendarUnit::Year,
                page_count: 1,
            },
            (CalendarPanel::Years, _) => Self {
                period: CalendarUnit::Year,
                unit: CalendarUnit::Year,
                unit_count: 1,
                columns: 3,
                page: CalendarUnit::Year,
                page_count: 12,
            },
        }
    }

    pub fn start(&self, date: Date, calendar: &Calendar) -> Date {
        interval_start(self.period, date, calendar)
    }

    pub fn is_available(
        &self,
        date: Date,
        calendar: &Calendar,
        availability: &DateAvailability,
    ) -> bool {
        if self.period == CalendarUnit::Day {
            availability.contains(date, calendar)
        } else {
            let start = interval_start(self.period, date, calendar);
            let end = calendar
                .date_interval_end(self.period, date)
                .unwrap_or(start);
            has_available_day(start, end, calendar, availability)
        }
    }

    /// The page containing `date`: a month of days or weeks, a year of months, or
    /// a twelve-year page of years.
    pub fn bounds(&self, date: Date, calendar: &Calendar) -> (Date, Date) {
        if self.page_count == 12 {
            year_page(date, calendar)
        } else {
            let start = interval_start(self.page, date, calendar);
            let end = calendar.date_interval_end(self.page, date).unwrap_or(start);
            (start, end)
        }
    }
}

/// One keyboard destination.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalendarNavigation {
    Step(i32),
    Page(i32),
    First,
    Last,
}

impl CalendarNavigation {
    pub fn new(scancode: u32, columns: i32) -> Option<Self> {
        Some(match scancode {
            crate::Scancode::ARROW_LEFT => Self::Step(-1),
            crate::Scancode::ARROW_RIGHT => Self::Step(1),
            crate::Scancode::ARROW_UP => Self::Step(-columns),
            crate::Scancode::ARROW_DOWN => Self::Step(columns),
            crate::Scancode::HOME => Self::First,
            crate::Scancode::END => Self::Last,
            crate::Scancode::PAGE_UP => Self::Page(-1),
            crate::Scancode::PAGE_DOWN => Self::Page(1),
            _ => return None,
        })
    }

    pub fn destination(
        &self,
        date: Date,
        cells: CalendarCells,
        calendar: &Calendar,
        availability: &DateAvailability,
    ) -> Option<Date> {
        match *self {
            Self::Step(count) => {
                let mut candidate = date;
                for _ in 0..3660 {
                    let next =
                        calendar.date_by_adding(cells.unit, count * cells.unit_count, candidate)?;
                    candidate = next;
                    if cells.is_available(candidate, calendar, availability) {
                        return Some(cells.start(candidate, calendar));
                    }
                }
                None
            }
            Self::First | Self::Last => {
                let direction = if *self == Self::First { 1 } else { -1 };
                let (start, end) = cells.bounds(date, calendar);
                let origin = if direction == 1 {
                    start
                } else {
                    end.adding_seconds(-1)
                };
                scan(origin, direction, start, end, cells, calendar, availability)
            }
            Self::Page(count) => {
                let target = calendar.date_by_adding(cells.page, count * cells.page_count, date)?;
                let (start, end) = cells.bounds(target, calendar);
                let direction = if count > 0 { 1 } else { -1 };
                // Try forward within the page, then the other way, so a page whose
                // first cells are unavailable still lands somewhere.
                scan(target, direction, start, end, cells, calendar, availability).or_else(|| {
                    scan(
                        target,
                        -direction,
                        start,
                        end,
                        cells,
                        calendar,
                        availability,
                    )
                })
            }
        }
    }
}

/// The first available cell overlapping `bounds`, walking from `date`.
fn scan(
    date: Date,
    direction: i32,
    bound_start: Date,
    bound_end: Date,
    cells: CalendarCells,
    calendar: &Calendar,
    availability: &DateAvailability,
) -> Option<Date> {
    let mut candidate = cells.start(date, calendar);
    for _ in 0..400 {
        let extent_start = interval_start(cells.period, candidate, calendar);
        let extent_end = calendar
            .date_interval_end(cells.period, candidate)
            .unwrap_or(extent_start);
        if extent_end <= bound_start || extent_start >= bound_end {
            return None;
        }
        if cells.is_available(candidate, calendar, availability) {
            return Some(candidate);
        }
        let next = calendar.date_by_adding(cells.unit, direction * cells.unit_count, candidate)?;
        candidate = cells.start(next, calendar);
    }
    None
}
