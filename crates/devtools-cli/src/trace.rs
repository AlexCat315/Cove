use crate::connection::{Connection, failure};
use cove_agent::AgentError;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

pub struct Capture {
    pub id: String,
    pub events: Vec<Value>,
    pub dropped: usize,
}
impl Capture {
    pub fn new(id: String, batches: Vec<Value>, dropped: usize) -> Self {
        let host_dropped = batches
            .iter()
            .filter_map(|batch| batch["dropped"].as_u64())
            .max()
            .unwrap_or(0) as usize;
        let mut events = Vec::new();
        let mut bytes = 0;
        let mut omitted = 0;
        for event in batches.into_iter().flat_map(|mut batch| {
            batch["events"]
                .take()
                .as_array()
                .cloned()
                .unwrap_or_default()
        }) {
            let size = serde_json::to_vec(&event).map_or(0, |bytes| bytes.len());
            if bytes + size <= 384 * 1024 && events.len() < 4096 {
                bytes += size;
                events.push(event);
            } else {
                omitted += 1;
            }
        }
        Self {
            id,
            events,
            dropped: dropped + host_dropped + omitted,
        }
    }
    pub fn summary(&self) -> Value {
        let mut phases: BTreeMap<String, Vec<f64>> = BTreeMap::new();
        for event in &self.events {
            if let (Some(phase), Some(duration)) =
                (event["phase"].as_str(), event["durationMs"].as_f64())
            {
                phases.entry(phase.into()).or_default().push(duration);
            }
        }
        let phases: BTreeMap<_, _> = phases
            .into_iter()
            .map(|(phase, values)| (phase, statistics(values)))
            .collect();
        json!({"capture_id": self.id, "event_count": self.events.len(), "dropped": self.dropped, "phases": phases})
    }
}

pub fn compare(baseline: &Capture, candidate: &Capture) -> Value {
    let baseline = baseline.summary();
    let candidate = candidate.summary();
    let phases: std::collections::BTreeSet<_> = baseline["phases"]
        .as_object()
        .into_iter()
        .flat_map(|phases| phases.keys())
        .chain(
            candidate["phases"]
                .as_object()
                .into_iter()
                .flat_map(|phases| phases.keys()),
        )
        .collect();
    let changes: BTreeMap<_, _> = phases.into_iter().map(|phase| {
        let before = &baseline["phases"][phase];
        let after = &candidate["phases"][phase];
        let delta = |key: &str| before[key].as_f64().zip(after[key].as_f64()).map(|(a, b)| b - a);
        (phase.clone(), json!({"count_delta": after["count"].as_i64().unwrap_or(0) - before["count"].as_i64().unwrap_or(0),
            "average_ms_delta": delta("average_ms"), "p95_ms_delta": delta("p95_ms")}))
    }).collect();
    json!({"baseline": baseline, "candidate": candidate, "changes": changes})
}

pub fn collect(
    peer: &mut Connection,
    kind: &str,
    milliseconds: u64,
) -> Result<(Vec<Value>, usize), AgentError> {
    let deadline = Instant::now() + Duration::from_millis(milliseconds);
    let mut values = Vec::new();
    let mut bytes = 0;
    let mut omitted = 0;
    while let Some(value) = peer.wait_event(kind, deadline)? {
        let size = serde_json::to_vec(&value)
            .map_err(|_| failure("Invalid capture event."))?
            .len();
        if bytes + size <= 384 * 1024 && values.len() < 4096 {
            bytes += size;
            values.push(value);
        } else {
            omitted += value["events"].as_array().map_or(1, Vec::len);
        }
    }
    Ok((values, peer.dropped + omitted))
}

pub fn statistics(mut values: Vec<f64>) -> Value {
    values.retain(|value| value.is_finite() && *value >= 0.);
    if values.is_empty() {
        return json!({"count": 0});
    }
    values.sort_by(f64::total_cmp);
    let percentile = |fraction: f64| {
        values[((values.len() as f64 * fraction).ceil() as usize)
            .saturating_sub(1)
            .min(values.len() - 1)]
    };
    json!({"count": values.len(), "average_ms": values.iter().sum::<f64>() / values.len() as f64,
        "p50_ms": percentile(0.5), "p95_ms": percentile(0.95), "max_ms": values.last()})
}
pub fn timing_summary(frames: &[Value]) -> Value {
    let stages: BTreeMap<_, _> = [
        "totalMs",
        "recompositionMs",
        "layoutMs",
        "drawMs",
        "presentMs",
    ]
    .into_iter()
    .map(|stage| {
        (
            stage,
            statistics(
                frames
                    .iter()
                    .filter_map(|frame| frame[stage].as_f64())
                    .collect(),
            ),
        )
    })
    .collect();
    json!({"frame_count": frames.len(), "presented_count": frames.iter().filter(|frame| frame["presented"] == true).count(), "stages": stages})
}
