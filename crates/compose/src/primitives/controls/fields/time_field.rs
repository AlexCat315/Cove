//! Translation of Primitives/Controls/Fields/TimeField.swift.
//!
//! An editable draft commits on Return or blur. Invalid text never overwrites
//! the bound time: the field keeps the draft and shows the accepted range.
use crate::Scope;
use std::rc::Rc;
use std::sync::{Arc, Mutex, MutexGuard};

use cove_core::Binding;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};

use crate::modifiers::layout::LayoutFrameExt;
use crate::modifiers::style::SemanticStyleViewExt;
use crate::primitives::controls::date_values::{DEFAULT_MINUTE_STEP, TimeOfDay};
use crate::primitives::controls::time_picker::TimeHourCycle;
use crate::primitives::text::Text;
use crate::primitives::text::text_buffer::TextBuffer;
use crate::primitives::text::text_field::{TextField, TextFieldEvents};
use crate::theme::SemanticFontRef;
use crate::{AnyView, BoxView, TupleView, View};

/// Authored time-field settings.
#[derive(Clone, Debug, PartialEq)]
pub struct TimeFieldOptions {
    pub show_seconds: bool,
    pub is_enabled: bool,
    pub minute_step: i32,
    pub minimum: TimeOfDay,
    pub maximum: TimeOfDay,
    /// Whether the field reads and writes a 12-hour clock with a half-day
    /// marker. The stored value is always a 24-hour [`TimeOfDay`].
    pub hour_cycle: TimeHourCycle,
    /// Half-day labels as `(am, pm)`.
    ///
    /// `None` uses `("AM", "PM")`; a Chinese interface sets `("上午", "下午")`.
    pub period_labels: Option<(String, String)>,
}

impl Default for TimeFieldOptions {
    fn default() -> Self {
        Self {
            show_seconds: false,
            is_enabled: true,
            minute_step: DEFAULT_MINUTE_STEP,
            minimum: TimeOfDay::new(0, 0, 0),
            maximum: TimeOfDay::new(23, 59, 59),
            hour_cycle: TimeHourCycle::TwentyFour,
            period_labels: None,
        }
    }
}

impl TimeFieldOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validate(&mut self) {
        self.minute_step = self.minute_step.clamp(1, 60);
        if self.maximum < self.minimum {
            self.maximum = self.minimum;
        }
    }

    pub fn validated(mut self) -> Self {
        self.validate();
        self
    }

    /// The placeholder text for the authored precision.
    pub fn placeholder(&self) -> &'static str {
        match (self.hour_cycle, self.show_seconds) {
            (TimeHourCycle::TwentyFour, false) => "HH:MM",
            (TimeHourCycle::TwentyFour, true) => "HH:MM:SS",
            (TimeHourCycle::Twelve, false) => "hh:mm AM/PM",
            (TimeHourCycle::Twelve, true) => "hh:mm:ss AM/PM",
        }
    }

    /// The `(am, pm)` labels the field shows.
    pub fn period_labels(&self) -> (&str, &str) {
        match self.period_labels.as_ref() {
            Some((am, pm)) => (am.as_str(), pm.as_str()),
            None => ("AM", "PM"),
        }
    }

    /// `time` rendered for the authored cycle and precision.
    pub fn format(&self, time: TimeOfDay) -> String {
        if self.hour_cycle == TimeHourCycle::TwentyFour {
            return time.formatted(self.show_seconds);
        }
        let (am, pm) = self.period_labels();
        let hour = if time.hour % 12 == 0 {
            12
        } else {
            time.hour % 12
        };
        let marker = if time.hour < 12 { am } else { pm };
        if self.show_seconds {
            format!("{hour:02}:{:02}:{:02} {marker}", time.minute, time.second)
        } else {
            format!("{hour:02}:{:02} {marker}", time.minute)
        }
    }

    /// Parses `text` for the authored cycle and precision.
    ///
    /// A 12-hour field accepts `hh:mm[:ss] AM/PM` with either the authored
    /// labels or the English markers. A bare `HH:MM[:SS]` is read as a 24-hour
    /// time in both cycles, so pasting `21:30` never lands on the wrong half of
    /// the day.
    pub fn parse(&self, text: &str) -> Option<TimeOfDay> {
        if self.hour_cycle == TimeHourCycle::TwentyFour {
            return TimeOfDay::parse(text, self.show_seconds);
        }
        let (am, pm) = self.period_labels();
        let trimmed = text.trim();
        let (body, is_pm) = if let Some(body) = strip_period(trimmed, pm) {
            (body, Some(true))
        } else if let Some(body) = strip_period(trimmed, am) {
            (body, Some(false))
        } else if let Some(body) = strip_period(trimmed, "PM") {
            (body, Some(true))
        } else if let Some(body) = strip_period(trimmed, "AM") {
            (body, Some(false))
        } else {
            (trimmed, None)
        };
        let time = TimeOfDay::parse(body.trim(), self.show_seconds)?;
        match is_pm {
            None => Some(time),
            Some(is_pm) => {
                if !(1..=12).contains(&time.hour) {
                    return None;
                }
                let hour = time.hour % 12 + if is_pm { 12 } else { 0 };
                Some(TimeOfDay::new(hour, time.minute, time.second))
            }
        }
    }
}

/// Splits a trailing half-day marker off `text`, ignoring ASCII case.
fn strip_period<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    if marker.is_empty() || text.len() < marker.len() {
        return None;
    }
    let split = text.len() - marker.len();
    if !text.is_char_boundary(split) {
        return None;
    }
    let (head, tail) = text.split_at(split);
    tail.eq_ignore_ascii_case(marker).then_some(head)
}

/// Edit session: the typed draft and its commit error.
#[derive(Clone, Debug, Default)]
struct TimeFieldSession {
    draft: TextBuffer,
    is_editing: bool,
    changed: bool,
    error: String,
}

/// Editable time text field.
pub struct TimeField {
    pub value: Binding<Option<TimeOfDay>>,
    pub options: TimeFieldOptions,
    session: Arc<Mutex<TimeFieldSession>>,
}

impl TimeField {
    pub fn new(value: Binding<Option<TimeOfDay>>, options: TimeFieldOptions) -> Self {
        Self {
            value,
            options: options.validated(),
            session: Arc::new(Mutex::new(TimeFieldSession::default())),
        }
    }

    fn snapshot(&self) -> TimeField {
        TimeField {
            value: self.value.clone(),
            options: self.options.clone(),
            session: self.session.clone(),
        }
    }

    fn committed(&self) -> TextBuffer {
        TextBuffer::new(
            &self
                .value
                .get()
                .map(|time| self.options.format(time))
                .unwrap_or_default(),
        )
    }

    /// Commits the draft. Empty text clears the value; text outside the
    /// authored range or unparseable stays in the field with an error.
    fn commit(&self) {
        let mut session = self
            .session
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !session.changed {
            return;
        }
        let text = session.draft.string_value();
        if text.trim().is_empty() {
            self.value.set(None);
            session.error.clear();
            session.changed = false;
            return;
        }
        let parsed = self
            .options
            .parse(&text)
            .filter(|time| *time >= self.options.minimum && *time <= self.options.maximum);
        match parsed {
            Some(time) => {
                self.value.set(Some(time));
                session.error.clear();
                session.changed = false;
            }
            None => {
                session.error = crate::i18n::format_template(
                    &crate::i18n::tr("Enter a valid time from {minimum} to {maximum}."),
                    &[
                        ("minimum", &self.options.format(self.options.minimum)),
                        ("maximum", &self.options.format(self.options.maximum)),
                    ],
                );
            }
        }
    }
}

impl View for TimeField {
    fn body(&self, _cx: &Scope) -> AnyView {
        let field = self.snapshot();
        let session = self.session.clone();
        let committed = self.committed();

        let text = Binding::new(
            {
                let session = session.clone();
                let committed = committed.clone();
                move || {
                    if lock(&session).is_editing {
                        lock(&session).draft.clone()
                    } else {
                        committed.clone()
                    }
                }
            },
            {
                let session = session.clone();
                move |buffer: TextBuffer| {
                    let mut session = lock(&session);
                    session.draft = buffer;
                    session.changed = true;
                }
            },
        );

        let input = TextField::new(self.options.placeholder(), text)
            .behavior(crate::primitives::text::text_field::TextFieldBehavior {
                disabled: !self.options.is_enabled,
                ..Default::default()
            })
            .events(TextFieldEvents {
                on_submit: Some({
                    let field = field.clone();
                    Rc::new(move || field.commit())
                }),
                on_focus: Some({
                    let session = session.clone();
                    let committed = committed.clone();
                    Rc::new(move || {
                        let mut session = lock(&session);
                        if !session.is_editing {
                            session.draft = committed.clone();
                            session.is_editing = true;
                            session.changed = false;
                        }
                    })
                }),
                on_blur: Some({
                    let field = field.clone();
                    let session = session.clone();
                    Rc::new(move || {
                        field.commit();
                        lock(&session).is_editing = false;
                    })
                }),
                on_cancel: Some({
                    let session = session.clone();
                    let committed = committed.clone();
                    Rc::new(move || {
                        let mut session = lock(&session);
                        session.error.clear();
                        session.changed = false;
                        session.draft = committed.clone();
                    })
                }),
                on_key_down: Some({
                    let field = field.clone();
                    Rc::new(move |event: &cove_platform::KeyEvent| {
                        if event.scancode != crate::Scancode::ARROW_UP
                            && event.scancode != crate::Scancode::ARROW_DOWN
                        {
                            return false;
                        }
                        // Shift steps an hour; the plain key steps a minute.
                        let step = if event.modifiers.has_shift() {
                            3600
                        } else {
                            field.options.minute_step * 60
                        };
                        let delta = if event.scancode == crate::Scancode::ARROW_UP {
                            step
                        } else {
                            -step
                        };
                        let current = field.value.get().unwrap_or(field.options.minimum);
                        let seconds = (current.seconds() + delta).clamp(
                            field.options.minimum.seconds(),
                            field.options.maximum.seconds(),
                        );
                        let next =
                            TimeOfDay::new(seconds / 3600, (seconds / 60) % 60, seconds % 60);
                        field.value.set(Some(next));
                        let mut session = field
                            .session
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        session.draft = TextBuffer::new(&field.options.format(next));
                        session.changed = false;
                        session.error.clear();
                        true
                    })
                }),
                ..Default::default()
            });

        let error = self
            .session
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .error
            .clone();
        let label = AnyView::new(TimeFieldLabel {
            input: AnyView::new(input),
        });
        if error.is_empty() {
            return label;
        }
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                4.0,
                AnyView::new(TupleView::new(vec![
                    label,
                    AnyView::new(
                        Text::new(error)
                            .semantic_font(SemanticFontRef::caption())
                            .semantic_foreground(crate::theme::SemanticColorRef::error()),
                    ),
                ])),
            )
            .frame_min_width(0.0),
        )
    }
}

/// Applies the field's accessible name.
struct TimeFieldLabel {
    input: AnyView,
}

impl crate::PrimitiveView for TimeFieldLabel {
    fn make_node(&self) -> cove_core::scene::Node {
        let node = cove_core::scene::Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, node: &cove_core::scene::Node) {
        node.set_accessibility(Some(cove_core::scene::AccessibilitySemantics {
            role: cove_core::scene::AccessibilityRole::Group,
            label: crate::i18n::tr("Time"),
            ..Default::default()
        }));
    }

    fn children(&self, _node: &cove_core::scene::Node) -> Vec<AnyView> {
        vec![self.input.clone()]
    }
}
crate::impl_primitive_view!(TimeFieldLabel);

impl TimeField {
    /// The error text currently shown under the field.
    pub fn error(&self) -> String {
        self.session
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .error
            .clone()
    }
}

impl Clone for TimeField {
    fn clone(&self) -> Self {
        self.snapshot()
    }
}

fn lock(session: &Arc<Mutex<TimeFieldSession>>) -> MutexGuard<'_, TimeFieldSession> {
    session
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod checks {
    use super::*;

    fn twelve_hour() -> TimeFieldOptions {
        TimeFieldOptions {
            hour_cycle: TimeHourCycle::Twelve,
            ..TimeFieldOptions::default()
        }
    }

    #[test]
    fn the_default_step_matches_the_picker() {
        assert_eq!(TimeFieldOptions::new().minute_step, DEFAULT_MINUTE_STEP);
        assert_eq!(
            TimeFieldOptions::new().minute_step,
            crate::primitives::controls::time_picker::TimePickerOptions::new().minute_step,
            "both time editors step by the same number of minutes"
        );
    }

    #[test]
    fn a_twenty_four_hour_field_formats_and_parses_plain_text() {
        let options = TimeFieldOptions::new();
        assert_eq!(options.format(TimeOfDay::new(21, 30, 0)), "21:30");
        assert_eq!(options.parse("21:30"), Some(TimeOfDay::new(21, 30, 0)));
        assert_eq!(options.placeholder(), "HH:MM");
    }

    #[test]
    fn a_twelve_hour_field_formats_with_the_marker() {
        let options = twelve_hour();
        assert_eq!(options.format(TimeOfDay::new(21, 30, 0)), "09:30 PM");
        assert_eq!(options.format(TimeOfDay::new(0, 5, 0)), "12:05 AM");
        assert_eq!(options.format(TimeOfDay::new(12, 0, 0)), "12:00 PM");
        assert_eq!(options.placeholder(), "hh:mm AM/PM");
    }

    #[test]
    fn a_twelve_hour_field_accepts_localised_markers() {
        let options = TimeFieldOptions {
            period_labels: Some(("上午".to_string(), "下午".to_string())),
            ..twelve_hour()
        };
        assert_eq!(options.format(TimeOfDay::new(21, 30, 0)), "09:30 下午");
        assert_eq!(options.parse("09:30 下午"), Some(TimeOfDay::new(21, 30, 0)));
        assert_eq!(options.parse("09:30 上午"), Some(TimeOfDay::new(9, 30, 0)));
    }

    #[test]
    fn a_twelve_hour_field_parses_the_english_markers_too() {
        let options = twelve_hour();
        assert_eq!(options.parse("09:30 PM"), Some(TimeOfDay::new(21, 30, 0)));
        assert_eq!(options.parse("09:30 am"), Some(TimeOfDay::new(9, 30, 0)));
        assert_eq!(options.parse("12:00 AM"), Some(TimeOfDay::new(0, 0, 0)));
        assert_eq!(options.parse("12:00 PM"), Some(TimeOfDay::new(12, 0, 0)));
    }

    #[test]
    fn a_bare_time_stays_on_the_twenty_four_hour_clock() {
        let options = twelve_hour();
        assert_eq!(
            options.parse("21:30"),
            Some(TimeOfDay::new(21, 30, 0)),
            "pasting 21:30 must not wrap to 09:30"
        );
    }

    #[test]
    fn an_hour_outside_the_twelve_hour_clock_is_rejected() {
        let options = twelve_hour();
        assert_eq!(options.parse("13:30 PM"), None);
        assert_eq!(options.parse("00:30 AM"), None);
        assert_eq!(options.parse("09:30 XM"), None);
    }
}
