//! Translation of `Tests/CoveUIComposeTests/ChartPerformanceTests.swift`.
//!
//! Behaviour checklist:
//! - a dense run keeps its envelope while shedding most of its points, and both
//!   extremes survive exactly
//! - a lone spike survives decimation
//! - a run that already fits the plot is left alone
//! - a dense line draws segments per pixel, not per sample
//! - drawing and hit-testing share one resolution
//! - the two spaces a chart lives in keep separate cache entries
//! - new data drops the cached geometry; a new selection keeps it

mod support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::PrimitiveView;
use cove_core::scene::{Direction, Node};
use cove_core::{Point, Rect, Size};
use cove_ui_compose::primitives::data::chart::axes::ChartValueAxes;
use cove_ui_compose::primitives::data::chart::axis::ChartText;
use cove_ui_compose::primitives::data::chart::cache::ChartResolutionCache;
use cove_ui_compose::primitives::data::chart::coordinates::{ChartGeometry, Mark};
use cove_ui_compose::primitives::data::chart::{
    Axis, ChartContent, ChartGuides, ChartOrientation, ChartPlot, ChartPlotInput, ChartPoint,
    ChartSeries, ChartSession, ChartStyle, ChartXDomain,
};
use cove_ui_compose::theme::Theme;

use support::GlobalTestLock;

/// Both axes unlabeled, so the plot is exactly the size the test asks for.
fn unlabeled() -> Axis {
    let mut axis = Axis::default();
    axis.x.is_labeled = false;
    axis.y.is_labeled = false;
    axis
}

fn plot(series: Vec<ChartSeries>, selection: Option<ChartPoint>, style: ChartStyle) -> ChartPlot {
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

fn flat_style() -> ChartStyle {
    ChartStyle {
        shows_grid: false,
        content_inset: 0.0,
        ..ChartStyle::default()
    }
}

/// The marks one series resolves to at a given plot width.
fn marks(values: Vec<f32>, width: f32) -> Vec<Mark> {
    let series = vec![ChartSeries::line(values)];
    let style = flat_style();
    let axes = ChartValueAxes::resolve(&series, &[true], &style, &ChartGuides::default(), &unlabeled());
    let geometry = ChartGeometry::new(
        &series,
        axes,
        &ChartXDomain::Categorical(Vec::new()),
        None,
        style.bar_spacing,
        style.orientation,
        Rect::new(0.0, 0.0, width, 40.0),
        None,
    );
    geometry.marks[0].clone()
}

fn all_nodes(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        all_nodes(&child, into);
    }
}

#[test]
fn a_dense_run_keeps_its_envelope_while_shedding_points() {
    GlobalTestLock::locked(|| {
        let values: Vec<f32> = (0..2000)
            .map(|index| (index as f32 / 40.0).sin() * 10.0 + 20.0)
            .collect();
        let full = marks(values, 120.0);
        let reduced = ChartGeometry::decimated(&full, ChartOrientation::Vertical, 120.0);

        assert!(
            reduced.len() <= 242,
            "decimation stays within two points per column (got {})",
            reduced.len()
        );
        assert!(
            reduced.len() < full.len() / 4,
            "most points are shed ({} of {})",
            reduced.len(),
            full.len()
        );

        let min = |list: &[Mark]| list.iter().map(|mark| mark.y).fold(f32::INFINITY, f32::min);
        let max = |list: &[Mark]| {
            list.iter()
                .map(|mark| mark.y)
                .fold(f32::NEG_INFINITY, f32::max)
        };
        assert!(
            (min(&reduced) - min(&full)).abs() < 0.001,
            "per-column min/max keeps the low extreme exactly"
        );
        assert!(
            (max(&reduced) - max(&full)).abs() < 0.001,
            "per-column min/max keeps the high extreme exactly"
        );

        assert_eq!(
            reduced.first().map(|mark| mark.x),
            full.first().map(|mark| mark.x),
            "the first endpoint survives"
        );
        assert_eq!(
            reduced.last().map(|mark| mark.x),
            full.last().map(|mark| mark.x),
            "the last endpoint survives"
        );
        assert!(
            reduced.windows(2).all(|pair| pair[0].x <= pair[1].x),
            "x never goes backwards"
        );
    });
}

#[test]
fn a_lone_spike_survives_decimation() {
    GlobalTestLock::locked(|| {
        let mut values = vec![5.0_f32; 1000];
        values[500] = 100.0;
        let full = marks(values, 120.0);
        let reduced = ChartGeometry::decimated(&full, ChartOrientation::Vertical, 120.0);
        assert!(
            reduced
                .iter()
                .any(|mark| (mark.y - full[500].y).abs() < 0.001),
            "a one-sample spike is not lost between columns"
        );
    });
}

#[test]
fn a_run_that_fits_is_left_alone() {
    GlobalTestLock::locked(|| {
        let full = marks((0..40).map(|index| index as f32).collect(), 120.0);
        assert_eq!(full.len(), 40, "40 samples resolve to 40 marks");
        assert_eq!(
            ChartGeometry::decimated(&full, ChartOrientation::Vertical, 120.0).len(),
            full.len(),
            "a run that already fits is returned unchanged"
        );
    });
}

/// Triangles drawn by a series at a given plot width.
fn triangles(series: Vec<ChartSeries>, width: f32) -> usize {
    let plot = plot(
        series,
        None,
        ChartStyle {
            shows_grid: false,
            line_width: 1.0,
            content_inset: 0.0,
            ..ChartStyle::default()
        },
    );
    let node = plot.make_node();
    node.set_frame(Rect::new(0.0, 0.0, width, 40.0));
    plot.update_node(&node);
    let mut list = cove_core::DrawList::default();
    if let Some(draw) = node.draw() {
        draw(&mut list, Point::default());
    }
    list.indices().len() / 3
}

#[test]
fn a_dense_line_draws_per_pixel_not_per_sample() {
    GlobalTestLock::locked(|| {
        let dense: Vec<f32> = (0..4000)
            .map(|index| (index as f32 / 60.0).sin() * 10.0 + 20.0)
            .collect();
        let compact = triangles(vec![ChartSeries::line(dense.clone())], 120.0);
        let roomy = triangles(vec![ChartSeries::line(dense)], 4000.0);

        assert!(compact > 0, "the compact plot still draws the line");
        assert!(
            compact * 8 < roomy,
            "a 120pt plot draws far fewer triangles than a 4000pt one ({compact} vs {roomy})"
        );
    });
}

#[test]
fn drawing_and_hit_testing_share_one_resolution() {
    GlobalTestLock::locked(|| {
        let plot = plot(vec![ChartSeries::line(vec![1.0, 2.0, 3.0, 4.0])], None, ChartStyle::default());
        let cache = Rc::new(RefCell::new(ChartResolutionCache::new()));
        let text = ChartText::new(&Theme::default_light());
        let size = Size { width: 120.0, height: 40.0 };

        let _ = plot.resolve(size, Point::default(), &text, Some(cache.clone()));
        let _ = plot.resolve(size, Point::default(), &text, Some(cache.clone()));
        assert_eq!(
            cache.borrow().builds,
            1,
            "a repeated resolve reuses the cached resolution"
        );

        let _ = plot.resolve(Size { width: 200.0, height: 40.0 }, Point::default(), &text, Some(cache.clone()));
        assert_eq!(
            cache.borrow().builds,
            2,
            "a new size builds a new resolution"
        );
    });
}

#[test]
fn the_two_spaces_a_chart_lives_in_keep_separate_entries() {
    GlobalTestLock::locked(|| {
        let plot = plot(vec![ChartSeries::line(vec![1.0, 2.0, 3.0])], None, ChartStyle::default());
        let cache = Rc::new(RefCell::new(ChartResolutionCache::new()));
        let text = ChartText::new(&Theme::default_light());
        let size = Size { width: 120.0, height: 40.0 };
        let absolute = Point { x: 20.0, y: 8.0 };

        let _ = plot.resolve(size, Point::default(), &text, Some(cache.clone()));
        let _ = plot.resolve(size, absolute, &text, Some(cache.clone()));
        assert_eq!(
            cache.borrow().builds,
            2,
            "a different origin is a different space"
        );

        // Alternating between them is free.
        let _ = plot.resolve(size, Point::default(), &text, Some(cache.clone()));
        let _ = plot.resolve(size, absolute, &text, Some(cache.clone()));
        assert_eq!(
            cache.borrow().builds,
            2,
            "both spaces stay cached so alternating costs nothing"
        );
    });
}

#[test]
fn new_data_drops_the_cache_and_a_selection_keeps_it() {
    GlobalTestLock::locked(|| {
        let size = Size { width: 160.0, height: 60.0 };
        let initial = plot(vec![ChartSeries::line(vec![1.0, 2.0, 3.0])], None, ChartStyle::default());
        let node = initial.make_node();
        node.set_frame(Rect::new(0.0, 0.0, size.width, size.height));
        initial.update_node(&node);
        let text = ChartText::new(&Theme::default_light());

        let session = node
            .attachment::<Rc<ChartSession>>(ChartSession::KEY)
            .expect("the plot installs a chart session");
        let cache = session.cache.clone();

        let _ = initial.resolve(size, Point::default(), &text, Some(cache.clone()));
        assert_eq!(cache.borrow().builds, 1, "the first resolve builds once");

        // A changed value moves the marks, so the cache goes.
        let changed = plot(vec![ChartSeries::line(vec![1.0, 2.0, 4.0])], None, ChartStyle::default());
        changed.update_node(&node);
        let _ = changed.resolve(size, Point::default(), &text, Some(cache.clone()));
        assert_eq!(
            cache.borrow().builds,
            2,
            "new data invalidates the cached geometry"
        );

        // A selection repaints but moves no mark, so the cache stays.
        let selected = plot(
            vec![ChartSeries::line(vec![1.0, 2.0, 4.0])],
            Some(ChartPoint::new(0, 1)),
            ChartStyle::default(),
        );
        selected.update_node(&node);
        let _ = selected.resolve(size, Point::default(), &text, Some(cache.clone()));
        assert_eq!(
            cache.borrow().builds,
            2,
            "a selection keeps the cached geometry"
        );

        let _ = Direction::Ltr;
        let mut nodes = Vec::new();
        all_nodes(&node, &mut nodes);
    });
}
