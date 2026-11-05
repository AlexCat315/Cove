//! Translation of `Tests/CoveUIComposeTests/ThemeTests.swift`.
//!
//! Behaviour checklist:
//! - `default_dark()` is a stable value across calls
//! - every structural color slot is non-clear
//! - the spacing and radius scales are strictly monotonic
//! - every typography token's line height covers its font size
//! - `Theme` is a value type: mutating a copy leaves the default untouched
//! - a theme flows through a `CompositionLocal`

use crate::support;

use cove_core::Color;
use cove_core::scene::{CompositionLocal, Node};
use cove_ui_compose::theme::Theme;

use support::GlobalTestLock;

#[test]
fn default_dark_is_stable() {
    let a = Theme::default_dark();
    let b = Theme::default_dark();
    assert_eq!(a.colors.accent, b.colors.accent, "accent is deterministic");
    assert_eq!(a.spacing.md, b.spacing.md, "spacing is deterministic");
    assert_eq!(
        a.typography.body.font.size(),
        b.typography.body.font.size(),
        "the body font size is deterministic"
    );
    assert_eq!(
        a.typography.body.font.weight(),
        b.typography.body.font.weight(),
        "the body font weight is deterministic"
    );
}

#[test]
fn default_dark_slots_are_populated() {
    let colors = Theme::default_dark().colors;
    for (name, color) in [
        ("background", colors.background),
        ("surface", colors.surface),
        ("on_surface", colors.on_surface),
        ("accent", colors.accent),
        ("success", colors.success),
        ("warning", colors.warning),
        ("error", colors.error),
        ("info", colors.info),
        ("border", colors.border),
        ("divider", colors.divider),
    ] {
        assert!(color.a > 0.0, "{name} must not be fully transparent");
    }
}

#[test]
fn spacing_and_radius_scales_are_monotonic() {
    let spacing = Theme::default_dark().spacing;
    assert!(spacing.xs < spacing.sm, "xs < sm");
    assert!(spacing.sm < spacing.md, "sm < md");
    assert!(spacing.md < spacing.lg, "md < lg");
    assert!(spacing.lg < spacing.xl, "lg < xl");
    assert!(spacing.xl < spacing.xxl, "xl < xxl");

    let radius = Theme::default_dark().radius;
    assert!(radius.none < radius.sm, "none < sm");
    assert!(radius.sm < radius.md, "sm < md");
    assert!(radius.md < radius.lg, "md < lg");
    assert!(radius.lg < radius.xl, "lg < xl");
    assert!(radius.xl < radius.pill, "xl < pill");
}

#[test]
fn spacing_scale_matches_the_authored_steps() {
    let spacing = Theme::default_dark().spacing;
    assert_eq!(
        [
            spacing.xs,
            spacing.sm,
            spacing.md,
            spacing.lg,
            spacing.xl,
            spacing.xxl
        ],
        [4.0, 6.0, 8.0, 12.0, 16.0, 24.0],
        "the authored 4/6/8/12/16/24 ramp"
    );
    let radius = Theme::default_dark().radius;
    assert_eq!(
        [radius.none, radius.sm, radius.md, radius.lg, radius.xl],
        [0.0, 4.0, 6.0, 8.0, 10.0],
        "the reference 0/4/6/8/10 ramp"
    );
    assert!(radius.pill >= 9999.0, "pill is the fully rounded sentinel");
}

#[test]
fn typography_line_height_covers_font_size() {
    let typography = Theme::default_dark().typography;
    for (name, token) in [
        ("display", &typography.display),
        ("title", &typography.title),
        ("headline", &typography.headline),
        ("body", &typography.body),
        ("body_strong", &typography.body_strong),
        ("caption", &typography.caption),
        ("label", &typography.label),
        ("mono", &typography.mono),
    ] {
        assert!(
            token.line_height >= token.font.size(),
            "{name} line height {} must cover font size {}",
            token.line_height,
            token.font.size()
        );
    }
}

#[test]
fn theme_is_a_value_type() {
    let original_accent = Theme::default_dark().colors.accent;

    let mut copy = Theme::default_dark();
    copy.colors.accent = Color {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    assert_eq!(
        Theme::default_dark().colors.accent,
        original_accent,
        "mutating a copy leaves the shared default untouched"
    );
    assert_ne!(
        copy.colors.accent, original_accent,
        "the copy carries the new accent"
    );
}

#[test]
fn theme_flows_through_a_composition_local() {
    GlobalTestLock::locked(|| {
        let theme_key = CompositionLocal::new(Theme::default_dark);
        let node = Node::new();

        let mut custom = Theme::default_dark();
        custom.colors.accent = Color {
            r: 0.9,
            g: 0.2,
            b: 0.4,
            a: 1.0,
        };
        node.set_composition_value(&theme_key, custom);

        assert_eq!(
            node.composition_value(&theme_key).colors.accent,
            Color {
                r: 0.9,
                g: 0.2,
                b: 0.4,
                a: 1.0
            },
            "the provided theme is what consumers read back"
        );
    });
}

#[test]
fn composition_local_defaults_to_the_dark_preset() {
    let theme_key = CompositionLocal::new(Theme::default_dark);
    let node = Node::new();
    assert_eq!(
        node.composition_value(&theme_key).colors.accent,
        Theme::default_dark().colors.accent,
        "without a provider the declaration's default applies"
    );
}

#[test]
fn light_and_dark_presets_differ() {
    let light = Theme::default_light();
    let dark = Theme::default_dark();
    assert_ne!(
        light.colors.background, dark.colors.background,
        "the two presets paint different backgrounds"
    );
    let sum = |color: Color| color.r + color.g + color.b;
    assert!(
        sum(light.colors.background) > sum(dark.colors.background),
        "the light preset has the brighter background"
    );
}

#[test]
fn desktop_presets_keep_text_and_selected_text_readable() {
    fn luminance(color: Color) -> f32 {
        fn linear(value: f32) -> f32 {
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
    }
    fn contrast(a: Color, b: Color) -> f32 {
        let a = luminance(a);
        let b = luminance(b);
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }
    for theme in [Theme::default_light(), Theme::default_dark()] {
        let colors = theme.colors;
        assert!(contrast(colors.on_accent, colors.accent) >= 4.5);
        for background in [colors.background, colors.surface_sunken] {
            assert!(contrast(colors.on_surface, background) >= 4.5);
            assert!(contrast(colors.on_surface, colors.selection.composited(background)) >= 4.5);
        }
        assert_eq!(theme.inputs.background, colors.surface_sunken);
        assert!(theme.inputs.border_width <= 1.0 && theme.inputs.focus_ring_width <= 1.0);
    }
}
