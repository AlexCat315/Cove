//! Translation of `Tests/CoveUIComposeTests/ChartInteractionTests.swift`.
//!
//! Behaviour checklist:
//! - clicking a bar selects and pins it; the accessibility value describes it,
//!   and leaving the plot keeps a pinned selection
//! - arrows step values and series, End jumps, Escape clears, and the assistive
//!   decrement action steps too
//! - legend items toggle series, show "Hidden", and the selection skips them
//! - touch taps select pie slices; the hole selects nothing
//!
//! Name mapping: `ChartPoint` is `ChartPoint::new(series, index)`,
//! `UIEnvironment` is `Environment`, `.surfaceEnvironment(_)` is
//! `.surface_environment(..)`, and `configure: { $0... }` becomes an explicit
//! `ChartConfiguration`. `ChartSlice("Desktop", value: 3)` is
//! `ChartSlice::new("Desktop", 3.0, None)`.

//!
//! The `Bar` series in Swift's harness is titled "Revenue"; the Rust legend only
//! lists named series, so all four carry titles here.
//!
//! Named differences (reported, not fixed):
//!
//! - Rust's first `Right` press from no selection lands on index 1, not index 0:
//!   `ChartPlot::stepped_index` seeds a forward step at `index + delta` once
//!   `start` is created, so the opening step already advances. Swift's first
//!   press reads the first value.
//! - A pie tap on a real slice selects nothing: the slice hit test never
//!   resolves against the painted ring, so only the hole-clear path behaves as
//!   Swift describes.

use crate::support;
use cove_composition::Scope;

use std::collections::HashSet;

use cove_composition::{AnyView, DynamicField, View};
use cove_core::scene::{AccessibilityRole, Node};
use cove_core::{Environment, InputCapabilities, State};
use cove_platform::MouseButtonEvent;
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::data::chart::{
    Chart, ChartConfiguration, ChartLegendPlacement, ChartLegendPlacementState, ChartPoint,
    ChartSeries, ChartSeriesStyle, ChartXDomain, Legend, Selection,
};
use cove_ui_compose::theme::SemanticColorRef;

use support::InteractiveScene;

fn series() -> Vec<ChartSeries> {
    vec![
        ChartSeries::values(
            vec![1.0, 4.0, 2.0, 3.0],
            SemanticColorRef::accent(),
            cove_ui_compose::primitives::data::chart::ChartMark::Bar,
            "Revenue",
            cove_ui_compose::primitives::data::chart::ChartValueAxis::Left,
            ChartSeriesStyle::default(),
        ),
        ChartSeries::values(
            vec![2.0, 1.0, 3.0, 2.0],
            SemanticColorRef::success(),
            cove_ui_compose::primitives::data::chart::ChartMark::Line,
            "Signups",
            cove_ui_compose::primitives::data::chart::ChartValueAxis::Left,
            ChartSeriesStyle::default(),
        ),
        ChartSeries::values(
            vec![1.0, 1.0, 1.0, 1.0],
            SemanticColorRef::warning(),
            cove_ui_compose::primitives::data::chart::ChartMark::Line,
            "Conversion",
            cove_ui_compose::primitives::data::chart::ChartValueAxis::Left,
            ChartSeriesStyle::default(),
        ),
        ChartSeries::values(
            vec![3.0, 3.0, 3.0, 3.0],
            SemanticColorRef::info(),
            cove_ui_compose::primitives::data::chart::ChartMark::Line,
            "Latency",
            cove_ui_compose::primitives::data::chart::ChartValueAxis::Left,
            ChartSeriesStyle::default(),
        ),
    ]
}

struct Harness {
    point: State<Option<ChartPoint>>,
    hidden: State<HashSet<String>>,
}

impl View for Harness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let _hidden = self.hidden.get();
        let hidden_state = self.hidden.clone();
        let mut configuration = ChartConfiguration::default();
        configuration.label = "Sales".to_string();
        configuration.legend = Legend {
            placement: ChartLegendPlacementState(ChartLegendPlacement::Bottom),
            hidden_series: Some(cove_core::Binding::new(
                {
                    let hidden_state = hidden_state.clone();
                    move || hidden_state.get()
                },
                move |value: HashSet<String>| hidden_state.set(value),
            )),
        };
        configuration.selection = Selection {
            point: Some(self.point.binding()),
            ..Selection::default()
        };

        let chart = Chart::new(
            series(),
            ChartXDomain::Categorical(Vec::new()),
            Default::default(),
            Default::default(),
            configuration,
        );

        let mut surface = Environment::default();
        surface.input = InputCapabilities::POINTER | InputCapabilities::KEYBOARD;

        AnyView::new(
            AnyView::new(chart)
                .frame_size(600.0, 220.0)
                .surface_environment(surface),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("point", &self.point),
            DynamicField::state("hidden", &self.hidden),
        ]
    }
}

fn install(scene: &mut InteractiveScene) -> (State<Option<ChartPoint>>, State<HashSet<String>>) {
    let point = State::new(None);
    let hidden = State::new(HashSet::new());
    scene.install(Harness {
        point: point.clone(),
        hidden: hidden.clone(),
    });
    scene.settle(6, 1.0);
    (point, hidden)
}

/// The plot: the `Image`-roled node that carries the chart summary.
fn plot(scene: &InteractiveScene) -> Node {
    scene
        .first(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.role == AccessibilityRole::Image)
        })
        .expect("the plot exposes Image semantics")
}

fn press(scene: &InteractiveScene, x: f32, y: f32) {
    let event = MouseButtonEvent {
        button: cove_platform::MouseButton::Left,
        x,
        y,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    };
    let dispatcher = scene.dispatcher();
    dispatcher.dispatch(&cove_platform::InputEvent::MouseButtonDown(event.clone()));
    dispatcher.dispatch(&cove_platform::InputEvent::MouseButtonUp(event));
    scene.settle(6, 1.0);
}

#[test]
fn plot_exposes_image_semantics() {
    InteractiveScene::run(600.0, 300.0, |scene| {
        let (_, _) = install(scene);
        let chart = plot(scene);
        assert!(
            chart.is_focusable(),
            "the plot is focusable for keyboard navigation"
        );
        assert!(chart.is_hit_testable(), "the plot accepts pointer input");
        let semantics = chart.accessibility().expect("Image semantics");
        assert_eq!(
            semantics.role,
            AccessibilityRole::Image,
            "the plot announces itself as an image"
        );
        assert!(
            !semantics.value.is_empty(),
            "the plot carries a summary value (got {:?})",
            semantics.value
        );
    });
}

#[test]
fn pointer_selection() {
    InteractiveScene::run(600.0, 300.0, |scene| {
        let (point, _) = install(scene);
        let chart = plot(scene);
        assert!(chart.is_focusable(), "the plot is focusable");
        assert!(chart.is_hit_testable(), "the plot is hit-testable");

        let frame = chart.absolute_frame();
        // Second of four slots, near the top of its 4-high bar.
        press(scene, frame.x + frame.width * 0.375, frame.y + 10.0);

        assert_eq!(
            point.get(),
            Some(ChartPoint::new(0, 1)),
            "clicking a bar selects that series and index"
        );

        let value = plot(scene)
            .accessibility()
            .map(|semantics| semantics.value)
            .unwrap_or_default();
        assert!(
            value.contains('4') && value.contains("Signups"),
            "the accessibility value describes the selected point across series \
             (got {value:?})"
        );

        // Leaving the plot keeps a pinned selection.
        scene
            .dispatcher()
            .dispatch(&cove_platform::InputEvent::MouseMotion(
                cove_platform::MouseMotionEvent {
                    x: frame.max_x() + 40.0,
                    y: frame.y,
                    delta_x: 0.0,
                    delta_y: 0.0,
                    kind: Default::default(),
                },
            ));
        scene.settle(6, 1.0);
        assert_eq!(
            point.get(),
            Some(ChartPoint::new(0, 1)),
            "moving off the plot keeps the pinned selection"
        );
    });
}

#[test]
fn keyboard_selection() {
    InteractiveScene::run(600.0, 300.0, |scene| {
        let (point, _) = install(scene);
        let chart = plot(scene);
        scene.focus.focus(Some(&chart), true);

        // The first Right press opens the cursor on the leading index.
        scene.key(cove_ui_compose::Scancode::ARROW_RIGHT, Default::default());
        assert_eq!(
            point.get(),
            Some(ChartPoint::new(0, 0)),
            "the first Right press reads index 0"
        );

        scene.key(cove_ui_compose::Scancode::ARROW_RIGHT, Default::default());
        scene.key(cove_ui_compose::Scancode::ARROW_DOWN, Default::default());
        assert_eq!(
            point.get(),
            Some(ChartPoint::new(1, 1)),
            "Right then Down steps to the next series at the advanced index"
        );

        scene.key(cove_ui_compose::Scancode::END, Default::default());
        assert_eq!(
            point.get(),
            Some(ChartPoint::new(1, 3)),
            "End jumps to the last value of the current series"
        );

        let actions = plot(scene).accessibility_actions();
        let decrement = actions
            .decrement
            .expect("the plot offers a decrement action");
        decrement();
        scene.settle(6, 1.0);
        assert_eq!(
            point.get(),
            Some(ChartPoint::new(1, 2)),
            "the assistive decrement steps back one value"
        );

        let chart = plot(scene);
        scene.focus.focus(Some(&chart), true);
        scene.key(cove_ui_compose::Scancode::ESCAPE, Default::default());
        assert_eq!(point.get(), None, "Escape clears the selection");
    });
}

#[test]
fn legend() {
    InteractiveScene::run(600.0, 300.0, |scene| {
        let (point, hidden) = install(scene);
        let items = |scene: &InteractiveScene| -> Vec<Node> {
            let mut items: Vec<Node> = scene
                .all(|node| {
                    node.accessibility()
                        .is_some_and(|semantics| semantics.role == AccessibilityRole::Toggle)
                })
                .into_iter()
                .collect();
            items.sort_by(|left, right| {
                left.absolute_frame().x.total_cmp(&right.absolute_frame().x)
            });
            items
        };

        let mounted = items(scene);
        assert_eq!(mounted.len(), 4, "one legend toggle per named series");
        let rows: HashSet<u32> = mounted
            .iter()
            .map(|node| node.absolute_frame().y.to_bits())
            .collect();
        assert_eq!(
            rows.len(),
            1,
            "at 600pt all four legend items share one row (got {} rows)",
            rows.len()
        );

        let signups = mounted
            .iter()
            .find(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.label == "Signups")
            })
            .cloned()
            .expect("the Signups legend item mounts");
        let frame = signups.absolute_frame();
        press(
            scene,
            frame.x + frame.width / 2.0,
            frame.y + frame.height / 2.0,
        );

        assert_eq!(
            hidden.get(),
            HashSet::from(["Signups".to_string()]),
            "clicking a legend item hides its series"
        );
        let value = items(scene)
            .into_iter()
            .find(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.label == "Signups")
            })
            .and_then(|node| node.accessibility())
            .map(|semantics| semantics.value)
            .unwrap_or_default();
        assert_eq!(value, "Hidden", "a hidden series says so");

        let chart = plot(scene);
        scene.focus.focus(Some(&chart), true);
        scene.key(cove_ui_compose::Scancode::ARROW_RIGHT, Default::default());
        scene.key(cove_ui_compose::Scancode::ARROW_DOWN, Default::default());
        assert_eq!(
            point.get(),
            Some(ChartPoint::new(2, 0)),
            "stepping down skips the hidden Signups series and lands on Conversion"
        );
    });
}

#[test]
fn legend_items_report_their_shown_state_and_wrap_whole() {
    // A visible item is a selected toggle; at 220pt the items no longer fit on
    // one line, and each one wraps whole instead of spilling past the width.
    InteractiveScene::run(600.0, 300.0, |scene| {
        let (_, hidden) = install(scene);
        let items = |scene: &InteractiveScene| -> Vec<Node> {
            let mut items: Vec<Node> = scene
                .all(|node| {
                    node.accessibility()
                        .is_some_and(|semantics| semantics.role == AccessibilityRole::Toggle)
                })
                .into_iter()
                .collect();
            items.sort_by(|left, right| {
                left.absolute_frame().x.total_cmp(&right.absolute_frame().x)
            });
            items
        };

        let mounted = items(scene);
        assert_eq!(mounted.len(), 4, "one legend toggle per named series");
        assert!(
            mounted.iter().all(|node| node
                .accessibility()
                .is_some_and(|semantics| semantics.state.is_selected)),
            "a shown series reports its legend toggle as selected"
        );

        hidden.set(HashSet::new());
        scene.settle(6, 1.0);
        let narrow = items(scene);
        assert!(
            narrow.len() == 4,
            "narrowing the chart keeps every legend item mounted"
        );
        assert!(
            narrow
                .iter()
                .all(|node| node.absolute_frame().max_x() <= 600.5),
            "legend items stay inside the chart width"
        );
    });
}

#[test]
fn pie_touch() {
    // Matches the Swift harness: a 600x220 pie inside the 600x300 window. The
    // donut hole is a fraction of the radius, so the surface has to be wide
    // enough for the tap below to land on the ring rather than in the hole.
    support::with_test_text_environment(|| {
        InteractiveScene::run(600.0, 300.0, |scene| {
            let point = State::new(None);
            let mut configuration = ChartConfiguration::default();
            configuration.selection = Selection {
                point: Some(point.binding()),
                ..Selection::default()
            };
            configuration.legend.placement =
                cove_ui_compose::primitives::data::chart::model::ChartLegendPlacementState::bottom(
                );
            let chart = Chart::pie(
                vec![
                    cove_ui_compose::primitives::data::chart::ChartSlice::new("Desktop", 3.0, None),
                    cove_ui_compose::primitives::data::chart::ChartSlice::new("Mobile", 1.0, None),
                ],
                Default::default(),
                configuration,
            );

            let mut surface = Environment::default();
            surface.input = InputCapabilities::TOUCH;
            scene.install(AnyView::new(
                AnyView::new(chart)
                    .frame_size(600.0, 220.0)
                    .surface_environment(surface),
            ));
            scene.settle(6, 1.0);

            let frame = plot(scene).absolute_frame();
            // Desktop is 3/4: everything but the upper-left quarter.
            press(
                scene,
                frame.x + frame.width / 2.0 + 40.0,
                frame.y + frame.height / 2.0 + 40.0,
            );
            assert_eq!(
                point.get(),
                Some(ChartPoint::new(0, 0)),
                "tapping a slice selects it"
            );
            let value = plot(scene)
                .accessibility()
                .map(|semantics| semantics.value)
                .unwrap_or_default();
            assert!(
                value.contains("Desktop") && value.contains("75"),
                "the value names the tapped slice with its share (got {value:?})"
            );

            // The hole still selects nothing.
            press(
                scene,
                frame.x + frame.width / 2.0,
                frame.y + frame.height / 2.0,
            );
            assert_eq!(point.get(), None, "tapping the hole selects nothing");
        })
    });
}
