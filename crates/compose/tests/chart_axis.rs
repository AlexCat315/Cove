//! Translation of `Tests/CoveUIComposeTests/ChartAxisTests.swift`.
//!
//! Behaviour checklist:
//! - a 0...47 range fits to 0...50 with a tick every 10
//! - explicit bounds are kept while ticks stay round inside them
//! - a fractional range ticks on quarters and trims trailing zeros
//! - negative ranges keep zero on a tick
//! - a constant series still gets a usable range
//! - a zero tick count keeps the fitted range but draws no ticks
//! - a log axis snaps to decades, spaces by power of ten and ignores
//!   non-positive values
//! - a percent stack stays linear even when the axis asks for log

use cove_core::Rect;
use cove_ui_compose::primitives::data::chart::{
    Axis, ChartRange, ChartScale, ChartScaleKind, ChartSeries, ChartStack, ChartStacking,
    ChartStyle,
};

/// A `ChartScale` fitted from one primary series, the Swift `scale(...)` helper.
fn scale(series: Vec<ChartSeries>, tick_count: i32, kind: ChartScaleKind) -> ChartScale {
    let style = ChartStyle::default();
    let visible = vec![true; series.len()];
    let stack = ChartStack::new(&series, &visible, &style, ChartAxisKind::Left);
    ChartScale::fit_stack(&stack, &[], &[], &style, tick_count, kind)
}

use cove_ui_compose::primitives::data::chart::ChartValueAxis as ChartAxisKind;

fn ticks_of(scale: &ChartScale) -> Vec<f32> {
    scale.ticks.clone()
}

fn labels_of(scale: &ChartScale) -> Vec<String> {
    scale
        .ticks
        .iter()
        .map(|value| scale.label(*value))
        .collect()
}

#[test]
fn a_round_range_fits_to_a_nice_upper_bound() {
    let fitted = scale(
        vec![ChartSeries::line(vec![0.0, 47.0])],
        4,
        ChartScaleKind::Linear,
    );
    assert_eq!(
        (fitted.domain.lower, fitted.domain.upper),
        (0.0, 50.0),
        "0...47 fits up to 0...50"
    );
    assert_eq!(
        ticks_of(&fitted),
        vec![0.0, 10.0, 20.0, 30.0, 40.0, 50.0],
        "ticks land every 10"
    );
    assert_eq!(
        labels_of(&fitted),
        vec!["0", "10", "20", "30", "40", "50"],
        "labels render the tick values"
    );
}

#[test]
fn explicit_bounds_are_kept_while_ticks_stay_round() {
    let style = ChartStyle::new(
        Some(0.0),
        Some(47.0),
        true,
        2.0,
        2.0,
        4.0,
        None,
        cove_ui_compose::theme::SemanticColorRef::divider(),
        ChartStacking::None,
        0.6,
        cove_ui_compose::primitives::data::chart::ChartOrientation::Vertical,
    );
    let series = vec![ChartSeries::line(vec![0.0, 47.0])];
    let visible = vec![true; series.len()];
    let stack = ChartStack::new(&series, &visible, &style, ChartAxisKind::Left);
    let fitted = ChartScale::fit_stack(&stack, &[], &[], &style, 4, ChartScaleKind::Linear);

    assert_eq!(
        (fitted.domain.lower, fitted.domain.upper),
        (0.0, 47.0),
        "authored bounds are honoured as-is"
    );
    assert_eq!(
        ticks_of(&fitted),
        vec![0.0, 10.0, 20.0, 30.0, 40.0],
        "ticks stay round and inside the bounds"
    );
}

#[test]
fn a_fractional_range_ticks_on_quarters() {
    let fitted = scale(
        vec![ChartSeries::line(vec![0.0, 1.0])],
        4,
        ChartScaleKind::Linear,
    );
    assert_eq!(
        ticks_of(&fitted),
        vec![0.0, 0.25, 0.5, 0.75, 1.0],
        "a 0...1 range ticks on quarters"
    );
    assert_eq!(
        labels_of(&fitted),
        vec!["0", "0.25", "0.5", "0.75", "1"],
        "trailing zeros are trimmed from the labels"
    );
}

#[test]
fn negative_ranges_keep_zero_on_a_tick() {
    let fitted = scale(
        vec![ChartSeries::bar(vec![-30.0, 80.0])],
        4,
        ChartScaleKind::Linear,
    );
    assert_eq!(
        fitted.domain,
        ChartRange::new(-50.0, 100.0),
        "the range widens to round bounds on both sides"
    );
    assert_eq!(
        fitted.ticks.first(),
        Some(&-50.0),
        "the first tick is the lower bound"
    );
    assert_eq!(
        fitted.ticks.last(),
        Some(&100.0),
        "the last tick is the upper bound"
    );
    assert!(
        fitted.ticks.contains(&0.0),
        "zero stays on a tick so the baseline is readable"
    );
}

#[test]
fn a_constant_series_still_gets_a_usable_range() {
    let fitted = scale(
        vec![ChartSeries::line(vec![5.0, 5.0])],
        4,
        ChartScaleKind::Linear,
    );
    assert!(
        fitted.domain.lower < 5.0,
        "a flat series pads below its value (lower {})",
        fitted.domain.lower
    );
    assert!(
        fitted.domain.upper > 5.0,
        "and pads above it (upper {})",
        fitted.domain.upper
    );
    assert!(
        fitted.ticks.len() >= 2,
        "there is still more than one tick (got {})",
        fitted.ticks.len()
    );
}

#[test]
fn a_zero_tick_count_keeps_the_range_but_draws_no_ticks() {
    let fitted = scale(
        vec![ChartSeries::line(vec![0.0, 47.0])],
        0,
        ChartScaleKind::Linear,
    );
    assert!(fitted.ticks.is_empty(), "no ticks are produced");
    assert_eq!(fitted.domain.upper, 50.0, "the fitted range is unchanged");
}

#[test]
fn a_log_axis_snaps_to_decades_and_stays_countable() {
    let fitted = scale(
        vec![ChartSeries::line(vec![1.0, 500.0])],
        4,
        ChartScaleKind::Log,
    );
    assert_eq!(
        fitted.domain,
        ChartRange::new(1.0, 1000.0),
        "a log range snaps outward to whole decades"
    );
    assert_eq!(
        fitted.kind,
        ChartScaleKind::Log,
        "the axis reports its kind"
    );
    assert!(
        fitted.ticks.contains(&1.0)
            && fitted.ticks.contains(&10.0)
            && fitted.ticks.contains(&100.0),
        "decades are ticked"
    );
    assert!(
        fitted
            .ticks
            .iter()
            .all(|value| *value >= 1.0 && *value <= 1000.0),
        "every tick is inside the domain"
    );
    assert!(
        fitted.ticks.len() <= 9,
        "the tick count stays countable (got {})",
        fitted.ticks.len()
    );
}

#[test]
fn a_log_axis_spaces_values_by_power_of_ten() {
    let fitted = scale(
        vec![ChartSeries::line(vec![1.0, 100.0])],
        4,
        ChartScaleKind::Log,
    );
    let plot = Rect::new(0.0, 0.0, 100.0, 100.0);
    assert!(
        (fitted.y(1.0, plot) - 100.0).abs() < 0.01,
        "the lower decade maps to the plot bottom"
    );
    assert!(
        (fitted.y(10.0, plot) - 50.0).abs() < 0.01,
        "the next decade maps to the midpoint"
    );
    assert!(
        (fitted.y(100.0, plot) - 0.0).abs() < 0.01,
        "the upper decade maps to the plot top"
    );
    assert_eq!(
        fitted.y(0.0, plot),
        100.0,
        "zero has no place on a log axis and clamps to the bottom"
    );
}

#[test]
fn a_log_axis_ignores_non_positive_values() {
    let fitted = scale(
        vec![ChartSeries::line(vec![-5.0, 0.0, 20.0, 2000.0])],
        4,
        ChartScaleKind::Log,
    );
    assert_eq!(
        fitted.domain,
        ChartRange::new(10.0, 10_000.0),
        "only the positive values widen the decades"
    );
    assert!(
        fitted.ticks.iter().all(|value| *value > 0.0),
        "no non-positive tick is produced"
    );
}

#[test]
fn a_percent_stack_stays_linear_even_when_the_axis_asks_for_log() {
    let style = ChartStyle::new(
        None,
        None,
        true,
        2.0,
        2.0,
        4.0,
        None,
        cove_ui_compose::theme::SemanticColorRef::divider(),
        ChartStacking::Percent,
        0.6,
        cove_ui_compose::primitives::data::chart::ChartOrientation::Vertical,
    );
    let series = vec![ChartSeries::bar(vec![1.0]), ChartSeries::bar(vec![3.0])];
    let visible = vec![true; series.len()];
    let stack = ChartStack::new(&series, &visible, &style, ChartAxisKind::Left);
    let fitted = ChartScale::fit_stack(&stack, &[], &[], &style, 4, ChartScaleKind::Log);

    assert_eq!(
        fitted.kind,
        ChartScaleKind::Linear,
        "a percent stack always fits linearly"
    );
    assert_eq!(
        fitted.domain,
        ChartRange::new(0.0, 100.0),
        "a percent axis spans 0...100"
    );
}

#[test]
fn the_axis_configuration_carries_authored_labels_and_titles() {
    let axis = Axis::default();
    assert_eq!(
        axis.y.tick_count, 4,
        "the value axis ticks four times by default"
    );
    assert_eq!(
        axis.x.tick_count, 5,
        "the x axis ticks five times by default"
    );
    assert!(axis.x.is_labeled, "the x axis is labelled by default");
    assert!(axis.y.is_labeled, "the y axis is labelled by default");

    let mut custom = Axis::default();
    custom.y.title = "Revenue".to_string();
    custom.x.title = "Month".to_string();
    custom.y.tick_count = 4;
    custom.y.kind = ChartScaleKind::Log;
    assert_eq!(custom.y.title, "Revenue");
    assert_eq!(custom.x.title, "Month");
    assert_eq!(custom.y.tick_count, 4);
    assert_eq!(custom.y.kind, ChartScaleKind::Log);
}

#[test]
fn scale_labels_fix_decimals_to_the_tick_step() {
    // `fixed` rounds to the requested precision then trims trailing fraction
    // zeros, matching the Swift `ChartScale.fixed`.
    assert_eq!(ChartScale::fixed(0.25, 2), "0.25", "two decimals are kept");
    assert_eq!(
        ChartScale::fixed(1.0, 2),
        "1",
        "trailing fraction zeros are trimmed"
    );
    assert_eq!(
        ChartScale::fixed(1.5, 0),
        "2",
        "zero decimals round to an integer"
    );
    assert_eq!(ChartScale::fixed(0.0, 0), "0");
    assert_eq!(
        ChartScale::fixed(-0.0001, 1),
        "0",
        "a rounded-to-zero negative drops its sign"
    );
    assert_eq!(
        ChartScale::fixed(f32::NAN, 2),
        "",
        "non-finite values render nothing"
    );

    let fractional = scale(
        vec![ChartSeries::line(vec![0.0, 1.0])],
        4,
        ChartScaleKind::Linear,
    );
    assert_eq!(
        fractional.decimals, 2,
        "the quarter step needs two decimals"
    );
    let integral = scale(
        vec![ChartSeries::line(vec![0.0, 47.0])],
        4,
        ChartScaleKind::Linear,
    );
    assert_eq!(integral.decimals, 0, "a 10-step needs no decimals");
}

#[test]
fn the_ratio_clamps_off_axis_values() {
    let fitted = scale(
        vec![ChartSeries::line(vec![0.0, 100.0])],
        4,
        ChartScaleKind::Linear,
    );
    assert_eq!(
        fitted.ratio(fitted.domain.lower),
        0.0,
        "the lower bound is 0"
    );
    assert_eq!(
        fitted.ratio(fitted.domain.upper),
        1.0,
        "the upper bound is 1"
    );
    assert_eq!(
        fitted.ratio(-1_000.0),
        0.0,
        "a value below the domain clamps to 0"
    );
    assert_eq!(
        fitted.ratio(1_000.0),
        1.0,
        "a value above the domain clamps to 1"
    );
}
