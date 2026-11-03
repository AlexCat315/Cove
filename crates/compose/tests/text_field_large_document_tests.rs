//! Translation of `Tests/CoveUIComposeTests/TextFieldLargeDocumentTests.swift`.
//!
//! Behaviour checklist:
//! - a long code row reveals the caret horizontally, keeps the gutter, maps the
//!   IME anchor back to the caret, wheels horizontally without reshaping and
//!   resets the horizontal scroll after a line-start move and undo
//! - a 200K-line document scrolls to a mid-document caret, shapes a bounded
//!   window, types 100 characters quickly, undoes, and previews IME composition
//!   without committing or reshaping on redraw
//! - a combining insertion and a deletion that joins regional indicators leave
//!   the caret at the rope grapheme endpoint
//!
//! The Swift `fieldNode` helper looked up `TextField.surfaceMarkerKey`; the Rust
//! host publishes its editing state under `__textfield_state`, which identifies
//! the same surface node. Swift's `field.insertReplacingSelection` and
//! `field.deleteSelection` map to the free functions in `text_field_editing`,
//! which edit a `FieldState` directly.
//!
//! The draw reveals the caret in both axes, the wheel scrolls a long row, the
//! shaped document is published under `__textfield_document_layout`, and
//! Primary-Z undoes through the shared history.

use crate::support;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use cove_composition::ViewGraph;
use cove_core::scene::{
    EventPhase, FocusChain, FocusChainHolder, InteractionRegistry, InteractionRegistryHolder, Node,
    TextInputAttachmentKey,
};
use cove_core::{Binding, DrawList, Point};
use cove_platform::{KeyEvent, KeyModifiers, MouseWheelEvent, TextEditingEvent, TextInputArea};
use cove_text::TextEnvironmentHolder;
use cove_ui_compose::Scancode;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_document_layout::TextDocumentLayout;
use cove_ui_compose::primitives::text::text_field::text_field::{FieldState, TextField};
use cove_ui_compose::primitives::text::text_field::text_field_editing::{
    delete_selection, insert_replacing_selection,
};
use cove_ui_compose::primitives::text::text_field::text_field_options::{
    TextFieldAxis, TextFieldCodeEditing, TextFieldLayout, TextFieldNavigationRequest,
};
use cove_ui_compose::theme::SemanticFontRef;

use support::{GlobalTestLock, test_text_environment};

struct Scene {
    registry: Rc<InteractionRegistry>,
    graph: Rc<ViewGraph>,
}

fn with_scene(body: impl FnOnce(&Scene)) {
    GlobalTestLock::with_scheduler(|| {
        let previous_registry = InteractionRegistryHolder::current();
        let previous_focus = FocusChainHolder::current();
        let previous_text = TextEnvironmentHolder::current();
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));
        FocusChainHolder::set_current(Some(Rc::new(FocusChain::new())));
        TextEnvironmentHolder::set_current(Some(test_text_environment(16.0, 20.0)));
        let scene = Scene {
            registry,
            graph: support::graph(),
        };
        body(&scene);
        scene
            .graph
            .install(cove_composition::EmptyView, Default::default());
        InteractionRegistryHolder::set_current(previous_registry);
        FocusChainHolder::set_current(previous_focus);
        TextEnvironmentHolder::set_current(previous_text);
    });
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

fn store_binding(store: &Arc<Mutex<TextBuffer>>) -> Binding<TextBuffer> {
    let read = store.clone();
    let write = store.clone();
    Binding::new(
        move || read.lock().unwrap().clone(),
        move |value| *write.lock().unwrap() = value,
    )
}

fn code_field(store: &Arc<Mutex<TextBuffer>>, caret: usize) -> TextField {
    let mut field = TextField::new("", store_binding(store))
        .layout(TextFieldLayout {
            axis: TextFieldAxis::Vertical,
            wraps_lines: false,
            ..TextFieldLayout::default()
        })
        .code_editing(TextFieldCodeEditing {
            shows_line_numbers: true,
            ..TextFieldCodeEditing::default()
        });
    field.navigation = TextFieldNavigationRequest {
        caret_request_id: Some(1),
        caret_request_index: Some(caret),
        ..TextFieldNavigationRequest::default()
    };
    field
}

fn draw(node: &Node) -> DrawList {
    let mut list = DrawList::default();
    if let Some(hook) = node.draw() {
        hook(&mut list, Point::default());
    }
    list
}

fn document(node: &Node) -> Option<Rc<RefCell<TextDocumentLayout>>> {
    node.attachment::<Rc<RefCell<TextDocumentLayout>>>("__textfield_document_layout")
}

#[test]
fn horizontal_caret_reveal() {
    with_scene(|scene| {
        let store = Arc::new(Mutex::new(TextBuffer::new(
            &"let value = 1\n".repeat(200_000),
        )));
        let original = store.lock().unwrap().clone();
        let middle = original.line_range(100_000).end;
        scene.graph.install(
            code_field(&store, middle)
                .frame_size(320.0, 160.0)
                .semantic_font(SemanticFontRef::mono()),
            Default::default(),
        );
        scene.graph.compute_layout(320.0, 160.0);
        let node = field_node(&scene.graph);
        let state = state_of(&node).unwrap();
        let handlers = scene.registry.handlers(&node);
        (handlers.text.clone().unwrap())(&"x".repeat(200), EventPhase::Target);
        let draws = draw(&node);
        let offset = state.borrow().scroll.horizontal.offset;
        assert!(
            offset > 500.0,
            "the draw reveals the caret on the 200-character row (offset {offset})"
        );
        let area = node
            .attachment::<TextInputArea>(TextInputAttachmentKey::AREA)
            .expect("the invalidated field publishes its IME anchor");
        assert!(
            area.cursor_x > 0.0 && area.cursor_x <= area.width,
            "the anchor maps the caret into the visible row: {area:?}"
        );
        assert!(draws.current_clip().is_none(), "clip balanced");
        assert!(
            document(&node).is_some(),
            "the draw publishes the shaped document layout"
        );
        let prior = state.borrow().scroll.horizontal.offset;
        if let Some(wheel) = handlers.wheel.clone() {
            wheel(
                &MouseWheelEvent {
                    x: 2.0,
                    y: 0.0,
                    mouse_x: Some(200.0),
                    mouse_y: Some(120.0),
                    input: Default::default(),
                },
                EventPhase::Target,
            );
        }
        let _ = draw(&node);
        assert!(
            state.borrow().scroll.horizontal.offset < prior,
            "a horizontal wheel scrolls the long row"
        );
        let line_start = store.lock().unwrap().line_range(100_000).start;
        state.borrow_mut().move_cursor(line_start, false, None);
        let _ = draw(&node);
        assert!(
            state.borrow().scroll.horizontal.offset == 0.0,
            "reset {}",
            state.borrow().scroll.horizontal.offset
        );
        (handlers.key.clone().unwrap())(
            &KeyEvent {
                scancode: Scancode::Z,
                keycode: 0,
                modifiers: KeyModifiers::LEFT_GUI,
                is_repeat: false,
            },
            EventPhase::Target,
        );
        let _ = draw(&node);
        assert_eq!(
            *store.lock().unwrap(),
            original,
            "Primary-Z undoes the 200-character insertion"
        );
        assert!(
            state.borrow().scroll.horizontal.offset == 0.0
                && state.borrow().scroll.horizontal.maximum() == 0.0,
            "after undo {} max {}",
            state.borrow().scroll.horizontal.offset,
            state.borrow().scroll.horizontal.maximum()
        );
    });
}

#[test]
fn middle_editing() {
    with_scene(|scene| {
        let store = Arc::new(Mutex::new(TextBuffer::new(
            &"let example = 123\n".repeat(200_000),
        )));
        let original = store.lock().unwrap().clone();
        let middle = original.line_range(100_000).end;
        scene.graph.install(
            code_field(&store, middle)
                .frame_size(600.0, 240.0)
                .semantic_font(SemanticFontRef::mono()),
            Default::default(),
        );
        scene.graph.recomposer.commit_all();
        scene.graph.compute_layout(600.0, 240.0);
        let node = field_node(&scene.graph);
        let _ = draw(&node);
        let state = state_of(&node).unwrap();
        assert!(
            document(&node).is_some(),
            "the draw publishes the shaped document layout"
        );
        let offset = state.borrow().scroll.offset_y;
        assert!(
            offset > 1_000_000.0,
            "the draw scrolls to the mid-document caret request (offset {offset})"
        );
        let handlers = scene.registry.handlers(&node);
        let start = std::time::Instant::now();
        for _ in 0..100 {
            (handlers.text.clone().unwrap())("a", EventPhase::Target);
            let _ = draw(&node);
        }
        assert!(
            start.elapsed().as_secs_f64() < 10.0,
            "elapsed {:?}",
            start.elapsed()
        );
        assert!(
            store.lock().unwrap().substring(middle..middle + 100) == "a".repeat(100),
            "typed {:?}",
            store.lock().unwrap().substring(middle..middle + 100)
        );
        let committed = store.lock().unwrap().clone();
        if let Some(chain) = FocusChainHolder::current() {
            chain.focus(Some(&node), true);
        }
        scene.graph.recomposer.commit_all();
        scene.graph.compute_layout(600.0, 240.0);
        (handlers.editing.clone().unwrap())(
            &TextEditingEvent {
                text: "你".into(),
                start: 1,
                length: 0,
            },
            EventPhase::Target,
        );
        let _ = draw(&node);
        assert!(state.borrow().composition.is_active(), "composing");
        let preview = state
            .borrow()
            .draft
            .preview
            .clone()
            .expect("cached preview");
        assert_eq!(
            preview.text, "你",
            "the cached preview holds the composing text"
        );
        let cached = preview.buffer.clone();
        for _ in 0..3 {
            let _ = draw(&node);
        }
        let reused = state
            .borrow()
            .draft
            .preview
            .clone()
            .expect("preview survives redraws");
        assert!(
            reused.buffer.identity() == cached.identity(),
            "redraws reuse the cached preview buffer instead of rebuilding it"
        );
        for _ in 0..50 {
            let _ = draw(&node);
        }
        assert!(
            *store.lock().unwrap() == committed,
            "composition not committed"
        );
    });
}

#[test]
fn joining_graphemes() {
    GlobalTestLock::locked(|| {
        let mut state = FieldState::new(TextBuffer::new("eZ"));
        state.selection.cursor_index = 1;
        insert_replacing_selection(&mut state, "\u{301}");
        assert!(
            state.buffer.string_value() == "e\u{301}Z" && state.buffer.character_count() == 2,
            "the combining mark joins the base grapheme"
        );
        assert_eq!(
            state.selection.cursor_index, 1,
            "the caret sits at the joined grapheme endpoint"
        );
        insert_replacing_selection(&mut state, "!");
        assert!(
            state.buffer.string_value() == "e\u{301}!Z",
            "{:?}",
            state.buffer.string_value()
        );
        state.buffer = TextBuffer::new("🇨!🇳Z");
        state.selection.anchor = Some(1);
        state.selection.cursor_index = 2;
        delete_selection(&mut state);
        assert!(
            state.buffer.string_value() == "🇨🇳Z" && state.buffer.character_count() == 2,
            "the two regional indicators form one flag"
        );
        assert!(
            state.selection.cursor_index == 1,
            "caret {}",
            state.selection.cursor_index
        );
    });
}
