//! Translation of `Tests/CoveUIRuntimeTests/Rendering/CompositionPaintTests.swift`.
//!
//! A custom painter reads a `CompositionLocal`. Its retained layer must stay
//! cached while the value it reads cannot change, and must be rebuilt when an
//! inherited provider above it changes — but a node that pins its own value is
//! unaffected by that ancestor.
//!
//! Behaviour checklist:
//! - a provider change invalidates a painter that inherits the value
//! - the re-recorded painter paints the new value
//! - a node that overrides the value locally keeps its cache across the change

use std::cell::Cell;
use std::rc::Rc;

use cove_core::scene::{CompositionLocal, LayerAwareNodeRenderer, Node, RenderTree};
use cove_core::{Color, DrawList, Point, Rect};

#[test]
fn inherited_value_changes() {
    let color = Rc::new(CompositionLocal::constant(Color::WHITE));
    let root = Node::new();
    let inherited = Node::new();
    let pinned = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
    inherited.set_frame(Rect::new(0.0, 0.0, 40.0, 40.0));
    pinned.set_frame(Rect::new(50.0, 0.0, 40.0, 40.0));
    root.add_child(&inherited);
    root.add_child(&pinned);
    root.set_composition_value(&color, Color::BLACK);
    pinned.set_composition_value(&color, Color::WHITE);

    let inherited_draws = Rc::new(Cell::new(0_usize));
    let pinned_draws = Rc::new(Cell::new(0_usize));

    let counter = inherited_draws.clone();
    let reader = inherited.clone();
    let local = color.clone();
    inherited.set_draw(Some(Rc::new(move |list: &mut DrawList, _: Point| {
        counter.set(counter.get() + 1);
        let _ = list.add_rect(
            Rect::new(0.0, 0.0, 40.0, 40.0),
            reader.composition_value(&local),
        );
    })));

    let counter = pinned_draws.clone();
    let reader = pinned.clone();
    let local = color.clone();
    pinned.set_draw(Some(Rc::new(move |list: &mut DrawList, _: Point| {
        counter.set(counter.get() + 1);
        let _ = list.add_rect(
            Rect::new(50.0, 0.0, 40.0, 40.0),
            reader.composition_value(&local),
        );
    })));

    let tree = RenderTree::new();
    tree.install(&root);
    let mut renderer = LayerAwareNodeRenderer::new();
    for _ in 0..3 {
        renderer.render(&tree, &mut DrawList::default());
    }
    assert_eq!(
        inherited_draws.get(),
        1,
        "the inherited painter is recorded once"
    );
    assert_eq!(pinned_draws.get(), 1, "the pinned painter is recorded once");

    root.set_composition_value(&color, Color::WHITE);
    let mut updated = DrawList::default();
    renderer.render(&tree, &mut updated);
    assert!(
        updated
            .vertices()
            .iter()
            .all(|vertex| vertex.color == Color::WHITE.rgba8()),
        "every vertex paints the new white value",
    );
    assert_eq!(
        inherited_draws.get(),
        2,
        "the inherited painter re-records for the new value",
    );
    assert_eq!(
        pinned_draws.get(),
        1,
        "the locally overridden painter keeps its cache",
    );
}
