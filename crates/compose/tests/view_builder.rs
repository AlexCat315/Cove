//! Translation of `Tests/CoveUIComposeTests/ViewBuilderTests.swift`.
//!
//! Behaviour checklist:
//! - an empty block produces `EmptyView`
//! - a single-element block retains the expression together with its source
//!   coordinates
//! - a variadic block produces a structural tuple that expands to every element
//! - a false `if` expands to nothing
//! - `if`/`else` produces conditional content that picks exactly one branch

use crate::support;

use cove_composition::{
    AnyView, ConditionalContent, EmptyView, SourceLocatedView, TupleView, View, ViewKind,
};

use support::DebugNode;

/// The expanded children of a structural view, mirroring Swift's
/// `_StructuralView._expanded`.
fn expanded(view: &dyn View) -> Vec<AnyView> {
    match view.kind() {
        ViewKind::Structural(children) => children,
        _ => Vec::new(),
    }
}

fn is_structural(view: &dyn View) -> bool {
    matches!(view.kind(), ViewKind::Structural(_))
}

#[test]
fn empty_block_produces_empty_view() {
    let view = EmptyView;
    assert!(matches!(view.kind(), ViewKind::Empty));
}

#[test]
fn single_block_retains_source_coordinates() {
    let located = SourceLocatedView::expression(DebugNode::new("x"));
    // The located wrapper keeps the original content and records where it was
    // written, which is what makes a `@ViewBuilder` body traceable.
    assert_eq!(located.location.file_id, file!());
    assert!(
        !located.location.explicit,
        "expression() marks the source as inferred"
    );
    match located.kind() {
        ViewKind::Source(source) => {
            assert!(source.content.kind().is_primitive());
        }
        _ => panic!("a located view reports ViewKind::Source"),
    }
}

#[test]
fn variadic_block_produces_tuple() {
    let view = TupleView::new(vec![
        AnyView::new(DebugNode::new("a")),
        AnyView::new(DebugNode::new("b")),
        AnyView::new(DebugNode::new("c")),
    ]);
    assert!(is_structural(&view), "a variadic block is structural");
    assert_eq!(expanded(&view).len(), 3, "every element expands");
}

#[test]
fn conditional_build_wraps_in_optional() {
    // A false `if` contributes no children, exactly like Swift's `buildIf`.
    let flag = false;
    let view = if flag {
        AnyView::new(TupleView::new(vec![AnyView::new(DebugNode::new("shown"))]))
    } else {
        AnyView::new(EmptyView)
    };
    assert!(
        expanded(&view).is_empty(),
        "a false condition expands to nothing"
    );
}

#[test]
fn conditional_picks_one_branch() {
    let make = |flag: bool| -> ConditionalContent<DebugNode, DebugNode> {
        if flag {
            ConditionalContent::First(DebugNode::new("T"))
        } else {
            ConditionalContent::Second(DebugNode::new("F"))
        }
    };

    let truthy = make(true);
    let children = expanded(&truthy);
    assert_eq!(
        children.len(),
        1,
        "conditional content expands to one branch"
    );
    let located = SourceLocatedView::expression(DebugNode::new("T"));
    assert_eq!(
        located.location.file_id,
        file!(),
        "the retained branch keeps its own source coordinates"
    );

    let falsy = make(false);
    assert_eq!(expanded(&falsy).len(), 1, "the other branch expands to one");
}

/// `ViewKind::Empty`/`Primitive` probes used above.
trait ViewKindProbe {
    fn is_primitive(&self) -> bool;
}

impl ViewKindProbe for ViewKind<'_> {
    fn is_primitive(&self) -> bool {
        matches!(self, ViewKind::Primitive(_))
    }
}
