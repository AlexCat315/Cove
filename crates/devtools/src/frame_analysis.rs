//! Per-frame attribution of the CPU timeline.
//!
//! A frame owns the timeline spans produced in the sequence range
//! `(timeline_start, timeline_end]`, closed when the host finishes preparing
//! it. Recomposition for the next frame runs before the host begins that frame,
//! so a window opens where the previous one closed: the commit that produced
//! the frame's tree is inside the frame it belongs to, not the one before.
//!
//! Only real retained spans are used. A frame with no window, or whose spans
//! are no longer retained, yields [`FrameStatus::Unavailable`] with the reason;
//! nothing is estimated from the frame's own millisecond fields.
use cove_core::scene::{PerformanceTimeline, TimelineEvent};
use cove_devtools_protocol::TimingFrame;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::rc::Rc;

pub type Timeline = Rc<RefCell<PerformanceTimeline>>;

/// Spans a single analysis may return. Totals still cover every span in the
/// window; `truncated` says the span list was cut.
pub const MAX_ANALYZED_SPANS: usize = 256;

/// The phases frame analysis attributes CPU time to, in frame order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FramePhase {
    Recomposition,
    Layout,
    Draw,
    Encode,
    Present,
}

impl FramePhase {
    pub const ALL: [Self; 5] = [
        Self::Recomposition,
        Self::Layout,
        Self::Draw,
        Self::Encode,
        Self::Present,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Recomposition => "recomposition",
            Self::Layout => "layout",
            Self::Draw => "draw",
            Self::Encode => "encode",
            Self::Present => "present",
        }
    }

    /// Which phase a retained span belongs to.
    ///
    /// `component` spans are the composition-scope bodies that `recomposition`
    /// commits, so they are build work for the same phase. Frame-level spans
    /// (surface environment, chrome hit test, accessibility sync, node count)
    /// belong to no phase and are reported separately as unclassified.
    pub fn of(phase: &str) -> Option<Self> {
        match phase {
            "recomposition" | "component" => Some(Self::Recomposition),
            "layout" => Some(Self::Layout),
            "draw" => Some(Self::Draw),
            "encode" => Some(Self::Encode),
            "present" => Some(Self::Present),
            _ => None,
        }
    }
}

/// Closes one frame's timeline window at a time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameWindow {
    cursor: u64,
}

/// What a frame's closed window holds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameAttribution {
    pub window_start: u64,
    pub window_end: u64,
    /// CPU time of the `recomposition` spans inside the window, or `None` when
    /// no commit span was recorded.
    pub recomposition_ms: Option<f64>,
}

impl FrameAttribution {
    /// Stamp the attribution onto a frame's timed fields and window.
    pub fn apply(&self, frame: &mut TimingFrame) {
        frame.recomposition_ms = self.recomposition_ms;
        self.apply_window(frame);
    }

    /// Stamp only the timeline window. Hosts that measure recomposition
    /// themselves keep their own `recomposition_ms` and call this.
    pub fn apply_window(&self, frame: &mut TimingFrame) {
        frame.timeline_start = self.window_start;
        frame.timeline_end = self.window_end;
    }
}

impl FrameWindow {
    /// Open the first window after the spans the timeline already holds.
    pub fn new(timeline: &Timeline) -> Self {
        Self {
            cursor: timeline.borrow().latest_sequence(),
        }
    }

    /// Close the window at the timeline's latest sequence and advance the
    /// cursor. Call once per finished frame, whether or not spans are being
    /// recorded: an idle timeline still advances its cursor, so spans emitted
    /// later cannot be attributed to an earlier frame.
    pub fn close(&mut self, timeline: &Timeline) -> FrameAttribution {
        let timeline = timeline.borrow();
        let end = timeline.latest_sequence().max(self.cursor);
        let recomposition_ms = {
            let mut total = 0.0;
            let mut recorded = false;
            for event in timeline.events_after(self.cursor) {
                if event.sequence > end {
                    break;
                }
                if event.phase == "recomposition" {
                    total += event.duration_ms.max(0.0);
                    recorded = true;
                }
            }
            recorded.then_some(total)
        };
        let start = self.cursor;
        self.cursor = end;
        FrameAttribution {
            window_start: start,
            window_end: end,
            recomposition_ms,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FrameStatus {
    Ok,
    Unavailable,
}

/// The timeline sequence range one frame owns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameRange {
    pub start_sequence: u64,
    pub end_sequence: u64,
}

/// CPU total for one attributed phase inside a frame's window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FramePhaseSummary {
    pub phase: String,
    pub span_count: usize,
    pub cpu_ms: f64,
    pub maximum_ms: f64,
    /// Share of the frame's classified CPU time, in `0.0..=1.0`.
    pub share: f64,
}

/// One retained span used by the analysis.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameSpan {
    pub sequence: u64,
    pub phase: String,
    pub name: String,
    #[serde(rename = "scopeID")]
    pub scope_id: Option<String>,
    pub start_ms: f64,
    pub duration_ms: f64,
}

/// The answer to `analyze_ui_frame`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameAnalysis {
    pub frame: u64,
    pub status: FrameStatus,
    pub window: Option<FrameRange>,
    pub dominant_phase: Option<String>,
    pub phases: Vec<FramePhaseSummary>,
    /// The spans behind the classification, in timeline order. When the window
    /// holds more than the cap, the longest spans are kept.
    pub spans: Vec<FrameSpan>,
    /// Spans in the window, including those beyond `spans`' cap.
    pub span_count: usize,
    /// More spans existed than were returned, or the window's earliest spans
    /// are no longer retained.
    pub truncated: bool,
    pub unclassified_span_count: usize,
    pub unclassified_cpu_ms: f64,
    /// Why the frame cannot be classified. `None` when the status is `Ok`.
    pub reason: Option<String>,
}

impl FrameAnalysis {
    fn unavailable(frame: u64, window: Option<FrameRange>, reason: impl Into<String>) -> Self {
        Self {
            frame,
            status: FrameStatus::Unavailable,
            window,
            dominant_phase: None,
            phases: Vec::new(),
            spans: Vec::new(),
            span_count: 0,
            truncated: false,
            unclassified_span_count: 0,
            unclassified_cpu_ms: 0.0,
            reason: Some(reason.into()),
        }
    }

    pub fn is_ok(&self) -> bool {
        self.status == FrameStatus::Ok
    }

    /// Analyze one recorded frame against the timeline it belongs to.
    pub fn for_frame(frame: &TimingFrame, timeline: &PerformanceTimeline) -> Self {
        analyze(frame, timeline, MAX_ANALYZED_SPANS)
    }

    /// A frame number the probe no longer holds.
    pub fn not_retained(frame: u64, retained: usize) -> Self {
        let held = if retained == 0 {
            "the probe has not recorded a frame yet".to_owned()
        } else {
            format!("the probe holds the most recent {retained} frame record(s)")
        };
        Self::unavailable(
            frame,
            None,
            format!("Frame #{frame} is not retained; {held}."),
        )
    }
}

/// Classify one frame from the spans the timeline still retains.
///
/// The frame's own window comes from its recorded [`TimingFrame`]; every number
/// here is measured by the timeline.
pub fn analyze(frame: &TimingFrame, timeline: &PerformanceTimeline, limit: usize) -> FrameAnalysis {
    analyze_spans(frame, &timeline.events(), limit)
}

/// [`analyze`] over an already-read timeline snapshot. The probe reads the
/// timeline once per command; tests can supply retained spans directly.
pub fn analyze_spans(
    frame: &TimingFrame,
    retained: &[TimelineEvent],
    limit: usize,
) -> FrameAnalysis {
    let window = FrameRange {
        start_sequence: frame.timeline_start,
        end_sequence: frame.timeline_end,
    };
    if frame.timeline_end <= frame.timeline_start {
        return FrameAnalysis::unavailable(
            frame.frame,
            None,
            format!(
                "Frame #{} has no trace window; the host recorded no CPU timeline sequence while preparing it.",
                frame.frame
            ),
        );
    }
    let bounded = limit.clamp(1, MAX_ANALYZED_SPANS);
    let in_window: Vec<&TimelineEvent> = retained
        .iter()
        .filter(|event| {
            event.sequence > window.start_sequence && event.sequence <= window.end_sequence
        })
        .collect();
    if in_window.is_empty() {
        let reason = match retained.first() {
            Some(oldest) if oldest.sequence > window.end_sequence => format!(
                "The timeline retains spans from sequence {} onward; frame #{} owns sequences {}–{}, which are no longer retained.",
                oldest.sequence, frame.frame, window.start_sequence, window.end_sequence
            ),
            _ => format!(
                "No CPU spans were recorded in sequences {}–{} for frame #{}; the timeline records only while a trace capture is open.",
                window.start_sequence, window.end_sequence, frame.frame
            ),
        };
        return FrameAnalysis::unavailable(frame.frame, Some(window), reason);
    }
    // A window whose first retained sequence is later than the first sequence
    // after its start lost spans: the ring wrapped inside this frame.
    let truncated = in_window.len() > bounded
        || in_window
            .first()
            .is_some_and(|event| event.sequence > window.start_sequence.saturating_add(1));

    let mut totals: Vec<(FramePhase, usize, f64, f64)> = Vec::new();
    let mut unclassified_span_count = 0;
    let mut unclassified_cpu_ms = 0.0;
    for event in &in_window {
        let duration = if event.duration_ms.is_finite() {
            event.duration_ms.max(0.0)
        } else {
            0.0
        };
        match FramePhase::of(&event.phase) {
            Some(phase) => match totals.iter_mut().find(|(known, ..)| *known == phase) {
                Some((_, count, total, maximum)) => {
                    *count += 1;
                    *total += duration;
                    *maximum = maximum.max(duration);
                }
                None => totals.push((phase, 1, duration, duration)),
            },
            None => {
                unclassified_span_count += 1;
                unclassified_cpu_ms += duration;
            }
        }
    }
    totals.sort_by_key(|entry| entry.0);
    let classified_cpu: f64 = totals.iter().map(|(_, _, total, _)| *total).sum();
    if totals.is_empty() || classified_cpu <= 0.0 {
        return FrameAnalysis {
            frame: frame.frame,
            status: FrameStatus::Unavailable,
            window: Some(window),
            dominant_phase: None,
            phases: Vec::new(),
            spans: project_spans(&in_window, bounded),
            span_count: in_window.len(),
            truncated,
            unclassified_span_count,
            unclassified_cpu_ms,
            reason: Some(format!(
                "Frame #{} holds {} retained span(s) in sequences {}–{}, but none measured recomposition, layout, draw, encode or present time.",
                frame.frame,
                in_window.len(),
                window.start_sequence,
                window.end_sequence
            )),
        };
    }
    let phases: Vec<FramePhaseSummary> = totals
        .iter()
        .map(|(phase, count, total, maximum)| FramePhaseSummary {
            phase: phase.as_str().to_owned(),
            span_count: *count,
            cpu_ms: *total,
            maximum_ms: *maximum,
            share: (*total / classified_cpu).clamp(0.0, 1.0),
        })
        .collect();
    let dominant = phases
        .iter()
        .fold(
            None::<&FramePhaseSummary>,
            |current, candidate| match current {
                Some(current) if current.cpu_ms >= candidate.cpu_ms => Some(current),
                _ => Some(candidate),
            },
        )
        .map(|summary| summary.phase.clone());
    FrameAnalysis {
        frame: frame.frame,
        status: FrameStatus::Ok,
        window: Some(window),
        dominant_phase: dominant,
        phases,
        spans: project_spans(&in_window, bounded),
        span_count: in_window.len(),
        truncated,
        unclassified_span_count,
        unclassified_cpu_ms,
        reason: None,
    }
}

/// Longest spans first, then back into timeline order for display.
fn project_spans(spans: &[&TimelineEvent], limit: usize) -> Vec<FrameSpan> {
    let mut ordered: Vec<&TimelineEvent> = spans.to_vec();
    ordered.sort_by(|left, right| {
        right
            .duration_ms
            .partial_cmp(&left.duration_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    ordered.truncate(limit);
    ordered.sort_by_key(|event| event.sequence);
    ordered
        .into_iter()
        .map(|event| FrameSpan {
            sequence: event.sequence,
            phase: event.phase.clone(),
            name: event.name.clone(),
            scope_id: event.scope_id.clone(),
            start_ms: event.start_ms,
            duration_ms: event.duration_ms,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cove_core::scene::RecompositionReason;

    fn event(sequence: u64, phase: &str, duration_ms: f64) -> TimelineEvent {
        TimelineEvent {
            sequence,
            phase: phase.to_owned(),
            name: format!("{phase} {sequence}"),
            scope_id: None,
            start_ms: sequence as f64,
            duration_ms,
            reasons: Vec::<RecompositionReason>::new(),
        }
    }

    fn frame(start: u64, end: u64) -> TimingFrame {
        TimingFrame {
            gpu_breakdown: None,
            frame: 7,
            recomposition_ms: None,
            layout_ms: 0.0,
            draw_ms: 0.0,
            render_thread_ms: None,
            gpu_ms: None,
            pipeline_creation: None,
            submitted: true,
            presented: None,
            total_ms: 0.0,
            node_count: 0,
            batch_count: 0,
            timeline_start: start,
            timeline_end: end,
        }
    }

    #[test]
    fn the_dominant_phase_is_the_largest_measured_total() {
        let retained = [
            event(1, "recomposition", 0.5),
            event(2, "component", 0.5),
            event(3, "layout", 2.0),
            event(4, "draw", 0.75),
            event(5, "frame", 9.0),
        ];
        let analysis = analyze_spans(&frame(0, 5), &retained, MAX_ANALYZED_SPANS);
        assert!(analysis.is_ok(), "{analysis:?}");
        assert_eq!(analysis.dominant_phase.as_deref(), Some("layout"));
        let layout = analysis
            .phases
            .iter()
            .find(|phase| phase.phase == "layout")
            .expect("layout is summarised");
        assert_eq!(layout.span_count, 1);
        assert!((layout.share - 2.0 / 3.75).abs() < 1e-9);
        let recomposition = analysis
            .phases
            .iter()
            .find(|phase| phase.phase == "recomposition")
            .expect("component spans roll up into recomposition");
        assert_eq!(recomposition.span_count, 2);
        assert!((recomposition.cpu_ms - 1.0).abs() < 1e-9);
        assert!((recomposition.maximum_ms - 0.5).abs() < 1e-9);
        assert_eq!(analysis.unclassified_span_count, 1);
        assert!((analysis.unclassified_cpu_ms - 9.0).abs() < 1e-9);
        assert_eq!(analysis.spans.len(), 5);
        assert!(analysis.spans.iter().any(|span| span.phase == "frame"));
        assert!(!analysis.truncated);
    }

    #[test]
    fn spans_outside_the_window_are_ignored() {
        let retained = [
            event(1, "layout", 5.0),
            event(2, "layout", 1.0),
            event(3, "draw", 0.25),
            event(4, "draw", 5.0),
        ];
        let analysis = analyze_spans(&frame(1, 3), &retained, MAX_ANALYZED_SPANS);
        assert!(analysis.is_ok());
        assert_eq!(analysis.span_count, 2);
        assert_eq!(analysis.dominant_phase.as_deref(), Some("layout"));
        assert_eq!(
            analysis
                .spans
                .iter()
                .map(|span| span.sequence)
                .collect::<Vec<_>>(),
            vec![2, 3]
        );
    }

    #[test]
    fn a_frame_without_a_window_is_unavailable() {
        let retained = [event(1, "layout", 1.0)];
        let analysis = analyze_spans(&frame(0, 0), &retained, MAX_ANALYZED_SPANS);
        assert_eq!(analysis.status, FrameStatus::Unavailable);
        assert!(analysis.window.is_none());
        assert!(
            analysis
                .reason
                .as_deref()
                .unwrap_or("")
                .contains("no trace window"),
            "the reason names the missing window: {analysis:?}"
        );
    }

    #[test]
    fn a_window_without_attributable_spans_is_unavailable() {
        let retained = [event(1, "frame", 1.0)];
        let analysis = analyze_spans(&frame(0, 1), &retained, MAX_ANALYZED_SPANS);
        assert_eq!(analysis.status, FrameStatus::Unavailable);
        assert_eq!(analysis.dominant_phase, None);
        assert_eq!(analysis.span_count, 1);
        assert!(
            analysis
                .reason
                .as_deref()
                .unwrap_or("")
                .contains("none measured"),
            "{analysis:?}"
        );
    }

    #[test]
    fn expired_windows_say_so_instead_of_guessing() {
        let retained = [event(9, "layout", 1.0)];
        let analysis = analyze_spans(&frame(0, 4), &retained, MAX_ANALYZED_SPANS);
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
    fn an_empty_timeline_names_the_missing_capture() {
        let analysis = analyze_spans(&frame(4, 9), &[], MAX_ANALYZED_SPANS);
        assert_eq!(analysis.status, FrameStatus::Unavailable);
        assert!(
            analysis
                .reason
                .as_deref()
                .unwrap_or("")
                .contains("trace capture"),
            "{analysis:?}"
        );
    }

    #[test]
    fn the_span_list_is_capped_and_reports_it() {
        let retained: Vec<TimelineEvent> =
            (1..=4).map(|index| event(index, "layout", 1.0)).collect();
        let analysis = analyze_spans(&frame(0, 4), &retained, 2);
        assert!(analysis.is_ok());
        assert_eq!(analysis.span_count, 4);
        assert_eq!(analysis.spans.len(), 2);
        assert!(analysis.truncated);
        assert!((analysis.phases[0].cpu_ms - 4.0).abs() < 1e-9);
    }

    #[test]
    fn a_wrapped_ring_sets_truncated() {
        let retained = [event(3, "layout", 1.0), event(4, "draw", 0.5)];
        let analysis = analyze_spans(&frame(0, 4), &retained, MAX_ANALYZED_SPANS);
        assert!(analysis.is_ok());
        assert!(
            analysis.truncated,
            "sequences 1 and 2 were dropped inside the window"
        );
    }

    #[test]
    fn the_wire_shape_is_the_client_contract() {
        let retained = [event(1, "layout", 2.0)];
        let value =
            serde_json::to_value(analyze_spans(&frame(0, 1), &retained, MAX_ANALYZED_SPANS))
                .expect("analysis serialises");
        assert_eq!(value["status"], "ok");
        assert_eq!(value["dominantPhase"], "layout");
        assert_eq!(value["window"]["startSequence"], 0);
        assert_eq!(value["window"]["endSequence"], 1);
        assert_eq!(value["phases"][0]["phase"], "layout");
        assert_eq!(value["phases"][0]["spanCount"], 1);
        assert_eq!(value["phases"][0]["cpuMs"], 2.0);
        assert_eq!(value["spans"][0]["durationMs"], 2.0);
        assert!(value["spans"][0].get("scopeID").is_some());
        assert!(value["reason"].is_null());

        let unavailable =
            serde_json::to_value(analyze_spans(&frame(0, 0), &retained, MAX_ANALYZED_SPANS))
                .expect("analysis serialises");
        assert_eq!(unavailable["status"], "unavailable");
        assert!(unavailable["reason"].is_string());
        assert!(unavailable["dominantPhase"].is_null());
        assert!(unavailable["spans"].as_array().is_some_and(Vec::is_empty));
    }
}
