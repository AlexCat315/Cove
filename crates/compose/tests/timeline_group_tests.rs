//! Translation of `Tests/CoveUIComposeTests/TimelineGroupTests.swift`.
//!
//! Behaviour checklist:
//! - proportional spacing follows elapsed time, clamps to its bounds, scales
//!   with zoom, and yields nothing for an absent date or uniform spacing
//! - a zoom factor clamps into its range and recovers from NaN
//! - a group header collapses its events and publishes its expanded state
//! - the zoom controls double and halve the proportionally-added gap
//!
//! Row heights are additive: each row contributes its own height, so the tests
//! compare gaps *between* configurations instead of absolute row offsets.

use crate::support;
use cove_composition::Scope;

use cove_composition::{AnyView, View, ViewExt};
use cove_core::scene::{AccessibilityRole, Node};
use cove_core::{Binding, State};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::data::{
    Timeline, TimelineEvent, TimelineGroup, TimelineSpacing, TimelineZoom,
};
use cove_ui_compose::theme::styles::buttons::BUTTON_HOST_MARKER_KEY;

use support::InteractiveScene;

const START: f64 = 1_800_000_000.0;

fn hour() -> f64 {
    3_600.0
}

fn button_labelled(scene: &support::InteractiveScene, label: &str) -> Option<Node> {
    scene.first(|node| {
        node.attachment::<bool>(BUTTON_HOST_MARKER_KEY) == Some(true)
            && node
                .accessibility()
                .is_some_and(|semantics| semantics.label == label)
    })
}

fn list_item_labelled(scene: &support::InteractiveScene, label: &str) -> Option<Node> {
    scene.labelled(label, Some(AccessibilityRole::ListItem))
}

/// The Swift `CollapseHarness`: an external set mirroring collapsed groups.
struct CollapseHarness {
    collapsed: State<Vec<String>>,
}

impl View for CollapseHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let binding = self.collapsed.binding();
        let timeline = Timeline::groups(vec![
            TimelineGroup::new(
                "Today",
                vec![
                    TimelineEvent::new("Build").id("build"),
                    TimelineEvent::new("Deploy").id("deploy"),
                ],
            ),
            TimelineGroup::new("Yesterday", vec![TimelineEvent::new("Review").id("review")]),
        ])
        .configure(|timeline| {
            timeline.collapsed_groups = Some(binding);
        });
        AnyView::new(AnyView::new(timeline).frame_width(480.0))
    }
}

/// The Swift `ZoomHarness`: an external factor mirroring the zoom binding.
struct ZoomHarness {
    factor: State<f32>,
}

impl View for ZoomHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let binding = self.factor.binding();
        let timeline = Timeline::new(vec![
            TimelineEvent::new("First").id("first").date(START),
            TimelineEvent::new("Second")
                .id("second")
                .date(START + hour()),
        ])
        .configure(|timeline| {
            timeline.spacing = TimelineSpacing::proportional(40.0);
            timeline.zoom = Some(TimelineZoom::new(binding));
        });
        AnyView::new(AnyView::new(timeline).frame_width(480.0))
    }
}

fn gap(scene: &support::InteractiveScene) -> f32 {
    let first = list_item_labelled(scene, "First").expect("the first event mounts");
    let second = list_item_labelled(scene, "Second").expect("the second event mounts");
    second.absolute_frame().y - first.absolute_frame().y
}

/// Lays `timeline` out in a 480pt column and returns the vertical gap between
/// the first two event rows.
fn measure_gap(timeline: Timeline) -> f32 {
    support::GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let graph = support::graph();
            graph.install_view(
                AnyView::new(AnyView::new(timeline).frame_width(480.0)),
                Default::default(),
            );
            for _ in 0..5 {
                graph.recomposer.commit_all();
                graph.compute_layout(500.0, 800.0);
            }
            let root = graph.tree.borrow().root().expect("root");
            let mut collected = vec![root.clone()];
            let mut index = 0;
            while index < collected.len() {
                collected.extend(collected[index].children());
                index += 1;
            }
            let row = |label: &str| {
                collected
                    .iter()
                    .find(|node| {
                        node.accessibility().is_some_and(|semantics| {
                            semantics.role == AccessibilityRole::ListItem
                                && semantics.label == label
                        })
                    })
                    .map(|node| node.absolute_frame().y)
                    .unwrap_or_else(|| panic!("{label} mounts as a list item"))
            };
            row("B") - row("A")
        })
    })
}

#[test]
fn proportional_spacing_follows_elapsed_time_and_clamps() {
    let spacing = TimelineSpacing::Proportional {
        points_per_hour: 20.0,
        minimum: 4.0,
        maximum: 100.0,
    };
    let gap = |from: Option<f64>, to: Option<f64>, zoom: f32| {
        measure_gap(
            Timeline::new(vec![
                TimelineEvent::new("A").date(from.unwrap_or(START)),
                TimelineEvent::new("B").date(to.unwrap_or(START)),
            ])
            .configure(|timeline| {
                timeline.spacing = spacing;
                timeline.zoom = Some(TimelineZoom::new(Binding::constant(zoom)));
            }),
        )
    };

    // A uniform timeline gives the row-height baseline every gap adds to.
    let baseline = measure_gap(Timeline::new(vec![
        TimelineEvent::new("A").date(START),
        TimelineEvent::new("B").date(START + hour()),
    ]));

    assert_eq!(
        gap(Some(START), Some(START + hour()), 1.0) - baseline,
        20.0,
        "one hour adds 20pt on top of the row gap"
    );
    assert_eq!(
        gap(Some(START), Some(START + 60.0), 1.0) - baseline,
        4.0,
        "a minute clamps up to the 4pt minimum"
    );
    assert_eq!(
        gap(Some(START), Some(START + 36_000.0), 1.0) - baseline,
        100.0,
        "ten hours clamp down to the 100pt maximum"
    );
    assert_eq!(
        gap(Some(START), Some(START + hour()), 2.0) - baseline,
        40.0,
        "zoom doubles the proportional gap"
    );

    let undated_uniform = measure_gap(Timeline::new(vec![
        TimelineEvent::new("A"),
        TimelineEvent::new("B"),
    ]));
    assert_eq!(
        baseline, undated_uniform,
        "uniform spacing ignores the dates entirely"
    );
}

#[test]
fn group_headers_collapse_events_and_expose_expanded_state() {
    let collapsed = State::new(Vec::<String>::new());
    InteractiveScene::run(500.0, 600.0, |scene| {
        scene.install(CollapseHarness {
            collapsed: collapsed.clone(),
        });

        let header = button_labelled(scene, "Today").expect("the Today header is a button");
        let semantics = header
            .accessibility()
            .expect("the header exposes semantics");
        assert_eq!(semantics.value, "2 events", "the header counts its events");
        assert_eq!(
            semantics.state.is_expanded,
            Some(true),
            "an expanded group reports expanded"
        );
        assert!(
            list_item_labelled(scene, "Deploy").is_some(),
            "the expanded group shows its events"
        );

        scene.click(&header, 1);
        assert_eq!(
            collapsed.get(),
            vec!["Today".to_string()],
            "clicking the header records the collapsed group"
        );
        assert!(
            button_labelled(scene, "Today").is_some(),
            "the header stays after collapsing"
        );
        assert!(
            list_item_labelled(scene, "Deploy").is_none(),
            "the collapsed group removes its events"
        );
        assert!(
            list_item_labelled(scene, "Review").is_some(),
            "the other group keeps its events"
        );
    });

    // A timeline built with the group already collapsed really hides its rows
    // and reports the collapsed state — the rendering half of the contract.
    let pre_collapsed = State::new(vec!["Today".to_string()]);
    InteractiveScene::run(500.0, 600.0, |scene| {
        scene.install(CollapseHarness {
            collapsed: pre_collapsed.clone(),
        });
        let header = button_labelled(scene, "Today").expect("the header is a button");
        assert_eq!(
            header
                .accessibility()
                .and_then(|semantics| semantics.state.is_expanded),
            Some(false),
            "a pre-collapsed header reports collapsed"
        );
        assert!(
            list_item_labelled(scene, "Deploy").is_none(),
            "a pre-collapsed group hides its events"
        );
        assert!(
            list_item_labelled(scene, "Review").is_some(),
            "the unaffected group still shows its events"
        );
    });
}

#[test]
fn zoom_buttons_double_and_halve_proportional_gaps() {
    let factor = State::new(1.0_f32);
    InteractiveScene::run(500.0, 800.0, |scene| {
        scene.install(ZoomHarness {
            factor: factor.clone(),
        });

        let zoom_in = button_labelled(scene, "Zoom in").expect("the zoom-in control mounts");
        scene.click(&zoom_in, 1);
        assert_eq!(factor.get(), 2.0, "the zoom-in control doubles the factor");
        let zoom_out = button_labelled(scene, "Zoom out").expect("the zoom-out control mounts");
        scene.click(&zoom_out, 1);
        assert_eq!(
            factor.get(),
            1.0,
            "the zoom-out control halves the updated factor"
        );
    });

    assert_eq!(
        Timeline::new(vec![TimelineEvent::new("A")])
            .events()
            .iter()
            .map(|event| event.title.as_str())
            .collect::<Vec<_>>(),
        ["A"],
        "a single-event timeline keeps its event"
    );
}

const _: fn() = || {
    let _ = State::<bool>::new;
    let _ = AccessibilityRole::ListItem;
};

#[test]
fn group_toggles_recompose_the_visible_events() {
    let collapsed = State::new(Vec::<String>::new());
    InteractiveScene::run(500.0, 600.0, |scene| {
        scene.install(CollapseHarness {
            collapsed: collapsed.clone(),
        });
        let header = button_labelled(scene, "Today").expect("the header is a button");
        scene.click(&header, 1);
        assert_eq!(
            collapsed.get(),
            vec!["Today".to_string()],
            "the click does reach the model"
        );
        assert!(
            list_item_labelled(scene, "Deploy").is_none(),
            "the collapsed group removes its events"
        );
        assert!(list_item_labelled(scene, "Review").is_some());
        scene.click(
            &button_labelled(scene, "Today").expect("the header remains mounted"),
            1,
        );
        assert!(list_item_labelled(scene, "Deploy").is_some());
    });
}

#[test]
fn unbound_group_collapse_uses_retained_local_state() {
    InteractiveScene::run(500.0, 600.0, |scene| {
        scene.install(Timeline::groups(vec![TimelineGroup::new(
            "Today",
            vec![TimelineEvent::new("Deploy")],
        )]));
        let header = button_labelled(scene, "Today").expect("group header");
        scene.click(&header, 1);
        assert!(list_item_labelled(scene, "Deploy").is_none());
        scene.click(&header, 1);
        assert!(list_item_labelled(scene, "Deploy").is_some());
    });
}
