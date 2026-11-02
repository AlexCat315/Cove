//! Regression guard for the "primitive view without `kind()`" mount panic.
//!
//! A type that implements `PrimitiveView` but leaves `View::kind()` at its
//! default is classified as a `Component`. `ViewGraph` then installs a scope and
//! calls the default `View::body()`, which panics with
//! "this view is materialized directly through ViewGraph". This bit us nine
//! times — `TabViewBody`, `TabStripItem`, `RovingFocusHost`, `BreadcrumbBody`,
//! `Matched`, `SidebarDestination`, `RailDestinationLabel`, `SidebarRowBody`,
//! `AdaptiveShell` and `LazyGridCell` — so it is worth a permanent guard.
//!
//! Every widget below must simply mount. A panic is the failure.

use crate::support;

use cove_composition::AnyView;
use cove_core::Binding;
use cove_core::scene::AccessibilityRole;
use cove_ui_compose::primitives::layout::sidebar::{
    Sidebar, SidebarItem, SidebarPresentation, SidebarSection,
};
use cove_ui_compose::primitives::layout::{TabItem, TabView};
use cove_ui_compose::primitives::overlays::RovingFocusHost;

use support::InteractiveScene;

#[test]
fn sidebar_mounts_without_a_primitive_view_panic() {
    InteractiveScene::run(400.0, 400.0, |scene| {
        let selection = Binding::constant(Some(1_i32));
        let mut sidebar = Sidebar::try_new(
            selection,
            vec![SidebarSection::new(
                "Group",
                vec![SidebarItem::new(1_i32, "One"), SidebarItem::new(2, "Two")],
            )],
        )
        .expect("unique sidebar IDs");
        sidebar.presentation = SidebarPresentation::Expanded;
        scene.install(sidebar);
        assert!(
            scene.nodes().len() > 1,
            "a Sidebar must materialise its destination rows"
        );
    });
}

#[test]
fn tab_view_mounts_without_a_primitive_view_panic() {
    InteractiveScene::run(400.0, 300.0, |scene| {
        scene.install(
            TabView::try_new(
                Binding::constant(0_u8),
                vec![
                    TabItem::new(0_u8, "A", cove_composition::EmptyView),
                    TabItem::new(1_u8, "B", cove_composition::EmptyView),
                ],
            )
            .expect("unique tab IDs"),
        );
        assert!(
            scene.nodes().len() > 1,
            "a TabView must materialise its strip"
        );
    });
}

#[test]
fn roving_focus_host_mounts() {
    InteractiveScene::run(300.0, 200.0, |scene| {
        scene.install(RovingFocusHost::new(
            vec![0_u8, 1],
            || None,
            |_value| {},
            |value| *value as u64,
            AnyView::new(cove_composition::EmptyView),
        ));
        assert!(scene.nodes().len() > 1);
    });
}

#[test]
fn sidebar_rows_publish_navigation_semantics() {
    InteractiveScene::run(400.0, 400.0, |scene| {
        let selection = Binding::constant(Some(1_i32));
        let mut sidebar = Sidebar::try_new(
            selection,
            vec![SidebarSection::new(
                "Group",
                vec![
                    SidebarItem::new(1_i32, "Alpha"),
                    SidebarItem::new(2, "Beta"),
                ],
            )],
        )
        .expect("unique sidebar IDs");
        sidebar.presentation = SidebarPresentation::Expanded;
        scene.install(sidebar);

        let rows = scene.all(|node| {
            node.accessibility()
                .is_some_and(|semantics| !semantics.label.is_empty())
        });
        assert!(
            !rows.is_empty(),
            "destination rows must publish accessible labels"
        );
        assert!(
            rows.iter().any(|node| {
                node.accessibility().is_some_and(|semantics| {
                    matches!(
                        semantics.role,
                        AccessibilityRole::Button
                            | AccessibilityRole::Link
                            | AccessibilityRole::ListItem
                    )
                })
            }),
            "a destination row is an actionable navigation target"
        );
    });
}
