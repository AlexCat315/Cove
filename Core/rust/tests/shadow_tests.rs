//! Translation of `Tests/CoveUIRuntimeTests/Transforms/ShadowTests.swift`.
//!
//! A shadow is drawn in the node's own units, then scaled with the box, so the
//! renderer must resolve its lengths at paint time without writing them back
//! into the authored style. A lifecycle transform (`scales_shadow == false`)
//! deliberately keeps elevation fixed while the paint shrinks, and that decision
//! has to survive both the uncached and the retained renderer.
//!
//! Behaviour checklist:
//! - an ordinary scale changes both shadow layers
//! - a lifecycle scale retains shadow offsets and blur while paint shrinks
//! - rendering never writes resolved lengths back into the authored styles
//! - nested ordinary scales accumulate around a lifecycle scale
//! - a larger inherited shadow brings an offscreen cached leaf back into view

use cove_core::scene::{LayerAwareNodeRenderer, Node, NodeRenderer, NodeTransform, RenderTree};
use cove_core::{Color, DrawList, LayeredShadow, Point, Rect, SurfaceShadow};

fn group(frame: Rect, scale: f32, scales_shadow: bool) -> Node {
    let node = Node::new();
    node.set_frame(frame);
    node.set_transform(NodeTransform::new(scale, Point { x: 0.0, y: 0.0 }).with_scales_shadow(scales_shadow));
    node
}

fn configure_shadows(node: &Node) {
    node.set_corner_radius(8.0);
    node.set_ambient_shadow(SurfaceShadow {
        color: Some(Color::WHITE),
        offset_x: -2.0,
        offset_y: -4.0,
        blur: 6.0,
    });
    node.set_shadow(SurfaceShadow {
        color: Some(Color::BLACK),
        offset_x: 6.0,
        offset_y: 10.0,
        blur: 8.0,
    });
}

fn reference_for(node: &Node, shadow_scale: f32, paint_scale: f32) -> DrawList {
    let bounds = node.absolute_frame();
    let rect = Rect::new(bounds.x, bounds.y, bounds.width, bounds.height);
    let mut list = DrawList::default();
    list.add_shadow(
        LayeredShadow {
            ambient: SurfaceShadow {
                color: Some(Color::WHITE),
                offset_x: -2.0 * shadow_scale,
                offset_y: -4.0 * shadow_scale,
                blur: 6.0 * shadow_scale,
            },
            key: SurfaceShadow {
                color: Some(Color::BLACK),
                offset_x: 6.0 * shadow_scale,
                offset_y: 10.0 * shadow_scale,
                blur: 8.0 * shadow_scale,
            },
        },
        rect,
        8.0 * paint_scale,
        1.0,
        None,
        None,
    );
    list
}

fn expect_same_geometry(actual: &DrawList, expected: &DrawList, label: &str) {
    assert_eq!(
        actual.vertices().len(),
        expected.vertices().len(),
        "{label}: vertex count"
    );
    assert_eq!(actual.indices(), expected.indices(), "{label}: indices");
    for (index, (actual, expected)) in actual
        .vertices()
        .iter()
        .zip(expected.vertices().iter())
        .enumerate()
    {
        assert!(
            (actual.x - expected.x).abs() < 0.0001 && (actual.y - expected.y).abs() < 0.0001,
            "{label}: vertex {index} at ({}, {}) expected ({}, {})",
            actual.x,
            actual.y,
            expected.x,
            expected.y,
        );
        assert_eq!(
            actual.color, expected.color,
            "{label}: vertex {index} colour"
        );
    }
}

#[test]
fn shadow_policy() {
    for scales_shadow in [true, false] {
        let root = group(Rect::new(0.0, 0.0, 800.0, 600.0), 1.0, true);
        let panel = group(Rect::new(100.0, 80.0, 200.0, 100.0), 0.5, scales_shadow);
        root.add_child(&panel);
        configure_shadows(&panel);

        let expected = reference_for(
            &panel,
            if scales_shadow { 0.5 } else { 1.0 },
            0.5,
        );

        let mut direct = DrawList::default();
        NodeRenderer.render(&root, &mut direct);
        expect_same_geometry(
            &direct,
            &expected,
            &format!("uncached scales_shadow={scales_shadow}"),
        );

        let render = RenderTree::new();
        render.install(&root);
        let mut renderer = LayerAwareNodeRenderer::new();
        for pass in 0..3 {
            let mut cached = DrawList::default();
            renderer.render(&render, &mut cached);
            expect_same_geometry(
                &cached,
                &expected,
                &format!("cached scales_shadow={scales_shadow} pass={pass}"),
            );
        }

        // Rendering never writes resolved lengths back into authored styles.
        assert_eq!(panel.shadow().offset_y, 10.0, "authored key offsetY");
        assert_eq!(panel.shadow().blur, 8.0, "authored key blur");
        assert_eq!(panel.ambient_shadow().offset_y, -4.0, "authored ambient offsetY");
        assert_eq!(panel.ambient_shadow().blur, 6.0, "authored ambient blur");
    }
}

#[test]
fn nested_scales_and_cache() {
    let root = group(Rect::new(0.0, 0.0, 1000.0, 800.0), 1.0, true);
    let outer = group(Rect::new(30.0, 40.0, 300.0, 200.0), 2.0, true);
    let reveal = group(Rect::new(20.0, 10.0, 200.0, 100.0), 0.5, false);
    let panel = group(Rect::new(12.0, 14.0, 100.0, 60.0), 0.5, true);
    root.add_child(&outer);
    outer.add_child(&reveal);
    reveal.add_child(&panel);
    configure_shadows(&panel);

    let render = RenderTree::new();
    render.install(&root);
    let mut renderer = LayerAwareNodeRenderer::new();

    for scales_shadow in [true, false, true] {
        outer.set_transform(
            NodeTransform::new(2.0, Point { x: 0.0, y: 0.0 }).with_scales_shadow(scales_shadow),
        );
        let expected = reference_for(&panel, if scales_shadow { 1.0 } else { 0.5 }, 0.5);
        for pass in 0..3 {
            let mut cached = DrawList::default();
            renderer.render(&render, &mut cached);
            expect_same_geometry(
                &cached,
                &expected,
                &format!("cached scales_shadow={scales_shadow} pass={pass}"),
            );
        }
        let mut direct = DrawList::default();
        NodeRenderer.render(&root, &mut direct);
        expect_same_geometry(
            &direct,
            &expected,
            &format!("uncached scales_shadow={scales_shadow}"),
        );
        assert_eq!(
            panel.absolute_frame(),
            Rect::new(82.0, 74.0, 50.0, 30.0),
            "the nested scale composition is unchanged by shadow policy",
        );
    }
}

#[test]
fn expanded_shadow_culling() {
    let root = group(Rect::new(0.0, 0.0, 500.0, 400.0), 1.0, true);
    let outer = group(Rect::new(0.0, 0.0, 800.0, 400.0), 2.0, false);
    let reveal = group(Rect::new(0.0, 0.0, 800.0, 400.0), 0.5, false);
    let panel = group(Rect::new(510.0, 100.0, 100.0, 60.0), 1.0, true);
    root.add_child(&outer);
    outer.add_child(&reveal);
    reveal.add_child(&panel);
    configure_shadows(&panel);

    let render = RenderTree::new();
    render.install(&root);
    let mut renderer = LayerAwareNodeRenderer::new();

    for pass in 0..3 {
        let mut list = DrawList::default();
        renderer.render(&render, &mut list);
        assert!(
            list.vertices().is_empty(),
            "the offscreen leaf stays culled on pass {pass}",
        );
    }

    outer.set_transform(
        NodeTransform::new(2.0, Point { x: 0.0, y: 0.0 }).with_scales_shadow(true),
    );
    let mut visible = DrawList::default();
    renderer.render(&render, &mut visible);
    assert!(
        !visible.vertices().is_empty(),
        "a larger inherited shadow expands the paint bounds"
    );
    assert!(
        visible.vertices().iter().any(|vertex| vertex.x < 500.0),
        "the enlarged shadow reaches back into the viewport",
    );
}
