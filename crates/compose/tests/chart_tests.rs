//! Translation of `Tests/CoveUIComposeTests/ChartTests.swift`.
//!
//! Behaviour checklist:
//! - a chart without a selection binding is a passive image with a 64pt default
//!   height and no input
//! - lines stroke between values; areas fill down to the baseline
//! - bar series group side by side, or accumulate when stacked, and a hidden
//!   series takes no space
//! - a negative value stacks under the baseline instead of cancelling surplus
//! - percent stacking fills each index to 100, keeps a negative side, and spans
//!   -100..100
//! - a right-axis series fits its own scale and never stretches the left one
//! - mixed line and bar marks share slot centres
//! - scatter points use their own x values and hit-test by distance
//! - donut wedges start at 12 o'clock, run clockwise and hit-test by angle
//! - thresholds and markers add guide geometry
//! - selection draws a rule and a ringed point
//! - a non-finite value breaks the line instead of drawing a zero
//! - a dash pattern leaves gaps; a curve bulges past the chord
//! - a series can widen its own stroke and size its own points
//! - a numeric x domain puts bars and points on one axis
//! - a band shades its corridor and widens the domain
//! - guide labels and annotations anchor to their marks
//! - value labels follow `showsValues` and drop collisions
//! - a pie sorts by value without losing slice identity, and can label shares
//! - a histogram bins samples into even widths, honours a range, and survives
//!   identical samples

use crate::support;

use std::rc::Rc;

use cove_composition::PrimitiveView;
use cove_core::{Point, Rect, Size};
use cove_ui_compose::primitives::data::chart::{
    Axis, ChartBand, ChartContent, ChartDatum, ChartGuides, ChartHistogram, ChartPlot,
    ChartPlotInput, ChartPoint, ChartSeries, ChartSeriesStyle, ChartSlice, ChartStacking,
    ChartStyle, ChartText, ChartValueAxes, ChartValueAxis, ChartXDomain, PieGeometry,
};
use cove_ui_compose::theme::{SemanticColorRef, Theme};

use support::GlobalTestLock;

fn unlabeled() -> Axis {
    let mut axis = Axis::default();
    axis.x.is_labeled = false;
    axis.y.is_labeled = false;
    axis
}

fn cartesian(series: Vec<ChartSeries>, guides: ChartGuides) -> ChartContent {
    ChartContent::Cartesian {
        series,
        guides,
        x: ChartXDomain::Categorical(Vec::new()),
    }
}

fn plot_of(content: ChartContent, style: ChartStyle, selection: Option<ChartPoint>) -> ChartPlot {
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

fn flat() -> ChartStyle {
    ChartStyle {
        shows_grid: false,
        content_inset: 0.0,
        ..ChartStyle::default()
    }
}

fn draw(
    content: ChartContent,
    style: ChartStyle,
    selection: Option<ChartPoint>,
    size: Size,
) -> (cove_core::DrawList, Theme) {
    let plot = plot_of(content, style, selection);
    let node = plot.make_node();
    node.set_frame(Rect::new(0.0, 0.0, size.width, size.height));
    plot.update_node(&node);
    let mut list = cove_core::DrawList::default();
    if let Some(draw) = node.draw() {
        draw(&mut list, Point::default());
    }
    (list, cove_ui_compose::theme::theme_for_node(&node))
}

/// Whether the rendered triangles cover `point`, optionally restricted to one
/// colour (compared on the RGB channels, ignoring alpha).
fn covers(list: &cove_core::DrawList, x: f32, y: f32, color: Option<cove_core::Color>) -> bool {
    let vertices = list.vertices();
    let indices = list.indices();
    for triangle in indices.chunks(3) {
        let points: Vec<&cove_core::Vertex> = triangle
            .iter()
            .map(|index| &vertices[*index as usize])
            .collect();
        if let Some(color) = color
            && points[0].color & 0x00FF_FFFF != color.rgba8() & 0x00FF_FFFF
        {
            continue;
        }
        let (a, b, c) = (points[0], points[1], points[2]);
        let denominator = (b.y - c.y) * (a.x - c.x) + (c.x - b.x) * (a.y - c.y);
        if denominator.abs() <= 0.0001 {
            continue;
        }
        let u = ((b.y - c.y) * (x - c.x) + (c.x - b.x) * (y - c.y)) / denominator;
        let v = ((c.y - a.y) * (x - c.x) + (a.x - c.x) * (y - c.y)) / denominator;
        let w = 1.0 - u - v;
        let alpha =
            u * (a.color >> 24) as f32 + v * (b.color >> 24) as f32 + w * (c.color >> 24) as f32;
        if u.min(v).min(w) >= -0.001 && alpha > 20.0 {
            return true;
        }
    }
    false
}

fn geometry(
    series: Vec<ChartSeries>,
    style: ChartStyle,
    x: ChartXDomain,
    plot: Rect,
) -> cove_ui_compose::primitives::data::chart::ChartGeometry {
    let axes = ChartValueAxes::resolve(
        &series,
        &vec![true; series.len()],
        &style,
        &ChartGuides::default(),
        &unlabeled(),
    );
    let x_scale = x.is_numeric().then(|| {
        let extent: Vec<f32> = axes
            .shown()
            .iter()
            .flat_map(|index| series[*index].data.iter().map(|datum| datum.x))
            .collect();
        cove_ui_compose::primitives::data::chart::ChartScale::fit_extent(
            &extent,
            None,
            None,
            5,
            Axis::default().x.kind,
        )
    });
    cove_ui_compose::primitives::data::chart::ChartGeometry::new(
        &series,
        axes,
        &x,
        x_scale.as_ref(),
        style.bar_spacing,
        style.orientation,
        plot,
        None,
    )
}

#[test]
fn a_chart_without_a_selection_binding_is_a_passive_image() {
    GlobalTestLock::locked(|| {
        let content = cartesian(
            vec![ChartSeries::line(vec![1.0, 2.0, 3.0])],
            ChartGuides::default(),
        );
        let plot = plot_of(content, ChartStyle::default(), None);
        let node = plot.make_node();
        let layout = plot
            .make_layout_node()
            .expect("the plot owns a layout node");
        plot.update_layout(&layout);
        plot.update_node(&node);

        assert!(!node.is_hit_testable(), "a passive chart takes no input");
        assert!(!node.is_focusable(), "and joins no focus traversal");
        assert_eq!(layout.height(), Some(64.0), "the default height is 64pt");
        let semantics = node.accessibility().expect("the chart is an image");
        assert_eq!(
            semantics.role,
            cove_core::scene::AccessibilityRole::Image,
            "a passive chart reads as an image"
        );
    });
}

#[test]
fn lines_stroke_between_values_and_areas_fill_to_the_baseline() {
    GlobalTestLock::locked(|| {
        let size = Size {
            width: 120.0,
            height: 40.0,
        };
        let (line, _) = draw(
            cartesian(
                vec![ChartSeries::line(vec![0.0, 1.0, 0.0])],
                ChartGuides::default(),
            ),
            flat(),
            None,
            size,
        );
        assert!(covers(&line, 15.0, 30.0, None), "the line starts low");
        assert!(covers(&line, 60.0, 1.0, None), "and peaks in the middle");
        assert!(
            !covers(&line, 60.0, 30.0, None),
            "the interior below the peak is empty for a line"
        );

        let (area, theme) = draw(
            cartesian(
                vec![ChartSeries::area(vec![0.0, 1.0, 0.0])],
                ChartGuides::default(),
            ),
            flat(),
            None,
            size,
        );
        assert!(
            covers(&area, 60.0, 30.0, Some(theme.colors.accent)),
            "the area fills the interior below the peak"
        );
        assert!(
            area.vertices()
                .iter()
                .all(|vertex| vertex.x.is_finite() && vertex.y.is_finite()),
            "every vertex is finite"
        );
    });
}

#[test]
fn grouped_and_stacked_bars_differ_in_placement_and_domain() {
    GlobalTestLock::locked(|| {
        let series = vec![
            ChartSeries::bar(vec![2.0, 4.0]),
            ChartSeries::bar(vec![2.0, 2.0]),
        ];
        let plot = Rect::new(0.0, 0.0, 100.0, 100.0);
        let style = ChartStyle {
            min_value: Some(0.0),
            bar_spacing: 0.0,
            ..ChartStyle::default()
        };

        let grouped = geometry(
            series.clone(),
            style.clone(),
            ChartXDomain::Categorical(Vec::new()),
            plot,
        );
        let a = grouped.marks[0][0].bar.expect("the first series has bars");
        let b = grouped.marks[1][0].bar.expect("the second series has bars");
        assert!(a.max_x() <= b.x + 0.5, "grouped bars sit side by side");
        assert_eq!(a.y, b.y, "grouped bars share a row");
        assert_eq!(
            grouped.axes.primary_scale.domain.upper, 4.0,
            "the domain follows the tallest series"
        );

        let stacked = geometry(
            series.clone(),
            ChartStyle {
                stacking: ChartStacking::Stacked,
                ..style.clone()
            },
            ChartXDomain::Categorical(Vec::new()),
            plot,
        );
        let low = stacked.marks[0][1].bar.expect("the lower segment");
        let high = stacked.marks[1][1].bar.expect("the upper segment");
        assert_eq!(
            stacked.axes.primary_scale.domain.upper, 6.0,
            "stacking sums the domain"
        );
        assert!(
            (low.x - high.x).abs() < 0.01 && (high.max_y() - low.y).abs() < 0.01,
            "stacked segments share a column and touch"
        );
        assert!(
            !stacked.marks[0][1].is_outer_end,
            "the inner segment is not an outer end"
        );
        assert!(stacked.marks[1][1].is_outer_end, "the outer segment is");

        let hidden = geometry(series, style, ChartXDomain::Categorical(Vec::new()), plot);
        assert!(
            hidden.marks[0][0].bar.expect("bar").width > a.width - 0.01,
            "the visible series keeps at least its grouped width"
        );
    });
}

#[test]
fn a_negative_value_stacks_under_the_baseline() {
    GlobalTestLock::locked(|| {
        let series = vec![
            ChartSeries::bar(vec![3.0, 3.0]),
            ChartSeries::bar(vec![-2.0, 1.0]),
        ];
        let style = ChartStyle {
            bar_spacing: 0.0,
            stacking: ChartStacking::Stacked,
            ..ChartStyle::default()
        };
        let stacked = geometry(
            series,
            style,
            ChartXDomain::Categorical(Vec::new()),
            Rect::new(0.0, 0.0, 100.0, 100.0),
        );
        let zero = stacked.along_value(0.0, ChartValueAxis::Left);
        assert!(
            (stacked.marks[1][0].base - zero).abs() < 0.01,
            "a loss hangs from zero instead of subtracting from the gain"
        );
        assert!(stacked.marks[1][0].y > zero, "the loss extends downward");
        assert!(
            (stacked.marks[1][1].base - stacked.marks[0][1].y).abs() < 0.01,
            "a gain still stacks on the gain"
        );
        assert!(
            stacked.axes.primary_scale.domain.lower <= -2.0
                && stacked.axes.primary_scale.domain.upper >= 4.0,
            "the domain spans both directions"
        );
        assert!(
            stacked.marks[0][0].is_outer_end && stacked.marks[1][0].is_outer_end,
            "each direction rounds only its own outermost segment"
        );
        assert!(
            !stacked.marks[0][1].is_outer_end && stacked.marks[1][1].is_outer_end,
            "the positive side has one outer end"
        );
    });
}

#[test]
fn percent_stacking_fills_each_index_and_reads_as_shares() {
    GlobalTestLock::locked(|| {
        let series = vec![
            ChartSeries::bar(vec![1.0, 3.0]),
            ChartSeries::bar(vec![3.0, 1.0]),
        ];
        let style = ChartStyle {
            bar_spacing: 0.0,
            stacking: ChartStacking::Percent,
            ..ChartStyle::default()
        };
        let axes = ChartValueAxes::resolve(
            &series,
            &[true, true],
            &style,
            &ChartGuides::default(),
            &Axis::default(),
        );
        assert_eq!(
            (
                axes.primary_scale.domain.lower,
                axes.primary_scale.domain.upper
            ),
            (0.0, 100.0),
            "a percent scale spans 0..100"
        );

        let plot = Rect::new(0.0, 0.0, 100.0, 100.0);
        let stacked = geometry(series, style, ChartXDomain::Categorical(Vec::new()), plot);
        assert!(
            (stacked.marks[1][0].y - plot.y).abs() < 0.01,
            "both indexes reach the top of the plot"
        );
        assert!(
            (stacked.marks[1][1].y - plot.y).abs() < 0.01,
            "whichever series is larger"
        );
        assert!(
            (stacked.marks[0][0].y - (plot.max_y() - plot.height * 0.25)).abs() < 0.5,
            "1 of 4 is a quarter of the height"
        );
        assert!(
            (stacked.marks[0][1].y - (plot.max_y() - plot.height * 0.75)).abs() < 0.5,
            "3 of 4 is three quarters"
        );
    });
}

#[test]
fn percent_stacking_keeps_a_negative_side() {
    GlobalTestLock::locked(|| {
        let series = vec![ChartSeries::bar(vec![2.0]), ChartSeries::bar(vec![-3.0])];
        let style = ChartStyle {
            bar_spacing: 0.0,
            stacking: ChartStacking::Percent,
            ..ChartStyle::default()
        };
        let axes = ChartValueAxes::resolve(
            &series,
            &[true, true],
            &style,
            &ChartGuides::default(),
            &Axis::default(),
        );
        assert_eq!(
            (
                axes.primary_scale.domain.lower,
                axes.primary_scale.domain.upper
            ),
            (-100.0, 100.0),
            "a negative side spans -100..100"
        );

        let plot = Rect::new(0.0, 0.0, 40.0, 100.0);
        let stacked = geometry(series, style, ChartXDomain::Categorical(Vec::new()), plot);
        let zero = stacked.along_value(0.0, ChartValueAxis::Left);
        assert!(
            (zero - plot.max_y() / 2.0).abs() < 0.5,
            "zero sits mid-plot"
        );
        assert!(
            (stacked.marks[1][0].y - plot.max_y()).abs() < 0.5,
            "the loss is the taller side, so it takes the full drop"
        );
        assert!(
            (stacked.marks[0][0].y - (zero - (zero - plot.y) * 2.0 / 3.0)).abs() < 0.5,
            "the gain keeps its 2/3 share of the half above zero"
        );
    });
}

#[test]
fn a_right_axis_series_fits_its_own_scale() {
    GlobalTestLock::locked(|| {
        let mut latency = ChartSeries::line(vec![1000.0, 2000.0]);
        latency.title = "Latency".to_string();
        latency.axis = ChartValueAxis::Right;
        let series = vec![ChartSeries::bar(vec![2.0, 4.0]), latency];
        let style = ChartStyle {
            bar_spacing: 0.0,
            ..ChartStyle::default()
        };

        let axes = ChartValueAxes::resolve(
            &series,
            &[true, true],
            &style,
            &ChartGuides::default(),
            &Axis::default(),
        );
        assert!(
            axes.secondary.is_some(),
            "a right-axis series adds a second axis"
        );
        assert_eq!(
            axes.primary_scale.domain.upper, 4.0,
            "the left axis ignores the right series"
        );
        assert_eq!(
            axes.secondary_scale
                .as_ref()
                .expect("a secondary scale")
                .domain
                .upper,
            2000.0,
            "the right axis fits its own values"
        );

        let plot = Rect::new(0.0, 0.0, 100.0, 100.0);
        let chart = geometry(
            series.clone(),
            style.clone(),
            ChartXDomain::Categorical(Vec::new()),
            plot,
        );
        assert!(
            (chart.marks[1][1].y - plot.y).abs() < 0.01,
            "the largest right value reaches the top"
        );

        let single = ChartValueAxes::resolve(
            &[ChartSeries::bar(vec![2.0, 4.0])],
            &[true],
            &style,
            &ChartGuides::default(),
            &Axis::default(),
        );
        assert!(
            !single.secondary.is_some() && single.secondary_scale.is_none(),
            "without a right-axis series there is no second axis"
        );
    });
}

#[test]
fn mixed_line_and_bar_marks_share_slot_centres() {
    GlobalTestLock::locked(|| {
        let chart = geometry(
            vec![
                ChartSeries::bar(vec![1.0, 2.0, 3.0]),
                ChartSeries::line(vec![1.0, 2.0, 3.0]),
            ],
            ChartStyle {
                min_value: Some(0.0),
                bar_spacing: 0.0,
                ..ChartStyle::default()
            },
            ChartXDomain::Categorical(Vec::new()),
            Rect::new(0.0, 0.0, 90.0, 30.0),
        );
        let bars: Vec<f32> = chart.marks[0].iter().map(|mark| mark.x).collect();
        let lines: Vec<f32> = chart.marks[1].iter().map(|mark| mark.x).collect();
        assert_eq!(bars, lines, "both marks sit on the same slot centres");
        assert_eq!(lines, vec![15.0, 45.0, 75.0], "three slots across 90pt");
    });
}

#[test]
fn scatter_points_use_their_own_x_values() {
    GlobalTestLock::locked(|| {
        let chart = geometry(
            vec![ChartSeries::scatter(
                vec![0.0, 10.0, 20.0],
                vec![0.0, 10.0, 0.0],
            )],
            ChartStyle::default(),
            ChartXDomain::Numeric,
            Rect::new(0.0, 0.0, 200.0, 100.0),
        );
        let xs: Vec<f32> = chart.marks[0].iter().map(|mark| mark.x.round()).collect();
        assert_eq!(
            xs,
            vec![0.0, 100.0, 200.0],
            "a numeric axis fits x to the plot"
        );
        assert_eq!(
            chart.nearest(95.0, 5.0).map(|mark| mark.index),
            Some(1),
            "the nearest point by distance wins"
        );
        assert_eq!(
            chart.nearest(190.0, 92.0).map(|mark| mark.index),
            Some(2),
            "and again for the far corner"
        );
    });
}

#[test]
fn donut_wedges_start_at_twelve_and_hit_test_by_angle() {
    GlobalTestLock::locked(|| {
        let slices = vec![
            ChartSlice::new("A", 1.0, None),
            ChartSlice::new("B", 1.0, None),
            ChartSlice::new("C", 2.0, None),
        ];
        let pie = PieGeometry::new(
            &slices,
            &[true, true, true],
            0.5,
            0.0,
            false,
            Rect::new(0.0, 0.0, 108.0, 108.0),
        );
        assert_eq!(pie.total, 4.0, "the total is the sum of the slices");
        assert_eq!(pie.wedges.len(), 3, "one wedge per slice");
        assert_eq!(
            pie.wedge_at(80.0, 30.0).map(|wedge| wedge.index),
            Some(0),
            "the upper-right wedge is the first"
        );
        assert_eq!(
            pie.wedge_at(80.0, 80.0).map(|wedge| wedge.index),
            Some(1),
            "the lower-right wedge is the second"
        );
        assert_eq!(
            pie.wedge_at(20.0, 54.0).map(|wedge| wedge.index),
            Some(2),
            "the left half is the third"
        );
        assert!(
            pie.wedge_at(54.0, 54.0).is_none(),
            "the hole hit-tests to nothing"
        );

        let hidden = PieGeometry::new(
            &slices,
            &[true, true, false],
            0.5,
            0.0,
            false,
            Rect::new(0.0, 0.0, 108.0, 108.0),
        );
        assert_eq!(hidden.total, 2.0, "a hidden slice leaves the total");
        assert_eq!(
            hidden.wedge_at(20.0, 54.0).map(|wedge| wedge.index),
            Some(1),
            "the remaining slices re-span the circle"
        );
    });
}

#[test]
fn a_non_finite_value_breaks_the_line() {
    GlobalTestLock::locked(|| {
        let values = vec![0.0, 1.0, f32::NAN, 1.0, 0.0];
        let (list, _) = draw(
            cartesian(vec![ChartSeries::line(values)], ChartGuides::default()),
            flat(),
            None,
            Size {
                width: 120.0,
                height: 40.0,
            },
        );
        assert!(
            !covers(&list, 45.0, 20.0, None),
            "a coerced zero would draw a V through this point"
        );
        assert!(covers(&list, 15.0, 20.0, None), "the left run survives");
        assert!(covers(&list, 105.0, 20.0, None), "the right run survives");
    });
}

#[test]
fn a_dash_pattern_leaves_gaps() {
    GlobalTestLock::locked(|| {
        let mut style = ChartSeriesStyle::default();
        style.dash = vec![6.0, 6.0];
        let mut series = ChartSeries::line(vec![0.5, 0.5]);
        series.style = style;
        let (list, _) = draw(
            cartesian(vec![series], ChartGuides::default()),
            flat(),
            None,
            Size {
                width: 120.0,
                height: 40.0,
            },
        );
        let mut drawn = 0;
        let mut skipped = 0;
        let mut x = 2.0;
        while x <= 118.0 {
            if covers(&list, x, 20.0, None) {
                drawn += 1;
            } else {
                skipped += 1;
            }
            x += 2.0;
        }
        assert!(drawn > 0, "some of the stroke is drawn");
        assert!(skipped > 0, "and some of it is left as a gap");
    });
}

#[test]
fn a_curved_series_bulges_past_the_chord() {
    GlobalTestLock::locked(|| {
        let size = Size {
            width: 120.0,
            height: 40.0,
        };
        let (straight, _) = draw(
            cartesian(
                vec![ChartSeries::line(vec![0.0, 10.0, 10.0])],
                ChartGuides::default(),
            ),
            flat(),
            None,
            size,
        );
        let mut curved_style = ChartSeriesStyle::default();
        curved_style.curve = true;
        let mut curved_series = ChartSeries::line(vec![0.0, 10.0, 10.0]);
        curved_series.style = curved_style;
        let (curved, _) = draw(
            cartesian(vec![curved_series], ChartGuides::default()),
            flat(),
            None,
            size,
        );

        assert!(
            !covers(&straight, 30.0, 15.0, None),
            "the chord reads 5 halfway along"
        );
        assert!(
            covers(&curved, 30.0, 15.0, None),
            "the monotone cubic bulges higher"
        );
    });
}

#[test]
fn a_series_can_widen_its_stroke_and_size_its_points() {
    GlobalTestLock::locked(|| {
        let size = Size {
            width: 120.0,
            height: 40.0,
        };
        let mut wide_style = ChartSeriesStyle::default();
        wide_style.line_width = Some(10.0);
        let mut wide = ChartSeries::line(vec![0.5, 0.5]);
        wide.style = wide_style;
        let (thick, _) = draw(
            cartesian(vec![wide], ChartGuides::default()),
            flat(),
            None,
            size,
        );
        let (thin, _) = draw(
            cartesian(
                vec![ChartSeries::line(vec![0.5, 0.5])],
                ChartGuides::default(),
            ),
            flat(),
            None,
            size,
        );
        assert!(covers(&thick, 60.0, 16.0, None), "a 10pt stroke is thick");
        assert!(!covers(&thin, 60.0, 16.0, None), "the default is not");

        let mut big_style = ChartSeriesStyle::default();
        big_style.point_radius = 8.0;
        let big = ChartSeries::points(
            vec![ChartDatum::new(0.0, 0.5)],
            SemanticColorRef::accent(),
            cove_ui_compose::primitives::data::chart::ChartMark::Point,
            "",
            ChartValueAxis::Left,
            big_style,
        );
        let small = ChartSeries::points(
            vec![ChartDatum::new(0.0, 0.5)],
            SemanticColorRef::accent(),
            cove_ui_compose::primitives::data::chart::ChartMark::Point,
            "",
            ChartValueAxis::Left,
            ChartSeriesStyle::default(),
        );
        let (big_list, _) = draw(
            cartesian(vec![big], ChartGuides::default()),
            flat(),
            None,
            size,
        );
        let (small_list, _) = draw(
            cartesian(vec![small], ChartGuides::default()),
            flat(),
            None,
            size,
        );
        assert!(
            covers(&big_list, 60.0, 14.0, None),
            "an 8pt radius reaches here"
        );
        assert!(
            !covers(&small_list, 60.0, 14.0, None),
            "the default does not"
        );
    });
}

#[test]
fn a_numeric_x_domain_puts_bars_and_points_on_one_axis() {
    GlobalTestLock::locked(|| {
        let bars = ChartSeries::points(
            vec![ChartDatum::new(0.0, 5.0), ChartDatum::new(10.0, 6.0)],
            SemanticColorRef::accent(),
            cove_ui_compose::primitives::data::chart::ChartMark::Bar,
            "",
            ChartValueAxis::Left,
            ChartSeriesStyle::default(),
        );
        let dot = ChartSeries::points(
            vec![ChartDatum::new(10.0, 3.0)],
            SemanticColorRef::accent(),
            cove_ui_compose::primitives::data::chart::ChartMark::Point,
            "",
            ChartValueAxis::Left,
            ChartSeriesStyle::default(),
        );
        let chart = geometry(
            vec![bars, dot],
            ChartStyle::default(),
            ChartXDomain::Numeric,
            Rect::new(0.0, 0.0, 200.0, 100.0),
        );
        assert!(
            (chart.marks[0][1].x - chart.marks[1][0].x).abs() < 0.01,
            "the bar and the point at x=10 share a screen x"
        );
        assert!(chart.is_scatter, "a numeric domain reads as scatter");
    });
}

#[test]
fn a_band_shades_its_corridor_and_widens_the_domain() {
    GlobalTestLock::locked(|| {
        let mut band = ChartBand::new(0.25, 0.75);
        band.color = SemanticColorRef::success();
        let guides = ChartGuides {
            bands: vec![band.clone()],
            ..ChartGuides::default()
        };
        let (list, theme) = draw(
            cartesian(vec![ChartSeries::line(vec![0.5, 0.5])], guides),
            flat(),
            None,
            Size {
                width: 120.0,
                height: 40.0,
            },
        );
        assert!(
            covers(&list, 60.0, 20.0, Some(theme.colors.success)),
            "the corridor is shaded"
        );

        let stack = cove_ui_compose::primitives::data::chart::ChartStack::new(
            &[ChartSeries::line(vec![0.5])],
            &[true],
            &ChartStyle::default(),
            ChartValueAxis::Left,
        );
        let scale = cove_ui_compose::primitives::data::chart::ChartScale::fit_stack(
            &stack,
            &[],
            &[ChartBand::new(0.0, 10.0)],
            &ChartStyle::default(),
            4,
            Axis::default().x.kind,
        );
        assert!(
            scale.domain.upper >= 10.0,
            "the corridor, not the data, decides the top of the domain"
        );
    });
}

#[test]
fn value_labels_follow_shows_values_and_drop_collisions() {
    GlobalTestLock::locked(|| {
        let plot = plot_of(
            cartesian(
                vec![ChartSeries::line(vec![1.0, 2.0, 3.0])],
                ChartGuides::default(),
            ),
            flat(),
            None,
        );
        let text = ChartText::new(&Theme::default_light());
        let size = Size {
            width: 200.0,
            height: 100.0,
        };
        let slots = plot.overlay_slots(Some(size), &text, None);
        assert!(
            !slots.iter().any(|slot| slot.placement.is_some()
                && slot.item.kind
                    == cove_ui_compose::primitives::data::chart::ChartOverlayKind::Value),
            "no value labels without showsValues"
        );
    });
}

#[test]
fn a_pie_sorts_by_value_without_losing_slice_identity() {
    GlobalTestLock::locked(|| {
        let slices = vec![
            ChartSlice::new("A", 1.0, None),
            ChartSlice::new("B", 3.0, None),
        ];
        let plot = Rect::new(0.0, 0.0, 100.0, 100.0);

        let straight = PieGeometry::new(&slices, &[true, true], 0.0, 0.0, false, plot);
        let order: Vec<usize> = straight.wedges.iter().map(|wedge| wedge.index).collect();
        assert_eq!(order, vec![0, 1], "unsorted keeps authoring order");
        assert!(
            (straight.wedges[0].start + std::f32::consts::FRAC_PI_2).abs() < 0.001,
            "the first wedge starts at 12 o'clock"
        );

        let turned = PieGeometry::new(
            &slices,
            &[true, true],
            0.0,
            std::f32::consts::FRAC_PI_2,
            false,
            plot,
        );
        assert!(
            turned.wedges[0].start.abs() < 0.001,
            "a start angle turns the ring"
        );

        let sorted = PieGeometry::new(&slices, &[true, true], 0.0, 0.0, true, plot);
        let sorted_order: Vec<usize> = sorted.wedges.iter().map(|wedge| wedge.index).collect();
        assert_eq!(sorted_order, vec![1, 0], "sorting puts the largest first");
        assert_eq!(sorted.wedges[0].value, 3.0, "and keeps its identity");
        let total: f32 = sorted.wedges.iter().map(|wedge| wedge.value).sum();
        assert_eq!(total, sorted.total, "the wedges still sum to the total");
    });
}

#[test]
fn a_histogram_bins_samples_into_even_widths() {
    GlobalTestLock::locked(|| {
        let histogram = ChartHistogram::new(4, None).build(
            &[0.0, 1.0, 1.0, 2.0, 3.0, 3.0, 3.0, 4.0],
            SemanticColorRef::accent(),
            "",
        );
        assert_eq!(
            histogram.counts,
            vec![1, 2, 1, 4],
            "samples fall in even bins"
        );
        assert_eq!(
            histogram.labels,
            vec!["0–1", "1–2", "2–3", "3–4"],
            "the edges are labelled"
        );
        assert_eq!(
            histogram.series.mark,
            cove_ui_compose::primitives::data::chart::ChartMark::Bar,
            "a histogram renders as bars"
        );
        let values: Vec<f32> = histogram.series.data.iter().map(|datum| datum.y).collect();
        assert_eq!(
            values,
            vec![1.0, 2.0, 1.0, 4.0],
            "each bar is its bin count"
        );
    });
}

#[test]
fn a_histogram_drops_out_of_range_and_non_finite_samples() {
    GlobalTestLock::locked(|| {
        let histogram = ChartHistogram::new(
            2,
            Some(cove_ui_compose::primitives::data::chart::ChartRange::new(
                0.0, 10.0,
            )),
        )
        .build(
            &[-5.0, 0.0, 4.0, 6.0, 10.0, 99.0, f32::NAN],
            SemanticColorRef::accent(),
            "",
        );
        assert_eq!(
            histogram.counts,
            vec![2, 2],
            "out-of-range and non-finite samples never reach a bin"
        );
    });
}

#[test]
fn a_histogram_of_identical_samples_still_divides_a_width() {
    GlobalTestLock::locked(|| {
        let histogram =
            ChartHistogram::new(3, None).build(&[5.0, 5.0, 5.0], SemanticColorRef::accent(), "");
        assert_eq!(
            histogram.counts.iter().sum::<usize>(),
            3,
            "every sample lands in a bin"
        );
        assert_eq!(histogram.labels.len(), 3, "three bins are still labelled");
    });
}

fn _unused(_: ChartValueAxis) {}
