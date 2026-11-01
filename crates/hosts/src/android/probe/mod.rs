//! Translation of Android/Sources/CoveAndroidProbe.
//!
//! The Swift target was an executable; this module exposes the same sequence as
//! [`main::main`] for the probe binary the Android build links.

pub mod layout;
pub mod main;
pub mod raster;
pub mod runner;
pub mod transfer;
pub mod ui;

pub use runner::{ProbeContext, ProbeFailure, ProbeReport, ProbeResult, require};
