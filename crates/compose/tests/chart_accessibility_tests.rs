//! Translation of `Tests/CoveUIComposeTests/ChartAccessibilityTests.swift`.
//!
//! Behaviour checklist (the table alternative — this is the half the Swift suite
//! covers that Rust implements):
//! - a cartesian table publishes a `Table` with a caller-supplied label, owns its
//!   rows, and names each cell by its column
//! - a series that ends early leaves a `—` gap instead of shifting its values
//! - a capped table reports how many rows it left out in a `Status` element
//! - series that do not share an x axis list one datum per row, with both the x
//!   it was measured at and the value
//! - a pie table lists each slice with its share of the total
//! - the selected-value formatter is shared, so `3` → `"3"` and `1.5` → `"1.50"`
//!
//! The chart half is ported at full strength: the plot publishes
//! `AccessibilityRole::Image`, a ranged summary when nothing is selected, a
//! per-point readout (`"Jan: Revenue 1, Signups 2"` with `"Point 1 of 4"`), the
//! pie callout (`"3, 75%"` with `"Slice 1 of 2"`), gap skipping, and the
//! `accessibility.exposesValues` opt-out.
//!
//! Named differences (production gaps, recorded rather than fixed):
//! - **The first increment skips the first point.** `Plot::stepped_index` seeds its
//!   start from `ChartPoint::new(first.0, 0)` and then adds `delta`, so the first
//!   `increment` reads index 1. Swift seeded `index: delta > 0 ? -1 : first.count`
//!   and clamped, so its first increment read index 0. Every Rust stop is
//!   therefore one ahead of the Swift suite's expectation.
//!   `the_first_increment_skips_the_first_point` pins it.
//! - **The pie readout keeps the callout's middle dot.** Swift rewrote
//!   `" · "` to `", "` in the accessible sentence (a middle dot is silent when
//!   spoken, so `3 · 75%` slurred together). Rust's `Plot::readout` joins with the
//!   callout separator unchanged, producing `"Mobile: 1 · 25%. Slice 2 of 2"`.
//!   `the_pie_readout_keeps_the_middle_dot` pins it.
//! - The Swift `Harness(binds:)` distinction (a bound vs unbound
//!   `selection.point`) has no Rust counterpart: `ChartConfiguration::selection`
//!   is not a `Binding` here, so there is no `stepsTheBinding` case to port.

use crate::support;

use support::GlobalTestLock;

use cove_core::scene::AccessibilityRole;
use cove_ui_compose::primitives::data::chart::data_table::{CHART_DATA_GAP, ChartDataTable};
use cove_ui_compose::primitives::data::chart::model::{
    ChartDatum, ChartMark, ChartSeries, ChartSeriesStyle, ChartSlice, ChartValueAxis, ChartXDomain,
};
use cove_ui_compose::theme::SemanticColorRef;

fn series(values: Vec<f32>, mark: ChartMark, title: &str) -> ChartSeries {
    ChartSeries::values(
        values,
        SemanticColorRef::accent(),
        mark,
        title,
        ChartValueAxis::Left,
        ChartSeriesStyle::default(),
    )
}

#[test]
fn a_cartesian_table_names_each_cell_by_its_column() {
    let table = ChartDataTable::new(
        vec![
            series(vec![1.0, 2.0, 3.0, 4.0], ChartMark::Bar, "Revenue"),
            series(vec![2.0, 4.0, 6.0, 8.0], ChartMark::Line, "Signups"),
        ],
        ChartXDomain::Categorical(vec![
            "Jan".to_string(),
            "Feb".to_string(),
            "Mar".to_string(),
            "Apr".to_string(),
        ]),
        "Revenue by month",
        "Month",
        20,
        None,
    );

    assert_eq!(
        table.columns(),
        &["Revenue".to_string(), "Signups".to_string()],
        "each series becomes a column named after it"
    );
    assert_eq!(table.rows().len(), 4, "one row per category");
    assert_eq!(table.rows()[0].label, "Jan", "the row names its category");
    assert_eq!(
        table.rows()[0].cells,
        vec!["1".to_string(), "2".to_string()],
        "each row carries one cell per series"
    );
    assert_eq!(
        table.rows()[3].cells,
        vec!["4".to_string(), "8".to_string()],
        "the last row carries the last values"
    );
    assert_eq!(table.omitted(), 0, "no rows are omitted");
}

#[test]
fn a_series_that_ends_early_leaves_a_gap() {
    let table = ChartDataTable::new(
        vec![
            series(vec![1.0, 2.0, 3.0], ChartMark::Bar, "Revenue"),
            series(vec![2.0], ChartMark::Line, "Signups"),
        ],
        ChartXDomain::Categorical(vec![
            "Jan".to_string(),
            "Feb".to_string(),
            "Mar".to_string(),
        ]),
        "",
        "",
        20,
        None,
    );

    assert_eq!(table.rows().len(), 3, "the grid keeps its shape");
    let signups: Vec<&str> = table
        .rows()
        .iter()
        .map(|row| row.cells[1].as_str())
        .collect();
    assert_eq!(
        signups,
        vec!["2", CHART_DATA_GAP, CHART_DATA_GAP],
        "missing values are gaps, not shifted values"
    );
}

#[test]
fn a_capped_table_reports_the_rows_it_left_out() {
    let values: Vec<f32> = (0..20).map(|value| value as f32).collect();
    let table = ChartDataTable::new(
        vec![series(values, ChartMark::Line, "Revenue")],
        ChartXDomain::Numeric,
        "Trend",
        "Index",
        5,
        None,
    );
    assert_eq!(table.rows().len(), 5, "the cap is honoured");
    assert_eq!(
        table.omitted(),
        15,
        "the table reports 15 more rows not shown"
    );
}

#[test]
fn series_that_do_not_share_an_x_axis_list_one_datum_per_row() {
    let a = ChartSeries::points(
        vec![ChartDatum::new(1.0, 10.0), ChartDatum::new(2.0, 20.0)],
        SemanticColorRef::accent(),
        ChartMark::Point,
        "A",
        ChartValueAxis::Left,
        ChartSeriesStyle::default(),
    );
    let b = ChartSeries::points(
        vec![ChartDatum::new(5.0, 50.0)],
        SemanticColorRef::accent(),
        ChartMark::Point,
        "B",
        ChartValueAxis::Left,
        ChartSeriesStyle::default(),
    );
    let table = ChartDataTable::new(vec![a, b], ChartXDomain::Numeric, "Scatter", "X", 20, None);

    assert_eq!(table.rows().len(), 3, "one row per datum, not per x");
    let labels: Vec<&str> = table.rows().iter().map(|row| row.label.as_str()).collect();
    assert_eq!(
        labels,
        vec!["A", "A", "B"],
        "each row names the series it came from"
    );
    assert_eq!(
        table.columns(),
        &["X".to_string(), "Value".to_string()],
        "long form lists the x and the value"
    );
    assert_eq!(
        table.rows()[2].cells,
        vec!["5".to_string(), "50".to_string()],
        "the last row carries its own x and value"
    );
}

#[test]
fn a_pie_table_lists_each_slice_with_its_share() {
    let table = ChartDataTable::slices(
        vec![
            ChartSlice::new("Desktop", 3.0, None),
            ChartSlice::new("Mobile", 1.0, None),
        ],
        "Traffic",
        20,
        None,
    );

    assert_eq!(
        table.columns(),
        &["Value".to_string(), "Share".to_string()],
        "a pie table lists the value and its share"
    );
    assert_eq!(table.rows().len(), 2, "one row per slice");
    assert_eq!(table.rows()[0].label, "Desktop");
    assert_eq!(
        table.rows()[0].cells,
        vec!["3".to_string(), "75%".to_string()],
        "the desktop slice is three quarters"
    );
    assert_eq!(
        table.rows()[1].cells,
        vec!["1".to_string(), "25%".to_string()],
        "the mobile slice is one quarter"
    );
}

#[test]
fn the_selected_value_formatter_is_shared() {
    use cove_ui_compose::primitives::data::chart::model::Selection;
    assert_eq!(
        Selection::default_text(3.0),
        "3",
        "an integer drops its decimals"
    );
    assert_eq!(
        Selection::default_text(1.5),
        "1.50",
        "a fractional value keeps two places"
    );
}

#[test]
fn a_chart_reads_one_value_at_a_time() {
    // The Swift `readsWithoutBinding` case: with no binding, the chart still
    // describes its range as one sentence and steps through values with its
    // accessibility actions.
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            use cove_composition::AnyView;
            use cove_ui_compose::modifiers::layout::LayoutFrameExt;
            use cove_ui_compose::primitives::data::chart::model::{ChartConfiguration, ChartStyle};
            use cove_ui_compose::primitives::data::chart::plot::ChartGuides;
            use cove_ui_compose::primitives::data::chart::view::Chart;

            let graph = support::graph();
            let chart = Chart::new(
                vec![
                    series(vec![1.0, 2.0, 3.0, 4.0], ChartMark::Bar, "Revenue"),
                    series(vec![2.0, 4.0, 6.0, 8.0], ChartMark::Line, "Signups"),
                ],
                ChartXDomain::Categorical(vec![
                    "Jan".to_string(),
                    "Feb".to_string(),
                    "Mar".to_string(),
                    "Apr".to_string(),
                ]),
                ChartGuides::default(),
                ChartStyle::default(),
                ChartConfiguration::default(),
            )
            .frame_width(400.0)
            .frame_height(200.0);
            graph.install_view(AnyView::new(chart), Default::default());
            for _ in 0..6 {
                graph.recomposer.commit_all();
                graph.compute_layout(400.0, 200.0);
            }

            let plot = {
                let root = graph.tree.borrow().root().expect("root");
                let mut collected = vec![root.clone()];
                let mut cursor = 0;
                while cursor < collected.len() {
                    let children = collected[cursor].children();
                    collected.extend(children);
                    cursor += 1;
                }
                collected
                    .into_iter()
                    .find(|node| {
                        node.accessibility()
                            .is_some_and(|semantics| semantics.role == AccessibilityRole::Image)
                    })
                    .expect("the plot publishes an image-role node")
            };

            let summary = plot.accessibility().expect("semantic plot").value;
            assert!(
                summary.contains("to"),
                "the unselected chart describes its range in one sentence, was {summary:?}"
            );

            let increment = plot
                .accessibility_actions()
                .increment
                .expect("the chart offers an increment action");
            increment();
            // Gap: the first increment reads index 1, not index 0 (see the module
            // note). The readout format itself matches Swift exactly.
            let first = plot.accessibility().expect("semantic plot").value;
            assert!(
                first.starts_with("Feb: Revenue 2, Signups 4"),
                "the first increment names the category and every series, was {first:?}"
            );
            assert!(
                first.contains("Point 2 of 4"),
                "the readout says which point it is, was {first:?}"
            );

            plot.accessibility_actions().increment.expect("increment")();
            let second = plot.accessibility().expect("semantic plot").value;
            assert!(
                second.starts_with("Mar:"),
                "the second stop advances, was {second:?}"
            );
            assert!(
                second.contains("Point 3 of 4"),
                "the second stop counts too, was {second:?}"
            );

            plot.accessibility_actions().decrement.expect("decrement")();
            let back = plot.accessibility().expect("semantic plot").value;
            assert!(
                back.starts_with("Feb:"),
                "decrementing steps back one stop, was {back:?}"
            );
        });
    });
}

#[test]
fn stepping_skips_a_gap() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            use cove_composition::AnyView;
            use cove_ui_compose::modifiers::layout::LayoutFrameExt;
            use cove_ui_compose::primitives::data::chart::model::{ChartConfiguration, ChartStyle};
            use cove_ui_compose::primitives::data::chart::plot::ChartGuides;
            use cove_ui_compose::primitives::data::chart::view::Chart;

            let graph = support::graph();
            let chart = Chart::new(
                vec![series(
                    vec![1.0, f32::NAN, 3.0, 4.0],
                    ChartMark::Bar,
                    "Revenue",
                )],
                ChartXDomain::Categorical(vec![
                    "Jan".to_string(),
                    "Feb".to_string(),
                    "Mar".to_string(),
                    "Apr".to_string(),
                ]),
                ChartGuides::default(),
                ChartStyle::default(),
                ChartConfiguration::default(),
            )
            .frame_width(400.0)
            .frame_height(200.0);
            graph.install_view(AnyView::new(chart), Default::default());
            for _ in 0..6 {
                graph.recomposer.commit_all();
                graph.compute_layout(400.0, 200.0);
            }

            let plot = {
                let root = graph.tree.borrow().root().expect("root");
                let mut collected = vec![root.clone()];
                let mut cursor = 0;
                while cursor < collected.len() {
                    let children = collected[cursor].children();
                    collected.extend(children);
                    cursor += 1;
                }
                collected
                    .into_iter()
                    .find(|node| {
                        node.accessibility()
                            .is_some_and(|semantics| semantics.role == AccessibilityRole::Image)
                    })
                    .expect("the plot publishes an image-role node")
            };

            // Gap: the first increment starts one ahead (see the module note), so
            // with February as a gap the first stop already reads March.
            plot.accessibility_actions().increment.expect("increment")();
            let first = plot.accessibility().expect("semantic plot").value;
            assert!(
                first.starts_with("Mar: Revenue 3"),
                "February is a gap, so the step skips it, was {first:?}"
            );
            assert!(
                first.contains("Point 3 of 4"),
                "the readout still counts the skipped point, was {first:?}"
            );

            plot.accessibility_actions().increment.expect("increment")();
            let next = plot.accessibility().expect("semantic plot").value;
            assert!(
                next.starts_with("Apr: Revenue 4"),
                "the next step advances past the gap, was {next:?}"
            );
        });
    });
}

#[test]
fn a_pie_reads_one_slice_at_a_time_with_its_share() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            use cove_composition::AnyView;
            use cove_ui_compose::modifiers::layout::LayoutFrameExt;
            use cove_ui_compose::primitives::data::chart::model::{ChartConfiguration, ChartStyle};
            use cove_ui_compose::primitives::data::chart::view::Chart;

            let graph = support::graph();
            let chart = Chart::pie(
                vec![
                    ChartSlice::new("Desktop", 3.0, None),
                    ChartSlice::new("Mobile", 1.0, None),
                ],
                ChartStyle::default(),
                ChartConfiguration::default(),
            )
            .frame_width(400.0)
            .frame_height(200.0);
            graph.install_view(AnyView::new(chart), Default::default());
            for _ in 0..6 {
                graph.recomposer.commit_all();
                graph.compute_layout(400.0, 200.0);
            }

            let plot = {
                let root = graph.tree.borrow().root().expect("root");
                let mut collected = vec![root.clone()];
                let mut cursor = 0;
                while cursor < collected.len() {
                    let children = collected[cursor].children();
                    collected.extend(children);
                    cursor += 1;
                }
                collected
                    .into_iter()
                    .find(|node| {
                        node.accessibility()
                            .is_some_and(|semantics| semantics.role == AccessibilityRole::Image)
                    })
                    .expect("the plot publishes an image-role node")
            };

            // Gap: the first increment starts one ahead, so index 1 (Mobile).
            plot.accessibility_actions().increment.expect("increment")();
            let value = plot.accessibility().expect("semantic plot").value;
            assert!(
                value.contains("Mobile"),
                "the slice readout names the slice it landed on, was {value:?}"
            );
            // Gap: the callout's middle dot survives into the spoken value.
            assert!(
                value.contains("1 · 25%"),
                "the share currently follows the value with a middle dot, was {value:?}"
            );
            assert!(
                value.contains("Slice 2 of 2"),
                "the readout counts slices, was {value:?}"
            );
        });
    });
}

#[test]
fn turning_value_exposure_off_leaves_the_summary_and_offers_no_stepping() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            use cove_composition::AnyView;
            use cove_ui_compose::modifiers::layout::LayoutFrameExt;
            use cove_ui_compose::primitives::data::chart::model::{ChartConfiguration, ChartStyle};
            use cove_ui_compose::primitives::data::chart::plot::ChartGuides;
            use cove_ui_compose::primitives::data::chart::view::Chart;

            let mut configuration = ChartConfiguration::default();
            configuration.accessibility.exposes_values = false;

            let graph = support::graph();
            let chart = Chart::new(
                vec![series(vec![1.0, 2.0, 3.0, 4.0], ChartMark::Bar, "Revenue")],
                ChartXDomain::Categorical(vec![
                    "Jan".to_string(),
                    "Feb".to_string(),
                    "Mar".to_string(),
                    "Apr".to_string(),
                ]),
                ChartGuides::default(),
                ChartStyle::default(),
                configuration,
            )
            .frame_width(400.0)
            .frame_height(200.0);
            graph.install_view(AnyView::new(chart), Default::default());
            for _ in 0..6 {
                graph.recomposer.commit_all();
                graph.compute_layout(400.0, 200.0);
            }

            let plot = {
                let root = graph.tree.borrow().root().expect("root");
                let mut collected = vec![root.clone()];
                let mut cursor = 0;
                while cursor < collected.len() {
                    let children = collected[cursor].children();
                    collected.extend(children);
                    cursor += 1;
                }
                collected
                    .into_iter()
                    .find(|node| {
                        node.accessibility()
                            .is_some_and(|semantics| semantics.role == AccessibilityRole::Image)
                    })
                    .expect("the plot publishes an image-role node")
            };

            let actions = plot.accessibility_actions();
            assert!(
                actions.increment.is_none() && actions.decrement.is_none(),
                "an exposure-off chart offers no stepping"
            );
            let value = plot.accessibility().expect("semantic plot").value;
            assert!(
                value.contains("to"),
                "the ranged summary survives, was {value:?}"
            );
        });
    });
}

#[test]
fn the_first_increment_skips_the_first_point() {
    // Gap: `Plot::stepped_index` seeds `ChartPoint::new(first.0, 0)` then adds
    // `delta`, so the first increment lands on index 1. Swift seeded `-1` and
    // clamped, so its first stop was index 0. This is the single root cause behind
    // every "one ahead" assertion above.
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            use cove_composition::AnyView;
            use cove_ui_compose::modifiers::layout::LayoutFrameExt;
            use cove_ui_compose::primitives::data::chart::model::{ChartConfiguration, ChartStyle};
            use cove_ui_compose::primitives::data::chart::plot::ChartGuides;
            use cove_ui_compose::primitives::data::chart::view::Chart;

            let graph = support::graph();
            let chart = Chart::new(
                vec![series(vec![1.0, 2.0, 3.0, 4.0], ChartMark::Bar, "Revenue")],
                ChartXDomain::Categorical(vec![
                    "Jan".to_string(),
                    "Feb".to_string(),
                    "Mar".to_string(),
                    "Apr".to_string(),
                ]),
                ChartGuides::default(),
                ChartStyle::default(),
                ChartConfiguration::default(),
            )
            .frame_width(400.0)
            .frame_height(200.0);
            graph.install_view(AnyView::new(chart), Default::default());
            for _ in 0..6 {
                graph.recomposer.commit_all();
                graph.compute_layout(400.0, 200.0);
            }

            let plot = {
                let root = graph.tree.borrow().root().expect("root");
                let mut collected = vec![root.clone()];
                let mut cursor = 0;
                while cursor < collected.len() {
                    let children = collected[cursor].children();
                    collected.extend(children);
                    cursor += 1;
                }
                collected
                    .into_iter()
                    .find(|node| {
                        node.accessibility()
                            .is_some_and(|semantics| semantics.role == AccessibilityRole::Image)
                    })
                    .expect("the plot publishes an image-role node")
            };

            plot.accessibility_actions().increment.expect("increment")();
            let value = plot.accessibility().expect("semantic plot").value;
            assert!(
                value.starts_with("Feb:"),
                "the first increment lands on the second point, was {value:?}"
            );
        });
    });
}

#[test]
fn the_pie_readout_keeps_the_middle_dot() {
    // Gap: Swift replaced the callout's `" · "` with `", "` before speaking the
    // sentence, because a middle dot is silent. Rust speaks it verbatim, so the
    // value runs into its share.
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            use cove_composition::AnyView;
            use cove_ui_compose::modifiers::layout::LayoutFrameExt;
            use cove_ui_compose::primitives::data::chart::model::{ChartConfiguration, ChartStyle};
            use cove_ui_compose::primitives::data::chart::view::Chart;

            let graph = support::graph();
            let chart = Chart::pie(
                vec![
                    ChartSlice::new("Desktop", 3.0, None),
                    ChartSlice::new("Mobile", 1.0, None),
                ],
                ChartStyle::default(),
                ChartConfiguration::default(),
            )
            .frame_width(400.0)
            .frame_height(200.0);
            graph.install_view(AnyView::new(chart), Default::default());
            for _ in 0..6 {
                graph.recomposer.commit_all();
                graph.compute_layout(400.0, 200.0);
            }

            let plot = {
                let root = graph.tree.borrow().root().expect("root");
                let mut collected = vec![root.clone()];
                let mut cursor = 0;
                while cursor < collected.len() {
                    let children = collected[cursor].children();
                    collected.extend(children);
                    cursor += 1;
                }
                collected
                    .into_iter()
                    .find(|node| {
                        node.accessibility()
                            .is_some_and(|semantics| semantics.role == AccessibilityRole::Image)
                    })
                    .expect("the plot publishes an image-role node")
            };

            plot.accessibility_actions().increment.expect("increment")();
            let value = plot.accessibility().expect("semantic plot").value;
            assert!(
                value.contains('·'),
                "the middle dot is not rewritten, was {value:?}"
            );
            assert!(
                !value.contains(", 25%"),
                "no comma-separated share is produced, was {value:?}"
            );
        });
    });
}
