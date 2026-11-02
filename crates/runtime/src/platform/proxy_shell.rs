//! The [`Shell`] a UI thread sees: writes are posted, reads come from the
//! published state, and results arrive as deferred callbacks. No method waits
//! for the platform thread.
use super::{DialogRequest, PlatformCommand, PlatformLink, PlatformMessage};
use cove_platform::{NativeMenuSnapshot, SystemCursor, TextInputArea, WindowId, WindowInputEvent};
use cove_shell::{
    ClipboardCompletion, DialogCompletion, DisplayRefreshTick, EventLoopWaker, FileDialogFilter,
    MenuCompletion, NativeRenderSurface, NativeWindowReference, Shell, ShellError,
    WindowChromeHitTest, WindowHandle, WindowOptions, WindowSafeArea, WindowSnapshot,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::Arc,
    time::Duration,
};

struct ProxyWindow {
    id: WindowId,
    link: Arc<PlatformLink>,
}

impl ProxyWindow {
    fn snapshot(&self) -> WindowSnapshot {
        self.link.state.window(self.id).unwrap_or_default()
    }
}

impl WindowHandle for ProxyWindow {
    fn id(&self) -> WindowId {
        self.id
    }
    fn native_window_reference(&self) -> Option<NativeWindowReference> {
        self.snapshot().native_window
    }
    fn render_surface(&self) -> Option<NativeRenderSurface> {
        self.snapshot().surface
    }
    fn safe_area(&self) -> WindowSafeArea {
        self.snapshot().safe_area
    }
    fn drawable_size(&self) -> (u32, u32) {
        self.snapshot().drawable_size
    }
    fn logical_size(&self) -> (u32, u32) {
        self.snapshot().logical_size
    }
    fn content_scale_factor(&self) -> f32 {
        self.snapshot().content_scale
    }
    fn is_focused(&self) -> bool {
        self.snapshot().focused
    }
    fn is_visible(&self) -> bool {
        self.snapshot().visible
    }
    fn is_minimized(&self) -> bool {
        self.snapshot().minimized
    }
    fn is_occluded(&self) -> bool {
        self.snapshot().occluded
    }
}

#[derive(Default)]
struct Pending {
    next: u64,
    dialogs: HashMap<u64, DialogCompletion>,
    menus: HashMap<u64, MenuCompletion>,
    clipboard: HashMap<u64, ClipboardCompletion>,
    finished: Vec<Box<dyn FnOnce()>>,
}

impl Pending {
    fn allocate(&mut self) -> u64 {
        self.next = self.next.wrapping_add(1);
        self.next
    }
}

pub struct ProxyShell {
    link: Arc<PlatformLink>,
    windows: HashMap<WindowId, Rc<ProxyWindow>>,
    order: Vec<WindowId>,
    next_window: u32,
    quitting: bool,
    close_interceptor: Option<Box<dyn FnMut(Option<WindowId>) -> bool>>,
    pending: RefCell<Pending>,
    refresh_requested: RefCell<HashMap<WindowId, bool>>,
    seen_generation: Cell<u64>,
    startup_error: Option<ShellError>,
    shut_down: bool,
}

impl ProxyShell {
    pub fn new(link: Arc<PlatformLink>) -> Self {
        Self {
            link,
            windows: HashMap::new(),
            order: Vec::new(),
            next_window: 0,
            quitting: false,
            close_interceptor: None,
            pending: RefCell::default(),
            refresh_requested: RefCell::default(),
            seen_generation: Cell::new(0),
            startup_error: None,
            shut_down: false,
        }
    }

    fn post(&self, command: PlatformCommand) {
        self.link.bridge.post(command);
    }

    fn forget_window(&mut self, id: WindowId) {
        self.windows.remove(&id);
        self.order.retain(|entry| *entry != id);
        self.refresh_requested.borrow_mut().remove(&id);
        self.link.state.remove(id);
    }

    fn deliver(&mut self, message: PlatformMessage, events: &mut Vec<WindowInputEvent>) {
        match message {
            PlatformMessage::Input(event) => {
                if self.windows.contains_key(&event.window_id) {
                    events.push(event);
                }
            }
            PlatformMessage::CloseRequested(None) => {
                if self.close_interceptor.as_mut().is_none_or(|f| f(None)) {
                    self.quitting = true;
                }
            }
            PlatformMessage::CloseRequested(Some(id)) => {
                if self.close_interceptor.as_mut().is_none_or(|f| f(Some(id))) {
                    self.destroy_window(id);
                }
            }
            PlatformMessage::WindowDestroyed(id) => self.forget_window(id),
            PlatformMessage::WindowFailed(id, reason) => {
                if self.order.first() == Some(&id) && self.startup_error.is_none() {
                    self.startup_error = Some(ShellError::InitializationFailed(reason));
                }
                self.forget_window(id);
            }
            PlatformMessage::DialogFinished(request, files) => {
                let mut pending = self.pending.borrow_mut();
                if let Some(done) = pending.dialogs.remove(&request) {
                    pending.finished.push(Box::new(move || done(files)));
                }
            }
            PlatformMessage::MenuFinished(request, chosen) => {
                let mut pending = self.pending.borrow_mut();
                if let Some(done) = pending.menus.remove(&request) {
                    pending.finished.push(Box::new(move || done(chosen)));
                }
            }
            PlatformMessage::ClipboardRead(request, text) => {
                let mut pending = self.pending.borrow_mut();
                if let Some(done) = pending.clipboard.remove(&request) {
                    pending.finished.push(Box::new(move || done(text)));
                }
            }
        }
    }

    fn dialog(
        &self,
        window: Option<WindowId>,
        folder: bool,
        path: Option<&str>,
        filters: &[FileDialogFilter],
        multiple: bool,
        done: DialogCompletion,
    ) {
        let request = {
            let mut pending = self.pending.borrow_mut();
            let request = pending.allocate();
            pending.dialogs.insert(request, done);
            request
        };
        self.post(PlatformCommand::OpenDialog(DialogRequest {
            request,
            window,
            folder,
            path: path.map(str::to_owned),
            filters: filters.to_vec(),
            multiple,
        }));
    }
}

impl Shell for ProxyShell {
    fn main_window_id(&self) -> Option<WindowId> {
        self.order.first().copied()
    }
    fn window_ids(&self) -> Vec<WindowId> {
        self.order.clone()
    }
    fn is_running(&self) -> bool {
        !self.quitting && !self.windows.is_empty()
    }
    fn set_close_interceptor(
        &mut self,
        interceptor: Option<Box<dyn FnMut(Option<WindowId>) -> bool>>,
    ) {
        self.close_interceptor = interceptor;
    }
    /// The identifier is allocated here, so opening a window never waits. The
    /// handle reports no surface until the platform thread has created it.
    fn create_window(
        &mut self,
        title: &str,
        options: WindowOptions,
    ) -> Result<Rc<dyn WindowHandle>, ShellError> {
        if options.width <= 0 || options.height <= 0 {
            return Err(ShellError::InitializationFailed(
                "invalid window dimensions".into(),
            ));
        }
        if title.contains('\0') {
            return Err(ShellError::InitializationFailed(
                "window title contains NUL".into(),
            ));
        }
        self.next_window += 1;
        let id = WindowId(self.next_window);
        let size = (options.width as u32, options.height as u32);
        self.link.state.publish(
            id,
            WindowSnapshot {
                drawable_size: size,
                logical_size: size,
                content_scale: 1.0,
                visible: true,
                focused: true,
                ..WindowSnapshot::default()
            },
        );
        let window = Rc::new(ProxyWindow {
            id,
            link: self.link.clone(),
        });
        self.windows.insert(id, window.clone());
        self.order.push(id);
        self.quitting = false;
        self.post(PlatformCommand::CreateWindow {
            window: id,
            title: title.to_owned(),
            options,
        });
        Ok(window)
    }
    fn window(&self, id: WindowId) -> Option<Rc<dyn WindowHandle>> {
        self.windows
            .get(&id)
            .cloned()
            .map(|window| window as Rc<dyn WindowHandle>)
    }
    fn destroy_window(&mut self, id: WindowId) {
        if self.windows.contains_key(&id) {
            self.post(PlatformCommand::CloseWindow(id));
            self.forget_window(id);
        }
    }
    fn poll_window_events(&mut self) -> Vec<WindowInputEvent> {
        self.link.beat();
        // Read before draining: anything posted later bumps the generation and
        // ends the next wait at once.
        self.seen_generation.set(self.link.generation());
        let mut events = Vec::new();
        for message in self.link.take_messages() {
            self.deliver(message, &mut events);
        }
        events
    }
    fn take_deferred_callbacks(&self) -> Vec<Box<dyn FnOnce()>> {
        std::mem::take(&mut self.pending.borrow_mut().finished)
    }
    fn wait_for_events(&self, timeout: Duration) -> bool {
        self.link.wait_ui(self.seen_generation.get(), timeout)
    }
    fn wake_event_loop(&self) {
        self.link.wake_ui();
    }
    fn waker(&self) -> EventLoopWaker {
        let link = self.link.clone();
        Arc::new(move || link.wake_ui())
    }
    fn set_text_input_area(&self, id: WindowId, area: Option<TextInputArea>) {
        self.post(PlatformCommand::SetTextInputArea(id, area));
    }
    fn set_cursor(&self, id: WindowId, cursor: SystemCursor) {
        self.post(PlatformCommand::SetCursor(id, cursor));
    }
    fn global_pointer_position(&self) -> Option<(f32, f32)> {
        self.link.state.global_pointer()
    }
    fn window_position(&self, id: WindowId) -> Option<(f32, f32)> {
        self.link.state.window(id).and_then(|s| s.position)
    }
    fn set_window_position(&self, id: WindowId, x: f32, y: f32) {
        self.post(PlatformCommand::SetPosition(id, x, y));
    }
    fn set_window_size(&self, id: WindowId, width: f32, height: f32) {
        self.post(PlatformCommand::SetSize(id, width, height));
    }
    fn raise_window(&self, id: WindowId) {
        self.post(PlatformCommand::Raise(id));
    }
    fn minimize_window(&self, id: WindowId) {
        self.post(PlatformCommand::Minimize(id));
    }
    fn maximize_window(&self, id: WindowId) {
        self.post(PlatformCommand::Maximize(id));
    }
    fn restore_window(&self, id: WindowId) {
        self.post(PlatformCommand::Restore(id));
    }
    fn hide_window(&self, id: WindowId) {
        self.post(PlatformCommand::Hide(id));
    }
    fn show_window(&self, id: WindowId) {
        self.post(PlatformCommand::Show(id));
    }
    fn is_window_maximized(&self, id: WindowId) -> bool {
        self.link.state.window(id).is_some_and(|s| s.maximized)
    }
    fn show_window_system_menu(&self, id: WindowId, x: f32, y: f32) {
        self.post(PlatformCommand::ShowSystemMenu(id, x, y));
    }
    fn set_window_chrome_hit_test(&self, id: WindowId, hit: Option<&WindowChromeHitTest>) {
        self.post(PlatformCommand::SetChromeHitTest(id, hit.cloned()));
    }
    fn show_open_folder_dialog(
        &self,
        id: Option<WindowId>,
        path: Option<&str>,
        accept: Box<dyn FnOnce(Option<String>)>,
    ) {
        self.dialog(
            id,
            true,
            path,
            &[],
            false,
            Box::new(move |files| accept(files.into_iter().next())),
        );
    }
    fn show_open_file_dialog(
        &self,
        id: Option<WindowId>,
        path: Option<&str>,
        filters: &[FileDialogFilter],
        multiple: bool,
        accept: DialogCompletion,
    ) {
        self.dialog(id, false, path, filters, multiple, accept);
    }
    fn display_refresh_rate(&self, id: Option<WindowId>) -> Option<f64> {
        id.or_else(|| self.main_window_id())
            .and_then(|id| self.link.state.window(id))
            .and_then(|s| s.refresh_rate)
    }
    /// Answers from the last published result. Before a window's first answer it
    /// reports success: a wrong yes waits one iteration for a tick, while a wrong
    /// no would report native refresh as missing on every platform that has it.
    fn set_display_refresh_enabled(&self, id: WindowId, enabled: bool) -> bool {
        if !self.windows.contains_key(&id) {
            return false;
        }
        let previous = self
            .refresh_requested
            .borrow_mut()
            .insert(id, enabled)
            .unwrap_or(false);
        if previous != enabled {
            self.link.state.discard_refresh_tick(id);
            self.post(PlatformCommand::SetDisplayRefresh(id, enabled));
        }
        enabled && self.link.state.refresh_active(id) != Some(false)
    }
    fn take_display_refresh(&self, id: WindowId) -> Option<DisplayRefreshTick> {
        self.link.state.take_refresh_tick(id)
    }
    fn shutdown(&mut self) {
        if std::mem::replace(&mut self.shut_down, true) {
            return;
        }
        self.windows.clear();
        self.order.clear();
        let mut pending = self.pending.borrow_mut();
        pending.dialogs.clear();
        pending.menus.clear();
        pending.clipboard.clear();
        pending.finished.clear();
        drop(pending);
        self.quitting = true;
        self.post(PlatformCommand::Shutdown);
    }
    fn take_startup_error(&mut self) -> Option<ShellError> {
        self.startup_error.take()
    }
    fn write_clipboard(&self, text: &str) {
        self.post(PlatformCommand::WriteClipboard(text.to_owned()));
    }
    fn read_clipboard(&self, done: ClipboardCompletion) {
        let request = {
            let mut pending = self.pending.borrow_mut();
            let request = pending.allocate();
            pending.clipboard.insert(request, done);
            request
        };
        self.post(PlatformCommand::ReadClipboard(request));
    }
    fn present_menu(
        &self,
        id: Option<WindowId>,
        snapshot: &NativeMenuSnapshot,
        x: f32,
        y: f32,
        done: MenuCompletion,
    ) {
        let request = {
            let mut pending = self.pending.borrow_mut();
            let request = pending.allocate();
            pending.menus.insert(request, done);
            request
        };
        self.post(PlatformCommand::ShowMenu {
            request,
            window: id,
            snapshot: snapshot.clone(),
            x,
            y,
        });
    }
    fn window_snapshot(&self, id: WindowId) -> Option<WindowSnapshot> {
        self.link.state.window(id)
    }
}

impl Drop for ProxyShell {
    fn drop(&mut self) {
        self.shutdown();
    }
}
