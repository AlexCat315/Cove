//! Translation of CoveUIScene/InvalidationLog.swift.
use super::{AmbientSlot, ElementId};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Why a node was marked dirty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InvalidationSource {
    StateWrite { scope: u64 },
    StyleSet { field: String },
    LayoutChange,
    StructuralChange,
    FocusChange,
    PlatformResize,
    Unknown,
}
impl InvalidationSource {
    pub fn style_set(field: impl Into<String>) -> Self {
        Self::StyleSet {
            field: field.into(),
        }
    }
}

/// Which subsystem the dirtiness applies to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InvalidationPhase {
    Layout,
    Render,
    Input,
    Structure,
}
impl InvalidationPhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Layout => "layout",
            Self::Render => "render",
            Self::Input => "input",
            Self::Structure => "structure",
        }
    }
}

/// One recorded invalidation event.
#[derive(Clone, Debug, PartialEq)]
pub struct DirtyReason {
    pub target: ElementId,
    pub source: InvalidationSource,
    pub phase: InvalidationPhase,
    pub timestamp: f64,
}
impl DirtyReason {
    pub fn new(target: ElementId, source: InvalidationSource, phase: InvalidationPhase) -> Self {
        Self {
            target,
            source,
            phase,
            timestamp: unix_seconds(),
        }
    }
    pub fn at(
        target: ElementId,
        source: InvalidationSource,
        phase: InvalidationPhase,
        timestamp: f64,
    ) -> Self {
        Self {
            target,
            source,
            phase,
            timestamp,
        }
    }
}
fn unix_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |elapsed| elapsed.as_secs_f64())
}

#[derive(Debug)]
struct Ring {
    buffer: Vec<DirtyReason>,
    head: usize,
    capacity: usize,
}
impl Ring {
    fn record(&mut self, reason: DirtyReason) {
        if self.buffer.len() < self.capacity {
            self.buffer.push(reason);
            self.head = self.buffer.len() % self.capacity;
        } else {
            self.buffer[self.head] = reason;
            self.head = (self.head + 1) % self.capacity;
        }
    }
    fn oldest_first(&self) -> Vec<DirtyReason> {
        if self.buffer.is_empty() {
            return Vec::new();
        }
        if self.buffer.len() < self.capacity {
            return self.buffer.clone();
        }
        (0..self.capacity)
            .map(|offset| self.buffer[(self.head + offset) % self.capacity].clone())
            .collect()
    }
}

/// Bounded ring buffer of recent `DirtyReason` entries.
#[derive(Debug)]
pub struct InvalidationLog {
    ring: Mutex<Ring>,
    capacity: usize,
}
impl InvalidationLog {
    pub const DEFAULT_CAPACITY: usize = 256;
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            ring: Mutex::new(Ring {
                buffer: Vec::with_capacity(capacity),
                head: 0,
                capacity,
            }),
            capacity,
        }
    }
    pub fn capacity(&self) -> usize {
        self.capacity
    }
    pub fn record(&self, reason: DirtyReason) {
        self.ring
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .record(reason);
    }
    /// Recorded entries in oldest-to-newest order, optionally limited to the newest `limit`.
    pub fn snapshot(&self, limit: Option<usize>) -> Vec<DirtyReason> {
        let entries = self
            .ring
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .oldest_first();
        match limit {
            Some(limit) if entries.len() > limit => entries[entries.len() - limit..].to_vec(),
            _ => entries,
        }
    }
    pub fn clear(&self) {
        let mut ring = self.ring.lock().unwrap_or_else(|error| error.into_inner());
        ring.buffer.clear();
        ring.head = 0;
    }
}
impl Default for InvalidationLog {
    fn default() -> Self {
        Self::new(Self::DEFAULT_CAPACITY)
    }
}

/// Per-window holder for the active invalidation log.
static HOLDER: AmbientSlot<Option<Arc<InvalidationLog>>> = AmbientSlot::new(None);

pub struct InvalidationLogHolder;
impl InvalidationLogHolder {
    pub fn current() -> Option<Arc<InvalidationLog>> {
        HOLDER.current()
    }
    pub fn set_current(log: Option<Arc<InvalidationLog>>) {
        HOLDER.set(log);
    }
    /// Run `body` with `log` installed, restoring the previous binding on exit.
    pub fn with<R>(log: Option<Arc<InvalidationLog>>, body: impl FnOnce() -> R) -> R {
        let _restore = Restore(HOLDER.current());
        HOLDER.set(log);
        body()
    }
}
struct Restore(Option<Arc<InvalidationLog>>);
impl Drop for Restore {
    fn drop(&mut self) {
        HOLDER.set(self.0.take());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ring_reports_oldest_first_after_wrap() {
        let log = InvalidationLog::new(3);
        for index in 0..5 {
            log.record(DirtyReason::at(
                ElementId(index),
                InvalidationSource::Unknown,
                InvalidationPhase::Render,
                index as f64,
            ));
        }
        let timestamps: Vec<_> = log
            .snapshot(None)
            .iter()
            .map(|entry| entry.timestamp)
            .collect();
        assert_eq!(timestamps, vec![2.0, 3.0, 4.0]);
        assert_eq!(log.snapshot(Some(2)).len(), 2);
        log.clear();
        assert!(log.snapshot(None).is_empty());
    }
    #[test]
    fn holder_restores_previous_binding() {
        let first = Arc::new(InvalidationLog::default());
        let second = Arc::new(InvalidationLog::default());
        InvalidationLogHolder::with(Some(first.clone()), || {
            assert!(Arc::ptr_eq(
                &InvalidationLogHolder::current().unwrap(),
                &first
            ));
            InvalidationLogHolder::with(Some(second), || {
                assert_eq!(InvalidationLogHolder::current().unwrap().capacity(), 256)
            });
            assert!(Arc::ptr_eq(
                &InvalidationLogHolder::current().unwrap(),
                &first
            ));
        });
        assert!(InvalidationLogHolder::current().is_none());
    }
}
