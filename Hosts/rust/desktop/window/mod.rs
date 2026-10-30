//! Translation of Desktop/Sources/CoveUI/Window.

pub mod auxiliary;
pub mod chrome;
pub mod native_menu;
pub mod native_renderer;
pub mod state;

pub use auxiliary::AuxiliaryAppWindow;
pub use chrome::{
    AppWindowChromeContext, AppWindowChromeContextHolder, ImmersiveWindowControlStyle,
    WindowChromeHitTestCollector,
};
pub use native_menu::{
    NativeApplicationMenuTitles, NativeMenu, NativeMenuAction, NativeMenuBar, NativeMenuItem,
    NativeMenuKeyModifiers,
};
pub use native_renderer::{AppWindowRenderSettings, AppWindowRenderer, NativeAppWindowRenderer};
pub use state::MainWindowState;
