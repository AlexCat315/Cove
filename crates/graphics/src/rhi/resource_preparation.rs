//! Translation of NativeRHI/Device/ResourcePreparation.swift.
//!
//! One preparation runs serially on its worker. Only cancellation crosses
//! threads; progress snapshots are immutable and callbacks are `Send + Sync`.
//!
//! Swift installs the active context in the thread dictionary and resumes a
//! continuation from a `DispatchQueue`. Rust has neither, so the context lives
//! in a thread-local and [`ResourcePreparationContext::run`] is synchronous; the
//! async wrapper belongs to the runtime crate that owns the executor.
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use super::{RhiError, RhiResult};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreparationPhase {
    Preparing,
    Finished,
    Cancelled,
    Failed,
}
impl PreparationPhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Preparing => "preparing",
            Self::Finished => "finished",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourcePreparationProgress {
    pub phase: PreparationPhase,
    pub sequence: u64,
    pub label: String,
    pub current_step: Option<String>,
    pub completed_steps: usize,
    pub total_steps: Option<usize>,
    pub completed_compilations: usize,
    pub current_compilation: Option<String>,
    pub failure: Option<String>,
}
impl ResourcePreparationProgress {
    /// Work-item progress, not an estimate of remaining driver compilation
    /// time.
    pub fn fraction(&self) -> Option<f64> {
        match self.total_steps {
            Some(total) if total > 0 => Some(self.completed_steps as f64 / total as f64),
            _ if self.phase == PreparationPhase::Finished => Some(1.0),
            _ => None,
        }
    }
}

type Observer = Box<dyn Fn(ResourcePreparationProgress) + Send + Sync>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct OwnedKey {
    device: u64,
    id: u32,
}

struct Owned {
    count: usize,
    order: usize,
    cleanup: Box<dyn FnMut()>,
}

struct OwnedState {
    owned: HashMap<OwnedKey, Owned>,
    ownership_order: usize,
}

thread_local! {
    /// The preparation currently running on this thread, if any.
    static CURRENT: RefCell<Option<std::rc::Rc<ResourcePreparationContext>>> =
        const { RefCell::new(None) };
}

pub struct ResourcePreparationContext {
    cancelled: AtomicBool,
    label: String,
    total: Option<usize>,
    observer: Observer,
    step_label: Mutex<Option<String>>,
    completed: Mutex<usize>,
    compilations: Mutex<usize>,
    compilation: Mutex<Option<String>>,
    sequence: Mutex<u64>,
    owned: Mutex<OwnedState>,
}
impl ResourcePreparationContext {
    pub fn new(
        label: impl Into<String>,
        total: Option<usize>,
        observer: impl Fn(ResourcePreparationProgress) + Send + Sync + 'static,
    ) -> Self {
        Self {
            cancelled: AtomicBool::new(false),
            label: label.into(),
            total,
            observer: Box::new(observer),
            step_label: Mutex::new(None),
            completed: Mutex::new(0),
            compilations: Mutex::new(0),
            compilation: Mutex::new(None),
            sequence: Mutex::new(0),
            owned: Mutex::new(OwnedState {
                owned: HashMap::new(),
                ownership_order: 0,
            }),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub fn check_cancellation(&self) -> RhiResult<()> {
        if self.is_cancelled() {
            Err(RhiError::Cancelled)
        } else {
            Ok(())
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    /// Runs one declared step. Must be called from inside [`Self::run`].
    pub fn step<T>(&self, label: &str, operation: impl FnOnce() -> RhiResult<T>) -> RhiResult<T> {
        if !Self::current_is(self) {
            return Err(RhiError::invalid_argument(
                "preparation steps must run inside their preparation operation",
            ));
        }
        self.check_cancellation()?;
        if let Some(total) = self.total
            && *self.completed.lock().unwrap() >= total
        {
            return Err(RhiError::invalid_argument(
                "preparation exceeded its declared step count",
            ));
        }
        *self.step_label.lock().unwrap() = Some(label.to_owned());
        self.emit(PreparationPhase::Preparing, None);
        let value = operation()?;
        self.check_cancellation()?;
        *self.completed.lock().unwrap() += 1;
        self.emit(PreparationPhase::Preparing, None);
        Ok(value)
    }

    pub fn compilation_started(&self, label: &str) -> RhiResult<()> {
        self.check_cancellation()?;
        *self.compilation.lock().unwrap() = Some(label.to_owned());
        self.emit(PreparationPhase::Preparing, None);
        Ok(())
    }

    pub fn compilation_finished(&self) {
        *self.compilations.lock().unwrap() += 1;
        *self.compilation.lock().unwrap() = None;
        self.emit(PreparationPhase::Preparing, None);
    }

    pub fn acquired(&self, device: u64, id: u32, cleanup: impl FnMut() + 'static) {
        let key = OwnedKey { device, id };
        let mut state = self.owned.lock().unwrap();
        if let Some(existing) = state.owned.get_mut(&key) {
            existing.count += 1;
            return;
        }
        state.ownership_order += 1;
        let order = state.ownership_order;
        state.owned.insert(
            key,
            Owned {
                count: 1,
                order,
                cleanup: Box::new(cleanup),
            },
        );
    }

    pub fn released(&self, device: u64, id: u32) {
        let key = OwnedKey { device, id };
        let mut state = self.owned.lock().unwrap();
        let Some(existing) = state.owned.get_mut(&key) else {
            return;
        };
        if existing.count == 1 {
            state.owned.remove(&key);
        } else {
            existing.count -= 1;
        }
    }

    fn discard_owned(&self) {
        let mut values: Vec<(usize, Owned)> = {
            let mut state = self.owned.lock().unwrap();
            let owned = std::mem::take(&mut state.owned);
            owned
                .into_values()
                .map(|value| (value.order, value))
                .collect()
        };
        // Newest first, matching the Swift teardown order.
        values.sort_by(|left, right| right.0.cmp(&left.0));
        for (_, mut value) in values {
            for _ in 0..value.count {
                (value.cleanup)();
            }
        }
    }

    fn emit(&self, phase: PreparationPhase, failure: Option<String>) {
        let sequence = {
            let mut sequence = self.sequence.lock().unwrap();
            *sequence += 1;
            *sequence
        };
        (self.observer)(ResourcePreparationProgress {
            phase,
            sequence,
            label: self.label.clone(),
            current_step: self.step_label.lock().unwrap().clone(),
            completed_steps: *self.completed.lock().unwrap(),
            total_steps: self.total,
            completed_compilations: *self.compilations.lock().unwrap(),
            current_compilation: self.compilation.lock().unwrap().clone(),
            failure,
        });
    }

    fn current_is(candidate: &ResourcePreparationContext) -> bool {
        CURRENT.with(|current| {
            current
                .borrow()
                .as_ref()
                .is_some_and(|current| std::ptr::eq(&**current, candidate))
        })
    }

    /// The preparation running on this thread, if any.
    pub fn current() -> Option<std::rc::Rc<ResourcePreparationContext>> {
        CURRENT.with(|current| current.borrow().clone())
    }

    /// Installs this context for the duration of `operation` and runs it.
    pub fn run<T>(
        self: std::rc::Rc<Self>,
        operation: impl FnOnce() -> RhiResult<T>,
    ) -> RhiResult<T> {
        let previous =
            CURRENT.with(|current| current.borrow_mut().replace(std::rc::Rc::clone(&self)));
        let result = (|| {
            self.check_cancellation()?;
            self.emit(PreparationPhase::Preparing, None);
            let value = operation()?;
            self.check_cancellation()?;
            if let Some(total) = self.total
                && *self.completed.lock().unwrap() != total
            {
                return Err(RhiError::invalid_argument(
                    "preparation did not complete its declared steps",
                ));
            }
            // Ownership transfers with the successfully prepared result.
            self.owned.lock().unwrap().owned.clear();
            *self.step_label.lock().unwrap() = None;
            *self.compilation.lock().unwrap() = None;
            self.emit(PreparationPhase::Finished, None);
            Ok(value)
        })();
        if let Err(error) = &result {
            self.discard_owned();
            let cancelled = *error == RhiError::Cancelled;
            self.emit(
                if cancelled {
                    PreparationPhase::Cancelled
                } else {
                    PreparationPhase::Failed
                },
                if cancelled {
                    None
                } else {
                    Some(error.to_string())
                },
            );
        }
        CURRENT.with(|current| *current.borrow_mut() = previous);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::{Arc, Mutex};

    type Events = Arc<Mutex<Vec<ResourcePreparationProgress>>>;

    fn recording_context(total: Option<usize>) -> (Rc<ResourcePreparationContext>, Events) {
        let events: Events = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&events);
        let context = Rc::new(ResourcePreparationContext::new(
            "shaders",
            total,
            move |progress| sink.lock().unwrap().push(progress),
        ));
        (context, events)
    }

    #[test]
    fn a_complete_run_reports_each_step_and_finishes() {
        let (context, events) = recording_context(Some(2));
        let value = ResourcePreparationContext::run(Rc::clone(&context), || {
            let first = context.step("upload", || Ok(1))?;
            let second = context.step("compile", || Ok(first + 1))?;
            Ok(second)
        })
        .expect("preparation");
        assert_eq!(value, 2);
        let events = events.lock().unwrap();
        assert_eq!(events.last().unwrap().phase, PreparationPhase::Finished);
        assert_eq!(events.last().unwrap().fraction(), Some(1.0));
        assert!(
            events
                .iter()
                .any(|event| event.current_step.as_deref() == Some("compile"))
        );
    }

    #[test]
    fn a_declared_step_shortfall_is_an_error() {
        let (context, _events) = recording_context(Some(3));
        let result = ResourcePreparationContext::run(Rc::clone(&context), || {
            context.step("only", || Ok(()))
        });
        assert!(result.is_err());
    }

    #[test]
    fn cancellation_reports_as_cancelled_and_runs_cleanups_newest_first() {
        let (context, events) = recording_context(None);
        let order = Rc::new(RefCell::new(Vec::new()));
        let first = Rc::clone(&order);
        let second = Rc::clone(&order);
        let result = ResourcePreparationContext::run(Rc::clone(&context), || {
            context.acquired(1, 10, move || first.borrow_mut().push(10));
            context.acquired(1, 11, move || second.borrow_mut().push(11));
            context.cancel();
            context.check_cancellation()
        });
        assert_eq!(result, Err(RhiError::Cancelled));
        assert_eq!(*order.borrow(), vec![11, 10]);
        assert_eq!(
            events.lock().unwrap().last().unwrap().phase,
            PreparationPhase::Cancelled
        );
    }

    #[test]
    fn steps_outside_their_own_preparation_are_rejected() {
        let (context, _events) = recording_context(None);
        assert!(context.step("orphan", || Ok(())).is_err());
        let other = ResourcePreparationContext::new("other", None, |_| {});
        let result =
            ResourcePreparationContext::run(Rc::new(other), || context.step("cross", || Ok(())));
        assert!(result.is_err());
    }

    #[test]
    fn released_ownership_is_not_cleaned_up() {
        let (context, _events) = recording_context(None);
        let cleaned = Rc::new(RefCell::new(0usize));
        let sink = Rc::clone(&cleaned);
        let result = ResourcePreparationContext::run(Rc::clone(&context), || {
            context.acquired(1, 5, move || *sink.borrow_mut() += 1);
            context.released(1, 5);
            context.acquired(1, 6, || {});
            context.released(1, 6);
            Ok(())
        });
        assert!(result.is_ok());
        assert_eq!(*cleaned.borrow(), 0);
    }
}
