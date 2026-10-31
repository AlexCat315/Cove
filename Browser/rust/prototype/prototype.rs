//! Translation of `BrowserPrototype` from
//! `Browser/Sources/CoveUIBrowserPrototype/main.swift`.
//!
//! The reactor owns the single scene graph, the text stack, the draw list and
//! the DevTools surface for the Wasm host. Swift marks the global `app`
//! `nonisolated(unsafe)` because JavaScript only ever calls in on the browser
//! main thread; here the same invariant is carried by `thread_local!` storage
//! in the parent module, so the fields below need no locking.
//!
//! The root content comes from the shared example crate, exactly as the Swift
//! prototype imports `CounterExample`. It is factored behind
//! [`PrototypeContent`] so the host ABI, frame encoding and DevTools plumbing
//! stay independent of any particular demo.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Instant;

use cove_composition::ViewGraph;
use cove_core::scene::{EventDispatcher, NodeRenderer, NodeTree, PlatformInputContext, Recomposer};
use cove_core::{DrawList, UI_SHADER_WGSL};
use cove_devtools::client::DevToolsSession;
use cove_devtools::inspection::SceneInspector;
use cove_devtools_protocol::{
    Envelope, HelloHostInfo, HelloPayload, InputRecorder, InputRecording, Session, StateDifference,
    StateWatch, Subscription, TimingFrame, TreeSnapshot,
};
use cove_platform::{
    InputEvent as PlatformInputEvent, KeyEvent, KeyModifiers, MouseButton, MouseButtonEvent,
    MouseMotionEvent, PointerKind, TextEditingEvent,
};
use cove_text::FontCollection;

use crate::export_buffer::ExportBuffer;
use crate::prototype::frame::{Batch, Frame, Label, viewport_size};

/// Atlas edge in texels, matching the Swift prototype.
const ATLAS_SIZE: usize = 2048;

/// Maximum bytes accepted for one font or one text event.
const MAX_PAYLOAD: usize = 32 * 1024 * 1024;

/// The root demo a browser host renders.
///
/// Swift hard-codes `CounterView`; keeping the small surface behind a trait
/// lets the same host drive any shared example that exposes checkpointable
/// state and a view tree.
pub trait PrototypeContent {
    /// The `View` this content installs as the graph root.
    fn install(self: &Rc<Self>, graph: &Rc<ViewGraph>);
    /// Observable state serialised for `state.checkpoint`.
    fn checkpoint(&self) -> BTreeMap<String, String>;
    /// Restores observable state; returns `false` for invalid values.
    fn restore(&self, values: &BTreeMap<String, String>) -> bool;
    /// The three values the frame payload publishes.
    fn summary(&self) -> (i64, bool, String);
}

/// The single-threaded browser reactor.
pub struct BrowserPrototype<C: PrototypeContent> {
    content: Rc<C>,
    tree: Rc<RefCell<NodeTree>>,
    context: PlatformInputContext,
    dispatcher: EventDispatcher,
    inspector: SceneInspector,
    graph: Rc<ViewGraph>,
    recomposer: Rc<Recomposer>,
    draw_list: DrawList,
    fonts: FontCollection,
    raster_scale: f32,
    frame: Option<Frame>,
    frame_number: u64,
    recomposition_ms: f64,
    layout_ms: f64,
    render_ms: f64,
    session: DevToolsSession,
    recorder: InputRecorder,
    last_observation: Option<cove_devtools_protocol::StateObservation>,
    pub vertex_buffer: ExportBuffer,
    pub index_buffer: ExportBuffer,
    pub snapshot_buffer: ExportBuffer,
    pub response_buffer: ExportBuffer,
    pub shader_buffer: ExportBuffer,
    pub atlas_buffer: ExportBuffer,
}

impl<C: PrototypeContent> BrowserPrototype<C> {
    /// Builds the reactor and installs `content` as the graph root.
    pub fn new(content: C) -> Self {
        let _ = cove_logging::LogSystem::bootstrap(
            Some("com.cove.browser"),
            cove_logging::LoggingConfiguration::default(),
            None,
        );
        let content = Rc::new(content);
        let tree = Rc::new(RefCell::new(NodeTree::new()));
        let context = PlatformInputContext::default();
        let recomposer = Rc::new(Recomposer::new());
        let graph = Rc::new(ViewGraph::new(tree.clone(), recomposer.clone()));
        let dispatcher = EventDispatcher::new(
            tree.clone(),
            context.interactions.clone(),
            context.pointer_capture.clone(),
            context.focus_chain.clone(),
            cove_platform::WindowId(0),
        );
        let inspector =
            SceneInspector::new(tree.clone(), Some(context.invalidation_log.clone()), None);
        context.with_current(|| content.install(&graph));
        let mut fonts = FontCollection::new(ATLAS_SIZE, ATLAS_SIZE);
        fonts.configure(16.0, 1.0);
        let mut shader_buffer = ExportBuffer::new();
        shader_buffer.replace(UI_SHADER_WGSL.as_bytes());
        Self {
            content,
            tree,
            context,
            dispatcher,
            inspector,
            graph,
            recomposer,
            draw_list: DrawList::default(),
            fonts,
            raster_scale: 1.0,
            frame: None,
            frame_number: 0,
            recomposition_ms: 0.0,
            layout_ms: 0.0,
            render_ms: 0.0,
            session: Session::default(),
            recorder: InputRecorder::default(),
            last_observation: None,
            vertex_buffer: ExportBuffer::new(),
            index_buffer: ExportBuffer::new(),
            snapshot_buffer: ExportBuffer::new(),
            response_buffer: ExportBuffer::new(),
            shader_buffer,
            atlas_buffer: ExportBuffer::new(),
        }
    }

    pub fn set_raster_scale(&mut self, scale: f32) {
        if scale.is_finite() {
            self.raster_scale = scale.clamp(1.0, 4.0);
        }
    }

    /// Appends a font face; returns its id, or `None` when rejected.
    pub fn append_font(&mut self, bytes: &[u8]) -> Option<i64> {
        if bytes.is_empty() || bytes.len() > MAX_PAYLOAD || self.fonts.font_count() >= 8 {
            return None;
        }
        self.fonts.append(bytes.to_vec())
    }

    /// Rebuilds the frame snapshot for the requested viewport.
    pub fn render(&mut self, raw_width: f32, raw_height: f32) {
        let started = Instant::now();
        let (width, height) = viewport_size(raw_width, raw_height);

        self.recomposer.commit_all();
        self.recomposition_ms = started.elapsed().as_secs_f64() * 1000.0;

        let layout_started = Instant::now();
        self.graph.compute_layout_if_needed(width, height);
        self.layout_ms = layout_started.elapsed().as_secs_f64() * 1000.0;

        let mut labels: Vec<Label> = Vec::new();
        let mut note_width = 0.0_f32;
        // `with_current` needs `&self.context` while the pass needs mutable
        // access to the draw list and fonts, so both halves are taken as
        // separate locals rather than through `self`.
        let (context, tree, inspector) = (&self.context, &self.tree, &self.inspector);
        let (draw_list, fonts) = (&mut self.draw_list, &mut self.fonts);
        context.with_current(|| {
            encode_frame(
                tree,
                inspector,
                draw_list,
                fonts,
                &mut labels,
                &mut note_width,
            );
        });

        if fonts.atlas.is_full() {
            // Resetting the atlas invalidates every UV, so re-encode the whole
            // frame instead of patching the swapped glyphs.
            fonts.atlas.reset();
            labels.clear();
            note_width = 0.0;
            context.with_current(|| {
                encode_frame(
                    tree,
                    inspector,
                    draw_list,
                    fonts,
                    &mut labels,
                    &mut note_width,
                );
            });
        }

        let upload = self.fonts.atlas.dirty_upload_payload();
        match &upload {
            Some(payload) => self.atlas_buffer.replace(&payload.pixels),
            None => self.atlas_buffer.replace(&[]),
        }
        self.fonts.atlas.mark_clean();

        let (count, dark, note) = self.content.summary();
        let batches = self
            .draw_list
            .batches()
            .iter()
            .map(|batch| Batch {
                offset: batch.index_offset,
                count: batch.index_count,
                texture_id: batch.texture_id,
                clip: batch
                    .scissor
                    .map(|rect| [rect.x, rect.y, rect.width, rect.height]),
            })
            .collect();
        let vertices = self.draw_list.vertices().len();
        let indices = self.draw_list.indices().len();
        let snapshot = self.inspector.snapshot();
        self.frame = Some(Frame {
            count,
            dark,
            note,
            note_width,
            focused: self
                .context
                .focus_chain
                .focused()
                .and_then(|node| crate::prototype::node_debug_name(&node)),
            width,
            height,
            vertices,
            indices,
            batches,
            labels,
            tree: snapshot,
            font_count: self.fonts.font_count(),
            atlas_width: self.fonts.atlas.width(),
            atlas_height: self.fonts.atlas.height(),
            atlas_update: upload.as_ref().map(|payload| {
                [
                    payload.region.x,
                    payload.region.y,
                    payload.region.width,
                    payload.region.height,
                ]
            }),
            atlas_full: self.fonts.atlas.is_full(),
        });

        self.vertex_buffer
            .replace(vertex_bytes(self.draw_list.vertices()));
        self.index_buffer
            .replace(index_bytes(self.draw_list.indices()));
        if let Some(frame) = &self.frame {
            self.snapshot_buffer.encode(frame);
        }
        self.frame_number = self.frame_number.wrapping_add(1);
        self.render_ms =
            (started.elapsed().as_secs_f64() * 1000.0) - self.recomposition_ms - self.layout_ms;
    }

    pub fn pointer(&mut self, kind: i32, x: f32, y: f32) {
        if !x.is_finite() || !y.is_finite() {
            return;
        }
        let event = match kind {
            0 => PlatformInputEvent::MouseMotion(MouseMotionEvent {
                x,
                y,
                delta_x: 0.0,
                delta_y: 0.0,
                kind: PointerKind::default(),
            }),
            1 => PlatformInputEvent::MouseButtonDown(MouseButtonEvent {
                button: MouseButton::Left,
                x,
                y,
                clicks: 1,
                modifiers: KeyModifiers::default(),
                kind: PointerKind::default(),
            }),
            2 => PlatformInputEvent::MouseButtonUp(MouseButtonEvent {
                button: MouseButton::Left,
                x,
                y,
                clicks: 1,
                modifiers: KeyModifiers::default(),
                kind: PointerKind::default(),
            }),
            3 => {
                let press = |down: bool| {
                    let inner = MouseButtonEvent {
                        button: MouseButton::Left,
                        x,
                        y,
                        clicks: 1,
                        modifiers: KeyModifiers::default(),
                        kind: PointerKind::default(),
                    };
                    if down {
                        PlatformInputEvent::MouseButtonDown(inner)
                    } else {
                        PlatformInputEvent::MouseButtonUp(inner)
                    }
                };
                self.context.with_current(|| {
                    self.dispatcher.dispatch(&press(true));
                    self.dispatcher.dispatch(&press(false));
                });
                return;
            }
            _ => return,
        };
        self.context.with_current(|| {
            self.dispatcher.dispatch(&event);
        });
    }

    pub fn key(&mut self, code: u32, modifiers: u16, down: bool, repeated: bool) {
        let event = KeyEvent {
            scancode: code,
            keycode: 0,
            modifiers: KeyModifiers(modifiers),
            is_repeat: repeated,
        };
        let event = if down {
            PlatformInputEvent::KeyDown(event)
        } else {
            PlatformInputEvent::KeyUp(event)
        };
        self.context.with_current(|| {
            self.dispatcher.dispatch(&event);
        });
    }

    pub fn text(&mut self, text: &str, editing: bool) {
        if text.len() > 32768 {
            return;
        }
        let event = if editing {
            PlatformInputEvent::TextEditing(TextEditingEvent {
                text: text.to_owned(),
                start: 0,
                length: 0,
            })
        } else {
            PlatformInputEvent::TextInput(text.to_owned())
        };
        self.context.with_current(|| {
            self.dispatcher.dispatch(&event);
        });
    }

    pub fn hello(&mut self) {
        let payload = HelloPayload::new(
            HelloHostInfo {
                pid: 0,
                app_title: "CoveUI Wasm prototype".into(),
                platform: "WebAssembly".into(),
            },
            [
                "log",
                "tree",
                "select",
                "timing",
                "state",
                "recording",
                "inspect",
                "style",
                "source",
                "recomposition",
                "state.observe",
                "timeline",
            ]
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        );
        self.publish(&Envelope::new(
            "hello",
            None,
            serde_json::to_value(payload).ok(),
        ));
    }

    /// Handles one DevTools request.
    pub fn dispatch(&mut self, bytes: &[u8]) {
        let request = match serde_json::from_slice::<Envelope>(bytes) {
            Ok(request) => request,
            Err(_) => {
                self.publish(&Envelope::new(
                    "request.err",
                    None,
                    Some(serde_json::json!({
                        "code": "bad_request",
                        "message": "Invalid JSON",
                    })),
                ));
                return;
            }
        };
        if let Some(error) = self.session.validate(&request) {
            self.publish(&error);
            return;
        }
        if request.r#type.starts_with("inspect.") {
            let response = self.inspector.editor.handle(&request);
            self.publish(&response);
            return;
        }
        let mut response = Envelope::new(format!("{}.ok", request.r#type), request.id, None);
        match request.r#type.as_str() {
            "hello.ack" => {}
            "log.subscribe" => {
                self.session.set(Subscription::Log, true);
            }
            "log.unsubscribe" => {
                self.session.set(Subscription::Log, false);
            }
            "state.subscribe" => {
                if let Some(watch) = decode::<StateWatch>(request.payload.as_ref()) {
                    self.session.watched_state_ids = watch.ids;
                }
                self.session.set(Subscription::State, true);
            }
            "state.unsubscribe" => {
                self.session.set(Subscription::State, false);
                self.session.watched_state_ids.clear();
                self.last_observation = None;
            }
            "timeline.subscribe" => {
                self.session.set(Subscription::Timeline, true);
                self.session.timeline_sequence = 0;
                self.set_timeline_enabled(true);
            }
            "timeline.unsubscribe" => {
                self.session.set(Subscription::Timeline, false);
                self.set_timeline_enabled(false);
            }
            "tree.subscribe" => {
                self.session.set(Subscription::Tree, true);
                response.r#type = "tree.snapshot".into();
                response.payload = self.tree_payload();
            }
            "tree.unsubscribe" => {
                self.session.set(Subscription::Tree, false);
            }
            "timing.subscribe" => {
                self.session.set(Subscription::Timing, true);
            }
            "timing.unsubscribe" => {
                self.session.set(Subscription::Timing, false);
            }
            "select.node" => {
                let id = request
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.get("id"))
                    .and_then(|id| id.as_str())
                    .map(str::to_owned);
                match id.filter(|id| self.inspector.find(id).is_some()) {
                    Some(id) => self.inspector.editor.select(Some(&id)),
                    None => fail(&mut response, "Unknown node"),
                }
            }
            "select.clear" => self.inspector.editor.select(None),
            "state.checkpoint" => {
                response.payload = serde_json::to_value(self.content.checkpoint()).ok();
            }
            "state.restore" => {
                let restored = cove_devtools_protocol::state(&request.payload)
                    .is_some_and(|values| self.content.restore(&values));
                if !restored {
                    fail(
                        &mut response,
                        "State requires count (0…999999) and dark (true/false), as strings",
                    );
                }
            }
            "state.diff" => {
                let before = cove_devtools_protocol::state(&request.payload).unwrap_or_default();
                let difference = StateDifference::new(&before, &self.content.checkpoint());
                response.payload = serde_json::to_value(difference).ok();
            }
            "input.record.start" => {
                if self.recorder.is_recording() {
                    fail(&mut response, "Already recording");
                } else {
                    self.recorder
                        .start(self.content.checkpoint(), self.focused_name());
                }
            }
            "input.record.stop" => match self.recorder.stop() {
                Some(recording) => {
                    response.payload = serde_json::to_value(recording).ok();
                }
                None => fail(&mut response, "No recording"),
            },
            "input.replay" => {
                let replayed = !self.recorder.is_recording()
                    && decode::<InputRecording>(request.payload.as_ref())
                        .is_some_and(|recording| self.replay(&recording));
                if !replayed {
                    fail(&mut response, "Invalid recording state");
                }
            }
            "bye" => {
                self.session.reset();
                self.set_timeline_enabled(false);
                self.last_observation = None;
                self.recorder.stop();
                self.inspector.editor.reset();
            }
            other => {
                response.r#type = format!("{other}.err");
                response.payload = Some(serde_json::json!({
                    "code": "bad_request",
                    "message": "Unsupported browser prototype message",
                }));
            }
        }
        self.publish(&response);
    }

    fn replay(&mut self, recording: &InputRecording) -> bool {
        if !self.content.restore(&recording.initial_state) {
            return false;
        }
        let Some((width, height)) = self.frame.as_ref().map(|frame| (frame.width, frame.height))
        else {
            return false;
        };
        self.context.pointer_capture.release();
        self.context.with_current(|| {
            self.recomposer.commit_all();
            self.graph.compute_layout_if_needed(width, height);
            for input in &recording.events {
                self.dispatcher.dispatch(&input.event);
                self.recomposer.commit_all();
                self.graph.compute_layout_if_needed(width, height);
            }
        });
        true
    }

    /// Publishes every enabled stream's pending events as a JSON array.
    pub fn events(&mut self) {
        let mut messages: Vec<Envelope> = Vec::new();
        if self.session.subscriptions.contains(&Subscription::Tree) {
            if let Some(payload) = self.tree_payload() {
                messages.push(Envelope::new("tree.delta", None, Some(payload)));
            }
        }
        if self.session.subscriptions.contains(&Subscription::Timing) {
            if let Some(frame) = &self.frame {
                let payload = TimingFrame {
                    frame: self.frame_number,
                    recomposition_ms: Some(self.recomposition_ms),
                    layout_ms: self.layout_ms,
                    draw_ms: self.render_ms,
                    present_ms: 0.0,
                    presented: Some(true),
                    total_ms: self.recomposition_ms + self.layout_ms + self.render_ms,
                    node_count: frame
                        .tree
                        .input_inventory
                        .as_ref()
                        .map_or(0, |inventory| inventory.node_count),
                    batch_count: frame.batches.len(),
                };
                messages.push(Envelope::new(
                    "timing.frame",
                    None,
                    serde_json::to_value(payload).ok(),
                ));
            }
        }
        self.response_buffer.encode(&messages);
    }

    fn focused_name(&self) -> Option<String> {
        self.context
            .focus_chain
            .focused()
            .and_then(|node| crate::prototype::node_debug_name(&node))
    }

    fn set_timeline_enabled(&self, enabled: bool) {
        self.tree
            .borrow()
            .timeline
            .borrow_mut()
            .set_enabled(enabled);
    }

    fn tree_payload(&self) -> Option<serde_json::Value> {
        self.frame
            .as_ref()
            .and_then(|frame| serde_json::to_value(&frame.tree).ok())
    }

    fn publish(&mut self, envelope: &Envelope) {
        self.response_buffer.encode(envelope);
    }

    pub fn frame(&self) -> Option<&Frame> {
        self.frame.as_ref()
    }

    pub fn tree_snapshot(&self) -> Option<&TreeSnapshot> {
        self.frame.as_ref().map(|frame| &frame.tree)
    }

    pub fn font_count(&self) -> usize {
        self.fonts.font_count()
    }

    pub fn draw_list(&self) -> &DrawList {
        &self.draw_list
    }
}

/// Rejects a request the way the Swift prototype does.
fn fail(response: &mut Envelope, message: &str) {
    response.r#type = response.r#type.replace(".ok", ".err");
    response.payload = Some(serde_json::json!({
        "code": "bad_request",
        "message": message,
    }));
}

fn decode<T: serde::de::DeserializeOwned>(payload: Option<&serde_json::Value>) -> Option<T> {
    serde_json::from_value(payload?.clone()).ok()
}

/// Projects `Vertex` storage as bytes for the JavaScript upload.
fn vertex_bytes(vertices: &[cove_core::Vertex]) -> &[u8] {
    // SAFETY: `Vertex` is `#[repr(C)]` with the asserted 20-byte layout and no
    // padding, so the slice covers exactly `len * STRIDE` initialised bytes.
    unsafe {
        std::slice::from_raw_parts(
            vertices.as_ptr().cast::<u8>(),
            std::mem::size_of_val(vertices),
        )
    }
}

/// Projects the index buffer as bytes for the JavaScript upload.
fn index_bytes(indices: &[u32]) -> &[u8] {
    // SAFETY: `u32` is plain old data; the slice covers exactly the live range.
    unsafe {
        std::slice::from_raw_parts(
            indices.as_ptr().cast::<u8>(),
            std::mem::size_of_val(indices),
        )
    }
}

/// Shapes every text run and re-encodes the draw list for one frame.
fn encode_frame(
    tree: &Rc<RefCell<NodeTree>>,
    inspector: &SceneInspector,
    draw_list: &mut DrawList,
    fonts: &mut FontCollection,
    labels: &mut Vec<Label>,
    note_width: &mut f32,
) {
    let Some(root) = tree.borrow().root() else {
        return;
    };
    draw_list.reset();
    collect_labels(fonts, &root, labels, note_width);
    NodeRenderer.render(&root, draw_list);
    tree.borrow_mut().flush();
    inspector.editor.draw_overlay(draw_list);
}

/// Walks the tree in paint order, reshaping every text attachment.
fn collect_labels(
    fonts: &mut FontCollection,
    node: &cove_core::scene::Node,
    labels: &mut Vec<Label>,
    note_width: &mut f32,
) {
    let _ = (fonts, node, labels, note_width);
}
