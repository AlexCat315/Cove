use cove_agent::{AgentTool, JsonSchema as S, ui_tools::Tool};

fn object(fields: Vec<(&str, S)>, required: &[&str]) -> S {
    S::object(
        fields
            .into_iter()
            .map(|(name, schema)| (name.into(), schema))
            .collect(),
        required.iter().map(|name| (*name).into()).collect(),
    )
}

fn text() -> S {
    S::string(Some(1), Some(256))
}

fn integer(min: f64, max: f64) -> S {
    S::integer(Some(min), Some(max))
}

fn choice(values: &[&str]) -> S {
    let mut schema = text();
    schema.allowed_values = values
        .iter()
        .map(|value| cove_agent::JsonSchemaScalar::String((*value).into()))
        .collect();
    schema
}

fn tool(
    name: &str,
    description: &str,
    mut fields: Vec<(&str, S)>,
    required: &[&str],
    read_only: bool,
) -> AgentTool {
    if name != "get_ui_session" {
        fields.push(("target", text()));
    }
    AgentTool::new(name, description, object(fields, required), read_only)
}

pub fn definitions() -> Vec<AgentTool> {
    let mut tools: Vec<_> = Tool::ALL
        .into_iter()
        .map(|tool| {
            let mut definition = tool.definition();
            if !matches!(tool, Tool::ListSkills | Tool::ReadSkill) {
                definition.schema.properties.insert("target".into(), text());
            }
            if matches!(tool, Tool::CaptureUiTrace) {
                definition.read_only = false;
            }
            definition
        })
        .collect();
    let point = || integer(-100_000., 100_000.);
    let button = || integer(0., 2.);
    let modifiers = || S::array_of(choice(&["Shift", "Control", "Alt", "Meta"]), Some(4));
    let input_text = || S::string(Some(1), Some(16_384));
    let action = S::choice(vec![
        object(
            vec![
                ("kind", choice(&["click"])),
                ("x", point()),
                ("y", point()),
                ("button", button()),
            ],
            &["kind", "x", "y"],
        ),
        object(
            vec![
                ("kind", choice(&["drag"])),
                ("x", point()),
                ("y", point()),
                ("to_x", point()),
                ("to_y", point()),
            ],
            &["kind", "x", "y", "to_x", "to_y"],
        ),
        object(
            vec![
                ("kind", choice(&["scroll"])),
                ("x", point()),
                ("y", point()),
                ("dx", point()),
                ("dy", point()),
            ],
            &["kind", "x", "y", "dx", "dy"],
        ),
        object(
            vec![
                ("kind", choice(&["key"])),
                ("key", text()),
                ("modifiers", modifiers()),
            ],
            &["kind", "key"],
        ),
        object(
            vec![("kind", choice(&["text"])), ("text", input_text())],
            &["kind", "text"],
        ),
        object(
            vec![("kind", choice(&["activate"])), ("element", text())],
            &["kind", "element"],
        ),
    ]);
    let dimension = || S::number(Some(1.), Some(16_384.));
    let color = || S::string(Some(7), Some(9));
    let style = object(
        vec![
            ("backgroundColor", color()),
            ("foregroundColor", color()),
            (
                "padding",
                object(
                    ["top", "right", "bottom", "left"]
                        .into_iter()
                        .map(|edge| (edge, S::number(Some(0.), Some(4096.))))
                        .collect(),
                    &["top", "right", "bottom", "left"],
                ),
            ),
        ],
        &[],
    );
    let environment = object(
        vec![
            ("clear", S::boolean()),
            ("preset", text()),
            (
                "platform",
                choice(&[
                    "macos", "windows", "linux", "ios", "ipados", "android", "web",
                ]),
            ),
            (
                "designLanguage",
                choice(&["cupertino", "material", "fluent", "adwaita"]),
            ),
            (
                "deviceClass",
                choice(&[
                    "phone",
                    "foldable-book",
                    "foldable-flip",
                    "foldable-dual-screen",
                    "tablet",
                    "desktop",
                ]),
            ),
            ("posture", choice(&["flat", "tabletop", "book"])),
            (
                "displayFeatures",
                S::array_of(
                    object(
                        vec![
                            ("kind", choice(&["fold", "hinge"])),
                            (
                                "bounds",
                                object(
                                    vec![
                                        ("x", point()),
                                        ("y", point()),
                                        ("width", dimension()),
                                        ("height", dimension()),
                                    ],
                                    &["x", "y", "width", "height"],
                                ),
                            ),
                            ("orientation", choice(&["horizontal", "vertical"])),
                            ("occludes", S::boolean()),
                            ("separates", S::boolean()),
                        ],
                        &["kind", "bounds", "orientation"],
                    ),
                    Some(8),
                ),
            ),
        ],
        &[],
    );
    tools.extend([
        tool("get_ui_session", "List connected Cove runtimes, platform windows, displays, and configured WebDriver sessions.", vec![], &[], true),
        tool("bind_ui_target", "Associate a runtime with its window, shared screen, or WebDriver session.", vec![("platform_target",text())], &["target","platform_target"], false),
        tool("inspect_ui_tree", "Read the selected runtime's Cove tree.", vec![], &[], true),
        tool("select_ui_node", "Highlight a Cove node or clear the highlight when node_id is omitted.", vec![("node_id",text())], &[], false),
        tool("capture_ui", "Capture system-composited pixels or the selected browser viewport. The result includes the source and the coordinate space for input. Use scope=display to include the display outside a desktop window.", vec![("scope",choice(&["window","display"]))], &[], true),
        tool("input_ui", "Perform one platform input action. Pointer coordinates use the coordinate space returned by capture_ui and require its capture_id. Delivery does not assert the resulting state.", vec![("action",action),("capture_id",text())], &["action"], false),
        tool("click_ui", "Click a point in the coordinate space of capture_ui. Supply a fresh capture ID and check the postcondition.", vec![("capture_id",text()),("x",point()),("y",point()),("button",button())], &["capture_id","x","y"], false),
        tool("press_ui_key", "Send a key to the selected platform target with optional modifiers.", vec![("key",text()),("modifiers",modifiers())], &["key"], false),
        tool("type_ui_text", "Enter text through the platform's text input operation. For IME behavior use actual key events and observe the candidate window.", vec![("text",input_text())], &["text"], false),
        tool("query_native_ui", "Read the platform accessibility or document tree when the selected adapter exposes it.", vec![], &[], true),
        tool("find_native_ui", "Resolve exactly one platform element by the adapter's selector strategy.", vec![("using",text()),("value",S::string(Some(1),Some(2048)))], &["using","value"], true),
        tool("activate_ui", "Invoke the platform action on an element returned by find_native_ui.", vec![("element",text())], &["element"], false),
        tool("wait_for_ui", "Check a Cove node postcondition. Value checks require node_id and expected.", vec![("node_id",text()),("text",text()),("role",text()),("source",text()),("state",choice(&["visible","hidden","enabled","value"])),("expected",S::string(Some(0),Some(4096))),("timeout_ms",integer(0.,10_000.))], &[], true),
        tool("resize_ui", "Request a runtime window resize in logical points.", vec![("width",dimension()),("height",dimension())], &["width","height"], false),
        tool("set_ui_environment", "Set the selected runtime's simulated environment.", vec![("environment",environment)], &["environment"], false),
        tool("edit_ui_style", "Apply, undo, or clear temporary style overrides by Cove node ID.", vec![
            ("command",choice(&["inspect.style.set","inspect.style.undo","inspect.style.redo","inspect.style.clear","inspect.style.clearAll","inspect.recomposition.reset"])),
            ("arguments",object(vec![("id",text()),("properties",style)],&[])),
        ], &["command","arguments"], false),
        tool("query_ui_logs", "Read the runtime's recent log records and loss counts.", vec![("after",integer(0.,9_007_199_254_740_991.))], &[], true),
        tool("record_ui_timings", "Read recorded CPU frame timings after a sequence number.", vec![("after",integer(0.,9_007_199_254_740_991.))], &[], true),
        tool("start_ui_trace", "Start recording CPU spans in the selected runtime.", vec![], &[], false),
        tool("stop_ui_trace", "Finish the selected runtime's CPU trace and return its summary.", vec![], &[], false),
    ]);
    tools
}
