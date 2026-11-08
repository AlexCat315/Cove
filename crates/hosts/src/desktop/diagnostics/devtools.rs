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
use cove_devtools::devtools::{DevTools, NativeAppFrameCapture};
use cove_devtools::inspection::DiagnosticStateSource;
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
        self.devtools
            .set_input_delivery(Some(Rc::new(move |event| inject(event))));
    }

    fn mirror_is_active(&self) -> bool {
        self.devtools.mirror_is_active()
    }

    fn mirror_wants_frame(&self) -> bool {
        self.devtools.mirror_wants_frame()
    }

    fn mirror_capture(&mut self, list: &DrawList, pixels: (u32, u32), logical: (f32, f32)) {
        self.devtools
            .mirror_capture(list, pixels.0, pixels.1, logical.0, logical.1);
    }

    fn stop(&mut self) {
        self.devtools.stop();
    }
}

/// Reads the live view graph, whose registry changes as views mount.
struct GraphStateSource(Rc<ViewGraph>);

impl DiagnosticStateSource for GraphStateSource {
    fn descriptors(&self) -> Vec<cove_core::scene::StateDescriptor> {
        self.0.state_descriptors()
    }
    fn values(&self, ids: &[String]) -> Vec<cove_core::scene::StateValue> {
        self.0.state_values(ids)
    }
}

/// The diagnostics backend a DevTools-enabled application installs on its
/// `ApplicationLaunch`, or in `set_diagnostics_factory` on Android.
pub fn factory() -> DiagnosticsFactory {
    Rc::new(
        |configuration: DevToolsConfiguration,
         tree: Rc<RefCell<NodeTree>>,
         graph: Rc<ViewGraph>,
         _schedule: Rc<dyn Fn()>,
         callbacks: DiagnosticsCallbacks|
         -> Result<Box<dyn DiagnosticsSession>, String> {
            let mut config = DevToolsConfig::from_environment(&configuration.app_title)
                .ok_or_else(|| "DevTools is only available in debug builds".to_string())?;
            if configuration.port != 0 {
                config.port = configuration.port;
            }
            let render_tree = graph.render_tree().clone();
            let mut devtools = DevTools::new(
                config,
                tree,
                callbacks.invalidation_log.clone(),
                Some(render_tree),
            );
            devtools.set_state_source(Some(Rc::new(GraphStateSource(graph))));

            if let Some(source) = callbacks.frame_source {
                let capture = Rc::new(
                    NativeAppFrameCapture::with_device(source.device, &source.renderer)
                        .map_err(|error| error.to_string())?,
                );
                let reset = capture.clone();
                devtools.attach_frame_tap(capture.frame_capture(), move || reset.reset());
            }
            let mirror_active = callbacks.mirror_active.clone();
            *devtools.on_mirror_start.borrow_mut() = Some(Rc::new(move || mirror_active(true)));
            let mirror_active = callbacks.mirror_active.clone();
            *devtools.on_mirror_stop.borrow_mut() = Some(Rc::new(move || mirror_active(false)));
            if let Some(resize) = callbacks.resize_window.clone() {
                devtools.window.resize = Some(Rc::new(move |size| {
                    resize(size.width as f32, size.height as f32)
                }));
            }
            if let Some(apply) = callbacks.environment.clone() {
                devtools.window.environment = Some(Rc::new(move |payload: &serde_json::Value| {
                    let request: cove_core::EnvironmentRequest =
                        serde_json::from_value(payload.clone())
                            .map_err(|error| format!("invalid environment request: {error}"))?;
                    apply(&request).map(|environment| environment_summary(&environment))
                }));
            }

            let memory = callbacks.memory.clone();
            devtools.set_memory_snapshot(Some(Rc::new(move || {
                let snapshot: DiagnosticMemorySnapshot = memory();
                cove_devtools::inspection::DiagnosticMemorySnapshot {
                    counters: snapshot
                        .counters
                        .iter()
                        .map(|(key, value)| (key.clone(), *value as i64))
                        .collect(),
                    windows: snapshot
                        .windows
                        .iter()
                        .map(|(key, value)| {
                            (
                                key.clone(),
                                BTreeMap::from([("bytes".to_owned(), *value as i64)]),
                            )
                        })
                        .collect(),
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

/// What a caller needs to verify an environment change without a capture.
pub fn environment_summary(environment: &cove_core::Environment) -> serde_json::Value {
    serde_json::json!({
        "platform": environment.platform,
        "designLanguage": environment.design_language,
        "deviceClass": environment.device_class,
        "posture": environment.posture,
        "input": {
            "touch": environment.input.contains(cove_core::InputCapabilities::TOUCH),
            "pointer": environment.input.contains(cove_core::InputCapabilities::POINTER),
            "keyboard": environment.input.contains(cove_core::InputCapabilities::KEYBOARD),
        },
        "viewport": {
            "width": environment.viewport.width,
            "height": environment.viewport.height,
            "widthClass": environment.width_class(),
            "heightClass": environment.height_class(),
        },
        "safeArea": environment.safe_area,
        "displayFeatures": environment.display_features,
        "segments": environment.segments(),
    })
}
