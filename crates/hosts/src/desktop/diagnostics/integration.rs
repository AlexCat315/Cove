//! Diagnostics integration seam shared by the desktop and Android hosts.
//!
//! The DevTools crate is not a dependency of `cove-hosts` and its Rust module
//! tree is currently declared without implementations, so the hosts accept an
//! injected diagnostics backend. When no backend is installed the session is a
//! null object: inspection overlays and input interception are absent, while
//! memory counters remain available to the native host.

use cove_core::DrawList;
use cove_core::scene::{InvalidationLog, NodeTree};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DevToolsConfiguration {
    pub app_title: String,
    pub window: Option<u64>,
    pub connection: Option<(String, String)>,
}

/// The host's platform memory-pressure signal, in the shape DevTools consumes.
///
/// `subscribed` is false where the host never wired the signal, which is not a
/// measured "normal".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MemoryPressure {
    pub subscribed: bool,
    pub low: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiagnosticMemorySnapshot {
    pub counters: BTreeMap<String, u64>,
    pub windows: BTreeMap<String, u64>,
    pub pressure: MemoryPressure,
}

#[cfg(not(target_arch = "wasm32"))]
pub type FrameCapture = std::sync::mpsc::Receiver<
    cove_graphics::rhi::RhiResult<crate::desktop::window::native_renderer::CapturedFrame>,
>;
#[cfg(not(target_arch = "wasm32"))]
pub type CaptureRequest = Rc<dyn Fn() -> Result<FrameCapture, String>>;

pub struct DiagnosticsCallbacks {
    #[cfg(not(target_arch = "wasm32"))]
    pub capture: Option<CaptureRequest>,
    pub viewport: Rc<dyn Fn() -> (f32, f32)>,
    pub input: Rc<dyn Fn(cove_platform::InputEvent)>,
    pub invalidate: Rc<dyn Fn()>,
    pub memory: Rc<dyn Fn() -> DiagnosticMemorySnapshot>,
    pub invalidation_log: Option<Arc<InvalidationLog>>,
    /// `None` where windows cannot be resized. Returns why a resize failed.
    pub resize_window: Option<Rc<dyn Fn(f32, f32) -> Option<String>>>,
    /// `None` where the host cannot replace its reported facts. Returns the
    /// environment the next frame publishes.
    pub environment: Option<
        Rc<dyn Fn(&cove_core::EnvironmentRequest) -> Result<cove_core::Environment, String>>,
    >,
    /// `None` where the host cannot observe where its content sits on screen.
    /// A client must then refuse point selection instead of guessing.
    pub content_geometry: Option<Rc<dyn Fn() -> Result<ContentGeometry, String>>>,
    /// The display's real refresh rate in Hz, or `None` where the host cannot
    /// observe it. A client must not assume 60 Hz when this is absent.
    pub display_refresh_rate: Option<Rc<dyn Fn() -> Option<f64>>>,
}

/// Where a runtime's content starts in the window-server coordinate space a
/// platform capture reports, plus the runtime viewport it holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContentGeometry {
    /// Content `(0, 0)` in window-server units (the space window bounds use).
    pub origin: (f64, f64),
    /// Window-server units per runtime logical point.
    pub scale: f64,
    /// Content viewport in runtime logical points.
    pub size: (f64, f64),
}

/// Whether the platform reports window positions in the global screen space a
/// capture uses.
///
/// Wayland deliberately withholds global window positions, so SDL only knows
/// the display origin there. Reporting that as content geometry would invent
/// coordinates; callers refuse instead. An explicitly x11-backed SDL session
/// keeps its real position.
#[cfg(target_os = "linux")]
pub fn window_content_position_is_global() -> bool {
    if let Ok(driver) = std::env::var("SDL_VIDEODRIVER") {
        if driver.eq_ignore_ascii_case("wayland") {
            return false;
        }
        if driver.eq_ignore_ascii_case("x11") {
            return true;
        }
    }
    std::env::var_os("WAYLAND_DISPLAY").is_none()
}

#[cfg(not(target_os = "linux"))]
pub fn window_content_position_is_global() -> bool {
    true
}

/// Builds content geometry from the window facts the platform shell publishes.
///
/// `position` is SDL's content-area origin: the vendored SDL backends store
/// `ClientToScreen` on Windows/X11 and the AppKit content rect on Apple, which
/// is the window-server space captures use. `logical_size` is the runtime
/// viewport. `position_is_logical` distinguishes Apple's point space from the
/// physical-pixel space Windows and X11 report.
pub fn window_content_geometry(
    position: Option<(f32, f32)>,
    logical_size: (u32, u32),
    content_scale: f32,
    position_is_logical: bool,
) -> Result<ContentGeometry, String> {
    let (x, y) = position.ok_or("The platform does not report the window's content position.")?;
    let scale = if position_is_logical {
        1.0
    } else {
        f64::from(content_scale)
    };
    if !x.is_finite() || !y.is_finite() || !scale.is_finite() || scale <= 0.0 {
        return Err("The platform reported non-finite content geometry.".to_owned());
    }
    Ok(ContentGeometry {
        origin: (f64::from(x), f64::from(y)),
        scale,
        size: (f64::from(logical_size.0), f64::from(logical_size.1)),
    })
}

/// One DevTools session bound to a single host tree and render graph.
pub trait DiagnosticsSession {
    fn drain(&mut self);
    fn overlay(&mut self, list: &mut DrawList);
    fn begin_frame(&mut self);
    fn prepared(&mut self, layout_ms: f64, draw_ms: f64);
    fn frame_finished(&mut self, nodes: usize, batches: usize, token: Option<u64>);
    fn presentation_finished(&mut self, token: u64, presented: bool, render_thread_ms: f64, gpu: Option<cove_graphics::rhi::GpuFrameTiming>);
    fn stop(&mut self);
}

pub type DiagnosticsFactory = Rc<
    dyn Fn(
        DevToolsConfiguration,
        Rc<RefCell<NodeTree>>,
        Rc<cove_composition::ViewGraph>,
        Rc<dyn Fn()>,
        DiagnosticsCallbacks,
    ) -> Result<Box<dyn DiagnosticsSession>, String>,
>;

thread_local! {
    static FACTORY: RefCell<Option<DiagnosticsFactory>> = const { RefCell::new(None) };
}

/// Installs the process diagnostics backend. Called by a DevTools-enabled build;
/// the default (no factory) is the null session.
pub fn set_diagnostics_factory(factory: Option<DiagnosticsFactory>) {
    FACTORY.with(|slot| *slot.borrow_mut() = factory);
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn take_factory() -> Option<DiagnosticsFactory> {
    FACTORY.with(|slot| slot.borrow().clone())
}

pub struct NullDiagnosticsSession;
impl DiagnosticsSession for NullDiagnosticsSession {
    fn drain(&mut self) {}
    fn overlay(&mut self, _list: &mut DrawList) {}
    fn begin_frame(&mut self) {}
    fn prepared(&mut self, _layout_ms: f64, _draw_ms: f64) {}
    fn frame_finished(&mut self, _nodes: usize, _batches: usize, _token: Option<u64>) {}
    fn presentation_finished(&mut self, _token: u64, _presented: bool, _render_thread_ms: f64, _gpu: Option<cove_graphics::rhi::GpuFrameTiming>) {}
    fn stop(&mut self) {}
}
