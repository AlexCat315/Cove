//! Overlays stay inside one window segment when a fold or hinge divides it.
//!
//! Behaviour checklist:
//! - a dialog across a vertical hinge sits in the leading segment, in the
//!   trailing one under right-to-left layout, and never overlaps the hinge
//! - in tabletop posture a dialog sits below the fold
//! - an anchored panel opened beside the hinge is moved back into the segment
//!   it was opened in instead of straddling it
//! - without display features nothing changes

use crate::support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::layout::LayoutDirectionExt;
use cove_composition::{AnyView, LayoutDebugExt, ViewGraph};
use cove_core::scene::{
    AccessibilityRole, Align, Direction, FlexDirection, Justify, NodeTree, Recomposer, Wrap,
};
use cove_core::{
    Binding, DeviceClass, DisplayFeature, Environment, FeatureOrientation, FoldableKind,
    InputCapabilities, Point, Posture, Rect,
};
use cove_ui_compose::BoxView;
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::layout::{FrameDimension, LayoutFrameExt};
use cove_ui_compose::primitives::overlays::modal::{Modal, ModalOptions};
use cove_ui_compose::primitives::overlays::portal::{LayerRoot, PortalStoreHolder};

use support::{GlobalTestLock, InteractiveScene};

const HINGE: Rect = Rect::new(540.0, 0.0, 34.0, 705.0);

fn dual_screen() -> Environment {
    let mut environment = Environment::default();
    environment.viewport.width = 1114.0;
    environment.viewport.height = 705.0;
    environment.input = InputCapabilities::POINTER | InputCapabilities::KEYBOARD;
    environment.device_class = DeviceClass::Foldable(FoldableKind::DualScreen);
    environment.display_features = vec![DisplayFeature::hinge(HINGE, FeatureOrientation::Vertical)];
    environment
}

fn tabletop() -> Environment {
    let mut environment = Environment::default();
    environment.viewport.width = 701.0;
    environment.viewport.height = 841.0;
    environment.input = InputCapabilities::POINTER | InputCapabilities::KEYBOARD;
    environment.device_class = DeviceClass::Foldable(FoldableKind::Book);
    environment.posture = Posture::Tabletop;
    environment.display_features = vec![DisplayFeature::fold(
        Rect::new(0.0, 420.5, 701.0, 0.0),
        FeatureOrientation::Horizontal,
        true,
    )];
    environment
}

fn empty_box() -> BoxView {
    BoxView::new(
        FlexDirection::Column,
        Align::Stretch,
        Justify::FlexStart,
        Wrap::NoWrap,
        0.0,
        AnyView::new(cove_composition::EmptyView),
    )
}

fn dialog(environment: Environment, direction: Direction) -> AnyView {
    let options = ModalOptions {
        label: "Fold dialog".into(),
        ..ModalOptions::default()
    };
    let content = AnyView::new(empty_box().frame(
        Some(FrameDimension::Percent(100.0)),
        Some(FrameDimension::Percent(100.0)),
        None,
        None,
        None,
        None,
    ));
    AnyView::new(
        AnyView::new(LayerRoot::new(AnyView::new(
            Modal::new(Binding::constant(true), content).options(options),
        )))
        .layout_direction(direction)
        .surface_environment(environment),
    )
}

fn panel(scene: &InteractiveScene) -> Rect {
    scene
        .first(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.role == AccessibilityRole::Dialog)
        })
        .expect("the dialog panel is mounted")
        .absolute_frame()
}

fn overlaps(a: Rect, b: Rect) -> bool {
    a.x < b.max_x() && b.x < a.max_x() && a.y < b.max_y() && b.y < a.max_y()
}

#[test]
fn a_dialog_sits_in_the_leading_segment_beside_a_hinge() {
    InteractiveScene::run(1114.0, 705.0, |scene| {
        scene.install(dialog(dual_screen(), Direction::Ltr));
        let frame = panel(scene);
        assert!(frame.width > 0.0 && frame.height > 0.0);
        assert!(
            frame.max_x() <= HINGE.x,
            "the dialog ends before the hinge: {frame:?}"
        );
        assert!(!overlaps(frame, HINGE));
    });
}

#[test]
fn a_right_to_left_dialog_sits_in_the_trailing_segment() {
    InteractiveScene::run(1114.0, 705.0, |scene| {
        scene.install(dialog(dual_screen(), Direction::Rtl));
        let frame = panel(scene);
        assert!(
            frame.x >= HINGE.max_x(),
            "the dialog starts after the hinge: {frame:?}"
        );
    });
}

#[test]
fn a_tabletop_dialog_sits_below_the_fold() {
    InteractiveScene::run(701.0, 841.0, |scene| {
        scene.install(dialog(tabletop(), Direction::Ltr));
        let frame = panel(scene);
        assert!(
            frame.y >= 420.5,
            "the dialog sits in the lower segment: {frame:?}"
        );
        assert!(frame.max_y() <= 841.0);
    });
}

#[test]
fn without_folds_a_dialog_is_centered_in_the_window() {
    InteractiveScene::run(1114.0, 705.0, |scene| {
        let mut flat = dual_screen();
        flat.display_features.clear();
        scene.install(dialog(flat, Direction::Ltr));
        let frame = panel(scene);
        let center = frame.x + frame.width / 2.0;
        assert!(
            (center - 557.0).abs() < 1.0,
            "centered in the window: {frame:?}"
        );
    });
}

fn anchored_frame(environment: Environment, origin: Point) -> Rect {
    GlobalTestLock::locked(|| {
        let store = PortalStoreHolder::current();
        store.clear();
        store.register(
            Some("fold-panel".to_string()),
            origin,
            Some(200.0),
            AnyView::new(
                AnyView::new(empty_box())
                    .frame_height(120.0)
                    .debug_name("fold-panel-content"),
            ),
        );
        let graph = Rc::new(ViewGraph::new(
            Rc::new(RefCell::new(NodeTree::new())),
            Rc::new(Recomposer::new()),
        ));
        graph.install(
            AnyView::new(
                AnyView::new(LayerRoot::new(
                    AnyView::new(empty_box()).frame_size(1114.0, 705.0),
                ))
                .surface_environment(environment),
            ),
            Default::default(),
        );
        for _ in 0..3 {
            graph.recomposer.commit_all();
            graph.compute_layout(1114.0, 705.0);
        }
        let frame = graph
            .layout_snapshot()
            .iter()
            .find(|entry| entry.debug_name.as_deref() == Some("fold-panel"))
            .expect("the entry is laid out")
            .absolute_frame;
        store.clear();
        frame
    })
}

#[test]
fn an_anchored_panel_does_not_straddle_a_hinge() {
    let left = anchored_frame(dual_screen(), Point::new(480.0, 100.0));
    assert!(
        left.max_x() <= HINGE.x,
        "opened in the left segment, it stays there: {left:?}"
    );
    let right = anchored_frame(dual_screen(), Point::new(600.0, 100.0));
    assert!(
        right.x >= HINGE.max_x(),
        "opened in the right segment, it stays there: {right:?}"
    );
    let mut flat = dual_screen();
    flat.display_features.clear();
    let unfolded = anchored_frame(flat, Point::new(480.0, 100.0));
    assert_eq!(unfolded.x, 480.0, "without a hinge nothing moves");
}

mod segmented_layout {
    use super::*;
    use cove_ui_compose::primitives::layout::{PostureLayout, SegmentSplit, SegmentedLayout};

    fn named(name: &'static str) -> AnyView {
        AnyView::new(
            AnyView::new(empty_box())
                .frame(
                    Some(FrameDimension::Percent(100.0)),
                    Some(FrameDimension::Percent(100.0)),
                    None,
                    None,
                    None,
                    None,
                )
                .debug_name(name),
        )
    }

    fn frame_of(scene: &InteractiveScene, name: &str) -> Option<Rect> {
        scene
            .first(|node| support::debug_name(node).as_deref() == Some(name))
            .map(|node| node.absolute_frame())
    }

    #[test]
    fn splits_resolve_against_the_container() {
        let environment = dual_screen();
        assert_eq!(
            SegmentSplit::resolve(&environment, Rect::new(0.0, 0.0, 1114.0, 705.0)),
            Some(SegmentSplit::Horizontal {
                leading: 540.0,
                gap: 34.0
            })
        );
        // A container offset by a 72pt rail still splits at the hinge.
        assert_eq!(
            SegmentSplit::resolve(&environment, Rect::new(72.0, 0.0, 1042.0, 705.0)),
            Some(SegmentSplit::Horizontal {
                leading: 468.0,
                gap: 34.0
            })
        );
        // A container entirely on one side is not divided.
        assert_eq!(
            SegmentSplit::resolve(&environment, Rect::new(600.0, 0.0, 400.0, 705.0)),
            None
        );
        assert_eq!(
            SegmentSplit::resolve(&tabletop(), Rect::new(0.0, 0.0, 701.0, 841.0)),
            Some(SegmentSplit::Vertical {
                top: 420.5,
                gap: 0.0
            })
        );
    }

    #[test]
    fn panes_fill_the_segments_beside_a_hinge() {
        InteractiveScene::run(1114.0, 705.0, |scene| {
            scene.install(AnyView::new(
                AnyView::new(
                    SegmentedLayout::new(named("single")).split(named("left"), named("right")),
                )
                .surface_environment(dual_screen()),
            ));
            scene.settle(3, 0.016);
            let left = frame_of(scene, "left").expect("the left pane is mounted");
            let right = frame_of(scene, "right").expect("the right pane is mounted");
            assert_eq!((left.x, left.width), (0.0, 540.0));
            assert_eq!((right.x, right.width), (574.0, 540.0));
            assert!(frame_of(scene, "single").is_none());
        });
    }

    #[test]
    fn an_undivided_window_shows_the_single_content() {
        InteractiveScene::run(1114.0, 705.0, |scene| {
            let mut flat = dual_screen();
            flat.display_features.clear();
            scene.install(AnyView::new(
                AnyView::new(
                    SegmentedLayout::new(named("single")).split(named("left"), named("right")),
                )
                .surface_environment(flat),
            ));
            scene.settle(3, 0.016);
            assert!(frame_of(scene, "single").is_some());
            assert!(frame_of(scene, "left").is_none());
        });
    }

    #[test]
    fn tabletop_puts_controls_below_the_fold() {
        InteractiveScene::run(701.0, 841.0, |scene| {
            scene.install(AnyView::new(
                AnyView::new(
                    PostureLayout::new(named("flat"))
                        .tabletop(named("video"), named("controls"))
                        .book(named("left-page"), named("right-page")),
                )
                .surface_environment(tabletop()),
            ));
            scene.settle(3, 0.016);
            let video = frame_of(scene, "video").expect("content above the fold");
            let controls = frame_of(scene, "controls").expect("controls below the fold");
            assert_eq!((video.y, video.height), (0.0, 420.5));
            assert_eq!(controls.y, 420.5);
            assert!(frame_of(scene, "left-page").is_none());
        });
    }

    #[test]
    fn a_flat_posture_uses_the_flat_content() {
        InteractiveScene::run(701.0, 841.0, |scene| {
            let mut flat = tabletop();
            flat.posture = Posture::Flat;
            scene.install(AnyView::new(
                AnyView::new(
                    PostureLayout::new(named("flat")).tabletop(named("video"), named("controls")),
                )
                .surface_environment(flat),
            ));
            scene.settle(3, 0.016);
            assert!(frame_of(scene, "flat").is_some());
            assert!(frame_of(scene, "video").is_none());
        });
    }
}
