//! Platform-neutral contract shared by the APK producer and the native host.

mod frame_clock;
mod launch;

pub use frame_clock::DisplayFrameClock;
pub use launch::{SceneLaunch, SceneLaunchMode};
