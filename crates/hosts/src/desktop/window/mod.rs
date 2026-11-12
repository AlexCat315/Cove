//! Translation of Desktop/Sources/CoveUI/Window.

#[cfg(not(target_arch = "wasm32"))]
pub mod appearance;
#[cfg(not(target_arch = "wasm32"))]
pub mod auxiliary;
#[cfg(not(target_arch = "wasm32"))]
pub mod chrome;
#[cfg(not(target_arch = "wasm32"))]
pub mod chrome_views;
#[cfg(target_os = "windows")]
mod immersive_menu;
pub mod native_menu;
#[cfg(not(target_arch = "wasm32"))]
pub mod native_renderer;
#[cfg(not(target_arch = "wasm32"))]
pub mod state;

#[cfg(not(target_arch = "wasm32"))]
pub use auxiliary::AuxiliaryAppWindow;
#[cfg(not(target_arch = "wasm32"))]
pub use chrome::{
    AppWindowChromeContext, AppWindowChromeContextHolder, ImmersiveWindowControlStyle,
    WindowChromeHitTestCollector,
};
#[cfg(not(target_arch = "wasm32"))]
pub use chrome_views::{
    ImmersiveWindowTitleBar, WindowChromeFixedSpace, WindowChromeInteractiveRegion,
    WindowControlButton, WindowControlGlyph, WindowControlStrip, WindowDragRegion,
};
#[cfg(target_os = "windows")]
pub(crate) use immersive_menu::ImmersiveMenuBar;
pub use native_menu::{
    NativeApplicationMenuTitles, NativeMenu, NativeMenuAction, NativeMenuBar, NativeMenuItem,
    NativeMenuKeyModifiers,
};
#[cfg(not(target_arch = "wasm32"))]
pub use native_renderer::{AppWindowRenderSettings, AppWindowRenderer, NativeAppWindowRenderer};
#[cfg(not(target_arch = "wasm32"))]
pub use state::MainWindowState;
