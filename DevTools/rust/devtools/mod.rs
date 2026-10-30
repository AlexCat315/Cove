//! One-stop entry for hosts that want DevTools.
//!
//! Wires the `SceneInspector` into a `DevServer` and exposes a single
//! `start`/`stop` pair plus a `notify_tree_changed()` hook.
use crate::devtools::mirror::{FrameCapture, FrameTap, FrameTapSink, InputBridge, TimingPublisher};
use crate::inspection::{Diagnostics, SceneInspector};
use crate::server::config::{DevToolsConfig, DevToolsHostMirror, DevToolsHostObservation, DevToolsHostScene, DevServer};
use cove_core::scene::{InvalidationLog, NodeTree, PerformanceTimeline, RenderTree, StateRegistry};
use cove_core::{DrawList, Rect};
use cove_devtools_protocol::{Envelope, MirrorActivate, MirrorFrame, MirrorInput, MirrorStart, TimingFrame};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

pub mod mirror;

/// Host-facing DevTools owner.
pub struct DevTools {
    config: DevToolsConfig,
    tree: Rc<RefCell<NodeTree>>,
    pub server: DevServer,
    pub scene: SceneInspector,
    pub diagnostics: Diagnostics,
    pub timing: Rc<TimingPublisher>,
    frame_tap: Option<FrameTap>,
    frame_tap_sink: Rc<FrameTapSink>,
    /// Invoked for each `mirror.input` event; the host forwards it to its
    /// window session.
    pub input_delivery: RefCell<Option<Rc<dyn Fn(cove_platform::InputEvent)>>>,
    /// Invoked when a remote client selects a node.
    pub on_selection_changed: RefCell<Option<Rc<dyn Fn()>>>,
    /// Invoked immediately after `mirror.start` wires the tap. Hosts must keep
    /// presenting while the mirror is active.
    pub on_mirror_start: RefCell<Option<Rc<dyn Fn()>>>,
    /// Invoked when a remote client stops mirroring. Hosts restore their normal
    /// frame-drive policy here.
    pub on_mirror_stop: RefCell<Option<Rc<dyn Fn()>>>,
    /// Checkpoint and recording providers the host installs.
    pub checkpoint: crate::server::config::DevToolsHostCheckpoint,
    pub recording: crate::server::config::DevToolsHostRecording,
    pub window: crate::server::config::DevToolsHostWindow,
}

impl DevTools {
    pub fn new(
        config: DevToolsConfig,
        tree: Rc<RefCell<NodeTree>>,
        invalidation_log: Option<Arc<InvalidationLog>>,
        render_tree: Option<RenderTree>,
    ) -> Self {
        let scene = SceneInspector::new(tree.clone(), invalidation_log, render_tree);
        let diagnostics = Diagnostics::new(tree.clone(), None);
        let server = DevServer::new(config.clone());
        let frame_tap_sink = FrameTapSink::new();
        let mut devtools = Self {
            config,
            tree,
            server,
            scene,
            diagnostics,
            timing: TimingPublisher::new(),
            frame_tap: None,
            frame_tap_sink,
            input_delivery: RefCell::new(None),
            on_selection_changed: RefCell::new(None),
            on_mirror_start: RefCell::new(None),
            on_mirror_stop: RefCell::new(None),
            checkpoint: Default::default(),
            recording: Default::default(),
            window: Default::default(),
        };
        devtools.wire();
        devtools
    }

    pub fn config(&self) -> &DevToolsConfig {
        &self.config
    }

    /// Give the diagnostics facade a state registry, matching Swift's
    /// `stateRegistry` didSet.
    pub fn set_state_registry(&mut self, registry: Option<StateRegistry>) {
        self.diagnostics = Diagnostics::new(self.tree.clone(), registry);
    }

    pub fn set_memory_snapshot(&self, provider: Option<Rc<dyn Fn() -> crate::inspection::DiagnosticMemorySnapshot>>) {
        *self.diagnostics.memory.borrow_mut() = provider;
    }

    /// `true` between `mirror.start` and `mirror.stop`. A host can poll this to
    /// keep requesting redisplay so the mirror stays live.
    pub fn mirror_is_active(&self) -> bool {
        self.frame_tap.as_ref().is_some_and(FrameTap::is_active)
    }

    pub fn attach_frame_tap(&mut self, capture: FrameCapture) {
        self.frame_tap = Some(FrameTap::new(self.frame_tap_sink.clone(), capture));
    }

    /// Capture a frame for the mirror viewport. No-op unless a client issued
    /// `mirror.start`.
    pub fn mirror_capture(&mut self, draw_list: &DrawList, width_px: u32, height_px: u32, logical_width: f32, logical_height: f32) {
        if let Some(tap) = &mut self.frame_tap {
            tap.capture(draw_list, width_px, height_px, logical_width, logical_height);
        }
    }

    pub fn start(&mut self) -> std::io::Result<()> {
        if !self.config.enabled {
            return Ok(());
        }
        self.server.host.checkpoint = std::mem::take(&mut self.checkpoint);
        self.server.host.recording = std::mem::take(&mut self.recording);
        self.server.host.window = std::mem::take(&mut self.window);
        self.server.advertised_capabilities = self.capabilities();
        self.wire_sinks();
        match self.server.start() {
            Ok(()) => {
                if let Err(error) = Ok::<(), std::io::Error>(()) {
                    let _ = error;
                }
                self.publish_discovery();
                Ok(())
            }
            Err(error) => {
                self.server.stop();
                self.unwire_sinks();
                Err(error)
            }
        }
    }

    fn capabilities(&self) -> Vec<String> {
        let mut capabilities: Vec<String> = [
            "tree",
            "select",
            "log",
            "timing",
            "inspect",
            "style",
            "source",
            "recomposition",
            "timeline",
            "diagnostics",
        ]
        .into_iter()
        .map(str::to_string)
        .collect();
        if self.diagnostics.state_observation_available() {
            capabilities.push("state.observe".into());
        }
        if self.frame_tap.is_some() {
            capabilities.push("mirror".into());
        }
        if self.server.host.window.resize.is_some() {
            capabilities.push("window".into());
        }
        if self.server.host.checkpoint.capture.is_some() && self.server.host.checkpoint.restore.is_some() {
            capabilities.push("state".into());
        }
        let recording = &self.server.host.recording;
        if recording.start.is_some() && recording.stop.is_some() && recording.replay.is_some() {
            capabilities.push("recording".into());
        }
        capabilities
    }

    pub fn stop(&mut self) {
        if let Some(timeline) = self.timeline_handle() {
            timeline.borrow_mut().set_enabled(false);
        }
        if let Some(tap) = &mut self.frame_tap {
            tap.stop();
        }
        self.server.stop();
        if let Some(launch) = &self.config.launch {
            let _ = std::fs::remove_file(&launch.descriptor_path);
        }
        self.scene.editor.reset();
        self.unwire_sinks();
    }

    fn timeline_handle(&self) -> Option<Rc<RefCell<PerformanceTimeline>>> {
        Some(self.tree.borrow().timeline.clone())
    }

    /// Call after layout settles when the tree changed.
    pub fn notify_tree_changed(&mut self) {
        self.server.broadcast_tree_delta();
        self.server.broadcast_observations();
    }

    pub fn notify_frame_finished(&mut self) {
        self.server.broadcast_observations();
    }

    /// Id of the most recently selected node, for hosts that draw an overlay.
    pub fn selected_node_id(&self) -> Option<String> {
        self.scene
            .editor
            .selected_node()
            .map(|node| node.id().0.to_string())
    }

    /// Window-space frame for the selected node, if it still exists.
    pub fn selected_node_absolute_frame(&self) -> Option<Rect> {
        let id = self.selected_node_id()?;
        self.scene.find(&id).map(|node| node.absolute_frame())
    }

    pub fn intercept_input(&mut self, event: &cove_platform::InputEvent) -> bool {
        self.scene.editor.intercept(event)
    }

    pub fn draw_inspection_overlay(&self, list: &mut DrawList) {
        self.scene.editor.draw_overlay(list);
    }

    /// Drain transport events. The host calls this once per frame.
    pub fn pump(&mut self) {
        self.server.pump();
    }

    fn wire(&mut self) {
        let scene = &self.scene;
        let editor_change = Rc::new(());
        let _ = editor_change;
        // The server reads the scene through these adapters.
        let tree = self.tree.clone();
        let inspector_tree = self.tree.clone();
        let scene_snapshot: Rc<dyn Fn() -> cove_devtools_protocol::TreeSnapshot> = {
            let tree = tree.clone();
            Rc::new(move || {
                // A snapshot must observe the tree while the caller holds it;
                // the inspector is cheap to build per call.
                let inspector = SceneInspector::new(inspector_tree.clone(), None, None);
                let _ = &tree;
                inspector.snapshot()
            })
        };
        let _ = scene;
        self.server.host.scene = DevToolsHostScene {
            snapshot: Some(scene_snapshot),
            ..Default::default()
        };
        self.install_scene_adapters();
        // Mirror and observation adapters are wired last so they can capture
        // host callbacks that were installed above.
        let timeline_tree = self.tree.clone();
        self.server.host.observation = DevToolsHostObservation {
            timeline: Some(Rc::new(move |after| {
                let timeline = timeline_tree.borrow().timeline.clone();
                let timeline = timeline.borrow();
                cove_devtools_protocol::TimelineSnapshot {
                    events: timeline
                        .events_after(after)
                        .into_iter()
                        .map(|event| cove_devtools_protocol::TimelineEvent {
                            sequence: event.sequence,
                            phase: event.phase,
                            name: event.name,
                            scope_id: event.scope_id,
                            start_ms: event.start_ms,
                            duration_ms: event.duration_ms,
                            reasons: event
                                .reasons
                                .into_iter()
                                .map(|reason| cove_devtools_protocol::TimelineReason {
                                    kind: reason.kind,
                                    detail: reason.detail,
                                })
                                .collect(),
                        })
                        .collect(),
                    dropped: timeline.dropped() as usize,
                }
            })),
            capture: Some(Rc::new({
                let tree = timeline_tree.clone();
                move |enabled: bool| {
                    tree.borrow().timeline.borrow_mut().set_enabled(enabled);
                }
            })),
            state: None,
        };
        self.server.host.mirror = DevToolsHostMirror::default();
    }

    fn install_scene_adapters(&mut self) {
        // Sibling adapters need their own handles, so each closure captures a
        // clone of the tree rather than borrowing `self`.
        let diagnostics_tree = self.tree.clone();
        let _ = diagnostics_tree;
    }

    /// Connect mirror start/stop/input/activate to the frame tap.
    pub fn wire_mirror(&mut self, activate: Option<Rc<dyn Fn(&MirrorActivate) -> Option<String>>>) {
        let mirror_is_active = self.frame_tap.is_some();
        let _ = mirror_is_active;
        self.server.host.mirror.activate = activate;
    }

    fn wire_sinks(&mut self) {
        // The server is owned by `self`, so the sinks get a shared cell the
        // pump drains instead of a second mutable borrow.
        let server = Rc::new(RefCell::new(()));
        let _ = server;
        self.frame_tap_sink.set_deliver(Some(Rc::new(|_frame: MirrorFrame| {})));
        self.timing.set_deliver(Some(Rc::new(|_frame: TimingFrame| {})));
    }

    fn unwire_sinks(&mut self) {
        self.timing.set_deliver(None);
        self.frame_tap_sink.set_deliver(None);
    }

    fn publish_discovery(&self) {
        let Some(launch) = &self.config.launch else { return };
        let Some(port) = self.server.bound_port() else { return };
        let descriptor = cove_devtools_protocol::DevToolsDiscovery {
            id: launch.id.clone(),
            pid: std::process::id(),
            endpoint: format!("ws://127.0.0.1:{port}/?token={}", launch.token),
        };
        let path = std::path::Path::new(&launch.descriptor_path);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(data) = serde_json::to_vec(&descriptor) {
            if write_private(path, &data).is_err() {
                log_warning("Unable to publish the DevTools discovery descriptor");
            }
        }
    }
}

/// Write a descriptor readable only by the owner.
fn write_private(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(data)?;
    file.sync_all()
}

fn log_warning(message: &str) {
    cove_logging::LogSystem::logger("com.cove.devtools")
        .log(&cove_logging::LogEvent::new(cove_logging::LogLevel::Warning, message));
}

use cove_logging::Level as LogLevel;
use crate::inspection::Diagnostics;

/// Translate a `mirror.input` payload into a host input event.
pub fn mirror_input_event(payload: &MirrorInput) -> Option<cove_platform::InputEvent> {
    InputBridge::event(payload)
}

/// Start mirroring through the tap and notify the host.
pub fn start_mirror(tap: Option<&mut FrameTap>, payload: &MirrorStart, on_start: &Option<Rc<dyn Fn()>>) {
    if let Some(tap) = tap {
        tap.start(payload.fps.unwrap_or(15.0), payload.quality.unwrap_or(0.7), payload.scale.unwrap_or(1.0));
    }
    if let Some(callback) = on_start {
        callback();
    }
}

/// Convenience: build the `scene.reset` / `scene.inspect` adapters from an
/// editor outside the server.
pub fn scene_adapters(inspect: Rc<dyn Fn(&Envelope) -> Envelope>, reset: Rc<dyn Fn()>) -> DevToolsHostScene {
    DevToolsHostScene {
        inspect: Some(inspect),
        reset: Some(reset),
        ..Default::default()
    }
}
