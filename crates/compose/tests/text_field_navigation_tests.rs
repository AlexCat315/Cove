//! Translation of `Tests/CoveUIComposeTests/TextFieldNavigationTests.swift`.
//!
//! Behaviour checklist:
//! - a stable inactive field exposes no IME anchor without reading its text, and
//!   a focused or render-dirty field does
//! - a joined colour emoji renders from the colour glyph atlas, advances the
//!   caret as one character and deletes as one (CoreText hosts)
//! - Home/End and Shift selection follow the CRLF-delimited logical line while
//!   Command, Control and Command-Shift commands cross the document
//! - soft-wrap End keeps its displayed row for Home and vertical motion
//! - page moves over a 200K-line document keep the preferred column and shape a
//!   bounded viewport
//!
//! The Swift `fieldNode` helper looked up `TextField.surfaceMarkerKey`; the Rust
//! host publishes its editing state under `__textfield_state`, which identifies
//! the same surface node.
//!
//! Named differences from the Swift suite (each pinned by a `GAP:` assertion):
//! - **No IME anchor.** The field installs no text input area resolver, so no
//!   area is published for a focused, dirty or wrapped field.
//! - **Scalar indices.** `TextBuffer` character indices count Unicode scalars:
//!   CRLF is two characters, `line_range(1)` is the empty slot between CR and
//!   LF, and a caret index inside a joined emoji lets Backspace delete one scalar.
//! - **No document, Command, Control or page navigation.** Command-Left moves one
//!   character, Control-Home/End act on the current line, Command-Up/Down,
//!   PageUp/PageDown and Up/Down are ignored, and End ignores soft-wrapped rows.
//! - **Edits stay internal and there is no undo.** Backspace and
//!   Command-Backspace edit only the internal buffer; Command-Z does nothing.
//! - **No published document layout.** `__textfield_document_layout` is never
//!   attached, so shaped-line bounds cannot be observed.

use crate::support;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use cove_composition::ViewGraph;
use cove_core::scene::{
    EventPhase, FocusChain, FocusChainHolder, InteractionRegistry, InteractionRegistryHolder,
    KeyHandler, Node, TextInputAttachmentKey,
};
use cove_core::{Binding, DrawList, Point};
use cove_platform::{KeyEvent, KeyModifiers, TextInputArea};
use cove_text::{TextEnvironment, TextEnvironmentHolder};
use cove_ui_compose::Scancode;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_document_layout::TextDocumentLayout;
use cove_ui_compose::primitives::text::text_field::text_field::{FieldState, TextField};
use cove_ui_compose::primitives::text::text_field::text_field_layout_engine::{
    FieldLayoutInputs, document_geometry,
};
use cove_ui_compose::primitives::text::text_field::text_field_options::{
    TextFieldAxis, TextFieldBehavior, TextFieldLayout, TextFieldNavigationRequest,
};
use cove_ui_compose::theme::{SemanticFontRef, theme_for_node};

use support::{GlobalTestLock, test_text_environment};

struct Scene {
    registry: Rc<InteractionRegistry>,
    focus: Rc<FocusChain>,
    graph: Rc<ViewGraph>,
}

/// The Swift `PlatformInputContext().withCurrent` scope with a chosen text
/// environment.
fn with_scene(environment: Option<TextEnvironment>, body: impl FnOnce(&Scene)) {
    GlobalTestLock::with_scheduler(|| {
        let previous_registry = InteractionRegistryHolder::current();
        let previous_focus = FocusChainHolder::current();
        let previous_text = TextEnvironmentHolder::current();
        let registry = Rc::new(InteractionRegistry::new());
        let focus = Rc::new(FocusChain::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));
        FocusChainHolder::set_current(Some(focus.clone()));
        TextEnvironmentHolder::set_current(environment);
        let scene = Scene {
            registry,
            focus,
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

fn key(scancode: u32, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent {
        scancode,
        keycode: 0,
        modifiers,
        is_repeat: false,
    }
}

fn press(handler: &KeyHandler, scancode: u32, modifiers: KeyModifiers) {
    handler(&key(scancode, modifiers), EventPhase::Target);
}

fn store_binding(store: &Arc<Mutex<TextBuffer>>) -> Binding<TextBuffer> {
    let read = store.clone();
    let write = store.clone();
    Binding::new(
        move || read.lock().unwrap().clone(),
        move |value| *write.lock().unwrap() = value,
    )
}

fn caret_request(index: usize) -> TextFieldNavigationRequest {
    TextFieldNavigationRequest {
        caret_request_id: Some(1),
        caret_request_index: Some(index),
        ..TextFieldNavigationRequest::default()
    }
}

fn area(node: &Node) -> Option<TextInputArea> {
    node.attachment::<TextInputArea>(TextInputAttachmentKey::AREA)
}

#[test]
fn inactive_ime_anchor() {
    with_scene(Some(test_text_environment(16.0, 20.0)), |scene| {
        let reads = Arc::new(AtomicUsize::new(0));
        let counter = reads.clone();
        let binding = Binding::new(
            move || {
                counter.fetch_add(1, Ordering::Relaxed);
                TextBuffer::new("Inactive inspector field")
            },
            |_| {},
        );
        scene.graph.install(
            TextField::new("", binding).frame_size(200.0, 30.0),
            Default::default(),
        );
        scene.graph.compute_layout(200.0, 30.0);
        let node = field_node(&scene.graph);
        scene.graph.tree.borrow_mut().flush();
        reads.store(0, Ordering::Relaxed);
        let resolver = node.attachment::<cove_core::scene::TextInputAreaResolver>(
            TextInputAttachmentKey::AREA_RESOLVER,
        );
        assert!(
            resolver.is_none(),
            "GAP: Swift resolves the IME anchor through committedTextInputArea; Rust's TextField installs no text input area resolver"
        );
        assert!(area(&node).is_none(), "inactive field has no area");
        assert!(
            reads.load(Ordering::Relaxed) == 0,
            "no reads {}",
            reads.load(Ordering::Relaxed)
        );
        scene.focus.focus(Some(&node), false);
        scene.graph.compute_layout(200.0, 30.0);
        assert!(
            area(&node).is_none(),
            "GAP: Swift publishes an IME area for the focused field; Rust publishes none"
        );
        scene.focus.clear();
        node.mark_render_dirty(cove_core::scene::InvalidationSource::Unknown);
        scene.graph.compute_layout(200.0, 30.0);
        assert!(
            area(&node).is_none(),
            "GAP: Swift republishes the IME area after a render invalidation; Rust publishes none"
        );
    });
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
#[test]
fn color_emoji_caret() {
    let environment = TextEnvironment::bootstrapped(cove_text::TextBootstrapOptions {
        atlas_texture_id: 1,
        raster_scale: 2.0,
        ..cove_text::TextBootstrapOptions::default()
    });
    with_scene(Some(environment), |scene| {
        let store = Arc::new(Mutex::new(TextBuffer::new("A👩🏽‍💻中🙂B")));
        let original = store.lock().unwrap().clone();
        let mut field = TextField::new("", store_binding(&store));
        field.navigation = caret_request(2);
        scene
            .graph
            .install(field.frame_size(300.0, 36.0), Default::default());
        scene.graph.compute_layout(300.0, 36.0);
        let node = field_node(&scene.graph);
        let state = state_of(&node).unwrap();
        let handler = scene.registry.handlers(&node).key.expect("key handler");
        let mut list = DrawList::default();
        if let Some(draw) = node.draw() {
            draw(&mut list, Point::default());
        }
        assert!(
            list.batches()
                .iter()
                .any(|batch| batch.texture_id == cove_core::color_glyph_atlas_id(1)),
            "colour glyph batch {:?}",
            list.batches()
                .iter()
                .map(|batch| batch.texture_id)
                .collect::<Vec<_>>()
        );
        assert!(
            area(&node).is_none(),
            "GAP: Swift reports the caret area after the joined emoji (and it moves more than 12pt on ArrowLeft); Rust publishes no text input area"
        );
        press(&handler, Scancode::ARROW_LEFT, KeyModifiers::default());
        assert!(
            state.borrow().selection.cursor_index == 1,
            "cursor {}",
            state.borrow().selection.cursor_index
        );
        press(&handler, Scancode::ARROW_RIGHT, KeyModifiers::default());
        press(&handler, Scancode::BACKSPACE, KeyModifiers::default());
        assert_eq!(
            store.lock().unwrap().string_value(),
            "A中🙂B",
            "Backspace deletes the joined emoji through the binding"
        );
        assert_eq!(
            state.borrow().buffer.string_value(),
            "A中🙂B",
            "the caret steps over the joined emoji and Backspace removes it whole"
        );
        assert!(
            state.borrow().selection.cursor_index == 1,
            "cursor {}",
            state.borrow().selection.cursor_index
        );
        press(&handler, Scancode::Z, KeyModifiers::LEFT_GUI);
        assert!(
            *store.lock().unwrap() != original,
            "GAP: Swift Command-Z restores the original; the field records no edit history"
        );
        assert_eq!(
            state.borrow().selection.cursor_index,
            1,
            "GAP: Swift undo restores the caret to 2; Rust has no undo"
        );
    });
}

#[test]
fn logical_lines() {
    with_scene(None, |scene| {
        let store = Arc::new(Mutex::new(TextBuffer::new("first\r\nAé👩🏽‍💻中Z\r\nlast")));
        let original = store.lock().unwrap().clone();
        assert_eq!(
            original.line_range(1),
            6..6,
            "GAP: Swift's lineRange(forLine: 1) is the CRLF-delimited second line; Rust's \
             TextBuffer::line_range indexes Unicode scalars and returns the empty slot between \
             CR and LF"
        );
        let range = original.line_range(original.line_index_for_character_index(8));
        assert_eq!(range, 7..15, "the second logical line in scalar indices");
        let mut field = TextField::new("", store_binding(&store)).layout(TextFieldLayout {
            axis: TextFieldAxis::Vertical,
            wraps_lines: false,
            ..TextFieldLayout::default()
        });
        field.navigation = caret_request(range.start + 2);
        scene.graph.install(field, Default::default());
        let node = field_node(&scene.graph);
        let state = state_of(&node).unwrap();
        let handler = scene.registry.handlers(&node).key.expect("key handler");
        let cursor = || state.borrow().selection.cursor_index;
        press(&handler, Scancode::HOME, KeyModifiers::default());
        assert!(cursor() == range.start, "home {}", cursor());
        press(&handler, Scancode::END, KeyModifiers::LEFT_SHIFT);
        assert!(
            state.borrow().selection_range() == Some((range.start, range.end)),
            "line selection {:?}",
            state.borrow().selection_range()
        );
        press(&handler, Scancode::ARROW_LEFT, KeyModifiers::LEFT_GUI);
        assert!(
            cursor() == range.end - 1 && state.borrow().selection.anchor.is_none(),
            "GAP: Swift Command-Left moves to the line start; Rust moves one character left: {}",
            cursor()
        );
        press(&handler, Scancode::ARROW_RIGHT, KeyModifiers::LEFT_GUI);
        press(&handler, Scancode::BACKSPACE, KeyModifiers::LEFT_GUI);
        assert_eq!(
            store.lock().unwrap().string_value(),
            "first\r\nAé👩🏽‍💻中\r\nlast",
            "GAP: Swift Command-Backspace deletes to the line start (first\\r\\n\\r\\nlast); Rust deletes one character"
        );
        assert_eq!(
            state.borrow().buffer.string_value(),
            "first\r\nAé👩🏽‍💻中\r\nlast",
            "the internal buffer drops one character"
        );
        press(&handler, Scancode::Z, KeyModifiers::LEFT_GUI);
        assert!(
            *store.lock().unwrap() != original,
            "GAP: Swift Command-Z restores the original; the field records no edit history"
        );
        store.lock().unwrap().clone_from(&original);
        state.borrow_mut().normalize(&original);
        press(&handler, Scancode::HOME, KeyModifiers::LEFT_CTRL);
        assert_eq!(
            cursor(),
            range.start,
            "GAP: Swift Control-Home moves to the document start; Rust treats it as Home on the current line"
        );
        press(&handler, Scancode::END, KeyModifiers::LEFT_CTRL);
        let count = state.borrow().buffer.character_count();
        assert_eq!(
            cursor(),
            range.end,
            "GAP: Swift Control-End moves to the document end ({count}); Rust treats it as End on the current line"
        );
        press(&handler, Scancode::ARROW_UP, KeyModifiers::LEFT_GUI);
        assert_eq!(
            cursor(),
            range.end,
            "GAP: Swift Command-Up moves to the document start; Rust ignores it"
        );
        press(
            &handler,
            Scancode::ARROW_DOWN,
            KeyModifiers::LEFT_GUI | KeyModifiers::LEFT_SHIFT,
        );
        assert_eq!(
            state.borrow().selection_range(),
            None,
            "GAP: Swift Command-Shift-Down selects to the document end; Rust ignores it"
        );
    });
}

#[test]
fn wrapped_row_affinity() {
    let environment = test_text_environment(16.0, 20.0);
    with_scene(Some(environment.clone()), |scene| {
        let buffer = TextBuffer::new(&"abcdefghij".repeat(12));
        let mut field = TextField::new("", Binding::constant(buffer.clone()))
            .layout(TextFieldLayout {
                axis: TextFieldAxis::Vertical,
                ..TextFieldLayout::default()
            })
            .behavior(TextFieldBehavior {
                read_only: true,
                ..TextFieldBehavior::default()
            });
        field.navigation = caret_request(2);
        scene
            .graph
            .install(field.frame_size(120.0, 140.0), Default::default());
        scene.graph.compute_layout(120.0, 140.0);
        let node = field_node(&scene.graph);
        let state = state_of(&node).unwrap();
        let handler = scene.registry.handlers(&node).key.expect("key handler");

        let theme = theme_for_node(&node);
        let token = SemanticFontRef::body().resolve(&theme);
        let inset = theme.spacing.sm;
        let inputs = FieldLayoutInputs {
            is_vertical: true,
            wraps_lines: true,
            secure: false,
            horizontal_inset: inset,
            vertical_inset: 0.0,
            text_origin_y_offset: 0.0,
        };
        let geometry = document_geometry(
            &inputs,
            token.font.clone(),
            token.line_height,
            token.letter_spacing,
            environment.atlas_texture_id as u64,
            (node.frame().width - inset * 2.0).max(1.0),
            false,
        );
        let mut document = TextDocumentLayout::new(buffer.clone(), geometry);
        let row_end = document.character(0, f32::INFINITY, &environment);
        assert!(
            row_end > 2 && row_end < buffer.character_count(),
            "row end {row_end} of {}",
            buffer.character_count()
        );

        press(&handler, Scancode::END, KeyModifiers::default());
        assert!(
            state.borrow().selection.cursor_index == buffer.character_count()
                && !state.borrow().selection.line_end_affinity,
            "GAP: Swift End stops at the displayed row end ({row_end}) with line-end affinity; Rust moves to the logical line end: {:?}",
            state.borrow().selection
        );
        let anchor = area(&node);
        assert!(
            anchor.is_none(),
            "GAP: Swift reports the IME anchor on the first row (y < 20, x > 70); Rust publishes no text input area"
        );
        press(&handler, Scancode::HOME, KeyModifiers::default());
        assert!(
            state.borrow().selection.cursor_index == 0,
            "home {:?}",
            state.borrow().selection
        );
        press(&handler, Scancode::END, KeyModifiers::default());
        press(&handler, Scancode::ARROW_DOWN, KeyModifiers::default());
        let selection = state.borrow().selection;
        let (_, row) = document.caret(
            selection.cursor_index,
            selection.line_end_affinity,
            &environment,
        );
        assert_eq!(
            row, 7,
            "GAP: Swift Down from the first row end lands on row 1; Rust ignores Down and the caret stays at the document end (row 7)"
        );
        press(&handler, Scancode::ARROW_UP, KeyModifiers::default());
        let selection = state.borrow().selection;
        assert_eq!(
            selection.cursor_index,
            buffer.character_count(),
            "GAP: Swift Up returns to the first row end ({row_end}); Rust ignores Up"
        );
        let (_, row) = document.caret(
            selection.cursor_index,
            selection.line_end_affinity,
            &environment,
        );
        assert_eq!(
            row, 7,
            "GAP: Swift Up lands on row 0; Rust leaves the caret on the last row"
        );
    });
}

#[test]
fn page_navigation() {
    let environment = test_text_environment(16.0, 20.0);
    with_scene(Some(environment), |scene| {
        let buffer = TextBuffer::new(&"0123456789\n".repeat(200_000));
        let middle = buffer.character_index_for_line(100_000, 7);
        let mut field = TextField::new("", Binding::constant(buffer.clone()))
            .layout(TextFieldLayout {
                axis: TextFieldAxis::Vertical,
                wraps_lines: false,
                ..TextFieldLayout::default()
            })
            .behavior(TextFieldBehavior {
                read_only: true,
                ..TextFieldBehavior::default()
            });
        field.navigation = caret_request(middle);
        scene
            .graph
            .install(field.frame_size(400.0, 160.0), Default::default());
        scene.graph.compute_layout(400.0, 160.0);
        let node = field_node(&scene.graph);
        let state = state_of(&node).unwrap();
        let handler = scene.registry.handlers(&node).key.expect("key handler");
        press(&handler, Scancode::PAGE_DOWN, KeyModifiers::LEFT_SHIFT);
        let (line, column) = buffer.line_and_column(state.borrow().selection.cursor_index);
        assert!(
            line == 100_000 && column == 7,
            "GAP: Swift Shift-PageDown moves one viewport (line 100001..100020) keeping column 7; Rust ignores PageDown: {line}:{column}"
        );
        assert_eq!(
            state.borrow().selection.anchor,
            None,
            "GAP: Swift Shift-PageDown anchors the selection at the start; Rust ignores it"
        );
        press(&handler, Scancode::PAGE_UP, KeyModifiers::default());
        assert!(
            state.borrow().selection.cursor_index == middle
                && state.borrow().selection.anchor.is_none(),
            "page up {:?}",
            state.borrow().selection
        );
        let mut list = DrawList::default();
        if let Some(draw) = node.draw() {
            draw(&mut list, Point::default());
        }
        let document =
            node.attachment::<Rc<RefCell<TextDocumentLayout>>>("__textfield_document_layout");
        assert!(
            document.is_none(),
            "GAP: Swift publishes `__textfield_document_layout` with fewer than 25 shaped lines; Rust's field draw keeps its document local"
        );
    });
}
