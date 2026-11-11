//! Translation of Desktop/Sources/CoveUI/Window.

pub mod appearance;
pub mod auxiliary;
pub mod chrome;
pub mod chrome_views;
#[cfg(target_os = "windows")]
mod immersive_menu;
pub mod native_menu;
pub mod native_renderer;
pub mod state;

pub use auxiliary::AuxiliaryAppWindow;
pub use chrome::{
    AppWindowChromeContext, AppWindowChromeContextHolder, ImmersiveWindowControlStyle,
    WindowChromeHitTestCollector,
};
pub use chrome_views::{
    ImmersiveWindowTitleBar, WindowChromeFixedSpace, WindowChromeInteractiveRegion,
    WindowControlButton, WindowControlGlyph, WindowControlStrip, WindowDragRegion,
};
#[cfg(target_os = "windows")]
pub use immersive_menu::ImmersiveMenuBar;
pub use native_menu::{
    NativeApplicationMenuTitles, NativeMenu, NativeMenuAction, NativeMenuBar, NativeMenuItem,
    NativeMenuKeyModifiers,
};
pub use native_renderer::{AppWindowRenderSettings, AppWindowRenderer, NativeAppWindowRenderer};
pub use state::MainWindowState;
