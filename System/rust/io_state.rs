//! Translation of `System/Sources/CoveProcess/IOState.swift`.
use super::command::ProcessError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// Cancellation handle replacing `DispatchSourceTimer`. The deadline thread
/// polls the flag so cancellation never blocks the caller.
pub struct ProcessTimer {
    cancelled: Arc<AtomicBool>,
    _worker: Option<JoinHandle<()>>,
}

impl ProcessTimer {
    pub fn new(cancelled: Arc<AtomicBool>, worker: JoinHandle<()>) -> Self {
        Self {
            cancelled,
            _worker: Some(worker),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

impl Drop for ProcessTimer {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

#[derive(Default)]
struct State {
    pending_input: usize,
    pipes_closed: u32,
    exit_status: Option<i32>,
    stopped: bool,
    input_closed: bool,
    timer: Option<ProcessTimer>,
}

#[derive(Default)]
pub struct ProcessIOState {
    state: Mutex<State>,
}

impl ProcessIOState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn install_timer(&self, timer: ProcessTimer) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.stopped {
            timer.cancel();
        } else {
            state.timer = Some(timer);
        }
    }

    pub fn cancel_timer(&self) {
        let timer = self
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .timer
            .take();
        if let Some(timer) = timer {
            timer.cancel();
        }
    }

    pub fn enqueue(&self, count: usize, maximum: usize) -> Result<(), ProcessError> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.stopped || state.input_closed || state.exit_status.is_some() {
            return Err(ProcessError::Stopped);
        }
        if count > maximum.saturating_sub(state.pending_input) {
            return Err(ProcessError::InputLimit);
        }
        state.pending_input += count;
        Ok(())
    }

    pub fn wrote(&self, count: usize) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.pending_input = state.pending_input.saturating_sub(count);
    }

    pub fn close_input(&self) {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .input_closed = true;
    }

    pub fn stop(&self) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let first = !state.stopped;
        state.stopped = true;
        first
    }

    pub fn closed_pipe(&self) -> Option<i32> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.pipes_closed += 1;
        if state.pipes_closed == 2 {
            state.exit_status
        } else {
            None
        }
    }

    pub fn terminated(&self, status: i32) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.exit_status = Some(status);
        state.pipes_closed == 2
    }
}
