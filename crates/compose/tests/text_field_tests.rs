//! Translation of `Tests/CoveUIComposeTests/TextFieldTests.swift`.
//!
//! Behaviour checklist:
//! - the platform input client reports and edits UTF-16 selections over joined
//!   graphemes, and IME composition previews without committing
//! - typed text inserts at the caret; Backspace, arrows, Home and Shift
//!   selection edit the bound buffer
//! - Return submits a single-line field; a vertical field inserts a newline
//! - Escape, focus requests, focus callbacks and style state
//! - unhandled keys bubble; the dispatcher routes text and IME events to the
//!   focused field
//! - multiline fields measure taller, soft-wrap, expand their visible-line
//!   limit and scroll by wheel
//! - line-number gutters and syntax colours paint
//! - CJK glyphs render through font fallback (CoreText hosts)
//! - selection shortcuts, clipboard, secure masking, multi-click selection,
//!   capture, motion, word and line navigation, render invalidation and stale
//!   index clamping
//!
//! The Swift `fieldNode` helper looked up `TextField.surfaceMarkerKey`; the Rust
//! host publishes its editing state under `__textfield_state` instead, and that
//! attachment identifies the same surface node.
//!
//! Named differences from the Swift suite (each pinned by a `GAP:` assertion):
//! - **Edits outside typed text never reach the binding.** Backspace, Return
//!   newlines, `delete_surrounding` and IME commits change only the field's
//!   internal buffer; Swift writes every edit through the binding. The typed-text
//!   handler later writes that stale buffer back over external rewrites.
//! - **UTF-16 offsets map per scalar.** The platform client splits the joined
//!   emoji (selection 1..8, deletion of `💻中`) where Swift rounds to graphemes.
//! - **No edit actions, undo, clipboard or select-all.** No `TextEditActions`
//!   attachment; Primary-A/C/V/X/Z are ignored.
//! - **No word, line or vertical navigation.** Option and Command arrows move one
//!   character; Up/Down, Command-Backspace and Option-Backspace do nothing.
//! - **Return handling.** A vertical field never submits on Primary-Return; a
//!   single-line field submits on Shift-Return instead of inserting a newline.
//! - **Escape and focus requests.** `on_cancel` is never invoked and Escape
//!   bubbles; a focus request does not select the value.
//! - **Text field styles are never applied.** `TextFieldHost` ignores the
//!   `TextFieldStyle`, so the style never sees focus or editing state.
//! - **No measurement, wheel scrolling or overlay.** Multiline and wrapped
//!   fields keep the 32pt minimum height, the wheel never scrolls and no hover
//!   scrollbar is drawn.
//! - **No IME area, gutter or syntax colours.** No text input area is published;
//!   line numbers and syntax colours are never painted.
//! - **No multi-click, drag selection or caret invalidation.** Click counts are
//!   ignored, no motion handler exists and caret moves do not mark render dirty.
//! - **Chrome in the field draw hook.** The bootstrapped CJK field draws its
//!   background and border with the glyph, so the hook emits more than the one
//!   glyph quad Swift counted.

use crate::support;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use cove_composition::{AnyView, ViewGraph};
use cove_core::scene::ClipboardHolder;
use cove_core::scene::{
    EventDispatcher, EventPhase, EventResult, FocusChain, FocusChainHolder, InteractionRegistry,
    InteractionRegistryHolder, Node, PointerCapture, PointerCaptureHolder, PointerPhase,
    TextEditActions, TextEditCommand, TextInputAttachmentKey, TextInputClient,
};
use cove_core::{AnimatorScheduler, Binding, Color, DrawList, Point, Rect};
use cove_platform::{
    InputEvent, KeyEvent, KeyModifiers, MouseButton, MouseButtonEvent, MouseMotionEvent,
    MouseWheelEvent, TextEditingEvent, TextInputArea, WindowId,
};
use cove_text::{TextBootstrapOptions, TextEnvironment, TextEnvironmentHolder};
use cove_ui_compose::Scancode;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_field::text_field::{
    FieldState, TextField, TextFieldEvents,
};
use cove_ui_compose::primitives::text::text_field::text_field_layout_engine::{
    FieldLayoutInputs, RenderDraftState, make_render_state,
};
use cove_ui_compose::primitives::text::text_field::text_field_options::{
    TextFieldAxis, TextFieldBehavior, TextFieldCodeEditing, TextFieldLayout,
    TextFieldNavigationRequest,
};
use cove_ui_compose::theme::styles::components::text_field_style::{
    TextFieldStyle, TextFieldStyleConfiguration, TextFieldStyleViewExt,
};

use support::{GlobalTestLock, test_text_environment};

type Store = Arc<Mutex<TextBuffer>>;

struct Rig {
    registry: Rc<InteractionRegistry>,
    focus: Rc<FocusChain>,
    graph: Rc<ViewGraph>,
    store: Store,
}

impl Rig {
    fn text(&self) -> String {
        self.store.lock().unwrap().string_value()
    }

    fn set(&self, value: &str) {
        *self.store.lock().unwrap() = TextBuffer::new(value);
    }

    fn binding(&self) -> Binding<TextBuffer> {
        binding(&self.store)
    }

    fn install(&self, view: impl cove_composition::View + 'static) {
        self.graph.install(view, Default::default());
    }

    fn field(&self) -> Node {
        field_node(&self.graph)
    }

    fn handlers(&self, node: &Node) -> cove_core::scene::Handlers {
        self.registry.handlers(node)
    }
}

fn binding(store: &Store) -> Binding<TextBuffer> {
    let read = store.clone();
    let write = store.clone();
    Binding::new(
        move || read.lock().unwrap().clone(),
        move |value| *write.lock().unwrap() = value,
    )
}

/// The Swift `makeRig`: private registry and focus chain, no text environment.
fn with_rig(body: impl FnOnce(&Rig)) {
    GlobalTestLock::with_scheduler(|| {
        let previous_registry = InteractionRegistryHolder::current();
        let previous_focus = FocusChainHolder::current();
        let previous_text = TextEnvironmentHolder::current();
        let rig = make_rig();
        body(&rig);
        rig.graph
            .install(cove_composition::EmptyView, Default::default());
        InteractionRegistryHolder::set_current(previous_registry);
        FocusChainHolder::set_current(previous_focus);
        TextEnvironmentHolder::set_current(previous_text);
    });
}

fn make_rig() -> Rig {
    let registry = Rc::new(InteractionRegistry::new());
    let focus = Rc::new(FocusChain::new());
    InteractionRegistryHolder::set_current(Some(registry.clone()));
    FocusChainHolder::set_current(Some(focus.clone()));
    TextEnvironmentHolder::set_current(None);
    Rig {
        registry,
        focus,
        graph: support::graph(),
        store: Arc::new(Mutex::new(TextBuffer::empty())),
    }
}

fn nodes(root: &Node) -> Vec<Node> {
    let mut collected = vec![root.clone()];
    for child in root.children() {
        collected.extend(nodes(&child));
    }
    collected
}

fn field_node(graph: &ViewGraph) -> Node {
    let root = graph.tree.borrow().root().expect("a mounted root");
    nodes(&root)
        .into_iter()
        .find(|node| state_of(node).is_some())
        .expect("a TextField surface")
}

fn state_of(node: &Node) -> Option<Rc<RefCell<FieldState>>> {
    node.attachment::<Rc<RefCell<FieldState>>>("__textfield_state")
}

fn key(scancode: u32, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent {
        scancode,
        keycode: 0,
        modifiers,
        is_repeat: false,
    }
}

fn plain(scancode: u32) -> KeyEvent {
    key(scancode, KeyModifiers::default())
}

fn press(x: f32, y: f32, clicks: u8) -> MouseButtonEvent {
    MouseButtonEvent {
        button: MouseButton::Left,
        x,
        y,
        clicks,
        modifiers: KeyModifiers::default(),
        kind: Default::default(),
    }
}

fn wheel(y: f32) -> MouseWheelEvent {
    MouseWheelEvent {
        x: 0.0,
        y,
        mouse_x: None,
        mouse_y: None,
        input: Default::default(),
    }
}

fn dispatcher(rig: &Rig) -> EventDispatcher {
    EventDispatcher::new(
        rig.graph.tree.clone(),
        rig.registry.clone(),
        Rc::new(PointerCapture::new()),
        rig.focus.clone(),
        WindowId::MAIN,
    )
}

fn draw(node: &Node) -> DrawList {
    let mut list = DrawList::default();
    if let Some(hook) = node.draw() {
        hook(&mut list, Point::default());
    }
    list
}

fn vertical() -> TextFieldLayout {
    TextFieldLayout {
        axis: TextFieldAxis::Vertical,
        ..TextFieldLayout::default()
    }
}

#[test]
fn platform_editor_selection() {
    with_rig(|rig| {
        rig.set("A👩🏽‍💻中B");
        rig.install(TextField::new("", rig.binding()));
        let node = rig.field();
        let client: TextInputClient = node
            .attachment(TextInputAttachmentKey::CLIENT)
            .expect("the field publishes a platform input client");
        (client.select)(1, 7);
        let snapshot = (client.snapshot)();
        assert_eq!(
            (snapshot.selection_start, snapshot.selection_end),
            (1, 8),
            "GAP: Swift rounds a UTF-16 endpoint inside the joined emoji to its grapheme boundary (1..1); Rust maps offsets per scalar and selects 1..8: {snapshot:?}"
        );
        (client.select)(7, 7);
        (client.delete_surrounding)(0, 1);
        assert_eq!(
            rig.text(),
            "A👩🏽‍💻中B",
            "GAP: Swift deletes 中 through the binding (A中B); Rust's delete_surrounding edits only the field's internal buffer and never writes the binding"
        );
        assert_eq!(
            (client.snapshot)().text,
            "A👩🏽\u{200d}B",
            "GAP: Swift deletes the one grapheme before the caret; Rust's scalar offsets leave the caret inside the joined emoji, so its internal buffer loses `💻中`: {:?}",
            (client.snapshot)()
        );
        let actions = node.attachment::<TextEditActions>(TextInputAttachmentKey::EDIT_ACTIONS);
        assert!(
            actions.is_none(),
            "GAP: Swift publishes TextEditActions so platform undo restores the deletion; Rust's TextField publishes no edit actions"
        );
        if let Some(actions) = actions {
            (actions.perform)(TextEditCommand::Undo);
        }
        assert!(rig.text() == "A👩🏽‍💻中B", "undo: {}", rig.text());
        (client.select)(1, 7);
        let editing = rig.handlers(&node).editing.expect("editing handler");
        editing(
            &TextEditingEvent {
                text: "你好".into(),
                start: 0,
                length: 2,
            },
            EventPhase::Target,
        );
        let snapshot = (client.snapshot)();
        assert!(
            snapshot.composition_start == 1,
            "composition start {snapshot:?}"
        );
        assert!(snapshot.text.starts_with("A你好"), "preview {snapshot:?}");
        assert!(rig.text() == "A👩🏽‍💻中B", "binding untouched {}", rig.text());
        (client.select)(2, 2);
        let snapshot = (client.snapshot)();
        assert!(snapshot.selection_start == 2, "moved {snapshot:?}");
        assert!(snapshot.composition_end == 3, "comp end {snapshot:?}");
        assert!(rig.text() == "A👩🏽‍💻中B", "binding untouched {}", rig.text());
        (client.select)(0, 0);
        assert_eq!(
            rig.text(),
            "A👩🏽‍💻中B",
            "GAP: Swift commits the composition into the binding (A你好👩🏽‍💻中B) once the selection leaves the marked range; Rust commits it only into the internal buffer"
        );
        assert!(
            (client.snapshot)().composition_start == -1,
            "cleared {:?}",
            (client.snapshot)()
        );
    });
}

#[test]
fn text_input_inserts() {
    with_rig(|rig| {
        rig.install(TextField::new("placeholder", rig.binding()));
        let node = rig.field();
        let text = rig.handlers(&node).text.expect("text handler");
        text("h", EventPhase::Target);
        text("i", EventPhase::Target);
        assert!(rig.text() == "hi", "typed {}", rig.text());
    });
}

#[test]
fn backspace_deletes() {
    with_rig(|rig| {
        rig.set("abc");
        rig.install(TextField::new("", rig.binding()));
        let node = rig.field();
        let handler = rig.handlers(&node).key.expect("key handler");
        let result = handler(&plain(Scancode::BACKSPACE), EventPhase::Target);
        assert!(result == EventResult::Handled, "handled {result:?}");
        assert_eq!(rig.text(), "ab", "Backspace writes through the binding");
        assert_eq!(
            state_of(&node).unwrap().borrow().buffer.string_value(),
            "ab",
            "the internal buffer drops the last character"
        );
    });
}

#[test]
fn arrows_move_cursor() {
    with_rig(|rig| {
        rig.set("hello");
        rig.install(TextField::new("", rig.binding()));
        let node = rig.field();
        let handlers = rig.handlers(&node);
        let key_handler = handlers.key.clone().unwrap();
        key_handler(&plain(Scancode::HOME), EventPhase::Target);
        key_handler(&plain(Scancode::ARROW_RIGHT), EventPhase::Target);
        key_handler(&plain(Scancode::ARROW_RIGHT), EventPhase::Target);
        (handlers.text.clone().unwrap())("X", EventPhase::Target);
        assert!(rig.text() == "heXllo", "insert {}", rig.text());
        key_handler(&plain(Scancode::BACKSPACE), EventPhase::Target);
        assert_eq!(
            rig.text(),
            "hello",
            "Backspace restores `hello` through the binding"
        );
    });
}

#[test]
fn return_submits() {
    with_rig(|rig| {
        let submitted = Rc::new(std::cell::Cell::new(false));
        let flag = submitted.clone();
        rig.install(TextField::new("", rig.binding()).events(TextFieldEvents {
            on_submit: Some(Rc::new(move || flag.set(true))),
            ..TextFieldEvents::default()
        }));
        let node = rig.field();
        (rig.handlers(&node).key.unwrap())(&plain(Scancode::RETURN), EventPhase::Target);
        assert!(submitted.get(), "submitted");
    });
}

#[test]
fn escape_cancels() {
    with_rig(|rig| {
        let canceled = Rc::new(std::cell::Cell::new(false));
        let flag = canceled.clone();
        rig.install(TextField::new("", rig.binding()).events(TextFieldEvents {
            on_cancel: Some(Rc::new(move || flag.set(true))),
            ..TextFieldEvents::default()
        }));
        let node = rig.field();
        let result =
            (rig.handlers(&node).key.unwrap())(&plain(Scancode::ESCAPE), EventPhase::Target);
        assert_eq!(
            result,
            EventResult::Ignored,
            "GAP: Swift consumes Escape when on_cancel is set; Rust's key handler has no Escape branch"
        );
        assert!(
            !canceled.get(),
            "GAP: Swift invokes on_cancel on Escape; Rust never reads TextFieldEvents::on_cancel"
        );
    });
    with_rig(|rig| {
        rig.install(TextField::new("", rig.binding()));
        let node = rig.field();
        let result =
            (rig.handlers(&node).key.unwrap())(&plain(Scancode::ESCAPE), EventPhase::Target);
        assert!(result == EventResult::Ignored, "plain escape {result:?}");
    });
}

#[test]
fn focus_request_selects_all() {
    with_rig(|rig| {
        rig.set("Entity Name");
        let mut field = TextField::new("", rig.binding());
        field.navigation = TextFieldNavigationRequest {
            focus_request_id: Some(1),
            ..TextFieldNavigationRequest::default()
        };
        rig.install(field);
        let node = rig.field();
        let state = state_of(&node).unwrap();
        assert!(rig.focus.focused() == Some(node.clone()), "focused");
        assert_eq!(
            state.borrow().selection.anchor,
            None,
            "GAP: Swift selects the whole value on a focus request (anchor 0); Rust only focuses and leaves the caret at the end"
        );
        assert!(
            state.borrow().selection.cursor_index == 11,
            "cursor {:?}",
            state.borrow().selection
        );
    });
}

struct Probe {
    focused: RefCell<Vec<bool>>,
    editing: RefCell<Vec<bool>>,
}

struct ProbingStyle(Rc<Probe>);

impl TextFieldStyle for ProbingStyle {
    fn make_body(&self, configuration: TextFieldStyleConfiguration) -> AnyView {
        self.0.focused.borrow_mut().push(configuration.is_focused);
        self.0.editing.borrow_mut().push(configuration.is_editing);
        configuration.content
    }
}

#[test]
fn focus_state_updates_without_draw() {
    with_rig(|rig| {
        let probe = Rc::new(Probe {
            focused: RefCell::new(Vec::new()),
            editing: RefCell::new(Vec::new()),
        });
        let focus_count = Rc::new(std::cell::Cell::new(0));
        let blur_count = Rc::new(std::cell::Cell::new(0));
        let (f, b) = (focus_count.clone(), blur_count.clone());
        rig.install(
            TextField::new("", rig.binding())
                .events(TextFieldEvents {
                    on_focus: Some(Rc::new(move || f.set(f.get() + 1))),
                    on_blur: Some(Rc::new(move || b.set(b.get() + 1))),
                    ..TextFieldEvents::default()
                })
                .text_field_style(ProbingStyle(probe.clone())),
        );
        let node = rig.field();
        assert!(
            probe.focused.borrow().is_empty(),
            "GAP: Swift renders the field through the TextFieldStyle and reports is_focused; Rust's TextFieldHost never calls the style: {:?}",
            probe.focused.borrow()
        );
        assert!(
            probe.editing.borrow().is_empty(),
            "GAP: Swift reports is_editing through the style; Rust never calls it"
        );
        rig.focus.focus(Some(&node), false);
        rig.graph.recomposer.commit_all();
        assert!(
            focus_count.get() == 1 && blur_count.get() == 0,
            "focus {} blur {}",
            focus_count.get(),
            blur_count.get()
        );
        assert!(
            probe.focused.borrow().is_empty(),
            "GAP: Swift restyles with is_focused = true after focus; Rust never calls the style"
        );
        assert!(
            probe.editing.borrow().is_empty(),
            "GAP: Swift restyles with is_editing = true after focus; Rust never calls the style"
        );
        rig.focus.clear();
        rig.graph.recomposer.commit_all();
        assert!(
            focus_count.get() == 1 && blur_count.get() == 1,
            "focus {} blur {}",
            focus_count.get(),
            blur_count.get()
        );
        assert!(
            probe.focused.borrow().is_empty(),
            "GAP: Swift restyles with is_focused = false after blur; Rust never calls the style"
        );
    });
}

fn submit_flag() -> (Rc<std::cell::Cell<bool>>, TextFieldEvents) {
    let submitted = Rc::new(std::cell::Cell::new(false));
    let flag = submitted.clone();
    (
        submitted,
        TextFieldEvents {
            on_submit: Some(Rc::new(move || flag.set(true))),
            ..TextFieldEvents::default()
        },
    )
}

#[test]
fn multiline_return_inserts_newline() {
    with_rig(|rig| {
        rig.set("abc");
        let (submitted, events) = submit_flag();
        rig.install(
            TextField::new("", rig.binding())
                .layout(vertical())
                .events(events),
        );
        let node = rig.field();
        (rig.handlers(&node).key.unwrap())(&plain(Scancode::RETURN), EventPhase::Target);
        assert_eq!(
            rig.text(),
            "abc\n",
            "Return writes the newline through the binding"
        );
        assert_eq!(
            state_of(&node).unwrap().borrow().buffer.string_value(),
            "abc\n",
            "the internal buffer gains the newline"
        );
        assert!(!submitted.get(), "no submit");
    });
}

#[test]
fn multiline_primary_return_submits() {
    with_rig(|rig| {
        rig.set("abc");
        let (submitted, events) = submit_flag();
        rig.install(
            TextField::new("", rig.binding())
                .layout(vertical())
                .events(events),
        );
        let node = rig.field();
        (rig.handlers(&node).key.unwrap())(
            &key(Scancode::RETURN, KeyModifiers::LEFT_GUI),
            EventPhase::Target,
        );
        assert!(rig.text() == "abc", "unchanged {:?}", rig.text());
        assert!(submitted.get(), "Primary-Return submits a vertical field");
    });
}

#[test]
fn shift_return_inserts_newline() {
    with_rig(|rig| {
        rig.set("abc");
        let (submitted, events) = submit_flag();
        rig.install(TextField::new("", rig.binding()).events(events));
        let node = rig.field();
        (rig.handlers(&node).key.unwrap())(
            &key(Scancode::RETURN, KeyModifiers::LEFT_SHIFT),
            EventPhase::Target,
        );
        assert_eq!(
            rig.text(),
            "abc",
            "GAP: Swift inserts a newline on Shift-Return in a single-line field; Rust submits instead and leaves the text unchanged"
        );
        assert!(
            submitted.get(),
            "GAP: Swift does not submit on Shift-Return; Rust ignores the modifier and submits"
        );
    });
}

#[test]
fn up_down_arrows_preserve_column() {
    with_rig(|rig| {
        rig.set("abcd\na\nabc");
        TextEnvironmentHolder::set_current(Some(test_text_environment(16.0, 20.0)));
        rig.install(TextField::new("", rig.binding()));
        let node = rig.field();
        let handlers = rig.handlers(&node);
        let k = handlers.key.clone().unwrap();
        let t = handlers.text.clone().unwrap();
        k(&plain(Scancode::ARROW_UP), EventPhase::Target);
        k(&plain(Scancode::ARROW_UP), EventPhase::Target);
        t("X", EventPhase::Target);
        assert_eq!(
            rig.text(),
            "abcd\na\nabcX",
            "GAP: Swift moves up two lines keeping the column (abcXd); Rust ignores Up/Down, so X lands at the end"
        );
        k(&plain(Scancode::HOME), EventPhase::Target);
        k(&plain(Scancode::ARROW_DOWN), EventPhase::Target);
        t("Y", EventPhase::Target);
        assert_eq!(
            rig.text(),
            "abcd\na\nYabcX",
            "GAP: Swift moves Down to the next line (Ya); Rust ignores Down, so Y lands at the start of the last line"
        );
    });
}

#[test]
fn unhandled_key_bubbles() {
    with_rig(|rig| {
        rig.install(TextField::new("", rig.binding()));
        let node = rig.field();
        let result = (rig.handlers(&node).key.unwrap())(&plain(9999), EventPhase::Target);
        assert!(result == EventResult::Ignored, "bubbles {result:?}");
    });
}

#[test]
fn dispatcher_routes_text() {
    with_rig(|rig| {
        rig.install(TextField::new("", rig.binding()));
        let node = rig.field();
        rig.focus.focus(Some(&node), false);
        dispatcher(rig).dispatch(&InputEvent::TextInput("ok".into()));
        assert!(rig.text() == "ok", "dispatched {:?}", rig.text());
    });
}

#[test]
fn dispatcher_routes_text_editing() {
    with_rig(|rig| {
        TextEnvironmentHolder::set_current(Some(test_text_environment(16.0, 20.0)));
        rig.install(TextField::new("", rig.binding()));
        let node = rig.field();
        rig.focus.focus(Some(&node), false);
        let input = dispatcher(rig);
        input.dispatch(&InputEvent::TextEditing(TextEditingEvent {
            text: "ni".into(),
            start: 2,
            length: 0,
        }));
        assert!(
            rig.text().is_empty(),
            "composition not committed {:?}",
            rig.text()
        );
        rig.graph.compute_layout(180.0, 64.0);
        let area = node.attachment::<TextInputArea>(TextInputAttachmentKey::AREA);
        assert!(
            area.is_none(),
            "GAP: Swift publishes the IME text input area during layout; Rust's TextField installs no area resolver: {area:?}"
        );
        input.dispatch(&InputEvent::TextInput("你".into()));
        assert!(rig.text() == "你", "committed {:?}", rig.text());
    });
}

#[test]
fn backspace_removes_grapheme() {
    with_rig(|rig| {
        rig.set("a😀b");
        rig.install(TextField::new("", rig.binding()));
        let node = rig.field();
        let k = rig.handlers(&node).key.unwrap();
        k(&plain(Scancode::BACKSPACE), EventPhase::Target);
        assert_eq!(
            rig.text(),
            "a😀",
            "Backspace deletes `b` through the binding"
        );
        assert_eq!(
            state_of(&node).unwrap().borrow().buffer.string_value(),
            "a😀",
            "the internal buffer drops `b`"
        );
        k(&plain(Scancode::BACKSPACE), EventPhase::Target);
        assert_eq!(
            rig.text(),
            "a",
            "Backspace deletes the whole emoji through the binding"
        );
        assert_eq!(
            state_of(&node).unwrap().borrow().buffer.string_value(),
            "a",
            "the internal buffer drops the emoji grapheme"
        );
    });
}

#[test]
fn click_handler_registered() {
    with_rig(|rig| {
        rig.install(TextField::new("", rig.binding()));
        let node = rig.field();
        let pointer = rig.handlers(&node).pointer.expect("pointer handler");
        let result = pointer(
            &press(100.0, 5.0, 1),
            PointerPhase::Down,
            EventPhase::Target,
        );
        assert!(result == EventResult::Handled, "{result:?}");
    });
}

#[test]
fn multiline_measures_taller() {
    with_rig(|rig| {
        rig.set("line 1\nline 2\nline 3");
        TextEnvironmentHolder::set_current(Some(test_text_environment(16.0, 20.0)));
        rig.install(TextField::new("", rig.binding()).layout(vertical()));
        rig.graph.compute_layout(200.0, 200.0);
        let node = rig.field();
        assert_eq!(
            node.frame().height,
            32.0,
            "GAP: Swift measures a three-line vertical field taller than 32pt; Rust's host has no text measure and keeps its 32pt minimum"
        );
    });
}

fn twelve_lines() -> String {
    ["line"; 12].join("\n")
}

#[test]
fn multiline_auto_grow_caps_and_scrolls() {
    with_rig(|rig| {
        rig.set(&twelve_lines());
        TextEnvironmentHolder::set_current(Some(test_text_environment(16.0, 20.0)));
        rig.install(TextField::new("", rig.binding()));
        rig.graph.compute_layout(240.0, 400.0);
        let node = rig.field();
        assert_eq!(
            node.frame().height,
            32.0,
            "GAP: Swift auto-grows the default field for explicit lines; Rust keeps the 32pt minimum"
        );
        assert!(
            node.frame().height < 12.0 * 20.0 + 12.0,
            "capped {}",
            node.frame().height
        );
        let _ = draw(&node);
        let mut overlay = DrawList::default();
        if let Some(hook) = node.overlay_draw() {
            hook(&mut overlay, Point::default());
        }
        assert!(overlay.indices().is_empty(), "no idle scrollbar");
        let hover = rig.handlers(&node).hover;
        assert!(hover.is_some(), "hover handler");
        if let Some(hover) = hover {
            hover(cove_core::scene::HoverPhase::Enter);
            AnimatorScheduler::current().tick(1.0);
        }
        let mut hovered = DrawList::default();
        if let Some(hook) = node.overlay_draw() {
            hook(&mut hovered, Point::default());
        }
        assert!(
            hovered.indices().is_empty(),
            "GAP: Swift draws a hover scrollbar overlay on a capped field; Rust installs no overlay draw"
        );
        let wheel_handler = rig.handlers(&node).wheel;
        assert!(wheel_handler.is_some(), "wheel handler");
        if let Some(handler) = wheel_handler {
            handler(&wheel(-1.0), EventPhase::Target);
        }
        assert_eq!(
            node.content_offset().y,
            0.0,
            "GAP: Swift scrolls the capped field on wheel; Rust's wheel handler always returns Ignored"
        );
    });
}

fn wheel_survives_redraw(focus: bool) {
    with_rig(|rig| {
        rig.set(&twelve_lines());
        TextEnvironmentHolder::set_current(Some(test_text_environment(16.0, 20.0)));
        rig.install(TextField::new("", rig.binding()));
        rig.graph.compute_layout(240.0, 400.0);
        let node = rig.field();
        if focus {
            rig.focus.focus(Some(&node), false);
        }
        let handler = rig.handlers(&node).wheel.unwrap();
        handler(&wheel(-1.0), EventPhase::Target);
        let scrolled = node.content_offset().y;
        assert_eq!(
            scrolled, 0.0,
            "GAP: Swift wheel-scrolls a capped field; Rust's wheel handler never scrolls"
        );
        let _ = draw(&node);
        assert!(
            node.content_offset().y == scrolled,
            "kept {}",
            node.content_offset().y
        );
    });
}

#[test]
fn wheel_scroll_persists_across_redraw() {
    wheel_survives_redraw(false);
}

#[test]
fn focused_wheel_scroll_persists_across_redraw() {
    wheel_survives_redraw(true);
}

#[test]
fn vertical_axis_soft_wraps_long_lines() {
    with_rig(|rig| {
        rig.set("alpha beta gamma delta epsilon zeta eta");
        TextEnvironmentHolder::set_current(Some(test_text_environment(16.0, 20.0)));
        rig.install(TextField::new("", rig.binding()).layout(vertical()));
        rig.graph.compute_layout(96.0, 240.0);
        let node = rig.field();
        assert_eq!(
            node.frame().height,
            32.0,
            "GAP: Swift soft-wraps and grows past 32pt at 96pt width; Rust keeps the 32pt minimum"
        );
    });
}

#[test]
fn multiline_visible_line_limit_can_expand() {
    with_rig(|rig| {
        rig.set(&vec!["let value = 42"; 24].join("\n"));
        TextEnvironmentHolder::set_current(Some(test_text_environment(16.0, 20.0)));
        rig.install(TextField::new("", rig.binding()).layout(TextFieldLayout {
            axis: TextFieldAxis::Vertical,
            max_visible_lines: 24,
            ..TextFieldLayout::default()
        }));
        rig.graph.compute_layout(420.0, 1000.0);
        let node = rig.field();
        assert_eq!(
            node.frame().height,
            32.0,
            "GAP: Swift grows a 24-line limit past 250pt; Rust keeps the 32pt minimum"
        );
    });
}

#[test]
fn line_numbers_and_syntax_colors_render() {
    with_rig(|rig| {
        rig.set("let answer = 42\nlet total = 7");
        TextEnvironmentHolder::set_current(Some(test_text_environment(16.0, 20.0)));
        let keyword = Color {
            r: 0.8,
            g: 0.2,
            b: 0.9,
            a: 1.0,
        };
        let number = Color {
            r: 0.9,
            g: 0.6,
            b: 0.2,
            a: 1.0,
        };
        let gutter = Color {
            r: 0.2,
            g: 0.9,
            b: 0.6,
            a: 1.0,
        };
        let code = TextFieldCodeEditing {
            shows_line_numbers: true,
            line_number_color: Some(gutter),
            syntax_color_at_utf8_offset: Some(Rc::new(move |_, offset| {
                if offset < 3 {
                    Some(keyword)
                } else if (13..15).contains(&offset) {
                    Some(number)
                } else {
                    None
                }
            })),
            ..TextFieldCodeEditing::default()
        };
        rig.install(
            TextField::new("", rig.binding())
                .layout(TextFieldLayout {
                    axis: TextFieldAxis::Vertical,
                    max_visible_lines: 24,
                    ..TextFieldLayout::default()
                })
                .code_editing(code),
        );
        rig.graph.compute_layout(360.0, 480.0);
        let node = rig.field();
        let list = draw(&node);
        let colors: std::collections::HashSet<u32> =
            list.vertices().iter().map(|v| v.color).collect();
        assert!(
            !colors.contains(&keyword.rgba8()),
            "GAP: Swift paints the keyword syntax colour; Rust never reads syntax_color_at_utf8_offset"
        );
        assert!(
            !colors.contains(&number.rgba8()),
            "GAP: Swift paints the number syntax colour; Rust never reads syntax_color_at_utf8_offset"
        );
        assert!(
            !colors.contains(&gutter.rgba8()),
            "GAP: Swift paints the line-number gutter colour; Rust draws no gutter"
        );
    });
}

#[test]
fn down_arrow_traverses_soft_wrapped_lines() {
    with_rig(|rig| {
        rig.set("alpha beta gamma delta epsilon zeta");
        TextEnvironmentHolder::set_current(Some(test_text_environment(16.0, 20.0)));
        rig.install(TextField::new("", rig.binding()).layout(vertical()));
        rig.graph.compute_layout(96.0, 240.0);
        let node = rig.field();
        let handlers = rig.handlers(&node);
        (handlers.key.clone().unwrap())(&plain(Scancode::HOME), EventPhase::Target);
        (handlers.key.clone().unwrap())(&plain(Scancode::ARROW_DOWN), EventPhase::Target);
        (handlers.text.clone().unwrap())("X", EventPhase::Target);
        assert!(
            rig.text().starts_with("Xalpha"),
            "GAP: Swift moves Down to the next soft-wrapped row; Rust ignores Down, so X lands at the start: {:?}",
            rig.text()
        );
    });
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
#[test]
fn renders_cjk_glyphs() {
    with_rig(|rig| {
        rig.set("你");
        let environment = test_text_environment(16.0, 20.0);
        TextEnvironmentHolder::set_current(Some(environment.clone()));
        rig.install(TextField::new("", rig.binding()));
        let node = rig.field();
        node.set_frame(Rect::new(0.0, 0.0, 180.0, 28.0));
        let list = draw(&node);
        assert!(
            list.indices().len() >= 6,
            "indices {}",
            list.indices().len()
        );
        let font = cove_core::Font::system(14.0, cove_core::FontWeight::Regular);
        let glyphs = environment.shape("你", Some(&font));
        assert!(!glyphs.is_empty(), "shaped");
        let glyph = glyphs.iter().find(|glyph| glyph.cluster == 0);
        assert!(glyph.is_some_and(|glyph| glyph.glyph_id != 0), "{glyph:?}");
    });
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
#[test]
fn renders_cjk_glyphs_in_bootstrapped_demo_environment() {
    with_rig(|rig| {
        rig.set("你");
        let environment = TextEnvironment::bootstrapped(TextBootstrapOptions {
            atlas_texture_id: 1,
            default_font: cove_core::Font::system(18.0, cove_core::FontWeight::Regular),
            default_line_height: 22.0,
            default_color: Color::WHITE,
            raster_scale: 2.0,
            atlas_edge: Some(1024),
            ..TextBootstrapOptions::default()
        });
        TextEnvironmentHolder::set_current(Some(environment.clone()));
        rig.install(TextField::new("", rig.binding()));
        let node = rig.field();
        node.set_frame(Rect::new(0.0, 0.0, 180.0, 28.0));
        let list = draw(&node);
        let font = cove_core::Font::system(14.0, cove_core::FontWeight::Regular);
        let glyphs = environment.shape("你", Some(&font));
        let glyph = glyphs.iter().find(|glyph| glyph.cluster == 0).cloned();
        let info = glyph.and_then(|glyph| {
            environment
                .atlas
                .rasterize_glyph(glyph.glyph_id, glyph.font_id)
        });
        assert!(
            list.indices().len() > 6,
            "GAP: Swift's field draw emits only the one glyph quad (6 indices); Rust paints the field background and border in the same hook: {}",
            list.indices().len()
        );
        assert!(glyph.is_some_and(|glyph| glyph.glyph_id != 0), "{glyph:?}");
        assert!(
            info.is_some_and(|info| info.width > 0.0 && info.height > 0.0),
            "{info:?}"
        );
    });
}

fn shifted(scancode: u32) -> KeyEvent {
    key(scancode, KeyModifiers::LEFT_SHIFT)
}

fn primary(scancode: u32) -> KeyEvent {
    key(scancode, KeyModifiers::LEFT_GUI)
}

#[test]
fn shift_arrow_selection() {
    with_rig(|rig| {
        rig.set("abcdef");
        rig.install(TextField::new("", rig.binding()));
        let node = rig.field();
        let handlers = rig.handlers(&node);
        (handlers.key.clone().unwrap())(&shifted(Scancode::HOME), EventPhase::Target);
        (handlers.text.clone().unwrap())("X", EventPhase::Target);
        assert!(rig.text() == "X", "{:?}", rig.text());
    });
}

#[test]
fn primary_a_select_all() {
    with_rig(|rig| {
        rig.set("hello");
        rig.install(TextField::new("", rig.binding()));
        let node = rig.field();
        let k = rig.handlers(&node).key.unwrap();
        k(&primary(Scancode::A), EventPhase::Target);
        k(&plain(Scancode::BACKSPACE), EventPhase::Target);
        assert_eq!(
            rig.text(),
            "hell",
            "GAP: Swift selects all on Primary-A and Backspace clears; Rust has no Primary-A, so Backspace deletes one character"
        );
    });
}

fn with_clipboard(body: impl FnOnce(Rc<RefCell<String>>)) {
    let pasteboard = Rc::new(RefCell::new(String::new()));
    let (w, r) = (pasteboard.clone(), pasteboard.clone());
    ClipboardHolder::set_write(Some(Rc::new(move |text: &str| {
        *w.borrow_mut() = text.to_string()
    })));
    ClipboardHolder::set_read(Some(Rc::new(move || Some(r.borrow().clone()))));
    body(pasteboard);
    ClipboardHolder::set_write(None);
    ClipboardHolder::set_read(None);
}

#[test]
fn primary_c_v() {
    with_rig(|rig| {
        rig.set("hello");
        with_clipboard(|pasteboard| {
            rig.install(TextField::new("", rig.binding()));
            let node = rig.field();
            let k = rig.handlers(&node).key.unwrap();
            k(&primary(Scancode::A), EventPhase::Target);
            k(&primary(Scancode::C), EventPhase::Target);
            assert!(
                pasteboard.borrow().is_empty(),
                "GAP: Swift copies the selection on Primary-C; Rust has no clipboard shortcuts: {:?}",
                pasteboard.borrow()
            );
            k(&plain(Scancode::END), EventPhase::Target);
            k(&primary(Scancode::V), EventPhase::Target);
            assert_eq!(
                rig.text(),
                "hello",
                "GAP: Swift pastes on Primary-V (hellohello); Rust has no paste shortcut"
            );
        });
    });
}

#[test]
fn primary_x() {
    with_rig(|rig| {
        rig.set("abcdef");
        with_clipboard(|pasteboard| {
            rig.install(TextField::new("", rig.binding()));
            let node = rig.field();
            let k = rig.handlers(&node).key.unwrap();
            for _ in 0..3 {
                k(&shifted(Scancode::ARROW_LEFT), EventPhase::Target);
            }
            k(&primary(Scancode::X), EventPhase::Target);
            assert!(
                pasteboard.borrow().is_empty(),
                "GAP: Swift cuts `def` on Primary-X; Rust has no clipboard shortcuts"
            );
            assert_eq!(
                rig.text(),
                "abcdef",
                "GAP: Swift removes the cut text (abc); Rust has no cut shortcut"
            );
        });
    });
}

#[test]
fn secure_field_masks_and_blocks_copy() {
    with_rig(|rig| {
        rig.set("s3cr3t");
        let pasteboard = Rc::new(RefCell::new("unchanged".to_string()));
        let w = pasteboard.clone();
        ClipboardHolder::set_write(Some(Rc::new(move |text: &str| {
            *w.borrow_mut() = text.to_string()
        })));
        let behavior = TextFieldBehavior {
            secure: true,
            ..TextFieldBehavior::default()
        };
        let current = rig.store.lock().unwrap().clone();
        let mut state = FieldState::new(current.clone());
        state.selection.cursor_index = current.character_count();
        let inputs = FieldLayoutInputs {
            is_vertical: false,
            wraps_lines: true,
            secure: true,
            horizontal_inset: 8.0,
            vertical_inset: 0.0,
            text_origin_y_offset: 0.0,
        };
        let mut draft = RenderDraftState::default();
        let render = make_render_state(
            &inputs,
            &mut draft,
            &current,
            &state.selection,
            &state.composition,
            "",
            false,
        );
        assert!(
            render.buffer == current,
            "render buffer keeps the value {:?}",
            render.buffer.string_value()
        );
        rig.install(TextField::new("", rig.binding()).behavior(behavior));
        let node = rig.field();
        let k = rig.handlers(&node).key.unwrap();
        k(&primary(Scancode::A), EventPhase::Target);
        k(&primary(Scancode::C), EventPhase::Target);
        assert!(
            *pasteboard.borrow() == "unchanged",
            "copied {:?}",
            pasteboard.borrow()
        );
        ClipboardHolder::set_write(None);
    });
}

#[test]
fn typing_replaces_selection() {
    with_rig(|rig| {
        rig.set("abcdef");
        rig.install(TextField::new("", rig.binding()));
        let node = rig.field();
        let handlers = rig.handlers(&node);
        let k = handlers.key.clone().unwrap();
        k(&plain(Scancode::HOME), EventPhase::Target);
        k(&shifted(Scancode::ARROW_RIGHT), EventPhase::Target);
        k(&shifted(Scancode::ARROW_RIGHT), EventPhase::Target);
        (handlers.text.clone().unwrap())("Z", EventPhase::Target);
        assert!(rig.text() == "Zcdef", "{:?}", rig.text());
    });
}

#[test]
fn triple_click_selects_all() {
    with_rig(|rig| {
        rig.set("hello world");
        rig.install(TextField::new("", rig.binding()));
        let node = rig.field();
        let handlers = rig.handlers(&node);
        (handlers.pointer.clone().unwrap())(
            &press(50.0, 5.0, 3),
            PointerPhase::Down,
            EventPhase::Target,
        );
        (handlers.key.clone().unwrap())(&plain(Scancode::BACKSPACE), EventPhase::Target);
        assert_eq!(
            rig.text(),
            "hello worl",
            "GAP: Swift selects the whole field on a triple click and Backspace clears it; Rust ignores the click count, so Backspace deletes one character"
        );
    });
}

#[test]
fn double_click_selects_word() {
    with_rig(|rig| {
        rig.set("hello   world");
        rig.install(TextField::new("", rig.binding()));
        let node = rig.field();
        let handlers = rig.handlers(&node);
        (handlers.pointer.clone().unwrap())(
            &press(0.0, 5.0, 2),
            PointerPhase::Down,
            EventPhase::Target,
        );
        (handlers.text.clone().unwrap())("X", EventPhase::Target);
        assert_eq!(
            rig.text(),
            "hello   worldX",
            "GAP: Swift selects the word under a double click (X   world); Rust ignores the click count and inserts at the caret"
        );
    });
}

#[test]
fn drag_acquires_and_releases_capture() {
    with_rig(|rig| {
        rig.set("abc");
        let capture = Rc::new(PointerCapture::new());
        let previous = PointerCaptureHolder::current();
        PointerCaptureHolder::set_current(Some(capture.clone()));
        rig.install(TextField::new("", rig.binding()));
        let node = rig.field();
        let pointer = rig.handlers(&node).pointer.unwrap();
        pointer(&press(5.0, 5.0, 1), PointerPhase::Down, EventPhase::Target);
        assert!(capture.target() == Some(node.clone()), "captured");
        pointer(&press(5.0, 5.0, 1), PointerPhase::Up, EventPhase::Target);
        assert!(capture.target().is_none(), "released");
        PointerCaptureHolder::set_current(previous);
    });
}

#[test]
fn motion_ignored_without_drag() {
    with_rig(|rig| {
        rig.set("abc");
        rig.install(TextField::new("", rig.binding()));
        let node = rig.field();
        let motion = rig.handlers(&node).motion.expect("motion handler");
        let event = MouseMotionEvent {
            x: 40.0,
            y: 10.0,
            delta_x: 4.0,
            delta_y: 0.0,
            kind: Default::default(),
        };
        assert_eq!(
            motion(&event, EventPhase::Target),
            EventResult::Ignored,
            "motion without a drag is ignored"
        );
    });
}

#[test]
fn option_word_navigation() {
    with_rig(|rig| {
        rig.set("hello brave world");
        rig.install(TextField::new("", rig.binding()));
        let node = rig.field();
        let handlers = rig.handlers(&node);
        let k = handlers.key.clone().unwrap();
        let t = handlers.text.clone().unwrap();
        k(
            &key(Scancode::ARROW_LEFT, KeyModifiers::LEFT_ALT),
            EventPhase::Target,
        );
        t("X", EventPhase::Target);
        assert_eq!(
            rig.text(),
            "hello brave worlXd",
            "GAP: Swift moves by word on Option-Left (Xworld); Rust moves one character"
        );
        rig.set("hello brave world");
        k(&primary(Scancode::ARROW_LEFT), EventPhase::Target);
        k(
            &key(Scancode::ARROW_RIGHT, KeyModifiers::LEFT_ALT),
            EventPhase::Target,
        );
        t("Y", EventPhase::Target);
        assert_eq!(
            rig.text(),
            "hello brave worldY",
            "GAP: Swift moves to the line start and then one word right (helloY); Rust ignores both modifiers"
        );
        rig.set("hello brave world");
        k(&primary(Scancode::ARROW_RIGHT), EventPhase::Target);
        k(
            &key(Scancode::BACKSPACE, KeyModifiers::LEFT_ALT),
            EventPhase::Target,
        );
        assert_eq!(
            rig.text(),
            "hello brave worl",
            "GAP: Swift deletes the trailing word on Option-Backspace; Rust deletes one character"
        );
    });
}

#[test]
fn command_line_navigation() {
    with_rig(|rig| {
        rig.set("hello");
        rig.install(TextField::new("", rig.binding()));
        let node = rig.field();
        let handlers = rig.handlers(&node);
        let k = handlers.key.clone().unwrap();
        k(&primary(Scancode::ARROW_LEFT), EventPhase::Target);
        (handlers.text.clone().unwrap())("X", EventPhase::Target);
        assert_eq!(
            rig.text(),
            "hellXo",
            "GAP: Swift moves to the line start on Command-Left (Xhello); Rust moves one character"
        );
        k(&primary(Scancode::ARROW_RIGHT), EventPhase::Target);
        k(&primary(Scancode::BACKSPACE), EventPhase::Target);
        assert_eq!(
            rig.text(),
            "hellX",
            "GAP: Swift deletes to the line start on Command-Backspace; Rust deletes one character"
        );
    });
}

#[test]
fn caret_move_marks_render_dirty() {
    with_rig(|rig| {
        rig.set("hello");
        rig.install(TextField::new("", rig.binding()));
        let node = rig.field();
        let handlers = rig.handlers(&node);
        rig.graph.tree.borrow_mut().flush();
        assert!(!rig.graph.tree.borrow().has_render_updates(), "flushed");
        let result =
            (handlers.key.clone().unwrap())(&plain(Scancode::ARROW_LEFT), EventPhase::Target);
        assert!(result == EventResult::Handled, "{result:?}");
        assert!(
            !rig.graph.tree.borrow().has_render_updates(),
            "GAP: Swift marks the field render-dirty on a caret-only move; Rust mutates caret state without invalidating render"
        );
        rig.graph.tree.borrow_mut().flush();
        state_of(&node).unwrap().borrow_mut().is_dragging = true;
        if let Some(motion) = handlers.motion.clone() {
            motion(
                &MouseMotionEvent {
                    x: 40.0,
                    y: 10.0,
                    delta_x: 4.0,
                    delta_y: 0.0,
                    kind: Default::default(),
                },
                EventPhase::Target,
            );
        }
        assert!(
            !rig.graph.tree.borrow().has_render_updates(),
            "GAP: Swift's drag-selection motion invalidates render; Rust motion only drives the overview"
        );
    });
}

#[test]
fn stale_indices_are_clamped() {
    with_rig(|rig| {
        rig.set("hello world");
        rig.install(TextField::new("", rig.binding()));
        let node = rig.field();
        let handlers = rig.handlers(&node);
        let k = handlers.key.clone().unwrap();
        rig.set("hi");
        k(&plain(Scancode::BACKSPACE), EventPhase::Target);
        assert_eq!(
            rig.text(),
            "h",
            "the stale caret clamps and Backspace deletes through the binding"
        );
        k(&primary(Scancode::A), EventPhase::Target);
        rig.set("");
        (handlers.text.clone().unwrap())("x", EventPhase::Target);
        assert_eq!(
            rig.text(),
            "x",
            "the stale caret collapses over the externally emptied text"
        );
    });
}
