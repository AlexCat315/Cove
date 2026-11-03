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
//! Command/word/page navigation, soft-wrap row affinity and undo run through the
//! shared history and row geometry, as in Swift.

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
        // A second commit settles the geometry the resolver records.
        scene.graph.compute_layout(200.0, 30.0);
        let node = field_node(&scene.graph);
        let mut list = DrawList::default();
        if let Some(draw) = node.draw() {
            draw(&mut list, Point::default());
        }
        // The commit after the paint drops the anchor of an inactive field:
        // flushing clears the render flag, so the resolver declines.
        scene.graph.tree.borrow_mut().flush();
        scene.graph.compute_layout(200.0, 30.0);
        reads.store(0, Ordering::Relaxed);
        let resolver = node.attachment::<cove_core::scene::TextInputAreaResolver>(
            TextInputAttachmentKey::AREA_RESOLVER,
        );
        assert!(
            resolver.is_some(),
            "the field installs a commit-phase IME resolver"
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
            area(&node).is_some(),
            "focused fields publish an IME area without needing a draw"
        );
        scene.focus.clear();
        node.mark_render_dirty(cove_core::scene::InvalidationSource::Unknown);
        scene.graph.compute_layout(200.0, 30.0);
        assert!(
            area(&node).is_some(),
            "an invalidated field republishes its IME area"
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
        let before = area(&node).expect("the caret request publishes an IME area");
        assert!(
            before.cursor_x > 0.0,
            "the anchor sits after the joined emoji ({})",
            before.cursor_x
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
        assert_eq!(
            *store.lock().unwrap(),
            original,
            "Command-Z restores the deleted emoji"
        );
        assert_eq!(
            state.borrow().selection.cursor_index,
            2,
            "undo restores the caret to where the deletion started"
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
            6..11,
            "Swift's lineRange(forLine: 1) is the CRLF-delimited second line; the \
             rope counts extended grapheme clusters, so the CRLF occupies one slot"
        );
        let range = original.line_range(original.line_index_for_character_index(8));
        assert_eq!(range, 6..11, "the second logical line in character indices");
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
            cursor() == range.start && state.borrow().selection.anchor.is_none(),
            "Command-Left moves to the line start: {}",
            cursor()
        );
        press(&handler, Scancode::ARROW_RIGHT, KeyModifiers::LEFT_GUI);
        press(&handler, Scancode::BACKSPACE, KeyModifiers::LEFT_GUI);
        assert_eq!(
            store.lock().unwrap().string_value(),
            "first\r\n\r\nlast",
            "Command-Backspace deletes to the line start"
        );
        assert_eq!(
            state.borrow().buffer.string_value(),
            "first\r\n\r\nlast",
            "the same edit reaches the internal buffer"
        );
        press(&handler, Scancode::Z, KeyModifiers::LEFT_GUI);
        assert_eq!(
            *store.lock().unwrap(),
            original,
            "Command-Z restores the deleted line"
        );
        store.lock().unwrap().clone_from(&original);
        state.borrow_mut().normalize(&original);
        press(&handler, Scancode::HOME, KeyModifiers::LEFT_CTRL);
        assert_eq!(cursor(), 0, "Control-Home moves to the document start");
        press(&handler, Scancode::END, KeyModifiers::LEFT_CTRL);
        let count = state.borrow().buffer.character_count();
        assert_eq!(cursor(), count, "Control-End moves to the document end");
        press(&handler, Scancode::ARROW_UP, KeyModifiers::LEFT_GUI);
        assert_eq!(cursor(), 0, "Command-Up moves to the document start");
        press(
            &handler,
            Scancode::ARROW_DOWN,
            KeyModifiers::LEFT_GUI | KeyModifiers::LEFT_SHIFT,
        );
        assert_eq!(
            state.borrow().selection_range(),
            Some((0, count)),
            "Command-Shift-Down selects to the document end"
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
            state.borrow().selection.cursor_index == row_end
                && state.borrow().selection.line_end_affinity,
            "End stops at the displayed row end ({row_end}) with line-end affinity: {:?}",
            state.borrow().selection
        );
        let anchor = area(&node);
        assert!(
            anchor.is_none(),
            "a read-only field supports navigation without activating the native IME"
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
            row, 1,
            "Down from the first row end lands on the next displayed row"
        );
        press(&handler, Scancode::ARROW_UP, KeyModifiers::default());
        let selection = state.borrow().selection;
        assert_eq!(
            selection.cursor_index, row_end,
            "Up returns to the first row end ({row_end})"
        );
        let (_, row) = document.caret(
            selection.cursor_index,
            selection.line_end_affinity,
            &environment,
        );
        assert_eq!(row, 0, "Up lands on the first displayed row");
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
        // 160pt frame, 20pt rows, 6pt insets: seven visible rows, one kept as
        // context, so a page step is six rows.
        assert!(
            line == 100_006 && column == 7,
            "Shift-PageDown moves one viewport keeping the column: {line}:{column}"
        );
        assert_eq!(
            state.borrow().selection.anchor,
            Some(middle),
            "Shift-PageDown anchors the selection where it started"
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
            document.is_some(),
            "the draw publishes the shaped document layout"
        );
    });
}
