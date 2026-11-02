//! Translation of `Tests/CoveUIRuntimeTests/SurfaceRenderingTests.swift`.
//!
//! Two contracts that only show up on a real surface: a transparent rounded
//! border must stroke the outline without filling the control's interior, and a
//! monospaced font must give every code character the same advance.
//!
//! Behaviour checklist:
//! - transparent rounded borders do not fill the control interior, in both the
//!   direct and the layer-cached renderer
//! - monospaced fonts give code characters equal advances

use cove_core::scene::{DrawList, LayerAwareNodeRenderer, Node, NodeRenderer, RenderTree};
use cove_core::{Color, Font, FontWeight, Rect};
use cove_text::{FontAtlas, SystemFontDefaults, TextFontResolver, Vertex};

fn node(x: f32, y: f32, width: f32, height: f32) -> Node {
    let node = Node::new();
    node.set_frame(Rect::new(x, y, width, height));
    node
}

fn contains(x: f32, y: f32, list: &DrawList) -> bool {
    let (triangles, _) = list.indices().as_chunks::<3>();
    for triangle in triangles {
        let a = list.vertices()[triangle[0] as usize];
        let b = list.vertices()[triangle[1] as usize];
        let c = list.vertices()[triangle[2] as usize];
        fn cross(p: Vertex, q: Vertex, px: f32, py: f32) -> f32 {
            (q.x - p.x) * (py - p.y) - (q.y - p.y) * (px - p.x)
        }
        if cross(a, b, c.x, c.y).abs() <= 0.00001 {
            continue;
        }
        let edges = [cross(a, b, x, y), cross(b, c, x, y), cross(c, a, x, y)];
        if edges.iter().all(|edge| *edge >= 0.0) || edges.iter().all(|edge| *edge <= 0.0) {
            return true;
        }
    }
    false
}

#[test]
fn transparent_rounded_borders_do_not_fill_the_control_interior() {
    let root = node(0.0, 0.0, 100.0, 40.0);
    root.set_border_color(Some(Color::WHITE));
    root.set_border_width(1.0);
    root.set_corner_radius(6.0);

    let mut reference = DrawList::default();
    NodeRenderer::new().render(&root, &mut reference);

    let tree = RenderTree::new();
    tree.install(&root);
    let mut cached = DrawList::default();
    LayerAwareNodeRenderer::new().render(&tree, &mut cached);

    for list in [&reference, &cached] {
        assert!(
            !contains(50.0, 20.0, list),
            "the border must not paint the interior",
        );
        assert!(
            contains(0.5, 20.0, list),
            "the left edge lies inside the stroke",
        );
        assert!(!contains(0.2, 0.2, list), "the corner is cut by the radius",);
    }
    assert_eq!(
        reference.indices(),
        cached.indices(),
        "the cached render matches the direct one",
    );
}

#[test]
fn monospaced_fonts_give_code_characters_equal_advances() {
    let atlas = FontAtlas::new(1024, 1024);
    let resolver = TextFontResolver::new(SystemFontDefaults::primary_font_name(), atlas, 1.0);
    let monospaced = Font::monospaced(14.0, FontWeight::Regular);
    let glyphs = resolver.shape("ilW0", &monospaced);
    assert_eq!(glyphs.len(), 4, "all four code characters shape");

    let advance = glyphs[0].x_advance;
    assert!(
        glyphs
            .iter()
            .all(|glyph| (glyph.x_advance - advance).abs() < 0.01),
        "monospaced advances were {:?}",
        glyphs.iter().map(|g| g.x_advance).collect::<Vec<_>>(),
    );
    assert_ne!(
        monospaced,
        Font::system(14.0, FontWeight::Regular),
        "the monospaced design is a different font",
    );
}
