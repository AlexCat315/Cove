//! Per-vertex shaded disk and annulus fills.
//!
//! Behaviour checklist:
//! - every disk vertex carries the colour `shade` returns for its position,
//!   scaled by the analytic rim coverage
//! - a disk tessellates a fan whose vertices stay within the radius
//! - an annulus leaves the hole empty: no vertex sits inside the inner radius
use cove_core::{Color, DrawList, Point};

fn shade_at(x: f32, _y: f32, center: Point, radius: f32) -> Color {
    let t = ((x - center.x).abs() / radius).clamp(0.0, 1.0);
    Color {
        r: t,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    }
}

#[test]
fn a_shaded_disk_colors_each_vertex_by_position() {
    let mut list = DrawList::default();
    let center = Point { x: 50.0, y: 50.0 };
    let radius = 40.0;
    list.add_disk_shaded(center, radius, |x, y| shade_at(x, y, center, radius))
        .expect("a shaded disk fills");
    let vertices = list.vertices();
    assert!(vertices.len() > 32, "the disk tessellates a fan");
    for vertex in vertices {
        let coverage = (vertex.color >> 24) as f32 / 255.0;
        let expected = shade_at(vertex.x, vertex.y, center, radius).multiplied_alpha(coverage);
        assert_eq!(
            vertex.color,
            expected.rgba8(),
            "vertex ({}, {}) carries its shaded colour",
            vertex.x,
            vertex.y
        );
    }
}

#[test]
fn an_annulus_leaves_the_hole_empty() {
    let mut list = DrawList::default();
    let center = Point { x: 0.0, y: 0.0 };
    list.add_shaded_annulus(center, 10.0, 20.0, |_x, _y| Color::WHITE)
        .expect("a shaded annulus fills");
    let vertices = list.vertices();
    assert!(!vertices.is_empty());
    for vertex in vertices {
        let distance = (vertex.x * vertex.x + vertex.y * vertex.y).sqrt();
        assert!(
            (9.5..=20.5).contains(&distance),
            "annulus vertices stay between the rims, got {distance}"
        );
    }
}
