//! Translation of `System/Sources/CoveProcess/EventBuffer.swift`.
use super::command::{ProcessError, ProcessEvent};
use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ProcessEventBufferError {
    #[error("process events already have a consumer")]
    MultipleConsumers,
}

#[derive(Debug, thiserror::Error)]
pub enum ProcessFailure {
    #[error(transparent)]
    Process(#[from] ProcessError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("process operation was cancelled")]
    Cancelled,
    #[error(transparent)]
    Buffer(#[from] ProcessEventBufferError),
}

pub type Failure = Arc<ProcessFailure>;

impl ProcessFailure {
    /// Shares one failure between the buffer and every reader it wakes.
    pub fn shared(self) -> Failure {
        Arc::new(self)
    }
}

/// Bytes an event is charged against the buffer budget. Terminal events cost
/// nothing, so a full buffer still accepts the exit notice.
pub trait ProcessEventSize {
    fn size(&self) -> usize;
}

impl ProcessEventSize for ProcessEvent {
    fn size(&self) -> usize {
        match self {
            Self::Stdout(data) | Self::Stderr(data) => data.len(),
            Self::Exited(_) | Self::StdoutClosed | Self::StderrClosed => 0,
        }
    }
}

impl ProcessEventSize for crate::process::ProcessEvent {
    fn size(&self) -> usize {
        match self {
            Self::Stdout(data) | Self::Stderr(data) => data.len(),
            Self::Exit { .. } => 0,
        }
    }
}

struct State<E> {
    events: VecDeque<E>,
    bytes: usize,
    waiting: bool,
    completion: Option<Result<(), Failure>>,
}

/// Bounded, single-consumer queue whose reader threads and deadline signal the
/// same condition.
pub struct ProcessEventBuffer<E = ProcessEvent> {
    maximum_bytes: usize,
    state: Mutex<State<E>>,
    signal: Condvar,
}

impl<E> ProcessEventBuffer<E> {
    pub fn new(maximum_bytes: usize) -> Self {
        Self {
            maximum_bytes,
            state: Mutex::new(State {
                events: VecDeque::new(),
                bytes: 0,
                waiting: false,
                completion: None,
            }),
            signal: Condvar::new(),
        }
    }
}

impl<E: ProcessEventSize> ProcessEventBuffer<E> {
    /// Returns `false` when the event does not fit; the caller reports the
    /// output limit. Events after completion are accepted and discarded.
    pub fn append(&self, event: E) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.completion.is_some() {
            return true;
        }
        let size = event.size();
        if size > self.maximum_bytes.saturating_sub(state.bytes) {
            return false;
        }
        state.events.push_back(event);
        state.bytes += size;
        self.signal.notify_all();
        true
    }

    pub fn finish(&self) {
        self.finish_with(None);
    }

    pub fn finish_with(&self, error: Option<Failure>) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.completion.is_some() {
            return;
        }
        state.completion = Some(match error {
            Some(error) => Err(error),
            None => Ok(()),
        });
        self.signal.notify_all();
    }

    pub fn next(&self) -> Result<Option<E>, Failure> {
        self.next_until(None)
    }

    /// Waits for one available event, the completion result or the deadline.
    /// Only one consumer may wait at a time.
    pub fn next_until(&self, deadline: Option<Instant>) -> Result<Option<E>, Failure> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.waiting {
            return Err(Arc::new(ProcessEventBufferError::MultipleConsumers.into()));
        }
        state.waiting = true;
        let result = loop {
            if let Some(event) = state.events.pop_front() {
                state.bytes = state.bytes.saturating_sub(event.size());
                break Ok(Some(event));
            }
            if let Some(completion) = &state.completion {
                break completion.clone().map(|()| None);
            }
            state = match deadline {
                Some(deadline) => {
                    let now = Instant::now();
                    if now >= deadline {
                        break Err(Arc::new(ProcessError::TimedOut.into()));
                    }
                    self.signal
                        .wait_timeout(state, deadline - now)
                        .unwrap_or_else(|error| error.into_inner())
                        .0
                }
                None => self
                    .signal
                    .wait(state)
                    .unwrap_or_else(|error| error.into_inner()),
            };
        };
        state.waiting = false;
        result
    }

    /// Synchronous discovery shares the same single-consumer rule.
    pub fn next_synchronously(
        &self,
        deadline: Option<Instant>,
    ) -> Result<Option<E>, Failure> {
        self.next_until(deadline)
    }
}
