//! Native attachment of the shared DevTools runtime probe.
use super::integration::{
    DevToolsConfiguration, DiagnosticMemorySnapshot, DiagnosticsCallbacks, DiagnosticsFactory,
    DiagnosticsSession,
};
use cove_composition::ViewGraph;
use cove_devtools::inspection::{DiagnosticMemorySnapshot as Memory, DiagnosticStateSource};
use cove_devtools::probe_transport::ProbeTransport;
use cove_devtools::protocol::{
    TimingFrame,
    control::{Registration, VERSION},
};
use cove_devtools::runtime_probe::RuntimeProbe;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Instant;

struct Session {
    probe: RuntimeProbe,
    transport: ProbeTransport,
    frame_start: Instant,
    layout_ms: f64,
    draw_ms: f64,
    pending: BTreeMap<u64, TimingFrame>,
}

impl DiagnosticsSession for Session {
    fn drain(&mut self) {
        self.transport.pump(&mut self.probe);
    }

    fn overlay(&mut self, list: &mut cove_core::DrawList) {
        self.probe.scene.editor.draw_overlay(list);
    }

    fn begin_frame(&mut self) {
        self.frame_start = Instant::now();
    }

    fn prepared(&mut self, layout_ms: f64, draw_ms: f64) {
        self.layout_ms = layout_ms;
        self.draw_ms = draw_ms;
    }

    fn frame_finished(&mut self, nodes: usize, batches: usize, token: Option<u64>) {
        let total_ms = self.frame_start.elapsed().as_secs_f64() * 1000.0;
        let frame = TimingFrame {
            frame: 0,
            recomposition_ms: None,
            layout_ms: self.layout_ms,
            draw_ms: self.draw_ms,
            render_thread_ms: None,
            submitted: token.is_some(),
            presented: token.is_none().then_some(false),
            total_ms,
            node_count: nodes,
            batch_count: batches,
        };
        if let Some(token) = token {
            if self.pending.len() >= 16
                && let Some((_, expired)) = self.pending.pop_first()
            {
                self.probe.record(expired);
            }
            self.pending.insert(token, frame);
        } else {
            self.probe.record(frame);
        }
    }

    fn presentation_finished(&mut self, token: u64, presented: bool, render_thread_ms: f64) {
        if let Some(mut frame) = self.pending.remove(&token) {
            frame.presented = Some(presented);
            frame.render_thread_ms = Some(render_thread_ms);
            self.probe.record(frame);
        }
    }

    fn stop(&mut self) {
        self.probe.disconnect();
    }
}

struct GraphState(Rc<ViewGraph>);

impl DiagnosticStateSource for GraphState {
    fn descriptors(&self) -> Vec<cove_core::scene::StateDescriptor> {
        self.0.state_descriptors()
    }

    fn values(&self, ids: &[String]) -> Vec<cove_core::scene::StateValue> {
        self.0.state_values(ids)
    }
}

pub fn factory() -> DiagnosticsFactory {
    Rc::new(
        |configuration: DevToolsConfiguration, tree, graph, _, callbacks: DiagnosticsCallbacks| {
            if !cfg!(debug_assertions) {
                return Err("The DevTools runtime probe requires a debug build.".into());
            }
            let mut probe = RuntimeProbe::new(
                tree,
                callbacks.invalidation_log,
                Some(graph.render_tree().clone()),
                Some(Rc::new(GraphState(graph))),
            );
            if let Some(resize) = callbacks.resize_window {
                probe.resize = Some(Rc::new(move |width, height| {
                    match resize(width as f32, height as f32) {
                        Some(error) => Err(error),
                        None => Ok(()),
                    }
                }));
            }
            if let Some(apply) = callbacks.environment {
                probe.environment = Some(Rc::new(move |value| {
                    let request = serde_json::from_value(value)
                        .map_err(|error| format!("Invalid environment: {error}"))?;
                    apply(&request).map(|environment| environment_summary(&environment))
                }));
            }
            *probe.diagnostics().memory.borrow_mut() = Some(Rc::new(move || {
                let snapshot: DiagnosticMemorySnapshot = (callbacks.memory)();
                Memory {
                    counters: snapshot
                        .counters
                        .into_iter()
                        .map(|(key, value)| (key, value as i64))
                        .collect(),
                    windows: snapshot
                        .windows
                        .into_iter()
                        .map(|(key, value)| {
                            (key, BTreeMap::from([("bytes".to_owned(), value as i64)]))
                        })
                        .collect(),
                }
            }));
            *probe.scene.editor.on_change.borrow_mut() = Some(callbacks.invalidate);
            let transport = ProbeTransport::start(
                Registration {
                    version: VERSION,
                    title: configuration.app_title,
                    platform: std::env::consts::OS.to_owned(),
                    pid: Some(std::process::id()),
                    window: configuration.window,
                    capabilities: probe.capabilities(),
                },
                configuration.connection,
            )?;
            Ok(Box::new(Session {
                probe,
                transport,
                frame_start: Instant::now(),
                layout_ms: 0.0,
                draw_ms: 0.0,
                pending: BTreeMap::new(),
            }))
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
