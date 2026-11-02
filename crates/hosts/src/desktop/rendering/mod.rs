//! Translation of Desktop/Sources/CoveUI/Rendering.

pub mod context;
pub mod render_thread;

pub use context::AppRenderingContext;
pub use render_thread::{FramePresented, FrameToken, MAX_FRAMES_IN_FLIGHT, ThreadedWindowRenderer};
