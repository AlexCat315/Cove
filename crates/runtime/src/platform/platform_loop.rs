//! The main-thread side: owns the real shell, applies posted commands, publishes
//! window state and forwards input. It never waits for the UI or render threads.
use super::accessibility::AccessibilityPresenters;
use super::{DialogRequest, PlatformCommand, PlatformLink, PlatformMessage};
use cove_platform::WindowId;
use cove_shell::{Shell, ShellError};
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// Bounds the delay of anything that arrives without a wake.
const MAX_WAIT: Duration = Duration::from_millis(250);
/// After the OS asked to quit, how long an unresponsive UI thread is tolerated.
const QUIT_GRACE: Duration = Duration::from_secs(5);
/// Layout and composition recurse with the view tree, so the UI thread gets the
/// stack headroom a main thread has.
const UI_STACK_BYTES: usize = 16 * 1024 * 1024;

/// A window's display refresh only needs another native call when one of these
/// changed: a hidden window refuses, and accepts once it is visible again.
#[derive(Clone, Copy, PartialEq)]
struct RefreshInputs {
    wanted: bool,
    visible: bool,
    minimized: bool,
    occluded: bool,
}

pub struct PlatformLoop {
    shell: Box<dyn Shell>,
    link: Arc<PlatformLink>,
    /// UI identifiers to native ones. The UI allocates its own so that opening a
    /// window does not wait for the platform.
    native_ids: HashMap<WindowId, WindowId>,
    ui_ids: HashMap<WindowId, WindowId>,
    close_requests: Rc<RefCell<Vec<Option<WindowId>>>>,
    refresh_wanted: HashMap<WindowId, bool>,
    /// What each window's display refresh was last applied for, and its answer.
    refresh_applied: HashMap<WindowId, (RefreshInputs, bool)>,
    accessibility: AccessibilityPresenters,
    quit_requested: Option<Instant>,
    last_heartbeat: (u64, Instant),
    abandoned: bool,
}

impl PlatformLoop {
    pub fn new(mut shell: Box<dyn Shell>, link: Arc<PlatformLink>) -> Self {
        let close_requests: Rc<RefCell<Vec<Option<WindowId>>>> = Rc::default();
        let requests = close_requests.clone();
        shell.set_close_interceptor(Some(Box::new(move |id| {
            requests.borrow_mut().push(id);
            false
        })));
        Self {
            shell,
            last_heartbeat: (link.heartbeat(), Instant::now()),
            link,
            native_ids: HashMap::new(),
            ui_ids: HashMap::new(),
            close_requests,
            refresh_wanted: HashMap::new(),
            refresh_applied: HashMap::new(),
            accessibility: AccessibilityPresenters::default(),
            quit_requested: None,
            abandoned: false,
        }
    }

    /// One iteration without blocking. Commands apply *after* the event poll: the
    /// poll consumes the coalesced native wake, so a command posted before this
    /// drain was seen and one posted after it issues a fresh wake.
    pub fn tick(&mut self) {
        let events = self.shell.poll_window_events();
        for work in self.shell.take_deferred_callbacks() {
            work();
        }
        for requested in std::mem::take(&mut *self.close_requests.borrow_mut()) {
            match requested {
                None => {
                    self.quit_requested.get_or_insert_with(Instant::now);
                    self.link.post(PlatformMessage::CloseRequested(None));
                }
                Some(native) => {
                    if let Some(id) = self.ui_ids.get(&native) {
                        self.link.post(PlatformMessage::CloseRequested(Some(*id)));
                    }
                }
            }
        }
        for mut event in events {
            if let Some(id) = self.ui_ids.get(&event.window_id) {
                event.window_id = *id;
                self.link.post(PlatformMessage::Input(event));
            }
        }
        for command in self.link.bridge.take() {
            self.apply(command);
        }
        self.sync_destroyed_windows();
        self.publish();
    }

    /// Whether the loop must stop: the UI thread ended or no longer responds to
    /// the OS asking the application to quit.
    fn should_stop(&mut self) -> bool {
        if self.link.is_finished() {
            return true;
        }
        let beat = self.link.heartbeat();
        if beat != self.last_heartbeat.0 {
            self.last_heartbeat = (beat, Instant::now());
        }
        if self.quit_requested.is_some() && self.last_heartbeat.1.elapsed() >= QUIT_GRACE {
            self.abandoned = true;
            return true;
        }
        false
    }

    /// Pumps until the UI thread ended. Returns false when it was abandoned, with
    /// the native windows left alone: its render threads may still present to them.
    pub fn run(&mut self) -> bool {
        while !self.should_stop() {
            crate::lifecycle::with_autorelease_pool(|| self.tick());
            if !self.link.is_finished() && !self.link.bridge.has_pending() {
                self.shell.wait_for_events(MAX_WAIT);
            }
        }
        if self.abandoned {
            return false;
        }
        crate::lifecycle::with_autorelease_pool(|| {
            for command in self.link.bridge.take() {
                self.apply(command);
            }
        });
        self.accessibility.clear();
        self.shell.shutdown();
        true
    }

    fn apply(&mut self, command: PlatformCommand) {
        let native = |ids: &HashMap<WindowId, WindowId>, id: WindowId| ids.get(&id).copied();
        match command {
            PlatformCommand::SetCursor(id, cursor) => {
                if let Some(id) = native(&self.native_ids, id) {
                    self.shell.set_cursor(id, cursor);
                }
            }
            PlatformCommand::SetTextInputArea(id, area) => {
                if let Some(id) = native(&self.native_ids, id) {
                    self.shell.set_text_input_area(id, area);
                }
            }
            PlatformCommand::SetChromeHitTest(id, hit) => {
                if let Some(id) = native(&self.native_ids, id) {
                    self.shell.set_window_chrome_hit_test(id, hit.as_ref());
                }
            }
            PlatformCommand::SetAppearance(id, appearance) => {
                if let Some(id) = native(&self.native_ids, id) {
                    self.shell.set_window_appearance(id, appearance);
                }
            }
            PlatformCommand::SetPosition(id, x, y) => {
                if let Some(id) = native(&self.native_ids, id) {
                    self.shell.set_window_position(id, x, y);
                }
            }
            PlatformCommand::SetSize(id, width, height) => {
                if let Some(id) = native(&self.native_ids, id) {
                    self.shell.set_window_size(id, width, height);
                }
            }
            PlatformCommand::SetDisplayRefresh(id, enabled) => {
                self.refresh_wanted.insert(id, enabled);
            }
            PlatformCommand::UpdateAccessibility(id, snapshot) => {
                let window = native(&self.native_ids, id)
                    .and_then(|native| self.shell.window_snapshot(native))
                    .and_then(|window| window.native_window);
                if let Some(window) = window {
                    self.accessibility.update(id, window, snapshot);
                }
            }
            PlatformCommand::CloseAccessibility(id, bridge) => {
                self.accessibility.close_bridge(id, bridge);
            }
            PlatformCommand::Raise(id) => {
                if let Some(id) = native(&self.native_ids, id) {
                    self.shell.raise_window(id);
                }
            }
            PlatformCommand::Minimize(id) => {
                if let Some(id) = native(&self.native_ids, id) {
                    self.shell.minimize_window(id);
                }
            }
            PlatformCommand::Maximize(id) => {
                if let Some(id) = native(&self.native_ids, id) {
                    self.shell.maximize_window(id);
                }
            }
            PlatformCommand::Restore(id) => {
                if let Some(id) = native(&self.native_ids, id) {
                    self.shell.restore_window(id);
                }
            }
            PlatformCommand::Hide(id) => {
                if let Some(id) = native(&self.native_ids, id) {
                    self.shell.hide_window(id);
                }
            }
            PlatformCommand::Show(id) => {
                if let Some(id) = native(&self.native_ids, id) {
                    self.shell.show_window(id);
                }
            }
            PlatformCommand::ShowSystemMenu(id, x, y) => {
                if let Some(id) = native(&self.native_ids, id) {
                    self.shell.show_window_system_menu(id, x, y);
                }
            }
            PlatformCommand::WriteClipboard(text) => self.shell.write_clipboard(&text),
            PlatformCommand::ReadClipboard(request) => {
                let link = self.link.clone();
                self.shell.read_clipboard(Box::new(move |text| {
                    link.post(PlatformMessage::ClipboardRead(request, text));
                }));
            }
            PlatformCommand::SampleColor(request) => {
                let link = self.link.clone();
                self.shell.sample_screen_color(Box::new(move |sample| {
                    link.post(PlatformMessage::ColorSampled(request, sample));
                }));
            }
            PlatformCommand::OpenDialog(dialog) => self.open_dialog(dialog),
            PlatformCommand::ShowMenu {
                request,
                window,
                snapshot,
                x,
                y,
            } => {
                let link = self.link.clone();
                let window = window.and_then(|id| native(&self.native_ids, id));
                // A native menu runs a nested event loop here. The UI thread keeps
                // running; only this loop pauses until the user chooses.
                self.shell.present_menu(
                    window,
                    &snapshot,
                    x,
                    y,
                    Box::new(move |chosen| {
                        link.post(PlatformMessage::MenuFinished(request, chosen));
                    }),
                );
            }
            PlatformCommand::CreateWindow {
                window,
                title,
                options,
            } => match self.shell.create_window(&title, options) {
                Ok(handle) => {
                    let native = handle.id();
                    self.native_ids.insert(window, native);
                    self.ui_ids.insert(native, window);
                }
                Err(error) => {
                    self.link
                        .post(PlatformMessage::WindowFailed(window, error.to_string()));
                }
            },
            PlatformCommand::CloseWindow(id) => {
                self.accessibility.remove(id);
                self.refresh_wanted.remove(&id);
                self.refresh_applied.remove(&id);
                if let Some(native) = self.native_ids.remove(&id) {
                    self.ui_ids.remove(&native);
                    self.shell.destroy_window(native);
                }
                self.link.state.remove(id);
            }
            PlatformCommand::Run(work) => work(),
            PlatformCommand::Shutdown => {
                self.accessibility.clear();
                for (_, native) in self.native_ids.drain() {
                    self.ui_ids.remove(&native);
                    self.shell.destroy_window(native);
                }
                self.refresh_wanted.clear();
                self.refresh_applied.clear();
            }
        }
    }

    fn open_dialog(&mut self, dialog: DialogRequest) {
        let DialogRequest {
            request,
            window,
            folder,
            path,
            filters,
            multiple,
        } = dialog;
        let window = window.and_then(|id| self.native_ids.get(&id).copied());
        let link = self.link.clone();
        if folder {
            self.shell.show_open_folder_dialog(
                window,
                path.as_deref(),
                Box::new(move |chosen| {
                    link.post(PlatformMessage::DialogFinished(
                        request,
                        chosen.into_iter().collect(),
                    ));
                }),
            );
        } else {
            self.shell.show_open_file_dialog(
                window,
                path.as_deref(),
                &filters,
                multiple,
                Box::new(move |files| {
                    link.post(PlatformMessage::DialogFinished(request, files));
                }),
            );
        }
    }

    /// Reports windows the OS destroyed without the UI asking.
    fn sync_destroyed_windows(&mut self) {
        let live = self.shell.window_ids();
        let gone = self
            .native_ids
            .iter()
            .filter(|(_, native)| !live.contains(native))
            .map(|(ui, native)| (*ui, *native))
            .collect::<Vec<_>>();
        for (ui, native) in gone {
            self.native_ids.remove(&ui);
            self.ui_ids.remove(&native);
            self.refresh_wanted.remove(&ui);
            self.refresh_applied.remove(&ui);
            self.accessibility.remove(ui);
            self.link.post(PlatformMessage::WindowDestroyed(ui));
        }
    }

    fn publish(&mut self) {
        let mut changed = false;
        for (&ui, &native) in &self.native_ids {
            let Some(snapshot) = self.shell.window_snapshot(native) else {
                continue;
            };
            changed |= self.link.state.publish(ui, snapshot);
            let wanted = self.refresh_wanted.get(&ui).copied().unwrap_or(false);
            let inputs = RefreshInputs {
                wanted,
                visible: snapshot.visible,
                minimized: snapshot.minimized,
                occluded: snapshot.occluded,
            };
            let active = match self.refresh_applied.get(&ui) {
                Some((applied, active)) if *applied == inputs => *active,
                _ => {
                    let active = self.shell.set_display_refresh_enabled(native, wanted);
                    self.refresh_applied.insert(ui, (inputs, active));
                    active
                }
            };
            let tick = active
                .then(|| self.shell.take_display_refresh(native))
                .flatten();
            if wanted {
                changed |= self.link.state.publish_refresh(ui, active, tick);
            }
        }
        self.link
            .state
            .publish_global_pointer(self.shell.global_pointer_position());
        if changed {
            self.link.wake_ui();
        }
    }
}

/// Pumps the platform on this thread while `ui` runs on a new UI thread, and
/// returns what `ui` returned. Must be called on the application's main thread.
///
/// A panic on the UI thread is resumed here.
pub fn run_with_ui_thread<T: Send + 'static>(
    shell: impl FnOnce() -> Result<Box<dyn Shell>, ShellError>,
    ui: impl FnOnce() -> T + Send + 'static,
) -> Result<T, ShellError> {
    let mut shell = shell()?;
    shell.prepare_event_loop()?;
    let link = Arc::new(PlatformLink::new(shell.waker()));
    let outcome: Arc<Mutex<Option<std::thread::Result<T>>>> = Arc::default();
    let thread = {
        let link = link.clone();
        let outcome = outcome.clone();
        let wake = shell.waker();
        std::thread::Builder::new()
            .name("cove-ui".into())
            .stack_size(UI_STACK_BYTES)
            .spawn(move || {
                PlatformLink::install_current(Some(link.clone()));
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(ui));
                PlatformLink::install_current(None);
                *outcome.lock().unwrap_or_else(|e| e.into_inner()) = Some(result);
                link.mark_finished();
                wake();
            })
            .map_err(|error| {
                ShellError::InitializationFailed(format!("the UI thread could not start: {error}"))
            })?
    };
    let mut platform = PlatformLoop::new(shell, link);
    if !platform.run() {
        // The thread is wedged. Ending `main` ends the process with it; until then
        // the shell must outlive whatever that thread is still touching.
        std::mem::forget(platform);
        return Err(ShellError::InitializationFailed(
            "the UI thread did not respond to the quit request".into(),
        ));
    }
    drop(platform);
    let _ = thread.join();
    match outcome.lock().unwrap_or_else(|e| e.into_inner()).take() {
        Some(Ok(value)) => Ok(value),
        Some(Err(panic)) => std::panic::resume_unwind(panic),
        None => Err(ShellError::InitializationFailed(
            "the UI thread ended without a result".into(),
        )),
    }
}
