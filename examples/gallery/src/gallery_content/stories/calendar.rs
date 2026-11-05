//! Calendar stories: day selection plus week, month and year granularity.
//!
//! Swift source: `GalleryContent/Stories/Data/DateTableStories.swift`
//! (`CalendarStory`).

use cove_composition::Scope;
use cove_core::Signal;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_ui_compose::foundation::{Date, weekday_of};
use cove_ui_compose::modifiers::layout::FlexExt;
use cove_ui_compose::primitives::controls::calendar::{
    CalendarGranularity, CalendarOptions, CalendarPresentation, CalendarView,
};
use cove_ui_compose::primitives::controls::date_values::DateAvailability;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::{SemanticColorRef, SemanticStyleViewExt};
use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView, View};
use std::rc::Rc;

use crate::gallery_content::catalog::GalleryOptions;
use crate::gallery_content::stories::StorySection;

/// Saturday and Sunday, keyed by `weekday_of`'s Sunday = 1 convention.
pub fn is_weekend(date: Date) -> bool {
    matches!(weekday_of(date.epoch_day()), 1 | 7)
}

/// A calendar configured the way the gallery does: enabled per the toolbar,
/// Monday-first, and weekends unavailable.
pub fn gallery_calendar(is_enabled: bool, granularity: CalendarGranularity) -> CalendarOptions {
    let mut options = CalendarOptions::default();
    options.presentation = CalendarPresentation {
        is_enabled,
        ..CalendarPresentation::default()
    };
    options.calendar.set_first_weekday(2);
    options.granularity = granularity;
    options.availability = DateAvailability {
        minimum: None,
        maximum: None,
        // The Swift story enables a date when it is NOT a weekend.
        is_enabled: Rc::new(|date: Date| !is_weekend(date)),
    };
    options
}

/// The caption shown under a granularity calendar.
pub fn granularity_line(
    title: &str,
    granularity: CalendarGranularity,
    value: Option<Date>,
) -> String {
    let label = match value {
        Some(date) => granularity_label(granularity, date),
        None => "None".to_string(),
    };
    format!("{title}: {label}")
}

/// `CalendarGranularity.galleryLabel(_:)`: the selected period, rendered short.
pub fn granularity_label(granularity: CalendarGranularity, date: Date) -> String {
    let parts = cove_ui_compose::foundation::Calendar::new().parts(date);
    match granularity {
        CalendarGranularity::Day => {
            format!("{:04}-{:02}-{:02}", parts.year, parts.month, parts.day)
        }
        CalendarGranularity::Week => {
            let (year, week) = cove_ui_compose::foundation::iso_week_of(date.epoch_day());
            format!("{year:04}-W{week:02}")
        }
        CalendarGranularity::Month => format!("{:04}-{:02}", parts.year, parts.month),
        CalendarGranularity::Year => format!("{:04}", parts.year),
    }
}

/// `CalendarStory`.
pub struct CalendarStory {
    pub options: GalleryOptions,
}

impl CalendarStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self { options }
    }

    /// `No date selected`, or the long-form date.
    pub fn selected_line(value: Option<Date>) -> String {
        match value {
            Some(date) => format_date_long(date),
            None => "No date selected".into(),
        }
    }
}

/// `date.formatted(date: .long, time: .omitted)` in a fixed, locale-free form.
pub fn format_date_long(date: Date) -> String {
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let parts = cove_ui_compose::foundation::Calendar::new().parts(date);
    let month = MONTHS
        .get((parts.month as usize).saturating_sub(1))
        .copied()
        .unwrap_or("");
    format!("{month} {}, {}", parts.day, parts.year)
}

struct GranularityCalendar {
    label: &'static str,
    granularity: CalendarGranularity,
    options: GalleryOptions,
    value: Signal<Option<Date>>,
}

impl View for GranularityCalendar {
    fn body(&self, _cx: &Scope) -> AnyView {
        let calendar = CalendarView::new(self.value.binding()).configure(|options| {
            *options = gallery_calendar(self.options.is_enabled, self.granularity)
        });
        let caption = {
            let value = self.value;
            let granularity = self.granularity;
            let label = self.label;
            move || granularity_line(label, granularity, value.get())
        };
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::FlexStart,
                Justify::FlexStart,
                Wrap::NoWrap,
                6.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(calendar),
                    AnyView::new(
                        Text::new(caption())
                            .semantic_foreground(SemanticColorRef::on_surface_muted()),
                    ),
                ])),
            )
            .debug_name(&format!("calendar-{}", self.label.to_lowercase())),
        )
    }
}

impl View for CalendarStory {
    fn body(&self, cx: &Scope) -> AnyView {
        let date = cx.state(|| None::<Date>);
        let week = cx.state(|| None::<Date>);
        let month = cx.state(|| None::<Date>);
        let year = cx.state(|| None::<Date>);
        let day_calendar = AnyView::new(CalendarView::new(date.binding()).configure(|options| {
            *options = gallery_calendar(self.options.is_enabled, CalendarGranularity::Day)
        }));
        let selected = {
            AnyView::new(
                Text::new(Self::selected_line(date.get()))
                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
            )
        };

        let granularities = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::FlexStart,
            Justify::FlexStart,
            Wrap::Wrap,
            16.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(GranularityCalendar {
                    label: "Week",
                    granularity: CalendarGranularity::Week,
                    options: self.options.clone(),
                    value: week,
                }),
                AnyView::new(GranularityCalendar {
                    label: "Month",
                    granularity: CalendarGranularity::Month,
                    options: self.options.clone(),
                    value: month,
                }),
                AnyView::new(GranularityCalendar {
                    label: "Year",
                    granularity: CalendarGranularity::Year,
                    options: self.options.clone(),
                    value: year,
                }),
            ])),
        ));

        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                24.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(StorySection::new(
                        "Choose a day",
                        "Move with arrows; Home/End focus the first/last available day of the month. PageUp/PageDown change months. Return selects the focused day. Weekend dates are unavailable. Click the month title to jump by month or year.",
                        AnyView::new(BoxView::new(
                            FlexDirection::Column,
                            Align::Stretch,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            8.0,
                            AnyView::new(TupleView::new(vec![day_calendar, selected])),
                        )),
                    )),
                    AnyView::new(StorySection::new(
                        "Week, month and year",
                        "Granularity sets what one selection covers; the bound date is the period's first day. Week rows select as a whole and show week numbers. Month and year grids use arrows by cell and row, PageUp/PageDown by page.",
                        granularities,
                    )),
                ])),
            )
            .flex(0.0)
            .debug_name("calendar-story"),
        )
    }
}

#[cfg(test)]
mod checks {
    use super::*;

    /// `Date` stores epoch *seconds*, so a civil day is converted explicitly.
    fn date(epoch_day: i64) -> Date {
        Date::from_seconds(epoch_day * Date::SECONDS_PER_DAY)
    }

    /// Regression test for `weekday_of` being off by one: 1970-01-01 must be
    /// weekday 5 (Thursday) in the Sunday=1 scheme, so 2024-01-01 is a Monday.
    #[test]
    fn weekends_are_saturday_and_sunday() {
        // 2024-01-01 is a Monday (epoch day 19723).
        let monday = date(19_723);
        assert!(!is_weekend(monday));
        assert!(is_weekend(date(19_723 + 5)), "Saturday");
        assert!(is_weekend(date(19_723 + 6)), "Sunday");
        assert!(!is_weekend(date(19_723 + 7)), "the next Monday");
    }

    #[test]
    fn gallery_calendar_is_monday_first_and_blocks_weekends() {
        let options = gallery_calendar(true, CalendarGranularity::Day);
        assert_eq!(options.calendar.first_weekday(), 2, "Monday-first");
        assert!(options.presentation.is_enabled);
        assert_eq!(options.granularity, CalendarGranularity::Day);
        let monday = date(19_723);
        assert!(options.availability.contains(monday, &options.calendar));
        assert!(
            !options
                .availability
                .contains(date(19_723 + 5), &options.calendar)
        );
    }

    #[test]
    fn granularity_labels_describe_the_period() {
        let monday = date(19_723);
        assert_eq!(
            granularity_label(CalendarGranularity::Day, monday),
            "2024-01-01"
        );
        assert_eq!(
            granularity_label(CalendarGranularity::Month, monday),
            "2024-01"
        );
        assert_eq!(granularity_label(CalendarGranularity::Year, monday), "2024");
        assert!(granularity_label(CalendarGranularity::Week, monday).starts_with("2024-W"));
    }

    #[test]
    fn captions_show_none_before_a_selection() {
        assert_eq!(
            granularity_line("Week", CalendarGranularity::Week, None),
            "Week: None"
        );
        assert_eq!(
            granularity_line("Month", CalendarGranularity::Month, Some(date(19_723))),
            "Month: 2024-01"
        );
    }

    #[test]
    fn long_date_format_matches_swift_long_style() {
        assert_eq!(format_date_long(date(19_723)), "January 1, 2024");
        assert_eq!(CalendarStory::selected_line(None), "No date selected");
        assert_eq!(
            CalendarStory::selected_line(Some(date(19_723))),
            "January 1, 2024"
        );
    }
}
