use std::{rc::Rc, time::Duration};
use cove_platform::{SystemCursor, TextInputArea, WindowId, WindowInputEvent, InputEvent};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeRenderSurface { MetalLayer(usize), Win32Window { hwnd: usize, hinstance: usize }, XlibWindow { display: usize, window: u64 }, WaylandSurface { display: usize, surface: usize }, AndroidWindow(usize) }
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WindowTitleBarStyle { #[default] Standard, HiddenInset }
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ChromeRect { pub x: f32, pub y: f32, pub width: f32, pub height: f32 }
impl ChromeRect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self { Self { x, y, width: width.max(0.0), height: height.max(0.0) } }
    pub fn contains(self, x: f32, y: f32) -> bool { x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WindowChromeHitTest { pub title_bar_height: f32, pub draggable_leading_inset: f32, pub draggable_trailing_inset: f32, pub resize_border_width: f32, pub draggable_rects: Vec<ChromeRect>, pub non_draggable_rects: Vec<ChromeRect>, pub uses_explicit_drag_rects: bool }
impl WindowChromeHitTest { pub fn new(title_bar_height: f32) -> Self { Self { title_bar_height: title_bar_height.max(0.0), resize_border_width: 6.0, ..Self::default() } } }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowOptions { pub width: i32, pub height: i32, pub title_bar_style: WindowTitleBarStyle, pub shows_dock_icon: bool }
impl Default for WindowOptions { fn default() -> Self { Self { width: 1280, height: 720, title_bar_style: WindowTitleBarStyle::Standard, shows_dock_icon: true } } }
#[derive(Debug, thiserror::Error)]
pub enum ShellError { #[error("{0}")] InitializationFailed(String), #[error("{0}")] UnsupportedPlatform(String) }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileDialogFilter { pub name: String, pub extensions: Vec<String> }
impl FileDialogFilter { pub fn pattern(&self) -> String { self.extensions.join(";") } }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeWindowReference { AppKit(usize), UIKit(usize), Win32(usize) }
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WindowSafeArea { pub top: f32, pub bottom: f32, pub leading: f32, pub trailing: f32 }
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
}
pub type DialogCompletion = Box<dyn FnOnce(Vec<String>)>;
pub trait Shell {
    fn main_window_id(&self) -> Option<WindowId>;
    fn window_ids(&self) -> Vec<WindowId>;
    fn is_running(&self) -> bool;
    fn set_close_interceptor(&mut self, interceptor: Option<Box<dyn FnMut(Option<WindowId>) -> bool>>);
    fn create_window(&mut self, title: &str, options: WindowOptions) -> Result<Rc<dyn WindowHandle>, ShellError>;
    fn window(&self, id: WindowId) -> Option<Rc<dyn WindowHandle>>;
    fn destroy_window(&mut self, id: WindowId);
    fn poll_window_events(&mut self) -> Vec<WindowInputEvent>;
    fn wait_for_events(&self, timeout: Duration) -> bool;
    fn wake_event_loop(&self);
    fn set_text_input_area(&self, id: WindowId, area: Option<TextInputArea>);
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
    fn show_open_folder_dialog(&self, id: Option<WindowId>, path: Option<&str>, accept: Box<dyn FnOnce(Option<String>)>);
    fn show_open_file_dialog(&self, id: Option<WindowId>, path: Option<&str>, filters: &[FileDialogFilter], multiple: bool, accept: DialogCompletion);
    fn display_refresh_rate(&self, id: Option<WindowId>) -> Option<f64>;
    fn set_display_refresh_enabled(&self, id: WindowId, enabled: bool) -> bool;
    fn take_display_refresh(&self, id: WindowId) -> Option<crate::DisplayRefreshTick>;
    fn shutdown(&mut self);
    fn initialize_window(&mut self, title: &str) -> Result<(), ShellError> { if self.main_window_id().is_none() { self.create_window(title, WindowOptions::default())?; } Ok(()) }
    fn poll_events(&mut self) -> Vec<InputEvent> { let main = self.main_window_id(); self.poll_window_events().into_iter().filter(|e| Some(e.window_id) == main).map(|e| e.event).collect() }
    fn drawable_size(&self) -> (u32,u32) { self.main_window_id().and_then(|id| self.window(id)).map_or((1,1),|w|w.drawable_size()) }
    fn logical_size(&self) -> (u32,u32) { self.main_window_id().and_then(|id| self.window(id)).map_or((1,1),|w|w.logical_size()) }
    fn render_surface(&self) -> Option<NativeRenderSurface> { self.main_window_id().and_then(|id| self.window(id)).and_then(|w|w.render_surface()) }
    fn is_focused(&self) -> bool { self.main_window_id().and_then(|id|self.window(id)).is_none_or(|w|w.is_focused()) }
    fn is_minimized(&self) -> bool { self.main_window_id().and_then(|id|self.window(id)).is_some_and(|w|w.is_minimized()) }
    fn is_occluded(&self) -> bool { self.main_window_id().and_then(|id|self.window(id)).is_some_and(|w|w.is_occluded()) }
}
