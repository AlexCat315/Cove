//! Optional media integration. Control and retained output do not depend on a UI or GPU.
mod frame;
mod source;
mod types;
pub use frame::*;
pub use source::*;
pub use types::*;

#[cfg(all(feature = "ffmpeg", not(target_arch = "wasm32")))]
mod engine;
#[cfg(all(feature = "ffmpeg", not(target_arch = "wasm32")))]
mod native;
#[cfg(all(feature = "ffmpeg", not(target_arch = "wasm32")))]
pub use native::{MediaContext, Player};
/// Advanced entry point: the raw FFmpeg bindings plus retained CPU frame storage.
#[cfg(all(feature = "ffmpeg", not(target_arch = "wasm32")))]
pub mod ffmpeg;
