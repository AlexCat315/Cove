//! AI tools over one persistent application connection; no renderer dependencies.
pub mod connection;
mod tools;
mod trace;

use connection::{Connection, failure};
use cove_agent::{
    AgentError, AgentSkill, AgentSkillLibrary, AgentToolResult, AgentToolbox, McpServer,
};
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

pub fn definitions() -> Vec<cove_agent::AgentTool> {
    tools::definitions()
}

fn skill_library() -> Result<AgentSkillLibrary, AgentError> {
    let mut skills = cove_agent::ui_tools::Tools::skills().skills;
    skills.push(AgentSkill::new(
        "ui-automation",
        "Inspect, interact, wait and capture Cove UI without scripts.",
        tools::GUIDE,
    )?);
    AgentSkillLibrary::new(skills)
}

pub fn toolbox(target: impl Into<String>) -> Result<AgentToolbox, AgentError> {
    let mut bridge = Bridge::new(target.into());
    type Invocation = (
        String,
        Value,
        mpsc::SyncSender<Result<AgentToolResult, AgentError>>,
    );
    let (sender, receiver) = mpsc::sync_channel::<Invocation>(1);
    std::thread::Builder::new()
        .name("cove.mcp.connection".into())
        .spawn(move || {
            loop {
                match receiver.recv_timeout(Duration::from_millis(20)) {
                    Ok((name, args, reply)) => {
                        let _ = reply.send(bridge.execute(&name, &args));
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if let Some(peer) = bridge.connection.as_mut() {
                            peer.poll();
                            if !peer.healthy {
                                bridge.connection = None;
                                bridge.tracing = false;
                            }
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        })
        .map_err(|_| failure("Cannot start DevTools connection worker."))?;
    AgentToolbox::new(
        definitions(),
        Arc::new(move |name, arguments| {
            let (reply, result) = mpsc::sync_channel(1);
            sender
                .send((name.to_owned(), arguments.clone(), reply))
                .map_err(|_| failure("DevTools bridge unavailable."))?;
            result
                .recv()
                .map_err(|_| failure("DevTools bridge unavailable."))?
        }),
    )
}

pub fn mcp_server(target: impl Into<String>) -> Result<McpServer, AgentError> {
    Ok(
        McpServer::new("cove-devtools", env!("CARGO_PKG_VERSION"), toolbox(target)?)
            .with_skills(skill_library()?),
    )
}

pub struct Bridge {
    target: String,
    connection: Option<Connection>,
    captures: VecDeque<trace::Capture>,
    capture_id: u64,
    tracing: bool,
}

impl Bridge {
    pub fn new(target: String) -> Self {
        Self {
            target,
            connection: None,
            captures: VecDeque::new(),
            capture_id: 0,
            tracing: false,
        }
    }

    /// Call through `toolbox` to apply input and result bounds/validation.
    fn execute(&mut self, name: &str, arguments: &Value) -> Result<AgentToolResult, AgentError> {
        if name == "list_skills" {
            let skills: Vec<_> = skill_library()?
                .skills
                .iter()
                .map(|skill| json!({"name": skill.name, "description": skill.description}))
                .collect();
            return Ok(AgentToolResult::encode(
                &json!({"ok": true, "data": skills}),
                false,
            ));
        }
        if name == "read_skill" {
            let library = skill_library()?;
            return Ok(AgentToolResult::encode(
                &json!({"ok": true, "data": library.read(arguments["name"].as_str().unwrap_or_default())?}),
                false,
            ));
        }
        if matches!(name, "get_ui_trace" | "compare_ui_traces") {
            return self.read_trace(name, arguments);
        }
        let mut connection = match self.connection.take() {
            Some(connection) => connection,
            None => {
                self.tracing = false;
                Connection::connect(&self.target)?
            }
        };
        let mut result = self.run(&mut connection, name, arguments);
        if matches!(
            name,
            "click_ui" | "fill_ui" | "press_ui_key" | "input_ui" | "activate_ui"
        ) && connection.healthy
            && connection.mirroring
        {
            let stop = connection.request("mirror.stop", None);
            if result.is_ok()
                && let Err(error) = stop
            {
                result = Err(error);
            }
        }
        if connection.healthy {
            self.connection = Some(connection);
        } else {
            self.tracing = false;
        }
        result
    }

    fn run(
        &mut self,
        peer: &mut Connection,
        name: &str,
        args: &Value,
    ) -> Result<AgentToolResult, AgentError> {
        let value = match name {
            "get_ui_session" => {
                json!({"host": peer.hello.host, "protocol": peer.hello.protocol, "capabilities": peer.hello.capabilities})
            }
            "click_ui" | "fill_ui" => {
                let node = resolve(peer, &args["selector"], true)?;
                if name == "fill_ui" {
                    let role = node["role"]
                        .as_str()
                        .unwrap_or_default()
                        .to_ascii_lowercase();
                    if !matches!(
                        role.as_str(),
                        "textfield" | "textarea" | "searchfield" | "securefield"
                    ) || node["state"]
                        .as_array()
                        .is_some_and(|states| states.iter().any(|state| state == "readOnly"))
                    {
                        return Err(failure(
                            "fill_ui requires an editable text field; select its actual semantic node.",
                        ));
                    }
                }
                ensure_input(peer, false)?;
                let frame = &node["frame"];
                let x = coordinate(frame, "x")? + coordinate(frame, "w")? / 2.;
                let y = coordinate(frame, "y")? + coordinate(frame, "h")? / 2.;
                let button = args["button"].as_u64().unwrap_or(0);
                for kind in ["pointerMove", "pointerDown", "pointerUp"] {
                    input(
                        peer,
                        json!({"kind": kind, "x": x, "y": y, "button": button}),
                    )?;
                }
                if name == "fill_ui" {
                    let platform = peer.hello.host.platform.to_ascii_lowercase();
                    let modifier = if platform.contains("mac") || platform.contains("ios") {
                        8
                    } else {
                        2
                    };
                    key(peer, "KeyA", modifier)?;
                    key(peer, "Backspace", 0)?;
                    if let Some(text) = args["text"].as_str().filter(|text| !text.is_empty()) {
                        input(peer, json!({"kind": "text", "text": text}))?;
                    }
                }
                let delivered = json!({"delivered": true, "node_id": node["id"]});
                if args["capture"] == true {
                    return capture(peer, &json!({}), delivered);
                }
                delivered
            }
            "press_ui_key" => {
                let code = args["key"].as_str().unwrap_or_default();
                validate_key(code)?;
                ensure_input(peer, false)?;
                key(peer, code, args["modifiers"].as_u64().unwrap_or(0))?;
                json!({"delivered": true})
            }
            "input_ui" => {
                let events = args["events"]
                    .as_array()
                    .ok_or_else(|| failure("events must be an array."))?;
                validate_events(events)?;
                ensure_input(peer, false)?;
                for event in events {
                    if event["kind"] == "wait" {
                        std::thread::sleep(Duration::from_millis(
                            event["milliseconds"].as_u64().unwrap_or(0),
                        ));
                    } else {
                        let mut event = event.clone();
                        if event["kind"] == "wheel" {
                            event["scroll"] = json!({"unit": "Points"});
                        }
                        input(peer, event)?;
                    }
                }
                json!({"delivered": true, "event_count": events.len()})
            }
            "activate_ui" => {
                let action = args["action"].as_str().unwrap_or("activate");
                let node = resolve(peer, &args["selector"], action != "reveal")?;
                ensure_input(peer, true)?;
                peer.request(
                    "mirror.activate",
                    Some(json!({"node_id": node["id"], "action": action})),
                )?;
                json!({"delivered": true, "node_id": node["id"]})
            }
            "wait_for_ui" => wait_for(peer, args)?,
            "capture_ui" => return capture(peer, args, json!({})),
            "resize_ui" => {
                peer.require("window")?;
                peer.request("window.resize", Some(args.clone()))?;
                json!({"delivered": true})
            }
            "select_ui" => {
                peer.require("select")?;
                if args["clear"] == true {
                    peer.request("select.clear", None)?;
                } else {
                    let node = resolve(peer, &args["selector"], false)?;
                    peer.request("select.node", Some(json!({"id": node["id"]})))?;
                }
                json!({"delivered": true})
            }
            "set_ui_style" => {
                peer.require("style")?;
                let node = resolve(peer, &args["selector"], false)?;
                peer.request(
                    "inspect.style.set",
                    Some(json!({"id": node["id"], "properties": args["properties"]})),
                )?
            }
            "edit_ui_style_history" => {
                peer.require("style")?;
                let action = args["action"].as_str().unwrap_or_default();
                let payload = if action == "clear" {
                    let node = resolve(peer, &args["selector"], false)?;
                    Some(json!({"id": node["id"]}))
                } else {
                    None
                };
                peer.request(&format!("inspect.style.{action}"), payload)?
            }
            "query_ui_logs" => {
                peer.require("log")?;
                let value = peer.request("log.subscribe", Some(args.clone()))?;
                peer.request("log.unsubscribe", None)?;
                value
            }
            "record_ui_timings" => {
                peer.require("timing")?;
                if self.tracing {
                    return Err(failure("Stop the active trace before recording timings."));
                }
                peer.clear_events();
                peer.request("timing.subscribe", None)?;
                let result = trace::collect(
                    peer,
                    "timing.frame",
                    args["duration_ms"].as_u64().unwrap_or(1000),
                );
                let stop = peer.request("timing.unsubscribe", None);
                let (frames, dropped) = result?;
                stop?;
                json!({"samples": frames, "dropped": dropped, "summary": trace::timing_summary(&frames)})
            }
            "start_ui_trace" => {
                self.start_trace(peer)?;
                json!({"recording": true})
            }
            "stop_ui_trace" => self.stop_trace(peer)?,
            "capture_ui_trace" => {
                self.start_trace(peer)?;
                let result = trace::collect(
                    peer,
                    "timeline.events",
                    args["duration_ms"].as_u64().unwrap_or(1000),
                );
                let stop = peer.request("timeline.unsubscribe", None);
                self.tracing = false;
                let (mut batches, dropped) = result?;
                stop?;
                let (tail, tail_dropped) = trace::collect(peer, "timeline.events", 0)?;
                batches.extend(tail);
                self.retain_trace(batches, dropped.max(tail_dropped))
            }
            _ => peer.diagnostic(name, args.clone())?,
        };
        Ok(AgentToolResult::encode(&value, false))
    }

    fn start_trace(&mut self, peer: &mut Connection) -> Result<(), AgentError> {
        peer.require("timeline")?;
        if self.tracing {
            return Err(failure(
                "A trace is already recording; call stop_ui_trace first.",
            ));
        }
        peer.clear_events();
        peer.request("timeline.subscribe", None)?;
        // The initial history is returned by subscribe and intentionally excluded.
        self.tracing = true;
        Ok(())
    }

    fn stop_trace(&mut self, peer: &mut Connection) -> Result<Value, AgentError> {
        if !self.tracing {
            return Err(failure("No trace is recording; call start_ui_trace first."));
        }
        let stop = peer.request("timeline.unsubscribe", None);
        self.tracing = false;
        stop?;
        let (batches, dropped) = trace::collect(peer, "timeline.events", 0)?;
        Ok(self.retain_trace(batches, dropped))
    }

    fn retain_trace(&mut self, batches: Vec<Value>, dropped: usize) -> Value {
        self.capture_id += 1;
        let capture = trace::Capture::new(format!("trace-{}", self.capture_id), batches, dropped);
        let summary = capture.summary();
        if self.captures.len() == 8 {
            self.captures.pop_front();
        }
        self.captures.push_back(capture);
        summary
    }

    fn read_trace(&self, name: &str, args: &Value) -> Result<AgentToolResult, AgentError> {
        let find = |key: &str| {
            self.captures
                .iter()
                .find(|capture| Some(capture.id.as_str()) == args[key].as_str())
                .ok_or_else(|| {
                    failure(format!(
                        "Unknown or expired {key}; captures belong to this MCP process."
                    ))
                })
        };
        let value = if name == "get_ui_trace" {
            let capture = find("capture_id")?;
            let offset = (args["offset"].as_u64().unwrap_or(0) as usize).min(capture.events.len());
            let end =
                (offset + args["limit"].as_u64().unwrap_or(50) as usize).min(capture.events.len());
            json!({"capture_id": capture.id, "items": capture.events[offset..end], "total": capture.events.len(), "nextOffset": (end < capture.events.len()).then_some(end), "dropped": capture.dropped})
        } else {
            trace::compare(find("baseline_id")?, find("candidate_id")?)
        };
        Ok(AgentToolResult::encode(&value, false))
    }
}

fn coordinate(frame: &Value, key: &str) -> Result<f64, AgentError> {
    frame[key]
        .as_f64()
        .filter(|value| value.is_finite() && (!matches!(key, "w" | "h") || *value > 0.))
        .ok_or_else(|| failure("Target has no usable visible bounds."))
}

fn nodes(
    peer: &mut Connection,
    selector: &Value,
    interactable: bool,
    include_value: bool,
    allow_missing: bool,
) -> Result<Vec<Value>, AgentError> {
    let object = selector
        .as_object()
        .ok_or_else(|| failure("selector must be an object."))?;
    if !object
        .values()
        .any(|value| value.as_str().is_some_and(|value| !value.is_empty()))
    {
        return Err(failure(
            "Provide node_id, label, text, role or source in selector.",
        ));
    }
    let mut query = selector.clone();
    query.as_object_mut().unwrap().remove("label");
    if query.get("text").is_none_or(Value::is_null)
        && let Some(label) = selector["label"].as_str()
    {
        query["text"] = json!(label);
    }
    query["compact"] = json!(true);
    query["visible_only"] = json!(interactable);
    query["input_enabled_only"] = json!(interactable);
    query["include_value"] = json!(include_value);
    query["limit"] = json!(100);
    let mut found = Vec::new();
    for _ in 0..10 {
        let response = peer.poll_nodes(query.clone(), allow_missing)?;
        let page = &response["data"];
        let items = page["items"]
            .as_array()
            .ok_or_else(|| failure("Malformed node query response."))?;
        found.extend(
            items
                .iter()
                .filter(|node| {
                    selector["label"]
                        .as_str()
                        .is_none_or(|label| node["label"] == label)
                })
                .cloned(),
        );
        match page["nextOffset"].as_u64() {
            Some(offset) => query["offset"] = json!(offset),
            None => return Ok(found),
        }
    }
    Err(failure(
        "Selector scans over 1000 nodes; narrow it with role, text, label or node_id.",
    ))
}

fn resolve(
    peer: &mut Connection,
    selector: &Value,
    interactable: bool,
) -> Result<Value, AgentError> {
    let mut matches = nodes(peer, selector, interactable, false, false)?;
    if matches.len() != 1 {
        let ids: Vec<_> = matches
            .iter()
            .take(8)
            .map(|node| node["id"].clone())
            .collect();
        return Err(failure(format!(
            "Selector matched {} nodes; expected one. Candidate ids: {ids:?}. Refine the selector.",
            matches.len()
        )));
    }
    Ok(matches.remove(0))
}

fn ensure_input(peer: &mut Connection, accessibility: bool) -> Result<(), AgentError> {
    peer.require(if accessibility {
        "accessibility"
    } else {
        "input"
    })?;
    peer.request("mirror.start", Some(json!({"fps": 1})))?;
    Ok(())
}
fn input(peer: &mut Connection, event: Value) -> Result<(), AgentError> {
    peer.request("mirror.input", Some(event))?;
    Ok(())
}
fn validate_key(code: &str) -> Result<(), AgentError> {
    if cove_devtools_protocol::key_scancode(Some(code)).is_none() {
        return Err(failure("Unsupported KeyboardEvent.code."));
    }
    Ok(())
}
fn key(peer: &mut Connection, code: &str, modifiers: u64) -> Result<(), AgentError> {
    for kind in ["keyDown", "keyUp"] {
        input(
            peer,
            json!({"kind": kind, "key": code, "modifiers": modifiers}),
        )?;
    }
    Ok(())
}
fn validate_events(events: &[Value]) -> Result<(), AgentError> {
    if events.is_empty() {
        return Err(failure("events must not be empty."));
    }
    let mut wait = 0;
    for event in events {
        match event["kind"].as_str().unwrap_or_default() {
            "wait" => {
                wait += event["milliseconds"]
                    .as_u64()
                    .ok_or_else(|| failure("wait requires milliseconds."))?;
            }
            "keyDown" | "keyUp" => validate_key(event["key"].as_str().unwrap_or_default())?,
            "text" if event["text"].as_str().is_some_and(|text| !text.is_empty()) => {}
            "externalDrop" => {
                for name in ["x", "y"] {
                    if event[name].as_f64().is_none() {
                        return Err(failure("externalDrop events require x and y."));
                    }
                }
                match event["phase"].as_str().unwrap_or_default() {
                    "begin" | "position" | "complete" => {}
                    "file" => {
                        let paths = event["paths"]
                            .as_array()
                            .ok_or_else(|| failure("a file drop requires paths."))?;
                        if paths.is_empty() || paths.len() > 256 {
                            return Err(failure("a file drop carries 1 to 256 paths."));
                        }
                        if paths.iter().any(|path| {
                            path.as_str()
                                .is_none_or(|path| path.is_empty() || path.len() > 4096)
                        }) {
                            return Err(failure("each dropped path is 1 to 4096 bytes."));
                        }
                    }
                    "text" if event["text"].as_str().is_some_and(|text| !text.is_empty()) => {}
                    _ => {
                        return Err(failure(
                            "externalDrop requires phase begin|position|complete, or file with paths, or text with text.",
                        ));
                    }
                }
            }
            "pointerMove" | "pointerDown" | "pointerUp" | "wheel" => {
                for name in ["x", "y"] {
                    if event[name].as_f64().is_none() {
                        return Err(failure("Pointer and wheel events require x and y."));
                    }
                }
                if event["kind"] == "wheel"
                    && event["deltaX"].as_f64().is_none()
                    && event["deltaY"].as_f64().is_none()
                {
                    return Err(failure("wheel requires deltaX or deltaY."));
                }
            }
            _ => return Err(failure("Invalid input event.")),
        }
    }
    if wait > 10_000 {
        return Err(failure("Input sequence exceeds 10000 ms total wait."));
    }
    Ok(())
}

fn wait_for(peer: &mut Connection, args: &Value) -> Result<Value, AgentError> {
    let state = args["state"].as_str().unwrap_or("visible");
    if state == "value"
        && (args["selector"]["node_id"].as_str().is_none() || args["expected"].as_str().is_none())
    {
        return Err(failure(
            "Waiting for value requires selector.node_id and expected.",
        ));
    }
    let deadline =
        Instant::now() + Duration::from_millis(args["timeout_ms"].as_u64().unwrap_or(3000));
    loop {
        let matches = nodes(
            peer,
            &args["selector"],
            false,
            state == "value",
            state == "hidden",
        )?;
        let visible = |node: &Value| node["frame"].is_object();
        let met = match state {
            "hidden" => !matches.iter().any(visible),
            "enabled" => {
                matches.len() == 1 && visible(&matches[0]) && matches[0]["inputEnabled"] == true
            }
            "value" => {
                matches.len() == 1
                    && matches[0]["value"].as_str().unwrap_or("")
                        == args["expected"].as_str().unwrap_or_default()
            }
            _ => matches.len() == 1 && visible(&matches[0]),
        };
        if met {
            return Ok(json!({"matched": true, "items": matches}));
        }
        if Instant::now() >= deadline {
            return Err(failure(format!(
                "Timed out waiting for UI {state}. Last observation: {}",
                json!(matches)
            )));
        }
        std::thread::sleep(
            Duration::from_millis(50).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
}

fn capture(
    peer: &mut Connection,
    args: &Value,
    mut metadata: Value,
) -> Result<AgentToolResult, AgentError> {
    peer.require("mirror")?;
    if let Some(delay) = args["settle_ms"].as_u64() {
        std::thread::sleep(Duration::from_millis(delay));
    }
    if peer.mirroring {
        peer.request("mirror.stop", None)?;
    }
    peer.discard_events("mirror.frame");
    peer.request("mirror.start", Some(json!({"fps": 30, "quality": args["quality"].as_f64().unwrap_or(0.7), "scale": args["scale"].as_f64().unwrap_or(1.)})))?;
    peer.discard_events("mirror.stopped");
    let result = peer.wait_event("mirror.frame", Instant::now() + Duration::from_secs(15));
    let stop = peer.request("mirror.stop", None);
    let mut frame = result?
        .ok_or_else(|| failure("No mirror frame within 15 seconds; inspect host renderer logs."))?;
    stop?;
    let data = frame
        .as_object_mut()
        .and_then(|frame| frame.remove("jpegBase64"))
        .and_then(|value| value.as_str().map(str::to_owned))
        .ok_or_else(|| failure("Mirror frame has no JPEG data."))?;
    if data.len() > cove_agent::MAX_TOOL_IMAGE_BYTES {
        return Err(failure(
            "Screenshot exceeds 1 MiB encoded; lower quality or scale.",
        ));
    }
    metadata["frame"] = frame;
    metadata["settle_ms"] = json!(args["settle_ms"].as_u64().unwrap_or(0));
    Ok(AgentToolResult::encode(&metadata, false).with_image(data, "image/jpeg"))
}
