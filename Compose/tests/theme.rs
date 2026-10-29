//! Translation of Tests/CoveUIComposeTests/ThemeTests.swift.
use cove_core::scene::{CompositionLocal, Node};
use cove_core::Color;
use cove_ui_compose::theme::{Theme, TextStyleToken};
use std::rc::Rc;

#[test]
fn default_dark_is_stable() {
    let a = Theme::default_dark();
    let b = Theme::default_dark();
    assert_eq!(a.colors.accent, b.colors.accent);
    assert_eq!(a.spacing.md, b.spacing.md);
    assert_eq!(a.typography.body.font, b.typography.body.font);
}

#[test]
fn default_dark_slots_are_populated() {
    let c = Theme::default_dark().colors;
    for color in [c.background, c.surface, c.on_surface, c.accent, c.success, c.warning, c.error, c.info, c.border, c.divider] {
        assert!(color.a > 0.0);
    }
}

#[test]
fn scales_are_monotonic() {
    let s = Theme::default_dark().spacing;
    assert!(s.xs < s.sm);
    assert!(s.sm < s.md);
    assert!(s.md < s.lg);
    assert!(s.lg < s.xl);
    assert!(s.xl < s.xxl);

    let r = Theme::default_dark().radius;
    assert!(r.none < r.sm);
    assert!(r.sm < r.md);
    assert!(r.md < r.lg);
    assert!(r.lg < r.xl);
    assert!(r.xl < r.pill);
}

#[test]
fn typography_line_height_covers_font_size() {
    let t = Theme::default_dark().typography;
    let slots: [TextStyleToken; 8] = [t.display, t.title, t.headline, t.body, t.body_strong, t.caption, t.label, t.mono];
    for token in slots {
        assert!(token.line_height >= token.font.size());
    }
}

#[test]
fn theme_is_value_type() {
    let mut copy = Theme::default_dark();
    let original_accent = Theme::default_dark().colors.accent;
    copy.colors.accent = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
    assert_eq!(Theme::default_dark().colors.accent, original_accent);
    assert_ne!(copy.colors.accent, original_accent);
}

#[test]
fn theme_flows_through_composition_local() {
    let theme_key = Rc::new(CompositionLocal::new(Theme::default_dark));
    let node = Node::new();
    let mut custom = Theme::default_dark();
    custom.colors.accent = Color { r: 0.9, g: 0.2, b: 0.4, a: 1.0 };
    node.set_composition_value(&theme_key, custom);
    assert_eq!(node.composition_value(&theme_key).colors.accent, Color { r: 0.9, g: 0.2, b: 0.4, a: 1.0 });
}
