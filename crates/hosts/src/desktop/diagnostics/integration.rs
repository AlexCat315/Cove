//! Diagnostics integration seam shared by the desktop and Android hosts.
//!
//! The DevTools crate is not a dependency of `cove-hosts` and its Rust module
//! tree is currently declared without implementations, so the hosts accept an
//! injected diagnostics backend. When no backend is installed the session is a
//! null object: inspection overlays and input interception are absent, while
//! memory counters remain available to the native host.

use cove_core::DrawList;
use cove_core::scene::{InvalidationLog, NodeTree};
use cove_graphics::rhi::Device;
use cove_graphics::ui::NativeDrawListRenderer;
use cove_platform::InputEvent;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DevToolsConfiguration {
    pub port: u16,
    pub app_title: String,
    pub launch: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiagnosticMemorySnapshot {
    pub counters: BTreeMap<String, u64>,
    pub windows: BTreeMap<String, u64>,
}

/// The renderer an offscreen mirror capture draws with.
pub struct DiagnosticsFrameSource {
    pub device: Arc<Device>,
    pub renderer: NativeDrawListRenderer,
}

pub struct DiagnosticsCallbacks {
    pub invalidate: Rc<dyn Fn()>,
    pub memory: Rc<dyn Fn() -> DiagnosticMemorySnapshot>,
    pub invalidation_log: Option<Arc<InvalidationLog>>,
    /// `None` where the host cannot read frames back.
    pub frame_source: Option<DiagnosticsFrameSource>,
    /// Keeps frames flowing while a mirror session is active, including while the
    /// window is occluded, and restores the host's policy afterwards.
    pub mirror_active: Rc<dyn Fn(bool)>,
    /// `None` where windows cannot be resized. Returns why a resize failed.
    pub resize_window: Option<Rc<dyn Fn(f32, f32) -> Option<String>>>,
    /// `None` where the host cannot replace its reported facts. Returns the
    /// environment the next frame publishes.
    pub environment: Option<
        Rc<dyn Fn(&cove_core::EnvironmentRequest) -> Result<cove_core::Environment, String>>,
    >,
}

/// One DevTools session bound to a single host tree and render graph.
pub trait DiagnosticsSession {
    fn drain(&mut self);
    fn intercept(&mut self, event: &InputEvent) -> bool;
    fn overlay(&mut self, list: &mut DrawList);
    fn begin_frame(&mut self);
    fn prepared(&mut self, layout_ms: f64, draw_ms: f64);
    /// Reports a frame handed to the render thread, with the list it recorded.
    /// `presented` is false when the host had to skip the frame and will retry.
    fn presented(&mut self, tree: &mut NodeTree, list: &DrawList, presented: bool);
    /// Receives the host's injection point for remote input. Hosts that cannot
    /// deliver synthetic events keep the default no-op.
    fn attach_input(&mut self, inject: Rc<dyn Fn(InputEvent)>) {
        let _ = inject;
    }
    /// Whether a mirror session is active, for hosts deciding what to keep warm.
    fn mirror_is_active(&self) -> bool {
        false
    }
    /// Whether the next frame would be captured, so the host only keeps a copy of
    /// the draw list when it is needed.
    fn mirror_wants_frame(&self) -> bool {
        false
    }
    /// Reads `list` back for the mirror. Runs after the frame is handed over; the
    /// capture takes the device's frame session, so it orders with presentation.
    fn mirror_capture(&mut self, list: &DrawList, pixels: (u32, u32), logical: (f32, f32)) {
        let _ = (list, pixels, logical);
    }
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

pub(crate) fn take_factory() -> Option<DiagnosticsFactory> {
    FACTORY.with(|slot| slot.borrow().clone())
}

pub struct NullDiagnosticsSession;
impl DiagnosticsSession for NullDiagnosticsSession {
    fn drain(&mut self) {}
    fn intercept(&mut self, _event: &InputEvent) -> bool {
        false
    }
    fn overlay(&mut self, _list: &mut DrawList) {}
    fn begin_frame(&mut self) {}
    fn prepared(&mut self, _layout_ms: f64, _draw_ms: f64) {}
    fn presented(&mut self, _tree: &mut NodeTree, _list: &DrawList, _presented: bool) {}
    fn stop(&mut self) {}
}
