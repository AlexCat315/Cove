//! Native attachment of the shared DevTools runtime probe.
use super::integration::{
    DevToolsConfiguration, DiagnosticMemorySnapshot, DiagnosticsCallbacks, DiagnosticsFactory,
    DiagnosticsSession,
};
use cove_composition::ViewGraph;
use cove_devtools::frame_analysis::{FrameWindow, Timeline};
use cove_devtools::inspection::{DiagnosticMemorySnapshot as Memory, DiagnosticStateSource};
use cove_devtools::probe_transport::{PrepareCapture, ProbeTransport};
use cove_devtools::protocol::{
    TimingFrame,
    control::{Fault, Registration, VERSION, Viewport},
};
use cove_devtools::runtime_capture::{self, FramePixels};
use cove_devtools::runtime_probe::RuntimeProbe;
use cove_devtools_cli::AttachedController;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::{Rc, Weak};
use std::time::Instant;

/// The port `cove-devtools serve` uses, so an embedded controller is reachable
/// at the address the CLI and MCP already expect.
const CONTROLLER_PORT: u16 = 9229;

thread_local! {
    static CONTROLLER: RefCell<Weak<AttachedController>> = const { RefCell::new(Weak::new()) };
}

/// The controller every window in this process shares. It lives as long as the
/// last diagnostics session, so stopping the app releases the discovery file.
fn controller() -> Result<Rc<AttachedController>, String> {
    CONTROLLER.with(|slot| {
        if let Some(controller) = slot.borrow().upgrade() {
            return Ok(controller);
        }
        let controller = Rc::new(
            AttachedController::attach(CONTROLLER_PORT).map_err(|error| error.to_string())?,
        );
        if controller.is_embedded() {
            eprintln!(
                "Inspector: {}",
                cove_devtools_cli::inspector_url(controller.connection())
            );
        }
        *slot.borrow_mut() = Rc::downgrade(&controller);
        Ok(controller)
    })
}

struct Session {
    probe: RuntimeProbe,
    transport: ProbeTransport,
    capture: Option<Box<PrepareCapture>>,
    frame_start: Instant,
    layout_ms: f64,
    draw_ms: f64,
    pending: BTreeMap<u64, (TimingFrame, cove_core::scene::LayerCensus)>,
    /// The tree's CPU timeline, and the span window closed for each frame.
    timeline: Timeline,
    window: FrameWindow,
    /// Layers the renderer composited for the frame currently being prepared.
    census: Option<cove_core::scene::LayerCensus>,
    /// Keeps an in-process controller alive for as long as this session.
    _controller: Option<Rc<AttachedController>>,
}

impl DiagnosticsSession for Session {
    fn drain(&mut self) {
        self.transport
            .pump(&mut self.probe, self.capture.as_deref());
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

    fn layers_composited(&mut self, census: &cove_core::scene::LayerCensus) {
        self.census = Some(census.clone());
    }

    fn frame_finished(&mut self, nodes: usize, batches: usize, token: Option<u64>) {
        let total_ms = self.frame_start.elapsed().as_secs_f64() * 1000.0;
        // Recomposition commits before the host begins the frame, so the window
        // spans everything since the previous frame closed.
        let attribution = self.window.close(&self.timeline);
        let mut frame = TimingFrame {
            gpu_breakdown: None,
            frame: 0,
            recomposition_ms: None,
            layout_ms: self.layout_ms,
            draw_ms: self.draw_ms,
            render_thread_ms: None,
            gpu_ms: None,
            pipeline_creation: None,
            submitted: token.is_some(),
            presented: token.is_none().then_some(false),
            total_ms,
            node_count: nodes,
            batch_count: batches,
            timeline_start: 0,
            timeline_end: 0,
        };
        attribution.apply(&mut frame);
        let census = self.census.take().unwrap_or_default();
        if let Some(token) = token {
            if self.pending.len() >= 16
                && let Some((_, (expired, _))) = self.pending.pop_first()
            {
                self.probe.record(expired);
            }
            self.pending.insert(token, (frame, census));
        } else {
            self.probe.record(frame);
        }
    }

    fn presentation_finished(
        &mut self,
        token: u64,
        presented: bool,
        render_thread_ms: f64,
        gpu: Option<cove_graphics::rhi::GpuFrameTiming>,
    ) {
        if let Some((mut frame, census)) = self.pending.remove(&token) {
            frame.presented = Some(presented);
            frame.render_thread_ms = Some(render_thread_ms);
            if let Some(gpu) = &gpu {
                let (count, nanos) = gpu.pipeline_creation();
                frame.pipeline_creation = Some(cove_devtools::protocol::PipelineCreation {
                    count,
                    cpu_ms: nanos as f64 / 1_000_000.0,
                });
                let passes = gpu.pass_reports();
                if !passes.is_empty() || !census.layers.is_empty() || !census.skipped.is_empty() {
                    frame.gpu_breakdown = Some(cove_devtools::gpu_breakdown::assemble(
                        &passes,
                        gpu.pass_count(),
                        &census,
                    ));
                }
            }
            self.probe.record_measured(frame, gpu);
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
            // An explicitly paired runtime (Android's launch extra) keeps its own
            // connection; otherwise this process supplies the controller.
            let controller = match configuration.connection {
                Some(_) => None,
                None => Some(controller()?),
            };
            let connection = configuration.connection.clone().or_else(|| {
                controller.as_ref().map(|controller| {
                    let info = controller.connection();
                    (info.endpoint.clone(), info.token.clone())
                })
            });
            let timeline: Timeline = tree.borrow().timeline.clone();
            let window = FrameWindow::new(&timeline);
            let mut probe = RuntimeProbe::new(
                tree,
                callbacks.invalidation_log,
                Some(graph.render_tree().clone()),
                Some(Rc::new(GraphState(graph))),
            );
            probe.input = Some(callbacks.input);
            probe.viewport = Some(Rc::new(move || {
                let (width, height) = (callbacks.viewport)();
                Viewport { width, height }
            }));
            let capture = callbacks.capture.map(|request| -> Box<PrepareCapture> {
                Box::new(move || {
                    let reply =
                        request().map_err(|message| Fault::new("capture_failed", message))?;
                    Ok(Box::new(move || {
                        let frame = reply
                            .recv_timeout(std::time::Duration::from_secs(3))
                            .map_err(|error| Fault::new("capture_failed", error.to_string()))?
                            .map_err(|error| Fault::new("capture_failed", error.to_string()))?;
                        let viewport = Viewport {
                            width: frame.logical_size.0,
                            height: frame.logical_size.1,
                        };
                        let capture = runtime_capture::encode(
                            FramePixels {
                                width: frame.width,
                                height: frame.height,
                                rgba: frame.rgba,
                            },
                            viewport,
                        )?;
                        serde_json::to_value(capture)
                            .map_err(|error| Fault::new("capture_failed", error.to_string()))
                    }))
                })
            });
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
            if let Some(geometry) = callbacks.content_geometry {
                probe.content_geometry = Some(Rc::new(move || {
                    let geometry = geometry()?;
                    Ok(cove_devtools::protocol::ContentGeometry {
                        origin_x: geometry.origin.0,
                        origin_y: geometry.origin.1,
                        scale: geometry.scale,
                        width: geometry.size.0,
                        height: geometry.size.1,
                    })
                }));
            }
            if let Some(refresh_rate) = callbacks.display_refresh_rate {
                probe.display_refresh_rate = Some(refresh_rate);
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
                    pressure: cove_devtools::inspection::MemoryPressure {
                        subscribed: snapshot.pressure.subscribed,
                        low: snapshot.pressure.low,
                    },
                }
            }));
            *probe.scene.editor.on_change.borrow_mut() = Some(callbacks.invalidate);
            // DevTools extensions register into this shared store; the Inspector
            // lists and toggles them. Packages add panels through the host API; a
            // built-in extension proves the framework end-to-end without a package.
            probe.diagnostics().set_extension_source(Rc::new(
                cove_devtools::inspection::builtin_extension_registry(),
            ));
            let mut capabilities = probe.capabilities();
            if capture.is_some() {
                capabilities.push("capture".into());
            }
            let transport = ProbeTransport::start(
                Registration {
                    version: VERSION,
                    title: configuration.app_title,
                    platform: std::env::consts::OS.to_owned(),
                    pid: Some(std::process::id()),
                    window: configuration.window,
                    capabilities,
                },
                connection,
            )?;
            Ok(Box::new(Session {
                probe,
                transport,
                capture,
                frame_start: Instant::now(),
                layout_ms: 0.0,
                draw_ms: 0.0,
                pending: BTreeMap::new(),
                census: None,
                timeline,
                window,
                _controller: controller,
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
