//! Translation of `Tests/CoveUIComposeTests/LabelSelectionTests.swift`.
//!
//! Behaviour checklist:
//! - `TextSelectionChrome::handle_center` hangs the start handle below its line
//!   and the end handle above it, so a one-line selection keeps them apart
//! - `TextSelectionChrome::endpoint` returns the endpoint under a press within the
//!   slop, and none for a press in the middle of the selection
//! - a selection scope reports whether it has a selection, the text it covers, and
//!   whether a window point lands on that text
//! - selecting, extending and clearing drive the scope's selected text
//!
//! Named differences (production gaps, recorded rather than fixed): the Swift
//! suite's first ten cases drive the whole mounted tree — a pointer drag over a
//! `Text(...).textSelection()` label, Cmd+C through `ClipboardHolder`, a finger
//! long-press, a drag that spans two labels, a right-click selection menu in the
//! portal store, and the theme highlight painted into a `DrawList`. None of that is
//! reachable in Rust:
//! - there is no `textSelection()` view modifier, so no label ever joins a region
//!   and no label becomes hit-testable or focusable through one;
//! - `TextSelectionScope` is a model driven by explicit `select` / `extend` /
//!   `clear` calls, and nothing binds it to pointer or key events, so no drag,
//!   shift-arrow, long-press or right-click path connects input to it;
//! - there is no clipboard holder and no selection-menu presentation.
//! The chrome geometry and the scope's own model, which Rust does implement, are
//! ported at full strength; `the_model_is_not_driven_by_input` pins the gap.

mod support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::{AnyView, View};
use cove_core::scene::Node;
use cove_core::{Point, Rect};
use cove_ui_compose::primitives::text::rich::text_selection::{
    SharedTextSelectionScope, TextSelectionChrome, TextSelectionEndpoint, TextSelectionPosition,
    TextSelectionScope,
};
use cove_ui_compose::primitives::text::Text;

use support::GlobalTestLock;

#[test]
fn the_start_handle_hangs_below_and_the_end_handle_above() {
    let caret = Rect::new(10.0, 20.0, 0.0, 16.0);
    let start = TextSelectionChrome::handle_center(caret, true);
    let end = TextSelectionChrome::handle_center(caret, false);
    assert_eq!(
        start,
        Point::new(10.0, 36.0 + TextSelectionChrome::HANDLE_RADIUS),
        "the start handle sits below the line's bottom edge"
    );
    assert_eq!(
        end,
        Point::new(10.0, 20.0 - TextSelectionChrome::HANDLE_RADIUS),
        "the end handle sits above the line's top edge"
    );
    assert!(
        start.y > end.y,
        "a one-line selection keeps the two handles apart"
    );
}

#[test]
fn a_press_within_the_slop_grabs_an_endpoint() {
    let start = Rect::new(0.0, 0.0, 0.0, 16.0);
    let end = Rect::new(40.0, 0.0, 0.0, 16.0);
    let start_center = TextSelectionChrome::handle_center(start, true);
    let end_center = TextSelectionChrome::handle_center(end, false);

    assert_eq!(
        TextSelectionChrome::endpoint(start_center, Some(start), Some(end)),
        Some(TextSelectionEndpoint::Start),
        "a press on the start handle grabs the start"
    );
    assert_eq!(
        TextSelectionChrome::endpoint(end_center, Some(start), Some(end)),
        Some(TextSelectionEndpoint::End),
        "a press on the end handle grabs the end"
    );
    assert_eq!(
        TextSelectionChrome::endpoint(Point::new(20.0, 8.0), Some(start), Some(end)),
        None,
        "a press in the middle of the selection grabs neither endpoint"
    );
    assert_eq!(
        TextSelectionChrome::endpoint(start_center, None, None),
        None,
        "with no selection there is nothing to grab"
    );
}

fn position(node: &Node, cluster: usize) -> TextSelectionPosition {
    TextSelectionPosition {
        member: node.id().0,
        offset: cluster,
    }
}

/// Registers `label` as a selection member covering `text`, the way a
/// `textSelection()` region would enrol it.
fn register(scope: &SharedTextSelectionScope, label: &Node, text: &str) {
    let source = text.to_string();
    let owned = source.clone();
    TextSelectionScope::register(
        scope,
        label,
        Rc::new(move || owned.clone()),
        Rc::new(move |point: Point| {
            // A crude caret map: one cluster per point of x, clamped to the text.
            (point.x.max(0.0) as usize).min(source.chars().count())
        }),
    );
}

#[test]
fn a_scope_reports_its_selection_and_the_text_it_covers() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let graph = support::graph();
            graph.install_view(
                AnyView::new(Text::new("hello world")),
                Default::default(),
            );
            graph.compute_layout(320.0, 40.0);
            let root = graph.tree.borrow().root().expect("root");
            let label = first_static_text(&root).expect("the label is mounted");

            let scope: SharedTextSelectionScope =
                Rc::new(RefCell::new(TextSelectionScope::new()));
            register(&scope, &label, "hello world");
            assert!(
                !scope.borrow().has_selection(),
                "a fresh scope selects nothing"
            );

            scope
                .borrow_mut()
                .select(position(&label, 0), position(&label, 5));
            assert!(
                scope.borrow().has_selection(),
                "selecting a range gives the scope one"
            );
            assert_eq!(
                scope.borrow_mut().selected_text(),
                "hello",
                "the selection covers the dragged range"
            );

            scope.borrow_mut().extend(position(&label, 11));
            assert_eq!(
                scope.borrow_mut().selected_text(),
                "hello world",
                "extending the focus grows the selected text"
            );

            scope.borrow_mut().clear();
            assert!(
                !scope.borrow().has_selection(),
                "clearing drops the selection"
            );
            assert_eq!(
                scope.borrow_mut().selected_text(),
                "",
                "the cleared selection is empty"
            );
        });
    });
}

#[test]
fn a_point_lands_on_text_only_where_text_is_drawn() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let graph = support::graph();
            graph.install_view(AnyView::new(Text::new("hello world")), Default::default());
            graph.compute_layout(320.0, 160.0);
            let root = graph.tree.borrow().root().expect("root");
            let label = first_static_text(&root).expect("the label is mounted");
            let frame = label.absolute_frame();

            let scope: SharedTextSelectionScope =
                Rc::new(RefCell::new(TextSelectionScope::new()));
            register(&scope, &label, "hello world");
            assert!(
                scope.borrow_mut().text_under(Point::new(
                    frame.x + frame.width / 2.0,
                    frame.y + frame.height / 2.0
                )),
                "a point on the drawn text lands on it"
            );
            assert!(
                !scope.borrow_mut().text_under(Point::new(
                    frame.x + frame.width / 2.0,
                    frame.max_y() + 40.0
                )),
                "a point well below the text lands on nothing"
            );
        });
    });
}

fn first_static_text(node: &Node) -> Option<Node> {
    if node
        .accessibility()
        .is_some_and(|semantics| {
            semantics.role == cove_core::scene::AccessibilityRole::StaticText
        })
    {
        return Some(node.clone());
    }
    node.children().iter().find_map(first_static_text)
}

#[test]
fn the_model_is_not_driven_by_input() {
    // Gap: see the module note. The scope is a pure model; nothing connects it to
    // a label's pointer or key events, and there is no `textSelection()` modifier
    // to enrol a label in a region.
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let graph = support::graph();
            graph.install_view(AnyView::new(Text::new("bare")), Default::default());
            graph.compute_layout(260.0, 60.0);
            let root = graph.tree.borrow().root().expect("root");
            let label = first_static_text(&root).expect("the label is mounted");
            assert!(
                !label.is_focusable(),
                "a bare label is not focusable, because no region enrols it"
            );

            let scope: SharedTextSelectionScope =
                Rc::new(RefCell::new(TextSelectionScope::new()));
            register(&scope, &label, "bare");
            assert!(
                !scope.borrow().has_selection(),
                "laying out a label never selects anything on its own"
            );
        });
    });
}
