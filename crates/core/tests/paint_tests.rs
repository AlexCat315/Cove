//! Translation of `Tests/CoveUIRuntimeTests/Transforms/PaintTests.swift`.
//!
//! Custom content, its clips and its overlay have to scale with a nested
//! translated node. The Swift suite runs the same scene through the legacy
//! `NodeRenderer` walk and through `LayerAwareNodeRenderer` and demands identical
//! geometry, because the two paths must not diverge on transformed subtrees.
//!
//! Behaviour checklist:
//! - custom draw content and its clip land in the scaled, translated content frame
//! - an overlay draws after the content and is likewise scaled
//! - the batch scissor stack is the content clip inside the group bounds
//! - the legacy walker and the layer-aware renderer agree vertex for vertex

use std::rc::Rc;

use cove_core::scene::{
    DrawHook, LayerAwareNodeRenderer, Node, NodeRenderer, NodeTransform, RenderTree,
};
use cove_core::{Color, DrawList, Rect};

fn framed(x: f32, y: f32, width: f32, height: f32) -> Node {
    let node = Node::new();
    node.set_frame(Rect::new(x, y, width, height));
    node
}

#[test]
fn custom_content() {
    let root_node = framed(0.0, 0.0, 400.0, 300.0);
    let group = framed(40.0, 20.0, 200.0, 160.0);
    group.set_transform(NodeTransform::anchored(0.5, 0.0, 0.0));
    root_node.add_child(&group);

    let content = framed(10.0, 20.0, 80.0, 40.0);
    content.set_clips_to_bounds(true);

    let draw: DrawHook = Rc::new(|list: &mut DrawList, origin: cove_core::Point| {
        let rect = Rect::new(origin.x + 4.0, origin.y + 6.0, 20.0, 12.0);
        list.push_clip_rect(rect);
        list.add_rect(rect, Color::WHITE).expect("the rect fits");
        list.pop_clip_rect();
    });
    content.set_draw(Some(draw));

    let overlay: DrawHook = Rc::new(|list: &mut DrawList, origin: cove_core::Point| {
        list.add_rect(
            Rect::new(origin.x + 30.0, origin.y + 10.0, 10.0, 8.0),
            Color::BLACK,
        )
        .expect("the rect fits");
    });
    content.set_overlay_draw(Some(overlay));

    group.add_child(&content);

    let tree = RenderTree::new();
    tree.install(&root_node);

    let mut renderer = LayerAwareNodeRenderer::new();
    for layered in [false, true] {
        for _ in 0..3 {
            let mut list = DrawList::default();
            if layered {
                renderer.render(&tree, &mut list);
            } else {
                NodeRenderer.render(&root_node, &mut list);
            }

            assert_eq!(
                content.absolute_frame(),
                Rect::new(45.0, 30.0, 40.0, 20.0),
                "the content is scaled about the group origin (layered={layered})"
            );

            let white = Color::WHITE.rgba8();
            let whites: Vec<_> = list
                .vertices()
                .iter()
                .filter(|vertex| vertex.color == white)
                .collect();
            assert_eq!(whites.len(), 4, "one white quad (layered={layered})");
            let white_x_min = whites.iter().fold(f32::MAX, |acc, v| acc.min(v.x));
            let white_x_max = whites.iter().fold(f32::MIN, |acc, v| acc.max(v.x));
            let white_y_min = whites.iter().fold(f32::MAX, |acc, v| acc.min(v.y));
            let white_y_max = whites.iter().fold(f32::MIN, |acc, v| acc.max(v.y));
            assert_eq!(
                white_x_min, 47.0,
                "white spans from x=47 (layered={layered})"
            );
            assert_eq!(white_x_max, 57.0, "white reaches x=57 (layered={layered})");
            assert_eq!(
                white_y_min, 33.0,
                "white spans from y=33 (layered={layered})"
            );
            assert_eq!(white_y_max, 39.0, "white reaches y=39 (layered={layered})");

            let black = Color::BLACK.rgba8();
            let blacks: Vec<_> = list
                .vertices()
                .iter()
                .filter(|vertex| vertex.color == black)
                .collect();
            let black_x_min = blacks.iter().fold(f32::MAX, |acc, v| acc.min(v.x));
            let black_x_max = blacks.iter().fold(f32::MIN, |acc, v| acc.max(v.x));
            let black_y_min = blacks.iter().fold(f32::MAX, |acc, v| acc.min(v.y));
            let black_y_max = blacks.iter().fold(f32::MIN, |acc, v| acc.max(v.y));
            assert_eq!(
                black_x_min, 60.0,
                "overlay starts at x=60 (layered={layered})"
            );
            assert_eq!(
                black_x_max, 65.0,
                "overlay reaches x=65 (layered={layered})"
            );
            assert_eq!(
                black_y_min, 35.0,
                "overlay starts at y=35 (layered={layered})"
            );
            assert_eq!(
                black_y_max, 39.0,
                "overlay reaches y=39 (layered={layered})"
            );

            let first_scissor = list
                .batches()
                .first()
                .and_then(|batch| batch.scissor)
                .expect("the content clip scissor");
            assert_eq!(
                first_scissor,
                Rect::new(47.0, 33.0, 10.0, 6.0),
                "the content's own clip is scaled with it (layered={layered})"
            );

            let last_scissor = list
                .batches()
                .last()
                .and_then(|batch| batch.scissor)
                .expect("the group clip scissor");
            assert_eq!(
                last_scissor,
                Rect::new(45.0, 30.0, 40.0, 20.0),
                "the scaled group bounds clip the overlay (layered={layered})"
            );
        }
    }
}
