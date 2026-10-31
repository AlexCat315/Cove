//! Translation of CoveUIScene/ComponentDiagnostics.swift.
use serde::{Deserialize, Serialize};

/// Compiler-captured source coordinates.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentSourceLocation {
    pub file_id: String,
    pub file_path: String,
    pub line: u64,
    pub column: u64,
    pub explicit: bool,
}
impl ComponentSourceLocation {
    pub fn new(file_id: impl Into<String>, file_path: impl Into<String>, line: u64) -> Self {
        Self {
            file_id: file_id.into(),
            file_path: file_path.into(),
            line,
            column: 1,
            explicit: false,
        }
    }
    pub fn with_column(mut self, column: u64) -> Self {
        self.column = column;
        self
    }
    pub fn explicit(mut self, explicit: bool) -> Self {
        self.explicit = explicit;
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecompositionReason {
    pub kind: String,
    pub detail: Option<String>,
    pub origin_scope: Option<u64>,
}
impl RecompositionReason {
    pub fn new(kind: impl Into<String>) -> Self {
        Self {
            kind: truncate_chars(kind.into(), 64),
            detail: None,
            origin_scope: None,
        }
    }
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(truncate_chars(detail.into(), 160));
        self
    }
    pub fn with_origin_scope(mut self, scope: u64) -> Self {
        self.origin_scope = Some(scope);
        self
    }
}
fn truncate_chars(text: String, limit: usize) -> String {
    if text.chars().count() <= limit {
        text
    } else {
        text.chars().take(limit).collect()
    }
}

/// Counts body evaluations, excluding the initial mount.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ComponentRecompositionMetrics {
    pub count: u64,
    pub initial_ms: f64,
    pub last_ms: f64,
    pub total_ms: f64,
    pub max_ms: f64,
    pub last_reasons: Vec<RecompositionReason>,
}
impl ComponentRecompositionMetrics {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn record(&mut self, milliseconds: f64, initial: bool, reasons: &[RecompositionReason]) {
        let milliseconds = if milliseconds.is_finite() {
            milliseconds.max(0.0)
        } else {
            0.0
        };
        if initial {
            self.initial_ms = milliseconds;
            return;
        }
        self.count = self.count.wrapping_add(1);
        self.last_ms = milliseconds;
        self.total_ms += milliseconds;
        self.max_ms = self.max_ms.max(milliseconds);
        self.last_reasons = reasons.iter().take(16).cloned().collect();
    }
    pub fn reset(&mut self) {
        let mount = self.initial_ms;
        *self = Self {
            initial_ms: mount,
            ..Self::default()
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metrics_keep_mount_time_across_reset() {
        let mut metrics = ComponentRecompositionMetrics::new();
        metrics.record(4.0, true, &[]);
        metrics.record(2.5, false, &[RecompositionReason::new("state")]);
        metrics.record(f64::NAN, false, &[]);
        assert_eq!(
            (
                metrics.count,
                metrics.initial_ms,
                metrics.last_ms,
                metrics.total_ms,
                metrics.max_ms
            ),
            (2, 4.0, 0.0, 2.5, 2.5)
        );
        metrics.reset();
        assert_eq!((metrics.count, metrics.initial_ms), (0, 4.0));
    }
    #[test]
    fn reason_truncates_to_declared_limits() {
        let reason = RecompositionReason::new("k".repeat(80)).with_detail("d".repeat(200));
        assert_eq!(reason.kind.chars().count(), 64);
        assert_eq!(reason.detail.unwrap().chars().count(), 160);
    }
}
