//! Translation of `Logging/Sources/CoveLogCore/Records/Context.swift`.
//!
//! Swift uses `@TaskLocal`; structured concurrency is not available in the
//! standard library, so the same scoped merge is thread-local. Async callers
//! must keep the guard on the polling thread (the executor contract already
//! confines logging to one worker).
use super::record::LogValue;
use std::cell::RefCell;
use std::collections::BTreeMap;

pub type Metadata = BTreeMap<String, LogValue>;

thread_local! {
    static METADATA: RefCell<Metadata> = const { RefCell::new(BTreeMap::new()) };
}

/// Restores the previous metadata when dropped, including on early return.
pub struct LogContextGuard {
    previous: Metadata,
    active: bool,
}
impl LogContextGuard {
    pub fn metadata(&self) -> Metadata {
        METADATA.with(|slot| slot.borrow().clone())
    }
}
impl Drop for LogContextGuard {
    fn drop(&mut self) {
        if self.active {
            let previous = std::mem::take(&mut self.previous);
            METADATA.with(|slot| *slot.borrow_mut() = previous);
        }
    }
}

pub struct LogContext;
impl LogContext {
    pub fn current() -> Metadata {
        METADATA.with(|slot| slot.borrow().clone())
    }

    /// Merges `values` over the current metadata for the dynamic extent of
    /// `operation`; later keys replace earlier ones.
    pub fn with_values<T>(values: Metadata, operation: impl FnOnce() -> T) -> T {
        let previous = METADATA.with(|slot| {
            let mut current = slot.borrow_mut();
            let previous = current.clone();
            for (key, value) in values {
                current.insert(key, value);
            }
            previous
        });
        let guard = LogContextGuard {
            previous,
            active: true,
        };
        let result = operation();
        drop(guard);
        result
    }

    /// Fallible variant; the guard is released before the error propagates.
    pub fn try_with_values<T, E>(
        values: Metadata,
        operation: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        let previous = METADATA.with(|slot| {
            let mut current = slot.borrow_mut();
            let previous = current.clone();
            for (key, value) in values {
                current.insert(key, value);
            }
            previous
        });
        let guard = LogContextGuard {
            previous,
            active: true,
        };
        let result = operation();
        drop(guard);
        result
    }
}
