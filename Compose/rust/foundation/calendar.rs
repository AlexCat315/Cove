//! Gregorian calendar arithmetic with an explicit first weekday and time zone
//! offset. Every date operation the UI needs routes through here, so no widget
//! does its own day arithmetic.
use super::date::{
    days_in_month, is_leap_year, iso_week_of, weekday_of, Date, DateParts,
};

/// The unit a date operation is expressed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CalendarUnit {
    Year,
    Month,
    WeekOfYear,
    YearForWeekOfYear,
    Day,
    Weekday,
    Hour,
    Minute,
    Second,
}

/// A proleptic Gregorian calendar in a fixed-offset time zone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Calendar {
    /// Offset from UTC in seconds; no DST transitions.
    offset_seconds: i64,
    /// First day of the week, Sunday = 1.
    first_weekday: u32,
}

impl Default for Calendar {
    fn default() -> Self {
        Self { offset_seconds: 0, first_weekday: 2 }
    }
}

impl Calendar {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_offset_seconds(mut self, offset_seconds: i64) -> Self {
        self.offset_seconds = offset_seconds;
        self
    }

    pub fn offset_seconds(&self) -> i64 {
        self.offset_seconds
    }

    pub fn first_weekday(&self) -> u32 {
        self.first_weekday
    }

    pub fn set_first_weekday(&mut self, weekday: u32) {
        self.first_weekday = weekday.clamp(1, 7);
    }

    /// Wall-clock parts for an instant.
    pub fn parts(&self, date: Date) -> DateParts {
        let local = date.seconds() + self.offset_seconds;
        let day = local.div_euclid(Date::SECONDS_PER_DAY);
        let within = local.rem_euclid(Date::SECONDS_PER_DAY);
        let mut parts = DateParts::from_epoch_day(day);
        parts.hour = (within / 3600) as u32;
        parts.minute = ((within % 3600) / 60) as u32;
        parts.second = (within % 60) as u32;
        parts
    }

    /// The instant for wall-clock parts.
    pub fn date(&self, parts: DateParts) -> Date {
        let day = DateParts::new(parts.year, parts.month, parts.day).epoch_day();
        let within = parts.hour as i64 * 3600 + parts.minute as i64 * 60 + parts.second as i64;
        Date::from_seconds(day * Date::SECONDS_PER_DAY + within - self.offset_seconds)
    }

    pub fn start_of_day(&self, date: Date) -> Date {
        let parts = self.parts(date);
        self.date(DateParts::new(parts.year, parts.month, parts.day))
    }

    /// Midnight UTC of this instant's day, matching `start_of_day` when the
    /// calendar's offset is zero.
    pub fn start_of_day_utc(&self, date: Date) -> Date {
        Date::from_epoch_day(self.parts(date).epoch_day())
    }

    pub fn component(&self, unit: CalendarUnit, date: Date) -> i32 {
        let parts = self.parts(date);
        let day = DateParts::new(parts.year, parts.month, parts.day).epoch_day();
        match unit {
            CalendarUnit::Year => parts.year,
            CalendarUnit::Month => parts.month as i32,
            CalendarUnit::Day => parts.day as i32,
            CalendarUnit::Hour => parts.hour as i32,
            CalendarUnit::Minute => parts.minute as i32,
            CalendarUnit::Second => parts.second as i32,
            CalendarUnit::Weekday => weekday_of(day) as i32,
            CalendarUnit::WeekOfYear => iso_week_of(day).1 as i32,
            CalendarUnit::YearForWeekOfYear => iso_week_of(day).0,
        }
    }

    /// Add whole units. Month and year arithmetic clamps to the month's last day,
    /// so `Jan 31 + 1 month` is `Feb 28/29`.
    pub fn date_by_adding(&self, unit: CalendarUnit, value: i32, date: Date) -> Option<Date> {
        let parts = self.parts(date);
        let day = DateParts::new(parts.year, parts.month, parts.day).epoch_day();
        let within = parts.hour as i64 * 3600 + parts.minute as i64 * 60 + parts.second as i64;
        let result_day = match unit {
            CalendarUnit::Day => day + value as i64,
            CalendarUnit::WeekOfYear | CalendarUnit::Weekday => day + value as i64 * 7,
            CalendarUnit::Month | CalendarUnit::Year | CalendarUnit::YearForWeekOfYear => {
                let months = if matches!(unit, CalendarUnit::Year | CalendarUnit::YearForWeekOfYear) {
                    value as i64 * 12
                } else {
                    value as i64
                };
                let total = parts.year as i64 * 12 + (parts.month as i64 - 1) + months;
                let year = total.div_euclid(12) as i32;
                let month = total.rem_euclid(12) as u32 + 1;
                let bound = days_in_month(year, month);
                DateParts::new(year, month, parts.day.min(bound)).epoch_day()
            }
            CalendarUnit::Hour => day + (within + value as i64 * 3600).div_euclid(Date::SECONDS_PER_DAY),
            CalendarUnit::Minute => day + (within + value as i64 * 60).div_euclid(Date::SECONDS_PER_DAY),
            CalendarUnit::Second => day + (within + value as i64).div_euclid(Date::SECONDS_PER_DAY),
        };
        let mut result = DateParts::from_epoch_day(result_day);
        match unit {
            CalendarUnit::Hour => {
                let seconds = (within + value as i64 * 3600).rem_euclid(Date::SECONDS_PER_DAY);
                result.hour = (seconds / 3600) as u32;
                result.minute = ((seconds % 3600) / 60) as u32;
                result.second = (seconds % 60) as u32;
            }
            CalendarUnit::Minute => {
                let seconds = (within + value as i64 * 60).rem_euclid(Date::SECONDS_PER_DAY);
                result.hour = (seconds / 3600) as u32;
                result.minute = ((seconds % 3600) / 60) as u32;
                result.second = (seconds % 60) as u32;
            }
            CalendarUnit::Second => {
                let seconds = (within + value as i64).rem_euclid(Date::SECONDS_PER_DAY);
                result.hour = (seconds / 3600) as u32;
                result.minute = ((seconds % 3600) / 60) as u32;
                result.second = (seconds % 60) as u32;
            }
            _ => {
                result.hour = parts.hour;
                result.minute = parts.minute;
                result.second = parts.second;
            }
        }
        Some(self.date(result))
    }

    /// Start of the unit containing `date`.
    pub fn date_interval_start(&self, unit: CalendarUnit, date: Date) -> Option<Date> {
        let parts = self.parts(date);
        match unit {
            CalendarUnit::Day => Some(self.date(DateParts::new(parts.year, parts.month, parts.day))),
            CalendarUnit::Month => Some(self.date(DateParts::new(parts.year, parts.month, 1))),
            CalendarUnit::Year => Some(self.date(DateParts::new(parts.year, 1, 1))),
            CalendarUnit::WeekOfYear | CalendarUnit::Weekday => {
                let day = DateParts::new(parts.year, parts.month, parts.day).epoch_day();
                let weekday = weekday_of(day) as i64;
                let leading = (weekday - self.first_weekday as i64).rem_euclid(7);
                Some(Date::from_epoch_day(day - leading))
            }
            _ => None,
        }
    }

    /// End of the unit containing `date` (start of the next unit).
    pub fn date_interval_end(&self, unit: CalendarUnit, date: Date) -> Option<Date> {
        let start = self.date_interval_start(unit, date)?;
        self.date_by_adding(unit, 1, start)
    }

    /// Days in the month containing `date`.
    pub fn days_in_month(&self, date: Date) -> u32 {
        let parts = self.parts(date);
        days_in_month(parts.year, parts.month)
    }

    pub fn is_leap_year(&self, date: Date) -> bool {
        is_leap_year(self.parts(date).year)
    }
}
