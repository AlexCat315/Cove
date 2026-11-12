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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiagnosticMemorySnapshot {
    pub counters: BTreeMap<String, u64>,
    pub windows: BTreeMap<String, u64>,
}

pub struct DiagnosticsCallbacks {
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
}

/// One DevTools session bound to a single host tree and render graph.
pub trait DiagnosticsSession {
    fn drain(&mut self);
    fn overlay(&mut self, list: &mut DrawList);
    fn begin_frame(&mut self);
    fn prepared(&mut self, layout_ms: f64, draw_ms: f64);
    fn frame_finished(&mut self, nodes: usize, batches: usize, token: Option<u64>);
    fn presentation_finished(&mut self, token: u64, presented: bool, render_thread_ms: f64);
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
    fn presentation_finished(&mut self, _token: u64, _presented: bool, _render_thread_ms: f64) {}
    fn stop(&mut self) {}
}
