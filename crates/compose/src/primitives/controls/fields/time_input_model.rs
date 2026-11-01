//! Translation of `Primitives/Controls/Fields/TimeInput.swift` (the model half).
//!
//! A segmented time field, like AppKit's date picker: hours, minutes and (in a
//! 12-hour cycle) AM/PM each take focus. Up/Down step the focused segment,
//! digits set it and advance to the next, Left/Right move between segments, and
//! the first letter of AM/PM chooses the half of the day.
//!
//! Behaviour checklist:
//! - a 12-hour cycle shows 12 for midnight and noon, and formats hours without a
//!   leading zero; a 24-hour cycle always uses two digits
//! - stepping the hour in a 12-hour cycle stays within the same half of the day
//! - stepping wraps at both ends (minute past 59 rolls into the hour)
//! - typing a digit sets the segment, and a digit that cannot start a valid
//!   value does not advance focus
//! - two digits form the value when it is still in range, else the buffer resets
//! - the period letter only toggles when it differs from the current half
//! - losing focus resets that segment's digit buffer

use crate::primitives::controls::date_values::TimeOfDay;

/// The hour cycle a time input displays.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TimeInputHourCycle {
    Twelve,
    #[default]
    TwentyFour,
}

/// The focusable segments of the field, in visual order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeSegment {
    Hour,
    Minute,
    Period,
}

impl TimeSegment {
    /// The segment after this one, honouring the hour cycle.
    pub fn next(self, cycle: TimeInputHourCycle) -> TimeSegment {
        match (self, cycle) {
            (Self::Hour, _) => Self::Minute,
            (Self::Minute, TimeInputHourCycle::Twelve) => Self::Period,
            (Self::Minute, TimeInputHourCycle::TwentyFour) => Self::Hour,
            (Self::Period, _) => Self::Hour,
        }
    }

    pub fn previous(self, cycle: TimeInputHourCycle) -> TimeSegment {
        match (self, cycle) {
            (Self::Hour, TimeInputHourCycle::Twelve) => Self::Period,
            (Self::Hour, TimeInputHourCycle::TwentyFour) => Self::Minute,
            (Self::Minute, _) => Self::Hour,
            (Self::Period, _) => Self::Minute,
        }
    }
}

/// Editable state of one time input.
#[derive(Clone, Debug, Default)]
pub struct TimeInputState {
    pub value: Option<TimeOfDay>,
    pub hour_buffer: String,
    pub minute_buffer: String,
}

impl TimeInputState {
    pub fn new(value: Option<TimeOfDay>) -> Self {
        Self {
            value,
            hour_buffer: String::new(),
            minute_buffer: String::new(),
        }
    }

    /// The displayed time; a default of 00:00 when nothing is selected.
    pub fn time(&self) -> TimeOfDay {
        self.value.unwrap_or_else(|| TimeOfDay::new(0, 0, 0))
    }

    pub fn hour_text(&self, cycle: TimeInputHourCycle) -> String {
        match cycle {
            // 0 and 12 both read as 12, and a 12-hour clock omits the leading zero.
            TimeInputHourCycle::Twelve => format!("{}", (self.time().hour + 11) % 12 + 1),
            TimeInputHourCycle::TwentyFour => format!("{:02}", self.time().hour),
        }
    }

    pub fn minute_text(&self) -> String {
        format!("{:02}", self.time().minute)
    }

    pub fn period_text(&self) -> &'static str {
        if self.time().hour < 12 { "AM" } else { "PM" }
    }

    /// Writes the time, wrapping both fields and only reporting a change.
    pub fn set(&mut self, hour: i32, minute: i32) -> bool {
        let next = TimeOfDay::new(
            hour.rem_euclid(24),
            minute.rem_euclid(60),
            self.time().second,
        );
        if Some(next) == self.value {
            return false;
        }
        self.value = Some(next);
        true
    }

    /// Steps the hour within the current half of the day for a 12-hour cycle, so
    /// stepping 11 → 12 does not flip AM/PM.
    pub fn step_hour(&mut self, delta: i32, cycle: TimeInputHourCycle) {
        let time = self.time();
        match cycle {
            TimeInputHourCycle::Twelve => {
                let half = time.hour / 12 * 12;
                let within = (time.hour % 12 + delta).rem_euclid(12);
                self.set(half + if within == 0 { 12 } else { within }, time.minute);
            }
            TimeInputHourCycle::TwentyFour => {
                self.set(time.hour + delta, time.minute);
            }
        }
    }

    pub fn step_minute(&mut self, delta: i32) {
        let time = self.time();
        self.set(time.hour, time.minute + delta);
    }

    /// Flips AM and PM by moving twelve hours.
    pub fn toggle_period(&mut self) {
        let time = self.time();
        self.set(time.hour + 12, time.minute);
    }

    /// Types a digit into the hour segment. Returns whether focus should advance
    /// to the minutes.
    pub fn type_hour(&mut self, digit: i32, cycle: TimeInputHourCycle) -> bool {
        let top = match cycle {
            TimeInputHourCycle::Twelve => 13,
            TimeInputHourCycle::TwentyFour => 24,
        };
        let time = self.time();
        if self.hour_buffer.is_empty() {
            self.hour_buffer = digit.to_string();
            match cycle {
                TimeInputHourCycle::Twelve => {
                    // A leading zero waits for the second digit.
                    if digit == 0 {
                        return false;
                    }
                    self.set(time.hour / 12 * 12 + digit % 12, time.minute);
                }
                TimeInputHourCycle::TwentyFour => {
                    self.set(digit, time.minute);
                }
            }
            // A digit that can only be a first digit keeps focus for the second.
            return digit * 10 >= top;
        }
        if let Ok(first) = self.hour_buffer.parse::<i32>() {
            let combined = first * 10 + digit;
            if combined < top {
                match cycle {
                    TimeInputHourCycle::Twelve => {
                        let hour = if combined == 0 { 12 } else { combined % 12 };
                        self.set(time.hour / 12 * 12 + hour, time.minute);
                    }
                    TimeInputHourCycle::TwentyFour => {
                        self.set(combined, time.minute);
                    }
                }
            }
        }
        self.hour_buffer.clear();
        true
    }

    /// Types a digit into the minute segment. Returns whether the segment is
    /// complete.
    pub fn type_minute(&mut self, digit: i32) -> bool {
        let time = self.time();
        if self.minute_buffer.is_empty() {
            self.minute_buffer = digit.to_string();
            self.set(time.hour, digit);
            return digit * 10 >= 60;
        }
        if let Ok(first) = self.minute_buffer.parse::<i32>() {
            let combined = first * 10 + digit;
            if combined < 60 {
                self.set(time.hour, combined);
            }
        }
        self.minute_buffer.clear();
        true
    }

    /// The first letter of AM/PM chooses the half of the day.
    pub fn type_period(&mut self, wants_pm: bool) -> bool {
        let is_pm = self.time().hour >= 12;
        if wants_pm != is_pm {
            self.toggle_period();
            return true;
        }
        false
    }

    /// Losing focus resets that segment's digit buffer.
    pub fn blur(&mut self, segment: TimeSegment) {
        match segment {
            TimeSegment::Hour => self.hour_buffer.clear(),
            TimeSegment::Minute => self.minute_buffer.clear(),
            TimeSegment::Period => {}
        }
    }
}
