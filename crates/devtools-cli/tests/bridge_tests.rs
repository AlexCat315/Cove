use cove_agent::AgentToolbox;
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, AccessibilityValueProvider, Node, NodeTree,
};
use cove_devtools::devtools::DevTools;
use cove_devtools::server::{DevToolsConfig, Outbound};
use cove_devtools_protocol::{DevToolsLaunch, MirrorFrame};
use serde_json::{Value, json};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;

struct Host {
    target: String,
    field: String,
    inputs: Arc<Mutex<Vec<cove_platform::InputEvent>>>,
    exit: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    _directory: tempfile::TempDir,
}
impl Host {
    fn start() -> Self {
        Self::with_mirror(true)
    }
    fn with_mirror(mirror: bool) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("session.json");
        let target = format!("@{}", path.display());
        let exit = Arc::new(AtomicBool::new(false));
        let done = exit.clone();
        let inputs = Arc::new(Mutex::new(Vec::new()));
        let recorded = inputs.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        let thread = std::thread::spawn(move || {
            use std::{cell::RefCell, rc::Rc};
            let root = Node::new();
            root.set_frame(cove_core::Rect::new(0., 0., 400., 300.));
            for (index, label) in ["Save", "Duplicate", "Duplicate"].into_iter().enumerate() {
                let button = Node::new();
                button.set_frame(cove_core::Rect::new(
                    10.,
                    10. + 50. * index as f32,
                    100.,
                    40.,
                ));
                button.set_is_hit_testable(true);
                button.set_accessibility(Some(
                    AccessibilitySemantics::new(AccessibilityRole::Button).with_label(label),
                ));
                root.add_child(&button);
            }
            let field = Node::new();
            field.set_frame(cove_core::Rect::new(10., 180., 200., 40.));
            field.set_is_hit_testable(true);
            field.set_accessibility(Some(
                AccessibilitySemantics::new(AccessibilityRole::TextField).with_label("Name"),
            ));
            let value = Rc::new(RefCell::new("old".to_owned()));
            let reading = value.clone();
            field.set_accessibility_value_provider(Some(AccessibilityValueProvider::new(
                1,
                move || reading.borrow().clone(),
            )));
            root.add_child(&field);
            let mut tree = NodeTree::new();
            tree.set_root(Some(root));
            let tree = Rc::new(RefCell::new(tree));
            let config = DevToolsConfig {
                port: 0,
                launch: Some(DevToolsLaunch {
                    id: "test".into(),
                    token: "private-test-token".into(),
                    descriptor_path: path.to_string_lossy().into_owned(),
                    app_title: "MCP test".into(),
                }),
                ..DevToolsConfig::default()
            };
            let mut devtools = DevTools::new(config, tree.clone(), None, None);
            let timeline = tree.borrow().timeline.clone();
            devtools.start().unwrap();
            devtools.set_input_delivery(Some(Rc::new(move |event| {
                if let cove_platform::InputEvent::TextInput(text) = &event {
                    *value.borrow_mut() = text.clone();
                }
                if let cove_platform::InputEvent::KeyDown(key) = &event
                    && key.scancode == 42
                {
                    value.borrow_mut().clear();
                }
                let start = timeline.borrow().begin();
                timeline
                    .borrow_mut()
                    .end(start, "input", "delivered", None, &[]);
                recorded.lock().unwrap().push(event);
            })));
            // Deterministic mirror provider exercises the actual server/bridge image path.
            let outbound = devtools.server.outbound();
            let sequence = Rc::new(std::cell::Cell::new(0));
            if mirror {
                devtools
                    .server
                    .advertised_capabilities
                    .push("mirror".into());
            }
            devtools.server.host.mirror.start = Some(Rc::new(move |_| {
                sequence.set(sequence.get() + 1);
                outbound
                    .borrow_mut()
                    .push(Outbound::MirrorFrame(MirrorFrame {
                        seq: sequence.get(),
                        width: 2,
                        height: 2,
                        logical_width: 2.,
                        logical_height: 2.,
                        jpeg_base64: "/9j/2Q==".into(),
                    }));
            }));
            sender.send(field.id().0.to_string()).unwrap();
            while !done.load(Ordering::Relaxed) {
                devtools.pump();
                std::thread::sleep(Duration::from_millis(1));
            }
            devtools.stop();
        });
        let field = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
        Self {
            target,
            field,
            inputs,
            exit,
            thread: Some(thread),
            _directory: directory,
        }
    }
    fn tools(&self) -> AgentToolbox {
        cove_devtools_cli::toolbox(self.target.clone()).unwrap()
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        self.exit.store(true, Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap();
    }
}
fn call(tools: &AgentToolbox, name: &str, args: Value) -> Value {
    tools.execute(name, &args).unwrap().value().unwrap()
}

#[test]
fn real_authenticated_host_query_click_fill_wait_and_validate_before_delivery() {
    let host = Host::start();
    let tools = host.tools();
    let session = call(&tools, "get_ui_session", json!({}));
    assert!(
        session["capabilities"]
            .as_array()
            .unwrap()
            .contains(&json!("input"))
    );
    assert!(!session.to_string().contains("private-test-token"));
    let page = call(
        &tools,
        "query_ui_nodes",
        json!({"role": "button", "text": "Save"}),
    );
    assert_eq!(page["data"]["total"], 1);
    assert!(
        page["data"]["items"][0]["id"].is_string(),
        "compact by default"
    );
    call(
        &tools,
        "click_ui",
        json!({"selector": {"label": "Save", "role": "button"}}),
    );
    assert_eq!(host.inputs.lock().unwrap().len(), 3);
    assert!(
        tools
            .execute("click_ui", &json!({"selector": {"label": "Duplicate"}}))
            .is_err()
    );
    assert!(tools.execute("input_ui", &json!({"events": [{"kind": "text", "text": "must not arrive"}, {"kind": "keyDown", "key": "Invalid"}]})).is_err());
    assert_eq!(host.inputs.lock().unwrap().len(), 3);
    call(
        &tools,
        "fill_ui",
        json!({"selector": {"node_id": host.field}, "text": "上海 🌊"}),
    );
    let value = call(
        &tools,
        "wait_for_ui",
        json!({"selector": {"node_id": host.field}, "state": "value", "expected": "上海 🌊", "timeout_ms": 1000}),
    );
    assert_eq!(value["items"][0]["value"], "上海 🌊");
    assert_eq!(value["items"][0]["valueTruncated"], false);
    assert!(tools.execute("wait_for_ui", &json!({"selector": {"node_id": host.field}, "state": "value", "expected": "never", "timeout_ms": 0})).is_err());
    call(
        &tools,
        "input_ui",
        json!({"events": [{"kind": "wheel", "x": 10, "y": 20, "deltaY": 50}]}),
    );
    assert!(host.inputs.lock().unwrap().iter().any(|event| matches!(event, cove_platform::InputEvent::MouseWheel(wheel) if wheel.input.unit == cove_platform::ScrollUnit::Points)));
}

#[test]
fn mcp_images_are_content_blocks_and_repeated_capture_is_fresh() {
    let host = Host::start();
    let server = cove_devtools_cli::mcp_server(host.target.clone()).unwrap();
    for id in 1..=3 {
        let response: Value = serde_json::from_slice(&server.respond(&serde_json::to_vec(&json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": "capture_ui", "arguments": {}}
        })).unwrap()).unwrap()).unwrap();
        assert_eq!(response["result"]["isError"], false, "{response}");
        assert_eq!(response["result"]["content"][1]["type"], "image");
        assert_eq!(response["result"]["content"][1]["mimeType"], "image/jpeg");
        assert_eq!(response["result"]["structuredContent"]["frame"]["seq"], id);
        assert!(
            !response["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("/9j/2Q==")
        );
    }
}

#[test]
fn trace_spans_real_actions_across_calls_and_retains_paged_events() {
    let host = Host::start();
    let tools = host.tools();
    call(&tools, "start_ui_trace", json!({}));
    assert!(tools.execute("start_ui_trace", &json!({})).is_err());
    call(
        &tools,
        "click_ui",
        json!({"selector": {"label": "Save", "role": "button"}}),
    );
    std::thread::sleep(Duration::from_millis(30));
    let capture = call(&tools, "stop_ui_trace", json!({}));
    assert!(capture["event_count"].as_u64().unwrap() >= 3, "{capture}");
    let events = call(
        &tools,
        "get_ui_trace",
        json!({"capture_id": capture["capture_id"], "limit": 1}),
    );
    assert_eq!(events["items"].as_array().unwrap().len(), 1);
    assert_eq!(events["nextOffset"], 1);
    let idle = call(&tools, "capture_ui_trace", json!({"duration_ms": 50}));
    assert_eq!(
        idle["event_count"], 0,
        "idle recording must not create fake work"
    );
    let comparison = call(
        &tools,
        "compare_ui_traces",
        json!({"baseline_id": capture["capture_id"], "candidate_id": idle["capture_id"]}),
    );
    assert_eq!(comparison["candidate"]["event_count"], 0);
}

#[test]
fn executable_stdio_discovers_tools_without_a_running_app() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new(env!("CARGO_BIN_EXE_cove-devtools"))
        .args(["--endpoint", "1", "mcp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"prompts/get\",\"params\":{\"name\":\"ui-automation\"}}\n"
    ).as_bytes()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let lines: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(
        lines.len(),
        3,
        "stdout contains only responses; notifications produce no output"
    );
    assert!(
        lines[1]["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["name"] == "click_ui")
    );
    assert!(
        lines[2]["result"]["messages"][0]["content"]["text"]
            .as_str()
            .unwrap()
            .contains("without scripts")
            || lines[2]["result"]["messages"][0]["content"]["text"]
                .as_str()
                .unwrap()
                .contains("do not write WebSocket scripts")
    );
    for tool in cove_devtools_cli::definitions() {
        assert!(tool.schema.is_strict_capability_input(), "{}", tool.name);
    }
}

#[test]
fn input_without_mirror_and_temporary_styles_share_the_session() {
    let host = Host::with_mirror(false);
    let tools = host.tools();
    assert!(tools.execute("capture_ui", &json!({})).is_err());
    call(
        &tools,
        "click_ui",
        json!({"selector": {"label": "Save", "role": "button"}}),
    );
    assert_eq!(host.inputs.lock().unwrap().len(), 3);
    call(
        &tools,
        "select_ui",
        json!({"selector": {"node_id": host.field}}),
    );
    call(
        &tools,
        "set_ui_style",
        json!({"selector": {"node_id": host.field}, "properties": {"backgroundColor": "#123456"}}),
    );
    let page = call(
        &tools,
        "query_ui_nodes",
        json!({"node_id": host.field, "compact": false}),
    );
    assert_eq!(
        page["data"]["items"][0]["node"]["style"]["backgroundColor"],
        "#123456ff"
    );
    call(&tools, "edit_ui_style_history", json!({"action": "undo"}));
    let page = call(
        &tools,
        "query_ui_nodes",
        json!({"node_id": host.field, "compact": false}),
    );
    assert_ne!(
        page["data"]["items"][0]["node"]["style"]["backgroundColor"],
        "#123456ff"
    );
    call(&tools, "edit_ui_style_history", json!({"action": "redo"}));
    call(
        &tools,
        "edit_ui_style_history",
        json!({"action": "clearAll"}),
    );
    let hidden = call(
        &tools,
        "wait_for_ui",
        json!({"selector": {"node_id": "unmounted"}, "state": "hidden", "timeout_ms": 0}),
    );
    assert_eq!(hidden["matched"], true);
}
