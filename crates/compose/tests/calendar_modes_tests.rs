//! Translation of `Tests/CoveUIComposeTests/Calendar/ModesTests.swift`.
//!
//! Behaviour checklist:
//! - week granularity renders six week rows as single buttons; the selected
//!   week is the one containing the selection, and choosing a week selects its
//!   first day
//! - week numbers and day numbers are centred in the row band, line up with the
//!   header columns, and today keeps the row's type size
//! - month granularity opens on a year of months, marks the selected month,
//!   disables months with no available day, and selects a month start
//! - year granularity pages twelve aligned years and selects a year start
//! - the title drills up to months and years; a pick drills back down without
//!   selecting until the base panel
//! - keys move by one cell, a row, a page, or to the page's ends in every panel
//! - period labels name the week, month or year
//!
//! Swift activated cells through `accessibilityActions.activate`; the Rust
//! calendar cells only install a pointer handler, so cells are pressed with a
//! real click through `InteractiveScene` (see GAP below).
//!
//! Named differences (GAP assertions):
//! - No accessibility activate: Swift calendar cells expose
//!   `accessibilityActions.activate`; Rust cells expose none.
//! - Week rows: Swift's week row is a laid-out button labelled "Week 11, 2024";
//!   the Rust `CalendarWeekButton` has no layout node (zero frame), is labelled
//!   "Week 11", and cannot be clicked, so activating Week 12 selects nothing
//!   (Swift: 2024-03-18). Row-band centring is checked per cell box instead.
//! - Month labels: Swift "March 2024"; Rust now matches it (the period label
//!   used to repeat the year).
//! - Day labels: Swift "Tuesday, September 15, 2026"; Rust omits the weekday.
//! - `CalendarGranularity.label(for:)` does not exist in Rust; the labels case
//!   asserts the rendered accessibility labels instead.

use crate::support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::AnyView;
use cove_core::scene::{AccessibilityRole, Node};
use cove_ui_compose::Scancode;
use cove_ui_compose::foundation::{Calendar, Date, DateParts};
use cove_ui_compose::primitives::controls::calendar::calendar::{CalendarOptions, CalendarView};
use cove_ui_compose::primitives::controls::calendar::navigation::{
    CalendarCells, CalendarNavigation,
};
use cove_ui_compose::primitives::controls::calendar::period::{CalendarGranularity, CalendarPanel};
use cove_ui_compose::primitives::controls::date_values::DateAvailability;

use support::InteractiveScene;

fn calendar() -> Calendar {
    let mut calendar = Calendar::new();
    calendar.set_first_weekday(2);
    calendar
}

fn date(year: i32, month: u32, day: u32) -> Date {
    calendar().date(DateParts::new(year, month, day))
}

fn descendants(node: &Node) -> Vec<Node> {
    let mut out = vec![node.clone()];
    for child in node.children() {
        out.extend(descendants(&child));
    }
    out
}

fn has_text(node: &Node) -> bool {
    node.layout_node()
        .is_some_and(|layout| layout.text_inputs().is_some())
}

fn text_of(node: &Node) -> String {
    node.layout_node()
        .and_then(|layout| layout.text_inputs())
        .map(|inputs| inputs.text.to_string())
        .unwrap_or_default()
}

fn buttons(scene: &InteractiveScene) -> Vec<Node> {
    scene.all(|node| {
        node.accessibility().is_some_and(|semantics| {
            semantics.role == AccessibilityRole::Button && !semantics.is_hidden
        })
    })
}

fn label(node: &Node) -> String {
    node.accessibility()
        .map(|semantics| semantics.label)
        .unwrap_or_default()
}

fn button(scene: &InteractiveScene, title: &str) -> Node {
    buttons(scene)
        .into_iter()
        .find(|node| label(node) == title)
        .unwrap_or_else(|| {
            panic!(
                "no {title:?}; buttons: {:?}",
                buttons(scene).iter().map(label).collect::<Vec<_>>()
            )
        })
}

/// A button found by the text it draws, for buttons Rust leaves unlabelled.
fn button_with_text(scene: &InteractiveScene, text: &str) -> Node {
    buttons(scene)
        .into_iter()
        .find(|node| descendants(node).iter().any(|child| text_of(child) == text))
        .unwrap_or_else(|| panic!("no button drawing {text:?}"))
}

fn press(scene: &InteractiveScene, node: &Node) {
    scene.click(node, 1);
    scene.settle(4, 1.0);
}

fn render(
    selection: Option<Date>,
    granularity: CalendarGranularity,
    availability: DateAvailability,
    test: impl FnOnce(&InteractiveScene, &Rc<RefCell<Option<Date>>>),
) {
    InteractiveScene::run(720.0, 900.0, |scene| {
        let store = Rc::new(RefCell::new(selection));
        let mut options = CalendarOptions::default();
        options.calendar = calendar();
        options.granularity = granularity;
        options.availability = availability;
        options.today = date(2024, 3, 13);
        let read = store.clone();
        let write = store.clone();
        scene.install(AnyView::new(
            CalendarView::new(cove_core::Binding::new(
                move || *read.borrow(),
                move |value| *write.borrow_mut() = value,
            ))
            .configure(|calendar| *calendar = options),
        ));
        test(scene, &store);
    });
}

#[test]
fn cells_expose_accessibility_activate() {
    render(
        Some(date(2024, 3, 13)),
        CalendarGranularity::Day,
        DateAvailability::default(),
        |scene, store| {
            let day = button(scene, "March 14, 2024");
            let activate = day
                .accessibility_actions()
                .activate
                .expect("a day cell offers an activate action");
            activate();
            assert_eq!(
                *store.borrow(),
                Some(date(2024, 3, 14)),
                "activating the cell selects its day"
            );
        },
    );
}

#[test]
fn week_selection() {
    render(
        Some(date(2024, 3, 13)),
        CalendarGranularity::Week,
        DateAvailability::default(),
        |scene, store| {
            let weeks: Vec<Node> = buttons(scene)
                .into_iter()
                .filter(|node| label(node).starts_with("Week "))
                .collect();
            assert_eq!(weeks.len(), 6, "a month shows six week rows");
            let selected = weeks
                .iter()
                .find(|node| node.accessibility().is_some_and(|s| s.state.is_selected))
                .expect("the selection's week is selected");
            assert!(
                label(selected).starts_with("Week 11, "),
                "the selected week names its number and date range, got {:?}",
                label(selected)
            );
            let next = weeks
                .iter()
                .find(|node| label(node).starts_with("Week 12, "))
                .expect("week 12 row")
                .clone();
            let frame = next.absolute_frame();
            assert!(
                frame.width > 0.0 && frame.height > 0.0,
                "the week row is laid out, got {frame:?}"
            );
            next.accessibility_actions()
                .activate
                .expect("the week row offers an activate action")();
            assert_eq!(
                *store.borrow(),
                Some(date(2024, 3, 18)),
                "activating week 12 selects its first day"
            );
        },
    );
}

#[test]
fn week_row_alignment() {
    render(
        Some(date(2024, 3, 11)),
        CalendarGranularity::Week,
        DateAvailability::default(),
        |scene, _| {
            let row = buttons(scene)
                .into_iter()
                .find(|node| node.accessibility().is_some_and(|s| s.state.is_selected))
                .expect("selected week row");
            let texts: Vec<Node> = descendants(&row).into_iter().filter(has_text).collect();
            assert_eq!(texts.len(), 8, "a week number plus seven days");
            let row_frame = row.absolute_frame();
            assert!(
                row_frame.width > 0.0 && row_frame.height > 0.0,
                "the week row band is laid out, got {row_frame:?}"
            );
            let first_mid = {
                let frame = texts[0].absolute_frame();
                frame.y + frame.height / 2.0
            };
            for text in &texts {
                let frame = text.absolute_frame();
                let band = text.parent().expect("a cell box").absolute_frame();
                assert!(
                    frame.height < band.height - 8.0,
                    "{} is not stretched to the row ({frame:?} in {band:?})",
                    text_of(text)
                );
                let mid = frame.y + frame.height / 2.0;
                let band_mid = band.y + band.height / 2.0;
                assert!(
                    (mid - band_mid).abs() < 1.0 && (mid - first_mid).abs() < 1.0,
                    "{} at {frame:?} is centred in {band:?}",
                    text_of(text)
                );
            }
            let mut header: Vec<f32> = scene
                .all(|node| has_text(node) && node.absolute_frame().height == 24.0)
                .iter()
                .map(|node| node.absolute_frame().x + node.absolute_frame().width / 2.0)
                .collect();
            header.sort_by(f32::total_cmp);
            let mut columns: Vec<f32> = texts
                .iter()
                .map(|node| node.absolute_frame().x + node.absolute_frame().width / 2.0)
                .collect();
            columns.sort_by(f32::total_cmp);
            assert_eq!(header.len(), 8, "Wk plus seven weekday headers: {header:?}");
            assert!(
                header
                    .iter()
                    .zip(columns.iter())
                    .all(|(a, b)| (a - b).abs() < 1.0),
                "header {header:?} vs row {columns:?}"
            );
            let height = |value: &str| {
                texts
                    .iter()
                    .find(|node| text_of(node) == value)
                    .map(|node| node.absolute_frame().height)
            };
            assert!(
                height("13").is_some() && height("13") == height("12"),
                "today 13 vs 12: {:?} {:?}",
                height("13"),
                height("12")
            );
        },
    );
}

#[test]
fn month_selection() {
    let mut availability = DateAvailability::default();
    availability.minimum = Some(date(2024, 3, 10));
    render(
        Some(date(2024, 3, 20)),
        CalendarGranularity::Month,
        availability,
        |scene, store| {
            assert!(
                buttons(scene)
                    .iter()
                    .all(|node| label(node) != "March 2024 2024"),
                "the period label carries the year once"
            );
            let march = button(scene, "March 2024");
            assert_eq!(
                march.accessibility().map(|s| s.state.is_selected),
                Some(true),
                "the selection's month is selected"
            );
            assert_eq!(
                button(scene, "February 2024")
                    .accessibility()
                    .map(|s| s.state.is_enabled),
                Some(false),
                "a month with no available day is disabled"
            );
            press(scene, &button(scene, "June 2024"));
            assert_eq!(*store.borrow(), Some(date(2024, 6, 1)));
        },
    );
}

#[test]
fn year_selection() {
    render(
        Some(date(2024, 7, 4)),
        CalendarGranularity::Year,
        DateAvailability::default(),
        |scene, store| {
            assert!(button(scene, "2016").is_focusable());
            assert!(button(scene, "2027").is_focusable());
            assert!(
                buttons(scene)
                    .iter()
                    .all(|node| label(node) != "2015" && label(node) != "2028"),
                "the page holds exactly twelve aligned years"
            );
            let next = button(scene, "Next years");
            press(scene, &next);
            assert!(
                buttons(scene).iter().any(|node| label(node) == "2028"),
                "paging renders the next twelve years without a borrow panic"
            );
            press(scene, &button(scene, "2031"));
            assert_eq!(
                *store.borrow(),
                Some(date(2031, 1, 1)),
                "selecting a year on the new page commits its start"
            );
        },
    );
}

#[test]
fn drilling() {
    render(
        Some(date(2024, 3, 13)),
        CalendarGranularity::Day,
        DateAvailability::default(),
        |scene, store| {
            assert!(
                buttons(scene)
                    .iter()
                    .any(|node| label(node) == "March 2024, show months"),
                "the drill-up title names what it opens"
            );
            press(scene, &button_with_text(scene, "March 2024"));
            assert!(
                buttons(scene).iter().any(|node| label(node) == "June 2024"),
                "the title drills up to months"
            );
            assert!(
                buttons(scene)
                    .iter()
                    .all(|node| label(node) != "March 13, 2024"),
                "the day panel is replaced"
            );
            assert_eq!(
                *store.borrow(),
                Some(date(2024, 3, 13)),
                "browsing does not commit"
            );
            press(scene, &button(scene, "June 2024"));
            assert!(
                buttons(scene)
                    .iter()
                    .any(|node| label(node) == "June 20, 2024"),
                "choosing a month drills down to its days"
            );
            assert_eq!(
                *store.borrow(),
                Some(date(2024, 3, 13)),
                "drilling down does not commit"
            );
            press(scene, &button(scene, "June 20, 2024"));
            assert_eq!(
                *store.borrow(),
                Some(date(2024, 6, 20)),
                "a day on the base panel commits"
            );
        },
    );
}

#[test]
fn panel_navigation() {
    let calendar = calendar();
    let all = DateAvailability::default();
    let travel =
        |code: u32, start: Date, panel: CalendarPanel, granularity: CalendarGranularity| {
            let cells = CalendarCells::new(panel, granularity);
            CalendarNavigation::new(code, cells.columns)
                .expect("a navigation key")
                .destination(start, cells, &calendar, &all)
        };
    let march = date(2024, 3, 1);
    use CalendarGranularity as G;
    use CalendarPanel as P;
    assert_eq!(
        travel(Scancode::ARROW_RIGHT, march, P::Months, G::Month),
        Some(date(2024, 4, 1))
    );
    assert_eq!(
        travel(Scancode::ARROW_DOWN, march, P::Months, G::Month),
        Some(date(2024, 6, 1))
    );
    assert_eq!(
        travel(Scancode::PAGE_DOWN, march, P::Months, G::Month),
        Some(date(2025, 3, 1))
    );
    assert_eq!(
        travel(Scancode::HOME, march, P::Months, G::Month),
        Some(date(2024, 1, 1))
    );
    assert_eq!(
        travel(Scancode::END, date(2024, 1, 1), P::Years, G::Year),
        Some(date(2027, 1, 1))
    );
    assert_eq!(
        travel(Scancode::ARROW_UP, date(2024, 1, 1), P::Years, G::Year),
        Some(date(2021, 1, 1))
    );
    assert_eq!(
        travel(Scancode::ARROW_DOWN, date(2024, 3, 11), P::Days, G::Week),
        Some(date(2024, 3, 18))
    );
    assert_eq!(
        travel(Scancode::HOME, date(2024, 3, 11), P::Days, G::Week),
        Some(date(2024, 2, 26))
    );
    assert_eq!(
        travel(Scancode::END, date(2024, 3, 11), P::Days, G::Week),
        Some(date(2024, 3, 25))
    );
}

#[test]
fn labels() {
    render(
        Some(date(2024, 3, 18)),
        CalendarGranularity::Week,
        DateAvailability::default(),
        |scene, _| {
            assert!(
                buttons(scene)
                    .iter()
                    .any(|node| label(node).starts_with("Week 12, ")),
                "the week row names its number and date range"
            );
        },
    );
    render(
        Some(date(2024, 3, 1)),
        CalendarGranularity::Month,
        DateAvailability::default(),
        |scene, _| {
            assert!(
                buttons(scene)
                    .iter()
                    .any(|node| label(node) == "March 2024"),
                "the month label carries the year once"
            );
        },
    );
    render(
        Some(date(2024, 1, 1)),
        CalendarGranularity::Year,
        DateAvailability::default(),
        |scene, _| {
            assert!(
                buttons(scene).iter().any(|node| label(node) == "2024"),
                "the year label is the year"
            );
        },
    );
}
