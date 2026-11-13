use crate::debugger::{ControlAction, DebuggerManager};
use crate::platform::{Action, Geometry, ListedTarget, Platform, Rect, Scope, Target};
use crate::registry::{Registry, random_id};
use cove_agent::{AgentTool, JsonSchemaValidator, ui_tools::Tool};
use cove_debugger::{DapSession, SessionSnapshot};
use cove_devtools_protocol::control::{Fault, ProbeCommand};
use cove_devtools_protocol::{
    ContentGeometry, MAX_TREE_PAGE_DEPTH, MAX_TREE_PAGE_NODES, NodeFrame, TreeSnapshot,
};
use image::{RgbaImage, imageops};
use serde_json::{Value, json};
use std::collections::{HashMap, VecDeque};
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct Frame {
    id: String,
    target: Target,
    /// The full capture geometry, used to detect a target that moved.
    geometry: Geometry,
    /// The part of `geometry.coordinates` this frame shows. A node crop is
    /// smaller than its window; input is bounded by this region.
    region: Rect,
    png: Arc<Vec<u8>>,
    width: u32,
    height: u32,
    time: Instant,
}

pub struct Controller {
    pub registry: Registry,
    platform: Platform,
    tools: Vec<AgentTool>,
    frames: Mutex<VecDeque<Frame>>,
    bindings: Mutex<HashMap<String, Target>>,
    input: Mutex<()>,
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

/// Maps a capture image's pixels onto the runtime's content logical space.
///
/// The capture reports `coordinates` in the platform's window-server space
/// while its image may be denser (a Retina window is 2x). The runtime reports
/// where its content `(0, 0)` sits in that same server space and how server
/// units divide into content logical points. Keeping the three conversions in
/// one place makes pixel → content and content → crop inverses of each other.
#[derive(Clone, Copy, Debug)]
struct ContentMapping {
    coordinates: Rect,
    image: (u32, u32),
    origin: (f64, f64),
    scale: f64,
}

/// An immutable copy of a retained capture's geometry.
#[derive(Clone, Debug)]
struct StoredFrame {
    geometry: Geometry,
    region: Rect,
    width: u32,
    height: u32,
}

/// The frame `capture_ui_node` crops for a node in a wire `tree` snapshot.
fn node_absolute_frame(tree: &TreeSnapshot, node_id: &str) -> Result<NodeFrame, Fault> {
    tree.root
        .as_ref()
        .and_then(|root| root.find(node_id))
        .ok_or_else(|| {
            Fault::new(
                "stale_node",
                "The selected component is no longer in the scene.",
            )
        })?
        .absolute_frame
        .clone()
        .ok_or_else(|| Fault::new("invalid_node", "The node has no absolute frame."))
}

impl StoredFrame {
    /// The coordinate space this frame's image shows: the crop for a node
    /// capture, the whole capture otherwise. Input bounds and pixel mapping use
    /// it, while `geometry` stays intact so movement checks compare like with
    /// like.
    fn presented(&self) -> Geometry {
        Geometry {
            coordinates: self.region,
            window: self.geometry.window,
            display: self.geometry.display,
            context: self.geometry.context.clone(),
        }
    }
}

/// A pixel rectangle inside a capture and the server-space rectangle it shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CropRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    coordinates: Rect,
}

impl ContentMapping {
    fn new(
        geometry: &Geometry,
        image: (u32, u32),
        content: &ContentGeometry,
    ) -> Result<Self, Fault> {
        let coordinates = geometry.coordinates;
        if image.0 == 0 || image.1 == 0 || coordinates.width == 0 || coordinates.height == 0 {
            return Err(Fault::new("capture_size", "The capture has no pixel area."));
        }
        if !content.is_valid() {
            return Err(Fault::new(
                "unavailable",
                "The runtime reported invalid content geometry.",
            ));
        }
        Ok(Self {
            coordinates,
            image,
            origin: (content.origin_x, content.origin_y),
            scale: content.scale,
        })
    }

    /// Window-server units per image pixel on each axis. The two differ when a
    /// scaled capture rounds its pixel width.
    fn units(&self) -> (f64, f64) {
        (
            f64::from(self.coordinates.width) / f64::from(self.image.0),
            f64::from(self.coordinates.height) / f64::from(self.image.1),
        )
    }

    /// Image pixel → content logical point, using the pixel's centre so a point
    /// inside a pixel stays inside the node drawn there.
    fn content_point(&self, pixel: (f64, f64)) -> Result<(f64, f64), Fault> {
        if !pixel.0.is_finite()
            || !pixel.1.is_finite()
            || !(0.0..f64::from(self.image.0)).contains(&pixel.0)
            || !(0.0..f64::from(self.image.1)).contains(&pixel.1)
        {
            return Err(invalid("The point is outside the captured image."));
        }
        let (ux, uy) = self.units();
        let server_x = f64::from(self.coordinates.x) + (pixel.0 + 0.5) * ux;
        let server_y = f64::from(self.coordinates.y) + (pixel.1 + 0.5) * uy;
        Ok((
            (server_x - self.origin.0) / self.scale,
            (server_y - self.origin.1) / self.scale,
        ))
    }

    /// Content logical frame → the pixel rectangle that contains it.
    ///
    /// Returns `None` when the frame has no area or none of it lies inside the
    /// capture.
    fn crop(&self, frame: &NodeFrame) -> Option<CropRect> {
        if !frame.x.is_finite()
            || !frame.y.is_finite()
            || !frame.w.is_finite()
            || !frame.h.is_finite()
            || frame.w <= 0.0
            || frame.h <= 0.0
        {
            return None;
        }
        let (ux, uy) = self.units();
        let left = self.origin.0 + frame.x * self.scale;
        let top = self.origin.1 + frame.y * self.scale;
        let right = left + frame.w * self.scale;
        let bottom = top + frame.h * self.scale;
        if ![left, top, right, bottom]
            .iter()
            .all(|value| value.is_finite())
        {
            return None;
        }
        let width = f64::from(self.image.0);
        let height = f64::from(self.image.1);
        let px0 = ((left - f64::from(self.coordinates.x)) / ux)
            .floor()
            .clamp(0.0, width);
        let py0 = ((top - f64::from(self.coordinates.y)) / uy)
            .floor()
            .clamp(0.0, height);
        let px1 = ((right - f64::from(self.coordinates.x)) / ux)
            .ceil()
            .clamp(0.0, width);
        let py1 = ((bottom - f64::from(self.coordinates.y)) / uy)
            .ceil()
            .clamp(0.0, height);
        if px1 <= px0 || py1 <= py0 {
            return None;
        }
        let x = px0 as u32;
        let y = py0 as u32;
        let crop_width = (px1 - px0) as u32;
        let crop_height = (py1 - py0) as u32;
        Some(CropRect {
            x,
            y,
            width: crop_width,
            height: crop_height,
            coordinates: Rect {
                x: (f64::from(self.coordinates.x) + px0 * ux).round() as i32,
                y: (f64::from(self.coordinates.y) + py0 * uy).round() as i32,
                width: (f64::from(crop_width) * ux).round().max(1.0) as u32,
                height: (f64::from(crop_height) * uy).round().max(1.0) as u32,
            },
        })
    }
}

impl Controller {
    #[cfg(test)]
    fn with_debugger(debugger: DebuggerManager) -> Self {
        let mut controller = Self::new(Vec::new()).unwrap();
        controller.debugger = debugger;
        controller
    }

    pub fn new(drivers: Vec<String>) -> Result<Self, Fault> {
        Ok(Self {
            registry: Registry::default(),
            platform: Platform::new(drivers)?,
            tools: crate::tools::definitions(),
            frames: Mutex::new(VecDeque::new()),
            bindings: Mutex::new(HashMap::new()),
            input: Mutex::new(()),
            debugger: DebuggerManager::new(),
        })
    }

    pub fn permissions(&self) -> Value {
        crate::platform::permissions()
    }

    fn targets(&self) -> Value {
        let runtimes = self.registry.list();
        let platforms = self.platform.list();
        let mut bindings = self.bindings.lock().unwrap();
        bindings.retain(|id, platform| {
            runtimes.iter().any(|runtime| runtime["id"] == *id)
                && platforms.iter().any(|current| current.target == *platform)
        });
        let mut targets = runtimes;
        for runtime in &mut targets {
            if let Some(platform) =
                self.resolve_binding(runtime["id"].as_str().unwrap(), &platforms, &bindings)
            {
                runtime["platform_target"] = json!(platform.id());
                if let (Some(caps), Some(platform_caps)) = (
                    runtime["capabilities"].as_array_mut(),
                    platform.describe()["capabilities"].as_array(),
                ) {
                    caps.extend(platform_caps.iter().cloned());
                }
            }
            #[cfg(target_os = "linux")]
            if runtime["platform"] == "linux"
                && platforms
                    .iter()
                    .any(|target| target.target == Target::Wayland)
                && let Some(caps) = runtime["capabilities"].as_array_mut()
            {
                caps.push(json!("native_elements"));
            }
        }
        targets.extend(platforms.iter().map(ListedTarget::describe));
        json!({"targets": targets})
    }

    fn resolve_binding<'a>(
        &self,
        id: &str,
        platform: &'a [ListedTarget],
        bindings: &HashMap<String, Target>,
    ) -> Option<&'a ListedTarget> {
        if let Some(bound) = bindings.get(id) {
            return platform.iter().find(|item| item.target == *bound);
        }
        let (_, runtime) = self
            .registry
            .registrations()
            .into_iter()
            .find(|(target, _)| target == id)?;
        let pid = runtime.pid?;
        let mut windows = platform.iter().filter(|item| {
            runtime.platform == item.platform && matches!(item.target,
                Target::Window { pid: window_pid, id: window_id }
                    if window_pid == pid && runtime.window.is_none_or(|id| id == u64::from(window_id)))
        });
        let first = windows.next()?;
        windows.next().is_none().then_some(first)
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

    fn platform(&self, id: Option<&str>) -> Result<Target, Fault> {
        let targets = self.platform.list();
        if let Some(id) = id
            && let Some(target) = targets.iter().find(|target| target.id() == id)
        {
            return Ok(target.target.clone());
        }
        let runtime = self.runtime(id)?;
        self.resolve_binding(&runtime, &targets, &self.bindings.lock().unwrap())
            .map(|target| target.target.clone())
            .ok_or_else(|| {
                Fault::new(
                    "target_unbound",
                    "Select a platform target or bind this runtime to one.",
                )
            })
    }

    fn native_target(&self, id: Option<&str>) -> Result<Target, Fault> {
        #[cfg(target_os = "linux")]
        if let Ok(runtime) = self.runtime(id)
            && let Some((_, registration)) = self
                .registry
                .registrations()
                .into_iter()
                .find(|(target, _)| *target == runtime)
            && let Some(target) = self.platform.native_process(&registration)
        {
            return Ok(target);
        }
        self.platform(id)
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
        if let Some(id) = id
            && !self
                .registry
                .registrations()
                .iter()
                .any(|(current, _)| current == id)
            && self.platform.list().iter().any(|listed| listed.id() == id)
        {
            return Err(Fault::new(
                "unavailable",
                "App Size measures the native executable of a running Cove runtime. The selected target is a platform window, display or browser session, which has no runtime executable.",
            ));
        }
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
                let mut session = self.targets();
                session["platform"] = json!(std::env::consts::OS);
                session["permissions"] = crate::platform::permissions();
                session["debugger"] = self.debugger.availability_json();
                session
            }
            "request_ui_permissions" => crate::platform::request_permissions(),
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
            "bind_ui_target" => {
                let runtime = self.runtime(target)?;
                let (_, registration) = self
                    .registry
                    .registrations()
                    .into_iter()
                    .find(|(id, _)| *id == runtime)
                    .ok_or_else(|| Fault::new("target_gone", "The runtime disconnected."))?;
                let platform = self
                    .platform
                    .list()
                    .into_iter()
                    .find(|item| item.id() == args["platform_target"].as_str().unwrap())
                    .ok_or_else(|| Fault::new("target_gone", "No such platform target."))?;
                let compatible = registration.platform == platform.platform
                    && match platform.target {
                        Target::Window { pid, id } => {
                            registration.pid == Some(pid)
                                && registration
                                    .window
                                    .is_none_or(|window| window == u64::from(id))
                        }
                        Target::Driver(_) => true,
                        Target::Display(_) => false,
                        #[cfg(target_os = "linux")]
                        Target::Wayland => true,
                        #[cfg(target_os = "linux")]
                        Target::AccessibleProcess(_) => false,
                    };
                if !compatible {
                    return Err(invalid(
                        "Select the runtime's own window or a WebDriver session for the same platform.",
                    ));
                }
                self.bindings
                    .lock()
                    .unwrap()
                    .insert(runtime, platform.target);
                json!({"bound": true})
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
                let platform_target = self.platform(target)?;
                let capture_id = args["capture_id"].as_str().unwrap();
                let frame = self.stored_frame(capture_id, &platform_target)?;
                let content = self.content_geometry(&runtime)?;
                let mapping =
                    ContentMapping::new(&frame.presented(), (frame.width, frame.height), &content)?;
                let point = mapping
                    .content_point((args["x"].as_f64().unwrap(), args["y"].as_f64().unwrap()))?;
                let mut hit = self.registry.invoke(
                    &runtime,
                    ProbeCommand::NodeAt {
                        x: point.0,
                        y: point.1,
                    },
                )?;
                hit["capture_id"] = json!(capture_id);
                hit["contentPoint"] = json!({"x": point.0, "y": point.1});
                hit
            }
            "capture_ui_node" => {
                let runtime = self.runtime(target)?;
                let platform_target = self.platform(target)?;
                let node_id = args["node_id"].as_str().unwrap();
                // Capture first: the crop is cut from these pixels, so the node
                // frame and content origin are read against the captured moment.
                let capture = self.platform.capture(&platform_target, Scope::Window)?;
                let content = self.content_geometry(&runtime)?;
                let tree: TreeSnapshot =
                    serde_json::from_value(self.registry.invoke(&runtime, ProbeCommand::Tree)?)
                        .map_err(|error| Fault::new("internal", error.to_string()))?;
                let frame = node_absolute_frame(&tree, node_id)?;
                let image = (capture.pixels.width(), capture.pixels.height());
                let mapping = ContentMapping::new(&capture.geometry, image, &content)?;
                let crop = mapping.crop(&frame).ok_or_else(|| {
                    Fault::new(
                        "node_not_visible",
                        "The node has no area inside the window capture.",
                    )
                })?;
                let pixels =
                    imageops::crop_imm(&capture.pixels, crop.x, crop.y, crop.width, crop.height)
                        .to_image();
                // The frame keeps the full window geometry so a later input
                // action can still detect that the target moved; only the
                // presented region is the crop.
                let mut result = self.store_capture(
                    platform_target,
                    capture.geometry,
                    crop.coordinates,
                    pixels,
                    capture.source,
                )?;
                result["node_id"] = json!(node_id);
                result["contentFrame"] = serde_json::to_value(frame)
                    .map_err(|error| Fault::new("internal", error.to_string()))?;
                result
            }
            "query_native_ui" => self
                .platform
                .native_elements(&self.native_target(target)?)?,
            "find_native_ui" => self.platform.find_element(
                &self.native_target(target)?,
                args["using"].as_str().unwrap(),
                args["value"].as_str().unwrap(),
            )?,
            "capture_ui" => {
                let scope = args["scope"].as_str().unwrap_or("window");
                self.capture(
                    self.platform(target)?,
                    if scope == "display" {
                        Scope::Display
                    } else {
                        Scope::Window
                    },
                )?
            }
            "input_ui" | "click_ui" | "press_ui_key" | "activate_ui" | "type_ui_text" => {
                let action = match name {
                    "input_ui" => args["action"].clone(),
                    "click_ui" => {
                        json!({"kind":"click", "x": args["x"], "y":args["y"], "button":args["button"].as_u64().unwrap_or(0)})
                    }
                    "press_ui_key" => {
                        json!({"kind":"key","key":args["key"],"modifiers":args["modifiers"].as_array().cloned().unwrap_or_default()})
                    }
                    "activate_ui" => json!({"kind":"activate","element":args["element"]}),
                    _ => json!({"kind":"text", "text": args["text"]}),
                };
                let action: Action =
                    serde_json::from_value(action).map_err(|error| invalid(error.to_string()))?;
                let selected = if matches!(action, Action::Activate { .. }) {
                    self.native_target(target)?
                } else {
                    self.platform(target)?
                };
                self.input(selected, args["capture_id"].as_str(), action)?
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

    fn capture(&self, target: Target, scope: Scope) -> Result<Value, Fault> {
        let capture = self.platform.capture(&target, scope)?;
        let region = capture.geometry.coordinates;
        self.store_capture(
            target,
            capture.geometry,
            region,
            capture.pixels,
            capture.source,
        )
    }

    /// Encodes and retains a capture under the shared frame budget.
    fn store_capture(
        &self,
        target: Target,
        geometry: Geometry,
        region: Rect,
        pixels: RgbaImage,
        source: &str,
    ) -> Result<Value, Fault> {
        let id = random_id()?;
        let width = pixels.width();
        let height = pixels.height();
        let mut png = Cursor::new(Vec::new());
        pixels
            .write_to(&mut png, image::ImageFormat::Png)
            .map_err(|error| Fault::new("capture_failed", error.to_string()))?;
        let png = png.into_inner();
        if png.len() > 32 * 1024 * 1024 {
            return Err(Fault::new(
                "capture_size",
                "The encoded capture exceeds the resource budget.",
            ));
        }
        let mut frames = self.frames.lock().unwrap();
        let result = json!({
            "capture_id": id, "resource": format!("/captures/{id}"), "source": source,
            "width": width, "height": height, "coordinates": region,
        });
        while frames.len() >= 8
            || frames.iter().map(|frame| frame.png.len()).sum::<usize>() + png.len()
                > 64 * 1024 * 1024
        {
            frames.pop_front();
        }
        frames.push_back(Frame {
            id,
            target,
            geometry,
            region,
            png: Arc::new(png),
            width,
            height,
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

    /// The live frame for a capture ID, rejecting expired or foreign captures.
    fn stored_frame(&self, id: &str, target: &Target) -> Result<StoredFrame, Fault> {
        self.frames
            .lock()
            .unwrap()
            .iter()
            .find(|frame| {
                frame.id == id
                    && frame.target == *target
                    && frame.time.elapsed() < Duration::from_secs(120)
            })
            .map(|frame| StoredFrame {
                geometry: frame.geometry.clone(),
                region: frame.region,
                width: frame.width,
                height: frame.height,
            })
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

    fn content_geometry(&self, runtime: &str) -> Result<ContentGeometry, Fault> {
        self.require_capability(runtime, "content_geometry")?;
        let value = self
            .registry
            .invoke(runtime, ProbeCommand::ContentGeometry)?;
        let geometry: ContentGeometry = serde_json::from_value(value)
            .map_err(|error| Fault::new("unavailable", error.to_string()))?;
        if !geometry.is_valid() {
            return Err(Fault::new(
                "unavailable",
                "The runtime reported invalid content geometry.",
            ));
        }
        Ok(geometry)
    }

    fn input(&self, target: Target, capture: Option<&str>, action: Action) -> Result<Value, Fault> {
        let frame = capture
            .map(|id| self.stored_frame(id, &target))
            .transpose()?;
        if let Some(frame) = &frame {
            // Bound the point to the region the client saw, while the full
            // capture geometry still detects a target that moved.
            action.validate(Some(&frame.presented()))?;
        }
        let _writer = self
            .input
            .try_lock()
            .map_err(|_| Fault::new("busy", "Another input action is in progress."))?;
        let source =
            self.platform
                .perform(&target, action, frame.as_ref().map(|frame| &frame.geometry))?;
        Ok(json!({"delivered": true, "source": source, "verified": false}))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn geometry(x: i32, y: i32, width: u32, height: u32) -> Geometry {
        Geometry {
            coordinates: Rect {
                x,
                y,
                width,
                height,
            },
            window: None,
            display: None,
            context: None,
        }
    }

    fn content(origin: (f64, f64), scale: f64) -> ContentGeometry {
        ContentGeometry {
            origin_x: origin.0,
            origin_y: origin.1,
            scale,
            width: 800.0,
            height: 600.0,
        }
    }

    fn frame(x: f64, y: f64, w: f64, h: f64) -> NodeFrame {
        NodeFrame { x, y, w, h }
    }

    #[test]
    fn pixel_centres_map_through_a_one_x_capture() {
        let mapping = ContentMapping::new(
            &geometry(100, 200, 400, 300),
            (400, 300),
            &content((100.0, 200.0), 1.0),
        )
        .unwrap();
        assert_eq!(mapping.content_point((0.0, 0.0)).unwrap(), (0.5, 0.5));
        assert_eq!(
            mapping.content_point((399.0, 299.0)).unwrap(),
            (399.5, 299.5)
        );
        assert_eq!(
            mapping.content_point((400.0, 0.0)).unwrap_err().code,
            "invalid_request"
        );
    }

    #[test]
    fn a_two_x_capture_halves_pixels_before_subtracting_the_origin() {
        let mapping = ContentMapping::new(
            &geometry(100, 200, 400, 300),
            (800, 600),
            &content((100.0, 200.0), 1.0),
        )
        .unwrap();
        assert_eq!(
            mapping.content_point((200.0, 100.0)).unwrap(),
            (100.25, 50.25)
        );
        assert_eq!(
            mapping.content_point((799.0, 599.0)).unwrap(),
            (399.75, 299.75)
        );
    }

    #[test]
    fn content_scale_converts_window_server_units_to_logical_points() {
        // A 200% Windows display: one logical point covers two server pixels.
        let mapping = ContentMapping::new(
            &geometry(0, 0, 400, 300),
            (400, 300),
            &content((0.0, 0.0), 2.0),
        )
        .unwrap();
        assert_eq!(mapping.content_point((20.0, 10.0)).unwrap(), (10.25, 5.25));
        assert_eq!(
            mapping.crop(&frame(10.0, 20.0, 30.0, 40.0)).unwrap(),
            CropRect {
                x: 20,
                y: 40,
                width: 60,
                height: 80,
                coordinates: Rect {
                    x: 20,
                    y: 40,
                    width: 60,
                    height: 80,
                },
            }
        );
    }

    #[test]
    fn a_two_x_capture_crops_the_node_region_in_image_pixels() {
        let mapping = ContentMapping::new(
            &geometry(0, 0, 400, 300),
            (800, 600),
            &content((100.0, 50.0), 1.0),
        )
        .unwrap();
        let crop = mapping.crop(&frame(10.0, 20.0, 30.0, 40.0)).unwrap();
        assert_eq!(
            (crop.x, crop.y, crop.width, crop.height),
            (220, 140, 60, 80)
        );
        assert_eq!(
            crop.coordinates,
            Rect {
                x: 110,
                y: 70,
                width: 30,
                height: 40,
            }
        );
    }

    #[test]
    fn a_node_outside_the_capture_has_no_crop() {
        let mapping = ContentMapping::new(
            &geometry(0, 0, 400, 300),
            (400, 300),
            &content((0.0, 0.0), 1.0),
        )
        .unwrap();
        assert!(mapping.crop(&frame(500.0, 20.0, 30.0, 40.0)).is_none());
        assert!(mapping.crop(&frame(10.0, 20.0, 0.0, 40.0)).is_none());
        let clipped = mapping.crop(&frame(390.0, 290.0, 30.0, 40.0)).unwrap();
        assert_eq!(
            (clipped.x, clipped.y, clipped.width, clipped.height),
            (390, 290, 10, 10)
        );
    }

    #[test]
    fn cropping_returns_only_the_node_pixels() {
        let mut image = RgbaImage::from_pixel(800, 600, image::Rgba([0, 0, 0, 255]));
        for y in 140..220 {
            for x in 220..280 {
                image.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
            }
        }
        let mapping = ContentMapping::new(
            &geometry(0, 0, 400, 300),
            (800, 600),
            &content((100.0, 50.0), 1.0),
        )
        .unwrap();
        let crop = mapping.crop(&frame(10.0, 20.0, 30.0, 40.0)).unwrap();
        let pixels = imageops::crop_imm(&image, crop.x, crop.y, crop.width, crop.height).to_image();
        assert_eq!((pixels.width(), pixels.height()), (60, 80));
        assert!(pixels.pixels().all(|pixel| pixel.0 == [255, 255, 255, 255]));
        assert_eq!(image.get_pixel(219, 140).0, [0, 0, 0, 255]);
    }

    #[test]
    fn a_runtime_without_the_capability_is_refused_before_dispatch() {
        use cove_devtools_protocol::control::{Registration, VERSION};
        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
    fn a_cropped_frame_maps_pixels_through_its_presented_region() {
        let frame = StoredFrame {
            geometry: geometry(100, 200, 400, 300),
            region: Rect {
                x: 110,
                y: 220,
                width: 30,
                height: 40,
            },
            width: 60,
            height: 80,
        };
        let mapping = ContentMapping::new(
            &frame.presented(),
            (frame.width, frame.height),
            &content((110.0, 220.0), 1.0),
        )
        .unwrap();
        assert_eq!(mapping.content_point((0.0, 0.0)).unwrap(), (0.25, 0.25));
        assert_eq!(mapping.content_point((59.0, 79.0)).unwrap(), (29.75, 39.75));
    }

    #[test]
    fn input_is_bounded_by_the_region_the_client_saw() {
        let full = geometry(0, 0, 400, 300);
        let region = Rect {
            x: 100,
            y: 100,
            width: 50,
            height: 50,
        };
        let validation = Geometry {
            coordinates: region,
            window: full.window,
            display: full.display,
            context: full.context.clone(),
        };
        assert!(
            Action::Click {
                x: 140,
                y: 140,
                button: 0
            }
            .validate(Some(&validation))
            .is_ok()
        );
        assert_eq!(
            Action::Click {
                x: 10,
                y: 10,
                button: 0
            }
            .validate(Some(&validation))
            .unwrap_err()
            .code,
            "invalid_request",
            "a crop cannot accept input outside the node it showed"
        );
        assert_eq!(
            validation.coordinates, region,
            "the full window geometry stays available for movement checks"
        );
    }

    #[test]
    fn frame_cache_keeps_eight_and_preserves_the_presented_region() {
        let controller = Controller::new(Vec::new()).unwrap();
        let target = Target::Display(1);
        let mut ids = Vec::new();
        for index in 0..9u8 {
            let pixels = RgbaImage::from_pixel(2, 2, image::Rgba([index, 0, 0, 255]));
            let result = controller
                .store_capture(
                    target.clone(),
                    geometry(0, 0, 100, 100),
                    Rect {
                        x: 10,
                        y: 10,
                        width: 2,
                        height: 2,
                    },
                    pixels,
                    "test",
                )
                .unwrap();
            ids.push(result["capture_id"].as_str().unwrap().to_owned());
        }
        assert_eq!(controller.frames.lock().unwrap().len(), 8);
        assert!(
            controller.image(&ids[0]).is_none(),
            "the oldest frame evicted"
        );
        let stored = controller.stored_frame(&ids[8], &target).unwrap();
        assert_eq!(
            stored.region,
            Rect {
                x: 10,
                y: 10,
                width: 2,
                height: 2,
            }
        );
        assert_eq!(
            stored.geometry.coordinates,
            Rect {
                x: 0,
                y: 0,
                width: 100,
                height: 100,
            }
        );
        assert_eq!(
            stored.presented().coordinates,
            Rect {
                x: 10,
                y: 10,
                width: 2,
                height: 2,
            }
        );
        assert_eq!(
            controller
                .stored_frame(&ids[8], &Target::Display(2))
                .unwrap_err()
                .code,
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
        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
    fn ui_app_size_refuses_a_platform_target() {
        let controller = Controller::new(vec!["web,http://127.0.0.1:9515,session".into()]).unwrap();
        let fault = controller
            .execute("get_ui_app_size", json!({"target": "driver:0"}))
            .unwrap_err();
        assert_eq!(fault.code, "unavailable");
        assert!(
            fault.message.contains("platform window"),
            "{}",
            fault.message
        );
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

        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
        let controller = Controller::new(Vec::new()).unwrap();
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
        let node = |id: &str, x: f64, children: Value| {
            json!({
                "id": id,
                "frame": {"x": 0.0, "y": 0.0, "w": 1.0, "h": 1.0},
                "absoluteFrame": {"x": x, "y": 20.0, "w": 30.0, "h": 40.0},
                "flags": {
                    "hitTestable": true, "focusable": false, "clipsToBounds": false,
                    "hasBackground": false, "hasBorder": false,
                },
                "children": children,
            })
        };
        let tree: TreeSnapshot = serde_json::from_value(json!({
            "root": node("root", 0.0, json!([
                node("a", 0.0, json!([])),
                node("b", 1.0, json!([node("c", 2.0, json!([]))])),
            ])),
        }))
        .unwrap();
        assert_eq!(tree.root.as_ref().unwrap().find("c").unwrap().id, "c");
        assert!(tree.root.as_ref().unwrap().find("missing").is_none());
        let frame = node_absolute_frame(&tree, "c").unwrap();
        assert_eq!(
            (frame.x, frame.y, frame.w, frame.h),
            (2.0, 20.0, 30.0, 40.0)
        );
        assert_eq!(
            node_absolute_frame(&tree, "missing").unwrap_err().code,
            "stale_node"
        );
    }
}
