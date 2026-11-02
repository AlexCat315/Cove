//! The UI-to-platform mailbox, the published window state and the proxy shell,
//! driven against a scripted shell.

use cove_platform::{InputEvent, SystemCursor, TextInputArea, WindowId, WindowInputEvent};
use cove_runtime::platform::{
    MainThreadBridge, PlatformCommand, PlatformLink, PlatformLoop, ProxyShell, run_with_ui_thread,
};
use cove_shell::{
    DialogCompletion, DisplayRefreshTick, EventLoopWaker, FileDialogFilter, MenuCompletion,
    NativeRenderSurface, NativeWindowReference, Shell, ShellError, WindowChromeHitTest,
    WindowHandle, WindowOptions, WindowSafeArea,
};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const NATIVE_ID: WindowId = WindowId(40);

#[derive(Clone, Copy)]
struct FakeState {
    size: (u32, u32),
    position: (f32, f32),
    scale: f32,
    focused: bool,
    minimized: bool,
    maximized: bool,
    refresh_rate: f64,
}

struct FakeWindow {
    id: WindowId,
    state: Rc<Cell<FakeState>>,
}

impl WindowHandle for FakeWindow {
    fn id(&self) -> WindowId {
        self.id
    }
    fn native_window_reference(&self) -> Option<NativeWindowReference> {
        None
    }
    fn render_surface(&self) -> Option<NativeRenderSurface> {
        Some(NativeRenderSurface::MetalLayer(7))
    }
    fn safe_area(&self) -> WindowSafeArea {
        WindowSafeArea {
            top: 4.0,
            ..WindowSafeArea::default()
        }
    }
    fn drawable_size(&self) -> (u32, u32) {
        let state = self.state.get();
        (
            (state.size.0 as f32 * state.scale) as u32,
            (state.size.1 as f32 * state.scale) as u32,
        )
    }
    fn logical_size(&self) -> (u32, u32) {
        self.state.get().size
    }
    fn content_scale_factor(&self) -> f32 {
        self.state.get().scale
    }
    fn is_focused(&self) -> bool {
        self.state.get().focused
    }
    fn is_visible(&self) -> bool {
        true
    }
    fn is_minimized(&self) -> bool {
        self.state.get().minimized
    }
    fn is_occluded(&self) -> bool {
        false
    }
}

#[derive(Default)]
struct Script {
    calls: Vec<String>,
    events: Vec<WindowInputEvent>,
    interceptor: Option<Box<dyn FnMut(Option<WindowId>) -> bool>>,
    close_requests: Vec<Option<WindowId>>,
    fail_create: bool,
    refresh_supported: bool,
    ticks: Vec<DisplayRefreshTick>,
    clipboard: Option<String>,
    menu_choice: Option<i64>,
    dialog_choice: Vec<String>,
    deferred: Vec<Box<dyn FnOnce()>>,
}

/// A shell whose windows and calls the test inspects.
#[derive(Clone)]
struct FakeShell {
    script: Rc<RefCell<Script>>,
    windows: Rc<RefCell<HashMap<WindowId, Rc<FakeWindow>>>>,
    state: Rc<Cell<FakeState>>,
    wakes: Arc<AtomicUsize>,
}

impl FakeShell {
    fn new() -> Self {
        Self {
            script: Rc::default(),
            windows: Rc::default(),
            state: Rc::new(Cell::new(FakeState {
                size: (800, 600),
                position: (10.0, 20.0),
                scale: 2.0,
                focused: true,
                minimized: false,
                maximized: false,
                refresh_rate: 120.0,
            })),
            wakes: Arc::default(),
        }
    }
    fn log(&self, call: impl Into<String>) {
        self.script.borrow_mut().calls.push(call.into());
    }
    fn calls(&self) -> Vec<String> {
        self.script
            .borrow()
            .calls
            .iter()
            .filter(|call| !call.starts_with("refresh"))
            .cloned()
            .collect()
    }
    fn refresh_calls(&self) -> usize {
        self.script
            .borrow()
            .calls
            .iter()
            .filter(|call| call.starts_with("refresh"))
            .count()
    }
}

impl Shell for FakeShell {
    fn main_window_id(&self) -> Option<WindowId> {
        self.windows.borrow().keys().next().copied()
    }
    fn window_ids(&self) -> Vec<WindowId> {
        self.windows.borrow().keys().copied().collect()
    }
    fn is_running(&self) -> bool {
        true
    }
    fn set_close_interceptor(
        &mut self,
        interceptor: Option<Box<dyn FnMut(Option<WindowId>) -> bool>>,
    ) {
        self.script.borrow_mut().interceptor = interceptor;
    }
    fn create_window(
        &mut self,
        title: &str,
        _: WindowOptions,
    ) -> Result<Rc<dyn WindowHandle>, ShellError> {
        self.log(format!("create {title}"));
        if self.script.borrow().fail_create {
            return Err(ShellError::InitializationFailed("no display".into()));
        }
        let window = Rc::new(FakeWindow {
            id: NATIVE_ID,
            state: self.state.clone(),
        });
        self.windows.borrow_mut().insert(NATIVE_ID, window.clone());
        Ok(window)
    }
    fn window(&self, id: WindowId) -> Option<Rc<dyn WindowHandle>> {
        self.windows
            .borrow()
            .get(&id)
            .cloned()
            .map(|window| window as Rc<dyn WindowHandle>)
    }
    fn destroy_window(&mut self, id: WindowId) {
        self.log(format!("destroy {}", id.0));
        self.windows.borrow_mut().remove(&id);
    }
    fn poll_window_events(&mut self) -> Vec<WindowInputEvent> {
        let requests = std::mem::take(&mut self.script.borrow_mut().close_requests);
        for request in requests {
            let mut interceptor = self.script.borrow_mut().interceptor.take();
            if let Some(interceptor) = &mut interceptor {
                interceptor(request);
            }
            self.script.borrow_mut().interceptor = interceptor;
        }
        std::mem::take(&mut self.script.borrow_mut().events)
    }
    fn take_deferred_callbacks(&self) -> Vec<Box<dyn FnOnce()>> {
        std::mem::take(&mut self.script.borrow_mut().deferred)
    }
    fn wait_for_events(&self, _: Duration) -> bool {
        std::thread::sleep(Duration::from_millis(1));
        false
    }
    fn wake_event_loop(&self) {}
    fn waker(&self) -> EventLoopWaker {
        let wakes = self.wakes.clone();
        Arc::new(move || {
            wakes.fetch_add(1, Ordering::SeqCst);
        })
    }
    fn set_text_input_area(&self, id: WindowId, area: Option<TextInputArea>) {
        self.log(format!("area {} {}", id.0, area.is_some()));
    }
    fn set_cursor(&self, id: WindowId, cursor: SystemCursor) {
        self.log(format!("cursor {} {cursor:?}", id.0));
    }
    fn global_pointer_position(&self) -> Option<(f32, f32)> {
        Some((3.0, 4.0))
    }
    fn window_position(&self, _: WindowId) -> Option<(f32, f32)> {
        Some(self.state.get().position)
    }
    fn set_window_position(&self, id: WindowId, x: f32, y: f32) {
        self.log(format!("position {} {x} {y}", id.0));
    }
    fn set_window_size(&self, id: WindowId, width: f32, height: f32) {
        self.log(format!("size {} {width} {height}", id.0));
    }
    fn raise_window(&self, id: WindowId) {
        self.log(format!("raise {}", id.0));
    }
    fn minimize_window(&self, id: WindowId) {
        self.log(format!("minimize {}", id.0));
    }
    fn maximize_window(&self, id: WindowId) {
        self.log(format!("maximize {}", id.0));
    }
    fn restore_window(&self, id: WindowId) {
        self.log(format!("restore {}", id.0));
    }
    fn hide_window(&self, id: WindowId) {
        self.log(format!("hide {}", id.0));
    }
    fn show_window(&self, id: WindowId) {
        self.log(format!("show {}", id.0));
    }
    fn is_window_maximized(&self, _: WindowId) -> bool {
        self.state.get().maximized
    }
    fn show_window_system_menu(&self, id: WindowId, _: f32, _: f32) {
        self.log(format!("system-menu {}", id.0));
    }
    fn set_window_chrome_hit_test(&self, id: WindowId, hit: Option<&WindowChromeHitTest>) {
        self.log(format!("chrome {} {}", id.0, hit.is_some()));
    }
    fn show_open_folder_dialog(
        &self,
        _: Option<WindowId>,
        _: Option<&str>,
        accept: Box<dyn FnOnce(Option<String>)>,
    ) {
        let chosen = self.script.borrow().dialog_choice.first().cloned();
        self.script
            .borrow_mut()
            .deferred
            .push(Box::new(move || accept(chosen)));
    }
    fn show_open_file_dialog(
        &self,
        _: Option<WindowId>,
        path: Option<&str>,
        filters: &[FileDialogFilter],
        multiple: bool,
        accept: DialogCompletion,
    ) {
        self.log(format!("file-dialog {path:?} {} {multiple}", filters.len()));
        let chosen = self.script.borrow().dialog_choice.clone();
        self.script
            .borrow_mut()
            .deferred
            .push(Box::new(move || accept(chosen)));
    }
    fn display_refresh_rate(&self, _: Option<WindowId>) -> Option<f64> {
        Some(self.state.get().refresh_rate)
    }
    fn set_display_refresh_enabled(&self, _: WindowId, enabled: bool) -> bool {
        self.log(format!("refresh {enabled}"));
        enabled && self.script.borrow().refresh_supported
    }
    fn take_display_refresh(&self, _: WindowId) -> Option<DisplayRefreshTick> {
        self.script.borrow_mut().ticks.pop()
    }
    fn shutdown(&mut self) {
        self.log("shutdown");
    }
    fn write_clipboard(&self, text: &str) {
        self.script.borrow_mut().clipboard = Some(text.to_owned());
    }
    fn read_clipboard(&self, done: cove_shell::ClipboardCompletion) {
        let text = self.script.borrow().clipboard.clone();
        done(text);
    }
    fn present_menu(
        &self,
        id: Option<WindowId>,
        _: &cove_platform::NativeMenuSnapshot,
        _: f32,
        _: f32,
        done: MenuCompletion,
    ) {
        self.log(format!("menu {:?}", id.map(|id| id.0)));
        done(self.script.borrow().menu_choice);
    }
}

fn pair() -> (FakeShell, PlatformLoop, ProxyShell, Arc<PlatformLink>) {
    let fake = FakeShell::new();
    let link = Arc::new(PlatformLink::new(fake.waker()));
    let platform = PlatformLoop::new(Box::new(fake.clone()), link.clone());
    let proxy = ProxyShell::new(link.clone());
    (fake, platform, proxy, link)
}

fn opened() -> (FakeShell, PlatformLoop, ProxyShell, WindowId) {
    let (fake, mut platform, mut proxy, _) = pair();
    let id = proxy
        .create_window("main", WindowOptions::default())
        .unwrap()
        .id();
    platform.tick();
    (fake, platform, proxy, id)
}

// MARK: - Bridge

fn no_wake() -> EventLoopWaker {
    Arc::new(|| {})
}

#[test]
fn a_burst_of_idempotent_commands_costs_one_platform_call() {
    let bridge = MainThreadBridge::new(no_wake());
    for x in 0..100 {
        bridge.post(PlatformCommand::SetPosition(WindowId(1), x as f32, 0.0));
    }
    let commands = bridge.take();
    assert_eq!(commands.len(), 1);
    assert!(matches!(
        commands[0],
        PlatformCommand::SetPosition(_, x, _) if x == 99.0
    ));
    assert!(!bridge.has_pending());
}

#[test]
fn slots_are_per_window_and_per_kind() {
    let bridge = MainThreadBridge::new(no_wake());
    bridge.post(PlatformCommand::SetCursor(
        WindowId(1),
        SystemCursor::Pointer,
    ));
    bridge.post(PlatformCommand::SetCursor(
        WindowId(2),
        SystemCursor::Pointer,
    ));
    bridge.post(PlatformCommand::SetSize(WindowId(1), 1.0, 1.0));
    assert_eq!(bridge.take().len(), 3);
}

#[test]
fn an_ordered_command_keeps_its_place_after_earlier_slots() {
    let bridge = MainThreadBridge::new(no_wake());
    bridge.post(PlatformCommand::SetSize(WindowId(1), 1.0, 1.0));
    bridge.post(PlatformCommand::Maximize(WindowId(1)));
    bridge.post(PlatformCommand::SetSize(WindowId(1), 2.0, 2.0));
    bridge.post(PlatformCommand::SetSize(WindowId(1), 3.0, 3.0));
    let commands = bridge.take();
    assert_eq!(commands.len(), 3);
    assert!(matches!(commands[0], PlatformCommand::SetSize(_, w, _) if w == 1.0));
    assert!(matches!(commands[1], PlatformCommand::Maximize(_)));
    assert!(matches!(commands[2], PlatformCommand::SetSize(_, w, _) if w == 3.0));
}

#[test]
fn only_the_post_that_finds_the_mailbox_empty_wakes_the_platform_thread() {
    let wakes = Arc::new(AtomicUsize::new(0));
    let counted = wakes.clone();
    let bridge = MainThreadBridge::new(Arc::new(move || {
        counted.fetch_add(1, Ordering::SeqCst);
    }));
    bridge.post(PlatformCommand::Raise(WindowId(1)));
    bridge.post(PlatformCommand::SetSize(WindowId(1), 1.0, 1.0));
    bridge.post(PlatformCommand::Raise(WindowId(1)));
    assert_eq!(wakes.load(Ordering::SeqCst), 1);
    assert_eq!(bridge.take().len(), 3);
    bridge.post(PlatformCommand::Raise(WindowId(1)));
    assert_eq!(
        wakes.load(Ordering::SeqCst),
        2,
        "drained, so a new post must wake"
    );
}

// MARK: - Published state

#[test]
fn the_published_snapshot_matches_the_shell_field_for_field() {
    let (fake, mut platform, proxy, id) = opened();
    let handle = proxy.window(id).unwrap();
    let native = Shell::window(&fake, NATIVE_ID).unwrap();
    let compare = |proxy: &ProxyShell| {
        assert_eq!(handle.drawable_size(), native.drawable_size());
        assert_eq!(handle.logical_size(), native.logical_size());
        assert_eq!(handle.content_scale_factor(), native.content_scale_factor());
        assert_eq!(handle.is_focused(), native.is_focused());
        assert_eq!(handle.is_visible(), native.is_visible());
        assert_eq!(handle.is_minimized(), native.is_minimized());
        assert_eq!(handle.is_occluded(), native.is_occluded());
        assert_eq!(handle.safe_area(), native.safe_area());
        assert_eq!(handle.render_surface(), native.render_surface());
        assert_eq!(proxy.window_position(id), fake.window_position(NATIVE_ID));
        assert_eq!(
            proxy.is_window_maximized(id),
            fake.is_window_maximized(NATIVE_ID)
        );
        assert_eq!(
            proxy.display_refresh_rate(Some(id)),
            fake.display_refresh_rate(Some(NATIVE_ID))
        );
        assert_eq!(
            proxy.global_pointer_position(),
            fake.global_pointer_position()
        );
    };
    compare(&proxy);
    fake.state.set(FakeState {
        size: (1024, 768),
        position: (50.0, 60.0),
        scale: 1.5,
        focused: false,
        minimized: true,
        maximized: true,
        refresh_rate: 60.0,
    });
    platform.tick();
    compare(&proxy);
}

#[test]
fn a_state_change_wakes_the_ui_and_an_unchanged_tick_does_not() {
    let (fake, mut platform, mut proxy, _) = opened();
    let _ = proxy.poll_window_events();
    assert!(!proxy.wait_for_events(Duration::from_millis(1)));
    platform.tick();
    assert!(!proxy.wait_for_events(Duration::from_millis(1)));
    fake.state.set(FakeState {
        focused: false,
        ..fake.state.get()
    });
    platform.tick();
    assert!(proxy.wait_for_events(Duration::from_millis(1)));
}

// MARK: - Proxy shell

#[test]
fn opening_a_window_never_waits_for_the_platform() {
    let (fake, mut platform, mut proxy, _) = pair();
    let handle = proxy
        .create_window("late", WindowOptions::default())
        .unwrap();
    assert!(fake.calls().is_empty(), "no platform call happened yet");
    assert_eq!(handle.render_surface(), None);
    assert!(proxy.is_running());
    platform.tick();
    assert_eq!(fake.calls(), ["create late"]);
    assert!(handle.render_surface().is_some());
}

#[test]
fn invalid_windows_fail_on_the_ui_side() {
    let (_, _, mut proxy, _) = pair();
    assert!(
        proxy
            .create_window(
                "bad",
                WindowOptions {
                    width: 0,
                    ..WindowOptions::default()
                }
            )
            .is_err()
    );
    assert!(
        proxy
            .create_window("bad\0title", WindowOptions::default())
            .is_err()
    );
}

#[test]
fn a_failed_creation_reports_after_the_handle_was_returned() {
    let (fake, mut platform, mut proxy, _) = pair();
    fake.script.borrow_mut().fail_create = true;
    let id = proxy
        .create_window("main", WindowOptions::default())
        .unwrap()
        .id();
    platform.tick();
    let _ = proxy.poll_window_events();
    assert!(proxy.window(id).is_none());
    assert!(proxy.take_startup_error().is_some());
    assert!(!proxy.is_running());
}

#[test]
fn commands_reach_the_shell_under_the_native_identifier() {
    let (fake, mut platform, proxy, id) = opened();
    proxy.set_cursor(id, SystemCursor::Ibeam);
    proxy.set_window_size(id, 300.0, 200.0);
    proxy.maximize_window(id);
    proxy.set_text_input_area(id, None);
    proxy.set_window_chrome_hit_test(id, Some(&WindowChromeHitTest::new(20.0)));
    proxy.show_window_system_menu(id, 1.0, 2.0);
    platform.tick();
    assert_eq!(
        fake.calls(),
        [
            "create main",
            "cursor 40 Ibeam",
            "size 40 300 200",
            "maximize 40",
            "area 40 false",
            "chrome 40 true",
            "system-menu 40",
        ]
    );
}

#[test]
fn input_arrives_under_the_ui_identifier_and_in_order() {
    let (fake, mut platform, mut proxy, id) = opened();
    for width in [1, 2, 3] {
        fake.script.borrow_mut().events.push(WindowInputEvent {
            window_id: NATIVE_ID,
            event: InputEvent::WindowResized { width, height: 1 },
        });
    }
    platform.tick();
    let events = proxy.poll_window_events();
    assert_eq!(events.len(), 3);
    assert!(events.iter().all(|event| event.window_id == id));
    assert!(matches!(
        events[2].event,
        InputEvent::WindowResized { width: 3, .. }
    ));
}

#[test]
fn a_close_request_runs_the_ui_interceptor_and_only_then_destroys() {
    let (fake, mut platform, mut proxy, id) = opened();
    let allow = Rc::new(Cell::new(false));
    let seen = allow.clone();
    proxy.set_close_interceptor(Some(Box::new(move |_| seen.get())));

    fake.script
        .borrow_mut()
        .close_requests
        .push(Some(NATIVE_ID));
    platform.tick();
    let _ = proxy.poll_window_events();
    platform.tick();
    assert!(proxy.window(id).is_some(), "the interceptor refused");
    assert!(!fake.calls().iter().any(|call| call.starts_with("destroy")));

    allow.set(true);
    fake.script
        .borrow_mut()
        .close_requests
        .push(Some(NATIVE_ID));
    platform.tick();
    let _ = proxy.poll_window_events();
    platform.tick();
    assert!(proxy.window(id).is_none());
    assert!(fake.calls().contains(&"destroy 40".to_owned()));
}

#[test]
fn a_window_the_os_destroyed_disappears_from_the_ui() {
    let (fake, mut platform, mut proxy, id) = opened();
    fake.windows.borrow_mut().remove(&NATIVE_ID);
    platform.tick();
    let _ = proxy.poll_window_events();
    assert!(proxy.window(id).is_none());
    assert!(!proxy.is_running());
}

#[test]
fn clipboard_reads_answer_later_and_see_earlier_writes() {
    let (_, mut platform, mut proxy, _) = opened();
    let text = Rc::new(RefCell::new(None));
    let seen = text.clone();
    proxy.write_clipboard("copied");
    proxy.read_clipboard(Box::new(move |value| *seen.borrow_mut() = value));
    assert_eq!(*text.borrow(), None, "nothing answered synchronously");
    platform.tick();
    let _ = proxy.poll_window_events();
    for work in proxy.take_deferred_callbacks() {
        work();
    }
    assert_eq!(text.borrow().as_deref(), Some("copied"));
}

#[test]
fn a_menu_choice_comes_back_as_a_callback_on_the_ui_side() {
    let (fake, mut platform, mut proxy, id) = opened();
    fake.script.borrow_mut().menu_choice = Some(9);
    let chosen = Rc::new(Cell::new(None));
    let seen = chosen.clone();
    proxy.present_menu(
        Some(id),
        &cove_platform::NativeMenuSnapshot::default(),
        0.0,
        0.0,
        Box::new(move |value| seen.set(value)),
    );
    assert_eq!(chosen.get(), None);
    platform.tick();
    let _ = proxy.poll_window_events();
    for work in proxy.take_deferred_callbacks() {
        work();
    }
    assert_eq!(chosen.get(), Some(9));
    assert!(fake.calls().contains(&"menu Some(40)".to_owned()));
}

#[test]
fn a_dialog_result_comes_back_as_a_callback_on_the_ui_side() {
    let (fake, mut platform, mut proxy, id) = opened();
    fake.script.borrow_mut().dialog_choice = vec!["a.txt".into()];
    let files = Rc::new(RefCell::new(Vec::new()));
    let seen = files.clone();
    let filter = FileDialogFilter {
        name: "Text".into(),
        extensions: vec!["txt".into()],
    };
    proxy.show_open_file_dialog(
        Some(id),
        Some("/tmp"),
        std::slice::from_ref(&filter),
        true,
        Box::new(move |chosen| *seen.borrow_mut() = chosen),
    );
    platform.tick();
    platform.tick();
    let _ = proxy.poll_window_events();
    for work in proxy.take_deferred_callbacks() {
        work();
    }
    assert_eq!(*files.borrow(), ["a.txt"]);
}

#[test]
fn display_refresh_starts_optimistic_then_follows_the_platform_answer() {
    let (fake, mut platform, proxy, id) = opened();
    assert!(
        proxy.set_display_refresh_enabled(id, true),
        "unanswered yet"
    );
    platform.tick();
    assert!(!proxy.set_display_refresh_enabled(id, true), "refused");

    // Becoming visible again is what prompts another native request.
    fake.script.borrow_mut().refresh_supported = true;
    fake.state.set(FakeState {
        minimized: true,
        ..fake.state.get()
    });
    platform.tick();
    assert!(proxy.set_display_refresh_enabled(id, true), "accepted");

    fake.script.borrow_mut().ticks.push(DisplayRefreshTick {
        timestamp: 1.0,
        target_timestamp: 2.0,
    });
    platform.tick();
    let tick = proxy
        .take_display_refresh(id)
        .expect("the tick was mirrored");
    assert_eq!(tick.target_timestamp, 2.0);
    assert!(
        proxy.take_display_refresh(id).is_none(),
        "one tick, one take"
    );

    assert!(!proxy.set_display_refresh_enabled(id, false));
}

#[test]
fn display_refresh_reaches_the_platform_only_when_its_inputs_change() {
    let (fake, mut platform, proxy, id) = opened();
    let baseline = fake.refresh_calls();
    for _ in 0..10 {
        platform.tick();
    }
    assert_eq!(
        fake.refresh_calls(),
        baseline,
        "idle ticks stay off the platform"
    );

    proxy.set_display_refresh_enabled(id, true);
    for _ in 0..10 {
        platform.tick();
    }
    assert_eq!(fake.refresh_calls(), baseline + 1);

    fake.state.set(FakeState {
        minimized: true,
        ..fake.state.get()
    });
    for _ in 0..10 {
        platform.tick();
    }
    assert_eq!(fake.refresh_calls(), baseline + 2);
}

#[test]
fn a_refused_refresh_stays_refused_when_requested_again() {
    let (_, mut platform, proxy, id) = opened();
    proxy.set_display_refresh_enabled(id, true);
    platform.tick();
    proxy.set_display_refresh_enabled(id, false);
    platform.tick();
    assert!(
        !proxy.set_display_refresh_enabled(id, true),
        "the earlier answer holds until the platform says otherwise"
    );
}

#[test]
fn a_wake_between_poll_and_wait_is_not_lost() {
    let (_, _, mut proxy, link) = pair();
    let _ = proxy.poll_window_events();
    link.wake_ui();
    let start = Instant::now();
    assert!(proxy.wait_for_events(Duration::from_secs(5)));
    assert!(start.elapsed() < Duration::from_secs(1));
}

#[test]
fn shutdown_reaches_the_platform_once_and_destroys_its_windows() {
    let (fake, mut platform, mut proxy, _) = opened();
    proxy.shutdown();
    proxy.shutdown();
    platform.tick();
    assert!(fake.windows.borrow().is_empty());
    assert_eq!(
        fake.calls()
            .iter()
            .filter(|call| call.starts_with("destroy"))
            .count(),
        1
    );
}

// MARK: - Threads

#[test]
fn a_blocked_ui_thread_never_blocks_the_platform_loop() {
    let (release, blocked) = std::sync::mpsc::channel::<()>();
    let ticks = Arc::new(AtomicUsize::new(0));
    let observed = ticks.clone();
    let shell = FakeShell::new();
    let wakes = shell.wakes.clone();
    let watcher = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        while wakes.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        std::thread::sleep(Duration::from_millis(50));
        let _ = release.send(());
    });
    let result = run_with_ui_thread(
        move || Ok(Box::new(shell) as Box<dyn Shell>),
        move || {
            let link = PlatformLink::current().expect("the UI thread has a link");
            link.bridge.post(PlatformCommand::Raise(WindowId(1)));
            // Stuck on something else entirely; the platform loop must keep going.
            blocked.recv().unwrap();
            observed.fetch_add(1, Ordering::SeqCst);
            42
        },
    );
    watcher.join().unwrap();
    assert_eq!(result.unwrap(), 42);
    assert_eq!(ticks.load(Ordering::SeqCst), 1);
}

#[test]
fn a_panic_on_the_ui_thread_surfaces_on_the_platform_thread() {
    let outcome = std::panic::catch_unwind(|| {
        run_with_ui_thread(
            || Ok(Box::new(FakeShell::new()) as Box<dyn Shell>),
            || -> () { panic!("ui failed") },
        )
    });
    assert!(outcome.is_err());
}

#[test]
fn a_shell_that_cannot_start_never_spawns_the_ui() {
    let ran = Arc::new(Mutex::new(false));
    let flag = ran.clone();
    let result = run_with_ui_thread(
        || Err(ShellError::InitializationFailed("no display".into())),
        move || *flag.lock().unwrap() = true,
    );
    assert!(result.is_err());
    assert!(!*ran.lock().unwrap());
}
