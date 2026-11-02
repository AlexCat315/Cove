//! Controls story: inline time wheels.
//!
//! Swift source: `GalleryContent/Stories/Controls/TimePickerStory.swift`.
//!
//! The story's own display formatter is the part worth pinning: a 12-hour clock
//! shows 12 for midnight and noon, and with seconds enabled the field order is
//! `HH:MM:SS` followed by the half-day marker.

use cove_composition::Scope;
use cove_core::State;
use cove_ui_compose::modifiers::layout::FlexExt;
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::controls::Checkbox;
use cove_ui_compose::primitives::controls::date_values::TimeOfDay;
use cove_ui_compose::primitives::controls::{TimeHourCycle, TimePicker, TimePickerOptions};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::SemanticColorRef;
use cove_ui_compose::{AnyView, BoxView, TupleView, View};

use super::feedback::StorySection;
use crate::gallery_content::catalog::GalleryOptions;

/// The time the story opens with.
pub const DEFAULT_HOUR: i32 = 9;
pub const DEFAULT_MINUTE: i32 = 30;

/// The current wall-clock time.
///
/// The gallery has no local-timezone source, so this reads UTC from the system
/// clock rather than pretending to know the offset.
pub fn now() -> Option<TimeOfDay> {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs()
        % 86_400;
    Some(TimeOfDay::new(
        (seconds / 3_600) as i32,
        ((seconds % 3_600) / 60) as i32,
        (seconds % 60) as i32,
    ))
}

/// Formats a time the way the story's badge shows it.
///
/// A 12-hour clock maps 0 and 12 to `12` and appends `AM`/`PM`; a 24-hour clock
/// uses zero-padded hours and no marker.
pub fn display_text(time: Option<TimeOfDay>, twelve_hour: bool, shows_seconds: bool) -> String {
    let Some(time) = time else {
        return "No time".to_owned();
    };
    if twelve_hour {
        let hour = if time.hour % 12 == 0 {
            12
        } else {
            time.hour % 12
        };
        let suffix = if time.hour < 12 { "AM" } else { "PM" };
        if shows_seconds {
            format!("{hour:02}:{:02}:{:02} {suffix}", time.minute, time.second)
        } else {
            format!("{hour:02}:{:02} {suffix}", time.minute)
        }
    } else {
        time.formatted(shows_seconds)
    }
}

/// `TimePickerStory`: the inline wheel form and its display badge.
pub struct TimePickerStory {
    pub options: GalleryOptions,
    time: State<Option<TimeOfDay>>,
    twelve_hour: State<bool>,
    shows_seconds: State<bool>,
}

impl TimePickerStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            time: State::new(Some(TimeOfDay::new(DEFAULT_HOUR, DEFAULT_MINUTE, 0))),
            twelve_hour: State::new(true),
            shows_seconds: State::new(false),
        }
    }

    pub fn time(&self) -> Option<TimeOfDay> {
        self.time.get()
    }

    pub fn set_time(&self, time: Option<TimeOfDay>) {
        self.time.set(time);
    }

    /// Clears the selection, which the badge reads as "No time".
    pub fn clear(&self) {
        self.time.set(None);
    }

    pub fn twelve_hour(&self) -> bool {
        self.twelve_hour.get()
    }

    pub fn set_twelve_hour(&self, value: bool) {
        self.twelve_hour.set(value);
    }

    pub fn shows_seconds(&self) -> bool {
        self.shows_seconds.get()
    }

    pub fn set_shows_seconds(&self, value: bool) {
        self.shows_seconds.set(value);
    }

    /// The badge text for the current state.
    pub fn display(&self) -> String {
        display_text(
            self.time.get(),
            self.twelve_hour.get(),
            self.shows_seconds.get(),
        )
    }
}

impl View for TimePickerStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let enabled = self.options.is_enabled;
        let state = self.time.clone();

        let label = Text::new("Start time")
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::body_strong());
        let badge = Text::new(self.display())
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::info());

        let twelve_label = Text::new("12-hour")
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted());
        let seconds_label = Text::new("Seconds")
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted());

        let controls = AnyView::new(BoxView::new(
            cove_core::scene::FlexDirection::Row,
            cove_core::scene::Align::Center,
            cove_core::scene::Justify::FlexStart,
            cove_core::scene::Wrap::Wrap,
            12.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(
                    cove_ui_compose::primitives::controls::Button::title("Now", move || {
                        state.set(now())
                    })
                    .enabled(enabled),
                ),
                AnyView::new(
                    cove_ui_compose::primitives::controls::Button::title("Clear", {
                        let state = self.time.clone();
                        move || state.set(None)
                    })
                    .enabled(enabled),
                ),
                AnyView::new(Checkbox::from_boolean(self.twelve_hour.binding())),
                AnyView::new(twelve_label),
                AnyView::new(Checkbox::from_boolean(self.shows_seconds.binding())),
                AnyView::new(seconds_label),
            ])),
        ));

        let mut picker_options = TimePickerOptions::new();
        picker_options.hour_cycle = if self.twelve_hour.get() {
            TimeHourCycle::Twelve
        } else {
            TimeHourCycle::TwentyFour
        };
        picker_options.shows_seconds = self.shows_seconds.get();
        picker_options.is_enabled = enabled;
        picker_options.label = "Start time".to_owned();
        let picker = AnyView::new(TimePicker::new(self.time.binding(), picker_options));

        let header = AnyView::new(BoxView::new(
            cove_core::scene::FlexDirection::Row,
            cove_core::scene::Align::Center,
            cove_core::scene::Justify::FlexStart,
            cove_core::scene::Wrap::NoWrap,
            8.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(label),
                AnyView::new(AnyView::new(Text::new("")).flex(1.0)),
                AnyView::new(badge),
            ])),
        ));

        AnyView::new(StorySection::new(
            "Inline time wheels",
            "The form owns the surface; the picker owns hour, minute, optional seconds and AM/PM. Every wheel is independently keyboard and accessibility adjustable.",
            AnyView::new(TupleView::new(vec![picker, controls, header])),
        ))
    }

    fn tag(&self) -> &'static str {
        "TimePickerStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; time, twelve_hour, shows_seconds)
    }
}
