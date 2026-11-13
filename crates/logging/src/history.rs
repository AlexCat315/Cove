//! Translation of `Logging/Sources/CoveLogging/Runtime/History.swift`.
use super::record::{LogRecord, Process};

/// Fixed storage prevents per-record shifts when the replay window is full.
pub struct LogHistory {
    storage: Vec<Option<LogRecord>>,
    next: usize,
    count: usize,
}
impl LogHistory {
    pub fn new(capacity: usize) -> Self {
        Self {
            storage: vec![None; capacity],
            next: 0,
            count: 0,
        }
    }

    pub fn records(&self) -> Vec<LogRecord> {
        if self.count == 0 || self.storage.is_empty() {
            return Vec::new();
        }
        let start = if self.count == self.storage.len() {
            self.next
        } else {
            0
        };
        (0..self.count)
            .filter_map(|offset| {
                self.storage
                    .get((start + offset) % self.storage.len())
                    .and_then(Clone::clone)
            })
            .collect()
    }

    pub fn append(&mut self, record: LogRecord) {
        if self.storage.is_empty() {
            return;
        }
        self.storage[self.next] = Some(record);
        self.next = (self.next + 1) % self.storage.len();
        self.count = (self.count + 1).min(self.storage.len());
    }

    /// Empties the replay window while keeping its capacity, so later records
    /// reuse the same storage without a reallocation or a shift. Returns the
    /// number of records removed.
    pub fn clear(&mut self) -> usize {
        let cleared = self.count;
        self.storage.fill(None);
        self.next = 0;
        self.count = 0;
        cleared
    }

    pub fn resize(&mut self, capacity: usize) {
        if capacity == self.storage.len() {
            return;
        }
        let retained = self.records();
        let retained = if retained.len() > capacity {
            retained[retained.len() - capacity..].to_vec()
        } else {
            retained
        };
        *self = Self::new(capacity);
        for record in retained {
            self.append(record);
        }
    }

    /// Startup identity replaces the placeholder stamped before configuration,
    /// including records already in the replay window.
    pub fn identify_startup(&mut self, process: &Process) {
        for record in self.storage.iter_mut().flatten() {
            record.process = process.clone();
        }
    }

    pub fn dropped_capacity(&self) -> usize {
        self.storage.len()
    }
}
