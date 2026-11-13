//! Translation of Android/Integration/Diagnostics.swift.

use crate::desktop::diagnostics::integration::{
    CaptureRequest, ContentGeometry, DevToolsConfiguration, DiagnosticMemorySnapshot,
    DiagnosticsCallbacks, DiagnosticsSession, NullDiagnosticsSession, take_factory,
};
use cove_composition::ViewGraph;
use cove_core::DrawList;
use cove_core::scene::{InvalidationLog, NodeTree};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

pub struct AndroidDiagnosticsHooks {
    pub capture: CaptureRequest,
    pub viewport: Rc<dyn Fn() -> (f32, f32)>,
    pub input: Rc<dyn Fn(cove_platform::InputEvent)>,
    pub invalidation_log: Arc<InvalidationLog>,
    pub environment: Option<
        Rc<dyn Fn(&cove_core::EnvironmentRequest) -> Result<cove_core::Environment, String>>,
    >,
    pub content_geometry: Option<Rc<dyn Fn() -> Result<ContentGeometry, String>>>,
    pub display_refresh_rate: Option<Rc<dyn Fn() -> Option<f64>>>,
}

pub struct AndroidDiagnostics {
    session: Box<dyn DiagnosticsSession>,
}

impl AndroidDiagnostics {
    pub fn new(
        config: DevToolsConfiguration,
        tree: Rc<RefCell<NodeTree>>,
        graph: Rc<ViewGraph>,
        schedule: Rc<dyn Fn()>,
        invalidate: Rc<dyn Fn()>,
        memory: Rc<dyn Fn() -> DiagnosticMemorySnapshot>,
        hooks: AndroidDiagnosticsHooks,
    ) -> Result<Self, String> {
        let session = if let Some(factory) = take_factory() {
            let callbacks = DiagnosticsCallbacks {
                capture: Some(hooks.capture),
                viewport: hooks.viewport,
                input: hooks.input,
                invalidate: {
                    let invalidate = invalidate.clone();
                    Rc::new(move || invalidate())
                },
                memory: memory.clone(),
                invalidation_log: Some(hooks.invalidation_log),
                // A surface fills the screen; the system owns its size.
                resize_window: None,
                environment: hooks.environment,
                content_geometry: hooks.content_geometry,
                display_refresh_rate: hooks.display_refresh_rate,
            };
            factory(config, tree, graph, schedule, callbacks)?
        } else {
            // DevTools is not linked into this build of `cove-hosts`; keep the
            // memory callback installable for a later backend.
            let _ = (config, tree, graph, invalidate, memory, hooks);
            Box::new(NullDiagnosticsSession) as Box<dyn DiagnosticsSession>
        };
        Ok(Self { session })
    }

    pub fn drain(&mut self) {
        self.session.drain();
    }

    pub fn overlay(&mut self, list: &mut DrawList) {
        self.session.overlay(list);
    }

    pub fn begin_frame(&mut self) {
        self.session.begin_frame();
    }

    pub fn prepared(&mut self, layout: f64, draw: f64) {
        self.session.prepared(layout, draw);
    }

    pub fn layers_composited(&mut self, census: &cove_core::scene::LayerCensus) {
        self.session.layers_composited(census);
    }

    pub fn submitted(&mut self, nodes: usize, batches: usize, token: u64) {
        self.session.frame_finished(nodes, batches, Some(token));
    }

    pub fn presentation_finished(
        &mut self,
        token: u64,
        presented: bool,
        render_thread_ms: f64,
        gpu: Option<cove_graphics::rhi::GpuFrameTiming>,
    ) {
        self.session
            .presentation_finished(token, presented, render_thread_ms, gpu);
    }

    pub fn stop(&mut self) {
        self.session.stop();
    }
}
