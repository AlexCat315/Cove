//! Translation of Android/Integration/Surface/State.swift.

mod device_facts;
mod state;

pub use device_facts::{DeviceFacts, FEATURE_STRIDE, MAXIMUM_FEATURES};
pub use state::{SurfaceOverrides, SurfaceState};
