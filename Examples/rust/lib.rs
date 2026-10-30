//! Example applications: gallery content, demo shell, mobile content, pet demo.
//!
//! Swift sources live under `Examples/`; each module names the files it ports.
pub mod counter;
pub mod demo;
pub mod gallery_content;
pub mod mobile;
pub mod pet_demo;

pub use counter::{
    CounterDebugName, CounterText, CounterTextRun, CounterView, ProvidedPainter, MAXIMUM_COUNT,
    MAXIMUM_NOTE_LENGTH,
};
