use cove_agent::{AgentTool, JsonSchema as S};
use std::collections::BTreeMap;

fn object(fields: Vec<(&str, S)>, required: &[&str]) -> S {
    S::object(
        fields
            .into_iter()
            .map(|(name, schema)| (name.into(), schema))
            .collect::<BTreeMap<_, _>>(),
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
    fields: Vec<(&str, S)>,
    required: &[&str],
    read_only: bool,
) -> AgentTool {
    AgentTool::new(name, description, object(fields, required), read_only)
}
fn selector() -> S {
    object(
        vec![
            ("node_id", text()),
            ("text", text()),
            ("label", text()),
            ("role", text()),
            ("source", text()),
        ],
        &[],
    )
}
pub fn definitions() -> Vec<AgentTool> {
    let mut tools: Vec<_> = cove_agent::ui_tools::Tools::tools();
    tools.extend([
        tool("get_ui_session", "Connect to the app and read its actual host capabilities. Available before the app starts; failed calls reconnect next time. No OS automation or scripts needed.", vec![], &[], true),
        tool("click_ui", "Click a unique visible, enabled node by selector. label is exact; text is substring. Ambiguous or stale targets fail. capture=true returns a fresh image on the same connection. Delivery alone does not assert resulting app state.", vec![("selector", selector()), ("button", integer(0., 4.)), ("capture", S::boolean())], &["selector"], false),
        tool("fill_ui", "Focus a unique visible editable text field and replace its text using the remote platform's select-all shortcut. Values are not echoed. Verify using query_ui_nodes with node_id and include_value=true.", vec![("selector", selector()), ("text", S::string(Some(0), Some(16_384))), ("capture", S::boolean())], &["selector", "text"], false),
        tool("press_ui_key", "Press and release a browser KeyboardEvent.code (Enter, Escape, KeyA, ArrowDown...). Modifier bits: shift=1, ctrl=2, alt=4, meta=8. Unsupported codes fail before delivery.", vec![("key", text()), ("modifiers", integer(0., 15.))], &["key"], false),
        tool("input_ui", "Deliver a bounded sequence on one connection for dragging, scrolling and keyboard/text input. The whole sequence is validated first. Waits are limited to 2000 ms each and 10000 ms total. Coordinates are logical points; wheel deltas use the host's point units.", vec![("events", S::array_of(object(vec![
            ("kind", choice(&["pointerMove", "pointerDown", "pointerUp", "wheel", "keyDown", "keyUp", "text", "wait", "externalDrop"])),
            ("x", S::number(Some(-100_000.), Some(100_000.))), ("y", S::number(Some(-100_000.), Some(100_000.))),
            ("deltaX", S::number(Some(-100_000.), Some(100_000.))), ("deltaY", S::number(Some(-100_000.), Some(100_000.))),
            ("button", integer(0., 4.)), ("key", text()), ("text", S::string(Some(1), Some(16_384))),
            ("modifiers", integer(0., 15.)), ("milliseconds", integer(0., 2000.)),
            ("phase", choice(&["begin", "file", "text", "position", "complete"])),
            ("paths", S::array_of(S::string(Some(1), Some(4096)), Some(256))),
            ("source", S::string(Some(0), Some(256))),
        ], &["kind"]), Some(256)))], &["events"], false),
        tool("activate_ui", "Perform activate/increment/decrement/reveal using a node's accessibility action. Reveal can target offscreen or disabled nodes. Other actions require enabled nodes; ambiguous selectors fail.", vec![("selector", selector()), ("action", choice(&["activate", "increment", "decrement", "reveal"]))], &["selector"], false),
        tool("wait_for_ui", "Poll a selector until visible, hidden, enabled or an exact field value is observed. Returns the matching nodes or a timeout error with the last observation. value requires selector.node_id and expected; secure values stay redacted.", vec![("selector", selector()), ("state", choice(&["visible", "hidden", "enabled", "value"])), ("expected", S::string(Some(0), Some(4096))), ("timeout_ms", integer(0., 10_000.))], &["selector"], true),
        tool("capture_ui", "Return a fresh renderer screenshot as MCP image content with logical/pixel dimensions. settle_ms is an optional animation sampling delay, not a condition assertion. Does not include OS window chrome.", vec![("scale", S::number(Some(1.), Some(3.))), ("quality", S::number(Some(0.1), Some(1.))), ("settle_ms", integer(0., 2000.))], &[], true),
        tool("resize_ui", "Resize the application viewport in logical points. Query resulting layout or capture_ui to verify the actual size.", vec![("width", S::number(Some(1.), Some(16_384.))), ("height", S::number(Some(1.), Some(16_384.)))], &["width", "height"], false),
        tool("select_ui", "Select a unique node in the runtime inspector overlay, or clear this connection's selection.", vec![("selector", selector()), ("clear", S::boolean())], &[], false),
        tool("set_ui_style", "Set temporary inspector overrides on a unique node. Colors use #RRGGBB or #RRGGBBAA; null resets a property. Overrides remain through recomposition and are discarded on disconnect.", vec![("selector", selector()), ("properties", object(vec![
            ("padding", object(vec![("top", S::number(Some(0.), Some(4096.))), ("right", S::number(Some(0.), Some(4096.))), ("bottom", S::number(Some(0.), Some(4096.))), ("left", S::number(Some(0.), Some(4096.)))], &["top", "right", "bottom", "left"])),
            ("backgroundColor", S::string(Some(7), Some(9))), ("foregroundColor", S::string(Some(7), Some(9)))
        ], &[]))], &["selector", "properties"], false),
        tool("edit_ui_style_history", "Undo/redo temporary styles or clear this connection's overrides. clear requires selector; clearAll removes all of this inspector session's overrides.", vec![("action", choice(&["undo", "redo", "clear", "clearAll"])), ("selector", selector())], &["action"], false),
        tool("query_ui_logs", "Read bounded host log history with level/text/module filters and explicit loss counts.", vec![("minimum", choice(&["trace", "debug", "info", "notice", "warning", "error", "critical"])), ("text", S::string(Some(0), Some(4096))), ("labels", S::array_of(text(), Some(64)))], &[], true),
        tool("record_ui_timings", "Collect actual frame timings for 50–5000 ms. Returns bounded samples and CPU summaries. Idle apps may produce no frames; CPU throughput is not display FPS or GPU duration.", vec![("duration_ms", integer(50., 5000.))], &[], false),
        tool("start_ui_trace", "Start bounded CPU timeline capture, then perform real UI actions and call stop_ui_trace. Captures remain subscribed between tool calls on this connection.", vec![], &[], false),
        tool("stop_ui_trace", "Stop the active CPU timeline capture and retain its bounded events. Returns a capture id, phase summary and loss counts. At most eight captures survive in this MCP session.", vec![], &[], false),
    ]);
    tools
}

pub const GUIDE: &str = "Use get_ui_session and get_diagnostics first. Query query_ui_nodes (compact is the default here) with text/role/source, then use click_ui or fill_ui with a unique selector; label matches exactly. Never guess coordinates or silently pick the first of several matches. Use input_ui for drag/scroll sequences and press_ui_key for shortcuts. Use wait_for_ui for observable postconditions, then capture_ui to inspect rendering; delivery acknowledgements are not behavior assertions. capture=true on click_ui/fill_ui returns an image on the same connection for transient states. Field values require explicit node_id and include_value=true; secure providers are never read. Start a CPU trace with start_ui_trace, perform the real operation, stop_ui_trace, then get_ui_trace or compare_ui_traces. capture_ui_trace records a fixed window; it does not create work. Missing host capabilities and idle or dropped samples must be reported. Nodes expire on unmount. The bridge reconnects on the next call after a transport failure and never retries mutations. Configure the cove-devtools mcp command once in the MCP client; do not write WebSocket scripts for these tools.";
