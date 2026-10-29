mod signal;
pub use signal::{DisplayRefreshTick, DisplayRefreshInbox};
// WindowDisplayLink, CoreVideo and MacDisplayLink use the native driver in
// apple.m. CADisplayLink follows the window's screen; CoreVideo is its fallback.
