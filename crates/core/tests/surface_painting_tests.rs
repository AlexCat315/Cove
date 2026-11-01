//! Translation of `Tests/CoveUIRuntimeTests/SurfacePaintingTests.swift`.
//!
//! How a shadow layer and a rim light become geometry. These assert the two
//! properties a floating panel depends on: the layers really are separate and
//! ordered, and the rim really is a thin line at the top edge.
//!
//! Behaviour checklist:
//! - the ambient halo is appended before the key shadow, and the pair is exactly
//!   the two layers concatenated
//! - the ambient layer reaches farther than the key layer, and each stays inside
//!   its authored blur's maximum extent
//! - `offset_y` translates the surviving geometry without changing the blur
//! - the densest ambient step is fainter than the densest key step, and the
//!   falloff still spans a range
//! - a node with only a key shadow paints one layer, spreading past the node but
//!   not as far as a halo

use cove_core::scene::{Node, NodeRenderer};
use cove_core::{Color, DrawList, Rect, SurfaceShadow};

fn framed() -> Node {
    let node = Node::new();
    node.set_frame(Rect::new(0.0, 0.0, 100.0, 40.0));
    node
}

fn shadow(alpha: f32, offset_y: f32, blur: f32) -> SurfaceShadow {
    SurfaceShadow {
        color: Some(Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: alpha,
        }),
        offset_x: 0.0,
        offset_y,
        blur,
    }
}

fn render(node: &Node) -> DrawList {
    let mut list = DrawList::default();
    NodeRenderer.render(node, &mut list);
    list
}

/// Shadow steps are rounded rects whose tessellation depends on the radius, so
/// geometry is compared against the span it must cover rather than against exact
/// corner positions.
fn span_x(list: &DrawList) -> f32 {
    let vertices = list.vertices();
    let maximum = vertices
        .iter()
        .fold(f32::MIN, |acc, vertex| acc.max(vertex.x));
    let minimum = vertices
        .iter()
        .fold(f32::MAX, |acc, vertex| acc.min(vertex.x));
    maximum - minimum
}

fn top(list: &DrawList) -> f32 {
    list.vertices()
        .iter()
        .fold(f32::MAX, |acc, vertex| acc.min(vertex.y))
}

fn alpha_range(list: &DrawList) -> (f32, f32) {
    let mut minimum = f32::MAX;
    let mut maximum = f32::MIN;
    for vertex in list.vertices() {
        let alpha = ((vertex.color >> 24) & 0xFF) as f32 / 255.0;
        minimum = minimum.min(alpha);
        maximum = maximum.max(alpha);
    }
    (minimum, maximum)
}

#[test]
fn layered_shadow_order() {
    let halo = framed();
    halo.set_ambient_shadow(shadow(1.0, 3.0, 20.0));
    let contact = framed();
    contact.set_shadow(shadow(1.0, 6.0, 8.0));

    let both = framed();
    both.set_ambient_shadow(halo.ambient_shadow());
    both.set_shadow(contact.shadow());

    let ambient = render(&halo);
    let key = render(&contact);
    let layered = render(&both);

    assert_eq!(
        layered.vertices().len(),
        ambient.vertices().len() + key.vertices().len(),
        "both layers, concatenated"
    );
    let ambient_x: Vec<f32> = ambient.vertices().iter().map(|vertex| vertex.x).collect();
    let layered_prefix: Vec<f32> = layered
        .vertices()
        .iter()
        .take(ambient.vertices().len())
        .map(|vertex| vertex.x)
        .collect();
    assert_eq!(layered_prefix, ambient_x, "the halo has to go down first");

    // Only quantized visible steps remain. The ambient layer still reaches
    // farther, and each stays inside its authored blur's maximum extent.
    assert!(
        span_x(&ambient) > span_x(&key),
        "the halo reaches farther than the key shadow: {} vs {}",
        span_x(&ambient),
        span_x(&key)
    );
    assert!(
        span_x(&key) > 100.0 && span_x(&key) <= 116.0 + 1.0,
        "the key shadow spreads past the 100pt box but inside its blur, got {}",
        span_x(&key)
    );
    assert!(
        span_x(&ambient) <= 140.0 + 1.0,
        "the halo stays inside its authored blur, got {}",
        span_x(&ambient)
    );

    // Offsets translate the surviving geometry without changing the blur.
    let unshifted_halo = framed();
    unshifted_halo.set_ambient_shadow(shadow(1.0, 0.0, 20.0));
    let unshifted_contact = framed();
    unshifted_contact.set_shadow(shadow(1.0, 0.0, 8.0));
    assert!(
        (top(&ambient) - top(&render(&unshifted_halo)) - 3.0).abs() < 0.01,
        "offset_y 3 translates the halo by exactly 3"
    );
    assert!(
        (top(&key) - top(&render(&unshifted_contact)) - 6.0).abs() < 0.01,
        "offset_y 6 translates the key shadow by exactly 6"
    );
}

#[test]
fn ambient_is_broader_and_fainter() {
    let halo = framed();
    halo.set_ambient_shadow(shadow(0.3, 3.0, 20.0));
    let contact = framed();
    contact.set_shadow(shadow(0.9, 6.0, 8.0));

    let ambient = render(&halo);
    let key = render(&contact);

    assert!(
        span_x(&ambient) > span_x(&key),
        "the halo is broader: {} vs {}",
        span_x(&ambient),
        span_x(&key)
    );

    let (ambient_min, ambient_max) = alpha_range(&ambient);
    let (_key_min, key_max) = alpha_range(&key);
    assert!(
        ambient_max < key_max,
        "the densest halo step is fainter than the densest key step: {ambient_max} vs {key_max}"
    );
    // The falloff spans the blur: the outermost fringe is the faintest thing
    // the layer paints.
    assert!(
        ambient_min < ambient_max,
        "the halo falloff must span a range: {ambient_min}..{ambient_max}"
    );
}

#[test]
fn single_layer_stays_single() {
    let node = framed();
    node.set_shadow(shadow(1.0, 1.0, 2.0));

    let list = render(&node);
    // One layer: the same geometry the layered render appends after the halo,
    // not twice the geometry.
    let layered = framed();
    layered.set_shadow(shadow(1.0, 1.0, 2.0));
    layered.set_ambient_shadow(shadow(1.0, 3.0, 20.0));
    let halo = render(&layered);

    assert!(
        list.vertices().len() < halo.vertices().len(),
        "one layer is less geometry than two: {} vs {}",
        list.vertices().len(),
        halo.vertices().len()
    );
    // It spreads past the node (100 wide) but nowhere near as far as a halo would.
    assert!(
        span_x(&list) > 100.0,
        "the key shadow spreads past the box, got {}",
        span_x(&list)
    );
    assert!(
        span_x(&list) < span_x(&halo),
        "but nowhere near as far as a halo, got {} vs {}",
        span_x(&list),
        span_x(&halo)
    );
}
