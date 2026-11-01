//! Translation of `Tests/CoveUIComposeTests/DateRangePickerTests.swift`.
//!
//! Behaviour checklist:
//! - two clicks on the calendar select a closed range
//! - clicking a day before the pending start swaps the bounds so the earlier
//!   day becomes the lower bound

mod support;

use cove_composition::AnyView;
use cove_core::State;
use cove_core::scene::{AccessibilityRole, Node};
use cove_ui_compose::foundation::{Calendar, Date};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::calendar::CalendarOptions;
use cove_ui_compose::primitives::controls::fields::date_range::{DateRange, DateRangeCalendar};
use cove_ui_compose::primitives::controls::fields::date_range_rules::DateRangeRules;

use support::InteractiveScene;

/// 2026-10-01T00:00:00Z.
const TODAY: i64 = 1_790_812_800;

fn today() -> Date {
    Date::from_seconds(TODAY)
}

fn options() -> CalendarOptions {
    let mut calendar = Calendar::new();
    calendar.set_first_weekday(1);
    CalendarOptions {
        calendar,
        today: today(),
        ..Default::default()
    }
}

fn nodes(root: &Node) -> Vec<Node> {
    let mut collected = vec![root.clone()];
    let mut index = 0;
    while index < collected.len() {
        collected.extend(collected[index].children());
        index += 1;
    }
    collected
}

/// The mounted day cell whose `calendar.day` attachment falls on `day`.
fn day_cell(scene: &support::InteractiveScene, day: i64) -> Option<Node> {
    let expected = Date::from_seconds(TODAY + (day - 1) * Date::SECONDS_PER_DAY);
    scene.first(|node| node.attachment::<Date>("calendar.day") == Some(expected))
}

#[test]
fn two_clicks_select_a_range_and_an_earlier_click_swaps_the_bounds() {
    let recorded: State<Option<DateRange>> = State::new(None);

    InteractiveScene::run(500.0, 600.0, |scene| {
        let binding = recorded.binding();
        let calendar = DateRangeCalendar::new(binding, options(), DateRangeRules::default());
        scene.install(AnyView::new(
            AnyView::new(calendar).frame_size(420.0, 520.0),
        ));

        let tenth = day_cell(scene, 10).expect("day 10 mounts");
        let fifteenth = day_cell(scene, 15).expect("day 15 mounts");
        let twentieth = day_cell(scene, 20).expect("day 20 mounts");
        let twelfth = day_cell(scene, 12).expect("day 12 mounts");

        scene.click(&tenth, 1);
        assert_eq!(
            recorded.get().map(|range| range.lower),
            Some(Date::from_seconds(TODAY + 9 * Date::SECONDS_PER_DAY)),
            "the first click anchors the range start at day 10"
        );
        scene.click(&fifteenth, 1);
        let range = recorded.get().expect("two clicks complete a range");
        assert_eq!(
            range.lower,
            Date::from_seconds(TODAY + 9 * Date::SECONDS_PER_DAY),
            "the lower bound is day 10"
        );
        assert_eq!(
            range.upper,
            Date::from_seconds(TODAY + 14 * Date::SECONDS_PER_DAY),
            "the upper bound is day 15"
        );

        // A later start followed by an earlier day swaps the bounds.
        scene.click(&twentieth, 1);
        scene.click(&twelfth, 1);
        let swapped = recorded.get().expect("the second range completes");
        assert_eq!(
            swapped.lower,
            Date::from_seconds(TODAY + 11 * Date::SECONDS_PER_DAY),
            "the earlier day becomes the lower bound"
        );
        assert_eq!(
            swapped.upper,
            Date::from_seconds(TODAY + 19 * Date::SECONDS_PER_DAY),
            "the later day stays the upper bound"
        );
    });

    let _ = AccessibilityRole::Button;
}
