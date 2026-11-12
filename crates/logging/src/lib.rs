//! Bounded structured logging, redaction, filtering, history and rotating JSONL output.
//!
//! The module layout follows the Swift packages. `CoveLogCore` supplies the
//! record, configuration and environment model (`record`, `context`,
//! `configuration`, `environment`, `exports`); `CoveLogging` supplies the
//! unified runtime, outputs and the process-wide system (`handler`, `runtime`,
//! `system`, `executor`, `sanitizer`, `history`, `observation`, `formatting`,
//! `file`).
pub mod configuration;
pub mod context;
pub mod environment;
pub mod executor;
mod exports;
pub mod file;
pub mod formatting;
pub mod handler;
pub mod history;
mod logging_exports;
pub mod observation;
pub mod record;
pub mod runtime;
pub mod sanitizer;
pub mod system;

pub use logging_exports::*;

pub use configuration::LoggingConfiguration;

/// Process identity for log records. WASI has no process ids and Rust's
/// `std::process::id()` panics there ("no pids on this platform"), so the Wasm
/// reactor reports 0 instead of aborting during logging bootstrap.
pub(crate) fn process_id() -> u32 {
    #[cfg(target_arch = "wasm32")]
    {
        0
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::process::id()
    }
}
