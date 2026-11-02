//! Translation of Android/Integration/Diagnostics.swift.

use crate::desktop::diagnostics::integration::{
    DevToolsConfiguration, DiagnosticMemorySnapshot, DiagnosticsCallbacks, DiagnosticsFrameSource,
    DiagnosticsSession, NullDiagnosticsSession, take_factory,
};
use cove_composition::ViewGraph;
use cove_core::DrawList;
use cove_core::scene::{InvalidationLog, NodeTree};
use cove_platform::InputEvent;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

/// The socket queue only enqueues work; it never reads the scene or renderer.
/// The Android host delivers all scene access on the UI thread, so this inbox
/// is a UI-thread queue rather than a locked cross-thread mailbox.
#[derive(Default)]
pub struct DiagnosticsInbox {
    work: RefCell<Vec<Box<dyn FnOnce()>>>,
    schedule: Option<Rc<dyn Fn()>>,
}

impl DiagnosticsInbox {
    pub fn new(schedule: Rc<dyn Fn()>) -> Self {
        Self {
            work: RefCell::new(Vec::new()),
            schedule: Some(schedule),
        }
    }

    pub fn enqueue(&self, operation: Box<dyn FnOnce()>) {
        let wake = {
            let mut work = self.work.borrow_mut();
            work.push(operation);
            work.len() == 1
        };
        if wake && let Some(schedule) = &self.schedule {
            schedule();
        }
    }

    pub fn take(&self) -> Vec<Box<dyn FnOnce()>> {
        std::mem::take(&mut *self.work.borrow_mut())
    }
}

/// What the scene hands the session beyond its tree: the mirror's renderer and
/// the hooks that keep frames flowing and deliver remote input.
pub struct AndroidDiagnosticsHooks {
    pub invalidation_log: Arc<InvalidationLog>,
    pub frame_source: Option<DiagnosticsFrameSource>,
    pub mirror_active: Rc<dyn Fn(bool)>,
    pub inject: Rc<dyn Fn(InputEvent)>,
}

pub struct AndroidDiagnostics {
    inbox: DiagnosticsInbox,
    session: Box<dyn DiagnosticsSession>,
    frame_start: Instant,
    layout_ms: f64,
    draw_ms: f64,
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
        let inbox = DiagnosticsInbox::new(schedule.clone());
        let session = if let Some(factory) = take_factory() {
            let callbacks = DiagnosticsCallbacks {
                invalidate: {
                    let invalidate = invalidate.clone();
                    Rc::new(move || invalidate())
                },
                memory: memory.clone(),
                invalidation_log: Some(hooks.invalidation_log),
                frame_source: hooks.frame_source,
                mirror_active: hooks.mirror_active,
                // A surface fills the screen; the system owns its size.
                resize_window: None,
            };
            let mut session = factory(config, tree, graph, schedule, callbacks)?;
            session.attach_input(hooks.inject);
            session
        } else {
            // DevTools is not linked into this build of `cove-hosts`; keep the
            // memory callback installable for a later backend.
            let _ = (config, tree, graph, invalidate, memory, hooks);
            Box::new(NullDiagnosticsSession) as Box<dyn DiagnosticsSession>
        };
        Ok(Self {
            inbox,
            session,
            frame_start: Instant::now(),
            layout_ms: 0.0,
            draw_ms: 0.0,
        })
    }

    pub fn drain(&mut self) {
        for operation in self.inbox.take() {
            operation();
        }
        self.session.drain();
    }

    pub fn intercept(&mut self, event: &InputEvent) -> bool {
        self.session.intercept(event)
    }

    pub fn overlay(&mut self, list: &mut DrawList) {
        self.session.overlay(list);
    }

    pub fn begin_frame(&mut self) {
        self.frame_start = Instant::now();
        self.session.begin_frame();
    }

    pub fn prepared(&mut self, layout: f64, draw: f64) {
        self.layout_ms = layout;
        self.draw_ms = draw;
        self.session.prepared(layout, draw);
    }

    pub fn presented(&mut self, tree: &mut NodeTree, list: &DrawList) {
        let total = self.frame_start.elapsed().as_secs_f64() * 1000.0;
        self.session.presented(tree, list, true);
        let _ = total;
    }

    pub fn wants_mirror_frame(&self) -> bool {
        self.session.mirror_wants_frame()
    }

    pub fn mirror_capture(&mut self, list: &DrawList, pixels: (u32, u32), logical: (f32, f32)) {
        self.session.mirror_capture(list, pixels, logical);
    }

    pub fn stop(&mut self) {
        self.session.stop();
    }
}
