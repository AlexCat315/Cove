//! Commands available inside every Cove runtime, independent of its transport.
use crate::inspection::{DiagnosticStateSource, Diagnostics, SceneInspector, Tool};
use cove_core::scene::{InvalidationLog, NodeTree, RenderTree};
use cove_devtools_protocol::control::{Fault, ProbeCommand};
use cove_devtools_protocol::{Envelope, TimingFrame};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;

pub type Resize = Rc<dyn Fn(f64, f64) -> Result<(), String>>;
pub type Environment = Rc<dyn Fn(Value) -> Result<Value, String>>;

pub struct RuntimeProbe {
    diagnostics: Diagnostics,
    pub scene: SceneInspector,
    pub resize: Option<Resize>,
    pub environment: Option<Environment>,
    timing: VecDeque<TimingFrame>,
    sequence: u64,
}

impl RuntimeProbe {
    pub fn new(
        tree: Rc<RefCell<NodeTree>>,
        invalidation: Option<Arc<InvalidationLog>>,
        render_tree: Option<RenderTree>,
        state: Option<Rc<dyn DiagnosticStateSource>>,
    ) -> Self {
        Self {
            diagnostics: Diagnostics::with_state_source(tree.clone(), state),
            scene: SceneInspector::new(tree, invalidation, render_tree),
            resize: None,
            environment: None,
            timing: VecDeque::new(),
            sequence: 0,
        }
    }

    pub fn diagnostics(&self) -> &Diagnostics {
        &self.diagnostics
    }

    pub fn capabilities(&self) -> Vec<String> {
        let mut capabilities = vec!["tree", "diagnostics", "style", "trace", "timing", "network"];
        if self.resize.is_some() {
            capabilities.push("resize");
        }
        if self.environment.is_some() {
            capabilities.push("environment");
        }
        capabilities.into_iter().map(str::to_owned).collect()
    }

    pub fn record(&mut self, mut frame: TimingFrame) {
        self.sequence += 1;
        frame.frame = self.sequence;
        if self.timing.len() == 240 {
            self.timing.pop_front();
        }
        self.timing.push_back(frame);
    }

    pub fn execute(&mut self, command: ProbeCommand) -> Result<Value, Fault> {
        let invalid = |message: String| Fault::new("invalid_request", message);
        let unavailable = || {
            Fault::new(
                "unavailable",
                "This runtime has no adapter for the operation.",
            )
        };
        let serialize =
            |result: serde_json::Result<Value>| result.map_err(|error| invalid(error.to_string()));
        match command {
            ProbeCommand::Tree => serialize(serde_json::to_value(self.scene.snapshot())),
            ProbeCommand::Select { node_id } => {
                if !self.scene.editor.select(node_id.as_deref()) {
                    return Err(Fault::new(
                        "stale_node",
                        "The selected component is no longer in the scene.",
                    ));
                }
                serialize(serde_json::to_value(self.scene.editor.state()))
            }
            ProbeCommand::Read { name, arguments } => {
                let tool = Tool::parse(&name)
                    .filter(|tool| tool.is_immediate_read())
                    .ok_or_else(|| Fault::new("unknown_command", "Unknown diagnostic command."))?;
                self.diagnostics
                    .execute_read_json(tool, &arguments)
                    .map_err(|error| invalid(error.to_string()))
            }
            ProbeCommand::Edit { name, arguments } => {
                use cove_devtools_protocol::InspectionValidation;
                if !InspectionValidation::is_command(&name) {
                    return Err(Fault::new("unknown_command", "Unknown inspection command."));
                }
                let request = Envelope::new(name, Some(1), Some(arguments));
                let result = self.scene.editor.handle(&request);
                let payload = result.payload.unwrap_or(Value::Null);
                if result.r#type.ends_with(".err") {
                    return Err(Fault::new(
                        payload["code"].as_str().unwrap_or("invalid_request"),
                        payload["message"].as_str().unwrap_or("Inspection failed."),
                    ));
                }
                Ok(payload)
            }
            ProbeCommand::Resize { width, height } => {
                let size = cove_devtools_protocol::WindowResize { width, height };
                if !size.is_valid() {
                    return Err(invalid("Invalid window size.".into()));
                }
                self.resize.as_ref().ok_or_else(unavailable)?(width, height).map_err(invalid)?;
                Ok(json!({"requested": true}))
            }
            ProbeCommand::Environment(args) => {
                self.environment.as_ref().ok_or_else(unavailable)?(args).map_err(invalid)
            }
            ProbeCommand::TraceStart => {
                self.diagnostics
                    .begin_capture(5000)
                    .map_err(|error| invalid(error.to_string()))?;
                Ok(json!({"recording": true}))
            }
            ProbeCommand::TraceStop => {
                let capture = self
                    .diagnostics
                    .finish_capture()
                    .map_err(|error| invalid(error.to_string()))?;
                serialize(serde_json::to_value(capture))
            }
            ProbeCommand::TraceRead { id, offset, limit } => {
                let page = self
                    .diagnostics
                    .query_trace(&id, offset, limit)
                    .map_err(|error| invalid(error.to_string()))?;
                serialize(serde_json::to_value(page))
            }
            ProbeCommand::TraceCompare {
                baseline,
                candidate,
            } => {
                let comparison = self
                    .diagnostics
                    .compare_traces(&baseline, &candidate)
                    .map_err(|error| invalid(error.to_string()))?;
                serialize(serde_json::to_value(comparison))
            }
            ProbeCommand::Timing { after } => Ok(json!({
                "frames": self.timing.iter().filter(|frame| frame.frame > after).collect::<Vec<_>>(),
                "dropped": self.timing.front().map_or(0, |frame| frame.frame.saturating_sub(after).saturating_sub(1)),
            })),
            ProbeCommand::Logs { after } => {
                let mut logs = cove_logging::LogSystem::snapshot(cove_logging::LogQuery {
                    after_sequence: after,
                    ..Default::default()
                });
                if logs.records.len() > 100 {
                    let excess = logs.records.len() - 100;
                    logs.records.drain(..excess);
                    logs.dropped += excess as u64;
                    logs.first_available_sequence =
                        logs.records.first().map(|record| record.sequence);
                }
                serialize(serde_json::to_value(logs))
            }
        }
    }

    pub fn disconnect(&mut self) {
        self.scene.editor.reset();
        self.diagnostics.reset_captures();
    }
}
