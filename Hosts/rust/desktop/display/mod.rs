//! Translation of Desktop/Sources/CoveUI/Display.

pub mod controls;
pub mod handle;

pub use controls::{MainWindowControls, PresentationControls, WindowControls};
pub use handle::{AppAuxiliaryWindowRequest, AppDisplayHandle, DisplayBindings, DisplaySignal};
