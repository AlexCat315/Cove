//! Commands available inside every Cove runtime, independent of its transport.
use crate::allocation_sites::AllocationSites;
use crate::frame_analysis::{FrameAnalysis, Timeline};
use crate::inspection::{
    DebugPaint, DiagnosticError, DiagnosticStateSource, Diagnostics, SceneInspector, Tool,
};
use crate::profile::UiProfiler;
use cove_core::Point;
use cove_core::scene::{InvalidationLog, NodeTree, RenderTree};
use cove_devtools_protocol::control::{Fault, ProbeCommand, Viewport};
use cove_devtools_protocol::{
    ContentGeometry, Envelope, MAX_TREE_PAGE_BYTES, NodeHit, NodePoint, TimingFrame,
};
use cove_platform::InputEvent;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;

pub type Resize = Rc<dyn Fn(f64, f64) -> Result<(), String>>;
pub type Environment = Rc<dyn Fn(Value) -> Result<Value, String>>;
/// Reports where the runtime content sits in the host's window-server space.
/// Hosts that cannot observe it leave this absent, and the probe answers
/// `unavailable` rather than inventing coordinates.
pub type ContentGeometrySource = Rc<dyn Fn() -> Result<ContentGeometry, String>>;
/// The display's real refresh rate in Hz. Hosts that cannot observe it leave
/// this absent, and the probe reports no rate rather than assuming 60 Hz.
pub type RefreshRateSource = Rc<dyn Fn() -> Option<f64>>;

pub type RuntimeInputSink = Rc<dyn Fn(InputEvent)>;

pub struct RuntimeProbe {
    pub viewport: Option<Rc<dyn Fn() -> Viewport>>,
    pub input: Option<RuntimeInputSink>,
    diagnostics: Diagnostics,
    pub scene: SceneInspector,
    pub resize: Option<Resize>,
    pub environment: Option<Environment>,
    pub content_geometry: Option<ContentGeometrySource>,
    pub display_refresh_rate: Option<RefreshRateSource>,
    timeline: Timeline,
    timing: VecDeque<(
        TimingFrame,
        Option<cove_devtools_protocol::GpuFrameBreakdown>,
    )>,
    pending_timing: VecDeque<(
        TimingFrame,
        Option<cove_graphics::rhi::GpuFrameTiming>,
        std::time::Instant,
    )>,
    sequence: u64,
    profile: UiProfiler,
    allocation_sites: AllocationSites,
}

impl RuntimeProbe {
    pub fn new(
        tree: Rc<RefCell<NodeTree>>,
        invalidation: Option<Arc<InvalidationLog>>,
        render_tree: Option<RenderTree>,
        state: Option<Rc<dyn DiagnosticStateSource>>,
    ) -> Self {
        let timeline = tree.borrow().timeline.clone();
        // The probe is built on the UI thread; the CPU profiler follows that
        // thread by the name Rust gave it, so the sampler can find it later.
        let thread = std::thread::current().name().unwrap_or("main").to_owned();
        Self {
            diagnostics: Diagnostics::with_state_source(tree.clone(), state),
            scene: SceneInspector::new(tree, invalidation, render_tree),
            viewport: None,
            input: None,
            resize: None,
            environment: None,
            content_geometry: None,
            display_refresh_rate: None,
            timeline,
            timing: VecDeque::new(),
            pending_timing: VecDeque::new(),
            sequence: 0,
            profile: UiProfiler::new(thread),
            allocation_sites: AllocationSites::new(),
        }
    }

    /// Records one measured render phase for a cooperative CPU profile. The
    /// browser runtime has no sampler, so it reports the phases it timed.
    pub fn record_phase(&self, label: &'static str, ms: f64) {
        crate::profile::record_phase(label, ms);
    }

    /// Closes a frame's cooperative entry with its measured total; the
    /// difference from the recorded phases becomes the frame's self time.
    pub fn record_frame(&self, total_ms: f64) {
        crate::profile::record_frame(total_ms);
    }

    /// Whether a cooperative profile is recording, so the render path can skip
    /// timings it would only discard.
    pub fn profiling_phases(&self) -> bool {
        crate::profile::is_recording()
    }

    /// The UI thread whose CPU time `profile_start` samples.
    pub fn ui_thread_name(&self) -> &str {
        self.profile.thread_name()
    }

    pub fn diagnostics(&self) -> &Diagnostics {
        &self.diagnostics
    }

    pub fn capabilities(&self) -> Vec<String> {
        let mut capabilities = vec![
            "tree",
            "tree_page",
            "diagnostics",
            "style",
            "trace",
            "timing",
            "frame_analysis",
            "component_activity",
            "semantics_tree",
            "network",
            "network_control",
            "inspect_nodes",
            "debug_paint",
            "baselines",
            "oversized_images",
            "repaint_highlight",
            "animation_speed",
        ];
        if self.viewport.is_some() && self.input.is_some() {
            capabilities.push("input");
        }
        if self.resize.is_some() {
            capabilities.push("resize");
        }
        if self.environment.is_some() {
            capabilities.push("environment");
        }
        if self.content_geometry.is_some() {
            capabilities.push("content_geometry");
        }
        if self.display_refresh_rate.is_some() {
            capabilities.push("refresh_rate");
        }
        if self.diagnostics.state_observation_available() {
            capabilities.push("state");
        }
        if self.diagnostics.extensions_available() {
            capabilities.push("extensions");
        }
        if crate::profile::unavailable_reason().is_none() {
            capabilities.push("cpu_profile");
        }
        if crate::allocation_sites::unavailable_reason().is_none() {
            capabilities.push("allocation_sites");
        }
        capabilities.into_iter().map(str::to_owned).collect()
    }

    pub fn record(&mut self, frame: TimingFrame) {
        self.record_measured(frame, None);
    }

    pub fn record_measured(
        &mut self,
        mut frame: TimingFrame,
        gpu: Option<cove_graphics::rhi::GpuFrameTiming>,
    ) {
        self.sequence += 1;
        frame.frame = self.sequence;
        self.pending_timing
            .push_back((frame, gpu, std::time::Instant::now()));
        self.flush_timing();
    }

    fn flush_timing(&mut self) {
        // Keep sequence order for incremental readers and bound how long a
        // missing GPU completion can delay the CPU timeline.
        while let Some((_, gpu, recorded)) = self.pending_timing.front() {
            let timing = gpu.as_ref().and_then(|gpu| gpu.retired());
            if gpu.is_some()
                && timing.is_none()
                && self.pending_timing.len() < 16
                && recorded.elapsed() < std::time::Duration::from_secs(1)
            {
                break;
            }
            let (mut frame, _, _) = self.pending_timing.pop_front().expect("front was present");
            if let Some((api, timing)) = timing {
                frame.gpu_ms = timing.nanoseconds.map(|nanos| nanos as f64 / 1_000_000.0);
                if let Some(breakdown) = &mut frame.gpu_breakdown {
                    crate::gpu_breakdown::apply_timing(breakdown, api, &timing);
                }
            }
            if self.timing.len() == 240 {
                self.timing.pop_front();
            }
            let breakdown = frame.gpu_breakdown.take();
            self.timing.push_back((frame, breakdown));
        }
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
            // The transport handles capture because only its host owns the render surface.
            ProbeCommand::Capture => Err(unavailable()),
            ProbeCommand::Input { viewport, action } => {
                let current = self.viewport.as_ref().ok_or_else(unavailable)?();
                let send = self.input.as_ref().ok_or_else(unavailable)?;
                crate::runtime_input::dispatch(action, viewport, current, send.as_ref())?;
                Ok(json!({"delivered": true, "source": "runtime", "verified": false}))
            }
            ProbeCommand::Tree => serialize(serde_json::to_value(self.scene.summary_snapshot())),
            ProbeCommand::TreePage {
                node_id,
                depth,
                limit,
            } => {
                let build = |limit: usize| {
                    self.scene
                        .tree_page(node_id.as_deref(), depth, limit)
                        .map_err(|error| Fault::new(error.code(), error.to_string()))
                };
                let mut page = build(limit)?;
                let mut bytes =
                    serde_json::to_vec(&page).map_err(|error| invalid(error.to_string()))?;
                // The agent toolbox refuses a result above 512 KiB. Shrink the
                // node cap until the serialised page fits, keeping the page
                // honest about the cut.
                while bytes.len() > MAX_TREE_PAGE_BYTES && page.node_count > 1 {
                    let scaled = page.node_count.saturating_mul(MAX_TREE_PAGE_BYTES) / bytes.len();
                    let next = scaled.clamp(1, page.node_count - 1);
                    page = build(next)?;
                    page.truncated = true;
                    bytes =
                        serde_json::to_vec(&page).map_err(|error| invalid(error.to_string()))?;
                }
                serialize(serde_json::to_value(page))
            }
            ProbeCommand::Select { node_id } => {
                if !self.scene.editor.select(node_id.as_deref()) {
                    return Err(Fault::new(
                        "stale_node",
                        "The selected component is no longer in the scene.",
                    ));
                }
                serialize(serde_json::to_value(self.scene.editor.state()))
            }
            ProbeCommand::NodeAt { x, y } => {
                if !x.is_finite() || !y.is_finite() {
                    return Err(invalid("The hit-test point must be finite.".into()));
                }
                let point = Point {
                    x: x as f32,
                    y: y as f32,
                };
                let hit = self.scene.node_at(point).map_or_else(
                    || NodeHit {
                        node_id: None,
                        path: Vec::new(),
                        local_point: None,
                        absolute_frame: None,
                    },
                    |hit| NodeHit {
                        node_id: Some(SceneInspector::identifier(&hit.node)),
                        path: hit.path.iter().map(SceneInspector::identifier).collect(),
                        local_point: Some(NodePoint {
                            x: hit.local_point.x as f64,
                            y: hit.local_point.y as f64,
                        }),
                        absolute_frame: Some(crate::inspection::encode_frame(
                            hit.node.absolute_frame(),
                        )),
                    },
                );
                serialize(serde_json::to_value(hit))
            }
            ProbeCommand::ContentGeometry => {
                let geometry = self.content_geometry.as_ref().ok_or_else(unavailable)?()
                    .map_err(|message| Fault::new("unavailable", message))?;
                if !geometry.is_valid() {
                    return Err(Fault::new(
                        "unavailable",
                        "The host reported content geometry that is not finite and positive.",
                    ));
                }
                serialize(serde_json::to_value(geometry))
            }
            ProbeCommand::Read { name, arguments } => {
                let tool = Tool::parse(&name)
                    .filter(|tool| tool.is_immediate_read())
                    .ok_or_else(|| Fault::new("unknown_command", "Unknown diagnostic command."))?;
                self.diagnostics
                    .execute_read_json(tool, &arguments)
                    .map_err(|error| match error {
                        DiagnosticError::MissingNode(_) => {
                            Fault::new("stale_node", error.to_string())
                        }
                        _ => invalid(error.to_string()),
                    })
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
            ProbeCommand::Timing { after } => {
                self.flush_timing();
                Ok(json!({
                "frames": self.timing.iter().map(|(frame, _)| frame).filter(|frame| frame.frame > after).collect::<Vec<_>>(),
                "dropped": self.timing.front().map_or(0, |(frame, _)| frame.frame.saturating_sub(after).saturating_sub(1)),
                "refreshRate": self.display_refresh_rate.as_ref().and_then(|query| query()),
                }))
            }
            ProbeCommand::AnalyzeUiFrame { frame } => {
                self.flush_timing();
                let analysis = match self.timing.iter().find(|(timing, _)| timing.frame == frame) {
                    Some((timing, breakdown)) => {
                        let mut analysis =
                            FrameAnalysis::for_frame(timing, &self.timeline.borrow());
                        analysis.gpu_breakdown = breakdown.clone();
                        analysis
                    }
                    None => FrameAnalysis::not_retained(frame, self.timing.len()),
                };
                serialize(serde_json::to_value(analysis))
            }
            ProbeCommand::ProfileStart { hz, duration_ms } => self.profile.start(hz, duration_ms),
            ProbeCommand::ProfileStop => self.profile.stop(),
            ProbeCommand::ProfileRead => self.profile.read(),
            ProbeCommand::ProfileHotspots { limit } => self.profile.hotspots(limit),
            ProbeCommand::AllocationStart { rate } => self.allocation_sites.start(rate),
            ProbeCommand::AllocationStop => self.allocation_sites.stop(),
            ProbeCommand::AllocationSites { limit } => self.allocation_sites.sites(limit),
            ProbeCommand::DebugPaint { guidelines } => {
                let applied = self.scene.editor.set_debug_paint(DebugPaint { guidelines });
                serialize(serde_json::to_value(applied))
            }
            ProbeCommand::Baselines { enabled } => {
                let applied = self.scene.editor.set_baselines(enabled);
                Ok(json!({"baselines": applied}))
            }
            ProbeCommand::OversizedImages { enabled } => {
                let applied = self.scene.editor.set_oversized_images(enabled);
                Ok(json!({"enabled": applied}))
            }
            ProbeCommand::RepaintHighlight { enabled } => {
                let applied = self.scene.editor.set_repaint_highlight(enabled);
                Ok(json!({"enabled": applied}))
            }
            ProbeCommand::AnimationScale { scale } => {
                if !scale.is_finite() {
                    return Err(invalid("The animation time scale must be finite.".into()));
                }
                let applied = cove_core::scene::set_animation_time_scale(scale);
                Ok(json!({"scale": applied}))
            }
            ProbeCommand::NetworkRecording { enabled } => {
                cove_http::recording::set_enabled(enabled);
                Ok(json!({"enabled": cove_http::recording::is_enabled()}))
            }
            ProbeCommand::ClearNetwork => Ok(json!({
                "cleared": cove_http::recording::clear(),
                "limit": cove_http::recording::MAX_RECORDS,
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
            ProbeCommand::ClearLogs => {
                let cleared = cove_logging::LogSystem::clear_history();
                let logs = cove_logging::LogSystem::snapshot(cove_logging::LogQuery::default());
                Ok(json!({
                    "cleared": cleared,
                    "dropped": logs.dropped,
                    "firstAvailableSequence": logs.first_available_sequence,
                }))
            }
        }
    }

    pub fn disconnect(&mut self) {
        self.scene.editor.reset();
        self.diagnostics.reset_captures();
        self.profile.reset();
        self.allocation_sites.reset();
        cove_core::scene::reset_animation_time_scale();
        cove_http::recording::reset();
    }
}
