//! One-stop entry for hosts that want DevTools.
//!
//! Wires the `SceneInspector` into a `DevServer` and exposes a single
//! `start`/`stop` pair plus `notify_tree_changed()`. Broadcasts triggered from
//! host callbacks (frame tap, timing publisher, tree changes) are queued and
//! delivered by `pump`, because the server owns the client list and a callback
//! cannot borrow it.
use crate::devtools::mirror::{FrameCapture, FrameTap, FrameTapSink, InputBridge, TimingPublisher};
use crate::inspection::{DiagnosticStateSource, Diagnostics, SceneInspector, UIAccessibilityActions};
use crate::server::DevServer;
use crate::server::Outbound;
use crate::server::config::{
    DevToolsConfig, DevToolsHostMirror, DevToolsHostObservation, DevToolsHostScene,
};
use cove_core::scene::{InvalidationLog, NodeTree, PerformanceTimeline, RenderTree};
use cove_core::{DrawList, Rect};
use cove_devtools_protocol::{
    Envelope, MirrorActivate, MirrorFrame, MirrorInput, MirrorStart, TimingFrame,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

pub mod mirror;

pub use mirror::NativeAppFrameCapture;

/// Host-facing DevTools owner.
pub struct DevTools {
    config: DevToolsConfig,
    tree: Rc<RefCell<NodeTree>>,
    pub server: DevServer,
    pub scene: SceneInspector,
    /// Shared with the wire adapter, which reads whichever facade is current.
    diagnostics: Rc<RefCell<Rc<Diagnostics>>>,
    pub timing: Rc<TimingPublisher>,
    frame_tap: Rc<RefCell<Option<FrameTap>>>,
    frame_tap_sink: Rc<FrameTapSink>,
    outbound: crate::devtools::mirror::OutboundQueue,
    /// Shared inspector the server adapters read through.
    scene_handle: Rc<RefCell<SceneInspector>>,
    /// Invoked for each `mirror.input` event; the host forwards it to its
    /// window session. Shared so a host may install it after `new`, which is
    /// when the wire-up adapters capture their handle.
    pub input_delivery: Rc<RefCell<Option<Rc<dyn Fn(cove_platform::InputEvent)>>>>,
    /// Invoked when a remote client selects a node.
    pub on_selection_changed: Rc<RefCell<Option<Rc<dyn Fn()>>>>,
    /// Invoked immediately after `mirror.start` wires the tap.
    pub on_mirror_start: Rc<RefCell<Option<Rc<dyn Fn()>>>>,
    /// Invoked when a remote client stops mirroring.
    pub on_mirror_stop: Rc<RefCell<Option<Rc<dyn Fn()>>>>,
    /// Checkpoint, recording and window providers the host installs.
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
        let diagnostics = Rc::new(RefCell::new(Rc::new(Diagnostics::new(tree.clone(), None))));
        let server = DevServer::new(config.clone());
        let outbound = server.outbound();
        let frame_tap_sink = FrameTapSink::new();
        let scene_handle = Rc::new(RefCell::new(SceneInspector::new(tree.clone(), None, None)));
        let mut devtools = Self {
            config,
            tree,
            server,
            scene,
            diagnostics,
            timing: TimingPublisher::new(),
            frame_tap: Rc::new(RefCell::new(None)),
            frame_tap_sink,
            outbound,
            scene_handle,
            input_delivery: Rc::new(RefCell::new(None)),
            on_selection_changed: Rc::new(RefCell::new(None)),
            on_mirror_start: Rc::new(RefCell::new(None)),
            on_mirror_stop: Rc::new(RefCell::new(None)),
            checkpoint: Default::default(),
            recording: Default::default(),
            window: Default::default(),
        };
        devtools.install_adapters();
        devtools
    }

    pub fn config(&self) -> &DevToolsConfig {
        &self.config
    }

    pub fn tree(&self) -> &Rc<RefCell<NodeTree>> {
        &self.tree
    }

    /// Give the diagnostics facade the state it reads.
    pub fn set_state_source(&mut self, source: Option<Rc<dyn DiagnosticStateSource>>) {
        let diagnostics = Rc::new(Diagnostics::with_state_source(self.tree.clone(), source));
        *diagnostics.memory.borrow_mut() = self.diagnostics.borrow().memory.borrow().clone();
        self.server.host.observation.state =
            diagnostics.state_observation_available().then(|| {
                let diagnostics = diagnostics.clone();
                Rc::new(move |ids: &[String]| diagnostics.state_observation(ids))
                    as Rc<dyn Fn(&[String]) -> cove_devtools_protocol::StateObservation>
            });
        *self.diagnostics.borrow_mut() = diagnostics;
    }

    pub fn set_memory_snapshot(
        &self,
        provider: Option<Rc<dyn Fn() -> crate::inspection::DiagnosticMemorySnapshot>>,
    ) {
        *self.diagnostics.borrow().memory.borrow_mut() = provider;
    }

    /// `true` between `mirror.start` and `mirror.stop`.
    pub fn mirror_is_active(&self) -> bool {
        self.frame_tap
            .borrow()
            .as_ref()
            .is_some_and(FrameTap::is_active)
    }

    /// Whether the next presented frame would be captured.
    pub fn mirror_wants_frame(&self) -> bool {
        self.frame_tap
            .borrow()
            .as_ref()
            .is_some_and(FrameTap::wants_frame)
    }

    pub fn attach_frame_tap(&mut self, capture: FrameCapture, reset: impl Fn() + 'static) {
        *self.frame_tap.borrow_mut() =
            Some(FrameTap::new(self.frame_tap_sink.clone(), capture).with_reset(reset));
    }

    /// Capture a frame for the mirror viewport. No-op unless a client issued
    /// `mirror.start`.
    pub fn mirror_capture(
        &mut self,
        draw_list: &DrawList,
        width_px: u32,
        height_px: u32,
        logical_width: f32,
        logical_height: f32,
    ) {
        if let Some(tap) = self.frame_tap.borrow_mut().as_mut() {
            tap.capture(
                draw_list,
                width_px,
                height_px,
                logical_width,
                logical_height,
            );
        }
    }

    fn install_adapters(&mut self) {
        // The scene is shared with the adapters through `Rc`; every editor
        // mutator is `&self` because its state already lives in `RefCell`s.
        let scene = Rc::new(RefCell::new(std::mem::replace(
            &mut self.scene,
            SceneInspector::new(self.tree.clone(), None, None),
        )));
        self.scene_handle = scene.clone();

        let selection_changed = self.on_selection_changed.clone();
        let editor_scene = scene.clone();
        *editor_scene.borrow().editor.on_change.borrow_mut() = Some(Rc::new(move || {
            if let Some(callback) = selection_changed.borrow().clone() {
                callback();
            }
        }));

        let snapshot_handle = scene.clone();
        let select_handle = scene.clone();
        let clear_handle = scene.clone();
        let inspect_handle = scene.clone();
        let reset_handle = scene.clone();
        self.server.host.scene = DevToolsHostScene {
            snapshot: Some(Rc::new(move || snapshot_handle.borrow().snapshot())),
            select: Some(Rc::new(move |id: &str| {
                select_handle.borrow().editor.select(Some(id))
            })),
            clear_selection: Some(Rc::new(move || clear_handle.borrow().editor.select(None))),
            inspect: Some(Rc::new(move |request: &Envelope| {
                inspect_handle.borrow().editor.handle(request)
            })),
            reset: Some(Rc::new(move || reset_handle.borrow().editor.reset())),
            diagnostics: None,
        };

        let diagnostics = self.diagnostics.clone();
        self.server.host.scene.diagnostics = Some(Rc::new(move |request: &Envelope| {
            let current = diagnostics.borrow().clone();
            current.handle_read_request(request)
        }));

        self.install_observers();
        self.wire_mirror();
    }

    fn install_observers(&mut self) {
        let outbound = self.outbound.clone();
        let timeline_tree = self.tree.clone();
        let capture_tree = self.tree.clone();
        self.server.host.observation = DevToolsHostObservation {
            timeline: Some(Rc::new(move |after: u64| {
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
            capture: Some(Rc::new(move |enabled: bool| {
                capture_tree
                    .borrow()
                    .timeline
                    .borrow_mut()
                    .set_enabled(enabled);
            })),
            state: None,
        };
        let _ = outbound;
    }

    fn wire_mirror(&mut self) {
        let start_tap = self.frame_tap.clone();
        let stop_tap = self.frame_tap.clone();
        let on_start = self.on_mirror_start.clone();
        let on_stop = self.on_mirror_stop.clone();
        let outbound = self.outbound.clone();
        let input_delivery = self.input_delivery.clone();
        let actions = UIAccessibilityActions::new(self.tree.clone());
        self.server.host.mirror = DevToolsHostMirror {
            start: Some(Rc::new(move |payload: &MirrorStart| {
                if let Some(tap) = start_tap.borrow_mut().as_mut() {
                    tap.start(
                        payload.fps.unwrap_or(15.0),
                        payload.quality.unwrap_or(0.7),
                        payload.scale.unwrap_or(1.0),
                    );
                }
                if let Some(callback) = on_start.borrow().clone() {
                    callback();
                }
            })),
            stop: Some(Rc::new(move || {
                if let Some(tap) = stop_tap.borrow_mut().as_mut() {
                    tap.stop();
                }
                if let Some(callback) = on_stop.borrow().clone() {
                    callback();
                }
                outbound
                    .borrow_mut()
                    .push(Outbound::MirrorStopped("client".into()));
            })),
            input: Some(Rc::new(move |payload: &MirrorInput| {
                let Some(event) = InputBridge::event(payload) else {
                    return;
                };
                if let Some(callback) = input_delivery.borrow().clone() {
                    callback(event);
                }
            })),
            activate: Some(Rc::new(move |payload: &MirrorActivate| {
                actions.perform(payload)
            })),
        };
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
                if let Err(error) = self.publish_discovery() {
                    self.server.stop();
                    self.unwire_sinks();
                    return Err(error);
                }
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
        if self.diagnostics.borrow().state_observation_available() {
            capabilities.push("state.observe".into());
        }
        if self.frame_tap.borrow().is_some() {
            capabilities.push("mirror".into());
        }
        if self.server.host.window.resize.is_some() {
            capabilities.push("window".into());
        }
        if self.server.host.checkpoint.capture.is_some()
            && self.server.host.checkpoint.restore.is_some()
        {
            capabilities.push("state".into());
        }
        let recording = &self.server.host.recording;
        if recording.start.is_some() && recording.stop.is_some() && recording.replay.is_some() {
            capabilities.push("recording".into());
        }
        capabilities
    }

    pub fn stop(&mut self) {
        let timeline = self.tree.borrow().timeline.clone();
        timeline.borrow_mut().set_enabled(false);
        if let Some(tap) = self.frame_tap.borrow_mut().as_mut() {
            tap.stop();
        }
        self.server.stop();
        if let Some(launch) = &self.config.launch {
            let _ = std::fs::remove_file(&launch.descriptor_path);
        }
        self.scene_handle.borrow().editor.reset();
        self.unwire_sinks();
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
        self.scene_handle
            .borrow()
            .editor
            .selected_node()
            .map(|node| node.id().0.to_string())
    }

    /// Window-space frame for the selected node, if it still exists.
    pub fn selected_node_absolute_frame(&self) -> Option<Rect> {
        let id = self.selected_node_id()?;
        self.scene_handle
            .borrow()
            .find(&id)
            .map(|node| node.absolute_frame())
    }

    pub fn intercept_input(&mut self, event: &cove_platform::InputEvent) -> bool {
        self.scene_handle.borrow().editor.intercept(event)
    }

    pub fn draw_inspection_overlay(&self, list: &mut DrawList) {
        self.scene_handle.borrow().editor.draw_overlay(list);
    }

    /// Drain transport events and queued broadcasts. The host calls this once
    /// per frame.
    pub fn pump(&mut self) {
        self.server.pump();
    }

    fn wire_sinks(&mut self) {
        let outbound = self.outbound.clone();
        self.frame_tap_sink
            .set_deliver(Some(Rc::new(move |frame: MirrorFrame| {
                outbound.borrow_mut().push(Outbound::MirrorFrame(frame));
            })));
        let outbound = self.outbound.clone();
        self.timing
            .set_deliver(Some(Rc::new(move |frame: TimingFrame| {
                outbound.borrow_mut().push(Outbound::Timing(frame));
            })));
    }

    fn unwire_sinks(&mut self) {
        self.timing.set_deliver(None);
        self.frame_tap_sink.set_deliver(None);
    }

    fn publish_discovery(&self) -> std::io::Result<()> {
        let Some(launch) = &self.config.launch else {
            return Ok(());
        };
        let Some(port) = self.server.bound_port() else {
            return Ok(());
        };
        let descriptor = cove_devtools_protocol::DevToolsDiscovery {
            id: launch.id.clone(),
            pid: std::process::id(),
            endpoint: format!("ws://127.0.0.1:{port}/?token={}", launch.token),
        };
        let path = std::path::Path::new(&launch.descriptor_path);
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_vec(&descriptor)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        write_private(path, &data)
    }
}

/// Install the accessibility-action handler the mirror uses for
/// `mirror.activate`.
pub fn accessibility_actions(
    tree: Rc<RefCell<NodeTree>>,
) -> Rc<dyn Fn(&MirrorActivate) -> Option<String>> {
    Rc::new(move |request: &MirrorActivate| {
        crate::inspection::perform_accessibility_action(&tree, request)
    })
}

/// Write a descriptor readable only by the owner, matching the Swift path's
/// `0o600` attribute.
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

/// Translate a `mirror.input` payload into a host input event.
pub fn mirror_input_event(payload: &MirrorInput) -> Option<cove_platform::InputEvent> {
    InputBridge::event(payload)
}

/// The live performance timeline for this tree.
pub fn timeline(tree: &Rc<RefCell<NodeTree>>) -> Rc<RefCell<PerformanceTimeline>> {
    tree.borrow().timeline.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_reflect_the_wired_adapters() {
        let tree = Rc::new(RefCell::new(NodeTree::new()));
        let devtools = DevTools::new(DevToolsConfig::default(), tree, None, None);
        let capabilities = devtools.capabilities();
        // Base capabilities plus diagnostics; no mirror until a tap is attached.
        assert!(capabilities.contains(&"diagnostics".to_string()));
        assert!(!capabilities.contains(&"mirror".to_string()));
        assert!(!capabilities.contains(&"state.observe".to_string()));
    }

    #[test]
    fn sink_broadcasts_are_queued_and_drained_by_pump() {
        let tree = Rc::new(RefCell::new(NodeTree::new()));
        let mut devtools = DevTools::new(DevToolsConfig::default(), tree, None, None);
        // Not started: pumping must not panic and must clear the queue.
        devtools.pump();
        assert!(devtools.server.outbound().borrow().is_empty());
    }
}
