//! Translation of `Tests/CoveUIComposeTests/Calendar/KeyboardNavigationTests.swift`.
//!
//! Behaviour checklist:
//! - month jumps clamp day numbers and cross leap years and year boundaries;
//!   Home/End land on the month's ends; non-navigation keys map to nothing
//! - month boundaries and page jumps skip unavailable days within the
//!   destination month, and a month with no available day yields no destination
//! - keyboard focus follows month commands while the selection waits for Enter
//! - empty target months, minimum bounds and modified commands leave focus and
//!   the visible month intact
//! - multiple displayed months keep their window until a destination leaves it
//! - `DatePicker` exposes month navigation without committing or dismissing
//!   until Enter
//!
//! Swift read the visible month from a `visibleMonth` binding; Rust has no such
//! option, so the visible month is read from the rendered month title.
//!
//! Named differences (GAP assertions):
//! - DST: Swift's America/New_York calendar makes 2024-02-15 → 2024-03-15 one
//!   hour short of 29 days; the Rust `Calendar` is a fixed offset (-5h here) with
//!   no DST, so the interval is exactly 29 days.

use crate::support;
use cove_composition::Scope;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::{AnyView, DynamicField, View};
use cove_core::scene::{AccessibilityRole, Node};
use cove_core::{Environment, InputCapabilities, State};
use cove_platform::KeyModifiers;
use cove_ui_compose::Scancode;
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::foundation::{Calendar, CalendarUnit, Date, DateParts};
use cove_ui_compose::primitives::controls::calendar::calendar::{CalendarOptions, CalendarView};
use cove_ui_compose::primitives::controls::calendar::navigation::{
    CalendarCells, CalendarNavigation,
};
use cove_ui_compose::primitives::controls::calendar::period::{CalendarGranularity, CalendarPanel};
use cove_ui_compose::primitives::controls::date_values::DateAvailability;
use cove_ui_compose::primitives::controls::{DatePicker, DatePickerOptions};
use cove_ui_compose::primitives::overlays::portal::LayerRoot;

use support::InteractiveScene;

fn calendar_with_offset(offset_seconds: i64) -> Calendar {
    let mut calendar = Calendar::new().with_offset_seconds(offset_seconds);
    calendar.set_first_weekday(2);
    calendar
}

fn calendar() -> Calendar {
    calendar_with_offset(0)
}

fn date_in(calendar: &Calendar, year: i32, month: u32, day: u32) -> Date {
    calendar.date(DateParts::new(year, month, day))
}

fn date(year: i32, month: u32, day: u32) -> Date {
    date_in(&calendar(), year, month, day)
}

fn day_cells() -> CalendarCells {
    CalendarCells::new(CalendarPanel::Days, CalendarGranularity::Day)
}

fn month_title(month: u32, year: i32) -> String {
    const NAMES: [&str; 12] = [
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
    format!("{} {year}", NAMES[month as usize - 1])
}

fn text_of(node: &Node) -> Option<String> {
    node.layout_node()
        .and_then(|layout| layout.text_inputs())
        .map(|inputs| inputs.text)
}

fn day(scene: &InteractiveScene, date: Date) -> Option<Node> {
    scene.first(|node| node.attachment::<Date>("calendar.day") == Some(date) && node.is_focusable())
}

fn shows_title(scene: &InteractiveScene, title: &str) -> bool {
    scene
        .first(|node| text_of(node).as_deref() == Some(title))
        .is_some()
}

fn focused_id(scene: &InteractiveScene) -> Option<cove_core::scene::ElementId> {
    scene.focus.focused().map(|node| node.id())
}

#[test]
fn month_arithmetic() {
    let calendar = calendar_with_offset(-5 * 3_600);
    let cases: [((i32, u32, u32), u32, (i32, u32, u32)); 7] = [
        ((2024, 1, 31), Scancode::PAGE_DOWN, (2024, 2, 29)),
        ((2023, 3, 31), Scancode::PAGE_UP, (2023, 2, 28)),
        ((2024, 1, 31), Scancode::PAGE_UP, (2023, 12, 31)),
        ((2024, 12, 31), Scancode::PAGE_DOWN, (2025, 1, 31)),
        ((2024, 3, 15), Scancode::HOME, (2024, 3, 1)),
        ((2024, 3, 15), Scancode::END, (2024, 3, 31)),
        ((2024, 2, 15), Scancode::PAGE_DOWN, (2024, 3, 15)),
    ];
    for (start, code, expected) in cases {
        let command = CalendarNavigation::new(code, 7).expect("a navigation key");
        assert_eq!(
            command.destination(
                date_in(&calendar, start.0, start.1, start.2),
                day_cells(),
                &calendar,
                &DateAvailability::default()
            ),
            Some(date_in(&calendar, expected.0, expected.1, expected.2)),
            "{start:?} with key {code}"
        );
    }
    let february = date_in(&calendar, 2024, 2, 15);
    let march = date_in(&calendar, 2024, 3, 15);
    assert_eq!(
        march.seconds_since(february),
        29 * 24 * 3_600,
        "GAP: Swift's America/New_York calendar crosses DST so the interval is one \
         hour short (29 days - 3600s); the Rust calendar is a fixed offset with no DST"
    );
    assert_eq!(CalendarNavigation::new(Scancode::A, 7), None);
}

#[test]
fn availability() {
    let calendar = calendar();
    let mut availability = DateAvailability::default();
    availability.minimum = Some(date(2024, 2, 8));
    availability.maximum = Some(date(2024, 2, 20));
    {
        let calendar = calendar.clone();
        availability.is_enabled =
            Rc::new(move |date| ![8, 9, 20].contains(&calendar.component(CalendarUnit::Day, date)));
    }
    for (start, command, expected) in [
        ((2024, 2, 15), CalendarNavigation::First, 10),
        ((2024, 2, 15), CalendarNavigation::Last, 19),
        ((2024, 1, 31), CalendarNavigation::Page(1), 19),
        ((2024, 3, 1), CalendarNavigation::Page(-1), 10),
    ] {
        assert_eq!(
            command.destination(
                date(start.0, start.1, start.2),
                day_cells(),
                &calendar,
                &availability
            ),
            Some(date(2024, 2, expected)),
            "{command:?} from {start:?}"
        );
    }
    availability.minimum = None;
    availability.maximum = None;
    {
        let calendar = calendar.clone();
        availability.is_enabled =
            Rc::new(move |date| [10, 16].contains(&calendar.component(CalendarUnit::Day, date)));
    }
    assert_eq!(
        CalendarNavigation::Page(1).destination(
            date(2024, 1, 15),
            day_cells(),
            &calendar,
            &availability
        ),
        Some(date(2024, 2, 16))
    );
    assert_eq!(
        CalendarNavigation::Page(-1).destination(
            date(2024, 3, 15),
            day_cells(),
            &calendar,
            &availability
        ),
        Some(date(2024, 2, 10))
    );
    availability.is_enabled = Rc::new(|_| false);
    assert_eq!(
        CalendarNavigation::Page(1).destination(
            date(2024, 1, 15),
            day_cells(),
            &calendar,
            &availability
        ),
        None
    );
    assert_eq!(
        CalendarNavigation::First.destination(
            date(2024, 1, 15),
            day_cells(),
            &calendar,
            &availability
        ),
        None
    );
}

/// The Swift `Harness`: a `@State` selection and enablement observed by the
/// body, at a surface width that picks the input capabilities.
struct Harness {
    selection: State<Option<Date>>,
    is_enabled: State<bool>,
    availability: DateAvailability,
    months: usize,
    width: f32,
}

impl Harness {
    fn new(initial: Date, availability: DateAvailability, months: usize, width: f32) -> Rc<Self> {
        Rc::new(Self {
            selection: State::new(Some(initial)),
            is_enabled: State::new(true),
            availability,
            months,
            width,
        })
    }
}

struct HarnessView(Rc<Harness>);

impl View for HarnessView {
    fn body(&self, _cx: &Scope) -> AnyView {
        let harness = &self.0;
        let mut surface = Environment::default();
        surface.viewport.width = harness.width;
        surface.input = if harness.width == 390.0 {
            InputCapabilities::TOUCH | InputCapabilities::KEYBOARD
        } else if harness.width == 720.0 {
            InputCapabilities::TOUCH | InputCapabilities::POINTER | InputCapabilities::KEYBOARD
        } else {
            InputCapabilities::POINTER | InputCapabilities::KEYBOARD
        };
        let mut options = CalendarOptions::default();
        options.calendar = calendar();
        options.availability = harness.availability.clone();
        options.presentation.number_of_months = harness.months;
        options.presentation.is_enabled = harness.is_enabled.get();
        let read = harness.selection.clone();
        let write = harness.selection.clone();
        AnyView::new(
            AnyView::new(
                CalendarView::new(cove_core::Binding::new(
                    move || read.get(),
                    move |value| write.set(value),
                ))
                .configure(|calendar| *calendar = options),
            )
            .surface_environment(surface),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("selection", &self.0.selection),
            DynamicField::state("is_enabled", &self.0.is_enabled),
        ]
    }
}

#[test]
fn keyboard_focus() {
    for width in [390.0_f32, 720.0, 1280.0] {
        InteractiveScene::run(width, 900.0, |scene| {
            let initial = date(2024, 1, 31);
            let harness = Harness::new(initial, DateAvailability::default(), 1, width);
            scene.install(HarnessView(harness.clone()));
            let start = day(scene, initial).expect("January 31 is mounted");
            scene.focus.focus(Some(&start), true);
            for (code, expected) in [
                (Scancode::PAGE_DOWN, (2u32, 29u32)),
                (Scancode::HOME, (2, 1)),
                (Scancode::END, (2, 29)),
                (Scancode::PAGE_UP, (1, 29)),
                (Scancode::PAGE_DOWN, (2, 29)),
            ] {
                scene.key(code, KeyModifiers::default());
                let target = date(2024, expected.0, expected.1);
                assert_eq!(
                    focused_id(scene),
                    day(scene, target).map(|node| node.id()),
                    "navigation focuses {target:?} at width {width}"
                );
                assert!(
                    shows_title(scene, &month_title(expected.0, 2024)),
                    "the visible month follows keyboard focus"
                );
                assert_eq!(harness.selection.get(), Some(initial), "keys never select");
                let focused = scene.focus.focused().expect("a focused day");
                assert_eq!(
                    focused.accessibility().map(|semantics| semantics.role),
                    Some(AccessibilityRole::Button)
                );
                assert!(
                    !scene.graph.recomposer.has_pending(),
                    "nothing is left pending"
                );
            }
            scene.key(Scancode::RETURN, KeyModifiers::default());
            assert_eq!(
                harness.selection.get(),
                Some(date(2024, 2, 29)),
                "Return commits the focused February 29"
            );
            let focused = scene.focus.focused().expect("a focused day");
            assert_eq!(
                focused
                    .accessibility()
                    .map(|semantics| semantics.state.is_selected),
                Some(true),
                "the focused day is the selected one"
            );
            if width < 1_000.0 {
                assert_eq!(
                    focused.frame().height,
                    44.0,
                    "touch surfaces raise cells to 44pt"
                );
            }
            harness.is_enabled.set(false);
            scene.settle(4, 1.0);
            assert!(
                scene.focus.focused().is_none(),
                "disabling the calendar clears focus (width {width})"
            );
        });
    }
}

#[test]
fn unavailable_months() {
    InteractiveScene::run(720.0, 900.0, |scene| {
        let initial = date(2024, 1, 15);
        let mut availability = DateAvailability::default();
        availability.minimum = Some(date(2024, 1, 1));
        {
            let calendar = calendar();
            availability.is_enabled =
                Rc::new(move |date| calendar.component(CalendarUnit::Month, date) != 2);
        }
        let harness = Harness::new(initial, availability, 1, 720.0);
        scene.install(HarnessView(harness.clone()));
        let original = day(scene, initial).expect("January 15 is mounted");
        scene.focus.focus(Some(&original), true);
        scene.key(Scancode::PAGE_DOWN, KeyModifiers::default());
        scene.key(Scancode::PAGE_UP, KeyModifiers::default());
        scene.key(Scancode::PAGE_DOWN, KeyModifiers::LEFT_CTRL);
        assert_eq!(
            focused_id(scene),
            Some(original.id()),
            "focus stays on January 15"
        );
        assert!(
            shows_title(scene, &month_title(1, 2024)),
            "January stays visible"
        );
        assert_eq!(
            harness.selection.get(),
            Some(initial),
            "the selection is untouched"
        );
    });
}

#[test]
fn month_window() {
    InteractiveScene::run(720.0, 900.0, |scene| {
        let initial = date(2024, 1, 15);
        let harness = Harness::new(initial, DateAvailability::default(), 2, 720.0);
        scene.install(HarnessView(harness.clone()));
        let january = day(scene, initial).expect("January 15 is mounted");
        scene.focus.focus(Some(&january), true);
        assert!(
            shows_title(scene, &month_title(1, 2024)) && shows_title(scene, &month_title(2, 2024)),
            "two months are displayed"
        );
        scene.key(Scancode::PAGE_DOWN, KeyModifiers::default());
        assert!(
            shows_title(scene, &month_title(1, 2024)),
            "the window still starts in January"
        );
        assert_eq!(
            focused_id(scene),
            day(scene, date(2024, 2, 15)).map(|node| node.id()),
            "focus moves to February 15 in the second displayed month"
        );
        assert_eq!(
            day(scene, initial).map(|node| node.id()),
            Some(january.id()),
            "the January cell keeps its identity"
        );
        scene.key(Scancode::PAGE_DOWN, KeyModifiers::default());
        assert!(
            shows_title(scene, &month_title(2, 2024)) && shows_title(scene, &month_title(3, 2024)),
            "the displayed window slides to February-March"
        );
        scene.key(Scancode::PAGE_UP, KeyModifiers::default());
        scene.key(Scancode::PAGE_UP, KeyModifiers::default());
        assert!(
            shows_title(scene, &month_title(1, 2024)),
            "the window starts in January"
        );
        assert_eq!(
            harness.selection.get(),
            Some(initial),
            "the selection is untouched"
        );
    });
}

#[test]
fn date_picker() {
    InteractiveScene::run(720.0, 900.0, |scene| {
        let initial = date(2024, 1, 31);
        let selection = Rc::new(RefCell::new(Some(initial)));
        let mut options = DatePickerOptions::default();
        options.calendar.calendar = calendar();
        options.calendar.today = initial;
        let read = selection.clone();
        let write = selection.clone();
        scene.install(LayerRoot::new(AnyView::new(
            DatePicker::new(cove_core::Binding::new(
                move || *read.borrow(),
                move |value| *write.borrow_mut() = value,
            ))
            .configure(|picker| *picker = options),
        )));
        let button = scene
            .first(|node| node.attachment::<bool>("__button_host") == Some(true))
            .expect("the picker trigger");
        scene.focus.focus(Some(&button), true);
        let activate = button
            .accessibility_actions()
            .activate
            .expect("the trigger exposes an activate action");
        activate();
        scene.settle(4, 1.0);
        let start = day(scene, initial).expect("activate presents the calendar");
        scene.focus.focus(Some(&start), true);
        assert_eq!(
            scene.portal.entries().len(),
            1,
            "activate presents one portal"
        );
        scene.key(Scancode::PAGE_DOWN, KeyModifiers::default());
        assert!(
            day(scene, date(2024, 2, 29)).is_some(),
            "PageDown renders the February destination without committing it"
        );
        assert_eq!(*selection.borrow(), Some(initial), "nothing is committed");
        scene.key(Scancode::RETURN, KeyModifiers::default());
        assert_eq!(
            *selection.borrow(),
            Some(date(2024, 2, 29)),
            "Return commits the focused date"
        );
        assert!(
            scene.portal.entries().is_empty(),
            "committing closes the picker"
        );
    });
}
