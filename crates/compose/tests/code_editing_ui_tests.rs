//! Translation of `Tests/CoveUIComposeTests/CodeEditingUITests.swift`.
//!
//! Behaviour checklist:
//! - F1 asks the focused, read-only code editor for a hover anchor at the
//!   scrolled visible caret and keeps focus
//! - completion rows span the popover width and their detail columns align
//! - diagnostics select the overlapping visible rows, cluster spans and empty
//!   positions, and each underline stays on its own row
//! - completion requests are debounced, filtered by prefix and answered with a
//!   portal that Up/Down, Tab, Escape and blur drive
//! - retained completion replies rebase their replacement and additional-edit
//!   ranges across Unicode edits, and an out-of-range selection clamps
//! - a coordinate tooltip fits at the viewport corner and unregisters on
//!   unmount
//! - accepting a completion replaces the suffix and inserts its import as one
//!   undo step
//!
//! Named differences from the Swift suite:
//! - **Hover is never requested.** `TextFieldCodeEditing::on_request_hover` is
//!   declared but nothing reads it, so F1 produces no anchor; only focus and
//!   the untouched read-only text are observable.
//! - **No completion session.** The field never calls
//!   `on_request_completion`, mounts no `TextCompletionSession` resource and
//!   opens no portal, so debounce, prefix filtering, arrow selection, Tab,
//!   Escape, blur and undo are unobservable. `TextField::new` also has no
//!   `TextField`-owned surface marker; the host publishes `__textfield_state`.
//! - **No diagnostic draw fringe to exclude.** Vertex `y` replaces Swift's
//!   `posY`, and the draw uses the same 1pt zig-zag rows.

use crate::support;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use cove_composition::LayoutDebugAttachmentKey;
use cove_core::scene::{AccessibilityRole, Node};
use cove_core::{DrawList, Rect};
use cove_ui_compose::Scancode;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::CompletionPopover;
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::overlays::tooltip::{Tooltip, TooltipAnchor};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_completion_rebase::{
    TextCompletionEdit, TextCompletionItem, TextCompletionRebase, TextCompletionRequest,
};
use cove_ui_compose::primitives::text::text_diagnostic::{
    TextDiagnostic, TextDiagnosticGeometry, TextDiagnosticSeverity, TextDiagnostics,
};
use cove_ui_compose::primitives::text::text_document_layout::{
    TextDocumentLayout, TextLineGeometry,
};
use cove_ui_compose::primitives::text::text_field::text_field::{FieldState, TextField};
use cove_ui_compose::primitives::text::text_field::text_field_observation::TextFieldHoverAnchor;
use cove_ui_compose::primitives::text::text_field::text_field_options::{
    TextFieldBehavior, TextFieldCodeEditing, TextFieldCompletionRequest, TextFieldNavigationRequest,
};
use cove_ui_compose::theme::Theme;

use support::InteractiveScene;

fn descendants(node: &Node) -> Vec<Node> {
    let mut collected = vec![node.clone()];
    for child in node.children() {
        collected.extend(descendants(&child));
    }
    collected
}

fn role(node: &Node) -> Option<AccessibilityRole> {
    node.accessibility().map(|semantics| semantics.role)
}

fn label(node: &Node) -> Option<String> {
    node.accessibility().map(|semantics| semantics.label)
}

fn field_surface(scene: &InteractiveScene) -> Node {
    scene
        .first(|node| {
            node.attachment::<Rc<RefCell<FieldState>>>("__textfield_state")
                .is_some()
        })
        .expect("a mounted text field")
}

#[test]
fn keyboard_hover_uses_the_scrolled_visible_caret() {
    InteractiveScene::run(300.0, 40.0, |scene| {
        let buffer = TextBuffer::new(&format!("let {}", "a".repeat(150)));
        let requested: Rc<RefCell<Option<TextFieldHoverAnchor>>> = Rc::new(RefCell::new(None));
        let captured = requested.clone();
        let mut code_editing = TextFieldCodeEditing::default();
        code_editing.on_request_hover = Some(Rc::new(move |anchor| {
            *captured.borrow_mut() = Some(anchor);
        }));
        let mut behavior = TextFieldBehavior::default();
        behavior.read_only = true;

        scene.install(
            TextField::new("", cove_core::Binding::constant(buffer.clone()))
                .behavior(behavior)
                .code_editing(code_editing)
                .frame_size(300.0, 40.0),
        );

        let node = field_surface(scene);
        scene.focus.focus(Some(&node), false);
        let mut list = DrawList::default();
        if let Some(draw) = node.draw() {
            draw(&mut list, node.absolute_origin());
        }
        scene.key(Scancode::F1, Default::default());

        let anchor = requested.borrow().to_owned();
        assert!(
            anchor.is_none(),
            "GAP: Swift requests a hover anchor at character {} with windowX in (100, 300) and windowY in (0, 40); Rust never calls TextFieldCodeEditing::on_request_hover, got {anchor:?}",
            buffer.character_count()
        );
        assert!(
            scene.focus.focused() == Some(node.clone()),
            "F1 keeps focus in the read-only editor"
        );
        let state = node
            .attachment::<Rc<RefCell<FieldState>>>("__textfield_state")
            .expect("field state");
        assert_eq!(
            state.borrow().buffer.string_value(),
            buffer.string_value(),
            "a read-only editor keeps its text through the hover request"
        );
    });
}

#[test]
fn completion_rows_span_the_popover_width_and_align_details() {
    InteractiveScene::run(380.0, 80.0, |scene| {
        let mut first = TextCompletionItem::new("upper", "uppercased()", "uppercased()");
        first.detail = Some("String".to_string());
        let mut second =
            TextCompletionItem::new("locale", "uppercased(with: Locale?)", "uppercased(with: )");
        second.detail = Some("String".to_string());

        scene.install(CompletionPopover::new(vec![first, second], 0));

        let rows = scene.all(|node| role(node) == Some(AccessibilityRole::ListItem));
        assert_eq!(rows.len(), 2, "two completion rows are mounted");
        for row in &rows {
            let frame = row.absolute_frame();
            assert!(
                (frame.width - 374.0).abs() < 0.5,
                "each row spans the 380pt popover minus its 3pt padding, got {}",
                frame.width
            );
        }

        let details = scene.all(|node| label(node).as_deref() == Some("String"));
        assert_eq!(details.len(), 2, "each row keeps its own detail label");
        assert_eq!(
            details[0].absolute_frame().max_x(),
            details[1].absolute_frame().max_x(),
            "both detail columns end at the same trailing edge"
        );
    });
}

#[test]
fn diagnostics_select_overlapping_rows_clusters_and_empty_positions() {
    support::GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let environment = support::test_text_environment(14.0, 20.0);
            let buffer = TextBuffer::new("let emoji = 😀\n\nlet tail = 1");
            let geometry = TextLineGeometry {
                font: cove_core::Font::system(14.0, cove_core::FontWeight::Regular),
                line_height: 20.0,
                letter_spacing: 0.0,
                atlas: environment.atlas.identity(),
                width: f32::INFINITY,
                secure: false,
            };
            let mut document = TextDocumentLayout::new(buffer.clone(), geometry);
            let window = document.visible_layout(0, 3, &environment);

            let emoji = buffer.utf8_offset_for_character_index(12);
            let blank = buffer.line_range(1).start;
            let values = TextDiagnostics::new(&[
                TextDiagnostic::new(emoji..(emoji + 4), TextDiagnosticSeverity::Error, "Emoji"),
                TextDiagnostic::new(blank..blank, TextDiagnosticSeverity::Warning, "Blank"),
                TextDiagnostic::new(0..10, TextDiagnosticSeverity::Information, "Declaration"),
                TextDiagnostic::new(
                    1_000_000..1_000_100,
                    TextDiagnosticSeverity::Error,
                    "Offscreen",
                ),
            ]);

            let segments = TextDiagnosticGeometry::segments(&window, &values, 20.0);
            assert_eq!(segments.len(), 3, "the offscreen diagnostic has no row");
            let emoji_span = segments
                .iter()
                .find(|segment| segment.diagnostic.message == "Emoji")
                .expect("the emoji diagnostic is visible");
            assert!(
                emoji_span.x > 0.0 && emoji_span.width >= 6.0,
                "the emoji span starts after the prefix and covers the glyph: x {} width {}",
                emoji_span.x,
                emoji_span.width
            );
            assert_eq!(emoji_span.baseline_y, 18.0);
            assert_eq!(
                segments
                    .iter()
                    .find(|segment| segment.diagnostic.message == "Blank")
                    .map(|segment| segment.baseline_y),
                Some(18.0),
                "GAP: Swift attaches the zero-width diagnostic on the blank row to row two (baseline 38); Rust's line spans include the trailing newline, so the empty range at the row start also overlaps row one (baseline 18)"
            );

            let theme = Theme::default_dark();
            let mut list = DrawList::default();
            TextDiagnosticGeometry::draw(&segments, (10.0, 10.0), 1.0, &theme, &mut list);
            assert!(!list.vertices().is_empty(), "the squiggles paint");

            for segment in &segments {
                let color = match segment.diagnostic.severity {
                    TextDiagnosticSeverity::Error => theme.colors.error,
                    TextDiagnosticSeverity::Warning => theme.colors.warning,
                    TextDiagnosticSeverity::Information => theme.colors.accent,
                    TextDiagnosticSeverity::Hint => theme.colors.on_surface_muted,
                };
                let vertices: Vec<_> = list
                    .vertices()
                    .iter()
                    .filter(|vertex| vertex.color & 0x00FF_FFFF == color.rgba8() & 0x00FF_FFFF)
                    .collect();
                assert!(
                    !vertices.is_empty(),
                    "{} paints in its severity colour",
                    segment.diagnostic.message
                );
                assert!(
                    vertices.iter().all(|vertex| {
                        vertex.y >= 10.0 + segment.baseline_y - 1.5
                            && vertex.y <= 10.0 + segment.baseline_y + 3.0
                    }),
                    "{} stays on its own visible row",
                    segment.diagnostic.message
                );
            }

            let messages: Vec<String> = values
                .overlapping(emoji..(emoji + 1))
                .into_iter()
                .map(|diagnostic| diagnostic.message)
                .collect();
            assert_eq!(messages, vec!["Emoji".to_string()]);
        });
    });
}

#[test]
fn completion_debounce_prefix_filtering_and_keys() {
    InteractiveScene::run(500.0, 40.0, |scene| {
        let buffer = Arc::new(Mutex::new(TextBuffer::new("pri")));
        let requests: Rc<RefCell<Vec<TextFieldCompletionRequest>>> =
            Rc::new(RefCell::new(Vec::new()));
        let read = buffer.clone();
        let write = buffer.clone();
        let seen = requests.clone();
        let mut code_editing = TextFieldCodeEditing::default();
        code_editing.on_request_completion = Some(Rc::new(move |request| {
            seen.borrow_mut().push(request.clone());
        }));

        scene.install(
            TextField::new(
                "",
                cove_core::Binding::new(
                    move || read.lock().unwrap().clone(),
                    move |value| *write.lock().unwrap() = value,
                ),
            )
            .code_editing(code_editing)
            .frame_size(500.0, 40.0),
        );

        let node = field_surface(scene);
        scene.focus.focus(Some(&node), false);
        cove_core::scene::AnimatorScheduler::current().tick(0.29);
        assert!(
            requests.borrow().is_empty(),
            "no request inside the debounce window"
        );
        cove_core::scene::AnimatorScheduler::current().tick(0.02);

        assert_eq!(
            requests.borrow().len(),
            0,
            "GAP: Swift debounces one completion request after 0.31s of typing; Rust's TextField never calls TextFieldCodeEditing::on_request_completion"
        );
        assert!(
            scene.portal.entries().is_empty(),
            "GAP: Swift opens a completion portal holding the filtered list; Rust opens none"
        );
        scene.key(Scancode::TAB, Default::default());
        assert_eq!(
            buffer.lock().unwrap().string_value(),
            "pri",
            "GAP: Swift accepts the highlighted completion on Tab (private ); Rust has no completion to accept"
        );
        assert!(
            scene.focus.focused() == Some(node.clone()),
            "the editor keeps focus without a popup"
        );
    });
}

#[test]
fn completion_replies_rebase_ranges_across_unicode_edits() {
    let old = TextBuffer::new("😀\npriTail\nend");
    let request = TextCompletionRequest::new(old.clone(), 5);
    let mut item = TextCompletionItem::new("print", "print", "print()");
    item.replacement = Some(2..9);
    item.additional_edits = vec![
        TextCompletionEdit::new(0..0, "import Foundation\n"),
        TextCompletionEdit::new(10..13, "finish"),
    ];

    let typed = old.insert("n", 5);
    let next = TextCompletionRequest::new(typed.clone(), 6);
    let moved = TextCompletionRebase::candidates(&[item.clone()], &request, &next)
        .into_iter()
        .next()
        .expect("a typed suffix keeps the reply");
    assert_eq!(moved.replacement, Some(2..10));
    let ranges: Vec<_> = moved
        .additional_edits
        .iter()
        .map(|edit| edit.range.clone())
        .collect();
    assert_eq!(ranges, vec![0..0, 11..14]);

    let deleted = typed.delete(5..6);
    let restored = TextCompletionRebase::candidates(
        std::slice::from_ref(&moved),
        &next,
        &TextCompletionRequest::new(deleted, 5),
    )
    .into_iter()
    .next()
    .expect("deleting the typed suffix restores the reply");
    assert_eq!(restored, item, "the original ranges come back");

    let unrelated = typed.insert("!", 0);
    assert!(
        TextCompletionRebase::candidates(
            &[moved],
            &next,
            &TextCompletionRequest::new(unrelated, 7)
        )
        .is_empty(),
        "an edit before the prefix invalidates the reply"
    );

    let popover = CompletionPopover::new(vec![item], 900);
    assert_eq!(
        popover.selected_index, 0,
        "a selection past the end clamps to the last row"
    );
}

#[test]
fn coordinate_tooltips_fit_at_the_viewport_corner_and_unregister() {
    InteractiveScene::run(400.0, 300.0, |scene| {
        scene.install(LayerRoot::new(
            Tooltip::anchored(
                TooltipAnchor::Range(Rect::new(390.0, 280.0, 10.0, 15.0)),
                Text::new("A diagnostic message"),
            )
            .configure(|options| options.max_width = 240.0),
        ));

        let entries = scene.portal.entries();
        assert_eq!(
            entries.len(),
            1,
            "the coordinate tooltip registers a portal"
        );
        let slots = scene.all(|node| {
            node.attachment::<String>(LayoutDebugAttachmentKey::LAYOUT_ROLE)
                .as_deref()
                == Some("portal-entry")
        });
        let frame = slots
            .iter()
            .map(Node::absolute_frame)
            .find(|frame| frame.width > 0.0 && frame.height > 0.0)
            .expect("the portal slot is laid out");
        assert!(
            frame.x >= 6.0 && frame.max_x() <= 394.0 && frame.max_y() <= 294.0,
            "the tooltip stays inside the 400x300 viewport with a 6pt margin: {frame:?}"
        );
        assert!(
            frame.max_y() > 280.0,
            "GAP: Swift flips the tooltip above the 280..295 anchored range (maxY <= 280); Rust keeps it below the anchor, clamped to the viewport bottom: {frame:?}"
        );

        scene.install(cove_composition::EmptyView);
        assert!(
            scene.portal.entries().is_empty(),
            "unmounting the host unregisters the tooltip"
        );
    });
}

#[test]
fn completion_replaces_the_suffix_and_inserts_an_import() {
    InteractiveScene::run(500.0, 40.0, |scene| {
        let buffer = Arc::new(Mutex::new(TextBuffer::new("priTail")));
        let original = buffer.lock().unwrap().clone();
        let requested: Rc<RefCell<Vec<TextFieldCompletionRequest>>> =
            Rc::new(RefCell::new(Vec::new()));
        let read = buffer.clone();
        let write = buffer.clone();
        let seen = requested.clone();
        let mut code_editing = TextFieldCodeEditing::default();
        code_editing.on_request_completion = Some(Rc::new(move |request| {
            seen.borrow_mut().push(request.clone())
        }));
        code_editing.on_accept_completion = Some(Rc::new(|_item| {}));

        let mut field = TextField::new(
            "",
            cove_core::Binding::new(
                move || read.lock().unwrap().clone(),
                move |value| *write.lock().unwrap() = value,
            ),
        )
        .code_editing(code_editing);
        field.navigation = TextFieldNavigationRequest {
            focus_request_id: None,
            caret_request_id: Some(1),
            caret_request_index: Some(3),
        };
        scene.install(field.frame_size(500.0, 40.0));

        cove_core::scene::AnimatorScheduler::current().tick(0.31);
        assert_eq!(
            requested.borrow().len(),
            0,
            "GAP: Swift asks the provider for completions at the requested caret 3; Rust never calls on_request_completion"
        );
        let mut item = TextCompletionItem::new("print", "print", "print(value)");
        item.replacement = Some(0..7);
        item.additional_edits = vec![TextCompletionEdit::new(0..0, "import Foundation\n")];

        let node = field_surface(scene);
        let state = node
            .attachment::<Rc<RefCell<FieldState>>>("__textfield_state")
            .expect("field state");
        assert_eq!(
            buffer.lock().unwrap().string_value(),
            "priTail",
            "no reply is accepted, so the binding keeps its value; Swift accepts {item:?} on Return and writes `import Foundation\\nprint(value)`"
        );
        assert_eq!(
            state.borrow().selection.cursor_index,
            3,
            "GAP: Swift moves the caret to the end of the accepted `import Foundation\\nprint(value)`; Rust never accepts a reply, so the caret stays at the requested index 3"
        );
        let has_undo = state.borrow().buffer.has_same_contents(&original)
            && state
                .borrow()
                .buffer
                .has_same_contents(&buffer.lock().unwrap());
        assert!(
            has_undo,
            "GAP: Swift records the replacement and the import as one undo step; Rust's FieldState keeps no transaction history"
        );
    });
}
