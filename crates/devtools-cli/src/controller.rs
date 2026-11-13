use crate::debugger::{ControlAction, DebuggerManager};
use crate::registry::{Registry, random_id};
use base64::{Engine, engine::general_purpose::STANDARD};
use cove_agent::{AgentTool, JsonSchemaValidator, ui_tools::Tool};
use cove_debugger::{DapSession, SessionSnapshot};
use cove_devtools_protocol::control::{
    Fault, MAX_CAPTURE_BYTES, ProbeCommand, RuntimeCapture, RuntimeInput, Viewport,
};
use cove_devtools_protocol::{MAX_TREE_PAGE_DEPTH, MAX_TREE_PAGE_NODES, NodeFrame, TreePage};
use image::{ImageDecoder, ImageFormat, ImageReader, Limits, RgbaImage, imageops};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct Frame {
    id: String,
    target: String,
    mapping: CaptureMapping,
    png: Arc<Vec<u8>>,
    time: Instant,
}

pub struct Controller {
    pub registry: Registry,
    tools: Vec<AgentTool>,
    frames: Mutex<VecDeque<Frame>>,
    debugger: DebuggerManager,
}

fn invalid(message: impl Into<String>) -> Fault {
    Fault::new("invalid_request", message)
}

/// `query_ui_tree` defaults: broad enough to orient in an unfamiliar screen,
/// small enough that the first read stays far inside the agent result budget.
const DEFAULT_PAGE_DEPTH: usize = 4;
const DEFAULT_PAGE_LIMIT: usize = 500;
/// Allocation sites one query returns when the caller gives no limit.
const DEFAULT_ALLOCATION_SITES: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
struct PixelRegion {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

/// Coordinates in a returned image are pixel-relative, including for node crops.
#[derive(Clone, Copy, Debug)]
struct CaptureMapping {
    viewport: Viewport,
    image: (u32, u32),
    region: PixelRegion,
}

impl CaptureMapping {
    fn full(viewport: Viewport, image: (u32, u32)) -> Self {
        Self {
            viewport,
            image,
            region: PixelRegion {
                x: 0,
                y: 0,
                width: image.0,
                height: image.1,
            },
        }
    }

    fn content_point(self, x: f64, y: f64) -> Result<(f32, f32), Fault> {
        if !(0.0..f64::from(self.region.width)).contains(&x)
            || !(0.0..f64::from(self.region.height)).contains(&y)
        {
            return Err(invalid("The point is outside the captured image."));
        }
        let map = |value: f64, offset, pixels, logical| {
            ((value.floor() + 0.5 + f64::from(offset)) / f64::from(pixels) * f64::from(logical))
                as f32
        };
        Ok((
            map(x, self.region.x, self.image.0, self.viewport.width),
            map(y, self.region.y, self.image.1, self.viewport.height),
        ))
    }

    fn crop(self, frame: &NodeFrame) -> Option<PixelRegion> {
        if ![frame.x, frame.y, frame.w, frame.h]
            .into_iter()
            .all(f64::is_finite)
            || frame.w <= 0.0
            || frame.h <= 0.0
        {
            return None;
        }
        let sx = f64::from(self.image.0) / f64::from(self.viewport.width);
        let sy = f64::from(self.image.1) / f64::from(self.viewport.height);
        let bounds = |start: f64, length: f64, scale: f64, pixels: u32| {
            (
                (start * scale).floor().clamp(0.0, f64::from(pixels)) as u32,
                ((start + length) * scale)
                    .ceil()
                    .clamp(0.0, f64::from(pixels)) as u32,
            )
        };
        let (x, right) = bounds(frame.x, frame.w, sx, self.image.0);
        let (y, bottom) = bounds(frame.y, frame.h, sy, self.image.1);
        (right > x && bottom > y).then(|| PixelRegion {
            x,
            y,
            width: right - x,
            height: bottom - y,
        })
    }
}

fn node_absolute_frame(tree: TreePage, node_id: &str) -> Result<NodeFrame, Fault> {
    tree.root
        .filter(|root| root.id == node_id)
        .map(|root| root.absolute_frame)
        .ok_or_else(|| {
            Fault::new(
                "stale_node",
                "The selected component is no longer in the scene.",
            )
        })
}

impl Controller {
    #[cfg(test)]
    fn with_debugger(debugger: DebuggerManager) -> Self {
        let mut controller = Self::new();
        controller.debugger = debugger;
        controller
    }

    pub fn new() -> Self {
        Self {
            registry: Registry::default(),
            tools: crate::tools::definitions(),
            frames: Mutex::new(VecDeque::new()),
            debugger: DebuggerManager::new(),
        }
    }

    fn runtime(&self, target: Option<&str>) -> Result<String, Fault> {
        let registrations = self.registry.registrations();
        match target {
            Some(id) if registrations.iter().any(|(current, _)| current == id) => Ok(id.to_owned()),
            Some(_) => Err(Fault::new(
                "target_gone",
                "Select a connected Cove runtime target.",
            )),
            None if registrations.len() == 1 => Ok(registrations[0].0.clone()),
            None => Err(Fault::new(
                "target_required",
                "Use get_ui_session to select a runtime target.",
            )),
        }
    }

    fn probe(&self, id: Option<&str>, command: ProbeCommand) -> Result<Value, Fault> {
        self.registry.invoke(&self.runtime(id)?, command)
    }

    /// Profiling commands only reach a runtime that advertises its sampler.
    fn profile(&self, id: Option<&str>, command: ProbeCommand) -> Result<Value, Fault> {
        let runtime = self.runtime(id)?;
        self.require_capability(&runtime, "cpu_profile")?;
        self.registry.invoke(&runtime, command)
    }

    /// Allocation-site commands only reach a runtime that can capture stacks.
    fn allocation_sites(&self, id: Option<&str>, command: ProbeCommand) -> Result<Value, Fault> {
        let runtime = self.runtime(id)?;
        self.require_capability(&runtime, "allocation_sites")?;
        self.registry.invoke(&runtime, command)
    }

    /// Resolves a runtime to the local executable App Size measures.
    ///
    /// The request never carries a path: only the registered runtime's PID can
    /// select a file. A display, window or browser session has no runtime
    /// executable and is refused explicitly, as is a runtime on another host.
    fn app_size_executable(
        &self,
        id: Option<&str>,
    ) -> Result<(String, u32, std::path::PathBuf), Fault> {
        let runtime = self.runtime(id)?;
        let registration = self
            .registry
            .registrations()
            .into_iter()
            .find(|(current, _)| *current == runtime)
            .map(|(_, registration)| registration)
            .ok_or_else(|| Fault::new("target_gone", "The runtime disconnected."))?;
        if registration.platform != std::env::consts::OS
            || !matches!(
                registration.platform.as_str(),
                "macos" | "windows" | "linux"
            )
        {
            return Err(Fault::new(
                "unavailable",
                format!(
                    "App Size reads the executable of a local desktop runtime; the selected {} runtime's process is not on this host.",
                    registration.platform
                ),
            ));
        }
        let pid = registration.pid.ok_or_else(|| {
            Fault::new(
                "unavailable",
                "The selected runtime reports no operating-system process ID, so its executable cannot be resolved.",
            )
        })?;
        let executable = crate::app_size::executable(pid)?;
        Ok((runtime, pid, executable))
    }

    /// Resolves a runtime to the operating-system process lldb-dap attaches to.
    fn debug_target(&self, id: Option<&str>) -> Result<(String, u32), Fault> {
        let runtime = self.runtime(id)?;
        let registration = self
            .registry
            .registrations()
            .into_iter()
            .find(|(current, _)| *current == runtime)
            .map(|(_, registration)| registration)
            .ok_or_else(|| Fault::new("target_gone", "The runtime disconnected."))?;
        if registration.platform != std::env::consts::OS
            || !matches!(
                registration.platform.as_str(),
                "macos" | "windows" | "linux"
            )
        {
            return Err(Fault::new(
                "unavailable",
                format!(
                    "lldb-dap attach is limited to desktop runtimes on this machine; the selected {} runtime's process is not local.",
                    registration.platform
                ),
            ));
        }
        let pid = registration.pid.ok_or_else(|| {
            Fault::new(
                "unavailable",
                "The selected runtime reports no operating-system process ID, so lldb-dap cannot attach.",
            )
        })?;
        Ok((runtime, pid))
    }

    fn debug_session(
        &self,
        id: Option<&str>,
    ) -> Result<(String, u32, std::sync::Arc<DapSession>), Fault> {
        let (runtime, pid) = self.debug_target(id)?;
        let session = self.debugger.session(pid).map_err(crate::debugger::fault)?;
        Ok((runtime, pid, session))
    }

    fn debugger_payload(&self, session: Value) -> Value {
        let mut value = self.debugger.availability_json();
        value["session"] = session;
        value
    }

    fn snapshot_value(snapshot: &SessionSnapshot) -> Value {
        serde_json::to_value(snapshot).unwrap_or(Value::Null)
    }

    /// Reads one frame's scopes and their first page of variables.
    fn scope_variables(session: &DapSession, frame_id: u64) -> Result<Value, Fault> {
        let scopes = session.scopes(frame_id).map_err(crate::debugger::fault)?;
        let truncated = scopes.len() > 6;
        let mut values = Vec::new();
        for scope in scopes.iter().take(6) {
            let mut value = json!({
                "name": scope.name,
                "variables_reference": scope.variables_reference,
                "expensive": scope.expensive,
                "presentation_hint": scope.presentation_hint,
                "source": scope.source,
                "line": scope.line,
            });
            match session.variables(scope.variables_reference, 0, 50, None) {
                Ok(page) => {
                    value["variables"] = json!(page.variables);
                    value["total"] = json!(page.total);
                    value["truncated"] = json!(page.truncated);
                }
                Err(error) => {
                    value["variables"] = json!([]);
                    value["error"] = json!({"code": error.code(), "message": error.message()});
                }
            }
            values.push(value);
        }
        Ok(json!({"scopes": values, "scopes_truncated": truncated}))
    }

    pub fn execute(&self, name: &str, mut args: Value) -> Result<Value, Fault> {
        let tool = self
            .tools
            .iter()
            .find(|tool| tool.name == name)
            .ok_or_else(|| Fault::new("unknown_command", format!("Unknown command {name}.")))?;
        JsonSchemaValidator::validate(&args, &tool.schema)
            .map_err(|error| invalid(error.to_string()))?;
        let target = args.as_object_mut().and_then(|args| args.remove("target"));
        let target = target.as_ref().and_then(Value::as_str);
        let result = match name {
            "get_ui_session" => {
                let mut session = json!({"targets": self.registry.list()});
                session["platform"] = json!(std::env::consts::OS);
                session["debugger"] = self.debugger.availability_json();
                session
            }
            "list_skills" => {
                let library = cove_agent::ui_tools::Tools::skills();
                let skills: Vec<_> = library
                    .skills
                    .iter()
                    .map(|skill| json!({"name": skill.name, "description": skill.description}))
                    .collect();
                json!({"ok": true, "data": skills})
            }
            "read_skill" => {
                let library = cove_agent::ui_tools::Tools::skills();
                let skill = library
                    .read(args["name"].as_str().unwrap())
                    .map_err(|error| invalid(error.to_string()))?;
                json!({"ok": true, "data": skill})
            }
            "inspect_ui_tree" => self.probe(target, ProbeCommand::Tree)?,
            "query_ui_tree" => {
                let runtime = self.runtime(target)?;
                self.require_capability(&runtime, "tree_page")?;
                let node_id = args["node_id"].as_str();
                if node_id.is_some_and(|id| id.is_empty() || id.len() > 256) {
                    return Err(invalid(
                        "node_id must be a live element ID of 1 to 256 bytes.",
                    ));
                }
                let depth = args["depth"]
                    .as_u64()
                    .map_or(DEFAULT_PAGE_DEPTH, |depth| depth as usize);
                let limit = args["limit"]
                    .as_u64()
                    .map_or(DEFAULT_PAGE_LIMIT, |limit| limit as usize);
                if !(1..=MAX_TREE_PAGE_DEPTH).contains(&depth) {
                    return Err(invalid(format!(
                        "depth must be an integer from 1 to {MAX_TREE_PAGE_DEPTH}."
                    )));
                }
                if !(1..=MAX_TREE_PAGE_NODES).contains(&limit) {
                    return Err(invalid(format!(
                        "limit must be an integer from 1 to {MAX_TREE_PAGE_NODES}."
                    )));
                }
                self.registry.invoke(
                    &runtime,
                    ProbeCommand::TreePage {
                        node_id: node_id.map(str::to_owned),
                        depth,
                        limit,
                    },
                )?
            }
            "select_ui_node" => self.probe(
                target,
                ProbeCommand::Select {
                    node_id: args["node_id"].as_str().map(str::to_owned),
                },
            )?,
            "select_ui_at" => {
                let runtime = self.runtime(target)?;
                self.require_capability(&runtime, "inspect_nodes")?;
                let capture_id = args["capture_id"].as_str().unwrap();
                let mapping = self.stored_frame(capture_id, &runtime)?;
                let point = mapping
                    .content_point(args["x"].as_f64().unwrap(), args["y"].as_f64().unwrap())?;
                let hit = self.registry.invoke(
                    &runtime,
                    ProbeCommand::NodeAt {
                        x: f64::from(point.0),
                        y: f64::from(point.1),
                    },
                )?;
                json!({
                    "capture_id": capture_id,
                    "contentPoint": {"x": point.0, "y": point.1},
                    "hit": hit,
                })
            }
            "capture_ui" | "capture_ui_node" => {
                let runtime = self.runtime(target)?;
                let node_id = args["node_id"].as_str();
                let (png, mut mapping) = self.capture(&runtime)?;
                if let Some(node_id) = node_id {
                    let tree: TreePage = serde_json::from_value(self.registry.invoke(
                        &runtime,
                        ProbeCommand::TreePage {
                            node_id: Some(node_id.into()),
                            depth: 1,
                            limit: 1,
                        },
                    )?)
                    .map_err(|error| Fault::new("internal", error.to_string()))?;
                    let frame = node_absolute_frame(tree, node_id)?;
                    let crop = mapping.crop(&frame).ok_or_else(|| {
                        Fault::new(
                            "node_not_visible",
                            "The node has no area inside the captured frame.",
                        )
                    })?;
                    let pixels = imageops::crop_imm(
                        &decode_capture(&png)?,
                        crop.x,
                        crop.y,
                        crop.width,
                        crop.height,
                    )
                    .to_image();
                    let mut png = Cursor::new(Vec::new());
                    pixels
                        .write_to(&mut png, ImageFormat::Png)
                        .map_err(|error| Fault::new("capture_failed", error.to_string()))?;
                    mapping.region = crop;
                    let mut result = self.store_capture(runtime, mapping, png.into_inner())?;
                    result["node_id"] = json!(node_id);
                    result["contentFrame"] = json!(frame);
                    result
                } else {
                    self.store_capture(runtime, mapping, png)?
                }
            }
            "input_ui" | "click_ui" | "press_ui_key" | "type_ui_text" => {
                let action = match name {
                    "input_ui" => args["action"].clone(),
                    "click_ui" => {
                        json!({"kind":"click", "x":args["x"], "y":args["y"], "button":args["button"].as_u64().unwrap_or(0)})
                    }
                    "press_ui_key" => {
                        json!({"kind":"key", "key":args["key"], "modifiers":args["modifiers"].as_array().cloned().unwrap_or_default()})
                    }
                    _ => json!({"kind":"text", "text":args["text"]}),
                };
                let runtime = self.runtime(target)?;
                self.require_capability(&runtime, "input")?;
                let mapping = args["capture_id"]
                    .as_str()
                    .map(|id| self.stored_frame(id, &runtime))
                    .transpose()?;
                let action = map_input(action, mapping)?;
                self.registry.invoke(
                    &runtime,
                    ProbeCommand::Input {
                        viewport: mapping.map(|mapping| mapping.viewport),
                        action,
                    },
                )?
            }
            "wait_for_ui" => self.wait_for(target, args)?,
            "resize_ui" => self.probe(
                target,
                ProbeCommand::Resize {
                    width: args["width"].as_f64().unwrap(),
                    height: args["height"].as_f64().unwrap(),
                },
            )?,
            "set_ui_environment" => self.probe(
                target,
                ProbeCommand::Environment(args["environment"].clone()),
            )?,
            "edit_ui_style" => self.probe(
                target,
                ProbeCommand::Edit {
                    name: args["command"].as_str().unwrap().to_owned(),
                    arguments: args["arguments"].clone(),
                },
            )?,
            "set_ui_debug_paint" => {
                let runtime = self.runtime(target)?;
                self.require_capability(&runtime, "debug_paint")?;
                self.registry.invoke(
                    &runtime,
                    ProbeCommand::DebugPaint {
                        guidelines: args["guidelines"].as_bool().unwrap_or(false),
                    },
                )?
            }
            "set_ui_baselines" => {
                let runtime = self.runtime(target)?;
                self.require_capability(&runtime, "baselines")?;
                self.registry.invoke(
                    &runtime,
                    ProbeCommand::Baselines {
                        enabled: args["enabled"].as_bool().unwrap_or(false),
                    },
                )?
            }
            "set_ui_oversized_images" => {
                let runtime = self.runtime(target)?;
                self.require_capability(&runtime, "oversized_images")?;
                self.registry.invoke(
                    &runtime,
                    ProbeCommand::OversizedImages {
                        enabled: args["enabled"].as_bool().unwrap_or(false),
                    },
                )?
            }
            "set_ui_repaint_highlight" => {
                let runtime = self.runtime(target)?;
                self.require_capability(&runtime, "repaint_highlight")?;
                self.registry.invoke(
                    &runtime,
                    ProbeCommand::RepaintHighlight {
                        enabled: args["enabled"].as_bool().unwrap_or(false),
                    },
                )?
            }
            "set_ui_animation_speed" => {
                let runtime = self.runtime(target)?;
                self.require_capability(&runtime, "animation_speed")?;
                let scale = args["scale"]
                    .as_f64()
                    .ok_or_else(|| invalid("The animation speed requires a numeric scale."))?;
                self.registry
                    .invoke(&runtime, ProbeCommand::AnimationScale { scale })?
            }
            "start_ui_trace" => self.probe(target, ProbeCommand::TraceStart)?,
            "stop_ui_trace" => self.probe(target, ProbeCommand::TraceStop)?,
            "capture_ui_trace" => {
                let id = self.runtime(target)?;
                self.registry.invoke(&id, ProbeCommand::TraceStart)?;
                std::thread::sleep(Duration::from_millis(
                    args["duration_ms"].as_u64().unwrap_or(500),
                ));
                self.registry.invoke(&id, ProbeCommand::TraceStop)?
            }
            "get_ui_trace" => self.probe(
                target,
                ProbeCommand::TraceRead {
                    id: args["capture_id"].as_str().unwrap().into(),
                    offset: args["offset"].as_u64().unwrap_or(0) as usize,
                    limit: args["limit"].as_u64().unwrap_or(100) as usize,
                },
            )?,
            "compare_ui_traces" => self.probe(
                target,
                ProbeCommand::TraceCompare {
                    baseline: args["baseline_id"].as_str().unwrap().into(),
                    candidate: args["candidate_id"].as_str().unwrap().into(),
                },
            )?,
            "record_ui_timings" => self.probe(
                target,
                ProbeCommand::Timing {
                    after: args["after"].as_u64().unwrap_or(0),
                },
            )?,
            "analyze_ui_frame" => self.probe(
                target,
                ProbeCommand::AnalyzeUiFrame {
                    // The schema requires an integer; reading it through `f64`
                    // also accepts a client that sends `12.0`.
                    frame: args["frame"].as_f64().unwrap_or(0.0) as u64,
                },
            )?,
            "query_ui_logs" => self.probe(
                target,
                ProbeCommand::Logs {
                    after: args["after"].as_u64(),
                },
            )?,
            "clear_ui_logs" => self.probe(target, ProbeCommand::ClearLogs)?,
            "set_ui_network_recording" => {
                let runtime = self.runtime(target)?;
                self.require_capability(&runtime, "network_control")?;
                let enabled = args["enabled"]
                    .as_bool()
                    .ok_or_else(|| invalid("Network recording requires a boolean enabled flag."))?;
                self.registry
                    .invoke(&runtime, ProbeCommand::NetworkRecording { enabled })?
            }
            "clear_ui_network" => {
                let runtime = self.runtime(target)?;
                self.require_capability(&runtime, "network_control")?;
                self.registry.invoke(&runtime, ProbeCommand::ClearNetwork)?
            }
            "start_ui_profile" => self.profile(
                target,
                ProbeCommand::ProfileStart {
                    hz: args["hz"].as_u64().unwrap_or(0) as u32,
                    duration_ms: args["duration_ms"].as_u64().unwrap_or(0) as u32,
                },
            )?,
            "stop_ui_profile" => self.profile(target, ProbeCommand::ProfileStop)?,
            "get_ui_profile" => self.profile(target, ProbeCommand::ProfileRead)?,
            "query_ui_hotspots" => self.profile(
                target,
                ProbeCommand::ProfileHotspots {
                    limit: args["limit"].as_u64().unwrap_or(20).clamp(1, 200) as usize,
                },
            )?,
            "start_ui_allocation_profile" => self.allocation_sites(
                target,
                ProbeCommand::AllocationStart {
                    rate: args["rate"].as_u64().unwrap_or(0) as u32,
                },
            )?,
            "stop_ui_allocation_profile" => {
                self.allocation_sites(target, ProbeCommand::AllocationStop)?
            }
            "query_ui_allocation_sites" => self.allocation_sites(
                target,
                ProbeCommand::AllocationSites {
                    limit: args["limit"]
                        .as_u64()
                        .map_or(DEFAULT_ALLOCATION_SITES, |limit| limit as usize)
                        .clamp(1, cove_devtools_protocol::MAX_ALLOCATION_SITES),
                },
            )?,
            "get_ui_app_size" => {
                let (runtime, pid, executable) = self.app_size_executable(target)?;
                let limits = crate::app_size::Limits {
                    segments: args["segments"]
                        .as_u64()
                        .map_or(crate::app_size::DEFAULT_LIMIT, |value| value as usize),
                    sections: args["sections"]
                        .as_u64()
                        .map_or(crate::app_size::DEFAULT_LIMIT, |value| value as usize),
                    symbols: args["symbols"]
                        .as_u64()
                        .map_or(crate::app_size::DEFAULT_LIMIT, |value| value as usize),
                    groups: args["groups"]
                        .as_u64()
                        .map_or(crate::app_size::DEFAULT_LIMIT, |value| value as usize),
                };
                let mut value = crate::app_size::analyze(pid, &executable, limits)?;
                value["target"] = json!(runtime);
                value
            }
            "get_ui_deep_links" => {
                let root = args["root"]
                    .as_str()
                    .ok_or_else(|| invalid("root must be a string."))?;
                crate::deep_links::inspect(root)?
            }
            "get_ui_debugger" => {
                let (_, pid) = self.debug_target(target)?;
                let session = self.debugger.summary(pid).unwrap_or(Value::Null);
                self.debugger_payload(session)
            }
            "attach_ui_debugger" => {
                let (runtime, pid) = self.debug_target(target)?;
                let timeout = Duration::from_millis(
                    args["timeout_ms"]
                        .as_u64()
                        .unwrap_or(crate::debugger::DEFAULT_ATTACH_TIMEOUT.as_millis() as u64),
                );
                self.debugger
                    .attach(&runtime, pid, timeout)
                    .map_err(crate::debugger::fault)?;
                let mut payload =
                    self.debugger_payload(self.debugger.summary(pid).unwrap_or(Value::Null));
                payload["attached"] = json!(true);
                payload["pid"] = json!(pid);
                payload["state"] = payload["session"]["state"].clone();
                payload
            }
            "detach_ui_debugger" => {
                let (_, pid) = self.debug_target(target)?;
                let outcome = self.debugger.detach(pid).map_err(crate::debugger::fault)?;
                json!({
                    "pid": pid,
                    "detached": outcome == "detached",
                    "outcome": outcome,
                    "session_removed": true,
                })
            }
            "set_ui_breakpoint" => {
                let (_, pid, session) = self.debug_session(target)?;
                let path = args["path"].as_str().unwrap();
                let line = args["line"].as_f64().unwrap_or(0.0) as u32;
                let condition = args["condition"].as_str();
                let breakpoints = session
                    .set_breakpoint(path, line, condition)
                    .map_err(crate::debugger::fault)?;
                let verified = breakpoints
                    .iter()
                    .find(|breakpoint| breakpoint.line == line)
                    .is_some_and(|breakpoint| breakpoint.verified);
                json!({
                    "pid": pid,
                    "path": path,
                    "line": line,
                    "condition": condition,
                    "verified": verified,
                    "breakpoints": breakpoints,
                })
            }
            "clear_ui_breakpoint" => {
                let (_, pid, session) = self.debug_session(target)?;
                let path = args["path"].as_str().unwrap();
                let line = args["line"].as_f64().map(|line| line as u32);
                let breakpoints = session
                    .clear_breakpoints(path, line)
                    .map_err(crate::debugger::fault)?;
                json!({"pid": pid, "path": path, "line": line, "breakpoints": breakpoints})
            }
            "control_ui_debugger" => {
                let (_, pid, session) = self.debug_session(target)?;
                let action = args["action"].as_str().unwrap();
                let thread_id = args["thread_id"].as_f64().map(|id| id as u64);
                let action = ControlAction::parse(action, thread_id).ok_or_else(|| {
                    invalid("Choose pause, resume, step_over, step_in or step_out.")
                })?;
                let snapshot = action.apply(&session).map_err(crate::debugger::fault)?;
                json!({
                    "pid": pid,
                    "state": snapshot.state,
                    "stop": snapshot.stop,
                    "session": Self::snapshot_value(&snapshot),
                })
            }
            "get_ui_debug_stack" => {
                let (runtime, pid, session) = self.debug_session(target)?;
                let thread_id = args["thread_id"].as_f64().map(|id| id as u64);
                let start = args["start_frame"].as_u64().unwrap_or(0);
                let levels = args["levels"].as_u64().unwrap_or(50).clamp(1, 200);
                let snapshot = session.snapshot();
                let stack = session
                    .stack_trace(thread_id, start, levels)
                    .map_err(crate::debugger::fault)?;
                let (threads, threads_error) = match session.threads() {
                    Ok(threads) => (threads, Value::Null),
                    Err(error) => (
                        Vec::new(),
                        json!({"code": error.code(), "message": error.message()}),
                    ),
                };
                let mut scopes = Vec::new();
                let mut scopes_error = Value::Null;
                if let Some(frame) = stack.frames.first() {
                    match session.scopes(frame.id) {
                        Ok(found) => scopes = found,
                        Err(error) => {
                            scopes_error =
                                json!({"code": error.code(), "message": error.message()});
                        }
                    }
                }
                json!({
                    "target": runtime,
                    "pid": pid,
                    "state": snapshot.state,
                    "stop": snapshot.stop,
                    "thread_id": stack.thread_id,
                    "threads": threads,
                    "threads_error": threads_error,
                    "frames": stack.frames,
                    "total_frames": stack.total_frames,
                    "start_frame": stack.start_frame,
                    "truncated": stack.truncated,
                    "scopes": scopes,
                    "scopes_error": scopes_error,
                })
            }
            "read_ui_debug_variables" => {
                let (_, pid, session) = self.debug_session(target)?;
                let state = session.snapshot().state;
                if let Some(reference) = args["variables_reference"].as_u64() {
                    let start = args["start"].as_u64().unwrap_or(0);
                    let count = args["count"].as_u64().unwrap_or(100).clamp(1, 200);
                    let filter = args["filter"].as_str();
                    let page = session
                        .variables(reference, start, count, filter)
                        .map_err(crate::debugger::fault)?;
                    json!({
                        "pid": pid,
                        "state": state,
                        "variables_reference": reference,
                        "start": page.start,
                        "count": page.count,
                        "total": page.total,
                        "truncated": page.truncated,
                        "variables": page.variables,
                    })
                } else {
                    let frame_id = match args["frame_id"].as_u64() {
                        Some(frame_id) => frame_id,
                        None => {
                            let stack = session
                                .stack_trace(None, 0, 1)
                                .map_err(crate::debugger::fault)?;
                            stack.frames.first().map(|frame| frame.id).ok_or_else(|| {
                                Fault::new(
                                    "no_frames",
                                    "The stopped thread has no stack frames to read.",
                                )
                            })?
                        }
                    };
                    let variables = Self::scope_variables(&session, frame_id)?;
                    json!({
                        "pid": pid,
                        "state": state,
                        "frame_id": frame_id,
                        "scopes": variables["scopes"],
                        "scopes_truncated": variables["scopes_truncated"],
                    })
                }
            }
            "evaluate_ui_debug" => {
                let (_, pid, session) = self.debug_session(target)?;
                let expression = args["expression"].as_str().unwrap();
                let frame_id = args["frame_id"].as_f64().map(|id| id as u64);
                let context = args["context"].as_str();
                let state = session.snapshot().state;
                let result = session
                    .evaluate(expression, frame_id, context)
                    .map_err(crate::debugger::fault)?;
                json!({
                    "pid": pid,
                    "state": state,
                    "expression": expression,
                    "result": result.result,
                    "result_truncated": result.result_truncated,
                    "type": result.r#type,
                    "variables_reference": result.variables_reference,
                    "named_variables": result.named_variables,
                    "indexed_variables": result.indexed_variables,
                    "memory_reference": result.memory_reference,
                })
            }
            _ if Tool::parse(name).is_some_and(Tool::is_immediate_read) => self.probe(
                target,
                ProbeCommand::Read {
                    name: name.to_owned(),
                    arguments: args,
                },
            )?,
            _ => {
                return Err(Fault::new(
                    "unknown_command",
                    format!("Unknown command {name}."),
                ));
            }
        };
        Ok(result)
    }

    fn capture(&self, target: &str) -> Result<(Vec<u8>, CaptureMapping), Fault> {
        self.require_capability(target, "capture")?;
        let capture: RuntimeCapture =
            serde_json::from_value(self.registry.invoke(target, ProbeCommand::Capture)?)
                .map_err(|error| Fault::new("capture_failed", error.to_string()))?;
        let viewport = capture.viewport;
        if ![viewport.width, viewport.height]
            .into_iter()
            .all(|axis| axis.is_finite() && axis > 0.0)
            || capture.png.len() > MAX_CAPTURE_BYTES.div_ceil(3) * 4
        {
            return Err(Fault::new("capture_size", "Invalid capture size."));
        }
        let png = STANDARD
            .decode(capture.png)
            .map_err(|error| Fault::new("capture_failed", error.to_string()))?;
        if png.len() > MAX_CAPTURE_BYTES {
            return Err(Fault::new(
                "capture_size",
                "The capture exceeds the probe message budget.",
            ));
        }
        let decoder = capture_reader(&png)
            .into_decoder()
            .map_err(|error| Fault::new("capture_failed", error.to_string()))?;
        let image = decoder.dimensions();
        drop(decoder);
        Ok((png, CaptureMapping::full(viewport, image)))
    }

    fn store_capture(
        &self,
        target: String,
        mapping: CaptureMapping,
        png: Vec<u8>,
    ) -> Result<Value, Fault> {
        let id = random_id()?;
        let region = mapping.region;
        let result = json!({
            "capture_id": id, "resource": format!("/captures/{id}"), "source": "runtime",
            "width": region.width, "height": region.height,
            "coordinates": PixelRegion { x: 0, y: 0, ..region },
        });
        if png.len() > 32 * 1024 * 1024 {
            return Err(Fault::new(
                "capture_size",
                "The encoded frame exceeds the capture budget.",
            ));
        }
        let mut frames = self.frames.lock().unwrap();
        while frames.len() >= 8
            || frames.iter().map(|frame| frame.png.len()).sum::<usize>() + png.len()
                > 64 * 1024 * 1024
        {
            frames.pop_front();
        }
        frames.push_back(Frame {
            id,
            target,
            mapping,
            png: Arc::new(png),
            time: Instant::now(),
        });
        Ok(result)
    }

    pub fn image(&self, id: &str) -> Option<Arc<Vec<u8>>> {
        self.frames
            .lock()
            .unwrap()
            .iter()
            .find(|frame| frame.id == id)
            .map(|frame| frame.png.clone())
    }

    fn stored_frame(&self, id: &str, target: &str) -> Result<CaptureMapping, Fault> {
        self.frames
            .lock()
            .unwrap()
            .iter()
            .find(|frame| {
                frame.id == id
                    && frame.target == target
                    && frame.time.elapsed() < Duration::from_secs(120)
            })
            .map(|frame| frame.mapping)
            .ok_or_else(|| {
                Fault::new(
                    "stale_capture",
                    "Capture expired or belongs to another target.",
                )
            })
    }

    /// Rejects a tool an older runtime cannot answer instead of sending it a
    /// command its protocol does not know.
    fn require_capability(&self, runtime: &str, capability: &str) -> Result<(), Fault> {
        let supported = self
            .registry
            .registrations()
            .into_iter()
            .any(|(id, registration)| {
                id == runtime
                    && registration
                        .capabilities
                        .iter()
                        .any(|name| name == capability)
            });
        if supported {
            Ok(())
        } else {
            Err(Fault::new(
                "unavailable",
                format!("The runtime does not advertise {capability}."),
            ))
        }
    }

    fn wait_for(&self, target: Option<&str>, args: Value) -> Result<Value, Fault> {
        let id = self.runtime(target)?;
        let state = args["state"].as_str().unwrap_or("visible");
        if state == "value"
            && (args["node_id"].as_str().is_none() || args["expected"].as_str().is_none())
        {
            return Err(invalid("A value assertion requires node_id and expected."));
        }
        let mut query = json!({"limit": 100, "compact": true, "visible_only": state != "value"});
        for key in ["node_id", "text", "role", "source"] {
            if let Some(value) = args.get(key) {
                query[key] = value.clone();
            }
        }
        if state == "enabled" {
            query["input_enabled_only"] = json!(true);
        }
        if state == "value" {
            query["include_value"] = json!(true);
        }
        let until =
            Instant::now() + Duration::from_millis(args["timeout_ms"].as_u64().unwrap_or(2000));
        loop {
            let result = self.registry.invoke(
                &id,
                ProbeCommand::Read {
                    name: "query_ui_nodes".into(),
                    arguments: query.clone(),
                },
            )?;
            let items = result["data"]["items"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default();
            let matched = match state {
                "hidden" => items.is_empty(),
                "value" => items.iter().any(|item| {
                    item["value"] == args["expected"]
                        && item["value"] != "<redacted>"
                        && item["valueTruncated"] != true
                }),
                _ => !items.is_empty(),
            };
            if matched {
                return Ok(json!({"verified": true, "items": items}));
            }
            if Instant::now() >= until {
                return Err(Fault::new(
                    "condition_timeout",
                    "The requested state was not observed.",
                ));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

fn capture_reader(bytes: &[u8]) -> ImageReader<Cursor<&[u8]>> {
    let mut reader = ImageReader::with_format(Cursor::new(bytes), ImageFormat::Png);
    let mut limits = Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    reader
}

fn decode_capture(bytes: &[u8]) -> Result<RgbaImage, Fault> {
    capture_reader(bytes)
        .decode()
        .map(|image| image.into_rgba8())
        .map_err(|error| Fault::new("capture_failed", error.to_string()))
}

fn map_input(mut action: Value, mapping: Option<CaptureMapping>) -> Result<RuntimeInput, Fault> {
    let kind = action["kind"]
        .as_str()
        .ok_or_else(|| invalid("Input action is required."))?;
    if matches!(kind, "click" | "drag" | "scroll") {
        let mapping = mapping.ok_or_else(|| invalid("Pointer operations require a capture ID."))?;
        let map = |action: &mut Value, x: &str, y: &str| {
            let point = mapping.content_point(
                action[x]
                    .as_f64()
                    .ok_or_else(|| invalid("Invalid pointer coordinates."))?,
                action[y]
                    .as_f64()
                    .ok_or_else(|| invalid("Invalid pointer coordinates."))?,
            )?;
            action[x] = json!(point.0);
            action[y] = json!(point.1);
            Ok::<(), Fault>(())
        };
        if kind == "drag" {
            map(&mut action, "to_x", "to_y")?;
        }
        map(&mut action, "x", "y")?;
        if action["kind"] == "click" && action["button"].is_null() {
            action["button"] = json!(0);
        }
    }
    if action["kind"] == "key" && action["modifiers"].is_null() {
        action["modifiers"] = json!([]);
    }
    serde_json::from_value(action).map_err(|error| invalid(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn cropped_input_uses_image_pixels_and_refuses_outside_points() {
        let viewport = Viewport {
            width: 400.0,
            height: 300.0,
        };
        let mut mapping = CaptureMapping::full(viewport, (800, 600));
        assert_eq!(
            mapping.content_point(200.0, 100.0).unwrap(),
            (100.25, 50.25)
        );
        let crop = mapping
            .crop(&NodeFrame {
                x: 10.0,
                y: 20.0,
                w: 30.0,
                h: 40.0,
            })
            .unwrap();
        assert_eq!(
            crop,
            PixelRegion {
                x: 20,
                y: 40,
                width: 60,
                height: 80
            }
        );
        mapping.region = crop;
        assert_eq!(
            map_input(json!({"kind":"click","x":0,"y":79}), Some(mapping)).unwrap(),
            RuntimeInput::Click {
                x: 10.25,
                y: 59.75,
                button: 0
            },
        );
        assert!(
            map_input(
                json!({"kind":"drag","x":0,"y":0,"to_x":60,"to_y":1}),
                Some(mapping)
            )
            .is_err()
        );
        assert!(map_input(json!({"kind":"click","x":0,"y":0}), None).is_err());
    }

    #[test]
    fn a_runtime_without_the_capability_is_refused_before_dispatch() {
        use cove_devtools_protocol::control::{Registration, VERSION};
        let controller = Controller::new();
        let lease = controller
            .registry
            .register(Registration {
                version: VERSION,
                title: "old".into(),
                platform: "test".into(),
                pid: None,
                window: None,
                capabilities: vec!["tree".into()],
            })
            .unwrap();
        assert!(controller.require_capability(&lease.id, "tree").is_ok());
        assert_eq!(
            controller
                .require_capability(&lease.id, "inspect_nodes")
                .unwrap_err()
                .code,
            "unavailable"
        );
    }

    #[test]
    fn ui_profile_tools_are_published_with_their_bounds() {
        let definitions = crate::tools::definitions();
        for (name, read_only) in [
            ("start_ui_profile", false),
            ("stop_ui_profile", false),
            ("get_ui_profile", true),
            ("query_ui_hotspots", true),
        ] {
            let tool = definitions
                .iter()
                .find(|tool| tool.name == name)
                .unwrap_or_else(|| panic!("{name} is published"));
            assert_eq!(tool.read_only, read_only, "{name} mutation flag");
            assert!(
                tool.schema.properties.contains_key("target"),
                "{name} can select a runtime"
            );
            assert!(
                tool.schema.is_strict_capability_input(),
                "{name} has a strict input schema"
            );
        }
        let start = definitions
            .iter()
            .find(|tool| tool.name == "start_ui_profile")
            .unwrap();
        assert_eq!(
            start.schema.required,
            vec!["duration_ms".to_string(), "hz".to_string()],
            "start_ui_profile requires a rate and a bound"
        );
    }

    #[test]
    fn ui_profile_tools_refuse_a_runtime_without_a_sampler() {
        use cove_devtools_protocol::control::{Registration, VERSION};
        let controller = Controller::new();
        let lease = controller
            .registry
            .register(Registration {
                version: VERSION,
                title: "old".into(),
                platform: "test".into(),
                pid: None,
                window: None,
                capabilities: vec!["tree".into()],
            })
            .unwrap();
        for (name, mut args) in [
            ("start_ui_profile", json!({"hz": 100, "duration_ms": 100})),
            ("stop_ui_profile", json!({})),
            ("get_ui_profile", json!({})),
            ("query_ui_hotspots", json!({"limit": 5})),
        ] {
            args["target"] = json!(lease.id);
            let fault = controller.execute(name, args).unwrap_err();
            assert_eq!(fault.code, "unavailable", "{name}");
            assert!(
                fault.message.contains("cpu_profile"),
                "{name}: {}",
                fault.message
            );
        }
        let fault = controller
            .execute(
                "start_ui_profile",
                json!({"target": lease.id, "hz": 0, "duration_ms": 100}),
            )
            .unwrap_err();
        assert_eq!(
            fault.code, "invalid_request",
            "an out-of-range rate is refused before dispatch"
        );
    }

    #[test]
    fn allocation_site_tools_are_published_with_their_bounds() {
        let definitions = crate::tools::definitions();
        for (name, read_only) in [
            ("start_ui_allocation_profile", false),
            ("stop_ui_allocation_profile", false),
            ("query_ui_allocation_sites", true),
        ] {
            let tool = definitions
                .iter()
                .find(|tool| tool.name == name)
                .unwrap_or_else(|| panic!("{name} is published"));
            assert_eq!(tool.read_only, read_only, "{name} mutation flag");
            assert!(
                tool.schema.properties.contains_key("target"),
                "{name} can select a runtime"
            );
            assert!(
                tool.schema.is_strict_capability_input(),
                "{name} has a strict input schema"
            );
        }
        let start = definitions
            .iter()
            .find(|tool| tool.name == "start_ui_allocation_profile")
            .unwrap();
        assert_eq!(
            start.schema.required,
            vec!["rate".to_string()],
            "start_ui_allocation_profile requires a rate"
        );
        assert_eq!(
            start.schema.properties["rate"].maximum,
            Some(f64::from(cove_devtools_protocol::MAX_ALLOCATION_RATE)),
            "the published rate bound is the probe's bound"
        );
        let query = definitions
            .iter()
            .find(|tool| tool.name == "query_ui_allocation_sites")
            .unwrap();
        // An optional property accepts null, so the bounded branch is one of
        // the schema's choices rather than the property itself.
        let limit = &query.schema.properties["limit"];
        let maximum = limit
            .maximum
            .or_else(|| limit.one_of.iter().find_map(|candidate| candidate.maximum));
        assert_eq!(
            maximum,
            Some(cove_devtools_protocol::MAX_ALLOCATION_SITES as f64),
            "the published site bound is the probe's bound"
        );
    }

    #[test]
    fn allocation_site_tools_refuse_a_runtime_without_the_sampler() {
        use cove_devtools_protocol::control::{Registration, VERSION};
        let controller = Controller::new();
        let lease = controller
            .registry
            .register(Registration {
                version: VERSION,
                title: "old".into(),
                platform: "test".into(),
                pid: None,
                window: None,
                capabilities: vec!["tree".into()],
            })
            .unwrap();
        for (name, mut args) in [
            ("start_ui_allocation_profile", json!({"rate": 64})),
            ("stop_ui_allocation_profile", json!({})),
            ("query_ui_allocation_sites", json!({})),
        ] {
            args["target"] = json!(lease.id);
            let fault = controller.execute(name, args).unwrap_err();
            assert_eq!(fault.code, "unavailable", "{name}");
            assert!(
                fault.message.contains("allocation_sites"),
                "{name}: {}",
                fault.message
            );
        }
        for (name, args) in [
            ("start_ui_allocation_profile", json!({"rate": 0})),
            ("query_ui_allocation_sites", json!({"limit": 65})),
        ] {
            let mut args = args;
            args["target"] = json!(lease.id);
            let fault = controller.execute(name, args).unwrap_err();
            assert_eq!(
                fault.code, "invalid_request",
                "{name} refuses a value outside its published bounds"
            );
        }
    }

    #[test]
    fn allocation_site_tools_dispatch_start_stop_and_query() {
        use cove_devtools_protocol::control::{Completion, Poll, Registration, VERSION};
        let controller = Controller::new();
        let lease = controller
            .registry
            .register(Registration {
                version: VERSION,
                title: "gallery".into(),
                platform: "test".into(),
                pid: None,
                window: None,
                capabilities: vec!["tree".into(), "allocation_sites".into()],
            })
            .unwrap();
        // A fake runtime answers the three probes through the real registry, so
        // this covers the tools, capability gate, wire commands and replies.
        let registry = &controller.registry;
        let target = lease.id.clone();
        let seen = std::thread::scope(|scope| {
            let runtime_lease = lease.clone();
            let runtime = scope.spawn(move || {
                let mut seen = Vec::new();
                let mut work = registry
                    .poll(Poll {
                        lease: runtime_lease.clone(),
                        completion: None,
                    })
                    .unwrap();
                while let Some(current) = work {
                    let reply = match &current.command {
                        ProbeCommand::AllocationStart { rate } => json!({
                            "status": "running",
                            "rate": rate,
                            "sites": [],
                        }),
                        ProbeCommand::AllocationStop => json!({
                            "status": "ready",
                            "rate": 64,
                            "sites": [{"bytes": 128, "blocks": 1, "framesTruncated": false, "frames": []}],
                        }),
                        ProbeCommand::AllocationSites { limit } => json!({
                            "status": "ready",
                            "limit": limit,
                            "sites": [],
                        }),
                        other => panic!("unexpected command: {other:?}"),
                    };
                    // The completion poll can deliver the next queued command,
                    // so it becomes the loop's work instead of being dropped.
                    work = registry
                        .poll(Poll {
                            lease: runtime_lease.clone(),
                            completion: Some(Completion {
                                sequence: current.sequence,
                                result: Ok(reply),
                            }),
                        })
                        .unwrap();
                    seen.push(current.command);
                }
                seen
            });
            let started = controller
                .execute(
                    "start_ui_allocation_profile",
                    json!({"target": target, "rate": 64}),
                )
                .unwrap();
            assert_eq!(started["status"], "running");
            assert_eq!(started["rate"], 64);
            let stopped = controller
                .execute("stop_ui_allocation_profile", json!({"target": target}))
                .unwrap();
            assert_eq!(stopped["sites"][0]["bytes"], 128);
            let queried = controller
                .execute("query_ui_allocation_sites", json!({"target": target}))
                .unwrap();
            assert_eq!(
                queried["limit"], DEFAULT_ALLOCATION_SITES,
                "an omitted limit selects the documented default"
            );
            runtime.join().unwrap()
        });
        assert_eq!(
            seen,
            vec![
                ProbeCommand::AllocationStart { rate: 64 },
                ProbeCommand::AllocationStop,
                ProbeCommand::AllocationSites {
                    limit: DEFAULT_ALLOCATION_SITES,
                },
            ]
        );
    }

    #[test]
    fn ui_debug_paint_tool_is_published_with_its_bounds() {
        let definitions = crate::tools::definitions();
        let tool = definitions
            .iter()
            .find(|tool| tool.name == "set_ui_debug_paint")
            .expect("set_ui_debug_paint is published");
        assert!(!tool.read_only, "toggling debug paint mutates the runtime");
        assert!(
            tool.schema.properties.contains_key("target"),
            "the tool can select a runtime"
        );
        assert!(tool.schema.is_strict_capability_input());
        assert_eq!(tool.schema.required, vec!["guidelines".to_string()]);
    }

    #[test]
    fn ui_debug_paint_dispatches_and_reports_the_applied_state() {
        use cove_devtools_protocol::control::{Completion, Poll, Registration, VERSION};
        let controller = Controller::new();
        let lease = controller
            .registry
            .register(Registration {
                version: VERSION,
                title: "gallery".into(),
                platform: "test".into(),
                pid: None,
                window: None,
                capabilities: vec!["tree".into(), "debug_paint".into()],
            })
            .unwrap();
        // A fake runtime on a scoped thread answers through the real registry,
        // so this covers the tool, capability gate, wire command and reply.
        let registry = &controller.registry;
        let target = lease.id.clone();
        let seen = std::thread::scope(|scope| {
            let runtime_lease = lease.clone();
            let runtime = scope.spawn(move || {
                let work = registry
                    .poll(Poll {
                        lease: runtime_lease.clone(),
                        completion: None,
                    })
                    .unwrap()
                    .expect("the tool dispatched a command");
                let reply = match &work.command {
                    ProbeCommand::DebugPaint { guidelines } => {
                        json!({"guidelines": guidelines})
                    }
                    other => panic!("unexpected command: {other:?}"),
                };
                registry
                    .poll(Poll {
                        lease: runtime_lease.clone(),
                        completion: Some(Completion {
                            sequence: work.sequence,
                            result: Ok(reply),
                        }),
                    })
                    .unwrap();
                work.command
            });
            let applied = controller
                .execute(
                    "set_ui_debug_paint",
                    json!({"target": target, "guidelines": true}),
                )
                .unwrap();
            assert_eq!(applied, json!({"guidelines": true}));
            runtime.join().unwrap()
        });
        assert_eq!(seen, ProbeCommand::DebugPaint { guidelines: true });
    }

    #[test]
    fn debug_paint_is_refused_without_the_capability() {
        use cove_devtools_protocol::control::{Registration, VERSION};
        let controller = Controller::new();
        let lease = controller
            .registry
            .register(Registration {
                version: VERSION,
                title: "old".into(),
                platform: "test".into(),
                pid: None,
                window: None,
                capabilities: vec!["tree".into()],
            })
            .unwrap();
        let fault = controller
            .execute(
                "set_ui_debug_paint",
                json!({"target": lease.id, "guidelines": true}),
            )
            .unwrap_err();
        assert_eq!(fault.code, "unavailable");
        assert!(fault.message.contains("debug_paint"), "{}", fault.message);
    }

    #[test]
    fn ui_baselines_tool_is_published_with_its_bounds() {
        let definitions = crate::tools::definitions();
        let tool = definitions
            .iter()
            .find(|tool| tool.name == "set_ui_baselines")
            .expect("set_ui_baselines is published");
        assert!(!tool.read_only, "toggling baselines mutates the runtime");
        assert!(
            tool.schema.properties.contains_key("target"),
            "the tool can select a runtime"
        );
        assert!(tool.schema.is_strict_capability_input());
        assert_eq!(tool.schema.required, vec!["enabled".to_string()]);
    }

    #[test]
    fn ui_baselines_dispatches_and_reports_the_applied_state() {
        use cove_devtools_protocol::control::{Completion, Poll, Registration, VERSION};
        let controller = Controller::new();
        let lease = controller
            .registry
            .register(Registration {
                version: VERSION,
                title: "gallery".into(),
                platform: "test".into(),
                pid: None,
                window: None,
                capabilities: vec!["tree".into(), "baselines".into()],
            })
            .unwrap();
        // A fake runtime on a scoped thread answers through the real registry,
        // so this covers the tool, capability gate, wire command and reply.
        let registry = &controller.registry;
        let target = lease.id.clone();
        let seen = std::thread::scope(|scope| {
            let runtime_lease = lease.clone();
            let runtime = scope.spawn(move || {
                let work = registry
                    .poll(Poll {
                        lease: runtime_lease.clone(),
                        completion: None,
                    })
                    .unwrap()
                    .expect("the tool dispatched a command");
                let reply = match &work.command {
                    ProbeCommand::Baselines { enabled } => json!({"baselines": enabled}),
                    other => panic!("unexpected command: {other:?}"),
                };
                registry
                    .poll(Poll {
                        lease: runtime_lease.clone(),
                        completion: Some(Completion {
                            sequence: work.sequence,
                            result: Ok(reply),
                        }),
                    })
                    .unwrap();
                work.command
            });
            let applied = controller
                .execute(
                    "set_ui_baselines",
                    json!({"target": target, "enabled": true}),
                )
                .unwrap();
            assert_eq!(applied, json!({"baselines": true}));
            runtime.join().unwrap()
        });
        assert_eq!(seen, ProbeCommand::Baselines { enabled: true });
    }

    #[test]
    fn baselines_are_refused_without_the_capability() {
        use cove_devtools_protocol::control::{Registration, VERSION};
        let controller = Controller::new();
        let lease = controller
            .registry
            .register(Registration {
                version: VERSION,
                title: "old".into(),
                platform: "test".into(),
                pid: None,
                window: None,
                capabilities: vec!["tree".into()],
            })
            .unwrap();
        let fault = controller
            .execute(
                "set_ui_baselines",
                json!({"target": lease.id, "enabled": true}),
            )
            .unwrap_err();
        assert_eq!(fault.code, "unavailable");
        assert!(fault.message.contains("baselines"), "{}", fault.message);
    }

    #[test]
    fn ui_oversized_images_tool_is_published_with_its_bounds() {
        let definitions = crate::tools::definitions();
        let tool = definitions
            .iter()
            .find(|tool| tool.name == "set_ui_oversized_images")
            .expect("set_ui_oversized_images is published");
        assert!(
            !tool.read_only,
            "toggling oversized images mutates the runtime"
        );
        assert!(
            tool.schema.properties.contains_key("target"),
            "the tool can select a runtime"
        );
        assert!(tool.schema.is_strict_capability_input());
        assert_eq!(tool.schema.required, vec!["enabled".to_string()]);
    }

    #[test]
    fn ui_oversized_images_dispatches_and_reports_the_applied_state() {
        use cove_devtools_protocol::control::{Completion, Poll, Registration, VERSION};
        let controller = Controller::new();
        let lease = controller
            .registry
            .register(Registration {
                version: VERSION,
                title: "gallery".into(),
                platform: "test".into(),
                pid: None,
                window: None,
                capabilities: vec!["tree".into(), "oversized_images".into()],
            })
            .unwrap();
        // A fake runtime on a scoped thread answers through the real registry,
        // so this covers the tool, capability gate, wire command and reply.
        let registry = &controller.registry;
        let target = lease.id.clone();
        let seen = std::thread::scope(|scope| {
            let runtime_lease = lease.clone();
            let runtime = scope.spawn(move || {
                let work = registry
                    .poll(Poll {
                        lease: runtime_lease.clone(),
                        completion: None,
                    })
                    .unwrap()
                    .expect("the tool dispatched a command");
                let reply = match &work.command {
                    ProbeCommand::OversizedImages { enabled } => json!({"enabled": enabled}),
                    other => panic!("unexpected command: {other:?}"),
                };
                registry
                    .poll(Poll {
                        lease: runtime_lease.clone(),
                        completion: Some(Completion {
                            sequence: work.sequence,
                            result: Ok(reply),
                        }),
                    })
                    .unwrap();
                work.command
            });
            let applied = controller
                .execute(
                    "set_ui_oversized_images",
                    json!({"target": target, "enabled": true}),
                )
                .unwrap();
            assert_eq!(applied, json!({"enabled": true}));
            runtime.join().unwrap()
        });
        assert_eq!(seen, ProbeCommand::OversizedImages { enabled: true });
    }

    #[test]
    fn oversized_images_are_refused_without_the_capability() {
        use cove_devtools_protocol::control::{Registration, VERSION};
        let controller = Controller::new();
        let lease = controller
            .registry
            .register(Registration {
                version: VERSION,
                title: "old".into(),
                platform: "test".into(),
                pid: None,
                window: None,
                capabilities: vec!["tree".into()],
            })
            .unwrap();
        let fault = controller
            .execute(
                "set_ui_oversized_images",
                json!({"target": lease.id, "enabled": true}),
            )
            .unwrap_err();
        assert_eq!(fault.code, "unavailable");
        assert!(
            fault.message.contains("oversized_images"),
            "{}",
            fault.message
        );
    }

    #[test]
    fn ui_repaint_highlight_tool_is_published_with_its_bounds() {
        let definitions = crate::tools::definitions();
        let tool = definitions
            .iter()
            .find(|tool| tool.name == "set_ui_repaint_highlight")
            .expect("set_ui_repaint_highlight is published");
        assert!(
            !tool.read_only,
            "toggling repaint highlighting mutates the runtime"
        );
        assert!(
            tool.schema.properties.contains_key("target"),
            "the tool can select a runtime"
        );
        assert!(tool.schema.is_strict_capability_input());
        assert_eq!(tool.schema.required, vec!["enabled".to_string()]);
    }

    #[test]
    fn ui_repaint_highlight_dispatches_and_reports_the_applied_state() {
        use cove_devtools_protocol::control::{Completion, Poll, Registration, VERSION};
        let controller = Controller::new();
        let lease = controller
            .registry
            .register(Registration {
                version: VERSION,
                title: "gallery".into(),
                platform: "test".into(),
                pid: None,
                window: None,
                capabilities: vec!["tree".into(), "repaint_highlight".into()],
            })
            .unwrap();
        // A fake runtime on a scoped thread answers through the real registry,
        // so this covers the tool, capability gate, wire command and reply.
        let registry = &controller.registry;
        let target = lease.id.clone();
        let seen = std::thread::scope(|scope| {
            let runtime_lease = lease.clone();
            let runtime = scope.spawn(move || {
                let work = registry
                    .poll(Poll {
                        lease: runtime_lease.clone(),
                        completion: None,
                    })
                    .unwrap()
                    .expect("the tool dispatched a command");
                let reply = match &work.command {
                    ProbeCommand::RepaintHighlight { enabled } => json!({"enabled": enabled}),
                    other => panic!("unexpected command: {other:?}"),
                };
                registry
                    .poll(Poll {
                        lease: runtime_lease.clone(),
                        completion: Some(Completion {
                            sequence: work.sequence,
                            result: Ok(reply),
                        }),
                    })
                    .unwrap();
                work.command
            });
            let applied = controller
                .execute(
                    "set_ui_repaint_highlight",
                    json!({"target": target, "enabled": true}),
                )
                .unwrap();
            assert_eq!(applied, json!({"enabled": true}));
            runtime.join().unwrap()
        });
        assert_eq!(seen, ProbeCommand::RepaintHighlight { enabled: true });
    }

    #[test]
    fn repaint_highlight_is_refused_without_the_capability() {
        use cove_devtools_protocol::control::{Registration, VERSION};
        let controller = Controller::new();
        let lease = controller
            .registry
            .register(Registration {
                version: VERSION,
                title: "old".into(),
                platform: "test".into(),
                pid: None,
                window: None,
                capabilities: vec!["tree".into()],
            })
            .unwrap();
        let fault = controller
            .execute(
                "set_ui_repaint_highlight",
                json!({"target": lease.id, "enabled": true}),
            )
            .unwrap_err();
        assert_eq!(fault.code, "unavailable");
        assert!(
            fault.message.contains("repaint_highlight"),
            "{}",
            fault.message
        );
    }

    #[test]
    fn ui_animation_speed_tool_is_published_with_its_bounds() {
        let definitions = crate::tools::definitions();
        let tool = definitions
            .iter()
            .find(|tool| tool.name == "set_ui_animation_speed")
            .expect("set_ui_animation_speed is published");
        assert!(
            !tool.read_only,
            "changing the runtime's time scale mutates it"
        );
        assert!(
            tool.schema.properties.contains_key("target"),
            "the tool can select a runtime"
        );
        assert!(tool.schema.is_strict_capability_input());
        assert_eq!(tool.schema.required, vec!["scale".to_string()]);
        let scale = &tool.schema.properties["scale"];
        assert_eq!(scale.minimum, Some(0.05));
        assert_eq!(scale.maximum, Some(10.0));
    }

    #[test]
    fn ui_animation_speed_dispatches_and_reports_the_applied_scale() {
        use cove_devtools_protocol::control::{Completion, Poll, Registration, VERSION};
        let controller = Controller::new();
        let lease = controller
            .registry
            .register(Registration {
                version: VERSION,
                title: "gallery".into(),
                platform: "test".into(),
                pid: None,
                window: None,
                capabilities: vec!["tree".into(), "animation_speed".into()],
            })
            .unwrap();
        // A fake runtime on a scoped thread answers through the real registry,
        // so this covers the tool, capability gate, wire command and reply.
        let registry = &controller.registry;
        let target = lease.id.clone();
        let seen = std::thread::scope(|scope| {
            let runtime_lease = lease.clone();
            let runtime = scope.spawn(move || {
                let work = registry
                    .poll(Poll {
                        lease: runtime_lease.clone(),
                        completion: None,
                    })
                    .unwrap()
                    .expect("the tool dispatched a command");
                let reply = match &work.command {
                    ProbeCommand::AnimationScale { scale } => json!({"scale": scale}),
                    other => panic!("unexpected command: {other:?}"),
                };
                registry
                    .poll(Poll {
                        lease: runtime_lease.clone(),
                        completion: Some(Completion {
                            sequence: work.sequence,
                            result: Ok(reply),
                        }),
                    })
                    .unwrap();
                work.command
            });
            let applied = controller
                .execute(
                    "set_ui_animation_speed",
                    json!({"target": target, "scale": 0.25}),
                )
                .unwrap();
            assert_eq!(applied, json!({"scale": 0.25}));
            runtime.join().unwrap()
        });
        assert_eq!(seen, ProbeCommand::AnimationScale { scale: 0.25 });
    }

    #[test]
    fn animation_speed_is_refused_without_the_capability() {
        use cove_devtools_protocol::control::{Registration, VERSION};
        let controller = Controller::new();
        let lease = controller
            .registry
            .register(Registration {
                version: VERSION,
                title: "old".into(),
                platform: "test".into(),
                pid: None,
                window: None,
                capabilities: vec!["tree".into()],
            })
            .unwrap();
        let fault = controller
            .execute(
                "set_ui_animation_speed",
                json!({"target": lease.id, "scale": 0.5}),
            )
            .unwrap_err();
        assert_eq!(fault.code, "unavailable");
        assert!(
            fault.message.contains("animation_speed"),
            "{}",
            fault.message
        );
    }

    #[test]
    fn animation_speed_schema_refuses_scales_outside_the_runtime_range() {
        let controller = Controller::new();
        for scale in [0.0, 0.01, 10.5, 100.0] {
            let fault = controller
                .execute(
                    "set_ui_animation_speed",
                    json!({"target": "runtime", "scale": scale}),
                )
                .unwrap_err();
            assert_eq!(fault.code, "invalid_request", "scale {scale}");
        }
    }

    #[test]
    fn clear_ui_logs_tool_is_published_as_a_mutation_that_requires_a_target() {
        let definitions = crate::tools::definitions();
        let tool = definitions
            .iter()
            .find(|tool| tool.name == "clear_ui_logs")
            .expect("clear_ui_logs is published");
        assert!(!tool.read_only, "clearing logs mutates the runtime");
        assert_eq!(
            tool.schema.required,
            vec!["target".to_string()],
            "the runtime is selected explicitly"
        );
        assert!(tool.schema.properties.contains_key("target"));
        assert!(tool.schema.is_strict_capability_input());
    }

    #[test]
    fn clear_ui_logs_dispatches_and_returns_the_clear_acknowledgement() {
        use cove_devtools_protocol::control::{Completion, Poll, Registration, VERSION};
        let controller = Controller::new();
        let lease = controller
            .registry
            .register(Registration {
                version: VERSION,
                title: "gallery".into(),
                platform: "test".into(),
                pid: None,
                window: None,
                capabilities: vec!["tree".into(), "diagnostics".into()],
            })
            .unwrap();
        // A fake runtime on a scoped thread answers through the real registry,
        // so this covers the tool, wire command and acknowledgement.
        let registry = &controller.registry;
        let target = lease.id.clone();
        let seen = std::thread::scope(|scope| {
            let runtime_lease = lease.clone();
            let runtime = scope.spawn(move || {
                let work = registry
                    .poll(Poll {
                        lease: runtime_lease.clone(),
                        completion: None,
                    })
                    .unwrap()
                    .expect("the tool dispatched a command");
                let reply = match &work.command {
                    ProbeCommand::ClearLogs => {
                        json!({"cleared": 3, "dropped": 0, "firstAvailableSequence": null})
                    }
                    other => panic!("unexpected command: {other:?}"),
                };
                registry
                    .poll(Poll {
                        lease: runtime_lease.clone(),
                        completion: Some(Completion {
                            sequence: work.sequence,
                            result: Ok(reply),
                        }),
                    })
                    .unwrap();
                work.command
            });
            let acknowledgement = controller
                .execute("clear_ui_logs", json!({"target": target}))
                .unwrap();
            assert_eq!(
                acknowledgement,
                json!({"cleared": 3, "dropped": 0, "firstAvailableSequence": null})
            );
            runtime.join().unwrap()
        });
        assert_eq!(seen, ProbeCommand::ClearLogs);
    }

    #[test]
    fn clear_ui_logs_requires_a_connected_runtime_target() {
        let controller = Controller::new();
        let fault = controller.execute("clear_ui_logs", json!({})).unwrap_err();
        assert_eq!(
            fault.code, "invalid_request",
            "the target is required by the schema"
        );
        assert!(fault.message.contains("target"), "{}", fault.message);
        let fault = controller
            .execute("clear_ui_logs", json!({"target": "missing"}))
            .unwrap_err();
        assert_eq!(fault.code, "target_gone");
    }

    #[test]
    fn network_control_tools_are_published_as_mutations() {
        let definitions = crate::tools::definitions();
        let tool = definitions
            .iter()
            .find(|tool| tool.name == "set_ui_network_recording")
            .expect("set_ui_network_recording is published");
        assert!(!tool.read_only, "pausing the recorder mutates the runtime");
        assert_eq!(tool.schema.required, vec!["enabled".to_string()]);
        assert!(tool.schema.properties.contains_key("target"));
        assert!(tool.schema.is_strict_capability_input());
        assert_eq!(
            tool.schema.properties["enabled"].r#type,
            Some(cove_agent::JsonSchemaType::Boolean)
        );

        let tool = definitions
            .iter()
            .find(|tool| tool.name == "clear_ui_network")
            .expect("clear_ui_network is published");
        assert!(
            !tool.read_only,
            "clearing network records mutates the runtime"
        );
        assert_eq!(tool.schema.required, vec!["target".to_string()]);
        assert!(tool.schema.properties.contains_key("target"));
        assert!(tool.schema.is_strict_capability_input());
    }

    #[test]
    fn set_ui_network_recording_dispatches_and_returns_the_applied_state() {
        use cove_devtools_protocol::control::{Completion, Poll, Registration, VERSION};
        let controller = Controller::new();
        let lease = controller
            .registry
            .register(Registration {
                version: VERSION,
                title: "gallery".into(),
                platform: "test".into(),
                pid: None,
                window: None,
                capabilities: vec!["tree".into(), "network".into(), "network_control".into()],
            })
            .unwrap();
        // A fake runtime on a scoped thread answers through the real registry,
        // so this covers the tool, capability gate, wire command and reply.
        let registry = &controller.registry;
        let target = lease.id.clone();
        let seen = std::thread::scope(|scope| {
            let runtime_lease = lease.clone();
            let runtime = scope.spawn(move || {
                let work = registry
                    .poll(Poll {
                        lease: runtime_lease.clone(),
                        completion: None,
                    })
                    .unwrap()
                    .expect("the tool dispatched a command");
                let reply = match &work.command {
                    ProbeCommand::NetworkRecording { enabled } => json!({"enabled": enabled}),
                    other => panic!("unexpected command: {other:?}"),
                };
                registry
                    .poll(Poll {
                        lease: runtime_lease.clone(),
                        completion: Some(Completion {
                            sequence: work.sequence,
                            result: Ok(reply),
                        }),
                    })
                    .unwrap();
                work.command
            });
            let applied = controller
                .execute(
                    "set_ui_network_recording",
                    json!({"target": target, "enabled": false}),
                )
                .unwrap();
            assert_eq!(applied, json!({"enabled": false}));
            runtime.join().unwrap()
        });
        assert_eq!(seen, ProbeCommand::NetworkRecording { enabled: false });
    }

    #[test]
    fn clear_ui_network_dispatches_and_returns_the_cleared_count() {
        use cove_devtools_protocol::control::{Completion, Poll, Registration, VERSION};
        let controller = Controller::new();
        let lease = controller
            .registry
            .register(Registration {
                version: VERSION,
                title: "gallery".into(),
                platform: "test".into(),
                pid: None,
                window: None,
                capabilities: vec!["tree".into(), "network".into(), "network_control".into()],
            })
            .unwrap();
        let registry = &controller.registry;
        let target = lease.id.clone();
        let seen = std::thread::scope(|scope| {
            let runtime_lease = lease.clone();
            let runtime = scope.spawn(move || {
                let work = registry
                    .poll(Poll {
                        lease: runtime_lease.clone(),
                        completion: None,
                    })
                    .unwrap()
                    .expect("the tool dispatched a command");
                let reply = match &work.command {
                    ProbeCommand::ClearNetwork => json!({"cleared": 7, "limit": 256}),
                    other => panic!("unexpected command: {other:?}"),
                };
                registry
                    .poll(Poll {
                        lease: runtime_lease.clone(),
                        completion: Some(Completion {
                            sequence: work.sequence,
                            result: Ok(reply),
                        }),
                    })
                    .unwrap();
                work.command
            });
            let acknowledgement = controller
                .execute("clear_ui_network", json!({"target": target}))
                .unwrap();
            assert_eq!(acknowledgement, json!({"cleared": 7, "limit": 256}));
            runtime.join().unwrap()
        });
        assert_eq!(seen, ProbeCommand::ClearNetwork);
    }

    #[test]
    fn network_control_is_refused_without_the_capability() {
        use cove_devtools_protocol::control::{Registration, VERSION};
        let controller = Controller::new();
        let lease = controller
            .registry
            .register(Registration {
                version: VERSION,
                title: "old".into(),
                platform: "test".into(),
                pid: None,
                window: None,
                capabilities: vec!["tree".into(), "network".into()],
            })
            .unwrap();
        for (name, args) in [
            (
                "set_ui_network_recording",
                json!({"target": lease.id, "enabled": false}),
            ),
            ("clear_ui_network", json!({"target": lease.id})),
        ] {
            let fault = controller.execute(name, args).unwrap_err();
            assert_eq!(fault.code, "unavailable", "{name}");
            assert!(
                fault.message.contains("network_control"),
                "{name}: {}",
                fault.message
            );
        }
    }

    #[test]
    fn network_control_errors_name_the_missing_input() {
        let controller = Controller::new();
        let fault = controller
            .execute("set_ui_network_recording", json!({"target": "runtime"}))
            .unwrap_err();
        assert_eq!(fault.code, "invalid_request");
        assert!(fault.message.contains("enabled"), "{}", fault.message);

        let fault = controller
            .execute("clear_ui_network", json!({}))
            .unwrap_err();
        assert_eq!(
            fault.code, "invalid_request",
            "the target is required by the schema"
        );
        assert!(fault.message.contains("target"), "{}", fault.message);

        let fault = controller
            .execute("clear_ui_network", json!({"target": "missing"}))
            .unwrap_err();
        assert_eq!(fault.code, "target_gone");
    }

    #[test]
    fn ui_debugger_tools_are_published_with_their_bounds() {
        let definitions = crate::tools::definitions();
        for (name, read_only) in [
            ("get_ui_debugger", true),
            ("attach_ui_debugger", false),
            ("detach_ui_debugger", false),
            ("set_ui_breakpoint", false),
            ("clear_ui_breakpoint", false),
            ("control_ui_debugger", false),
            ("get_ui_debug_stack", true),
            ("read_ui_debug_variables", true),
            ("evaluate_ui_debug", false),
        ] {
            let tool = definitions
                .iter()
                .find(|tool| tool.name == name)
                .unwrap_or_else(|| panic!("{name} is published"));
            assert_eq!(tool.read_only, read_only, "{name} mutation flag");
            assert!(
                tool.schema.properties.contains_key("target"),
                "{name} can select a runtime"
            );
            assert!(
                tool.schema.is_strict_capability_input(),
                "{name} has a strict input schema"
            );
        }
        let set = definitions
            .iter()
            .find(|tool| tool.name == "set_ui_breakpoint")
            .unwrap();
        assert_eq!(set.schema.required, vec!["line", "path"]);
        let evaluate = definitions
            .iter()
            .find(|tool| tool.name == "evaluate_ui_debug")
            .unwrap();
        assert_eq!(evaluate.schema.required, vec!["expression"]);
        let variables = definitions
            .iter()
            .find(|tool| tool.name == "read_ui_debug_variables")
            .unwrap();
        assert!(variables.schema.required.is_empty());
    }

    fn mock_debugger() -> DebuggerManager {
        use cove_debugger::testing::MockAdapter;
        use std::sync::Arc;
        DebuggerManager::with_source(
            cove_debugger::Availability {
                adapter: Some(cove_debugger::Adapter {
                    path: "/mock/lldb-dap".into(),
                    version: "mock-lldb".into(),
                }),
                developer_mode: Some(false),
                platform: "macos".into(),
                reason: None,
            },
            Arc::new(|| {
                let (session, _handle) = MockAdapter::pair();
                Ok(session)
            }),
        )
    }

    #[test]
    fn ui_debugger_tools_attach_breakpoint_read_and_detach() {
        use cove_devtools_protocol::control::{Registration, VERSION};
        let controller = Controller::with_debugger(mock_debugger());
        let lease = controller
            .registry
            .register(Registration {
                version: VERSION,
                title: "gallery".into(),
                platform: "macos".into(),
                pid: Some(424_242),
                window: None,
                capabilities: vec!["tree".into()],
            })
            .unwrap();
        let target = lease.id.clone();
        let attached = controller
            .execute("attach_ui_debugger", json!({"target": target}))
            .unwrap();
        assert_eq!(attached["attached"], true);
        assert_eq!(attached["session"]["state"], "stopped");
        assert_eq!(attached["session"]["pid"], 424_242);

        let status = controller
            .execute("get_ui_debugger", json!({"target": target}))
            .unwrap();
        assert_eq!(status["session"]["state"], "stopped");

        let set = controller
            .execute(
                "set_ui_breakpoint",
                json!({"target": target, "path": "/tmp/mock/src/lib.rs", "line": 12}),
            )
            .unwrap();
        assert_eq!(set["verified"], true);
        assert_eq!(set["breakpoints"][0]["line"], 12);

        let stack = controller
            .execute("get_ui_debug_stack", json!({"target": target}))
            .unwrap();
        assert_eq!(stack["state"], "stopped");
        assert_eq!(stack["frames"][0]["name"], "cove::Mock::run");
        assert_eq!(stack["scopes"][0]["variables_reference"], 100);
        assert_eq!(stack["threads"].as_array().unwrap().len(), 2);

        let variables = controller
            .execute(
                "read_ui_debug_variables",
                json!({"target": target, "frame_id": 11}),
            )
            .unwrap();
        assert_eq!(variables["scopes"][0]["variables"][0]["name"], "x");
        assert_eq!(variables["scopes"][0]["variables"][0]["value"], "42");

        let evaluated = controller
            .execute(
                "evaluate_ui_debug",
                json!({"target": target, "expression": "x + 1", "frame_id": 11}),
            )
            .unwrap();
        assert_eq!(evaluated["result"], "42");
        assert_eq!(evaluated["type"], "Int");

        let resumed = controller
            .execute(
                "control_ui_debugger",
                json!({"target": target, "action": "resume"}),
            )
            .unwrap();
        assert_eq!(resumed["state"], "running");
        let fault = controller
            .execute("get_ui_debug_stack", json!({"target": target}))
            .unwrap_err();
        assert_eq!(fault.code, "not_stopped");

        let paused = controller
            .execute(
                "control_ui_debugger",
                json!({"target": target, "action": "pause"}),
            )
            .unwrap();
        assert_eq!(paused["state"], "stopped");

        let cleared = controller
            .execute(
                "clear_ui_breakpoint",
                json!({"target": target, "path": "/tmp/mock/src/lib.rs", "line": 12}),
            )
            .unwrap();
        assert!(cleared["breakpoints"].as_array().unwrap().is_empty());

        let detached = controller
            .execute("detach_ui_debugger", json!({"target": target}))
            .unwrap();
        assert_eq!(detached["detached"], true);
        let fault = controller
            .execute("get_ui_debug_stack", json!({"target": target}))
            .unwrap_err();
        assert_eq!(fault.code, "no_session");
    }

    #[test]
    fn ui_debugger_tools_refuse_a_runtime_without_an_attached_session() {
        use cove_devtools_protocol::control::{Registration, VERSION};
        let controller = Controller::new();
        let lease = controller
            .registry
            .register(Registration {
                version: VERSION,
                title: "gallery".into(),
                platform: "macos".into(),
                pid: Some(424_242),
                window: None,
                capabilities: vec!["tree".into()],
            })
            .unwrap();
        let session = controller.execute("get_ui_session", json!({})).unwrap();
        assert!(session["debugger"]["available"].is_boolean());
        assert!(
            session["debugger"]["sessions"]
                .as_array()
                .unwrap()
                .is_empty()
        );

        let debugger = controller
            .execute("get_ui_debugger", json!({"target": lease.id}))
            .unwrap();
        assert!(debugger["available"].is_boolean());
        assert!(debugger["session"].is_null());

        for (name, args) in [
            (
                "set_ui_breakpoint",
                json!({"target": lease.id, "path": "/tmp/lib.rs", "line": 12}),
            ),
            (
                "clear_ui_breakpoint",
                json!({"target": lease.id, "path": "/tmp/lib.rs"}),
            ),
            (
                "control_ui_debugger",
                json!({"target": lease.id, "action": "resume"}),
            ),
            ("get_ui_debug_stack", json!({"target": lease.id})),
            (
                "read_ui_debug_variables",
                json!({"target": lease.id, "variables_reference": 10}),
            ),
            (
                "evaluate_ui_debug",
                json!({"target": lease.id, "expression": "1 + 1"}),
            ),
            ("detach_ui_debugger", json!({"target": lease.id})),
        ] {
            let fault = controller.execute(name, args).unwrap_err();
            assert_eq!(fault.code, "no_session", "{name}: {}", fault.message);
        }
    }

    #[test]
    fn a_non_local_platform_pid_is_not_attached() {
        use cove_devtools_protocol::control::{Registration, VERSION};
        let controller = Controller::new();
        let lease = controller
            .registry
            .register(Registration {
                version: VERSION,
                title: "ipad".into(),
                platform: "ios".into(),
                pid: Some(std::process::id()),
                window: None,
                capabilities: vec!["tree".into()],
            })
            .unwrap();
        let fault = controller
            .execute("attach_ui_debugger", json!({"target": lease.id}))
            .unwrap_err();
        assert_eq!(fault.code, "unavailable");
        assert!(fault.message.contains("not local"));
    }

    #[test]
    fn a_runtime_without_a_pid_reports_why_the_debugger_cannot_attach() {
        use cove_devtools_protocol::control::{Registration, VERSION};
        let controller = Controller::new();
        let lease = controller
            .registry
            .register(Registration {
                version: VERSION,
                title: "native".into(),
                platform: "macos".into(),
                pid: None,
                window: None,
                capabilities: vec!["tree".into()],
            })
            .unwrap();
        let fault = controller
            .execute("attach_ui_debugger", json!({"target": lease.id}))
            .unwrap_err();
        assert_eq!(fault.code, "unavailable");
        assert!(fault.message.contains("process ID"));
    }

    #[test]
    fn capture_cache_keeps_eight_and_isolates_targets() {
        let controller = Controller::new();
        let mapping = CaptureMapping::full(
            Viewport {
                width: 100.0,
                height: 100.0,
            },
            (100, 100),
        );
        let mut ids = Vec::new();
        for index in 0..9u8 {
            let result = controller
                .store_capture("one".into(), mapping, vec![index])
                .unwrap();
            ids.push(result["capture_id"].as_str().unwrap().to_owned());
        }
        assert_eq!(controller.frames.lock().unwrap().len(), 8);
        assert!(controller.image(&ids[0]).is_none());
        assert!(controller.stored_frame(&ids[8], "one").is_ok());
        assert_eq!(
            controller.stored_frame(&ids[8], "two").unwrap_err().code,
            "stale_capture"
        );
    }

    fn app_size_runtime(controller: &Controller, platform: &str, pid: Option<u32>) -> String {
        use cove_devtools_protocol::control::{Registration, VERSION};
        controller
            .registry
            .register(Registration {
                version: VERSION,
                title: "gallery".into(),
                platform: platform.into(),
                pid,
                window: None,
                capabilities: vec!["tree".into()],
            })
            .unwrap()
            .id
    }

    #[test]
    fn ui_app_size_tool_is_published_with_its_bounds() {
        let definitions = crate::tools::definitions();
        let tool = definitions
            .iter()
            .find(|tool| tool.name == "get_ui_app_size")
            .expect("get_ui_app_size is published");
        assert!(tool.read_only, "size analysis only reads the executable");
        assert!(
            tool.schema.properties.contains_key("target"),
            "the tool can select a runtime"
        );
        assert!(tool.schema.is_strict_capability_input());
        assert!(tool.schema.required.is_empty());
        for (field, maximum) in [
            ("sections", crate::app_size::MAX_SECTIONS),
            ("symbols", crate::app_size::MAX_SYMBOLS),
            ("groups", crate::app_size::MAX_GROUPS),
        ] {
            // Optional properties are published as a nullable choice, so the
            // numeric bounds live on the non-null branch.
            let schema = &tool.schema.properties[field];
            let schema = schema.one_of.first().unwrap_or(schema);
            assert_eq!(schema.minimum, Some(1.0), "{field} minimum");
            assert_eq!(schema.maximum, Some(maximum as f64), "{field} maximum");
        }
    }

    #[test]
    fn ui_app_size_reads_the_registered_runtime_executable() {
        let controller = Controller::new();
        let target = app_size_runtime(&controller, std::env::consts::OS, Some(std::process::id()));
        let value = controller
            .execute(
                "get_ui_app_size",
                json!({"target": target, "sections": 2, "symbols": 2, "groups": 2}),
            )
            .unwrap();
        assert_eq!(value["available"], true);
        assert_eq!(value["measurement"], "native_executable");
        assert_eq!(value["label"], crate::app_size::LABEL);
        assert_eq!(value["target"], target);
        assert_eq!(value["pid"], std::process::id());
        assert!(value["file_bytes"].as_u64().unwrap() > 0);
        let sections = value["sections"].as_array().unwrap();
        assert!(!sections.is_empty(), "a test executable has sections");
        assert!(sections.len() <= 2);
        assert!(value["symbols"].as_array().unwrap().len() <= 2);
        assert!(value["groups"].as_array().unwrap().len() <= 2);
        let executable = std::fs::canonicalize(value["executable"].as_str().unwrap()).unwrap();
        let current = std::fs::canonicalize(std::env::current_exe().unwrap()).unwrap();
        assert_eq!(executable, current);
    }

    #[test]
    fn ui_app_size_is_unavailable_without_a_local_pid() {
        let controller = Controller::new();
        let remote = app_size_runtime(&controller, "ios", Some(std::process::id()));
        let fault = controller
            .execute("get_ui_app_size", json!({"target": remote}))
            .unwrap_err();
        assert_eq!(fault.code, "unavailable");
        assert!(
            fault.message.contains("not on this host"),
            "{}",
            fault.message
        );
        let no_pid = app_size_runtime(&controller, std::env::consts::OS, None);
        let fault = controller
            .execute("get_ui_app_size", json!({"target": no_pid}))
            .unwrap_err();
        assert_eq!(fault.code, "unavailable");
        assert!(fault.message.contains("process ID"), "{}", fault.message);
    }

    #[test]
    fn ui_deep_links_tool_is_published_and_reads_a_project_root() {
        let definitions = crate::tools::definitions();
        let tool = definitions
            .iter()
            .find(|tool| tool.name == "get_ui_deep_links")
            .expect("get_ui_deep_links is published");
        assert!(tool.read_only);
        assert_eq!(tool.schema.required, vec!["root".to_owned()]);
        assert!(tool.schema.properties.contains_key("root"));
        assert!(
            tool.schema.properties.contains_key("target"),
            "the common target field stays available"
        );

        let controller = Controller::new();
        let gallery = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/gallery")
            .canonicalize()
            .unwrap();
        let value = controller
            .execute(
                "get_ui_deep_links",
                json!({"root": gallery.display().to_string()}),
            )
            .unwrap();
        assert_eq!(value["root"], gallery.display().to_string());
        assert_eq!(value["android"]["configured"], false);
        assert_eq!(value["apple"]["configured"], false);
        assert_eq!(value["summary"]["configured"], false);
        assert!(
            value["summary"]["text"]
                .as_str()
                .unwrap()
                .contains("No deep links configured")
        );
    }

    #[test]
    fn ui_deep_links_rejects_a_relative_root() {
        let controller = Controller::new();
        let fault = controller
            .execute("get_ui_deep_links", json!({"root": "examples/gallery"}))
            .unwrap_err();
        assert_eq!(fault.code, "invalid_request");
    }

    #[test]
    fn ui_tree_page_tool_is_published_with_its_bounds() {
        let definitions = crate::tools::definitions();
        let tool = definitions
            .iter()
            .find(|tool| tool.name == "query_ui_tree")
            .expect("query_ui_tree is published");
        assert!(tool.read_only, "reading a page mutates nothing");
        assert!(
            tool.schema.properties.contains_key("target"),
            "the tool can select a runtime"
        );
        assert!(tool.schema.is_strict_capability_input());
        assert!(tool.schema.required.is_empty());
        for (field, minimum, maximum) in [("depth", 1.0, 16.0), ("limit", 1.0, 2000.0)] {
            // An optional field accepts its integer bound or null.
            let schema = &tool.schema.properties[field];
            let integer = schema.one_of.first().unwrap_or(schema);
            assert_eq!(integer.minimum, Some(minimum), "{field} minimum");
            assert_eq!(integer.maximum, Some(maximum), "{field} maximum");
        }
    }

    #[test]
    fn ui_tree_page_dispatches_the_bounded_query() {
        use cove_devtools_protocol::control::{Completion, Poll, Registration, VERSION};
        let controller = Controller::new();
        let lease = controller
            .registry
            .register(Registration {
                version: VERSION,
                title: "gallery".into(),
                platform: "test".into(),
                pid: None,
                window: None,
                capabilities: vec!["tree".into(), "tree_page".into()],
            })
            .unwrap();
        let registry = &controller.registry;
        let target = lease.id.clone();
        let page = json!({
            "root": {"id": "root", "frame": {"x":0.0,"y":0.0,"w":1.0,"h":1.0}, "absoluteFrame": {"x":0.0,"y":0.0,"w":1.0,"h":1.0}, "children": []},
            "depth": 2,
            "limit": 200,
            "nodeCount": 1,
            "truncated": true,
        });
        let seen = std::thread::scope(|scope| {
            let runtime_lease = lease.clone();
            let reply = page.clone();
            let runtime = scope.spawn(move || {
                let work = registry
                    .poll(Poll {
                        lease: runtime_lease.clone(),
                        completion: None,
                    })
                    .unwrap()
                    .expect("the tool dispatched a command");
                registry
                    .poll(Poll {
                        lease: runtime_lease.clone(),
                        completion: Some(Completion {
                            sequence: work.sequence,
                            result: Ok(reply),
                        }),
                    })
                    .unwrap();
                work.command
            });
            let result = controller
                .execute(
                    "query_ui_tree",
                    json!({"target": target, "node_id": "root", "depth": 2, "limit": 200}),
                )
                .unwrap();
            assert_eq!(result, page);
            runtime.join().unwrap()
        });
        assert_eq!(
            seen,
            ProbeCommand::TreePage {
                node_id: Some("root".into()),
                depth: 2,
                limit: 200,
            }
        );
    }

    #[test]
    fn ui_tree_page_defaults_to_four_levels_and_five_hundred_nodes() {
        use cove_devtools_protocol::control::{Completion, Poll, Registration, VERSION};
        let controller = Controller::new();
        let lease = controller
            .registry
            .register(Registration {
                version: VERSION,
                title: "gallery".into(),
                platform: "test".into(),
                pid: None,
                window: None,
                capabilities: vec!["tree".into(), "tree_page".into()],
            })
            .unwrap();
        let registry = &controller.registry;
        let target = lease.id.clone();
        let seen = std::thread::scope(|scope| {
            let runtime_lease = lease.clone();
            let runtime = scope.spawn(move || {
                let work = registry
                    .poll(Poll {
                        lease: runtime_lease.clone(),
                        completion: None,
                    })
                    .unwrap()
                    .expect("the tool dispatched a command");
                registry
                    .poll(Poll {
                        lease: runtime_lease.clone(),
                        completion: Some(Completion {
                            sequence: work.sequence,
                            result: Ok(json!({"root": null, "nodeCount": 0})),
                        }),
                    })
                    .unwrap();
                work.command
            });
            controller
                .execute("query_ui_tree", json!({"target": target}))
                .unwrap();
            runtime.join().unwrap()
        });
        assert_eq!(
            seen,
            ProbeCommand::TreePage {
                node_id: None,
                depth: 4,
                limit: 500,
            }
        );
    }

    #[test]
    fn ui_tree_page_rejects_out_of_range_bounds_and_node_ids() {
        use cove_devtools_protocol::control::{Registration, VERSION};
        let controller = Controller::new();
        let lease = controller
            .registry
            .register(Registration {
                version: VERSION,
                title: "gallery".into(),
                platform: "test".into(),
                pid: None,
                window: None,
                capabilities: vec!["tree".into(), "tree_page".into()],
            })
            .unwrap();
        for args in [
            json!({"target": lease.id, "depth": 0}),
            json!({"target": lease.id, "depth": 17}),
            json!({"target": lease.id, "limit": 0}),
            json!({"target": lease.id, "limit": 2001}),
            json!({"target": lease.id, "node_id": ""}),
            json!({"target": lease.id, "node_id": "x".repeat(257)}),
        ] {
            let fault = controller
                .execute("query_ui_tree", args.clone())
                .unwrap_err();
            assert_eq!(fault.code, "invalid_request", "args: {args}");
        }
    }

    #[test]
    fn ui_tree_page_requires_the_runtime_capability() {
        use cove_devtools_protocol::control::{Registration, VERSION};
        let controller = Controller::new();
        let lease = controller
            .registry
            .register(Registration {
                version: VERSION,
                title: "old".into(),
                platform: "test".into(),
                pid: None,
                window: None,
                capabilities: vec!["tree".into()],
            })
            .unwrap();
        let fault = controller
            .execute("query_ui_tree", json!({"target": lease.id, "depth": 2}))
            .unwrap_err();
        assert_eq!(fault.code, "unavailable");
        assert!(fault.message.contains("tree_page"), "{}", fault.message);
    }

    #[test]
    fn ui_tree_page_surfaces_a_stale_node_from_the_runtime() {
        use cove_devtools_protocol::control::{Completion, Poll, Registration, VERSION};
        let controller = Controller::new();
        let lease = controller
            .registry
            .register(Registration {
                version: VERSION,
                title: "gallery".into(),
                platform: "test".into(),
                pid: None,
                window: None,
                capabilities: vec!["tree".into(), "tree_page".into()],
            })
            .unwrap();
        let registry = &controller.registry;
        let target = lease.id.clone();
        std::thread::scope(|scope| {
            let runtime_lease = lease.clone();
            let runtime = scope.spawn(move || {
                let work = registry
                    .poll(Poll {
                        lease: runtime_lease.clone(),
                        completion: None,
                    })
                    .unwrap()
                    .expect("the tool dispatched a command");
                registry
                    .poll(Poll {
                        lease: runtime_lease.clone(),
                        completion: Some(Completion {
                            sequence: work.sequence,
                            result: Err(Fault::new(
                                "stale_node",
                                "No live node has element ID gone.",
                            )),
                        }),
                    })
                    .unwrap();
            });
            let fault = controller
                .execute(
                    "query_ui_tree",
                    json!({"target": target, "node_id": "gone"}),
                )
                .unwrap_err();
            assert_eq!(fault.code, "stale_node");
            assert!(fault.message.contains("gone"), "{}", fault.message);
            runtime.join().unwrap();
        });
    }

    #[test]
    fn a_summary_tree_finds_a_nested_node_and_its_frame() {
        let tree: TreePage = serde_json::from_value(json!({
            "root": {
                "id": "c",
                "frame": {"x": 0.0, "y": 0.0, "w": 1.0, "h": 1.0},
                "absoluteFrame": {"x": 2.0, "y": 20.0, "w": 30.0, "h": 40.0}
            },
            "depth": 1, "limit": 1, "nodeCount": 1, "truncated": false
        }))
        .unwrap();
        let frame = node_absolute_frame(tree.clone(), "c").unwrap();
        assert_eq!(
            (frame.x, frame.y, frame.w, frame.h),
            (2.0, 20.0, 30.0, 40.0)
        );
        assert_eq!(
            node_absolute_frame(tree, "missing").unwrap_err().code,
            "stale_node"
        );
    }
}
