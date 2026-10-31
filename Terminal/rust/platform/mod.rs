//! Host integration points the terminal surface needs from the platform layer.
//!
//! The Swift module reached these through `CoveUIRuntime` (`UIWorkSchedulerHolder`,
//! `FocusChainHolder`, `ClipboardHolder`, `InteractionRegistryHolder`) and through
//! platform events. The Rust runtime keeps the same holders in `cove_core::scene`
//! and `cove_platform`, so this module is the single place that names them for
//! the terminal, instead of every view file depending on the runtime layout.

pub use cove_platform::{
    KeyEvent, KeyModifiers, MouseButton, MouseButtonEvent, MouseMotionEvent, MouseWheelEvent,
    ScrollUnit, SystemCursor, TextEditingEvent, TextInputArea,
};

/// Pasteboard bridge used by Cmd-C and Cmd-V.
pub use cove_core::scene::ClipboardHolder;
/// Pointer capture, so a drag that leaves the surface keeps selecting.
pub use cove_core::scene::PointerCaptureHolder;
/// How the host wants terminal work scheduled onto its UI thread.
pub use cove_core::scene::UIWorkSchedulerHolder;
/// Handler registration shared with the rest of the Compose input system.
pub use cove_core::scene::{
    EventPhase, EventResult, InputHandlerRoute, InteractionRegistry, InteractionRegistryHolder,
};
/// Focus tracking, so the cursor blinks only while the surface is focused.
pub use cove_core::scene::{FocusChain, FocusChainHolder};

/// The scene clock that drives cursor and text blinking.
pub use cove_core::scene::AnimatorScheduler;

/// Reduced-motion preference, honoured by the blink timers.
pub use cove_core::scene::ReducedMotionEnvironment;

/// Whether the host has a usable surface for a terminal at all.
///
/// Terminals need a pseudo terminal, which every desktop platform provides; the
/// predicate exists so a host can hide the surface rather than fail to spawn.
pub const fn supports_pseudo_terminal() -> bool {
    cfg!(any(unix, windows))
}

/// Path seam used by hosts that open hyperlinks themselves.
pub fn open_external_link(link: &str) {
    crate::view::link_opener::open(link);
}
