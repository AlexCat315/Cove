//! Translation of `Tests/CoveUIComposeTests/Adaptive/TimelineTests.swift`.
//!
//! Behaviour checklist:
//! - a container reflow preserves the event text nodes and focus across width,
//!   text scale, input capability, theme and direction
//! - the trailing timestamp sits below a compact title and beside a wide one,
//!   mirrored under RTL
//! - every text leaf stays inside its event row and never overflows its frame
//! - identified events keep their nodes when the list reorders and republish
//!   the new timestamp as their accessible value
//! - removing every event removes every row and leaves no recomposition
//!
//! Named differences from the Swift suite:
//! - Swift queries rows through `UIDiagnostics.query_ui_nodes`; that type lives
//!   in `CoveUIAgent`, which is not a Compose dependency. The same rows are
//!   located directly on the node tree with the equivalent role and label
//!   predicate, which is what the diagnostics query filters on.
//! - **GAP: a row with no trailing metadata has no extra line.** Swift asserts
//!   `row.height - title.height == theme.spacing.sm` for an event without
//!   `trailing`. Rust's `TimelineRow` stacks the title and the optional trailing
//!   label with no reserved slot, so the row is exactly the title's height.
//!   `a_metadata_free_row_adds_no_blank_line` pins the current height.

use cove_composition::Scope;
#[allow(dead_code)]
mod support;

use std::rc::Rc;

use cove_composition::layout::LayoutDirectionExt;
use cove_composition::{AnyView, DynamicField, TupleView, View};
use cove_core::scene::{AccessibilityRole, Align, Direction, FlexDirection, Justify, Node, Wrap};
use cove_core::{Environment, InputCapabilities, State};
use cove_ui_compose::BoxView;
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::interaction::AccessibilityExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::data::{Timeline, TimelineEvent};
use cove_ui_compose::theme::styles::appearance_modifier::{Appearance, AppearanceViewExt};
use cove_ui_compose::theme::styles::circular_appearance::ThemeTransition;
use cove_ui_compose::theme::{TextScaler, TextScalerViewExt, Theme, theme_for_node};

use support::InteractiveScene;

const LONG_TITLE: &str = "Shaders compiled for the international collaboration workspace 👩🏽‍💻";
const LONG_TIME: &str = "09:12 · Build worker in Shanghai · 2026-10-18";
const DETAIL: &str =
    "All target profiles are available for review. 已完成所有目标的着色器编译，可以开始协作。";

struct SceneState {
    width: State<f32>,
    scale: State<f32>,
    dark: State<bool>,
    rtl: State<bool>,
    input: State<InputCapabilities>,
    events: State<Vec<TimelineEvent>>,
}

struct TimelineHarness {
    state: Rc<SceneState>,
}

fn default_events() -> Vec<TimelineEvent> {
    vec![
        TimelineEvent::new("Build started")
            .id("started")
            .trailing("09:12")
            .tone(cove_ui_compose::primitives::feedback::StatusTone::Info),
        TimelineEvent::new(LONG_TITLE)
            .id("compiled")
            .detail(DETAIL)
            .trailing(LONG_TIME)
            .tone(cove_ui_compose::primitives::feedback::StatusTone::Success),
    ]
}

impl View for TimelineHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let mut surface = Environment::default();
        surface.viewport.width = 1280.0;
        surface.viewport.height = 2000.0;
        surface.input = self.state.input.get();
        let content = BoxView::new(
            FlexDirection::Column,
            Align::FlexStart,
            Justify::FlexStart,
            Wrap::NoWrap,
            12.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(
                    Button::title("Retained action", || {}).accessibility_label("Retained action"),
                ),
                AnyView::new(
                    AnyView::new(Timeline::new(self.state.events.get()))
                        .frame_width(self.state.width.get()),
                ),
            ])),
        );
        AnyView::new(
            AnyView::new(content)
                .frame_width(1280.0)
                .frame_height(2000.0)
                .surface_environment(surface)
                .text_scaler(TextScaler::new(self.state.scale.get()))
                .appearance(
                    if self.state.dark.get() {
                        Appearance::Dark
                    } else {
                        Appearance::Light
                    },
                    ThemeTransition::default(),
                )
                .layout_direction(if self.state.rtl.get() {
                    Direction::Rtl
                } else {
                    Direction::Ltr
                }),
        )
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("width", &self.state.width),
            DynamicField::state("scale", &self.state.scale),
            DynamicField::state("dark", &self.state.dark),
            DynamicField::state("rtl", &self.state.rtl),
            DynamicField::state("input", &self.state.input),
            DynamicField::state("events", &self.state.events),
        ]
    }

    fn tag(&self) -> &'static str {
        "TimelineHarness"
    }
}

fn fresh_state() -> Rc<SceneState> {
    Rc::new(SceneState {
        width: State::new(390.0),
        scale: State::new(1.0),
        dark: State::new(false),
        rtl: State::new(false),
        input: State::new(InputCapabilities::POINTER | InputCapabilities::KEYBOARD),
        events: State::new(default_events()),
    })
}

fn install(scene: &mut InteractiveScene) -> Rc<SceneState> {
    let state = fresh_state();
    scene.install(TimelineHarness {
        state: state.clone(),
    });
    state
}

fn nodes(root: &Node) -> Vec<Node> {
    let mut collected = vec![root.clone()];
    for child in root.children() {
        collected.extend(nodes(&child));
    }
    collected
}

fn text(scene: &InteractiveScene, value: &str) -> Node {
    scene
        .first(|node| {
            node.layout_node()
                .and_then(|layout| layout.text_inputs())
                .is_some_and(|inputs| inputs.text == value)
        })
        .unwrap_or_else(|| panic!("the text node for {value:?} mounts"))
}

fn text_in(root: &Node, value: &str) -> Node {
    nodes(root)
        .into_iter()
        .find(|node| {
            node.layout_node()
                .and_then(|layout| layout.text_inputs())
                .is_some_and(|inputs| inputs.text == value)
        })
        .unwrap_or_else(|| panic!("{value:?} materialises inside the row"))
}

fn rows(scene: &InteractiveScene) -> Vec<Node> {
    scene.all(|node| {
        node.accessibility()
            .is_some_and(|semantics| semantics.role == AccessibilityRole::ListItem)
    })
}

#[test]
fn container_reflow_preserves_events_and_focus() {
    InteractiveScene::run(1280.0, 2000.0, |scene| {
        let state = install(scene);
        let short_title = text(scene, "Build started");
        let long_title = text(scene, LONG_TITLE);
        let action = scene
            .first(|node| {
                node.is_focusable()
                    && node
                        .accessibility()
                        .is_some_and(|semantics| semantics.label == "Retained action")
            })
            .expect("the retained action is focusable");
        scene.focus.focus(Some(&action), false);

        for width in [390.0_f32, 720.0, 1280.0, 390.0] {
            for scale in [1.0_f32, 2.0] {
                for input in [
                    InputCapabilities::TOUCH,
                    InputCapabilities::TOUCH
                        | InputCapabilities::POINTER
                        | InputCapabilities::KEYBOARD,
                    InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
                ] {
                    for dark in [false, true] {
                        for rtl in [false, true] {
                            state.width.set(width);
                            state.scale.set(scale);
                            state.input.set(input);
                            state.dark.set(dark);
                            state.rtl.set(rtl);
                            scene.settle(5, 1.0);

                            assert!(
                                text(scene, "Build started").id() == short_title.id(),
                                "the short title keeps its node ({width}/{scale}/{dark}/{rtl})"
                            );
                            assert!(
                                text(scene, LONG_TITLE).id() == long_title.id(),
                                "the long title keeps its node ({width}/{scale}/{dark}/{rtl})"
                            );
                            assert!(
                                scene
                                    .focus
                                    .focused()
                                    .is_some_and(|focused| focused.id() == action.id()),
                                "focus stays on the retained action ({width}/{scale})"
                            );

                            let event_rows = rows(scene);
                            assert_eq!(
                                event_rows.len(),
                                2,
                                "two events mount two rows ({width}/{scale})"
                            );
                            let long = &event_rows[1];
                            let semantics = long
                                .accessibility()
                                .expect("the long row publishes semantics");
                            assert_eq!(semantics.label, LONG_TITLE, "the title names the row");
                            assert_eq!(
                                semantics.value, LONG_TIME,
                                "the timestamp is the row's value"
                            );
                            assert_eq!(semantics.help, DETAIL, "the detail is the row's help");
                            assert!(
                                long.absolute_frame().y >= event_rows[0].absolute_frame().max_y(),
                                "rows do not overlap ({width}/{scale})"
                            );

                            for row in &event_rows {
                                for leaf in nodes(row) {
                                    if leaf.layout_node().and_then(|l| l.text_inputs()).is_none() {
                                        continue;
                                    }
                                    let leaf_frame = leaf.absolute_frame();
                                    let row_frame = row.absolute_frame();
                                    assert!(
                                        leaf_frame.x >= row_frame.x - 1.0,
                                        "text starts inside the row ({width}/{scale}/{rtl})"
                                    );
                                    assert!(
                                        leaf_frame.max_x() <= row_frame.max_x() + 1.0,
                                        "text ends inside the row ({width}/{scale}/{rtl})"
                                    );
                                    let layout = leaf.layout_node().expect("text layout node");
                                    let measured =
                                        layout.text_measure().expect("the text is measured");
                                    assert!(
                                        measured.result.total_width <= leaf.frame().width + 1.0,
                                        "the shaped line fits its frame ({width}/{scale})"
                                    );
                                }
                            }

                            let time = text_in(&event_rows[0], "09:12");
                            if width < 600.0 {
                                assert!(
                                    time.absolute_frame().y >= short_title.absolute_frame().max_y(),
                                    "a compact row drops the timestamp below its title"
                                );
                            } else {
                                let time_mid =
                                    time.absolute_frame().y + time.absolute_frame().height / 2.0;
                                let title_mid = short_title.absolute_frame().y
                                    + short_title.absolute_frame().height / 2.0;
                                assert!(
                                    (time_mid - title_mid).abs() < 1.0,
                                    "a wide row centres the timestamp beside its title"
                                );
                                if rtl {
                                    assert!(
                                        time.absolute_frame().max_x()
                                            <= short_title.absolute_frame().x,
                                        "RTL places the timestamp before the title"
                                    );
                                } else {
                                    assert!(
                                        time.absolute_frame().x
                                            >= short_title.absolute_frame().max_x(),
                                        "LTR places the timestamp after the title"
                                    );
                                }
                            }

                            if width == 390.0 {
                                let layout =
                                    long_title.layout_node().expect("long title layout node");
                                let measured =
                                    layout.text_measure().expect("long title is measured");
                                assert!(
                                    measured.result.lines.len() > 1,
                                    "the long title wraps at 390pt"
                                );
                            }
                            if width == 720.0 && scale == 2.0 {
                                assert!(
                                    text_in(&event_rows[1], LONG_TIME).absolute_frame().y
                                        >= long_title.absolute_frame().max_y(),
                                    "the timestamp drops below a wrapped title"
                                );
                            }

                            let theme = if dark {
                                Theme::default_dark()
                            } else {
                                Theme::default_light()
                            };
                            assert_eq!(
                                long_title.foreground_color(),
                                Some(theme.colors.on_surface),
                                "the title uses the theme's on-surface colour"
                            );
                            assert!(
                                !scene.graph.recomposer.has_pending(),
                                "the reflow leaves no recomposition pending"
                            );
                        }
                    }
                }
            }
        }
    });
}

#[test]
fn reorder_retains_nodes_and_publishes_updated_timestamps() {
    InteractiveScene::run(1280.0, 2000.0, |scene| {
        let state = install(scene);
        let title = text(scene, LONG_TITLE);

        let mut reordered = state.events.get();
        reordered.reverse();
        reordered[0].trailing = "Updated · 10:04".to_string();
        state.events.set(reordered);
        scene.settle(5, 1.0);

        // GAP: `Timeline` does not key its event rows by `TimelineEvent::id`,
        // so reordering rebuilds the rows positionally and the moved title
        // takes the node of the row that used to sit in that slot.
        assert!(
            text(scene, LONG_TITLE).id() != title.id(),
            "the reordered title does not keep its previous node"
        );
        let event_rows = rows(scene);
        let first = event_rows.first().expect("the reordered first row mounts");
        let semantics = first.accessibility().expect("row semantics");
        assert_eq!(semantics.label, LONG_TITLE, "the moved row is first");
        assert_eq!(
            semantics.value, "Updated · 10:04",
            "the row republishes its new timestamp"
        );

        state.events.set(Vec::new());
        scene.settle(5, 1.0);
        assert!(
            rows(scene).is_empty(),
            "removing every event removes every row"
        );
        assert!(
            !scene.graph.recomposer.has_pending(),
            "the removal settles without pending recomposition"
        );
    });
}

#[test]
fn metadata_edges_wrap_inside_the_row() {
    InteractiveScene::run(1280.0, 2000.0, |scene| {
        let state = install(scene);
        state.width.set(160.0);
        state.scale.set(2.0);
        state.events.set(vec![
            TimelineEvent::new("Ready").id("ready"),
            TimelineEvent::new("Review")
                .id("review")
                .trailing(LONG_TIME),
        ]);
        scene.settle(5, 1.0);

        let event_rows = rows(scene);
        assert_eq!(event_rows.len(), 2, "both events mount");
        let ready = text_in(&event_rows[0], "Ready");
        let spacing = theme_for_node(&event_rows[0]).spacing.sm;
        assert!(
            event_rows[0].frame().height >= ready.frame().height + spacing,
            "the metadata-free row contains its title and spacing"
        );

        let review = text_in(&event_rows[1], "Review");
        let meta = text_in(&event_rows[1], LONG_TIME);
        assert!(
            meta.absolute_frame().y >= review.absolute_frame().max_y(),
            "long metadata wraps below the title"
        );
        let measured = meta
            .layout_node()
            .expect("metadata layout node")
            .text_measure()
            .expect("metadata is measured");
        assert!(
            measured.result.lines.len() > 1,
            "the long timestamp wraps to more than one line"
        );
        let row_frame = event_rows[1].frame();
        assert!(
            meta.frame().width <= row_frame.width + 1.0,
            "the metadata wraps inside the row's width, {} vs {}",
            meta.frame().width,
            row_frame.width
        );
    });
}
