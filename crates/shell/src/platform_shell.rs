use cove_platform::{
    InputEvent, NativeMenuSnapshot, ScreenColorCompletion, ScreenColorSample, SystemCursor,
    TextInputArea, WindowId, WindowInputEvent,
};
use std::{rc::Rc, sync::Arc, time::Duration};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeRenderSurface {
    MetalLayer(usize),
    Win32Window { hwnd: usize, hinstance: usize },
    XlibWindow { display: usize, window: u64 },
    WaylandSurface { display: usize, surface: usize },
    AndroidWindow(usize),
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WindowTitleBarStyle {
    #[default]
    Standard,
    HiddenInset,
}
/// Light or dark native chrome: title bar, window controls and materials.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WindowAppearance {
    #[default]
    Light,
    Dark,
}
/// What shows through where the UI leaves the window clear.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WindowBackdrop {
    /// The UI paints every pixel.
    #[default]
    Opaque,
    /// The platform's blurred sidebar material, the way a macOS source list
    /// shows the desktop through it.
    Sidebar,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ChromeRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
impl ChromeRect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width: width.max(0.0),
            height: height.max(0.0),
        }
    }
    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WindowChromeHitTest {
    pub title_bar_height: f32,
    pub draggable_leading_inset: f32,
    pub draggable_trailing_inset: f32,
    pub resize_border_width: f32,
    pub draggable_rects: Vec<ChromeRect>,
    pub non_draggable_rects: Vec<ChromeRect>,
    pub uses_explicit_drag_rects: bool,
}
impl WindowChromeHitTest {
    pub fn new(title_bar_height: f32) -> Self {
        Self {
            title_bar_height: title_bar_height.max(0.0),
            resize_border_width: 6.0,
            ..Self::default()
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowOptions {
    pub width: i32,
    pub height: i32,
    pub title_bar_style: WindowTitleBarStyle,
    pub shows_dock_icon: bool,
    pub backdrop: WindowBackdrop,
}
impl Default for WindowOptions {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            title_bar_style: WindowTitleBarStyle::Standard,
            shows_dock_icon: true,
            backdrop: WindowBackdrop::Opaque,
        }
    }
}
#[derive(Debug, thiserror::Error)]
pub enum ShellError {
    #[error("{0}")]
    InitializationFailed(String),
    #[error("{0}")]
    UnsupportedPlatform(String),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileDialogFilter {
    pub name: String,
    pub extensions: Vec<String>,
}
impl FileDialogFilter {
    pub fn pattern(&self) -> String {
        self.extensions.join(";")
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeWindowReference {
    AppKit(usize),
    UIKit(usize),
    Win32(usize),
}
/// Hardware form the shell can observe. Phones are not distinguished here:
/// handheld hosts report their class themselves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DeviceForm {
    #[default]
    Computer,
    /// iPad, or an Android tablet running the SDL path.
    Tablet,
    /// A convertible PC in slate mode with an integrated touch screen.
    TouchComputer,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WindowSafeArea {
    pub top: f32,
    pub bottom: f32,
    pub leading: f32,
    pub trailing: f32,
}
pub trait WindowHandle {
    fn id(&self) -> WindowId;
    fn native_window_reference(&self) -> Option<NativeWindowReference>;
    fn render_surface(&self) -> Option<NativeRenderSurface>;
    fn safe_area(&self) -> WindowSafeArea;
    fn drawable_size(&self) -> (u32, u32);
    fn logical_size(&self) -> (u32, u32);
    fn content_scale_factor(&self) -> f32;
    fn is_focused(&self) -> bool;
    fn is_visible(&self) -> bool;
    fn is_minimized(&self) -> bool;
    fn is_occluded(&self) -> bool;
    /// The backdrop the platform actually applied; `Opaque` where the
    /// requested material is unavailable.
    fn backdrop(&self) -> WindowBackdrop {
        WindowBackdrop::Opaque
    }
}
/// Every window fact a thread other than the platform thread may read. The
/// platform thread publishes it; readers never call the platform.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WindowSnapshot {
    pub drawable_size: (u32, u32),
    pub logical_size: (u32, u32),
    pub content_scale: f32,
    pub focused: bool,
    pub visible: bool,
    pub minimized: bool,
    pub occluded: bool,
    pub maximized: bool,
    pub fullscreen: bool,
    pub safe_area: WindowSafeArea,
    pub position: Option<(f32, f32)>,
    pub refresh_rate: Option<f64>,
    pub surface: Option<NativeRenderSurface>,
    pub native_window: Option<NativeWindowReference>,
    pub backdrop: WindowBackdrop,
}
pub type DialogCompletion = Box<dyn FnOnce(Vec<String>)>;
pub type ClipboardCompletion = Box<dyn FnOnce(Option<String>)>;
pub type MenuCompletion = Box<dyn FnOnce(Option<i64>)>;
/// Wakes a blocked event loop from any thread.
pub type EventLoopWaker = Arc<dyn Fn() + Send + Sync>;
pub trait Shell {
    fn main_window_id(&self) -> Option<WindowId>;
    fn window_ids(&self) -> Vec<WindowId>;
    fn is_running(&self) -> bool;
    fn set_close_interceptor(
        &mut self,
        interceptor: Option<Box<dyn FnMut(Option<WindowId>) -> bool>>,
    );
    fn create_window(
        &mut self,
        title: &str,
        options: WindowOptions,
    ) -> Result<Rc<dyn WindowHandle>, ShellError>;
    fn window(&self, id: WindowId) -> Option<Rc<dyn WindowHandle>>;
    fn destroy_window(&mut self, id: WindowId);
    fn poll_window_events(&mut self) -> Vec<WindowInputEvent>;
    /// Execute the returned work after releasing any external shell borrow.
    fn take_deferred_callbacks(&self) -> Vec<Box<dyn FnOnce()>> {
        Vec::new()
    }
    fn wait_for_events(&self, timeout: Duration) -> bool;
    fn wake_event_loop(&self);
    fn set_text_input_area(&self, id: WindowId, area: Option<TextInputArea>);
    /// Explicit on-screen keyboard control for touch hosts. Shells without a
    /// software keyboard keep the native text-input focus path unchanged.
    fn show_soft_keyboard(&self, _id: WindowId) {}
    fn hide_soft_keyboard(&self, _id: WindowId) {}
    /// Keyboard inset in logical points; zero when hidden or on desktop.
    fn keyboard_height(&self, _id: WindowId) -> f32 {
        0.0
    }
    /// Read every frame: a convertible can enter slate mode at any time.
    fn device_form(&self) -> DeviceForm {
        DeviceForm::Computer
    }
    fn set_cursor(&self, id: WindowId, cursor: SystemCursor);
    fn global_pointer_position(&self) -> Option<(f32, f32)>;
    fn window_position(&self, id: WindowId) -> Option<(f32, f32)>;
    fn set_window_position(&self, id: WindowId, x: f32, y: f32);
    fn set_window_size(&self, id: WindowId, width: f32, height: f32);
    fn raise_window(&self, id: WindowId);
    fn minimize_window(&self, id: WindowId);
    fn maximize_window(&self, id: WindowId);
    fn restore_window(&self, id: WindowId);
    fn hide_window(&self, id: WindowId);
    fn show_window(&self, id: WindowId);
    fn is_window_maximized(&self, id: WindowId) -> bool;
    fn show_window_system_menu(&self, id: WindowId, x: f32, y: f32);
    fn set_window_chrome_hit_test(&self, id: WindowId, hit: Option<&WindowChromeHitTest>);
    /// Makes the native chrome follow the UI theme instead of the system.
    fn set_window_appearance(&self, _id: WindowId, _appearance: WindowAppearance) {}
    /// Enters or leaves full screen (a separate Space on macOS). Shells without
    /// window management ignore it and keep reporting `false`.
    fn set_window_fullscreen(&self, _id: WindowId, _fullscreen: bool) {}
    fn is_window_fullscreen(&self, _id: WindowId) -> bool {
        false
    }
    /// Keeps the window above normal windows, including other applications'
    /// full-screen Spaces on macOS: the basis of picture-in-picture windows.
    fn set_window_always_on_top(&self, _id: WindowId, _on_top: bool) {}
    fn show_open_folder_dialog(
        &self,
        id: Option<WindowId>,
        path: Option<&str>,
        accept: Box<dyn FnOnce(Option<String>)>,
    );
    fn show_open_file_dialog(
        &self,
        id: Option<WindowId>,
        path: Option<&str>,
        filters: &[FileDialogFilter],
        multiple: bool,
        accept: DialogCompletion,
    );
    fn display_refresh_rate(&self, id: Option<WindowId>) -> Option<f64>;
    fn set_display_refresh_enabled(&self, id: WindowId, enabled: bool) -> bool;
    fn take_display_refresh(&self, id: WindowId) -> Option<crate::DisplayRefreshTick>;
    fn shutdown(&mut self);
    /// Claims the native event queue before any window exists, so a wake can
    /// land and a wait can block from the first loop iteration.
    fn prepare_event_loop(&mut self) -> Result<(), ShellError> {
        Ok(())
    }
    /// A window creation that failed after `create_window` already returned.
    fn take_startup_error(&mut self) -> Option<ShellError> {
        None
    }
    fn waker(&self) -> EventLoopWaker {
        Arc::new(crate::Sdl3Shell::wake)
    }
    fn write_clipboard(&self, text: &str) {
        crate::Sdl3Clipboard::write(text);
    }
    /// `done` runs after the clipboard was read, never with a shell borrow held
    /// by a threaded implementation.
    fn read_clipboard(&self, done: ClipboardCompletion) {
        done(crate::Sdl3Clipboard::read());
    }
    /// Starts a screen-colour pick. `done` is called exactly once on the
    /// platform thread; native backends answer later and must never block this
    /// call. Hosts without a native sampler report `Unsupported` so the UI can
    /// hide the affordance.
    fn sample_screen_color(&self, done: ScreenColorCompletion) {
        done(ScreenColorSample::Unsupported(
            "this shell has no screen-colour backend".into(),
        ));
    }
    /// Shows a native menu and reports the chosen item through `done`.
    fn present_menu(
        &self,
        id: Option<WindowId>,
        snapshot: &NativeMenuSnapshot,
        x: f32,
        y: f32,
        done: MenuCompletion,
    ) {
        let window = id
            .and_then(|id| self.window(id))
            .and_then(|window| window.native_window_reference());
        done(
            crate::native_menu_popover::NativeMenuPopover::pop_up(snapshot, x, y, window)
                .ok()
                .flatten(),
        );
    }
    fn window_snapshot(&self, id: WindowId) -> Option<WindowSnapshot> {
        let window = self.window(id)?;
        Some(WindowSnapshot {
            drawable_size: window.drawable_size(),
            logical_size: window.logical_size(),
            content_scale: window.content_scale_factor(),
            focused: window.is_focused(),
            visible: window.is_visible(),
            minimized: window.is_minimized(),
            occluded: window.is_occluded(),
            maximized: self.is_window_maximized(id),
            fullscreen: self.is_window_fullscreen(id),
            safe_area: window.safe_area(),
            position: self.window_position(id),
            refresh_rate: self.display_refresh_rate(Some(id)),
            surface: window.render_surface(),
            native_window: window.native_window_reference(),
            backdrop: window.backdrop(),
        })
    }
    fn initialize_window(&mut self, title: &str) -> Result<(), ShellError> {
        if self.main_window_id().is_none() {
            self.create_window(title, WindowOptions::default())?;
        }
        Ok(())
    }
    fn poll_events(&mut self) -> Vec<InputEvent> {
        let main = self.main_window_id();
        self.poll_window_events()
            .into_iter()
            .filter(|e| Some(e.window_id) == main)
            .map(|e| e.event)
            .collect()
    }
    fn drawable_size(&self) -> (u32, u32) {
        self.main_window_id()
            .and_then(|id| self.window(id))
            .map_or((1, 1), |w| w.drawable_size())
    }
    fn logical_size(&self) -> (u32, u32) {
        self.main_window_id()
            .and_then(|id| self.window(id))
            .map_or((1, 1), |w| w.logical_size())
    }
    fn render_surface(&self) -> Option<NativeRenderSurface> {
        self.main_window_id()
            .and_then(|id| self.window(id))
            .and_then(|w| w.render_surface())
    }
    fn is_focused(&self) -> bool {
        self.main_window_id()
            .and_then(|id| self.window(id))
            .is_none_or(|w| w.is_focused())
    }
    fn is_minimized(&self) -> bool {
        self.main_window_id()
            .and_then(|id| self.window(id))
            .is_some_and(|w| w.is_minimized())
    }
    fn is_occluded(&self) -> bool {
        self.main_window_id()
            .and_then(|id| self.window(id))
            .is_some_and(|w| w.is_occluded())
    }
}
