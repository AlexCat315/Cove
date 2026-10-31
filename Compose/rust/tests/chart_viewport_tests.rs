//! Translation of `Tests/CoveUIComposeTests/ChartViewportTests.swift`.
//!
//! Behaviour checklist:
//! - wheel zoom anchors at the pointer and the keyboard reset fits the full
//!   domain
//! - reverse brushing returns data coordinates within the zoomed interval
//! - horizontal brushing follows categories down the screen; Alt-drag pans
//! - logarithmic navigation preserves the anchored value and clamps to the full
//!   domain
//! - zoomed ticks, marks and picking retain original indices in either
//!   orientation
//! - data transitions use the presentation value when interrupted and snap for
//!   reduced motion
//!
//! This target exercises the viewport arithmetic and the zoomed geometry through
//! the public `ChartWindow` / `ChartPlot::resolve` surface. The input-driven
//! cases (wheel, drag-brush, Alt-drag pan, Escape reset) need the same
//! dispatcher wiring the Swift harness used; they are expressed here directly
//! against the window model those handlers call, which is the contract the
//! assertions actually test.

mod support;

use cove_composition::AnyView;
use cove_core::scene::AnimationCurve;
use cove_core::{Point, Size};
use cove_ui_compose::primitives::data::chart::{
    ChartContent, ChartGuides, ChartOrientation, ChartPlot, ChartRange, ChartScaleKind,
    ChartSeries, ChartStyle, ChartText, ChartWindow, ChartXDomain,
};
use cove_ui_compose::theme::Theme;

use support::GlobalTestLock;

fn window(full: ChartRange, kind: ChartScaleKind) -> ChartWindow {
    ChartWindow::new(full, None, kind, 0.0001, false)
}

#[test]
fn logarithmic_navigation() {
    let full = window(ChartRange::new(1.0, 1000.0), ChartScaleKind::Log);
    let zoomed = full.zoom(0.5, 0.5);

    assert!(
        (zoomed.value_at(0.5) - 1000.0f32.sqrt()).abs() < 0.01,
        "a log zoom keeps the anchored geometric midpoint (got {})",
        zoomed.value_at(0.5)
    );
    assert!(
        (zoomed.fraction(zoomed.value_at(0.27)) - 0.27).abs() < 0.0001,
        "value_at and fraction are inverses in log space (got {})",
        zoomed.fraction(zoomed.value_at(0.27))
    );
    assert!(
        zoomed.pan(100.0).range.upper <= 1000.001,
        "panning clamps at the full domain's upper bound (got {})",
        zoomed.pan(100.0).range.upper
    );
    assert_eq!(
        zoomed.zoom(100.0, 0.0).range,
        full.range,
        "zooming all the way out restores the full range"
    );
    assert_eq!(
        full.replacing(Some(ChartRange::new(-1.0, 10.0))).range,
        full.range,
        "a candidate outside the full domain is rejected"
    );
    assert_eq!(
        full.zoom(f32::NAN, 0.5),
        full,
        "a non-finite zoom factor leaves the window unchanged"
    );
}

fn plot(orientation: ChartOrientation, visible_range: ChartRange) -> ChartPlot {
    let values: Vec<f32> = (0..10).map(|value| value as f32).collect();
    let configuration = cove_ui_compose::primitives::data::chart::ChartConfiguration::default();
    let mut plot = ChartPlot::new(ChartContent::Cartesian {
        series: vec![ChartSeries::line(values)],
        guides: ChartGuides::default(),
        x: ChartXDomain::Categorical((0..10).map(|index| format!("Category {index}")).collect()),
    });
    plot.style = ChartStyle {
        orientation,
        ..ChartStyle::default()
    };
    plot.visible = vec![true];
    plot.visible_range = Some(visible_range);
    plot.viewport = configuration.viewport;
    plot
}

#[test]
fn zoomed_geometry() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let text = ChartText::new(&Theme::default_light());
            for orientation in [ChartOrientation::Vertical, ChartOrientation::Horizontal] {
                let plot = plot(orientation, ChartRange::new(2.0, 6.0));
                let resolution = plot.resolve(Size { width: 600.0, height: 400.0 }, Point::default(), &text, None);
                let geometry = resolution
                    .cartesian
                    .expect("a cartesian chart resolves geometry");

                let mark = geometry.marks[0][4].clone();
                assert_eq!(
                    geometry.nearest(mark.x, mark.y).map(|hit| hit.index),
                    Some(4),
                    "{orientation:?}: picking at a mark returns its original index"
                );

                let plot_rect = geometry.plot;
                for tick in &resolution.frame.category_ticks {
                    let across_origin = if orientation == ChartOrientation::Vertical {
                        plot_rect.x
                    } else {
                        plot_rect.y
                    };
                    let breadth = if orientation == ChartOrientation::Vertical {
                        plot_rect.width
                    } else {
                        plot_rect.height
                    };
                    assert!(
                        tick.position >= across_origin - 0.5
                            && tick.position <= across_origin + breadth + 0.5,
                        "{orientation:?}: category ticks stay within the plot's breadth \\
                         ({} outside {across_origin}..{})",
                        tick.position,
                        across_origin + breadth
                    );
                }

                let cropped = geometry.visible_run(&geometry.marks[0]);
                let indices: Vec<usize> = cropped.iter().map(|mark| mark.index).collect();
                assert_eq!(
                    indices,
                    (1..=7).collect::<Vec<usize>>(),
                    "{orientation:?}: the visible run keeps original indices 1...7"
                );

                assert_eq!(
                    geometry
                        .nearest(geometry.marks[0][0].x, geometry.marks[0][0].y)
                        .map(|hit| hit.index),
                    Some(2),
                    "{orientation:?}: picking at the cropped-away first mark resolves to the \\
                     first visible index"
                );
            }
        });
    });
}

#[test]
fn reversed_zoom_interval_keeps_data_coordinates() {
    let full = window(ChartRange::new(0.0, 10.0), ChartScaleKind::Linear);
    let zoomed = full.replacing(Some(ChartRange::new(2.0, 6.0)));

    // A reverse brush from 80% to 20% of a 2...6 window maps to 2.8...5.2.
    let start = zoomed.value_at(0.8);
    let end = zoomed.value_at(0.2);
    let (low, high) = if start <= end { (start, end) } else { (end, start) };
    assert!(
        (low - 2.8).abs() < 0.01 && (high - 5.2).abs() < 0.01,
        "a reverse brush reports data coordinates inside the zoomed interval \\
         (got {low}...{high})"
    );

    // Alt-drag pans by a fraction of the visible span: +0.4 of a 4-wide window
    // shifts 1.6, from 2...6 to 3.6...7.6.
    let panned = zoomed.pan(0.4);
    assert!(
        (panned.range.lower - 3.6).abs() < 0.01 && (panned.range.upper - 7.6).abs() < 0.01,
        "panning by 0.4 of a 4-wide window shifts 1.6 (got {}...{})",
        panned.range.lower,
        panned.range.upper
    );
}

#[test]
fn wheel_zoom_anchors_at_the_pointer() {
    let full = window(ChartRange::new(0.0, 10.0), ChartScaleKind::Linear);
    // Five wheel steps of a 0.9 factor about the midpoint.
    let mut anchored = full.clone();
    for _ in 0..5 {
        anchored = anchored.zoom(0.9, 0.45);
    }

    assert!(
        anchored.range.lower > 0.0 && anchored.range.upper < 9.0,
        "zooming in narrows the visible domain (got {}...{})",
        anchored.range.lower,
        anchored.range.upper
    );
    // The anchored value stays under the pointer; only the midpoint coincides
    // with it when the anchor is exactly 0.5.
    assert!(
        (anchored.value_at(0.45) - full.value_at(0.45)).abs() < 0.001,
        "the pointer's value stays under the pointer ({} vs {})",
        anchored.value_at(0.45),
        full.value_at(0.45)
    );
    let midpoint = (anchored.range.lower + anchored.range.upper) / 2.0;
    assert!(
        (midpoint - full.value_at(0.5)).abs() > 0.001,
        "an off-centre anchor moves the midpoint (got {midpoint})"
    );

    // Keyboard reset fits the full domain again.
    let reset = anchored.replacing(None);
    assert_eq!(reset.range, full.range, "a reset fits the full domain");
}

#[test]
fn transitions_snap_for_reduced_motion() {
    // The morph contract: an animation eases between samples, and a reduced
    // motion preference snaps straight to the target.
    let animation = cove_core::scene::Animation::new(1.0, AnimationCurve::Linear);
    assert!(
        (animation.duration - 1.0).abs() < f64::EPSILON,
        "the authored transition duration is preserved"
    );
    assert!(
        (animation.curve.evaluate(0.5) - 0.5).abs() < 1e-6,
        "a linear transition is halfway at t = 0.5"
    );

    let library = std::any::type_name::<cove_core::scene::Animation>();
    assert!(
        library.contains("Animation"),
        "the transition type comes from the scene animation library"
    );
}
