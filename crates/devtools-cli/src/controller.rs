use crate::platform::{Action, Geometry, ListedTarget, Platform, Scope, Target};
use crate::registry::{Registry, random_id};
use cove_agent::{AgentTool, JsonSchemaValidator, ui_tools::Tool};
use cove_devtools_protocol::control::{Fault, ProbeCommand};
use serde_json::{Value, json};
use std::collections::{HashMap, VecDeque};
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct Frame {
    id: String,
    target: Target,
    geometry: Geometry,
    png: Arc<Vec<u8>>,
    time: Instant,
}

pub struct Controller {
    pub registry: Registry,
    platform: Platform,
    tools: Vec<AgentTool>,
    frames: Mutex<VecDeque<Frame>>,
    bindings: Mutex<HashMap<String, Target>>,
    input: Mutex<()>,
}

fn invalid(message: impl Into<String>) -> Fault {
    Fault::new("invalid_request", message)
}

impl Controller {
    pub fn new(drivers: Vec<String>) -> Result<Self, Fault> {
        Ok(Self {
            registry: Registry::default(),
            platform: Platform::new(drivers)?,
            tools: crate::tools::definitions(),
            frames: Mutex::new(VecDeque::new()),
            bindings: Mutex::new(HashMap::new()),
            input: Mutex::new(()),
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
            "select_ui_node" => self.probe(
                target,
                ProbeCommand::Select {
                    node_id: args["node_id"].as_str().map(str::to_owned),
                },
            )?,
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
            "query_ui_logs" => self.probe(
                target,
                ProbeCommand::Logs {
                    after: args["after"].as_u64(),
                },
            )?,
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
        let id = random_id()?;
        let width = capture.pixels.width();
        let height = capture.pixels.height();
        let mut png = Cursor::new(Vec::new());
        capture
            .pixels
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
            "capture_id": id, "resource": format!("/captures/{id}"), "source": capture.source,
            "width": width, "height": height, "coordinates": capture.geometry.coordinates,
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
            geometry: capture.geometry,
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

    fn input(&self, target: Target, capture: Option<&str>, action: Action) -> Result<Value, Fault> {
        let geometry = capture
            .map(|id| {
                let frames = self.frames.lock().unwrap();
                frames
                    .iter()
                    .find(|frame| {
                        frame.id == id
                            && frame.target == target
                            && frame.time.elapsed() < Duration::from_secs(120)
                    })
                    .map(|frame| frame.geometry.clone())
                    .ok_or_else(|| {
                        Fault::new(
                            "stale_capture",
                            "Capture expired or belongs to another target.",
                        )
                    })
            })
            .transpose()?;
        let _writer = self
            .input
            .try_lock()
            .map_err(|_| Fault::new("busy", "Another input action is in progress."))?;
        let source = self.platform.perform(&target, action, geometry.as_ref())?;
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
