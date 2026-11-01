//! Translation of `Tests/CoveUIRuntimeTests/Rendering/BackdropOpacityTests.swift`.
//!
//! Frost opacity is the product of every ancestor's opacity, the node's own
//! opacity and its subtree opacity. An error here is visible as a frosted panel
//! that stays fully opaque while its contents fade, so both the direct and the
//! layer-cached renderer must compute the same value — including once a cached
//! layer is reused at a stable opacity.
//!
//! Behaviour checklist:
//! - frost follows nested opacity through cached and scaled rendering
//! - a scaled custom painter preserves its backdrop opacity

use std::rc::Rc;

use cove_core::scene::{
    DrawList, LayerAwareNodeRenderer, Node, NodeRenderer, NodeTransform, RenderTree,
};
use cove_core::{Backdrop, Color, Point, Rect};

fn node(x: f32, y: f32, width: f32, height: f32) -> Node {
    let node = Node::new();
    node.set_frame(Rect::new(x, y, width, height));
    node
}

fn frost(list: &DrawList) -> Option<Backdrop> {
    list.batches().iter().find_map(|batch| batch.backdrop)
}

#[test]
fn frost_follows_nested_opacity_through_cached_and_scaled_rendering() {
    let root = node(0.0, 0.0, 200.0, 200.0);
    root.set_background_color(Some(Color::BLACK));
    let group = node(0.0, 0.0, 200.0, 200.0);
    root.add_child(&group);
    let panel = node(20.0, 20.0, 100.0, 80.0);
    group.add_child(&panel);
    panel.set_background_color(Some(Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.1,
    }));
    panel.set_backdrop_blur(12.0);
    panel.set_opacity(0.5);
    panel.set_subtree_opacity(0.5);

    let tree = RenderTree::new();
    tree.install(&root);
    let mut renderer = LayerAwareNodeRenderer::new();

    for opacity in [0.0_f32, 0.25, 0.25, 1.0, 0.0] {
        group.set_opacity(opacity);
        group.set_transform(NodeTransform::new(
            if opacity == 0.25 { 0.94 } else { 1.0 },
            Point { x: 0.0, y: 0.0 },
        ));
        // Render twice, including a stable cache reuse at each value.
        for _ in 0..2 {
            let mut direct = DrawList::default();
            NodeRenderer::new().render(&root, &mut direct);
            let mut cached = DrawList::default();
            renderer.render(&tree, &mut cached);

            let direct_frost = frost(&direct).expect("the direct render frosted the panel");
            let cached_frost = frost(&cached).expect("the cached render frosted the panel");
            assert!(
                (direct_frost.opacity - opacity * 0.25).abs() < 0.0001,
                "at ancestor opacity {opacity} the frost was {}",
                direct_frost.opacity,
            );
            assert_eq!(cached_frost, direct_frost);
            assert_eq!(direct.vertices()[0].color, Color::BLACK.rgba8());
            assert_eq!(cached.vertices()[0].color, Color::BLACK.rgba8());
        }
    }
}

#[test]
fn a_scaled_custom_painter_preserves_its_backdrop_opacity() {
    let root = node(0.0, 0.0, 200.0, 200.0);
    let panel = node(0.0, 0.0, 200.0, 200.0);
    root.add_child(&panel);
    panel.set_transform(NodeTransform::new(0.94, Point { x: 0.0, y: 0.0 }));
    panel.set_draw(Some(Rc::new(|list: &mut DrawList, origin: Point| {
        let rect = Rect::new(origin.x + 10.0, origin.y + 10.0, 80.0, 60.0);
        list.begin_backdrop(Backdrop {
            rect,
            corner_radius: 0.0,
            blur_radius: 12.0,
            opacity: 0.3,
        });
        list.add_rect(rect, Color::WHITE);
    })));

    let mut list = DrawList::default();
    NodeRenderer::new().render(&root, &mut list);

    let frost = frost(&list).expect("the custom painter frosted its rect");
    assert_eq!(frost.opacity, 0.3);
    assert!(
        (frost.rect.width - 75.2).abs() < 0.001,
        "the frosted rect scales with the subtree, width was {}",
        frost.rect.width,
    );
}
