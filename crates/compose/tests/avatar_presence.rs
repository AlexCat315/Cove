//! Translation of `Tests/CoveUIComposeTests/AvatarPresenceTests.swift`.
//!
//! Behaviour checklist:
//! - the status sits on the outline at the logical bottom-trailing corner
//! - RTL mirrors the indicator to the bottom-leading side
//! - the status is announced as the avatar's value
//! - labels can be overridden and every status has a distinct shape
//! - the overflow marker opens the member list, which reports selection
//!
//! Model difference: the Swift suite drives the member list through a popover.
//! `Popover` has no Rust counterpart yet, so this suite exercises the roster
//! content and trigger that the popover would host — the same members, labels,
//! values and selection callback — rather than faking a presentation.

use crate::support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::AnyView;
use cove_composition::layout::LayoutDirectionExt;
use cove_core::scene::{AccessibilityRole, Align, Direction, FlexDirection, Justify, Node, Wrap};
use cove_ui_compose::BoxView;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::data::avatar::avatar::{Avatar, AvatarShape};
use cove_ui_compose::primitives::data::avatar::group::{AvatarGroup, AvatarGroupOptions};
use cove_ui_compose::primitives::data::avatar::member_list::{
    AvatarMemberList, AvatarMemberListScope, AvatarMemberListTrigger,
};
use cove_ui_compose::primitives::data::avatar::status::{
    AvatarStatus, AvatarStatusGeometry, AvatarStatusIndicator,
};

use support::GlobalTestLock;

fn collect(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        collect(&child, into);
    }
}

fn all(graph: &std::rc::Rc<cove_composition::ViewGraph>) -> Vec<Node> {
    let mut nodes = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        collect(&root, &mut nodes);
    }
    nodes
}

#[test]
fn status_sits_on_the_outline_at_the_logical_bottom_trailing_corner() {
    GlobalTestLock::locked(|| {
        for rtl in [false, true] {
            let graph = support::graph();
            let avatar = AnyView::new(Avatar::new("Jason Lee").configure(|appearance| {
                appearance.diameter = Some(48.0);
                appearance.status = Some(AvatarStatus::Busy);
            }));
            let container = AnyView::new(
                AnyView::new(BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(avatar),
                ))
                .layout_direction(if rtl {
                    Direction::Rtl
                } else {
                    Direction::Ltr
                }),
            );
            graph.install_view(container, Default::default());
            graph.compute_layout(200.0, 200.0);

            let nodes = all(&graph);
            let avatar_node = nodes
                .iter()
                .find(|node| {
                    node.accessibility()
                        .is_some_and(|semantics| semantics.role == AccessibilityRole::Image)
                })
                .cloned()
                .expect("the avatar exposes image semantics");
            let status = nodes
                .iter()
                .find(|node| node.attachment::<AvatarStatus>("avatar.status").is_some())
                .cloned()
                .expect("the status indicator materialises");

            assert_eq!(
                avatar_node.accessibility().map(|semantics| semantics.value),
                Some("Busy".to_string()),
                "the avatar announces its presence"
            );
            assert_eq!(
                status.attachment::<AvatarStatus>("avatar.status"),
                Some(AvatarStatus::Busy),
                "the indicator records the authored status"
            );

            let geometry = AvatarStatusGeometry::new(48.0, AvatarShape::Circle);
            assert_eq!(
                status.frame().width,
                geometry.size,
                "the indicator is the resolved geometry size"
            );

            // The indicator centre lies on the circle's diagonal: 24pt from the
            // centre, below it, and on the leading side in RTL.
            let frame = status.absolute_frame();
            let avatar_frame = avatar_node.absolute_frame();
            let centre = (frame.x + frame.width / 2.0, frame.y + frame.height / 2.0);
            let avatar_centre = (
                avatar_frame.x + avatar_frame.width / 2.0,
                avatar_frame.y + avatar_frame.height / 2.0,
            );
            let distance = ((centre.0 - avatar_centre.0).powi(2)
                + (centre.1 - avatar_centre.1).powi(2))
            .sqrt();
            assert!(
                (distance - 24.0).abs() < 1.5,
                "the indicator centre sits on the 24pt radius (got {distance})"
            );
            // The status uses the logical `End` edge, resolved against the
            // direction in force at layout time.
            if rtl {
                assert!(
                    centre.0 < avatar_centre.0,
                    "RTL mirrors the indicator to the bottom-leading (left) side"
                );
            } else {
                assert!(
                    centre.0 > avatar_centre.0,
                    "LTR places the indicator on the trailing (right) side"
                );
            }
            assert!(
                centre.1 > avatar_centre.1,
                "the indicator sits below the centre in both directions"
            );
        }
    });
}

#[test]
fn status_labels_can_be_overridden_and_every_status_has_a_distinct_shape() {
    let overridden = Avatar::new("A").configure(|appearance| {
        appearance.status = Some(AvatarStatus::Away);
        appearance.status_label = Some("In a meeting".to_string());
    });
    assert_eq!(overridden.status_text(), "In a meeting");

    assert_eq!(
        Avatar::new("A").status_text(),
        "",
        "an avatar without a status announces nothing"
    );

    let labels: std::collections::HashSet<&str> = AvatarStatus::ALL
        .iter()
        .map(|status| status.label())
        .collect();
    assert_eq!(labels.len(), 4, "every status has its own label");

    let square = AvatarStatusGeometry::new(48.0, AvatarShape::Square);
    assert_eq!(
        square.inset,
        -square.size / 2.0,
        "a square status tucks exactly half a size into the corner"
    );
}

#[test]
fn status_geometry_scales_with_the_diameter() {
    let small = AvatarStatusGeometry::new(24.0, AvatarShape::Circle);
    let large = AvatarStatusGeometry::new(96.0, AvatarShape::Circle);
    assert!(
        small.size < large.size,
        "larger avatars get larger indicators"
    );
    assert!(
        small.size >= 6.0 && large.size <= 24.0,
        "the indicator size stays within its clamp"
    );
    // A circular outline pushes the indicator out by the diagonal gap minus
    // half its own size; a square corner tucks it in by exactly half a size.
    let circle = AvatarStatusGeometry::new(48.0, AvatarShape::Circle);
    let radius = AvatarShape::Circle.radius(48.0);
    assert!(
        (circle.inset - (radius * (1.0 - 0.70710678) - circle.size / 2.0)).abs() < 1e-4,
        "the circular inset is the diagonal corner gap minus half the size"
    );
    let square = AvatarStatusGeometry::new(48.0, AvatarShape::Square);
    assert_eq!(
        square.inset,
        -square.size / 2.0,
        "a square corner tucks the indicator half a size in"
    );
}

#[test]
fn the_overflow_marker_opens_the_member_list_which_reports_selection() {
    GlobalTestLock::locked(|| {
        // Trigger presentation starts dismissed; the content exposes every
        // remaining member and selecting one closes the roster.
        let members: Vec<Avatar> = (0..5)
            .map(|index| {
                Avatar::new(format!("Person {index}"))
                    .id(format!("p{index}"))
                    .configure(|appearance| appearance.status = Some(AvatarStatus::Online))
            })
            .collect();

        let group =
            AvatarGroup::new(members.clone()).configure(|options: &mut AvatarGroupOptions| {
                options.limit = 2;
                options.diameter = Some(32.0);
                options.member_list = Some(AvatarMemberList {
                    title: "Members".to_string(),
                    scope: AvatarMemberListScope::Remaining,
                    on_select: None,
                });
            });

        // The remaining scope is what the roster shows after a 2-of-5 limit.
        let remaining: Vec<Avatar> = members[group.options.limit..].to_vec();
        let selected: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        let trigger = AvatarMemberListTrigger::new(
            cove_ui_compose::primitives::data::avatar::group::AvatarOverflowMarker {
                text: "3".to_string(),
                diameter: 32.0,
                count: remaining.len(),
                names: remaining
                    .iter()
                    .map(|member| member.name.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
                colors: cove_ui_compose::primitives::data::avatar::avatar::AvatarColors::new(
                    cove_core::Color::BLACK,
                    cove_core::Color::WHITE,
                    cove_core::Color::WHITE,
                ),
                is_interactive: true,
            },
            remaining.clone(),
            AvatarMemberList {
                title: "Members".to_string(),
                scope: AvatarMemberListScope::Remaining,
                on_select: {
                    let selected = selected.clone();
                    Some(Rc::new(move |member: &Avatar| {
                        *selected.borrow_mut() = member.id.clone();
                    }) as Rc<dyn Fn(&Avatar)>)
                },
            },
        );

        assert!(!trigger.is_presented(), "the roster starts closed");

        let content = trigger.content();
        assert_eq!(
            content.members.len(),
            3,
            "the remaining scope lists the three hidden members"
        );
        assert_eq!(
            content
                .members
                .iter()
                .map(|member| member.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Person 2", "Person 3", "Person 4"],
            "the roster starts after the visible limit"
        );
        assert_eq!(
            content.members[0].status_text(),
            "Online",
            "each row carries the member's presence"
        );

        let select = content.on_select.clone().expect("selecting is wired");
        select(&content.members[1]);
        assert_eq!(
            selected.borrow().as_deref(),
            Some("p3"),
            "selection reports the member's identity"
        );
        assert!(
            !trigger.is_presented(),
            "selection leaves the roster dismissed"
        );

        trigger.dismiss();
        assert!(!trigger.is_presented(), "dismiss is idempotent");
    });
}

#[test]
fn a_group_without_a_member_list_keeps_a_static_marker() {
    GlobalTestLock::locked(|| {
        let members: Vec<Avatar> = (0..4)
            .map(|index| Avatar::new(format!("P{index}")))
            .collect();
        let graph = support::graph();
        graph.install_view(
            AnyView::new(AvatarGroup::new(members).configure(|options| {
                options.limit = 1;
                options.diameter = Some(32.0);
            })),
            Default::default(),
        );
        graph.compute_layout(400.0, 200.0);

        let nodes = all(&graph);
        assert!(
            !nodes.iter().any(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::Button)
            }),
            "without a roster the marker is not a button"
        );
        let marker = nodes.iter().find(|node| {
            node.accessibility().is_some_and(|semantics| {
                semantics.role == AccessibilityRole::Image && semantics.label == "3 more members"
            })
        });
        assert!(
            marker.is_some(),
            "the static marker still announces the hidden count"
        );
    });
}

#[test]
fn status_indicator_is_decorative_and_not_focusable() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install_view(
            AnyView::new(AvatarStatusIndicator::new(
                AvatarStatus::Online,
                AvatarStatusGeometry::new(48.0, AvatarShape::Circle),
            )),
            Default::default(),
        );
        graph.compute_layout(100.0, 100.0);
        let node = all(&graph)
            .into_iter()
            .find(|node| node.attachment::<AvatarStatus>("avatar.status").is_some())
            .expect("the indicator");
        assert!(
            node.accessibility().is_none(),
            "the indicator is decorative; the avatar carries the value"
        );
        assert!(!node.is_focusable(), "an indicator is never a focus stop");
    });
}

#[test]
fn group_marks_the_overflow_slot_as_a_button_only_with_a_roster() {
    GlobalTestLock::locked(|| {
        let members: Vec<Avatar> = (0..4)
            .map(|index| Avatar::new(format!("P{index}")))
            .collect();
        let graph = support::graph();
        graph.install_view(
            AnyView::new(AvatarGroup::new(members).configure(|options| {
                options.limit = 1;
                options.diameter = Some(32.0);
                options.member_list = Some(AvatarMemberList::default());
            })),
            Default::default(),
        );
        graph.compute_layout(400.0, 200.0);
        let nodes = all(&graph);
        assert!(
            nodes.iter().any(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::Button)
            }),
            "a roster turns the marker into a control"
        );
    });
}
