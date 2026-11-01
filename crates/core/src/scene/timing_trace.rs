//! Translation of CoveUIScene/TimingTrace.swift.
use std::sync::OnceLock;
use std::time::Instant;

fn epoch() -> Instant {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}
/// Monotonic seconds since the first call, with nanosecond resolution.
pub fn monotonic_seconds() -> f64 {
    epoch().elapsed().as_secs_f64()
}

#[derive(Clone, Debug)]
struct Sample {
    stage: String,
    milliseconds: f64,
}

#[derive(Clone, Debug)]
pub struct TimingTrace {
    label: String,
    started_at: f64,
    last_mark: f64,
    samples: Vec<Sample>,
}
impl TimingTrace {
    pub fn new(label: impl Into<String>) -> Self {
        let now = Self::now();
        Self {
            label: label.into(),
            started_at: now,
            last_mark: now,
            samples: Vec::new(),
        }
    }
    #[inline(always)]
    pub fn now() -> f64 {
        monotonic_seconds()
    }
    pub fn mark(&mut self, stage: impl Into<String>) {
        let now = Self::now();
        self.samples.push(Sample {
            stage: stage.into(),
            milliseconds: (now - self.last_mark) * 1000.0,
        });
        self.last_mark = now;
    }
    pub fn summary(&self) -> String {
        self.summary_with(&[])
    }
    pub fn summary_with(&self, extra: &[&str]) -> String {
        let mut parts = vec![self.label.clone()];
        parts.extend(self.samples.iter().map(|sample| {
            format!(
                "{}={}",
                sample.stage,
                format_milliseconds(sample.milliseconds)
            )
        }));
        parts.push(format!(
            "total={}",
            format_milliseconds((Self::now() - self.started_at) * 1000.0)
        ));
        parts.extend(
            extra
                .iter()
                .filter(|text| !text.is_empty())
                .map(|text| (*text).to_owned()),
        );
        parts.join(" ")
    }
    pub fn label(&self) -> &str {
        &self.label
    }
}
fn format_milliseconds(milliseconds: f64) -> String {
    format!("{milliseconds:.2}ms")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn summary_lists_stages_and_total() {
        let mut trace = TimingTrace::new("frame");
        trace.mark("layout");
        trace.mark("paint");
        let summary = trace.summary_with(&["extra=1", ""]);
        assert!(summary.starts_with("frame layout="));
        assert!(summary.contains(" paint="));
        assert!(summary.contains(" total="));
        assert!(summary.ends_with("extra=1"));
    }
    #[test]
    fn clock_is_monotonic() {
        let first = TimingTrace::now();
        let second = TimingTrace::now();
        assert!(second >= first);
    }
}
