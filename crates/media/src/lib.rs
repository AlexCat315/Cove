//! Optional media integration. Control and retained output do not depend on a UI or GPU.
mod audio;
mod control;
mod frame;
mod info;
mod kernel;
mod source;
mod types;
pub use audio::*;
pub use control::*;
pub use frame::*;
pub use info::*;
pub use kernel::{SampleStats, channel_stats, sample_stats};
pub use source::*;
pub use types::*;

/// Locating the FFmpeg command-line tools used by export and probing.
#[cfg(not(target_arch = "wasm32"))]
mod tools;
#[cfg(not(target_arch = "wasm32"))]
pub use tools::{FfmpegTools, ToolError, ToolSearch, resolve as resolve_ffmpeg_tools};


#[cfg(all(feature = "ffmpeg", not(target_arch = "wasm32")))]
mod engine;
#[cfg(all(feature = "ffmpeg", not(target_arch = "wasm32")))]
mod http;
#[cfg(all(feature = "ffmpeg", not(target_arch = "wasm32")))]
pub use http::HttpInput;
#[cfg(all(feature = "ffmpeg", not(target_arch = "wasm32")))]
mod native;
#[cfg(all(feature = "ffmpeg", not(target_arch = "wasm32")))]
pub use engine::{AudioRender, AudioStream, AudioStreamOptions, DecodedFrame, MediaDecoder, probe};
#[cfg(all(feature = "ffmpeg", not(target_arch = "wasm32")))]
pub use native::{MediaContext, Player, audio_output_devices};
/// Advanced entry point: the raw FFmpeg bindings plus retained CPU frame storage.
#[cfg(all(feature = "ffmpeg", not(target_arch = "wasm32")))]
pub mod ffmpeg;
