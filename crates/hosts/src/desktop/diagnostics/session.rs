//! Translation of Desktop/Sources/CoveUI/Diagnostics/Session.swift.
//!
//! The Swift type held the DevTools session and the query facade. `cove-hosts`
//! does not link DevTools (see `diagnostics::integration`), so the session slot
//! holds the injected session object and the query facade is the host-owned
//! memory/capture surface the DevTools adapter reads.

use super::integration::{DiagnosticMemorySnapshot, DiagnosticsSession};

/// Capture control exposed to DevTools for the running window.
#[derive(Default)]
pub struct DiagnosticsQueries {
    /// Bytes the host can reclaim right now, per window.
    pub memory: Option<std::rc::Rc<dyn Fn() -> DiagnosticMemorySnapshot>>,
    captures: std::cell::Cell<bool>,
}

impl DiagnosticsQueries {
    pub fn reset_captures(&self) {
        self.captures.set(false);
    }

    pub fn is_capturing(&self) -> bool {
        self.captures.get()
    }

    pub fn begin_capture(&self) {
        self.captures.set(true);
    }
}

/// Application-owned diagnostics state, created once per run loop.
#[derive(Default)]
pub struct ApplicationDiagnostics {
    pub dev_tools: Option<Box<dyn DiagnosticsSession>>,
    pub queries: Option<DiagnosticsQueries>,
}

impl ApplicationDiagnostics {
    /// Drops the DevTools session and its capture state, mirroring the Swift
    /// teardown ordering.
    pub fn stop(&mut self) {
        if let Some(mut session) = self.dev_tools.take() {
            session.stop();
        }
        if let Some(queries) = &self.queries {
            queries.reset_captures();
        }
        self.queries = None;
    }
}
