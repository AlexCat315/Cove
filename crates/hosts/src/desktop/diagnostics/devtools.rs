//! `DiagnosticsSession` adapter for the DevTools crate.
//!
//! `cove-hosts` links DevTools only when the `devtools` feature is on; the
//! adapter is host code, so applications install it with
//! [`factory`] instead of re-implementing the session themselves.

use super::integration::{
    DevToolsConfiguration, DiagnosticMemorySnapshot, DiagnosticsCallbacks, DiagnosticsFactory,
    DiagnosticsSession,
};
use cove_composition::ViewGraph;
use cove_core::DrawList;
use cove_core::scene::NodeTree;
use cove_devtools::devtools::DevTools;
use cove_devtools::server::config::DevToolsConfig;
use cove_platform::InputEvent;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Instant;

struct DevToolsSession {
    devtools: DevTools,
    frame_start: Instant,
    layout_ms: f64,
    draw_ms: f64,
}

impl DiagnosticsSession for DevToolsSession {
    fn drain(&mut self) {
        self.devtools.pump();
        self.devtools.notify_tree_changed();
        self.devtools.notify_frame_finished();
    }

    fn intercept(&mut self, event: &InputEvent) -> bool {
        self.devtools.intercept_input(event)
    }

    fn overlay(&mut self, list: &mut DrawList) {
        self.devtools.draw_inspection_overlay(list);
    }

    fn begin_frame(&mut self) {
        self.frame_start = Instant::now();
    }

    fn prepared(&mut self, layout_ms: f64, draw_ms: f64) {
        self.layout_ms = layout_ms;
        self.draw_ms = draw_ms;
    }

    fn presented(&mut self, tree: &mut NodeTree, list: &DrawList, presented: bool) {
        let total_ms = self.frame_start.elapsed().as_secs_f64() * 1000.0;
        // The host reports the present stage only to its fps logger, so the
        // remainder of the frame is the presentation cost here.
        let present_ms = (total_ms - self.layout_ms - self.draw_ms).max(0.0);
        self.devtools.timing.record(
            self.layout_ms,
            self.draw_ms,
            present_ms,
            total_ms,
            tree.node_count(),
            list.batches().len(),
            presented,
        );
    }

    fn attach_input(&mut self, inject: Rc<dyn Fn(InputEvent)>) {
        *self.devtools.input_delivery.borrow_mut() = Some(Rc::new(move |event| inject(event)));
    }

    fn stop(&mut self) {
        self.devtools.stop();
    }
}

/// The diagnostics backend a DevTools-enabled application installs on its
/// `ApplicationLaunch`, or in `set_diagnostics_factory` on Android.
pub fn factory() -> DiagnosticsFactory {
    Rc::new(
        |configuration: DevToolsConfiguration,
         tree: Rc<RefCell<NodeTree>>,
         _graph: Rc<ViewGraph>,
         _schedule: Rc<dyn Fn()>,
         callbacks: DiagnosticsCallbacks|
         -> Result<Box<dyn DiagnosticsSession>, String> {
            let mut config = DevToolsConfig::from_environment(&configuration.app_title)
                .ok_or_else(|| "DevTools is only available in debug builds".to_string())?;
            if configuration.port != 0 {
                config.port = configuration.port;
            }
            let mut devtools = DevTools::new(config, tree, None, None);

            let memory = callbacks.memory.clone();
            devtools.set_memory_snapshot(Some(Rc::new(move || {
                let snapshot: DiagnosticMemorySnapshot = memory();
                cove_devtools::inspection::DiagnosticMemorySnapshot {
                    counters: snapshot
                        .counters
                        .iter()
                        .map(|(key, value)| (key.clone(), *value as i64))
                        .collect(),
                    windows: BTreeMap::new(),
                }
            })));

            // A remote selection only reaches the draw list on the next frame.
            let invalidate = callbacks.invalidate.clone();
            *devtools.on_selection_changed.borrow_mut() = Some(Rc::new(move || invalidate()));

            devtools.start().map_err(|error| error.to_string())?;
            Ok(Box::new(DevToolsSession {
                devtools,
                frame_start: Instant::now(),
                layout_ms: 0.0,
                draw_ms: 0.0,
            }) as Box<dyn DiagnosticsSession>)
        },
    )
}
