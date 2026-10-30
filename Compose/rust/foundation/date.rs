//! A civil date/time value independent of platform `Date` types.
//!
//! `Date` is epoch seconds in UTC. All calendar arithmetic goes through
//! `Calendar`, which applies a fixed-offset time zone and the proleptic Gregorian
//! rules; days are never assumed to be 24 hours in the arithmetic itself, only in
//! the offset conversion.
use std::cmp::Ordering;

/// A point in time, seconds since the Unix epoch (UTC).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Date {
    /// Whole seconds since 1970-01-01T00:00:00Z.
    seconds: i64,
}

/// Wall-clock parts in a calendar's time zone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DateParts {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

impl Date {
    pub const SECONDS_PER_DAY: i64 = 86_400;

    pub fn from_seconds(seconds: i64) -> Self {
        Self { seconds }
    }

    pub fn seconds(self) -> i64 {
        self.seconds
    }

    pub fn adding_seconds(self, seconds: i64) -> Self {
        Self { seconds: self.seconds.saturating_add(seconds) }
    }

    pub fn seconds_since(self, other: Self) -> i64 {
        self.seconds - other.seconds
    }

    /// Midnight UTC of this instant's day, ignoring any time zone.
    pub fn floor_day(self) -> Self {
        Self { seconds: self.seconds.div_euclid(Self::SECONDS_PER_DAY) * Self::SECONDS_PER_DAY }
    }

    /// Whole days between two instants' UTC days.
    pub fn days_between(self, other: Self) -> i64 {
        self.floor_day().seconds_since(other.floor_day()) / Self::SECONDS_PER_DAY
    }

    /// Days since 1970-01-01 for the UTC day.
    pub fn epoch_day(self) -> i64 {
        self.seconds.div_euclid(Self::SECONDS_PER_DAY)
    }

    pub fn from_epoch_day(day: i64) -> Self {
        Self { seconds: day * Self::SECONDS_PER_DAY }
    }
}

impl PartialOrd for DateParts {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DateParts {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.year, self.month, self.day, self.hour, self.minute, self.second)
            .cmp(&(other.year, other.month, other.day, other.hour, other.minute, other.second))
    }
}

impl DateParts {
    pub fn new(year: i32, month: u32, day: u32) -> Self {
        Self { year, month, day, hour: 0, minute: 0, second: 0 }
    }

    /// Days since 1970-01-01 for this civil date (Howard Hinnant's algorithm).
    pub fn epoch_day(self) -> i64 {
        let year = self.year as i64;
        let month = self.month as i64;
        let day = self.day as i64;
        let year = if month <= 2 { year - 1 } else { year };
        let era = if year >= 0 { year } else { year - 399 } / 400;
        let year_of_era = year - era * 400;
        let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
        let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
        era * 146_097 + day_of_era - 719_468
    }

    /// Civil date for a day count since 1970-01-01.
    pub fn from_epoch_day(day: i64) -> Self {
        let day = day + 719_468;
        let era = if day >= 0 { day } else { day - 146_096 } / 146_097;
        let day_of_era = day - era * 146_097;
        let year_of_era =
            (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let year = year_of_era + era * 400;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let month_prime = (5 * day_of_year + 2) / 153;
        let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
        let month = month_prime + if month_prime < 10 { 3 } else { -9 };
        Self {
            year: (year + if month <= 2 { 1 } else { 0 }) as i32,
            month: month as u32,
            day: day as u32,
            hour: 0,
            minute: 0,
            second: 0,
        }
    }
}

/// Weekday, Sunday = 1.
pub fn weekday_of(epoch_day: i64) -> u32 {
    // 1970-01-01 was a Thursday (5).
    (epoch_day.rem_euclid(7) + 5).rem_euclid(7) as u32 + 1
}

/// ISO weekday, Monday = 1.
pub fn iso_weekday_of(epoch_day: i64) -> u32 {
    let sunday_based = weekday_of(epoch_day);
    if sunday_based == 1 { 7 } else { sunday_based - 1 }
}

pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 30,
    }
}

/// ISO-8601 week and week-year for a day count.
pub fn iso_week_of(epoch_day: i64) -> (i32, u32) {
    let weekday = iso_weekday_of(epoch_day) as i64;
    // The Thursday of this week decides the week-year.
    let thursday = epoch_day - (weekday - 4);
    let parts = DateParts::from_epoch_day(thursday);
    let jan1 = DateParts::new(parts.year, 1, 1).epoch_day();
    let week = (thursday - jan1).div_euclid(7) as u32 + 1;
    (parts.year, week)
}
