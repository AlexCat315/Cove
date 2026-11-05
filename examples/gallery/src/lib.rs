//! Example applications: gallery content, demo shell, mobile content, pet demo.
//!
//! Swift sources live under `examples/gallery/`; each module names the files it ports.

// Showcase stories keep Swift's builder style with many small field assignments.
#![allow(clippy::field_reassign_with_default, clippy::type_complexity)]

pub mod counter;
pub mod demo;
pub mod gallery_content;
pub mod mobile;
pub mod pet_demo;

pub use counter::{
    CounterDebugName, CounterText, CounterTextRun, CounterView, MAXIMUM_COUNT, MAXIMUM_NOTE_LENGTH,
    ProvidedPainter,
};
