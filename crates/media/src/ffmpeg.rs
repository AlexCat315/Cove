//! Raw FFmpeg bindings and the retained CPU frame storage this backend publishes.
//!
//! Integrations that own their demux/decode loop can build [`crate::VideoFrame`]
//! values with [`FrameStorage`] and share them through [`crate::VideoOutput`].
pub use crate::engine::FrameStorage;
pub use ffmpeg_sys_next as sys;
