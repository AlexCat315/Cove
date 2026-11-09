//! List–detail navigation and adaptive tabs across window sizes and folds.
//!
//! Behaviour checklist:
//! - a phone shows one pane; a wide window shows two; a vertical fold or hinge
//!   places the split on it; a tabletop fold puts the detail above the list
//! - selecting in one pane opens the detail; leaving it keeps the selection
//! - unfolding, folding and resizing keep the list and detail mounted (their
//!   own state survives), and the selection reappears in two panes
//! - tabs: bottom bar on compact windows, rail on wider touch surfaces, top
//!   tabs on pointer desktops, with the selected content mounted once

use crate::support;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use cove_composition::{AnyView, DynamicField, Scope, View};
use cove_core::scene::Node;
use cove_core::{
    Binding, DeviceClass, DisplayFeature, Environment, FeatureOrientation, FoldableKind,
    InputCapabilities, Posture, Rect, State,
};
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::primitives::layout::TabItem;
use cove_ui_compose::primitives::navigation::{
    AdaptiveTabs, ListDetail, ListDetailPresentation, ListDetailState, TabsPresentation,
};
use cove_ui_compose::primitives::text::Text;

use support::InteractiveScene;

fn surface(width: f32, height: f32) -> Environment {
    let mut environment = Environment::default();
    environment.viewport.width = width;
    environment.viewport.height = height;
    environment
}

fn phone() -> Environment {
    let mut environment = surface(393.0, 852.0);
    environment.device_class = DeviceClass::Phone;
    environment.input = InputCapabilities::TOUCH;
    environment
}

fn fold_open() -> Environment {
    let mut environment = surface(841.0, 701.0);
    environment.device_class = DeviceClass::Foldable(FoldableKind::Book);
    environment.input = InputCapabilities::TOUCH;
    environment.display_features = vec![DisplayFeature::fold(
        Rect::new(420.5, 0.0, 0.0, 701.0),
        FeatureOrientation::Vertical,
        false,
    )];
    environment
}

fn fold_cover() -> Environment {
    let mut environment = surface(360.0, 800.0);
    environment.device_class = DeviceClass::Foldable(FoldableKind::Book);
    environment.input = InputCapabilities::TOUCH;
    environment
}

fn dual_screen() -> Environment {
    let mut environment = surface(1114.0, 705.0);
    environment.device_class = DeviceClass::Foldable(FoldableKind::DualScreen);
    environment.input = InputCapabilities::TOUCH;
    environment.display_features = vec![DisplayFeature::hinge(
        Rect::new(540.0, 0.0, 34.0, 705.0),
        FeatureOrientation::Vertical,
    )];
    environment
}

fn tabletop() -> Environment {
    let mut environment = surface(701.0, 841.0);
    environment.device_class = DeviceClass::Foldable(FoldableKind::Book);
    environment.input = InputCapabilities::TOUCH;
    environment.posture = Posture::Tabletop;
    environment.display_features = vec![DisplayFeature::fold(
        Rect::new(0.0, 420.5, 701.0, 0.0),
        FeatureOrientation::Horizontal,
        true,
    )];
    environment
}

#[test]
fn presentation_follows_space_and_folds() {
    let full = |environment: &Environment| environment.viewport.bounds();
    let resolve = |environment: Environment| {
        ListDetailPresentation::resolve(&environment, full(&environment), 320.0)
    };
    assert_eq!(resolve(phone()), ListDetailPresentation::Single);
    assert_eq!(resolve(fold_cover()), ListDetailPresentation::Single);
    assert_eq!(
        resolve(surface(1280.0, 800.0)),
        ListDetailPresentation::SideBySide {
            list_width: 320.0,
            gap: 0.0
        }
    );
    assert_eq!(
        resolve(fold_open()),
        ListDetailPresentation::SideBySide {
            list_width: 420.5,
            gap: 0.0
        },
        "the split sits on the crease"
    );
    assert_eq!(
        resolve(dual_screen()),
        ListDetailPresentation::SideBySide {
            list_width: 540.0,
            gap: 34.0
        },
        "nothing is placed under the hinge"
    );
    assert_eq!(
        resolve(tabletop()),
        ListDetailPresentation::Stacked {
            detail_height: 420.5,
            gap: 0.0
        }
    );
    // Beside a 72pt rail the split still lands on the hinge.
    let environment = dual_screen();
    assert_eq!(
        ListDetailPresentation::resolve(&environment, Rect::new(72.0, 0.0, 1042.0, 705.0), 320.0),
        ListDetailPresentation::SideBySide {
            list_width: 468.0,
            gap: 34.0
        }
    );
    // A medium window without folds stays single-pane.
    assert_eq!(
        resolve(surface(700.0, 900.0)),
        ListDetailPresentation::Single
    );
}

static NEXT_MOUNT: AtomicU64 = AtomicU64::new(1);

/// Records the mount identity it remembers on first composition, so a remount
/// shows up as a new identity.
struct Probe {
    name: &'static str,
    mounts: Rc<RefCell<Vec<(&'static str, u64)>>>,
}

impl View for Probe {
    fn body(&self, cx: &Scope) -> AnyView {
        let identity = cx.remember(|| NEXT_MOUNT.fetch_add(1, Ordering::Relaxed));
        self.mounts.borrow_mut().push((self.name, identity));
        AnyView::new(Text::new(self.name))
    }
}

/// Re-renders the list–detail view under a changing environment.
struct Device {
    environment: State<Environment>,
    state: State<ListDetailState<u32>>,
    mounts: Rc<RefCell<Vec<(&'static str, u64)>>>,
}

impl View for Device {
    fn body(&self, _cx: &Scope) -> AnyView {
        let state = self.state.clone();
        let mounts = self.mounts.clone();
        let detail_mounts = self.mounts.clone();
        AnyView::new(
            AnyView::new(
                ListDetail::new(
                    Binding::new(
                        {
                            let state = state.clone();
                            move || state.get()
                        },
                        move |value| state.set(value),
                    ),
                    move |_selection| {
                        AnyView::new(Probe {
                            name: "list",
                            mounts: mounts.clone(),
                        })
                    },
                    move |_item, _context| {
                        AnyView::new(Probe {
                            name: "detail",
                            mounts: detail_mounts.clone(),
                        })
                    },
                )
                .placeholder(Text::new("placeholder")),
            )
            .surface_environment(self.environment.get()),
        )
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("environment", &self.environment),
            DynamicField::state("state", &self.state),
        ]
    }
}

/// A device reports the window it renders into: resize the scene with it.
fn switch(scene: &mut InteractiveScene, environment: &State<Environment>, next: Environment) {
    scene.width = next.viewport.width;
    scene.height = next.viewport.height;
    environment.set(next);
    scene.settle(3, 0.016);
}

fn pane(scene: &InteractiveScene, role: &str) -> Rect {
    scene
        .first(|node: &Node| support::debug_name(node).as_deref() == Some(role))
        .unwrap_or_else(|| panic!("{role} is mounted"))
        .absolute_frame()
}

fn identities(mounts: &Rc<RefCell<Vec<(&'static str, u64)>>>, name: &str) -> Vec<u64> {
    let mut seen: Vec<u64> = mounts
        .borrow()
        .iter()
        .filter(|(probe, _)| *probe == name)
        .map(|(_, identity)| *identity)
        .collect();
    seen.dedup();
    seen.sort_unstable();
    seen.dedup();
    seen
}

#[test]
fn folding_keeps_panes_mounted_and_the_selection() {
    InteractiveScene::run(360.0, 800.0, |scene| {
        let environment = State::new(fold_cover());
        let state = State::new(ListDetailState::<u32>::default());
        let mounts: Rc<RefCell<Vec<(&'static str, u64)>>> = Rc::default();
        scene.install(Device {
            environment: environment.clone(),
            state: state.clone(),
            mounts: mounts.clone(),
        });
        scene.settle(3, 0.016);
        assert!(pane(scene, "list-detail-list").width > 0.0);
        assert_eq!(pane(scene, "list-detail-detail").width, 0.0);

        // Selecting on the cover screen opens the detail over the list.
        let mut next = state.get();
        next.select(7);
        state.set(next);
        scene.settle(3, 0.016);
        assert_eq!(pane(scene, "list-detail-list").width, 0.0);
        assert!(pane(scene, "list-detail-detail").width > 0.0);

        // Unfolding shows both, split on the crease, with the detail kept.
        switch(scene, &environment, fold_open());
        let list = pane(scene, "list-detail-list");
        let detail = pane(scene, "list-detail-detail");
        assert_eq!((list.x, list.width), (0.0, 420.5));
        assert!(
            (detail.x - 421.5).abs() < 0.01,
            "detail after the hairline: {detail:?}"
        );

        // Going back on the cover screen keeps the selection for the next unfold.
        switch(scene, &environment, fold_cover());
        let mut next = state.get();
        next.back();
        state.set(next);
        scene.settle(3, 0.016);
        assert!(pane(scene, "list-detail-list").width > 0.0);
        assert_eq!(state.get().selection, Some(7));
        switch(scene, &environment, dual_screen());
        let detail = pane(scene, "list-detail-detail");
        assert_eq!(detail.x, 574.0, "the detail starts after the hinge");

        assert_eq!(
            identities(&mounts, "list").len(),
            1,
            "the list never remounts"
        );
        assert_eq!(
            identities(&mounts, "detail").len(),
            1,
            "the detail never remounts"
        );
    });
}

#[test]
fn tabletop_puts_the_detail_above_the_list() {
    InteractiveScene::run(701.0, 841.0, |scene| {
        let mut initial = ListDetailState::default();
        initial.select(3);
        scene.install(Device {
            environment: State::new(tabletop()),
            state: State::new(initial),
            mounts: Rc::default(),
        });
        scene.settle(3, 0.016);
        let detail = pane(scene, "list-detail-detail");
        let list = pane(scene, "list-detail-list");
        assert_eq!((detail.y, detail.height), (0.0, 420.5));
        assert!(list.y >= 420.5, "the list sits below the fold: {list:?}");
    });
}

#[test]
fn tab_presentation_follows_platform_conventions() {
    assert_eq!(
        TabsPresentation::resolve(&phone()),
        TabsPresentation::BottomBar
    );
    assert_eq!(
        TabsPresentation::resolve(&fold_open()),
        TabsPresentation::Rail
    );
    let mut desktop = surface(1280.0, 800.0);
    desktop.input = InputCapabilities::POINTER | InputCapabilities::KEYBOARD;
    assert_eq!(
        TabsPresentation::resolve(&desktop),
        TabsPresentation::TopTabs
    );
    let mut tablet = surface(1024.0, 768.0);
    tablet.device_class = DeviceClass::Tablet;
    tablet.input = InputCapabilities::TOUCH | InputCapabilities::POINTER;
    assert_eq!(TabsPresentation::resolve(&tablet), TabsPresentation::Rail);
}

struct Tabs {
    environment: State<Environment>,
    mounts: Rc<RefCell<Vec<(&'static str, u64)>>>,
}

impl View for Tabs {
    fn body(&self, _cx: &Scope) -> AnyView {
        let tabs = vec![
            TabItem::new(
                1u8,
                "Home",
                Probe {
                    name: "home",
                    mounts: self.mounts.clone(),
                },
            ),
            TabItem::new(2u8, "Search", Text::new("search")),
        ];
        AnyView::new(
            AnyView::new(AdaptiveTabs::try_new(Binding::constant(1u8), tabs).expect("unique ids"))
                .surface_environment(self.environment.get()),
        )
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("environment", &self.environment)]
    }
}

#[test]
fn tabs_keep_their_content_across_presentations() {
    InteractiveScene::run(360.0, 800.0, |scene| {
        let environment = State::new(fold_cover());
        let mounts: Rc<RefCell<Vec<(&'static str, u64)>>> = Rc::default();
        scene.install(Tabs {
            environment: environment.clone(),
            mounts: mounts.clone(),
        });
        scene.settle(3, 0.016);
        assert!(pane(scene, "adaptive-tabs-bottom").height > 0.0);
        assert_eq!(pane(scene, "adaptive-tabs-rail").width, 0.0);

        switch(scene, &environment, fold_open());
        assert!(pane(scene, "adaptive-tabs-rail").width > 0.0);
        assert_eq!(pane(scene, "adaptive-tabs-bottom").height, 0.0);

        let mut desktop = surface(841.0, 701.0);
        desktop.input = InputCapabilities::POINTER | InputCapabilities::KEYBOARD;
        switch(scene, &environment, desktop);
        assert!(pane(scene, "adaptive-tabs-top").height > 0.0);
        assert_eq!(pane(scene, "adaptive-tabs-rail").width, 0.0);

        assert_eq!(
            identities(&mounts, "home").len(),
            1,
            "the content never remounts"
        );
    });
}

struct SidebarDevice {
    environment: State<Environment>,
    seen: Rc<RefCell<Vec<DeviceClass>>>,
}

impl View for SidebarDevice {
    fn body(&self, _cx: &Scope) -> AnyView {
        use cove_ui_compose::adaptive::SurfaceReader;
        use cove_ui_compose::primitives::layout::sidebar::{Sidebar, SidebarItem, SidebarSection};
        let seen = self.seen.clone();
        let footer = AnyView::new(SurfaceReader::new(move |surface: &Environment| {
            seen.borrow_mut().push(surface.device_class);
            AnyView::new(Text::new("footer"))
        }));
        AnyView::new(
            AnyView::new(
                Sidebar::try_new(
                    Binding::constant(Some(1u8)),
                    vec![SidebarSection::new("s", vec![SidebarItem::new(1u8, "One")])],
                )
                .expect("unique ids")
                .footer(footer),
            )
            .surface_environment(self.environment.get()),
        )
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("environment", &self.environment)]
    }
}

#[test]
fn sidebar_slots_follow_environment_changes() {
    InteractiveScene::run(360.0, 800.0, |scene| {
        let environment = State::new(phone());
        let seen: Rc<RefCell<Vec<DeviceClass>>> = Rc::default();
        scene.install(SidebarDevice {
            environment: environment.clone(),
            seen: seen.clone(),
        });
        scene.settle(3, 0.016);
        assert_eq!(seen.borrow().last(), Some(&DeviceClass::Phone));
        let mut desktop = surface(360.0, 800.0);
        desktop.input = InputCapabilities::POINTER | InputCapabilities::KEYBOARD;
        environment.set(desktop);
        scene.settle(3, 0.016);
        assert_eq!(
            seen.borrow().last(),
            Some(&DeviceClass::Desktop),
            "the footer slot sees the new environment"
        );
    });
}

mod sidebar_adaptation {
    use super::*;
    use cove_composition::adaptive::AdaptationMark;
    use cove_core::{DesignLanguage, Platform};
    use cove_ui_compose::primitives::layout::sidebar::{
        Sidebar, SidebarItem, SidebarItemStyle, SidebarPresentation, SidebarSection,
    };
    use cove_ui_compose::theme::styles::buttons::ButtonStyle;

    fn sidebar() -> Sidebar<u8> {
        Sidebar::try_new(
            Binding::constant(Some(1u8)),
            vec![SidebarSection::new(
                "s",
                vec![SidebarItem::new(1u8, "One"), SidebarItem::new(2u8, "Two")],
            )],
        )
        .expect("unique ids")
    }

    fn on(platform: Platform, mut environment: Environment) -> Environment {
        environment.platform = platform;
        environment.design_language = platform.design_language();
        environment
    }

    #[test]
    fn presentation_follows_the_device() {
        let iphone = on(Platform::IOS, phone());
        let android = on(Platform::Android, phone());
        assert_eq!(
            sidebar().resolved_presentation(&iphone),
            SidebarPresentation::List { grouped: true }
        );
        assert_eq!(
            sidebar().resolved_presentation(&android),
            SidebarPresentation::List { grouped: false }
        );
        let mut tablet = surface(1024.0, 768.0);
        tablet.device_class = DeviceClass::Tablet;
        assert_eq!(
            sidebar().resolved_presentation(&tablet),
            SidebarPresentation::Expanded
        );
        assert_eq!(
            sidebar().resolved_presentation(&surface(1280.0, 800.0)),
            SidebarPresentation::Expanded
        );
        assert_eq!(
            sidebar().adapts(false).resolved_presentation(&iphone),
            SidebarPresentation::Expanded
        );
        assert_eq!(iphone.design_language, DesignLanguage::Cupertino);
    }

    #[test]
    fn row_style_identity_includes_presentation_and_height() {
        let identity = |presentation, minimum_height| {
            SidebarItemStyle {
                minimum_height,
                presentation,
            }
            .style_identity()
        };
        let grouped = identity(SidebarPresentation::List { grouped: true }, 44.0);
        assert_ne!(
            grouped,
            identity(SidebarPresentation::List { grouped: false }, 44.0)
        );
        assert_ne!(
            grouped,
            identity(SidebarPresentation::List { grouped: true }, 48.0)
        );
        assert_ne!(grouped, identity(SidebarPresentation::Expanded, 44.0));
    }

    fn rows(scene: &InteractiveScene) -> Vec<Rect> {
        ["One", "Two"]
            .iter()
            .map(|label| {
                scene
                    .labelled(label, Some(cove_core::scene::AccessibilityRole::Button))
                    .or_else(|| scene.labelled(label, None))
                    .unwrap_or_else(|| panic!("{label} is mounted"))
                    .absolute_frame()
            })
            .collect()
    }

    fn desktop_marks(scene: &InteractiveScene) -> usize {
        scene
            .nodes()
            .iter()
            .filter(|node| AdaptationMark::of(node) == Some(AdaptationMark::DesktopOnly))
            .count()
    }

    #[test]
    fn a_handheld_sidebar_is_a_touch_list_and_a_desktop_one_a_panel() {
        InteractiveScene::run(393.0, 852.0, |scene| {
            scene.install(AnyView::new(
                AnyView::new(sidebar()).surface_environment(on(Platform::IOS, phone())),
            ));
            scene.settle(3, 0.016);
            for row in rows(scene) {
                assert!(row.height >= 44.0, "a touch row: {row:?}");
            }
            assert_eq!(desktop_marks(scene), 0);
        });
        InteractiveScene::run(1280.0, 800.0, |scene| {
            let mut desktop = surface(1280.0, 800.0);
            desktop.input = InputCapabilities::POINTER | InputCapabilities::KEYBOARD;
            scene.install(AnyView::new(
                AnyView::new(sidebar()).surface_environment(desktop),
            ));
            scene.settle(3, 0.016);
            assert!(rows(scene).iter().all(|row| row.height < 44.0));
            assert_eq!(desktop_marks(scene), 1, "the panel is marked desktop-only");
        });
    }
}
