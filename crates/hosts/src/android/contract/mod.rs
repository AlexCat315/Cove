//! Platform-neutral contract shared by the APK producer and the native host.

pub mod frame_clock;
pub mod launch;

pub use frame_clock::DisplayFrameClock;
pub use launch::{SceneLaunch, SceneLaunchMode};
