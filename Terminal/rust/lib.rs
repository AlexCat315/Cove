//! Ghostty-style terminal engine, PTY/ConPTY process and terminal view.
//!
//! Translation of the Swift `CoveUITerminal` module. The emulator itself is
//! libghostty-vt; this crate owns the process, the render-ready snapshot and the
//! Compose-side surface.
pub mod engine;
