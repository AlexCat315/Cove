//! Translation of Desktop/Sources/CoveUI/Application/Runtime.swift.
//!
//! The high-level application host. It hides window, device, draw-list renderer,
//! `TextEnvironment` and `ViewGraph` assembly behind one entry point.
//!
//! The main window and auxiliary windows share UI resources but each owns its
//! presentation target.

use super::composition::{AppDisplayHandleHolder, AssetDropRegistryHolder, MainComposition};
use super::configuration::{AppFrameDrivePolicy, ApplicationConfiguration};
use super::memory::{AppMemorySnapshot, ApplicationMemoryPressure};
use super::services::ApplicationServices;
use super::typography::{AdaptiveApplicationContent, ApplicationTypography};
use crate::desktop::diagnostics::checkpoint::ApplicationCheckpoint;
use crate::desktop::diagnostics::frame_metrics::FrameMetrics;
use crate::desktop::diagnostics::integration::{
    DiagnosticsCallbacks, DiagnosticsSession, take_factory,
};
use crate::desktop::diagnostics::performance_hud::PerformanceHud;
use crate::desktop::diagnostics::recording::{InputRecorder, RecordingControl};
use crate::desktop::diagnostics::session::{ApplicationDiagnostics, DiagnosticsQueries};
use crate::desktop::display::controls::{MainWindowControls, PresentationControls, WindowControls};
use crate::desktop::display::{AppAuxiliaryWindowRequest, AppDisplayHandle, DisplayBindings};
use crate::desktop::rendering::ThreadedWindowRenderer;
use crate::desktop::rendering::context::AppRenderingContext;
use crate::desktop::window::auxiliary::AuxiliaryAppWindow;
use crate::desktop::window::chrome::{self, WindowChromeHitTestCollector};
use crate::desktop::window::native_renderer::AppWindowRenderSettings;
use crate::desktop::window::state::MainWindowState;
use cove_composition::AnyView;
use cove_core::scene::{
    ClipboardHolder, FocusChainHolder, InteractionRegistryHolder, PointerCaptureHolder,
    UIWorkSchedulerHolder,
};
use cove_core::{
    Environment, Font, FontWeight, InputCapabilities, Insets, Keyboard, State, set_content_scale,
};
use cove_graphics::rhi::Device;
use cove_imaging::ImageAssetRegistryHolder;
use cove_logging::{Level, LogEvent, LogSystem};
use cove_platform::WindowId;
use cove_runtime::{
    FrameOutcome, NativeAccessibility, PlatformFrameRateMode, PlatformHost, Sdl3PlatformHost,
};
use cove_shell::{NativeRenderSurface, WindowChromeHitTest, WindowOptions};
use cove_text::{TextBootstrapOptions, TextEnvironment, TextEnvironmentHolder};
use cove_ui_compose::primitives::interaction::{DroppedFile, FileDropRegistryHolder};
use cove_ui_compose::primitives::overlays::portal::portal_registry::{
    PortalStore, PortalStoreAmbient,
};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::time::Instant;

/// Adapts `PortalStoreAmbient` to the runtime's scoped-ambient hook.
///
/// `PortalStoreAmbient` already exposes `activate`; the runtime trait needs that
/// same closure boxed as `FnOnce`, so this host-side adapter supplies it without
/// editing Compose.
struct PortalStoreScope {
    store: Rc<PortalStore>,
}

impl PortalStoreScope {
    fn new(store: Rc<PortalStore>) -> Self {
        Self { store }
    }
}

impl cove_core::scene::ScopedAmbient for PortalStoreScope {
    fn activate(&self) -> Box<dyn FnOnce()> {
        let restore = PortalStoreAmbient::new(self.store.clone()).activate();
        Box::new(move || restore())
    }
}

/// Callbacks the caller supplies to a run.
#[derive(Clone, Default)]
pub struct ApplicationCallbacks {
    /// Subscribers observe every platform event the host pumps, plus the
    /// window exposure transitions this application emits itself.
    pub events: cove_runtime::PlatformEventBridge,
    pub tick: Option<Rc<dyn Fn(f64)>>,
    pub ready: Option<Rc<dyn Fn(&AppDisplayHandle)>>,
    pub diagnostics_ready: Option<Rc<dyn Fn(bool)>>,
}

fn environment_flag(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

fn previews_phone() -> bool {
    environment_flag("COVE_PREVIEW_DEVICE").is_some_and(|value| value.eq_ignore_ascii_case("phone"))
}

fn force_vsync_off() -> bool {
    environment_flag("COVE_VSYNC").is_some_and(|value| {
        matches!(
            value.to_ascii_lowercase().as_str(),
            "off" | "0" | "false" | "no"
        )
    })
}

fn fps_log_enabled() -> bool {
    environment_flag("COVE_FPS_LOG").as_deref() == Some("1")
}

fn force_continuous_frame_drive() -> bool {
    environment_flag("COVEUI_FORCE_CONTINUOUS_FRAMES").as_deref() == Some("1")
}

/// Mutable host state. Held behind an `Rc` so the shell callbacks can reach it.
struct ApplicationState {
    config: ApplicationConfiguration,
    callbacks: ApplicationCallbacks,
    checkpoint: Option<ApplicationCheckpoint>,
    recorder: Rc<RefCell<InputRecorder>>,
    composition: Rc<MainComposition>,
    host: Sdl3PlatformHost,
    rendering: Rc<AppRenderingContext>,
    window_renderer: RefCell<Option<ThreadedWindowRenderer>>,
    /// Render-thread cost of the latest finished frame.
    last_render_ms: Cell<f64>,
    /// Frames refused for lack of an in-flight slot, reported with the next
    /// `[cove.fps]` line so a stall that produces no frames is still visible.
    frame_drops: Cell<u32>,
    main_window: RefCell<MainWindowState>,
    typography: RefCell<ApplicationTypography>,
    auxiliary_windows: RefCell<HashMap<WindowId, AuxiliaryAppWindow>>,
    metrics: RefCell<FrameMetrics>,
    performance_hud: RefCell<PerformanceHud>,
    run_loop_error: RefCell<Option<String>>,
    diagnostics: RefCell<ApplicationDiagnostics>,
    memory_pressure: RefCell<Option<ApplicationMemoryPressure>>,
    display_handle: AppDisplayHandle,
    environment: State<Environment>,
    recording_control: RefCell<RecordingControl>,
    root_view: RefCell<Option<AnyView>>,
    chrome_collector: RefCell<WindowChromeHitTestCollector>,
    last_chrome_hit_test: RefCell<Option<WindowChromeHitTest>>,
}

/// The high-level application host.
#[derive(Clone)]
pub struct Application {
    state: Rc<ApplicationState>,
}

impl Application {
    /// Starts the main window and blocks until it closes.
    ///
    /// `on_tick` runs before each frame's layout so an external subsystem (a
    /// game engine host, for example) can advance at the UI frame rate.
    /// `delta_time` is the seconds between the previous frame and this one.
    #[allow(
        clippy::too_many_arguments,
        reason = "translated run entry point carries the same optional host hooks as the Swift declaration"
    )]
    pub fn run(
        config: ApplicationConfiguration,
        backend: Option<Arc<Device>>,
        root_view: AnyView,
        callbacks: ApplicationCallbacks,
        checkpoint: Option<ApplicationCheckpoint>,
    ) -> Result<(), String> {
        let mut logging = config.logging.clone();
        logging
            .apply_process_environment()
            .map_err(|error| format!("logging configuration failed: {error}"))?;
        let application_name = std::env::args()
            .next()
            .unwrap_or_else(|| "CoveUI".to_owned());
        let _ = LogSystem::bootstrap(Some(&application_name), logging, None);
        LogSystem::logger("com.cove.ui.application").log(&LogEvent::new(
            Level::Info,
            format!("Application starting title={}", config.window.title),
        ));
        // Remote image loading needs a platform transport; without this call the
        // holder stays empty and every http(s) image reports HostUnavailable.
        let capabilities = crate::application_services::install_platform_services();
        LogSystem::logger("com.cove.ui.application").log(&LogEvent::new(
            Level::Info,
            format!(
                "Platform services installed http={} https={} file={}",
                capabilities.http, capabilities.https, capabilities.file
            ),
        ));

        let application = Application::new(config, backend, callbacks, checkpoint)?;
        let result = application.start(root_view);
        LogSystem::flush();
        result
    }

    fn new(
        config: ApplicationConfiguration,
        backend: Option<Arc<Device>>,
        callbacks: ApplicationCallbacks,
        checkpoint: Option<ApplicationCheckpoint>,
    ) -> Result<Self, String> {
        let rendering =
            Rc::new(AppRenderingContext::new(backend, &config).map_err(|error| error.to_string())?);
        let options = WindowOptions {
            width: if previews_phone() {
                390
            } else {
                config.window.width.max(1)
            },
            height: if previews_phone() {
                844
            } else {
                config.window.height.max(1)
            },
            title_bar_style: config.window.title_bar_style.platform_style(),
            shows_dock_icon: config.window.shows_dock_icon,
            backdrop: config.window.backdrop,
        };
        let host = Sdl3PlatformHost::new(config.window.title.clone(), options);
        let portal_store = Rc::new(PortalStore::new());
        let composition = Rc::new(MainComposition::new(host.recomposer(), portal_store));
        composition.apply_image_limits(config.memory.images);
        let composition = composition;
        composition.draw_list.borrow_mut().geometry_antialiasing = config
            .window
            .rendering
            .geometry_antialiasing
            .unwrap_or(config.rendering.geometry_antialiasing);

        let is_vsync = config.rendering.vsync && !force_vsync_off();
        let environment = State::new(Environment::default());
        let display_handle = AppDisplayHandle::new({
            let host = host.clone();
            move || host.wake_event_loop()
        });

        let state = Rc::new(ApplicationState {
            config,
            callbacks,
            checkpoint,
            recorder: Rc::new(RefCell::new(InputRecorder::default())),
            composition,
            host,
            rendering,
            window_renderer: RefCell::new(None),
            last_render_ms: Cell::new(0.0),
            frame_drops: Cell::new(0),
            main_window: RefCell::new(MainWindowState::new(is_vsync)),
            typography: RefCell::new(ApplicationTypography::default()),
            auxiliary_windows: RefCell::new(HashMap::new()),
            metrics: RefCell::new(FrameMetrics::default()),
            performance_hud: RefCell::new(PerformanceHud::default()),
            run_loop_error: RefCell::new(None),
            diagnostics: RefCell::new(ApplicationDiagnostics::default()),
            memory_pressure: RefCell::new(None),
            display_handle,
            environment,
            recording_control: RefCell::new(RecordingControl::default()),
            root_view: RefCell::new(None),
            chrome_collector: RefCell::new(WindowChromeHitTestCollector::new()),
            last_chrome_hit_test: RefCell::new(None),
        });
        let application = Self { state };
        application.apply_frame_rate_policy();
        Ok(application)
    }

    pub fn host(&self) -> &Sdl3PlatformHost {
        &self.state.host
    }

    pub fn composition(&self) -> &Rc<MainComposition> {
        &self.state.composition
    }

    pub fn display_handle(&self) -> &AppDisplayHandle {
        &self.state.display_handle
    }

    /// Applies the configured frame-drive policy. Also restores the policy after
    /// a mirror session forces continuous presentation.
    pub fn apply_frame_rate_policy(&self) {
        let host = &self.state.host;
        if let Some(target) = self.state.config.frames.validated_target_rate() {
            host.set_target_frame_rate(Some(target));
            return;
        }
        if force_continuous_frame_drive() {
            host.set_frame_rate_mode(PlatformFrameRateMode::DisplayRefresh);
            return;
        }
        match self.state.config.frames.policy {
            AppFrameDrivePolicy::Continuous => {
                host.set_frame_rate_mode(PlatformFrameRateMode::DisplayRefresh)
            }
            AppFrameDrivePolicy::EventDriven => {
                host.set_frame_rate_mode(PlatformFrameRateMode::EventDriven)
            }
        }
    }

    fn start(&self, root_view: AnyView) -> Result<(), String> {
        let state = self.state.clone();
        *state.root_view.borrow_mut() = Some(root_view);
        let services = ApplicationServices::capture();

        let composition = state.composition.clone();
        let host = state.host.clone();

        ImageAssetRegistryHolder::set_current(Some(composition.image_assets.clone()));
        AssetDropRegistryHolder::set_current(Some(composition.asset_drop_registry.clone()));
        // Both registries need their own handle: `set_enqueue` and the restore
        // registry outlive this scope independently.
        fn enqueue_on(host: &Sdl3PlatformHost, work: Rc<dyn Fn()>) {
            let host = host.clone();
            host.enqueue_main_thread_work(move || work());
        }
        let work_host = host.clone();
        UIWorkSchedulerHolder::set_enqueue(Some(Rc::new(move |work: Rc<dyn Fn()>| {
            enqueue_on(&work_host, work);
        })));
        let remember_host = host.clone();
        super::services::work_scheduler::remember(Some(Rc::new(move |work: Rc<dyn Fn()>| {
            enqueue_on(&remember_host, work);
        })));
        let input = host.input_context();
        InteractionRegistryHolder::set_current(Some(input.interactions.clone()));
        FocusChainHolder::set_current(Some(input.focus_chain.clone()));
        PointerCaptureHolder::set_current(Some(input.pointer_capture.clone()));
        let read: cove_core::scene::ClipboardRequester = Rc::new({
            let host = host.clone();
            move |done| host.read_clipboard(done)
        });
        ClipboardHolder::set_read_requester(Some(read.clone()));
        let write: Rc<dyn Fn(&str)> = Rc::new({
            let host = host.clone();
            move |text: &str| host.write_clipboard(text)
        });
        ClipboardHolder::set_write(Some(write.clone()));
        super::services::clipboard_handlers::remember(Some(read), Some(write));
        // Only install the ambient when the host has a native backend: that is
        // what makes `ColorField` show or hide the eyedropper affordance.
        let sample: Option<cove_core::scene::ColorSamplerCallback> =
            if host.screen_color_backend().is_available() {
                Some(Rc::new({
                    let host = host.clone();
                    move |done: cove_core::scene::ColorSampleCompletion| {
                        host.sample_screen_color(Box::new(move |sample| {
                            done(sample.color().map(|color| cove_core::Color {
                                r: color.r,
                                g: color.g,
                                b: color.b,
                                a: color.a,
                            }));
                        }));
                    }
                }))
            } else {
                None
            };
        cove_core::scene::ColorSampler::set_requester(sample.clone());
        super::services::color_sampler_handler::remember(sample);
        AppDisplayHandleHolder::set_current(Some(state.display_handle.clone()));
        cove_ui_compose::core::WindowPresentation::set_current(Some(Rc::new(
            HostWindowPresenter {
                state: Rc::downgrade(&state),
            },
        )));
        // The open panel is what makes a drop zone usable without a pointer
        // drag: touch and keyboard users reach the same callback through it.
        let picker: cove_core::scene::FilePickerCallback = Rc::new({
            let display = state.display_handle.clone();
            move |filters: &[cove_core::scene::FilePickerFilter],
                  multiple: bool,
                  done: cove_core::scene::FilePickCompletion| {
                let filters: Vec<cove_shell::FileDialogFilter> = filters
                    .iter()
                    .map(|filter| cove_shell::FileDialogFilter {
                        name: filter.name.clone(),
                        extensions: filter
                            .extensions
                            .iter()
                            .map(|extension| extension.trim_start_matches('.').to_owned())
                            .collect(),
                    })
                    .collect();
                display.request_open_file(&filters, multiple, None, done);
            }
        });
        cove_core::scene::FilePicker::set_presenter(Some(picker.clone()));
        super::services::file_picker_handler::remember(Some(picker));
        if let Some(link) = cove_runtime::platform::PlatformLink::current() {
            cove_core::scene::PlatformThread::set_executor(Some(Rc::new(move |work| {
                link.bridge
                    .post(cove_runtime::platform::PlatformCommand::Run(work));
            })));
        }

        self.install_display_bindings();
        if let Some(ready) = state.callbacks.ready.clone() {
            ready(&state.display_handle);
        }
        if let Some(ready) = state.callbacks.diagnostics_ready.clone() {
            let capturing = state
                .diagnostics
                .borrow()
                .queries
                .as_ref()
                .is_some_and(|q| q.is_capturing());
            ready(capturing);
        }

        self.wire_host_callbacks();
        self.start_diagnostics();
        state
            .recording_control
            .borrow_mut()
            .install(state.checkpoint.as_ref(), state.recorder.clone());

        *state.memory_pressure.borrow_mut() = Some(ApplicationMemoryPressure::new());

        let run_result = host.run(composition.tree.clone());
        // UIKit owns the main loop: `run` returned with the display link driving
        // frames. The state stays alive through the host callbacks, and the host's
        // teardown callback releases the GPU when the application ends.
        if cfg!(target_os = "ios") {
            return run_result.map_err(|error| error.to_string());
        }
        if let Some(mut pressure) = state.memory_pressure.borrow_mut().take() {
            pressure.stop();
        }
        self.teardown();
        cove_core::scene::PlatformThread::set_executor(None);
        services.restore();
        run_result.map_err(|error| error.to_string())?;
        match state.run_loop_error.borrow_mut().take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    /// Builds the diagnostics session when the configuration requested DevTools
    /// and the build installed a backend factory.
    fn start_diagnostics(&self) {
        let state = &self.state;
        let Some(configuration) = state.config.diagnostics.dev_tools.clone() else {
            return;
        };
        let Some(factory) = take_factory() else {
            LogSystem::logger("com.cove.ui.diagnostics").log(&LogEvent::new(
                Level::Warning,
                "DevTools requested but no diagnostics backend is installed",
            ));
            return;
        };
        let host = state.host.clone();
        let schedule: Rc<dyn Fn()> = Rc::new(move || host.request_display(None));
        let callbacks = DiagnosticsCallbacks {
            invalidate: schedule.clone(),
            memory: {
                let application = Application {
                    state: state.clone(),
                };
                Rc::new(move || application.diagnostic_memory_snapshot())
            },
            invalidation_log: Some(state.host.input_context().invalidation_log.clone()),
            frame_source: match state.rendering.frame_source() {
                Ok(source) => Some(source),
                Err(error) => {
                    LogSystem::logger("com.cove.ui.diagnostics").log(&LogEvent::new(
                        Level::Warning,
                        format!("DevTools mirror is unavailable: {error}"),
                    ));
                    None
                }
            },
            mirror_active: {
                let application = Application {
                    state: state.clone(),
                };
                Rc::new(move |active| application.set_mirror_active(active))
            },
            resize_window: {
                let host = state.host.clone();
                Some(Rc::new(move |width, height| {
                    let Some(window) = host.main_window() else {
                        return Some("No main window".to_owned());
                    };
                    // A maximized native window can ignore explicit size requests.
                    host.restore_window(window);
                    host.set_window_size(window, width, height);
                    host.request_display(None);
                    None
                }))
            },
        };
        match factory(
            configuration,
            state.composition.tree.clone(),
            state.composition.graph.clone(),
            schedule,
            callbacks,
        ) {
            Ok(mut session) => {
                let injection_host = state.host.clone();
                session.attach_input(Rc::new(move |event| {
                    if let Some(session) = injection_host.main_session() {
                        session.inject_event(event);
                    }
                }));
                let mut diagnostics = state.diagnostics.borrow_mut();
                diagnostics.queries = Some(DiagnosticsQueries::default());
                diagnostics.dev_tools = Some(session);
            }
            Err(error) => LogSystem::logger("com.cove.ui.diagnostics").log(&LogEvent::new(
                Level::Error,
                format!("DevTools failed to start: {error}"),
            )),
        }
    }

    /// Keeps presenting for the mirror, even while the window is occluded. The
    /// swapchain is still released on occlusion, so a background mirror does not
    /// keep presentation memory resident.
    fn set_mirror_active(&self, active: bool) {
        let host = &self.state.host;
        if let Some(window) = host.main_window() {
            host.set_window_keep_rendering(window, active);
        }
        if active {
            host.set_frame_rate_mode(PlatformFrameRateMode::DisplayRefresh);
        } else {
            self.apply_frame_rate_policy();
        }
        host.request_display(None);
    }

    /// Runs `body` against the live session without holding the `RefCell` borrow
    /// across it: protocol dispatch is synchronous and may re-enter the runtime.
    fn with_diagnostics(&self, body: impl FnOnce(&mut dyn DiagnosticsSession)) {
        let mut session = self.state.diagnostics.borrow_mut().dev_tools.take();
        if let Some(session) = session.as_mut() {
            body(session.as_mut());
        }
        self.state.diagnostics.borrow_mut().dev_tools = session;
    }

    /// Delivers a platform drag to the file drop zones of the current window.
    ///
    /// A drag is a batch, not one event: the platform reports each dropped path
    /// as its own step between `Begin` and `Complete`. The registry accumulates
    /// them and delivers once, so a zone's `maximum_files` sees the whole drag
    /// instead of one file at a time.
    fn route_external_drop(state: &Rc<ApplicationState>, event: &cove_platform::InputEvent) {
        let cove_platform::InputEvent::ExternalDrop(drop) = event else {
            return;
        };
        let Some(registry) = FileDropRegistryHolder::current() else {
            return;
        };
        let point = cove_core::Point::new(drop.x, drop.y);
        match drop.phase {
            cove_platform::DropPhase::Begin => {
                registry.begin(point);
            }
            cove_platform::DropPhase::Position => {
                registry.update_position(point);
            }
            cove_platform::DropPhase::File => {
                // Inspected, not merely parsed: a folder and a file of the same
                // name take different paths, and only the filesystem knows which
                // this is.
                let files: Vec<DroppedFile> =
                    drop.paths.iter().map(DroppedFile::inspected).collect();
                registry.stage(files, point);
            }
            cove_platform::DropPhase::Text => {}
            cove_platform::DropPhase::Complete => {
                // A platform that sends `File` with no `Begin` still has a
                // batch to deliver: `stage` opened it implicitly.
                registry.finish(point);
                registry
                    .update_position(cove_core::Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY));
            }
        }
        let _ = state;
    }

    /// Lets the session consume a platform event before the app dispatcher sees
    /// it, which is what makes DevTools picking work.
    fn install_input_interceptor(&self) {
        let Some(session) = self.state.host.main_session() else {
            return;
        };
        let state = self.state.clone();
        *session.callbacks.input_interceptor.borrow_mut() = Some(Rc::new(move |event| {
            // A platform drag is delivered as an input event but has no
            // dispatcher route of its own, so the host consumes it here: this
            // runs for every event, including one injected by DevTools, without
            // depending on the window's `on_event` slot.
            if matches!(event, cove_platform::InputEvent::ExternalDrop(_)) {
                Application::route_external_drop(&state, event);
            }
            let mut session = state.diagnostics.borrow_mut().dev_tools.take();
            let consumed = session
                .as_mut()
                .is_some_and(|session| session.intercept(event));
            state.diagnostics.borrow_mut().dev_tools = session;
            consumed
        }));
    }

    fn install_display_bindings(&self) {
        let state = self.state.clone();
        let host = state.host.clone();
        let display_handle = state.display_handle.clone();

        let presentation = PresentationControls {
            set_runtime_target_frame_rate: Some(Rc::new({
                let host = host.clone();
                move |rate: Option<f64>| host.set_target_frame_rate(rate)
            })),
            set_runtime_frame_rate_mode: Some(Rc::new({
                let host = host.clone();
                move |mode: PlatformFrameRateMode| host.set_frame_rate_mode(mode)
            })),
            set_runtime_vsync_enabled: Some(Rc::new({
                let application = Application {
                    state: state.clone(),
                };
                move |enabled: bool| application.set_vsync_enabled(enabled)
            })),
            current_runtime_display_refresh_rate: Some(Rc::new({
                let host = host.clone();
                move || host.current_display_refresh_rate(None)
            })),
        };

        let main_window = MainWindowControls {
            minimize_main_window_action: Some(Rc::new({
                let host = host.clone();
                move || {
                    if let Some(id) = host.main_window() {
                        host.minimize_window(id);
                    }
                }
            })),
            maximize_main_window_action: Some(Rc::new({
                let host = host.clone();
                move || {
                    if let Some(id) = host.main_window() {
                        host.maximize_window(id);
                    }
                }
            })),
            restore_main_window_action: Some(Rc::new({
                let host = host.clone();
                move || {
                    if let Some(id) = host.main_window() {
                        host.restore_window(id);
                    }
                }
            })),
            hide_main_window_action: Some(Rc::new({
                let host = host.clone();
                move || {
                    if let Some(id) = host.main_window() {
                        host.hide_window(id);
                    }
                }
            })),
            show_main_window_action: Some(Rc::new({
                let host = host.clone();
                move || {
                    if let Some(id) = host.main_window() {
                        host.show_window(id);
                    }
                }
            })),
            close_main_window_action: Some(Rc::new({
                let host = host.clone();
                move || {
                    if let Some(id) = host.main_window() {
                        host.close_window(id);
                    }
                }
            })),
            main_window_maximized_query: Some(Rc::new({
                let host = host.clone();
                move || {
                    host.main_window()
                        .is_some_and(|id| host.is_window_maximized(id))
                }
            })),
            set_main_window_chrome_hit_test_action: Some(Rc::new({
                let host = host.clone();
                move |hit_test: Option<&WindowChromeHitTest>| {
                    if let Some(id) = host.main_window() {
                        host.set_window_chrome_hit_test(id, hit_test);
                    }
                }
            })),
        };

        let windows = WindowControls {
            minimize_window_by_id_action: Some(Rc::new({
                let host = host.clone();
                move |id: WindowId| host.minimize_window(id)
            })),
            maximize_window_by_id_action: Some(Rc::new({
                let host = host.clone();
                move |id: WindowId| host.maximize_window(id)
            })),
            restore_window_by_id_action: Some(Rc::new({
                let host = host.clone();
                move |id: WindowId| host.restore_window(id)
            })),
            close_window_by_id_action: Some(Rc::new({
                let host = host.clone();
                let state = state.clone();
                move |id: WindowId| {
                    host.close_window(id);
                    state.auxiliary_windows.borrow_mut().remove(&id);
                }
            })),
            window_maximized_by_id_query: Some(Rc::new({
                let host = host.clone();
                move |id: WindowId| host.is_window_maximized(id)
            })),
            show_window_system_menu_action: Some(Rc::new({
                let host = host.clone();
                move |id: WindowId, x: f32, y: f32| host.show_window_system_menu(id, x, y)
            })),
        };

        let bindings = DisplayBindings {
            presentation,
            main_window,
            windows,
            open_auxiliary_window: Some(Rc::new({
                let application = Application {
                    state: state.clone(),
                };
                move |request| application.open_auxiliary_window(request)
            })),
            close_auxiliary_window: Some(Rc::new({
                let application = Application {
                    state: state.clone(),
                };
                move |id| application.close_auxiliary_window(id)
            })),
            auxiliary_window_is_open: Some(Rc::new({
                let application = Application {
                    state: state.clone(),
                };
                move |id| application.is_auxiliary_window_open(id)
            })),
            install_native_menu_bar: Some(Rc::new({
                let host = host.clone();
                move |menu_bar| {
                    let host = host.clone();
                    crate::desktop::window::native_menu::install(
                        menu_bar,
                        Rc::new(move |menu, point, done| {
                            host.present_menu(None, menu, point.x, point.y, done);
                        }),
                    );
                }
            })),
            set_close_interceptor: Some(Rc::new({
                let host = host.clone();
                move |handler: Option<Rc<dyn Fn(Option<WindowId>) -> bool>>| {
                    // The close interceptor is a run-loop callback slot rather than
                    // a shell method: the host consults it before destroying a
                    // window the OS asked to close.
                    let callbacks = host.callbacks();
                    *callbacks.window_close_interceptor.borrow_mut() = handler;
                }
            })),
            quit: Some(Rc::new({
                let host = host.clone();
                move || host.stop()
            })),
            main_window_id_query: Some(Rc::new({
                let host = host.clone();
                move || host.main_window()
            })),
            open_folder_dialog: Some(Rc::new({
                let host = host.clone();
                move |path: Option<&str>, accept: Box<dyn FnOnce(Option<String>)>| {
                    let window = host.main_window();
                    host.show_open_folder_dialog(window, path, Box::new(accept));
                }
            })),
            open_file_dialog: Some(Rc::new({
                let host = host.clone();
                move |path: Option<&str>,
                      filters: &[cove_shell::FileDialogFilter],
                      multiple: bool,
                      accept: Box<dyn FnOnce(Vec<String>)>| {
                    let window = host.main_window();
                    host.show_open_file_dialog(window, path, filters, multiple, Box::new(accept));
                }
            })),
            memory_query: Some(Rc::new({
                let application = Application {
                    state: state.clone(),
                };
                move || Some(application.diagnostic_memory_snapshot().counters)
            })),
            trim_memory: Some(Rc::new({
                let application = Application {
                    state: state.clone(),
                };
                move || application.trim_memory()
            })),
        };
        display_handle.install(bindings);
    }

    fn wire_host_callbacks(&self) {
        let state = self.state.clone();
        let host = state.host.clone();
        let callbacks = host.callbacks();

        {
            let state = state.clone();
            *callbacks.on_init.borrow_mut() = Some(Rc::new(
                move |native: NativeRenderSurface, width: u32, height: u32| {
                    let application = Application {
                        state: state.clone(),
                    };
                    if let Err(error) = application.handle_init(native, width, height) {
                        application.fail_run_loop(&error, "main window initialization");
                    }
                },
            ));
        }
        {
            let state = state.clone();
            *callbacks.on_resize.borrow_mut() = Some(Rc::new(move |width: u32, height: u32| {
                let application = Application {
                    state: state.clone(),
                };
                if let Err(error) = application.handle_resize(width, height) {
                    application.fail_run_loop(&error, "main window resize");
                }
            }));
        }
        {
            let state = state.clone();
            *callbacks.on_event.borrow_mut() =
                Some(Rc::new(move |event: &cove_platform::InputEvent| {
                    state.callbacks.events.publish(event);
                }));
        }
        {
            // Serviced every loop iteration, not only when a frame is due, so an
            // idle app still answers DevTools between two display refreshes.
            let weak: Weak<ApplicationState> = Rc::downgrade(&state);
            *callbacks.on_tick.borrow_mut() = Some(Rc::new(move || {
                if let Some(state) = weak.upgrade() {
                    let application = Application { state };
                    application.drain_presented_frames();
                    application.with_diagnostics(|session| session.drain());
                }
            }));
        }
        {
            let state = state.clone();
            let weak: Weak<ApplicationState> = Rc::downgrade(&state);
            *callbacks.on_before_commit.borrow_mut() = Some(Rc::new(move |delta_time: f64| {
                if let Some(state) = weak.upgrade() {
                    Application { state }.handle_frame_preparation(delta_time);
                }
            }));
        }
        {
            let state = state.clone();
            let weak: Weak<ApplicationState> = Rc::downgrade(&state);
            *callbacks.on_frame.borrow_mut() = Some(Rc::new(
                move |_native: NativeRenderSurface| -> FrameOutcome {
                    match weak.upgrade() {
                        Some(state) => Application { state }.handle_frame(),
                        None => FrameOutcome::Skipped,
                    }
                },
            ));
        }
        {
            let state = state.clone();
            *callbacks.on_window_teardown.borrow_mut() =
                Some(Rc::new(move |window_id: WindowId| {
                    Application {
                        state: state.clone(),
                    }
                    .release_gpu_surface(window_id);
                }));
        }
        {
            let state = state.clone();
            *callbacks.on_window_presentation_changed.borrow_mut() =
                Some(Rc::new(move |window_id: WindowId, active: bool| {
                    Application {
                        state: state.clone(),
                    }
                    .handle_presentation_changed(window_id, active);
                }));
        }
        {
            let state = state.clone();
            *callbacks.on_teardown.borrow_mut() = Some(Rc::new(move || {
                let application = Application {
                    state: state.clone(),
                };
                application.release_all_gpu_surfaces();
                application.state.diagnostics.borrow_mut().stop();
            }));
        }
    }

    fn teardown(&self) {
        let state = &self.state;
        if let Some(mut pressure) = state.memory_pressure.borrow_mut().take() {
            pressure.stop();
        }
        if let Some(ready) = state.callbacks.diagnostics_ready.clone() {
            ready(false);
        }
        state.diagnostics.borrow_mut().stop();
        self.release_all_gpu_surfaces();
    }

    /// Collects finished frames: recycles their buffers, surfaces render errors and
    /// retries frames the renderer could not present.
    fn drain_presented_frames(&self) {
        let state = &self.state;
        let result = match state.window_renderer.borrow_mut().as_mut() {
            Some(renderer) => renderer.take_presented(),
            None => Ok(Vec::new()),
        };
        match result {
            Ok(frames) => {
                if let Some(frame) = frames.last() {
                    state.last_render_ms.set(frame.cpu_ms);
                }
                if frames.iter().any(|frame| !frame.presented) {
                    state.host.retry_display(None);
                }
            }
            Err(error) => self.fail_run_loop(&error.to_string(), "Frame rendering"),
        }
        // A refused frame becomes retryable the moment a slot frees, and slots
        // free here. Turning that into a display request is the wake the drop
        // path depends on: it can no longer be lost, because the render thread
        // is what reports it.
        let retry = match state.window_renderer.borrow_mut().as_mut() {
            Some(renderer) => renderer.take_retry_pending(),
            None => false,
        };
        if retry {
            state.host.retry_display(None);
        }
        let mut failed = Vec::new();
        let mut retry = Vec::new();
        for (id, window) in state.auxiliary_windows.borrow_mut().iter_mut() {
            match window.take_presented() {
                Ok(needs_retry) => {
                    if needs_retry {
                        retry.push(*id);
                    }
                }
                Err(error) => failed.push((*id, error)),
            }
        }
        for id in retry {
            state.host.retry_display(Some(id));
        }
        for (id, error) in failed {
            self.fail_auxiliary_window(id, &error, "rendering");
        }
    }

    fn fail_run_loop(&self, error: &str, context: &str) {
        {
            let mut slot = self.state.run_loop_error.borrow_mut();
            if slot.is_some() {
                return;
            }
            *slot = Some(format!("{context} failed: {error}"));
        }
        LogSystem::logger("com.cove.ui.app").log(&LogEvent::new(
            Level::Error,
            format!("{context} failed: {error}"),
        ));
        self.state.host.stop();
    }

    // MARK: - Text environment

    fn update_surface_environment(&self) {
        let state = &self.state;
        let host = &state.host;
        let mut environment = Environment::default();
        let logical = host.logical_size();
        environment.viewport.width = logical.0 as f32;
        environment.viewport.height = logical.1 as f32;
        environment.viewport.scale = host.content_scale_factor();
        let insets = host.safe_area();
        environment.safe_area = Insets {
            top: insets.top,
            bottom: insets.bottom,
            leading: insets.leading,
            trailing: insets.trailing,
        };
        // Refreshed every frame: UIKit posts keyboard frame changes on the main
        // queue and wakes the event loop, so the writing surface can avoid it.
        environment.keyboard = Keyboard::new(host.keyboard_height());
        #[cfg(any(target_os = "ios", target_os = "android"))]
        {
            environment.input = InputCapabilities::TOUCH | InputCapabilities::KEYBOARD;
            environment.idiom = cove_core::Idiom::Phone;
        }
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        {
            if previews_phone() {
                environment.input = InputCapabilities::TOUCH | InputCapabilities::KEYBOARD;
                environment.idiom = cove_core::Idiom::Phone;
                environment.safe_area.top = insets.top.max(47.0);
                environment.safe_area.bottom = insets.bottom.max(34.0);
            } else {
                environment.idiom = cove_core::Idiom::Desktop;
            }
        }
        environment.translucent_window = host.backdrop() != cove_shell::WindowBackdrop::Opaque;
        if environment != state.environment.get() {
            state.environment.set(environment);
        }
    }

    fn configure_text_environment(&self, requested_scale: f32) {
        let state = &self.state;
        let scale = requested_scale.max(1.0);
        {
            let typography = state.typography.borrow();
            if typography.atlas.is_some() && (scale - typography.scale).abs() < 0.01 {
                return;
            }
        }
        let mut typography = state.typography.borrow_mut();
        typography.scale = scale;
        let atlas_edge = ((1024.0 * scale).ceil() as usize).max(1024);
        let environment = TextEnvironment::bootstrapped(TextBootstrapOptions {
            atlas_texture_id: typography.texture_id,
            primary_font_name: state.config.typography.primary_font_name.clone(),
            default_font: Font::system(state.config.typography.font_size, FontWeight::Regular),
            default_line_height: state.config.typography.line_height,
            default_color: cove_core::Color::WHITE,
            raster_scale: scale,
            atlas_edge: Some(atlas_edge),
            cache_limits: state.config.memory.text,
        });
        typography.atlas = Some(environment.atlas.clone());
        let texture_id = typography.texture_id;
        drop(typography);
        TextEnvironmentHolder::set_current(Some(environment));
        set_content_scale(scale);
        let _ = texture_id;
    }

    fn upload_atlas_if_needed(&self, force: bool) -> Result<(), String> {
        let state = &self.state;
        let atlas = state.typography.borrow().atlas.clone();
        let Some(atlas) = atlas else { return Ok(()) };
        if !force && !atlas.is_dirty() {
            return Ok(());
        }
        let texture_id = state.typography.borrow().texture_id;
        // With a window renderer the upload is queued on its render thread, in
        // order with the frames that sample the new glyphs.
        let mut renderer = state.window_renderer.borrow_mut();
        match renderer.as_mut() {
            Some(renderer) => match state
                .rendering
                .take_font_atlas_upload(&atlas, texture_id)
                .map_err(|error| error.to_string())?
            {
                Some(upload) => renderer
                    .upload_font_atlas(upload)
                    .map_err(|error| error.to_string()),
                None => Ok(()),
            },
            None => state
                .rendering
                .upload_font_atlas(&atlas, texture_id)
                .map_err(|error| error.to_string()),
        }
    }

    // MARK: - Lifecycle callbacks

    fn handle_init(
        &self,
        native: NativeRenderSurface,
        width_px: u32,
        height_px: u32,
    ) -> Result<(), String> {
        let state = &self.state;
        {
            let mut main = state.main_window.borrow_mut();
            main.drawable_w = width_px;
            main.drawable_h = height_px;
            let logical = state.host.logical_size();
            main.logical_w = logical.0;
            main.logical_h = logical.1;
        }
        let mut settings =
            AppWindowRenderSettings::resolve(&state.config, state.config.window.rendering);
        if state.host.backdrop() != cove_shell::WindowBackdrop::Opaque {
            // Premultiplied clear: untouched pixels reveal the platform material.
            settings.clear_color = [0.0; 4];
            // An eight-bit sRGB layer is read as encoded premultiplied colour,
            // but the frame holds linear premultiplied colour: every partly
            // covered edge of a light shape would composite past white.
            settings.surface_format = cove_graphics::rhi::TextureFormat::Rgba16Float;
        }
        let mut renderer = state
            .rendering
            .make_window(settings)
            .map_err(|error| error.to_string())?;
        let vsync = state.main_window.borrow().is_vsync_enabled;
        renderer
            .configure(native, (width_px, height_px), vsync)
            .map_err(|error| error.to_string())?;
        *state.window_renderer.borrow_mut() = Some(renderer);

        self.update_surface_environment();
        self.configure_text_environment(state.host.content_scale_factor());

        let did_install_root = state.main_window.borrow().did_install_root;
        if !did_install_root {
            let window_id = state.host.main_window();
            let root_view = state.root_view.borrow().clone();
            if let Some(root_view) = root_view {
                let composition = state.composition.clone();
                let environment = state.environment.clone();
                let scaler = state.config.typography.scaler.clone();
                let logical = {
                    let main = state.main_window.borrow();
                    (main.logical_w as f32, main.logical_h as f32)
                };
                // Scope this window's overlay store for every consumer that reads
                // the process ambient (menus, popovers, tooltips). It has to be in
                // place before the install: a portal host resolves its store when
                // it mounts, and would otherwise watch the shared fallback while
                // presenters publish to this window's store.
                let context = state.host.input_context();
                context.add_scoped_ambient(Rc::new(PortalStoreScope::new(
                    composition.portal_store.clone(),
                )));
                context.with_current(|| {
                    chrome::with_window_chrome_context(window_id, || {
                        use cove_ui_compose::theme::tokens::TextScalerViewExt;
                        // The portal store is scoped per window by the input context
                        // ambient above, which is what `PortalStoreHolder::current()`
                        // actually reads; the composition-local copy would need
                        // `PortalStore: PartialEq`, which the registry does not provide.
                        let content = AdaptiveApplicationContent::new(root_view, environment)
                            .text_scaler(scaler);
                        composition.install_view(AnyView::new(content), logical.0, logical.1);
                    });
                });
                self.sync_main_window_chrome_hit_test();
                state.main_window.borrow_mut().did_install_root = true;
            }
        }

        self.upload_atlas_if_needed(false)?;
        state.main_window.borrow_mut().last_frame_time = Some(Instant::now());
        self.install_input_interceptor();
        state.host.request_display(None);
        Ok(())
    }

    fn handle_resize(&self, width_px: u32, height_px: u32) -> Result<(), String> {
        let state = &self.state;
        {
            let mut main = state.main_window.borrow_mut();
            main.drawable_w = width_px;
            main.drawable_h = height_px;
            let logical = state.host.logical_size();
            main.logical_w = logical.0;
            main.logical_h = logical.1;
        }
        let vsync = state.main_window.borrow().is_vsync_enabled;
        if let Some(renderer) = state.window_renderer.borrow_mut().as_mut() {
            renderer
                .resize((width_px, height_px), vsync)
                .map_err(|error| error.to_string())?;
        }
        self.update_surface_environment();
        self.configure_text_environment(state.host.content_scale_factor());
        self.upload_atlas_if_needed(false)
    }

    pub fn set_vsync_enabled(&self, enabled: bool) {
        let state = &self.state;
        let enabled = enabled && !force_vsync_off();
        let changed = state.main_window.borrow().is_vsync_enabled != enabled;
        if changed {
            state.main_window.borrow_mut().is_vsync_enabled = enabled;
            let (width, height) = {
                let main = state.main_window.borrow();
                (main.drawable_w, main.drawable_h)
            };
            if let Some(renderer) = state.window_renderer.borrow_mut().as_mut()
                && let Err(error) = renderer.resize((width, height), enabled)
            {
                LogSystem::logger("com.cove.ui.app").log(&LogEvent::new(
                    Level::Warning,
                    format!("Main window present mode update failed: {error}"),
                ));
            }
            for window in state.auxiliary_windows.borrow_mut().values_mut() {
                window.set_vsync_enabled(enabled);
            }
        }
        // VSync controls the surface present mode only. Frame cadence is owned
        // by the frame-drive policy or an explicit runtime frame-rate override;
        // tying it to VSync would change input/update pacing when the user only
        // asked to change presentation.
        state.host.request_display(None);
    }

    fn sync_main_window_chrome_hit_test(&self) {
        let state = &self.state;
        let Some(window_id) = state.host.main_window() else {
            return;
        };
        let root = state.composition.tree.borrow().root();
        if let Some(root) = root.as_ref() {
            if let Some(appearance) = crate::desktop::window::appearance::changed_appearance(root) {
                state.host.set_window_appearance(window_id, appearance);
            }
            crate::desktop::window::appearance::sync_backdrop_tint(
                root,
                state.environment.get().translucent_window,
            );
        }
        let next = root.and_then(|root| state.chrome_collector.borrow_mut().collect(&root));
        let mut last = state.last_chrome_hit_test.borrow_mut();
        match next {
            Some(next) => {
                if last.as_ref() == Some(&next) {
                    return;
                }
                state
                    .host
                    .set_window_chrome_hit_test(window_id, Some(&next));
                *last = Some(next);
            }
            None => {
                if last.is_some() {
                    state.host.set_window_chrome_hit_test(window_id, None);
                    *last = None;
                }
            }
        }
    }

    fn handle_frame_preparation(&self, delta_time: f64) {
        let state = &self.state;
        // Platform pressure is sampled off the UI thread; the trim itself must run
        // here, where every cache it touches is single-threaded.
        if state
            .memory_pressure
            .borrow()
            .as_ref()
            .is_some_and(ApplicationMemoryPressure::take_pending_trim)
        {
            self.trim_memory();
        }
        state.performance_hud.borrow_mut().tick();
        let delta = delta_time.max(0.0);
        state.main_window.borrow_mut().last_frame_time = Some(Instant::now());
        let tick_start = Instant::now();
        if let Some(tick) = state.callbacks.tick.clone() {
            tick(delta);
        }
        if fps_log_enabled() {
            state.metrics.borrow_mut().fps_tick_accum +=
                tick_start.elapsed().as_secs_f64() * 1000.0;
        }
        self.sync_auxiliary_windows();
    }

    fn handle_frame(&self) -> FrameOutcome {
        let state = &self.state;
        {
            let mut renderer = state.window_renderer.borrow_mut();
            let Some(renderer) = renderer.as_mut() else {
                return FrameOutcome::Skipped;
            };
            if !renderer.is_configured() {
                return FrameOutcome::Skipped;
            }
            if !renderer.has_frame_capacity() {
                // Every slot is in flight. This is pacing, not failure, and the
                // renderer owes us a wake once a slot frees.
                renderer.note_frame_dropped();
                state
                    .frame_drops
                    .set(state.frame_drops.get().saturating_add(1));
                return FrameOutcome::Skipped;
            }
        }
        let Some(root) = state.composition.tree.borrow().root() else {
            return FrameOutcome::Skipped;
        };

        let frame_start = Instant::now();
        self.with_diagnostics(|session| session.begin_frame());
        let timeline = state.composition.tree.borrow().timeline.clone();

        let environment_trace = timeline.borrow().begin();
        self.update_surface_environment();
        self.configure_text_environment(state.host.content_scale_factor());
        timeline.borrow_mut().end(
            environment_trace,
            "frame",
            "Surface + text environment",
            None,
            &[],
        );

        let layout_start = Instant::now();
        let (logical_w, logical_h) = {
            let main = state.main_window.borrow();
            (main.logical_w as f32, main.logical_h as f32)
        };
        let _ = state
            .composition
            .graph
            .compute_layout_if_needed(logical_w, logical_h);

        let chrome_trace = timeline.borrow().begin();
        self.sync_main_window_chrome_hit_test();
        timeline
            .borrow_mut()
            .end(chrome_trace, "frame", "Window chrome hit test", None, &[]);

        let session = state.host.main_session();
        let accessibility_trace = timeline.borrow().begin();
        if let Some(session) = &session {
            NativeAccessibility::synchronize(&state.host, session, &root, &timeline);
        }
        timeline.borrow_mut().end(
            accessibility_trace,
            "frame",
            "Accessibility sync",
            None,
            &[],
        );
        let layout_end = Instant::now();

        {
            let mut list = state.composition.draw_list.borrow_mut();
            list.reset();
            // Edges are snapped to the physical pixel grid as they are emitted;
            // see `DrawList::pixel_scale`. Set before drawing, not at record time.
            list.pixel_scale = state.host.content_scale_factor();
        }
        let draw_trace = timeline.borrow().begin();
        {
            let mut list = state.composition.draw_list.borrow_mut();
            if state.composition.use_legacy_renderer {
                state.composition.node_renderer.render(&root, &mut list);
            } else {
                state
                    .composition
                    .layer_renderer
                    .borrow_mut()
                    .render(state.composition.graph.render_tree(), &mut list);
            }
            if state.config.diagnostics.shows_performance_hud {
                state.performance_hud.borrow_mut().draw(&mut list);
            }
            self.with_diagnostics(|session| session.overlay(&mut list));
        }
        let draw_end = Instant::now();
        timeline
            .borrow_mut()
            .end(draw_trace, "draw", "Encode draw list", None, &[]);
        let prepared_layout_ms = layout_end.duration_since(layout_start).as_secs_f64() * 1000.0;
        let prepared_draw_ms = draw_end.duration_since(layout_end).as_secs_f64() * 1000.0;
        self.with_diagnostics(|session| session.prepared(prepared_layout_ms, prepared_draw_ms));

        let count_trace = timeline.borrow().begin();
        let frame_node_count = state.composition.tree.borrow_mut().node_count();
        timeline
            .borrow_mut()
            .end(count_trace, "frame", "Node count", None, &[]);

        if state
            .typography
            .borrow()
            .atlas
            .as_ref()
            .is_some_and(|atlas| atlas.is_dirty())
            && let Err(error) = self.upload_atlas_if_needed(true)
        {
            self.fail_run_loop(&error, "Font atlas upload");
            return FrameOutcome::Failed;
        }

        {
            let mut tree = state.composition.tree.borrow_mut();
            let list = state.composition.draw_list.borrow();
            self.with_diagnostics(|session| session.presented(&mut tree, &list, true));
        }
        // Submitting takes the list, and the readback belongs behind the primary
        // frame, so a capture works from a copy.
        let mut mirror_frame = None;
        {
            let list = state.composition.draw_list.borrow();
            self.with_diagnostics(|session| {
                if session.mirror_wants_frame() {
                    mirror_frame = Some(list.clone());
                }
            });
        }
        let submitted = {
            let mut renderer = state.window_renderer.borrow_mut();
            let Some(renderer) = renderer.as_mut() else {
                return FrameOutcome::Skipped;
            };
            let mut list = state.composition.draw_list.borrow_mut();
            renderer.submit(&mut list, (logical_w, logical_h))
        };
        if let Some(list) = mirror_frame.filter(|_| submitted.is_ok()) {
            let pixels = {
                let main = state.main_window.borrow();
                (main.drawable_w, main.drawable_h)
            };
            self.with_diagnostics(|session| {
                session.mirror_capture(&list, pixels, (logical_w, logical_h));
            });
        }
        match submitted {
            Ok(Some(_)) => {}
            Ok(None) => {
                // Lost the race for a slot between the check above and here.
                state
                    .frame_drops
                    .set(state.frame_drops.get().saturating_add(1));
                return FrameOutcome::Skipped;
            }
            Err(error) => {
                self.fail_run_loop(&error.to_string(), "Frame rendering");
                return FrameOutcome::Failed;
            }
        }
        if state.config.frames.policy == AppFrameDrivePolicy::Continuous
            || force_continuous_frame_drive()
        {
            // Legacy behavior: keep re-rendering at display rate. Under
            // `.eventDriven` the loop's needs-display machinery schedules the
            // next frame instead.
            state.host.request_display(None);
        }
        // The mirror needs a steady frame stream even when the UI is idle.
        let mut mirroring = false;
        self.with_diagnostics(|session| mirroring = session.mirror_is_active());
        if mirroring {
            state.host.request_display(None);
        }
        state.performance_hud.borrow_mut().presented();
        // Read-and-reset either way, so a window measured later does not report
        // drops that happened while the log was off.
        let dropped = state.frame_drops.replace(0);
        if fps_log_enabled() {
            let frame_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
            state.metrics.borrow_mut().record(
                layout_end.duration_since(layout_start).as_secs_f64() * 1000.0,
                draw_end.duration_since(layout_end).as_secs_f64() * 1000.0,
                state.last_render_ms.get(),
                frame_ms,
                dropped,
            );
        }
        let _ = frame_node_count;
        FrameOutcome::Rendered
    }

    // MARK: - Presentation

    fn handle_presentation_changed(&self, window_id: WindowId, active: bool) {
        let state = &self.state;
        if state.host.main_window() == Some(window_id) {
            if let Some(renderer) = state.window_renderer.borrow_mut().as_mut()
                && let Err(error) = renderer.set_presentation_active(active)
            {
                self.fail_run_loop(&error.to_string(), "window presentation transition");
                return;
            }
            state.callbacks.events.publish(&if active {
                cove_platform::InputEvent::WindowExposed
            } else {
                cove_platform::InputEvent::WindowOccluded
            });
            // When the window goes off-screen the swapchain / MSAA / framebuffer
            // are already released. Also drop the heavy CPU caches (glyph atlas,
            // image assets, render tree) so backgrounding actually reclaims
            // memory. Skip this while mirroring so the offscreen capture path
            // keeps its shared texture uploads warm.
            let mut mirroring = false;
            self.with_diagnostics(|session| mirroring = session.mirror_is_active());
            if !active && !mirroring {
                self.trim_memory();
            }
        } else if let Some(window) = state.auxiliary_windows.borrow_mut().get_mut(&window_id)
            && let Err(error) = window.set_presentation_active(active)
        {
            self.fail_run_loop(&error, "window presentation transition");
        }
    }

    // MARK: - Memory

    pub fn memory_snapshot(&self) -> AppMemorySnapshot {
        let state = &self.state;
        let mut windows = HashMap::new();
        for (id, window) in state.auxiliary_windows.borrow().iter() {
            if let Some(snapshot) = window.memory_snapshot() {
                windows.insert(*id, snapshot);
            }
        }
        if let Some(id) = state.host.main_window()
            && let Some(renderer) = state.window_renderer.borrow().as_ref()
        {
            windows.insert(id, renderer.memory_snapshot());
        }
        let text = TextEnvironmentHolder::current();
        AppMemorySnapshot {
            shaping: text.as_ref().map(TextEnvironment::shaping_cache_snapshot),
            text_layout: text.as_ref().map(TextEnvironment::layout_cache_snapshot),
            cached_image_bytes: state.composition.image_assets.cached_bytes(),
            windows,
        }
    }

    /// Snapshot in the counters/windows shape DevTools consumes.
    pub fn diagnostic_memory_snapshot(
        &self,
    ) -> crate::desktop::diagnostics::DiagnosticMemorySnapshot {
        let value = self.memory_snapshot();
        let state = &self.state;
        let counters = BTreeMap::from([
            (
                "shapingEntries".to_owned(),
                value
                    .shaping
                    .as_ref()
                    .map_or(0, |cache| cache.entries as u64),
            ),
            (
                "shapingBytes".to_owned(),
                value
                    .shaping
                    .as_ref()
                    .map_or(0, |cache| cache.estimated_bytes as u64),
            ),
            (
                "layoutEntries".to_owned(),
                value
                    .text_layout
                    .as_ref()
                    .map_or(0, |cache| cache.entries as u64),
            ),
            (
                "layoutBytes".to_owned(),
                value
                    .text_layout
                    .as_ref()
                    .map_or(0, |cache| cache.estimated_bytes as u64),
            ),
            (
                "imageEntries".to_owned(),
                state.composition.image_assets.cached_count() as u64,
            ),
            ("imageBytes".to_owned(), value.cached_image_bytes as u64),
            (
                "skippedParentUpdates".to_owned(),
                state.composition.graph.skipped_parent_updates() as u64,
            ),
        ]);
        // `DiagnosticMemorySnapshot.windows` is a flat map, so each window's
        // counters are keyed `<window>.<counter>` to keep every value while
        // matching the shared shape.
        let mut windows = BTreeMap::new();
        for (id, window) in value.windows {
            let prefix = format!("{id:?}");
            for (name, count) in window.counters() {
                windows.insert(format!("{prefix}.{name}"), count);
            }
        }
        crate::desktop::diagnostics::DiagnosticMemorySnapshot { counters, windows }
    }

    pub fn trim_memory(&self) {
        let state = &self.state;
        if let Some(text) = TextEnvironmentHolder::current() {
            text.trim_text_caches();
        }
        state.composition.trim_memory();
        for window in state.auxiliary_windows.borrow().values() {
            window.trim_memory();
        }
    }

    fn release_all_gpu_surfaces(&self) {
        self.release_main_window_renderer();
        for window in self.state.auxiliary_windows.borrow_mut().values_mut() {
            window.release_gpu_surface();
        }
    }

    fn release_main_window_renderer(&self) {
        let renderer = self.state.window_renderer.borrow_mut().take();
        if let Some(mut renderer) = renderer
            && let Err(error) = renderer.close()
        {
            let mut slot = self.state.run_loop_error.borrow_mut();
            if slot.is_none() {
                *slot = Some(format!("Main window GPU cleanup failed: {error}"));
            }
            LogSystem::logger("com.cove.ui.app").log(&LogEvent::new(
                Level::Error,
                format!("Main window GPU cleanup failed: {error}"),
            ));
        }
    }

    fn release_gpu_surface(&self, window_id: WindowId) {
        if self.state.host.main_window() == Some(window_id) {
            self.release_main_window_renderer();
        } else if let Some(window) = self
            .state
            .auxiliary_windows
            .borrow_mut()
            .get_mut(&window_id)
        {
            window.release_gpu_surface();
        }
    }

    // MARK: - Auxiliary windows

    pub fn open_auxiliary_window(&self, request: AppAuxiliaryWindowRequest) -> Option<WindowId> {
        let state = &self.state;
        self.sync_auxiliary_windows();

        let tree = Rc::new(RefCell::new(cove_core::scene::NodeTree::new()));
        let recomposer = state.host.recomposer();
        let name = request.title.clone();
        let options = WindowOptions {
            width: request.width,
            height: request.height,
            title_bar_style: state.config.window.title_bar_style.platform_style(),
            shows_dock_icon: state.config.window.shows_dock_icon,
            ..WindowOptions::default()
        };
        let session = match state.host.open_window(
            &name,
            tree,
            recomposer,
            state.host.input_context(),
            options,
        ) {
            Ok(session) => session,
            Err(error) => {
                LogSystem::logger("com.cove.ui.app").log(&LogEvent::new(
                    Level::Warning,
                    format!("Auxiliary window open failed: {error}"),
                ));
                return None;
            }
        };
        let window_id = session.id;
        let portal_store = Rc::new(PortalStore::new());
        let host = state.host.clone();
        let vsync = state.main_window.borrow().is_vsync_enabled;
        let window = AuxiliaryAppWindow::new(
            session.clone(),
            request.root_view,
            portal_store,
            state.rendering.clone(),
            state.config.clone(),
            request.rendering,
            vsync,
            state.composition.use_legacy_renderer,
            Rc::new({
                let host = host.clone();
                move |id, hit_test| host.set_window_chrome_hit_test(id, hit_test)
            }),
            Rc::new(move |id, appearance| host.set_window_appearance(id, appearance)),
        );
        state
            .auxiliary_windows
            .borrow_mut()
            .insert(window_id, window);

        // The Swift version installed per-window init/resize/frame hooks; the
        // runtime session owns the callback slots, so the host drives them from
        // those same slots.
        {
            let state = state.clone();
            *session.callbacks.on_init.borrow_mut() = Some(Rc::new(
                move |native: NativeRenderSurface, width: u32, height: u32| {
                    let application = Application {
                        state: state.clone(),
                    };
                    if let Err(error) = application.auxiliary_init(window_id, native, width, height)
                    {
                        application.fail_auxiliary_window(window_id, &error, "initialization");
                    }
                },
            ));
        }
        {
            let state = state.clone();
            *session.callbacks.on_resize.borrow_mut() =
                Some(Rc::new(move |width: u32, height: u32| {
                    let application = Application {
                        state: state.clone(),
                    };
                    if let Err(error) = application.auxiliary_resize(window_id, width, height) {
                        application.fail_auxiliary_window(window_id, &error, "resize");
                    }
                }));
        }
        {
            let state = state.clone();
            *session.callbacks.on_frame.borrow_mut() = Some(Rc::new(
                move |_native: NativeRenderSurface| -> FrameOutcome {
                    Application {
                        state: state.clone(),
                    }
                    .auxiliary_frame(window_id)
                },
            ));
        }
        session.request_display();
        Some(window_id)
    }

    fn auxiliary_init(
        &self,
        window_id: WindowId,
        native: NativeRenderSurface,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        let state = &self.state;
        let mut windows = state.auxiliary_windows.borrow_mut();
        let Some(window) = windows.get_mut(&window_id) else {
            return Ok(());
        };
        let typography = ApplicationTypography {
            scale: state.typography.borrow().scale,
            atlas: None,
            texture_id: state.typography.borrow().texture_id,
        };
        let _ = typography;
        window.handle_init(
            native,
            width,
            height,
            &|scale| self.configure_text_environment(scale),
            &|force| self.upload_atlas_if_needed(force),
        )
    }

    fn auxiliary_resize(&self, window_id: WindowId, width: u32, height: u32) -> Result<(), String> {
        let mut windows = self.state.auxiliary_windows.borrow_mut();
        let Some(window) = windows.get_mut(&window_id) else {
            return Ok(());
        };
        window.handle_resize(
            width,
            height,
            &|scale| self.configure_text_environment(scale),
            &|force| self.upload_atlas_if_needed(force),
        )
    }

    fn auxiliary_frame(&self, window_id: WindowId) -> FrameOutcome {
        let mut windows = self.state.auxiliary_windows.borrow_mut();
        let Some(window) = windows.get_mut(&window_id) else {
            return FrameOutcome::Skipped;
        };
        if window.handle_frame(
            &self.state.host,
            &|scale| self.configure_text_environment(scale),
            &|force| self.upload_atlas_if_needed(force),
        ) {
            FrameOutcome::Rendered
        } else {
            // An auxiliary window that cannot draw yet is pacing, not failure.
            FrameOutcome::Skipped
        }
    }

    pub fn close_auxiliary_window(&self, window_id: WindowId) {
        self.state.host.close_window(window_id);
        self.state.auxiliary_windows.borrow_mut().remove(&window_id);
    }

    fn fail_auxiliary_window(&self, window_id: WindowId, error: &str, context: &str) {
        LogSystem::logger("com.cove.ui.app").log(&LogEvent::new(
            Level::Error,
            format!("auxiliary window {context} failed: {error}"),
        ));
        let state = self.state.clone();
        let host = state.host.clone();
        host.enqueue_main_thread_work(move || {
            Application { state }.close_auxiliary_window(window_id);
        });
    }

    pub fn is_auxiliary_window_open(&self, window_id: WindowId) -> bool {
        self.sync_auxiliary_windows();
        let known = self
            .state
            .auxiliary_windows
            .borrow()
            .contains_key(&window_id);
        known && self.state.host.session(window_id).is_some()
    }

    fn sync_auxiliary_windows(&self) {
        let live = self.state.host.window_ids();
        self.state
            .auxiliary_windows
            .borrow_mut()
            .retain(|id, _| live.contains(id));
    }
}

/// Full screen and floating windows for widgets such as the video player.
struct HostWindowPresenter {
    state: Weak<ApplicationState>,
}

impl cove_ui_compose::core::WindowPresenter for HostWindowPresenter {
    fn set_fullscreen(&self, fullscreen: bool) -> bool {
        let Some(state) = self.state.upgrade() else {
            return false;
        };
        let Some(window) = state.host.main_window() else {
            return false;
        };
        state.host.set_window_fullscreen(window, fullscreen);
        true
    }

    fn is_fullscreen(&self) -> bool {
        self.state.upgrade().is_some_and(|state| {
            state
                .host
                .main_window()
                .is_some_and(|window| state.host.is_window_fullscreen(window))
        })
    }

    fn open_floating(
        &self,
        request: cove_ui_compose::core::FloatingWindowRequest,
    ) -> Option<cove_ui_compose::core::FloatingWindowId> {
        let state = self.state.upgrade()?;
        let application = Application {
            state: state.clone(),
        };
        let id = application.open_auxiliary_window(AppAuxiliaryWindowRequest {
            title: request.title,
            width: request.width.round().max(160.0) as i32,
            height: request.height.round().max(90.0) as i32,
            root_view: request.content,
            rendering: Default::default(),
        })?;
        state.host.set_window_always_on_top(id, true);
        Some(cove_ui_compose::core::FloatingWindowId(u64::from(id.0)))
    }

    fn close_floating(&self, id: cove_ui_compose::core::FloatingWindowId) {
        if let Some(state) = self.state.upgrade()
            && let Ok(raw) = u32::try_from(id.0)
        {
            Application { state }.close_auxiliary_window(WindowId(raw));
        }
    }

    fn is_floating_open(&self, id: cove_ui_compose::core::FloatingWindowId) -> bool {
        let Some(state) = self.state.upgrade() else {
            return false;
        };
        u32::try_from(id.0)
            .is_ok_and(|raw| Application { state }.is_auxiliary_window_open(WindowId(raw)))
    }
}
