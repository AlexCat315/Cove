//! Translation of Primitives/Controls/DateValues.swift.
use crate::foundation::{Calendar, CalendarUnit, Date};
use serde::{Deserialize, Deserializer, Serialize};
use std::rc::Rc;

#[derive(Clone)]
pub struct DateAvailability {
    pub minimum: Option<Date>,
    pub maximum: Option<Date>,
    pub is_enabled: Rc<dyn Fn(Date) -> bool>,
}

impl Default for DateAvailability {
    fn default() -> Self {
        Self { minimum: None, maximum: None, is_enabled: Rc::new(|_| true) }
    }
}

impl DateAvailability {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn contains(&self, date: Date, calendar: &Calendar) -> bool {
        let day = calendar.start_of_day(date);
        if let Some(minimum) = self.minimum
            && day < calendar.start_of_day(minimum)
        {
            return false;
        }
        if let Some(maximum) = self.maximum
            && day > calendar.start_of_day(maximum)
        {
            return false;
        }
        (self.is_enabled)(day)
    }
}

/// Date arithmetic uses calendar components, so a day is not assumed to be 24 hours.
pub struct CalendarGrid;

impl CalendarGrid {
    pub fn month_start(date: Date, calendar: &Calendar) -> Date {
        calendar.date_interval_start(CalendarUnit::Month, date).unwrap_or_else(|| calendar.start_of_day(date))
    }

    pub fn days_in_month(month: Date, calendar: &Calendar) -> Vec<Date> {
        let start = Self::month_start(month, calendar);
        let weekday = calendar.component(CalendarUnit::Weekday, start);
        let leading = (weekday - calendar.first_weekday() as i32).rem_euclid(7);
        let Some(first) = calendar.date_by_adding(CalendarUnit::Day, -leading, start) else { return Vec::new() };
        (0..42).filter_map(|offset| calendar.date_by_adding(CalendarUnit::Day, offset, first)).collect()
    }

    pub fn move_day(date: Date, days: i32, calendar: &Calendar, availability: &DateAvailability) -> Option<Date> {
        if days == 0 {
            return if availability.contains(date, calendar) { Some(date) } else { None };
        }
        let mut candidate = date;
        for _ in 0..3660 {
            let next = calendar.date_by_adding(CalendarUnit::Day, days, candidate)?;
            candidate = next;
            if let Some(maximum) = availability.maximum
                && calendar.start_of_day(candidate) > calendar.start_of_day(maximum)
                && days > 0
            {
                return None;
            }
            if let Some(minimum) = availability.minimum
                && calendar.start_of_day(candidate) < calendar.start_of_day(minimum)
                && days < 0
            {
                return None;
            }
            if availability.contains(candidate, calendar) {
                return Some(candidate);
            }
        }
        None
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
pub struct TimeOfDay {
    pub hour: i32,
    pub minute: i32,
    pub second: i32,
}

impl TimeOfDay {
    pub fn new(hour: i32, minute: i32, second: i32) -> Self {
        Self { hour: hour.clamp(0, 23), minute: minute.clamp(0, 59), second: second.clamp(0, 59) }
    }

    pub fn seconds(self) -> i32 {
        self.hour * 3600 + self.minute * 60 + self.second
    }

    pub fn formatted(self, show_seconds: bool) -> String {
        if show_seconds {
            format!("{:02}:{:02}:{:02}", self.hour, self.minute, self.second)
        } else {
            format!("{:02}:{:02}", self.hour, self.minute)
        }
    }

    pub fn parse(text: &str, show_seconds: bool) -> Option<Self> {
        let trimmed = text.trim();
        let parts: Vec<&str> = trimmed.split(':').collect();
        if parts.len() != if show_seconds { 3 } else { 2 } {
            return None;
        }
        if !parts.iter().all(|part| !part.is_empty() && part.chars().all(|character| character.is_ascii_digit())) {
            return None;
        }
        let hour: i32 = parts[0].parse().ok()?;
        let minute: i32 = parts[1].parse().ok()?;
        if !(0..=23).contains(&hour) || !(0..=59).contains(&minute) {
            return None;
        }
        let second: i32 = if show_seconds { parts[2].parse().ok()? } else { 0 };
        if !(0..=59).contains(&second) {
            return None;
        }
        Some(Self::new(hour, minute, second))
    }
}

#[derive(Deserialize)]
struct TimeOfDayWire {
    #[serde(default)]
    hour: i32,
    #[serde(default)]
    minute: i32,
    #[serde(default)]
    second: i32,
}

impl<'de> Deserialize<'de> for TimeOfDay {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let values = TimeOfDayWire::deserialize(deserializer)?;
        Ok(Self::new(values.hour, values.minute, values.second))
    }
}
