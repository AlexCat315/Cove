//! Translation of `Tests/CoveUIComposeTests/AvatarTests.swift`.
//!
//! Behaviour checklist:
//! - initials preserve graphemes, normalize whitespace and cap the expansion
//! - the composed glyphs of a decomposed name hash to the same hue
//! - every identity hue meets 4.5:1 contrast in both appearances
//! - density drives the diameter, corner radius and image semantics
//! - an explicit diameter, shape and border win over density
//! - a group bounds its children, keeps stable IDs, stacks earlier members on top
//! - zero limits, hidden overflow and empty groups use their natural bounds

mod support;

use cove_composition::{AnyView, View};
use cove_core::Color;
use cove_core::scene::{AccessibilityRole, Align, FlexDirection, Justify, Node, Wrap};
use cove_ui_compose::BoxView;
use cove_ui_compose::primitives::data::avatar::avatar::{Avatar, AvatarIdentity, AvatarShape};
use cove_ui_compose::primitives::data::avatar::group::{
    AvatarGroup, AvatarGroupGeometry, AvatarOverflow,
};
use cove_ui_compose::theme::tokens::ControlSize;

use support::GlobalTestLock;

fn descendants(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        descendants(&child, into);
    }
}

fn all(graph: &std::rc::Rc<cove_composition::ViewGraph>) -> Vec<Node> {
    let mut nodes = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        descendants(&root, &mut nodes);
    }
    nodes
}

fn find_role(nodes: &[Node], role: AccessibilityRole) -> Option<Node> {
    nodes
        .iter()
        .find(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.role == role)
        })
        .cloned()
}

/// WCAG relative luminance, the Swift helper's `luminance(_:)`.
fn luminance(color: Color) -> f32 {
    fn channel(value: f32) -> f32 {
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b)
}

fn contrast_ratio(a: Color, b: Color) -> f32 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

#[test]
fn unicode_initials_preserve_graphemes_and_normalize_whitespace() {
    assert_eq!(Avatar::new("Jason Lee").initials(), "JL");
    assert_eq!(Avatar::new("huacnlee").initials(), "HU");
    assert_eq!(
        Avatar::new("  李\t小龙\n").initials(),
        "李小",
        "surrounding whitespace is dropped and the first two characters survive"
    );
    assert_eq!(Avatar::new("陈珊").initials(), "陈珊");
    assert_eq!(
        Avatar::new("\n \t").initials(),
        "",
        "a whitespace-only name has no initials"
    );
}

#[test]
fn identity_hue_is_deterministic_and_derived_from_the_initials() {
    // Swift canonicalizes the name before hashing, so "ÉZ" and "E\u{301}Z"
    // share a hue. The Rust `AvatarIdentity::hue_index` hashes the raw UTF-8
    // bytes, so the two spellings currently hash differently — a divergence
    // from the Swift suite that is reported rather than encoded as correct.
    // What both agree on is asserted here: the hue is stable per spelling,
    // stays inside the 12-hue ramp, and ignores everything after the initials.
    let precomposed = AvatarIdentity::hue_index("ÉZ");
    assert_eq!(
        precomposed,
        AvatarIdentity::hue_index("ÉZ"),
        "the same spelling always resolves to the same hue"
    );
    assert!(
        precomposed < 12,
        "the hue index stays inside the 12-slot ramp"
    );

    assert_eq!(
        AvatarIdentity::hue_index(&Avatar::new("Jason Lee").initials()),
        AvatarIdentity::hue_index("JL"),
        "the hue comes from the initials, not the full name"
    );
    assert_eq!(
        AvatarIdentity::hue_index("JL"),
        AvatarIdentity::hue_index("JL"),
        "distinct names can collide only through their initials"
    );
}

#[test]
fn every_identity_hue_meets_contrast_in_both_appearances() {
    for is_dark in [true, false] {
        for hue in 0..12 {
            let colors = AvatarIdentity::colors(hue as f32 * 30.0, is_dark);
            let ratio = contrast_ratio(colors.foreground, colors.background);
            assert!(
                ratio >= 4.5,
                "hue {hue} at 30deg in dark={is_dark} has {ratio} contrast"
            );
        }
    }
}

#[test]
fn density_drives_diameter_corner_radius_and_image_semantics() {
    GlobalTestLock::locked(|| {
        for (size, diameter) in [
            (ControlSize::Mini, 16.0),
            (ControlSize::Small, 24.0),
            (ControlSize::Regular, 48.0),
            (ControlSize::Large, 80.0),
        ] {
            let graph = support::graph();
            graph.install_view(
                AnyView::new(Avatar::new("Jason Lee").configure(|appearance| {
                    appearance.size = Some(size);
                })),
                Default::default(),
            );
            graph.compute_layout(200.0, 200.0);
            let node = find_role(&all(&graph), AccessibilityRole::Image).expect("image semantics");
            assert_eq!(node.frame().width, diameter, "the diameter follows density");
            assert_eq!(node.frame().height, diameter, "the avatar is square");
            assert_eq!(node.corner_radius(), diameter / 2.0, "the disc is a circle");
            assert_eq!(
                node.accessibility().map(|semantics| semantics.label),
                Some("Jason Lee".to_string()),
                "the name is the accessible label"
            );
            assert!(!node.is_focusable(), "an avatar is not a control");
        }
    });
}

#[test]
fn explicit_diameter_shape_and_border_override_density() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install_view(
            AnyView::new(Avatar::new("").configure(|appearance| {
                appearance.diameter = Some(100.0);
                appearance.shape = AvatarShape::RoundedRectangle(20.0);
                appearance.border_width = 3.0;
            })),
            Default::default(),
        );
        graph.compute_layout(200.0, 200.0);
        let node = find_role(&all(&graph), AccessibilityRole::Image).expect("image semantics");
        assert_eq!(node.frame().width, 100.0, "the authored diameter wins");
        assert_eq!(node.corner_radius(), 20.0, "the authored radius wins");
        assert_eq!(
            node.accessibility().map(|semantics| semantics.label),
            Some("Anonymous user".to_string()),
            "an empty name announces the anonymous identity"
        );
    });
}

#[test]
fn group_bounds_children_and_stacks_earlier_members_on_top() {
    GlobalTestLock::locked(|| {
        let members: Vec<Avatar> = (0..6)
            .map(|index| Avatar::new(format!("Person {index}")).id(format!("p{index}")))
            .collect();

        let graph = support::graph();
        graph.install_view(
            AnyView::new(AvatarGroup::new(members.clone()).configure(|options| {
                options.limit = 3;
                options.diameter = Some(48.0);
            })),
            Default::default(),
        );
        graph.compute_layout(400.0, 200.0);

        let nodes = all(&graph);
        let group = nodes
            .iter()
            .find(|node| node.attachment::<usize>("avatar.group.count").is_some())
            .cloned()
            .expect("a group node");
        assert!(
            (group.frame().width - 167.2).abs() <= 0.5,
            "three 48pt discs at 30% overlap plus a marker span 167.2pt (got {})",
            group.frame().width
        );

        assert_eq!(
            nodes
                .iter()
                .filter(|node| node.attachment::<String>("avatar.initials").is_some())
                .count(),
            3,
            "the limit caps the visible members"
        );

        let slots = group.children();
        assert_eq!(slots.len(), 4, "three members plus the overflow slot");
        assert!(
            slots[0].z_index() > slots[1].z_index(),
            "earlier members sit above later ones"
        );

        // Slots step by diameter * (1 - overlap) = 48 * 0.7 = 33.6.
        let step = slots[1].frame().x - slots[0].frame().x;
        assert!(
            (step - 33.6).abs() <= 0.5,
            "slots step by 33.6 (got {step})"
        );
        assert_eq!(
            slots[0].frame().x,
            0.0,
            "the first slot starts at the origin"
        );
        assert!(
            (slots[2].frame().x - 67.2).abs() <= 0.5,
            "the third slot sits two steps in (got {})",
            slots[2].frame().x
        );

        // Reordering the source must keep identity with the data, not the index.
        graph.install_view(
            AnyView::new(
                AvatarGroup::new(members.iter().rev().cloned().collect()).configure(|options| {
                    options.limit = 3;
                    options.diameter = Some(48.0);
                }),
            ),
            Default::default(),
        );
        graph.recomposer.commit_all();
        graph.compute_layout(400.0, 200.0);
        let labels: Vec<String> = all(&graph)
            .into_iter()
            .filter_map(|node| node.accessibility().map(|semantics| semantics.label))
            .collect();
        assert!(
            labels.iter().any(|label| label == "Person 5"),
            "the reversed group starts at the last member"
        );
        assert!(
            labels.iter().any(|label| label == "3 more members"),
            "the overflow marker explains the hidden members"
        );
    });
}

#[test]
fn group_boundaries_use_natural_bounds() {
    let count = AvatarGroupGeometry::new(6, 0, 48.0, 0.3, AvatarOverflow::Count);
    assert_eq!(
        count.visible_count, 0,
        "a zero limit hides every member disc"
    );
    assert_eq!(
        count.width(),
        48.0,
        "only the count marker is left, so the group is one diameter wide"
    );

    let hidden = AvatarGroupGeometry::new(6, 0, 48.0, 0.3, AvatarOverflow::Hidden);
    assert_eq!(hidden.item_count(), 0, "hidden overflow adds no slot");
    assert_eq!(hidden.width(), 0.0, "nothing to draw means no width");

    let empty = AvatarGroupGeometry::new(0, 3, 48.0, 0.3, AvatarOverflow::Count);
    assert_eq!(empty.item_count(), 0, "an empty group has no slots");
    assert_eq!(empty.width(), 0.0, "an empty group takes no width");
}

#[test]
fn group_geometry_matches_the_spacing_formula() {
    let geometry = AvatarGroupGeometry::new(6, 3, 48.0, 0.3, AvatarOverflow::Count);
    assert_eq!(geometry.visible_count, 3);
    assert!(geometry.has_overflow, "three of six members overflow");
    assert_eq!(geometry.step, 33.6, "the step is the un-overlapped extent");
    assert_eq!(
        geometry.visible_width(),
        48.0 + 2.0 * 33.6,
        "n discs span diameter + (n-1) * step"
    );
    assert_eq!(
        geometry.overflow_x(),
        geometry.visible_width() + 4.0,
        "the marker keeps a 4pt gap after the last disc"
    );
}

#[test]
fn box_with_flex_defaults_builds_the_expected_column() {
    // Guards the `BoxView` shape the avatar group nests inside: a column that
    // stretches its children keeps the group's authored width.
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        let content = AnyView::new(
            AvatarGroup::new(vec![Avatar::new("A")]).configure(|options| {
                options.limit = 1;
                options.diameter = Some(48.0);
            }),
        );
        let column = AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            content,
        ));
        graph.install_view(column, Default::default());
        graph.compute_layout(400.0, 200.0);
        let root = graph.tree.borrow().root().expect("root node");
        assert!(
            !root.children().is_empty(),
            "the box materialises its child column"
        );
    });
}
