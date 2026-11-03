//! Translation of `Tests/CoveUIComposeTests/TextSelectionScopeTests.swift`.
//!
//! Behaviour checklist:
//! - dragging repaints only the changed paragraphs and ignores repeated endpoints
//! - cached hit frames follow ancestor scrolling, movement and removal
//! - detached subtrees and reused nodes leave their previous selection scope
//! - reflow and insertion update cached ranges and retained highlights
//! - reverse selections preserve UTF-8 and table reading order
//! - large pages keep selection lookup and endpoint updates responsive
//!
//! Model differences:
//! - Swift stores the endpoint as `TextSelectionScope.Position(member:
//!   ObjectIdentifier(node), offset:)`; Rust uses `TextSelectionPosition {
//!   member: u64, offset: usize }` keyed by `NodeId`. Positions compare the same
//!   way.
//! - `TextSelectionScope::register` takes a `&SharedTextSelectionScope` (an
//!   `Rc<RefCell<..>>`) plus `Rc<dyn Fn>` closures, and `range_for`/
//!   `selected_text` take `&mut self`. The harness holds the `Rc` and borrows it
//!   per call, which is the same ownership the Swift class had.
//! - `NodeTree::has_render_updates()` / `NodeTree::flush()` are the direct Rust
//!   counterparts of the Swift pair, so the render-pass boundary is modelled
//!   exactly: `flush` walks from the root and clears every reachable flag.

use crate::support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_core::Rect;
use cove_core::scene::{Node, NodeTree};
use cove_ui_compose::primitives::text::rich::text_selection::{
    SharedTextSelectionScope, TextSelectionPosition, TextSelectionScope,
};

use support::GlobalTestLock;

fn new_scope() -> SharedTextSelectionScope {
    Rc::new(RefCell::new(TextSelectionScope::new()))
}

fn position(node: &Node, offset: usize) -> TextSelectionPosition {
    TextSelectionPosition {
        member: node.id().0,
        offset,
    }
}

#[test]
fn keyboard_selection_preserves_joined_emoji_and_combining_characters() {
    GlobalTestLock::locked(|| {
        let scope = new_scope();
        let root = Node::new();
        let node = paragraph(
            &scope,
            "A👩🏽‍💻e\u{301}B",
            Rect::new(0.0, 0.0, 200.0, 20.0),
            &root,
        );
        scope
            .borrow_mut()
            .select(position(&node, 1), position(&node, 1));
        assert!(scope.borrow_mut().extend_focus(1));
        assert_eq!(scope.borrow_mut().selected_text(), "👩🏽‍💻");
        assert!(scope.borrow_mut().extend_focus(1));
        assert_eq!(scope.borrow_mut().selected_text(), "👩🏽‍💻e\u{301}");
        assert!(scope.borrow_mut().extend_focus(-1));
        assert_eq!(scope.borrow_mut().selected_text(), "👩🏽‍💻");
    });
}

/// Registers one paragraph that reports `text` and a clamped caret offset.
fn paragraph(
    scope: &SharedTextSelectionScope,
    text: &'static str,
    at: Rect,
    parent: &Node,
) -> Node {
    let node = Node::new();
    node.set_frame(at);
    parent.add_child(&node);
    TextSelectionScope::register(
        scope,
        &node,
        Rc::new(move || text.to_string()),
        Rc::new(|_point: cove_core::Point| 2.min(text.len())),
    );
    node
}

/// Clears every dirty bit from the root, the Swift `tree.flush()`.
fn flush(tree: &mut NodeTree) {
    tree.flush();
}

fn dirty_indices(nodes: &[Node]) -> Vec<usize> {
    nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.render_dirty())
        .map(|(index, _)| index)
        .collect()
}

#[test]
fn dragging_repaints_only_changed_paragraphs() {
    GlobalTestLock::locked(|| {
        let scope = new_scope();
        let root = Node::new();
        let mut tree = NodeTree::new();
        tree.set_root(Some(root.clone()));
        let nodes: Vec<Node> = (0..8)
            .map(|index| {
                paragraph(
                    &scope,
                    "abcdef",
                    Rect::new(0.0, index as f32 * 24.0, 100.0, 20.0),
                    &root,
                )
            })
            .collect();
        flush(&mut tree);

        scope
            .borrow_mut()
            .select(position(&nodes[1], 1), position(&nodes[1], 1));
        assert!(
            dirty_indices(&nodes).is_empty(),
            "a collapsed selection paints nothing"
        );

        scope.borrow_mut().extend(position(&nodes[3], 2));
        assert_eq!(
            dirty_indices(&nodes),
            vec![1, 2, 3],
            "extending across paragraphs repaints only the crossed ones"
        );
        flush(&mut tree);

        scope.borrow_mut().extend(position(&nodes[3], 4));
        assert_eq!(
            dirty_indices(&nodes),
            vec![3],
            "growing the endpoint inside one paragraph repaints only that one"
        );
        flush(&mut tree);

        scope.borrow_mut().extend(position(&nodes[3], 4));
        assert!(
            dirty_indices(&nodes).is_empty(),
            "repeating the same endpoint repaints nothing"
        );

        scope.borrow_mut().extend(position(&nodes[5], 2));
        assert_eq!(
            dirty_indices(&nodes),
            vec![3, 4, 5],
            "extending forward repaints the newly crossed paragraphs"
        );
        flush(&mut tree);

        scope.borrow_mut().extend(position(&nodes[0], 3));
        assert_eq!(
            dirty_indices(&nodes),
            vec![0, 1, 2, 3, 4, 5],
            "reversing across the anchor repaints the whole previous span plus the new one"
        );

        let mut scope_mut = scope.borrow_mut();
        assert_eq!(
            scope_mut.range_for(&nodes[0]),
            Some(3..6),
            "the first paragraph keeps its trailing run"
        );
        assert_eq!(
            scope_mut.range_for(&nodes[1]),
            Some(0..1),
            "the second paragraph keeps only its leading byte"
        );
        assert_eq!(
            scope_mut.range_for(&nodes[2]),
            None,
            "a fully covered middle paragraph reports no per-node range"
        );
        drop(scope_mut);
        flush(&mut tree);

        scope.borrow_mut().clear();
        assert_eq!(
            dirty_indices(&nodes),
            vec![0, 1],
            "clearing repaints only the endpoints"
        );
        assert!(
            scope.borrow_mut().selected_text().is_empty(),
            "clearing drops the selected text"
        );
    });
}

#[test]
fn cached_hit_frames_follow_scrolling_movement_and_removal() {
    GlobalTestLock::locked(|| {
        let scope = new_scope();
        let root = Node::new();
        let container = Node::new();
        root.add_child(&container);

        let first = paragraph(
            &scope,
            "first",
            Rect::new(10.0, 100.0, 80.0, 20.0),
            &container,
        );
        let second = paragraph(
            &scope,
            "second",
            Rect::new(10.0, 140.0, 80.0, 20.0),
            &container,
        );

        assert_eq!(
            scope
                .borrow_mut()
                .position(cove_core::Point::new(20.0, 110.0)),
            Some(position(&first, 2)),
            "a point inside the first paragraph resolves to its caret"
        );
        assert!(
            scope
                .borrow_mut()
                .text_under(cove_core::Point::new(20.0, 110.0)),
            "the first paragraph answers hit tests"
        );

        container.set_content_offset(cove_core::Point::new(0.0, 100.0));
        assert!(
            !scope
                .borrow_mut()
                .text_under(cove_core::Point::new(20.0, 110.0)),
            "scrolling the ancestor moves the paragraph out from under the point"
        );
        assert_eq!(
            scope
                .borrow_mut()
                .position(cove_core::Point::new(20.0, 10.0)),
            Some(position(&first, 2)),
            "the scrolled position resolves to the same paragraph"
        );
        assert_eq!(
            scope
                .borrow_mut()
                .position(cove_core::Point::new(200.0, 50.0)),
            Some(position(&second, 2)),
            "a point past the paragraph's right edge clamps to its caret"
        );
        assert_eq!(
            scope
                .borrow_mut()
                .position(cove_core::Point::new(20.0, 30.0)),
            Some(position(&second, 0)),
            "a point above a paragraph clamps to its first caret"
        );
        assert_eq!(
            scope
                .borrow_mut()
                .position(cove_core::Point::new(20.0, 90.0)),
            Some(position(&second, 6)),
            "a point below a paragraph clamps to its last caret"
        );

        let mut frame = container.frame();
        frame.x = 100.0;
        container.set_frame(frame);
        assert!(
            !scope
                .borrow_mut()
                .text_under(cove_core::Point::new(20.0, 10.0)),
            "moving the ancestor horizontally moves the paragraph too"
        );
        assert!(
            scope
                .borrow_mut()
                .text_under(cove_core::Point::new(120.0, 10.0)),
            "the paragraph now answers at the shifted point"
        );

        container.remove_child(&first);
        assert!(
            !scope
                .borrow_mut()
                .text_under(cove_core::Point::new(120.0, 10.0)),
            "removing the paragraph drops it from hit testing"
        );
        assert_eq!(
            scope
                .borrow_mut()
                .position(cove_core::Point::new(120.0, 10.0)),
            Some(position(&second, 0)),
            "the remaining paragraph answers instead"
        );
    });
}

#[test]
fn detached_subtrees_and_reused_nodes_leave_their_previous_scope() {
    GlobalTestLock::locked(|| {
        let scope = new_scope();
        let next = TextSelectionScope::new();
        let next = Rc::new(RefCell::new(next));
        let root = Node::new();
        let container = Node::new();
        root.add_child(&container);

        let node = paragraph(&scope, "hello", Rect::new(0.0, 0.0, 80.0, 20.0), &container);
        assert!(
            scope
                .borrow_mut()
                .text_under(cove_core::Point::new(10.0, 10.0)),
            "the mounted paragraph answers hit tests"
        );

        root.remove_child(&container);
        assert_eq!(
            node.parent().map(|parent| parent.id()),
            Some(container.id()),
            "the paragraph keeps its immediate parent while its subtree is detached"
        );
        assert!(
            !scope
                .borrow_mut()
                .text_under(cove_core::Point::new(10.0, 10.0)),
            "a detached subtree leaves the scope"
        );

        root.add_child(&container);
        TextSelectionScope::register(
            &scope,
            &node,
            Rc::new(|| "hello".to_string()),
            Rc::new(|_point| 2),
        );
        assert!(
            scope
                .borrow_mut()
                .text_under(cove_core::Point::new(10.0, 10.0)),
            "re-registering restores it"
        );

        TextSelectionScope::register(
            &next,
            &node,
            Rc::new(|| "hello".to_string()),
            Rc::new(|_point| 2),
        );
        assert!(
            !scope
                .borrow_mut()
                .text_under(cove_core::Point::new(10.0, 10.0)),
            "registering with another scope removes it from the first"
        );
        assert!(
            next.borrow_mut()
                .text_under(cove_core::Point::new(10.0, 10.0)),
            "the new scope owns the node"
        );

        TextSelectionScope::unregister(&node);
        assert!(
            !next
                .borrow_mut()
                .text_under(cove_core::Point::new(10.0, 10.0)),
            "unregistering removes it from the current scope"
        );
    });
}

#[test]
fn reflow_and_insertion_update_cached_ranges() {
    GlobalTestLock::locked(|| {
        let scope = new_scope();
        let root = Node::new();
        let mut tree = NodeTree::new();
        tree.set_root(Some(root.clone()));
        let a = paragraph(&scope, "alpha", Rect::new(0.0, 0.0, 80.0, 20.0), &root);
        let b = paragraph(&scope, "beta", Rect::new(0.0, 30.0, 80.0, 20.0), &root);
        let c = paragraph(&scope, "gamma", Rect::new(0.0, 60.0, 80.0, 20.0), &root);

        scope.borrow_mut().select(position(&a, 0), position(&c, 5));
        assert_eq!(
            scope.borrow_mut().selected_text(),
            "alpha\nbeta\ngamma",
            "rows join with newlines in reading order"
        );
        flush(&mut tree);

        let mut frame = b.frame();
        frame.y = 90.0;
        b.set_frame(frame);
        assert_eq!(
            scope.borrow_mut().range_for(&b),
            None,
            "a row that reflows out of the span drops its range"
        );
        assert!(
            b.render_dirty(),
            "the moved row repaints to clear its highlight"
        );
        assert_eq!(
            scope.borrow_mut().selected_text(),
            "alpha\ngamma",
            "the reflowed row leaves the copied text"
        );

        let d = paragraph(&scope, "delta", Rect::new(0.0, 30.0, 80.0, 20.0), &root);
        assert_eq!(
            scope.borrow_mut().range_for(&d),
            Some(0..5),
            "a paragraph inserted into the span joins the selection"
        );
        assert_eq!(
            scope.borrow_mut().selected_text(),
            "alpha\ndelta\ngamma",
            "the inserted paragraph takes the reflowed row's place"
        );

        root.remove_child(&c);
        assert!(
            scope.borrow_mut().selected_text().is_empty(),
            "removing an endpoint collapses the selection"
        );
        assert_eq!(
            scope.borrow_mut().range_for(&a),
            None,
            "no paragraph keeps a range once the selection collapses"
        );
    });
}

#[test]
fn reverse_selections_preserve_utf8_and_table_reading_order() {
    GlobalTestLock::locked(|| {
        let scope = new_scope();
        let root = Node::new();
        let right = paragraph(&scope, "🙂右", Rect::new(100.0, 0.0, 80.0, 20.0), &root);
        let left = paragraph(&scope, "中文", Rect::new(0.0, 0.4, 80.0, 20.0), &root);
        let bottom = paragraph(&scope, "末尾", Rect::new(0.0, 30.0, 80.0, 20.0), &root);

        scope
            .borrow_mut()
            .select(position(&bottom, 3), position(&left, 3));
        assert_eq!(
            scope.borrow_mut().selected_text(),
            "文\t🙂右\n末",
            "same-row cells join with a tab, rows with a newline, and UTF-8 survives"
        );
        assert_eq!(
            scope.borrow_mut().range_for(&right),
            Some(0..7),
            "the fully covered cell keeps its whole byte range"
        );

        scope.borrow_mut().select_all();
        assert_eq!(
            scope.borrow_mut().selected_text(),
            "中文\t🙂右\n末尾",
            "select-all follows the same reading order"
        );

        scope.borrow_mut().clear();
        assert_eq!(
            scope
                .borrow_mut()
                .position(cove_core::Point::new(-100.0, -100.0)),
            Some(position(&left, 0)),
            "a point outside every paragraph clamps to the first one"
        );
    });
}

#[test]
fn large_pages_keep_lookup_and_endpoint_updates_responsive() {
    GlobalTestLock::locked(|| {
        let scope = new_scope();
        let root = Node::new();
        let mut container = root.clone();
        for _ in 0..12 {
            let child = Node::new();
            container.add_child(&child);
            container = child;
        }

        let mut tree = NodeTree::new();
        tree.set_root(Some(root.clone()));
        let nodes: Vec<Node> = (0..1_000)
            .map(|index| {
                paragraph(
                    &scope,
                    "paragraph",
                    Rect::new(0.0, index as f32 * 24.0, 100.0, 20.0),
                    &container,
                )
            })
            .collect();

        let start = std::time::Instant::now();
        scope.borrow_mut().select_all();
        let mut selected_bytes = 0usize;
        for _ in 0..3 {
            let mut scope_mut = scope.borrow_mut();
            for node in &nodes {
                selected_bytes += scope_mut.range_for(node).map_or(0, |range| range.len());
            }
        }
        assert_eq!(
            selected_bytes, 27_000,
            "three passes over 1,000 nine-byte paragraphs total 27,000 bytes"
        );

        for offset in 1..=200usize {
            flush(&mut tree);
            let mut scope_mut = scope.borrow_mut();
            scope_mut.extend(position(&nodes[999], 1 + offset % 8));
            drop(scope_mut);
            assert!(
                !nodes[0].render_dirty(),
                "an endpoint update near the end never repaints the first paragraph"
            );
            assert!(
                !nodes[500].render_dirty(),
                "an endpoint update near the end never repaints the middle"
            );
        }

        assert_eq!(
            scope.borrow_mut().selected_text().split('\n').count(),
            1_000,
            "every paragraph contributes one line"
        );
        let elapsed = start.elapsed();
        assert!(
            elapsed < std::time::Duration::from_secs(3),
            "1,000 paragraphs, 3,000 lookups and 200 endpoint updates took {elapsed:?}"
        );
    });
}

#[test]
fn selection_scope_survives_rebuild() {
    use cove_composition::{AnyView, Column, HorizontalAlignment, TupleView};
    use cove_core::scene::AccessibilityRole;
    use cove_ui_compose::primitives::text::Text;
    use cove_ui_compose::primitives::text::rich::text_selection::TextSelectionRoot;
    fn region() -> TextSelectionRoot {
        TextSelectionRoot::new(AnyView::new(Column::new(
            HorizontalAlignment::Leading,
            8.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(Text::new("first paragraph")),
                AnyView::new(Text::new("middle paragraph")),
                AnyView::new(Text::new("last paragraph")),
            ])),
        )))
    }
    struct RefreshingRegion(cove_core::State<u32>);
    impl cove_composition::View for RefreshingRegion {
        fn body(&self, _cx: &cove_composition::Scope) -> AnyView {
            let _ = self.0.get();
            AnyView::new(region())
        }
    }
    support::InteractiveScene::run(400.0, 120.0, |scene| {
        let generation = cove_core::State::new(0);
        scene.install(RefreshingRegion(generation.clone()));
        let first = scene
            .labelled("first paragraph", Some(AccessibilityRole::StaticText))
            .unwrap();
        let last = scene
            .labelled("last paragraph", Some(AccessibilityRole::StaticText))
            .unwrap();
        let scope = first
            .composition_value(&TextSelectionScope::key())
            .unwrap()
            .0;
        scope
            .borrow_mut()
            .select(position(&first, 2), position(&last, 4));
        assert!(scope.borrow().has_selection());

        generation.set(1);
        scene.settle(2, 1.0);
        let first = scene
            .labelled("first paragraph", Some(AccessibilityRole::StaticText))
            .unwrap();
        let mounted = first.composition_value(&TextSelectionScope::key()).unwrap();
        assert!(
            Rc::ptr_eq(&scope, &mounted.0),
            "rebuilding the wrapper preserves its mounted scope"
        );
        assert!(scope.borrow().has_selection());
    });
}

#[test]
fn clicking_outside_text_clears_selection_without_consuming_the_click() {
    use cove_composition::{AnyView, Column, HorizontalAlignment, TupleView};
    use cove_core::scene::AccessibilityRole;
    use cove_ui_compose::primitives::controls::Button;
    use cove_ui_compose::primitives::text::Text;
    use cove_ui_compose::primitives::text::rich::text_selection::TextSelectionRoot;
    use std::cell::Cell;

    support::InteractiveScene::run(320.0, 120.0, |scene| {
        let calls = Rc::new(Cell::new(0));
        let button_calls = calls.clone();
        scene.install(Column::new(
            HorizontalAlignment::Leading,
            8.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(TextSelectionRoot::new(AnyView::new(Text::new(
                    "selected text",
                )))),
                AnyView::new(Button::new(Text::new("Other control"), move || {
                    button_calls.set(button_calls.get() + 1);
                })),
            ])),
        ));

        let text = scene
            .labelled("selected text", Some(AccessibilityRole::StaticText))
            .unwrap();
        let text_frame = text.absolute_frame();
        let button = scene
            .first(|node| {
                let frame = node.absolute_frame();
                frame.y > text_frame.y && frame.width > 0.0 && frame.height > 0.0
            })
            .unwrap();
        let scope = text
            .composition_value(&TextSelectionScope::key())
            .unwrap()
            .0;
        scope
            .borrow_mut()
            .select(position(&text, 0), position(&text, 8));
        assert!(scope.borrow().has_selection());

        scene.click(&button, 1);

        assert!(!scope.borrow().has_selection());
        assert_eq!(calls.get(), 1, "the clicked control still receives input");
    });
}

#[test]
fn selection_adapters_cannot_leave_endpoints_inside_a_joined_grapheme() {
    GlobalTestLock::locked(|| {
        let scope = new_scope();
        let root = Node::new();
        let node = paragraph(
            &scope,
            "e\u{301}👩🏽‍💻x",
            Rect::new(0.0, 0.0, 160.0, 20.0),
            &root,
        );
        scope
            .borrow_mut()
            .select(position(&node, 0), position(&node, 6));
        assert_eq!(scope.borrow_mut().selected_text(), "e\u{301}");
        scope
            .borrow_mut()
            .select(position(&node, 1), position(&node, 18));
        assert_eq!(scope.borrow_mut().selected_text(), "e\u{301}👩🏽‍💻");
    });
}
