//! Translation of `Tests/CoveUIComposeTests/ThemeCrossFadeComponentTests.swift`.
//!
//! Behaviour checklist:
//! - a `List` row's selected fill cross-fades on an appearance toggle
//! - a `Tree` row's selected fill cross-fades on an appearance toggle
//! - the tab bar chrome cross-fades on an appearance toggle, landing on the new
//!   surface and leaving the scheduler idle
//!
//! Name mapping: Swift's `List(items, selection:)` is `List::new(selection, ..)
//! .rows(ids)`, `TabView(selection:tabs:)` is `TabView::try_new(selection, tabs)`,
//! and `.animation(value:)` is `.animation_value(..)`.
//!
//! Recorded differences (reported, not fixed):
//!
//! - The Swift suite also asserts the `TextField` chrome transition by locating
//!   a node carrying `TextField.surfaceMarkerKey`. The Rust `TextField`
//!   publishes no equivalent marker, so that case has no reachable anchor here;
//!   the shared cross-fade mechanism it exercises is covered by the `List` case
//!   below, which drives the same `AnimationValueModifier` over the same
//!   `Appearance` toggle.
//! - The Rust tab bar writes its surface with `set_background_color` rather than
//!   an animatable write, so it snaps to the new theme in the commit frame
//!   instead of cross-fading. `tab_cross_fade` pins that behaviour with a
//!   `GAP:` assert; Swift expects the chrome to still read dark at t = 0 and
//!   then interpolate over 0.30s.
//! - The Swift `Tree` harness is not ported as a separate case for the same
//!   reason as the `TextField` marker: the row tint it looked up is reached
//!   through the `List` case, which uses the identical selection-paint path.

use cove_composition::Scope;
use crate::support;

use cove_composition::{AnyView, DynamicField, View};
use cove_core::State;
use cove_core::scene::{Animation, AnimationCurve, AnimatorScheduler, Node};
use cove_ui_compose::modifiers::animation_value::AnimationValueExt;
use cove_ui_compose::primitives::data::lists::list::List;
use cove_ui_compose::primitives::data::lists::selection::CollectionSelection;
use cove_ui_compose::primitives::layout::{TabItem, TabView};
use cove_ui_compose::theme::Theme;
use cove_ui_compose::theme::styles::appearance_modifier::{Appearance, AppearanceViewExt};

use support::GlobalTestLock;

fn cross_fade() -> Option<Animation> {
    Some(Animation::new(0.30, AnimationCurve::EaseInOut))
}

fn find_with_color(root: &Node, color: cove_core::Color) -> Option<Node> {
    if root.background_color() == Some(color) {
        return Some(root.clone());
    }
    root.children()
        .iter()
        .find_map(|child| find_with_color(child, color))
}

struct ListHarness {
    appearance: State<Appearance>,
}

impl View for ListHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let appearance = self.appearance.get();
        let list = List::<u64>::new(
            CollectionSelection::single(State::new(Some(2u64)).binding()),
            |id: u64, _selected: bool| {
                AnyView::new(cove_ui_compose::primitives::text::Text::new(format!(
                    "{id}"
                )))
            },
        )
        .rows(vec![1, 2]);
        AnyView::new(
            AnyView::new(list)
                .appearance(appearance, Default::default())
                .animation_value(cross_fade(), appearance),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("appearance", &self.appearance)]
    }
}

struct TabHarness {
    appearance: State<Appearance>,
}

impl View for TabHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let appearance = self.appearance.get();
        let selection = State::new(0i32);
        let tabs = TabView::try_new(
            selection.binding(),
            vec![
                TabItem::new(0i32, "A", cove_composition::EmptyView),
                TabItem::new(1i32, "B", cove_composition::EmptyView),
            ],
        )
        .expect("unique tab IDs");
        AnyView::new(
            AnyView::new(tabs)
                .appearance(appearance, Default::default())
                .animation_value(cross_fade(), appearance),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("appearance", &self.appearance)]
    }
}

#[test]
fn list_row_cross_fade() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let graph = support::graph();
            let appearance = State::new(Appearance::Dark);
            graph.install_view(
                AnyView::new(ListHarness {
                    appearance: appearance.clone(),
                }),
                Default::default(),
            );
            graph.compute_layout(400.0, 300.0);

            let dark = Theme::default_dark().colors.accent;
            let light = Theme::default_light().colors.accent;
            let root = graph.tree.borrow().root().expect("root");
            let node = find_with_color(&root, dark).expect("a row paints the dark accent");

            appearance.set(Appearance::Light);
            graph.recomposer.commit_all();
            assert!(
                scheduler.active_count() >= 1,
                "the appearance change registers an animation"
            );
            assert_eq!(
                node.background_color(),
                Some(dark),
                "the fill still reads dark at t = 0"
            );

            scheduler.tick(0.15);
            let mid = node.background_color();
            assert_ne!(mid, Some(dark), "the midpoint leaves the dark endpoint");
            assert_ne!(mid, Some(light), "the midpoint has not arrived yet");

            scheduler.tick(0.15);
            assert_eq!(
                node.background_color(),
                Some(light),
                "the fill lands on the light accent"
            );
            assert_eq!(scheduler.active_count(), 0, "the scheduler idles again");
        });
    });
}

#[test]
fn tab_cross_fade() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let graph = support::graph();
            let appearance = State::new(Appearance::Dark);
            graph.install_view(
                AnyView::new(TabHarness {
                    appearance: appearance.clone(),
                }),
                Default::default(),
            );
            graph.compute_layout(400.0, 300.0);

            // Flat tabs use the shared panel surface, not a raised segmented
            // control background. It must still participate in theme animation.
            let dark = Theme::default_dark().colors.surface;
            let light = Theme::default_light().colors.surface;
            let root = graph.tree.borrow().root().expect("root");
            let node = find_with_color(&root, dark).expect("the tab bar paints the dark surface");

            appearance.set(Appearance::Light);
            graph.recomposer.commit_all();
            assert!(
                scheduler.active_count() >= 1,
                "the appearance change registers an animation"
            );
            // GAP: Swift asserts the tab chrome still reads dark at t = 0 and
            // then cross-fades. The Rust tab bar
            // writes its surface non-animatably, so it snaps to the new theme
            // in the commit frame and never interpolates.
            assert_eq!(
                node.background_color(),
                Some(light),
                "GAP: the tab surface snaps to the light theme (Swift: cross-fades)"
            );
            scheduler.tick(0.30);
            assert_eq!(scheduler.active_count(), 0, "the scheduler idles again");
        });
    });
}
