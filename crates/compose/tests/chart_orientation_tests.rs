//! Translation of `Tests/CoveUIComposeTests/ChartOrientationTests.swift`.
//!
//! Behaviour checklist:
//! - a horizontal bar grows right from the zero line and is thick across rows
//! - categories stack down the left while values run along the bottom
//! - a negative bar grows left of the zero line
//! - stacked segments butt together along the value axis
//! - a percent stack fills the plot across the value axis
//! - the value axis keeps the bottom gutter and the categories take the left one
//! - a vertical chart keeps its own gutters under the same options
//! - grid rules cross the value axis, so a horizontal chart draws them upright
//! - the selection rule crosses the selected row
//! - hit-testing picks the row the pointer is on, not the nearest column
//! - a plot resolves its geometry in the style's orientation
//! - an area fills to the baseline along the value axis, whichever way it runs

use crate::support;

use std::rc::Rc;

use cove_core::scene::{Direction, Node};
use cove_core::{Point, Rect, Size};
use cove_ui_compose::primitives::data::chart::{
    Axis, ChartContent, ChartFrame, ChartGuides, ChartOrientation, ChartPlot, ChartPlotInput,
    ChartPoint, ChartScale, ChartSeries, ChartStacking, ChartStyle, ChartText, ChartValueAxes,
    ChartXDomain,
};
use cove_ui_compose::theme::Theme;

use cove_composition::PrimitiveView;

use support::GlobalTestLock;

fn unlabeled() -> Axis {
    let mut axis = Axis::default();
    axis.x.is_labeled = false;
    axis.y.is_labeled = false;
    axis
}

fn flat(orientation: ChartOrientation, stacking: ChartStacking) -> ChartStyle {
    ChartStyle {
        min_value: Some(0.0),
        max_value: Some(4.0),
        shows_grid: false,
        content_inset: 0.0,
        stacking,
        orientation,
        ..ChartStyle::default()
    }
}

fn geometry(
    series: Vec<ChartSeries>,
    style: ChartStyle,
) -> cove_ui_compose::primitives::data::chart::ChartGeometry {
    let axes = ChartValueAxes::resolve(
        &series,
        &vec![true; series.len()],
        &style,
        &ChartGuides::default(),
        &unlabeled(),
    );
    cove_ui_compose::primitives::data::chart::ChartGeometry::new(
        &series,
        axes,
        &ChartXDomain::Categorical(Vec::new()),
        None,
        style.bar_spacing,
        style.orientation,
        Rect::new(0.0, 0.0, 200.0, 100.0),
        None,
    )
}

fn frame_with(
    series: Vec<ChartSeries>,
    x: ChartXDomain,
    style: ChartStyle,
    axis: Axis,
) -> ChartFrame {
    let axes = ChartValueAxes::resolve(
        &series,
        &vec![true; series.len()],
        &style,
        &ChartGuides::default(),
        &axis,
    );
    let x_scale = x.is_numeric().then(|| {
        let extent: Vec<f32> = axes
            .shown()
            .iter()
            .flat_map(|index| series[*index].data.iter().map(|datum| datum.x))
            .collect();
        ChartScale::fit_extent(&extent, None, None, 5, axis.x.kind)
    });
    ChartFrame::cartesian(
        &axes,
        &x,
        x_scale.as_ref(),
        &style,
        &axis,
        Size {
            width: 300.0,
            height: 200.0,
        },
        Point::default(),
        &ChartText::new(&Theme::default_light()),
        None,
    )
}

fn plot_of(
    series: Vec<ChartSeries>,
    style: ChartStyle,
    selection: Option<ChartPoint>,
) -> ChartPlot {
    let content = ChartContent::Cartesian {
        series,
        guides: ChartGuides::default(),
        x: ChartXDomain::Categorical(Vec::new()),
    };
    let count = content.count();
    ChartPlot {
        content,
        visible: vec![true; count],
        style,
        axis: unlabeled(),
        pie: Default::default(),
        format: Rc::new(|value: f32| format!("{}", value.round() as i64)),
        selection,
        summary: ("Chart".to_string(), String::new()),
        callout: Rc::new(|_| None),
        accessibility: Default::default(),
        viewport: Default::default(),
        visible_range: None,
        brush: None,
        brushed_range: None,
        animation: None,
        input: ChartPlotInput::default(),
        theme: None,
    }
}

fn draw(
    plot: &ChartPlot,
    size: Size,
) -> (
    cove_core::DrawList,
    cove_ui_compose::primitives::data::chart::ChartResolution,
) {
    let node = plot.make_node();
    node.set_frame(Rect::new(0.0, 0.0, size.width, size.height));
    plot.update_node(&node);
    let mut list = cove_core::DrawList::default();
    if let Some(draw) = node.draw() {
        draw(&mut list, Point::default());
    }
    let text = ChartText::new(&Theme::default_light());
    let resolution = plot.resolve(size, Point::default(), &text, None);
    (list, resolution)
}

/// A rule spanning `left`..`right` at `y`, with both ends present so a line
/// crossing the row at an angle does not count.
fn has_horizontal_rule(list: &cove_core::DrawList, y: f32, left: f32, right: f32) -> bool {
    let mut saw_left = false;
    let mut saw_right = false;
    for vertex in list.vertices() {
        if (vertex.y - y).abs() >= 1.5 {
            continue;
        }
        if (vertex.x - left).abs() < 1.5 {
            saw_left = true;
        }
        if (vertex.x - right).abs() < 1.5 {
            saw_right = true;
        }
    }
    saw_left && saw_right
}

/// The same, for a rule running down the plot.
fn has_vertical_rule(list: &cove_core::DrawList, x: f32, top: f32, bottom: f32) -> bool {
    let mut saw_top = false;
    let mut saw_bottom = false;
    for vertex in list.vertices() {
        if (vertex.x - x).abs() >= 1.5 {
            continue;
        }
        if (vertex.y - top).abs() < 1.5 {
            saw_top = true;
        }
        if (vertex.y - bottom).abs() < 1.5 {
            saw_bottom = true;
        }
    }
    saw_top && saw_bottom
}

fn bar_series(values: Vec<f32>, title: &str) -> ChartSeries {
    let mut series = ChartSeries::bar(values);
    series.title = title.to_string();
    series
}

#[test]
fn a_horizontal_bar_grows_right_from_the_zero_line() {
    GlobalTestLock::locked(|| {
        let chart = geometry(
            vec![bar_series(vec![2.0, 4.0], "Revenue")],
            flat(ChartOrientation::Horizontal, ChartStacking::None),
        );
        let first = chart.marks[0][0].bar.expect("the bar mark has a rect");
        assert!(first.x.abs() < 0.01, "the bar starts at the left edge");
        assert!(
            (first.max_x() - 100.0).abs() < 0.01,
            "2 of 0..4 is half the plot (got {})",
            first.max_x()
        );
        assert!(
            first.width > first.height,
            "the slot runs down the plot, so the bar is wide and short"
        );
        assert!(
            (first.height - 37.5).abs() < 0.01,
            "the bar is one quarter of a 150pt-ish slot (got {})",
            first.height
        );
    });
}

#[test]
fn categories_stack_down_the_left_and_values_run_along_the_bottom() {
    GlobalTestLock::locked(|| {
        let chart = geometry(
            vec![bar_series(vec![2.0, 4.0], "Revenue")],
            flat(ChartOrientation::Horizontal, ChartStacking::None),
        );
        assert!(
            chart.marks[0][0].y < chart.marks[0][1].y,
            "later categories sit lower"
        );
        assert!(
            chart.marks[0][0].x < chart.marks[0][1].x,
            "later values sit further right"
        );

        let vertical = geometry(
            vec![bar_series(vec![2.0, 4.0], "Revenue")],
            flat(ChartOrientation::Vertical, ChartStacking::None),
        );
        assert!(
            vertical.marks[0][0].x < vertical.marks[0][1].x,
            "a column chart advances along x"
        );
        assert!(
            vertical.marks[0][0].y > vertical.marks[0][1].y,
            "a bigger value sits higher, so its y is smaller"
        );
    });
}

#[test]
fn a_negative_bar_grows_left_of_the_zero_line() {
    GlobalTestLock::locked(|| {
        let chart = geometry(
            vec![bar_series(vec![-2.0, 2.0], "Change")],
            ChartStyle {
                min_value: Some(-2.0),
                max_value: Some(2.0),
                ..flat(ChartOrientation::Horizontal, ChartStacking::None)
            },
        );
        let loss = chart.marks[0][0].bar.expect("the loss bar has a rect");
        let gain = chart.marks[0][1].bar.expect("the gain bar has a rect");
        assert!(
            loss.x.abs() < 0.01 && (loss.max_x() - 100.0).abs() < 0.01,
            "the loss bar grows left of the zero line"
        );
        assert!(
            (gain.x - 100.0).abs() < 0.01 && (gain.max_x() - 200.0).abs() < 0.01,
            "the gain bar grows right of the zero line"
        );
        let _ = Direction::Ltr;
    });
}

#[test]
fn stacked_segments_butt_together_along_the_value_axis() {
    GlobalTestLock::locked(|| {
        let chart = geometry(
            vec![
                bar_series(vec![1.0, 1.0], "Online"),
                bar_series(vec![1.0, 1.0], "Retail"),
            ],
            ChartStyle {
                min_value: Some(0.0),
                max_value: Some(2.0),
                ..flat(ChartOrientation::Horizontal, ChartStacking::Stacked)
            },
        );
        let lower = chart.marks[0][0].bar.expect("the lower segment");
        let upper = chart.marks[1][0].bar.expect("the upper segment");
        assert!(
            (lower.max_x() - upper.x).abs() < 0.01,
            "the segments butt together"
        );
        assert!(
            lower.x.abs() < 0.01 && (upper.max_x() - 200.0).abs() < 0.01,
            "the stack spans the whole plot"
        );
        assert!(
            (lower.y - upper.y).abs() < 0.01,
            "both segments share the row, so their thickness matches"
        );
    });
}

#[test]
fn a_percent_stack_fills_the_plot_across_the_value_axis() {
    GlobalTestLock::locked(|| {
        let chart = geometry(
            vec![
                bar_series(vec![3.0, 1.0], "Online"),
                bar_series(vec![1.0, 3.0], "Retail"),
            ],
            ChartStyle {
                min_value: None,
                max_value: None,
                ..flat(ChartOrientation::Horizontal, ChartStacking::Percent)
            },
        );
        for index in 0..2 {
            let segments: Vec<Rect> = (0..2)
                .filter_map(|series| chart.marks[series][index].bar)
                .collect();
            let left = segments
                .iter()
                .map(|rect| rect.x)
                .fold(f32::INFINITY, f32::min);
            let right = segments
                .iter()
                .map(|rect| rect.max_x())
                .fold(f32::NEG_INFINITY, f32::max);
            assert!(left.abs() < 0.01, "row {index} starts at the plot edge");
            assert!(
                (right - 200.0).abs() < 0.01,
                "row {index} fills the plot (got {right})"
            );
        }
    });
}

#[test]
fn the_value_axis_keeps_the_bottom_gutter_and_categories_take_the_left() {
    GlobalTestLock::locked(|| {
        let mut axis = Axis::default();
        axis.y.title = "Revenue".to_string();
        axis.x.title = "Country".to_string();

        let chart = frame_with(
            vec![bar_series(vec![2.0, 4.0], "Revenue")],
            ChartXDomain::Categorical(vec!["A".to_string(), "B".to_string()]),
            ChartStyle {
                orientation: ChartOrientation::Horizontal,
                ..ChartStyle::default()
            },
            axis,
        );

        assert_eq!(chart.orientation, ChartOrientation::Horizontal);
        assert!(
            chart.plot.x > chart.bounds.x,
            "the plot gives up room on the left for category labels"
        );
        assert!(
            chart.plot.max_y() < chart.bounds.max_y(),
            "and at the bottom for value labels and the value title"
        );
        assert!(
            chart
                .value_ticks
                .windows(2)
                .all(|pair| pair[0].position < pair[1].position),
            "values advance along x"
        );
        assert_eq!(chart.category_ticks.len(), 2, "one tick per category");
        assert!(
            chart
                .category_ticks
                .windows(2)
                .all(|pair| pair[0].position < pair[1].position),
            "categories advance down y"
        );
        assert!(
            (chart.category_ticks[0].position - (chart.plot.y + chart.plot.height / 4.0)).abs()
                < 0.5,
            "the first category sits a quarter down the plot"
        );
        assert_eq!(chart.value_title, "Revenue", "the value title is published");
        assert_eq!(
            chart.category_title, "Country",
            "the category title is published"
        );
    });
}

#[test]
fn a_vertical_chart_keeps_its_own_gutters() {
    GlobalTestLock::locked(|| {
        let mut axis = Axis::default();
        axis.y.title = "Revenue".to_string();
        axis.x.title = "Country".to_string();

        let chart = frame_with(
            vec![bar_series(vec![2.0, 4.0], "Revenue")],
            ChartXDomain::Categorical(vec!["A".to_string(), "B".to_string()]),
            ChartStyle {
                orientation: ChartOrientation::Vertical,
                ..ChartStyle::default()
            },
            axis,
        );

        assert_eq!(chart.orientation, ChartOrientation::Vertical);
        assert!(chart.plot.x > chart.bounds.x, "the left gutter remains");
        assert!(
            chart.plot.max_y() < chart.bounds.max_y(),
            "the bottom gutter remains"
        );
        assert!(
            chart
                .value_ticks
                .windows(2)
                .all(|pair| pair[0].position > pair[1].position),
            "values run up the y axis in a vertical chart"
        );
        assert!(
            chart
                .category_ticks
                .windows(2)
                .all(|pair| pair[0].position < pair[1].position),
            "categories still advance left to right"
        );
    });
}

#[test]
fn grid_rules_cross_the_value_axis_so_a_horizontal_chart_draws_them_upright() {
    GlobalTestLock::locked(|| {
        let style = ChartStyle {
            min_value: Some(0.0),
            max_value: Some(4.0),
            shows_grid: true,
            content_inset: 0.0,
            orientation: ChartOrientation::Horizontal,
            ..ChartStyle::default()
        };
        let plot = plot_of(vec![bar_series(vec![2.0, 4.0], "Revenue")], style, None);
        let (list, resolution) = draw(
            &plot,
            Size {
                width: 200.0,
                height: 100.0,
            },
        );
        let frame = resolution.frame;
        // Ticks land on 0..4 across the plot width.
        assert!(
            has_vertical_rule(&list, 100.0, frame.plot.y, frame.plot.max_y()),
            "a grid rule crosses the value axis, so it is vertical here"
        );
        assert!(
            has_horizontal_rule(&list, frame.plot.max_y(), frame.plot.x, frame.plot.max_x()),
            "the axis line still closes the bottom edge"
        );
    });
}

#[test]
fn the_selection_rule_crosses_the_selected_row() {
    GlobalTestLock::locked(|| {
        let style = ChartStyle {
            min_value: Some(0.0),
            max_value: Some(4.0),
            shows_grid: false,
            content_inset: 0.0,
            orientation: ChartOrientation::Horizontal,
            ..ChartStyle::default()
        };
        let plot = plot_of(
            vec![bar_series(vec![2.0, 4.0], "Revenue")],
            style,
            Some(ChartPoint::new(0, 1)),
        );
        let (list, resolution) = draw(
            &plot,
            Size {
                width: 200.0,
                height: 100.0,
            },
        );
        let chart = resolution.cartesian.as_ref().expect("a cartesian geometry");
        assert!(
            has_horizontal_rule(&list, chart.marks[0][1].y, chart.plot.x, chart.plot.max_x()),
            "the selection rule crosses the selected row"
        );
    });
}

#[test]
fn hit_testing_picks_the_row_the_pointer_is_on() {
    GlobalTestLock::locked(|| {
        let chart = geometry(
            vec![bar_series(vec![2.0, 4.0], "Revenue")],
            flat(ChartOrientation::Horizontal, ChartStacking::None),
        );
        assert_eq!(
            chart.nearest(120.0, 20.0).map(|mark| mark.index),
            Some(0),
            "a pointer near the top picks the first row"
        );
        assert_eq!(
            chart.nearest(120.0, 80.0).map(|mark| mark.index),
            Some(1),
            "a pointer near the bottom picks the second row"
        );
    });
}

#[test]
fn a_plot_resolves_its_geometry_in_the_styles_orientation() {
    GlobalTestLock::locked(|| {
        let plot = plot_of(
            vec![bar_series(vec![1.0, 2.0, 3.0], "Revenue")],
            ChartStyle {
                orientation: ChartOrientation::Horizontal,
                ..ChartStyle::default()
            },
            None,
        );
        let text = ChartText::new(&Theme::default_light());
        let resolution = plot.resolve(
            Size {
                width: 200.0,
                height: 100.0,
            },
            Point::default(),
            &text,
            None,
        );
        assert_eq!(
            resolution.frame.orientation,
            ChartOrientation::Horizontal,
            "the frame follows the style"
        );
        assert_eq!(
            resolution.cartesian.as_ref().map(|chart| chart.orientation),
            Some(ChartOrientation::Horizontal),
            "the geometry follows the style too"
        );
    });
}

#[test]
fn an_area_fills_to_the_baseline_along_the_value_axis() {
    GlobalTestLock::locked(|| {
        let mut area = ChartSeries::area(vec![2.0, 4.0]);
        area.title = "Revenue".to_string();
        let series = vec![area];
        let style = ChartStyle {
            min_value: Some(0.0),
            max_value: Some(4.0),
            shows_grid: false,
            content_inset: 0.0,
            orientation: ChartOrientation::Horizontal,
            ..ChartStyle::default()
        };
        let plot = plot_of(series, style, None);
        let (list, resolution) = draw(
            &plot,
            Size {
                width: 200.0,
                height: 100.0,
            },
        );
        let chart = resolution.cartesian.as_ref().expect("a cartesian geometry");
        let mid = (chart.marks[0][0].y + chart.marks[0][1].y) / 2.0;

        let mut covered = false;
        let vertices = list.vertices();
        let indices = list.indices();
        for triangle in indices.chunks(3) {
            let points: Vec<(f32, f32)> = triangle
                .iter()
                .map(|index| {
                    let vertex = &vertices[*index as usize];
                    (vertex.x, vertex.y)
                })
                .collect();
            let ys: Vec<f32> = points.iter().map(|point| point.1).collect();
            let xs: Vec<f32> = points.iter().map(|point| point.0).collect();
            let y_min = ys.iter().copied().fold(f32::INFINITY, f32::min);
            let y_max = ys.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            let x_min = xs.iter().copied().fold(f32::INFINITY, f32::min);
            let x_max = xs.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            if y_min <= mid && y_max >= mid && x_min <= 20.0 && x_max >= 100.0 {
                covered = true;
                break;
            }
        }
        assert!(covered, "the area fills back to the zero line on the left");
    });
}

fn _unused(_: &Node) {}
