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
    ]);
    let dimension = || S::number(Some(1.), Some(16_384.));
    let color = || S::string(Some(7), Some(9));
    let flex_factor = || S::number(Some(0.), Some(cove_devtools_protocol::MAX_FLEX_FACTOR));
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
            ("flexGrow", flex_factor()),
            ("flexShrink", flex_factor()),
            (
                "justifyContent",
                choice(&cove_devtools_protocol::JUSTIFY_CONTENT),
            ),
            ("alignItems", choice(&cove_devtools_protocol::ALIGN_ITEMS)),
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
        tool("get_ui_session", "List connected Cove runtimes and their capabilities.", vec![], &[], true),
        tool("inspect_ui_tree", "Read the selected runtime's Cove tree.", vec![], &[], true),
        tool("query_ui_tree", "Read a bounded, real subtree of the live Cove tree. Omit node_id to start at the scene root. depth counts node levels including that root (default 4, max 16); limit caps returned nodes (default 500, max 2000). Returns nodeCount and truncated; nodes cut by depth, limit or the wire byte budget are omitted, never synthesised. Use this instead of inspect_ui_tree for a gallery-sized tree that exceeds the agent result budget.", vec![("node_id",text()),("depth",integer(1.,16.)),("limit",integer(1.,2000.))], &[], true),
        tool("select_ui_node", "Highlight a Cove node or clear the highlight when node_id is omitted.", vec![("node_id",text())], &[], false),
        tool("select_ui_at", "Hit-test the live Cove tree at an image pixel in a capture without dispatching input. Returns the deepest node, its root-first path, and the content-space point used.", vec![("capture_id",text()),("x",integer(0.,100_000.)),("y",integer(0.,100_000.))], &["capture_id","x","y"], true),
        tool("capture_ui_node", "Capture a fresh window image and crop it to a node's absolute frame. Returns a new capture ID and the crop's coordinate space for input.", vec![("node_id",text())], &["node_id"], true),
        tool("capture_ui", "Read a PNG from the selected Cove runtime. Pointer tools take image-relative pixel coordinates and the returned capture ID.", vec![], &[], true),
        tool("input_ui", "Dispatch one action inside the selected Cove runtime. Pointer coordinates use the coordinate space returned by capture_ui and require its capture_id. Delivery does not assert the resulting state.", vec![("action",action),("capture_id",text())], &["action"], false),
        tool("click_ui", "Click a point in the coordinate space of capture_ui. Supply a fresh capture ID and check the postcondition.", vec![("capture_id",text()),("x",point()),("y",point()),("button",button())], &["capture_id","x","y"], false),
        tool("press_ui_key", "Dispatch a key inside the selected Cove runtime.", vec![("key",text()),("modifiers",modifiers())], &["key"], false),
        tool("type_ui_text", "Dispatch committed text to the selected Cove runtime's focused control.", vec![("text",input_text())], &["text"], false),
        tool("wait_for_ui", "Check a Cove node postcondition. Value checks require node_id and expected.", vec![("node_id",text()),("text",text()),("role",text()),("source",text()),("state",choice(&["visible","hidden","enabled","value"])),("expected",S::string(Some(0),Some(4096))),("timeout_ms",integer(0.,10_000.))], &[], true),
        tool("resize_ui", "Request a runtime window resize in logical points.", vec![("width",dimension()),("height",dimension())], &["width","height"], false),
        tool("set_ui_environment", "Set the selected runtime's simulated environment.", vec![("environment",environment)], &["environment"], false),
        tool("edit_ui_style", "Apply, undo, or clear temporary style overrides by Cove node ID: colors, padding, flex grow/shrink and justify/align. Flex overrides write the live LayoutNode and restore the node's original values on clear, undo or disconnect. A null property clears just that property.", vec![
            ("command",choice(&["inspect.style.set","inspect.style.undo","inspect.style.redo","inspect.style.clear","inspect.style.clearAll","inspect.recomposition.reset"])),
            ("arguments",object(vec![("id",text()),("properties",style)],&[])),
        ], &["command","arguments"], false),
        tool("set_ui_debug_paint", "Toggle debug paint guidelines: one-pixel outlines around every visible live node frame. Returns the applied state. Text baselines, repaint highlighting and oversized images are separate toggles (set_ui_baselines, set_ui_repaint_highlight, set_ui_oversized_images); use set_ui_animation_speed for slow animations.", vec![("guidelines",S::boolean())], &["guidelines"], false),
        tool("set_ui_baselines", "Toggle text baselines: one-pixel lines at the measured baseline of every text line on visible nodes, clipped by the root and each node's clipping ancestors. Only nodes with real text layout contribute. Returns the applied state.", vec![("enabled",S::boolean())], &["enabled"], false),
        tool("set_ui_oversized_images", "Toggle oversized-image highlighting: a two-pixel outline around every visible image whose decoded source area is more than four times its painted display area. Images without a known decoded size and vector sources rasterized at the draw size are skipped. Returns the applied state.", vec![("enabled",S::boolean())], &["enabled"], false),
        tool("set_ui_repaint_highlight", "Toggle repaint highlighting: a two-pixel outline around every visible node that has really painted, colored by that node's paint generation. The generation advances only on actual paint execution, so a node retained in the layer cache keeps its color while a repainting node cycles. Returns the applied state.", vec![("enabled",S::boolean())], &["enabled"], false),
        tool("set_ui_animation_speed", "Set the runtime's global animation time scale: 1.0 is real time, 0.5 half speed, 2.0 double speed. Applies to every animation's delta; frame timing, input and diagnostics keep wall-clock time. Returns the applied scale.", vec![("scale",S::number(Some(0.05),Some(10.0)))], &["scale"], false),
        tool("query_ui_logs", "Read the runtime's recent log records and loss counts.", vec![("after",integer(0.,9_007_199_254_740_991.))], &[], true),
        tool("clear_ui_logs", "Empty the runtime's retained log history and reset the loss count for the cleared window. Log sequences keep increasing; records logged afterwards are retained normally.", vec![], &["target"], false),
        tool("set_ui_network_recording", "Pause or resume the runtime's in-process HTTP recorder. Pausing stops new requests from being recorded and keeps the retained history; resuming records new requests again. Returns the state the recorder actually holds.", vec![("enabled",S::boolean())], &["enabled"], false),
        tool("clear_ui_network", "Empty the runtime's retained network records and return how many were removed. The recorder's enabled state is unchanged; requests after the clear are retained normally.", vec![], &["target"], false),
        tool("record_ui_timings", "Read recorded CPU frame timings after a sequence number. Each frame carries the CPU timeline sequence window it owns; frames prepared while no trace capture was open have no window.", vec![("after",integer(0.,9_007_199_254_740_991.))], &[], true),
        tool("analyze_ui_frame", "Classify one recorded frame's dominant CPU phase (recomposition, layout, draw, encode, present) from the trace spans retained inside its timeline window. Returns the spans used, or unavailable with the reason when the frame has no window, no attributable spans, or expired spans. Requires a trace capture around the frame.", vec![("frame",integer(1.,9_007_199_254_740_991.))], &["frame"], true),
        tool("start_ui_trace", "Start recording CPU spans in the selected runtime.", vec![], &[], false),
        tool("stop_ui_trace", "Finish the selected runtime's CPU trace and return its summary.", vec![], &[], false),
        tool("get_ui_app_size", "Measure the selected runtime's own native executable: file bytes, loadable segments with file/VM bytes, their sections with file/VM/zero-fill sizes, top symbols with section, end address and alias counts, and top demangled crate/group sums. Reports attributed, __LINKEDIT and unattributed file bytes separately. This is a native-executable measurement of the running process, not Dart AOT, package attribution or a call graph; remote, mobile and display targets return unavailable.", vec![
            ("segments",integer(1.,crate::app_size::MAX_SEGMENTS as f64)),
            ("sections",integer(1.,crate::app_size::MAX_SECTIONS as f64)),
            ("symbols",integer(1.,crate::app_size::MAX_SYMBOLS as f64)),
            ("groups",integer(1.,crate::app_size::MAX_GROUPS as f64)),
        ], &[], true),
        tool("get_ui_deep_links", "Validate a Cove project's Android intent filters and assetlinks.json plus Apple URL types, associated domains and apple-app-site-association. Pass the project's canonical absolute root. Missing platform files mean no deep links are configured, not an error.", vec![("root",S::string(Some(1),Some(4096)))], &["root"], true),
        tool("get_ui_debugger", "Read debugger availability and the selected runtime's session, stop and breakpoints.", vec![], &[], true),
        tool("attach_ui_debugger", "Attach lldb-dap to the selected runtime process. A denied task_for_pid attach returns unavailable with the adapter's reason; a hung authorization prompt returns timeout. Never launches a process.", vec![("timeout_ms",integer(500.,30_000.))], &[], false),
        tool("detach_ui_debugger", "Detach the UI debugger without terminating the runtime.", vec![], &[], false),
        tool("set_ui_breakpoint", "Set or replace one source breakpoint at an absolute path and line. Returns the adapter's verified flag and resolved line.", vec![("path",S::string(Some(1),Some(4096))),("line",integer(1.,10_000_000.)),("condition",S::string(Some(0),Some(1024)))], &["path","line"], false),
        tool("clear_ui_breakpoint", "Clear one source line, or every breakpoint in the source when line is omitted.", vec![("path",S::string(Some(1),Some(4096))),("line",integer(1.,10_000_000.))], &["path"], false),
        tool("control_ui_debugger", "Pause, resume, or step a stopped debuggee. Pause and step only report stopped after a real stopped event.", vec![("action",choice(&["pause","resume","step_over","step_in","step_out"])),("thread_id",integer(1.,9_007_199_254_740_991.))], &["action"], false),
        tool("get_ui_debug_stack", "Read threads, stack frames and the top frame's scopes. Requires a stopped debuggee and returns not_stopped instead of stale frames.", vec![("thread_id",integer(1.,9_007_199_254_740_991.)),("start_frame",integer(0.,10_000.)),("levels",integer(1.,200.))], &[], true),
        tool("read_ui_debug_variables", "Read a variables reference, or a frame's scopes and their first variables. Pass variables_reference from an earlier scope, variable or evaluate result.", vec![("variables_reference",integer(0.,9_007_199_254_740_991.)),("frame_id",integer(1.,9_007_199_254_740_991.)),("filter",choice(&["named","indexed"])),("start",integer(0.,9_007_199_254_740_991.)),("count",integer(1.,200.))], &[], true),
        tool("evaluate_ui_debug", "Evaluate an expression in a stopped frame and return the adapter's result, type and child reference.", vec![("expression",S::string(Some(1),Some(16_384))),("frame_id",integer(1.,9_007_199_254_740_991.)),("context",choice(&["repl","watch","hover","clipboard"]))], &["expression"], false),
    ]);
    tools
}
