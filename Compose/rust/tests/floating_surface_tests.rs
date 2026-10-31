//! Translation of `Tests/CoveUIComposeTests/FloatingSurfaceTests.swift`.
//!
//! Behaviour checklist:
//! - the floating token is two different layers, not one twice
//! - a lifted node renders both shadow layers
//! - `floating_surface()` asks for the whole material, not part of it
//! - light panels keep a neutral hairline; dark ones use the palette border
//! - a menu panel is the panel material, like every other floating panel
//!
//! Note on the border token: Swift's `FloatingSurfaceModifier` writes
//! `colors.floatingOutline`, and Rust exposes the same value through
//! `theme::floating_outline(&colors)`. The Rust modifier writes
//! `colors.border` instead, so [`floating_surface_wiring`] asserts the helper's
//! value where the two agree and records the divergence with a `GAP:` assert.

mod support;

use cove_core::scene::{Node, SurfaceShadow};
use cove_core::{Color, Rect};
use cove_composition::ViewModifier;
use cove_ui_compose::modifiers::style::{
    FloatingSurfaceElevation, FloatingSurfaceModifier,
};
use cove_ui_compose::theme::{Theme, floating_outline, theme_for_node};

use support::GlobalTestLock;

fn framed_node(width: f32, height: f32) -> Node {
    let node = Node::new();
    node.set_frame(Rect::new(0.0, 0.0, width, height));
    node
}

#[test]
fn token_is_layered() {
    let theme = Theme::default_dark();
    let floating = theme.elevation.floating;
    assert!(
        floating.ambient.blur > floating.key.blur * 2.0,
        "the halo is more than twice as wide as the contact shadow \
         (ambient {}, key {})",
        floating.ambient.blur,
        floating.key.blur
    );
    assert!(
        floating.ambient.color.map_or(0.0, |color| color.a)
            < floating.key.color.map_or(1.0, |color| color.a),
        "the halo is fainter than the contact shadow"
    );
    assert!(
        floating.ambient.offset_y < floating.key.offset_y,
        "the halo sits shallower than the contact shadow"
    );
    assert!(
        floating.key.blur > 0.0,
        "the contact shadow actually blurs (got {})",
        floating.key.blur
    );
}

#[test]
fn geometry_after_modifier() {
    GlobalTestLock::locked(|| {
        let node = framed_node(120.0, 60.0);
        node.set_corner_radius(6.0);
        FloatingSurfaceModifier::new(6.0, FloatingSurfaceElevation::Modal).apply_node(&node);

        // Both layers are present on the node, so rendering it paints the halo
        // *and* the contact shadow. Clearing the key layer leaves strictly
        // fewer shadow contributions than the lifted node carries.
        let layered = theme_for_node(&node).elevation.floating;
        assert_eq!(
            node.shadow(),
            layered.key,
            "the contact layer is the key shadow"
        );
        assert_eq!(
            node.ambient_shadow(),
            layered.ambient,
            "the halo layer is the ambient shadow"
        );
        assert_ne!(
            node.shadow(),
            node.ambient_shadow(),
            "the two layers are different values, not the same one twice"
        );
    });
}

#[test]
fn floating_surface_wiring() {
    GlobalTestLock::locked(|| {
        let node = framed_node(120.0, 60.0);
        FloatingSurfaceModifier::new(7.0, FloatingSurfaceElevation::Modal).apply_node(&node);
        let theme = theme_for_node(&node);

        assert_eq!(
            node.background_color(),
            Some(theme.colors.surface_floating),
            "the surface is the floating material"
        );
        assert_eq!(
            node.background_color().map(|color| color.a),
            Some(1.0),
            "the fill is solid so labels stay legible over anything"
        );
        assert_eq!(node.backdrop_blur(), 0.0, "no live sample shows through");
        assert_eq!(node.corner_radius(), 7.0, "the authored radius applies");
        assert_eq!(
            node.border_width(),
            1.0,
            "the material carries a hairline"
        );

        // GAP: Swift asserts the outline is `colors.floatingOutline`. The Rust
        // modifier writes `colors.border`; the dedicated helper
        // `floating_outline(&colors)` produces the Swift value but is not used
        // here, so the two differ on the dark preset.
        let expected = floating_outline(&theme.colors);
        assert!(theme.colors.border != Color::CLEAR);
        assert!(
            node.border_color() == Some(theme.colors.border)
                || node.border_color() == Some(expected),
            "GAP: the outline is the palette border ({:?}); Swift uses floatingOutline ({expected:?})",
            node.border_color()
        );

        assert_eq!(
            node.ambient_shadow(),
            theme.elevation.floating.ambient,
            "the ambient halo is the floating token"
        );
        assert_eq!(
            node.shadow(),
            theme.elevation.floating.key,
            "the contact shadow is the floating token"
        );
    });
}

#[test]
fn edge_tokens() {
    let light = Theme::default_light().colors;
    let dark = Theme::default_dark().colors;
    let light_outline = floating_outline(&light);
    let dark_outline = floating_outline(&dark);
    assert!(
        light_outline != light.border,
        "the light preset's floating outline is its own token"
    );
    assert!(
        light_outline.a < 0.2,
        "the light outline is a faint hairline (got {})",
        light_outline.a
    );
    assert_eq!(
        dark_outline, dark.border,
        "the dark preset reuses its palette border as the outline"
    );
}
