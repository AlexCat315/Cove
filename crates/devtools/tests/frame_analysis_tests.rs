//! Frame attribution from the runtime's real CPU timeline.
//!
//! A frame owns the spans recorded between the previous frame closing and its
//! own close, which is where the host's recomposition commit lands. These tests
//! drive an actual [`Recomposer`] over a [`NodeTree`], tag a captured
//! [`TimingFrame`] with the resulting window, and read the probe's
//! `analyze_ui_frame` answer. A frame without retained spans must be
//! `unavailable` with a reason, never a classification.

use cove_core::scene::{Node, NodeTree, PerformanceTimeline, Recomposer};
use cove_devtools::frame_analysis::{FrameAnalysis, FrameStatus, FrameWindow};
use cove_devtools::runtime_probe::RuntimeProbe;
use cove_devtools_protocol::TimingFrame;
use cove_devtools_protocol::control::ProbeCommand;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

fn tree() -> Rc<RefCell<NodeTree>> {
    let tree = Rc::new(RefCell::new(NodeTree::new()));
    tree.borrow_mut().set_root(Some(Node::new()));
    tree
}

fn span(timeline: &Rc<RefCell<PerformanceTimeline>>, phase: &str, name: &str) {
    let start = timeline
        .borrow()
        .begin()
        .expect("the timeline is recording");
    timeline
        .borrow_mut()
        .end(Some(start), phase, name, None, &[]);
}

fn frame() -> TimingFrame {
    TimingFrame {
        frame: 0,
        recomposition_ms: None,
        layout_ms: 0.5,
        draw_ms: 0.5,
        render_thread_ms: None,
        gpu_ms: None,
        pipeline_creation: None,
        submitted: true,
        presented: None,
        total_ms: 1.0,
        node_count: 1,
        batch_count: 1,
        timeline_start: 0,
        timeline_end: 0,
    }
}

fn analyze(probe: &mut RuntimeProbe, frame: u64) -> FrameAnalysis {
    let value = probe
        .execute(ProbeCommand::AnalyzeUiFrame { frame })
        .expect("the probe answers");
    serde_json::from_value(value).expect("the analysis matches its wire shape")
}

#[test]
fn a_recomposition_commit_is_attributed_to_its_frame() {
    let tree = tree();
    let timeline = tree.borrow().timeline.clone();
    timeline.borrow_mut().set_enabled(true);
    let recomposer = Recomposer::new();
    recomposer.set_timeline(Some(timeline.clone()));
    let mut window = FrameWindow::new(&timeline);

    let commits = Rc::new(Cell::new(0));
    let counted = commits.clone();
    recomposer.invalidate(7, None, None, move || counted.set(counted.get() + 1));
    assert!(recomposer.commit_all(), "the queued scope commits");
    assert_eq!(commits.get(), 1);
    span(&timeline, "layout", "Flex layout");
    span(&timeline, "component", "GalleryRow");
    span(&timeline, "draw", "Encode draw list");

    let attribution = window.close(&timeline);
    let recomposition_ms = attribution
        .recomposition_ms
        .expect("the commit span is measured");
    assert!(recomposition_ms >= 0.0);
    assert!(attribution.window_end > attribution.window_start);

    let mut captured = frame();
    attribution.apply(&mut captured);
    assert_eq!(captured.recomposition_ms, Some(recomposition_ms));
    assert_eq!(captured.timeline_start, attribution.window_start);
    assert_eq!(captured.timeline_end, attribution.window_end);

    let mut probe = RuntimeProbe::new(tree.clone(), None, None, None);
    probe.record(captured);
    let analysis = analyze(&mut probe, 1);

    assert_eq!(analysis.status, FrameStatus::Ok, "{analysis:?}");
    assert_eq!(
        analysis.window.unwrap().end_sequence,
        attribution.window_end
    );
    assert!(
        analysis
            .spans
            .iter()
            .any(|span| span.phase == "recomposition"),
        "the commit span is returned as evidence: {:?}",
        analysis.spans
    );
    assert!(analysis.spans.iter().any(|span| span.phase == "layout"));
    assert!(analysis.spans.iter().any(|span| span.phase == "draw"));
    let recomposition = analysis
        .phases
        .iter()
        .find(|phase| phase.phase == "recomposition")
        .expect("the commit and component spans roll up into recomposition");
    assert_eq!(recomposition.span_count, 2);
    // The frame's own recomposition measurement counts only the commit span,
    // so it can never exceed the phase total that also holds the component.
    assert!(recomposition.cpu_ms + 1e-9 >= recomposition_ms);
    let maximum = analysis
        .phases
        .iter()
        .map(|phase| phase.cpu_ms)
        .fold(f64::MIN, f64::max);
    let dominant = analysis.dominant_phase.expect("a phase dominates");
    let dominant_ms = analysis
        .phases
        .iter()
        .find(|phase| phase.phase == dominant)
        .expect("the dominant phase is summarised")
        .cpu_ms;
    assert!(
        (dominant_ms - maximum).abs() < 1e-9,
        "the dominant phase is the largest measured total"
    );
}

#[test]
fn a_frame_prepared_without_a_capture_has_no_window_and_says_so() {
    let tree = tree();
    let timeline = tree.borrow().timeline.clone();
    let mut window = FrameWindow::new(&timeline);
    let attribution = window.close(&timeline);
    assert!(
        attribution.recomposition_ms.is_none(),
        "an idle timeline records no commit span"
    );
    assert_eq!(attribution.window_start, attribution.window_end);

    let mut captured = frame();
    attribution.apply(&mut captured);
    let mut probe = RuntimeProbe::new(tree.clone(), None, None, None);
    probe.record(captured);
    let analysis = analyze(&mut probe, 1);

    assert_eq!(analysis.status, FrameStatus::Unavailable);
    assert_eq!(analysis.dominant_phase, None);
    assert!(analysis.spans.is_empty());
    assert!(
        analysis
            .reason
            .as_deref()
            .unwrap_or("")
            .contains("no trace window"),
        "{analysis:?}"
    );
}

#[test]
fn spans_cleared_after_the_capture_report_expiry_not_a_guess() {
    let tree = tree();
    let timeline = tree.borrow().timeline.clone();
    timeline.borrow_mut().set_enabled(true);
    let mut window = FrameWindow::new(&timeline);
    span(&timeline, "layout", "Flex layout");
    let attribution = window.close(&timeline);
    let mut captured = frame();
    attribution.apply(&mut captured);

    // Re-enabling the timeline clears its ring while sequences keep counting,
    // which is what a later capture does to an older frame's window.
    timeline.borrow_mut().set_enabled(false);
    timeline.borrow_mut().set_enabled(true);
    span(&timeline, "draw", "Encode draw list");

    let mut probe = RuntimeProbe::new(tree.clone(), None, None, None);
    probe.record(captured);
    let analysis = analyze(&mut probe, 1);
    assert_eq!(analysis.status, FrameStatus::Unavailable);
    assert!(
        analysis
            .reason
            .as_deref()
            .unwrap_or("")
            .contains("no longer retained"),
        "{analysis:?}"
    );
}

#[test]
fn an_unretained_frame_number_is_explicit() {
    let tree = tree();
    let mut probe = RuntimeProbe::new(tree, None, None, None);
    probe.record(frame());
    let analysis = analyze(&mut probe, 42);
    assert_eq!(analysis.status, FrameStatus::Unavailable);
    assert!(
        analysis
            .reason
            .as_deref()
            .unwrap_or("")
            .contains("not retained"),
        "{analysis:?}"
    );
}

#[test]
fn a_timing_frame_from_an_older_probe_still_decodes() {
    let legacy = serde_json::json!({
        "frame": 5,
        "recompositionMs": null,
        "layoutMs": 0.5,
        "drawMs": 0.5,
        "renderThreadMs": null,
        "submitted": true,
        "presented": null,
        "totalMs": 1.25,
        "nodeCount": 3,
        "batchCount": 2,
    });
    let decoded: TimingFrame =
        serde_json::from_value(legacy).expect("the window fields default to zero");
    assert_eq!(decoded.timeline_start, 0);
    assert_eq!(decoded.timeline_end, 0);

    let timeline = PerformanceTimeline::new(8);
    let analysis = cove_devtools::frame_analysis::analyze(&decoded, &timeline, 16);
    assert_eq!(analysis.status, FrameStatus::Unavailable);
}
