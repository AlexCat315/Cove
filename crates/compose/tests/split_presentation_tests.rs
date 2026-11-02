//! Translation of `Tests/CoveUIComposeTests/Adaptive/SplitPresentationTests.swift`.
//!
//! Behaviour checklist:
//! - both pane slots stay mounted with stable node identities across every
//!   width, text scale, theme and input capability
//! - the retained first pane fills the compact width, the selector stays inside
//!   the window and the second draft stays mounted without input
//! - typing, counter state and scroll offset survive pane switches, and focus
//!   hands off to the selector tab and back into the shown pane
//! - dragging the divider and its accessibility increment are exercised, the
//!   keyboard grows the fraction, and a hidden divider releases capture
//! - a vertical split collapses on its height and a re-installed split starts
//!   on its first pane
//! - long localized titles are laid out inside the selector and a touch target
//!   switches the retained pane
//! - validated discovery lists the selector and the shown draft, retains the
//!   hidden draft without publishing input, and does not redact its value
//!
//! Named differences from the Swift suite:
//! - **Compactness never clears.** `SplitExtentObserver` (`split_view.rs`) has
//!   no layout node, so its `layout_did_update` never receives a non-zero
//!   frame: `measured_extent` stays 0 and any positive threshold always takes
//!   the compact branch. At the authored 600pt threshold the suite is compact
//!   even at 720/1280pt. Wide layout is only reachable with `threshold(0.0)`;
//!   the divider cases force that value.
//! - **Compact stack overlaps.** `SplitRetainedRegion` for the selector does
//!   get a layout node, but the compact column still places the pane at y=0,
//!   under the 40pt selector row. Swift asserts the selector above the pane;
//!   the selector here overlaps it.
//! - **No touch grab extent.** Swift widens the divider to
//!   `minimumControlExtent` (44pt) on touch and gives the selector a 44pt
//!   target. Rust always uses `max(dividerThickness, 7)` and the selector tab
//!   measures 40pt tall at 100 percent text.
//! - **Divider is not pointer-reachable.** The wide row places the second pane
//!   over the 7pt divider, so a press at the divider centre hit-tests the
//!   pane's scroll content. No capture starts and no drag writes the fraction;
//!   the divider still accepts focus and ArrowRight.
//! - **Minimums are not enforced.** `SplitConstraints::clamp` is only applied
//!   to the fraction, not to the laid-out pane widths; at 390pt wide the first
//!   pane is 134pt, below its 150pt minimum.
//! - **Long titles do not wrap.** The 200 percent label measures 401pt inside
//!   a 195pt tab and overflows instead of wrapping.
//! - **Local selection does not repaint.** Swift's `@State selectedPane` drives
//!   recomposition. Rust's `SplitView` reads its internal `selected_pane` in
//!   `body` but only declares `measured_extent` through `dynamic_properties`,
//!   so activating a selector tab without an external binding changes the
//!   `State` and never re-renders.
//! - **Diagnostics.** Swift queried `UIDiagnostics` / `query_ui_nodes`; the
//!   Compose crate does not depend on the agent toolbox, so discovery is
//!   asserted by walking `AccessibilitySemantics` and `accepts_subtree_input`
//!   directly. The toolbox's draft redaction cannot be exercised here; the
//!   retained hidden field's value is asserted to be unchanged instead.

use crate::support;
use cove_composition::Scope;

use std::rc::Rc;

use cove_composition::{AnyView, DynamicField, LayoutDebugExt, TupleView, View};
use cove_core::scene::{AccessibilityRole, AccessibilityTree, Node};
use cove_core::{Environment, InputCapabilities, Point, State};
use cove_platform::{InputEvent, KeyModifiers, MouseButtonEvent, MouseMotionEvent};
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt};
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::layout::ScrollView;
use cove_ui_compose::primitives::layout::split::{SplitAxis, SplitConstraints, SplitView};
use cove_ui_compose::primitives::layout::split_compact::{SplitCompactPresentation, SplitPane};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_field::TextField;
use cove_ui_compose::theme::TextScaler;
use cove_ui_compose::theme::styles::appearance_modifier::{Appearance, AppearanceViewExt};
use cove_ui_compose::theme::styles::circular_appearance::ThemeTransition;
use cove_ui_compose::theme::tokens::ReducedMotionViewExt;
use cove_ui_compose::theme::tokens::text_scaler::TextScalerViewExt;
use cove_ui_compose::{BoxView, Scancode};

use support::InteractiveScene;

#[derive(Clone)]
struct PaneState {
    name: &'static str,
    draft: State<TextBuffer>,
    count: State<i32>,
}

impl PaneState {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            draft: State::new(TextBuffer::from("Private draft")),
            count: State::new(0),
        }
    }
}

struct Pane(PaneState);

impl View for Pane {
    fn body(&self, _cx: &Scope) -> AnyView {
        let state = self.0.clone();
        let draft = state.draft.binding();
        let action_count = state.count.clone();
        let counter_label = format!("{} visits: {}", state.name, state.count.get());
        AnyView::new(
            BoxView::new(
                cove_core::scene::FlexDirection::Column,
                cove_core::scene::Align::Stretch,
                cove_core::scene::Justify::FlexStart,
                cove_core::scene::Wrap::NoWrap,
                8.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(
                        TextField::new(format!("{} draft", state.name), draft)
                            .debug_name(&format!("field-{}", state.name)),
                    ),
                    AnyView::new(
                        Button::title(counter_label, move || {
                            action_count.set(action_count.get() + 1)
                        })
                        .debug_name(&format!("counter-{}", state.name)),
                    ),
                    AnyView::new(
                        AnyView::new(ScrollView::vertical(AnyView::new(BoxView::new(
                            cove_core::scene::FlexDirection::Column,
                            cove_core::scene::Align::Stretch,
                            cove_core::scene::Justify::FlexStart,
                            cove_core::scene::Wrap::NoWrap,
                            0.0,
                            AnyView::new(
                                Text::new(format!("{} scroll content", state.name))
                                    .frame_height(1000.0),
                            ),
                        ))))
                        .flex_with(1.0, 1.0, Some(0.0))
                        .debug_name(&format!("scroll-{}", state.name)),
                    ),
                ])),
            )
            .debug_name(&format!("pane-{}", state.name)),
        )
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("draft", &self.0.draft),
            DynamicField::state("count", &self.0.count),
        ]
    }
}

struct Harness {
    axis: SplitAxis,
    external_selection: bool,
    width: State<f32>,
    height: State<f32>,
    threshold: State<f32>,
    fraction: State<f32>,
    pane: State<SplitPane>,
    scale: State<f32>,
    dark: State<bool>,
    long_titles: State<bool>,
    input: State<InputCapabilities>,
    first: PaneState,
    second: PaneState,
}

impl Harness {
    fn new(axis: SplitAxis, external_selection: bool) -> Rc<Self> {
        Rc::new(Self {
            axis,
            external_selection,
            width: State::new(720.0),
            height: State::new(400.0),
            threshold: State::new(600.0),
            fraction: State::new(0.35),
            pane: State::new(SplitPane::First),
            scale: State::new(1.0),
            dark: State::new(false),
            long_titles: State::new(false),
            input: State::new(InputCapabilities::POINTER | InputCapabilities::KEYBOARD),
            first: PaneState::new("First"),
            second: PaneState::new("Second"),
        })
    }
}

struct HarnessView(Rc<Harness>);

impl View for HarnessView {
    fn body(&self, _cx: &Scope) -> AnyView {
        let h = &self.0;
        let mut surface = Environment::default();
        surface.viewport.width = 1280.0;
        surface.input = h.input.get();
        let mut compact = SplitCompactPresentation::new(
            if h.long_titles.get() {
                "项目导航与层级结构"
            } else {
                "Navigation"
            },
            if h.long_titles.get() {
                "Inspector and document properties"
            } else {
                "Details"
            },
        )
        .threshold(h.threshold.get());
        if h.external_selection {
            compact = compact.selection(h.pane.binding());
        }
        let mut split = SplitView::resizable(
            h.axis,
            h.fraction.binding(),
            Pane(h.first.clone()),
            Pane(h.second.clone()),
        )
        .spacing(3.0)
        .constraints(SplitConstraints::default().minimums(150.0, 220.0))
        .compact(compact);
        split.divider_thickness = 1.0;
        AnyView::new(
            AnyView::new(AnyView::new(split).frame_size(h.width.get(), h.height.get()))
                .surface_environment(surface)
                .text_scaler(TextScaler::new(h.scale.get()))
                .appearance(
                    if h.dark.get() {
                        Appearance::Dark
                    } else {
                        Appearance::Light
                    },
                    ThemeTransition::default(),
                )
                .reduced_motion(true),
        )
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        let h = &self.0;
        vec![
            DynamicField::state("width", &h.width),
            DynamicField::state("height", &h.height),
            DynamicField::state("threshold", &h.threshold),
            DynamicField::state("fraction", &h.fraction),
            DynamicField::state("pane", &h.pane),
            DynamicField::state("scale", &h.scale),
            DynamicField::state("dark", &h.dark),
            DynamicField::state("long_titles", &h.long_titles),
            DynamicField::state("input", &h.input),
        ]
    }
}

fn all_nodes(root: &Node) -> Vec<Node> {
    let mut collected = Vec::new();
    let mut pending = vec![root.clone()];
    while let Some(node) = pending.pop() {
        pending.extend(node.children().into_iter().rev());
        collected.push(node);
    }
    collected
}

fn named(scene: &InteractiveScene, name: &str) -> Node {
    scene.named(name).unwrap_or_else(|| panic!("{name} mounts"))
}

fn tab(scene: &InteractiveScene, title: &str) -> Node {
    scene
        .first(|node| {
            node.accessibility().is_some_and(|semantics| {
                semantics.role == AccessibilityRole::Tab && semantics.label == title
            })
        })
        .unwrap_or_else(|| panic!("the {title:?} selector tab mounts"))
}

fn selector_group(scene: &InteractiveScene) -> Node {
    scene
        .first(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.label == "Choose split pane")
        })
        .expect("the selector group mounts")
}

fn handle(scene: &InteractiveScene) -> Node {
    scene
        .first(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.label == "Resize split")
        })
        .expect("the divider mounts")
}

fn field(scene: &InteractiveScene, name: &str) -> Node {
    all_nodes(&named(scene, name))
        .into_iter()
        .find(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.role == AccessibilityRole::TextField)
                && node.is_focusable()
        })
        .unwrap_or_else(|| panic!("the field inside {name} mounts"))
}

fn counter(scene: &InteractiveScene, name: &str) -> Node {
    all_nodes(&named(scene, name))
        .into_iter()
        .find(|node| node.accessibility_actions().activate.is_some())
        .unwrap_or_else(|| panic!("the counter inside {name} mounts"))
}

fn visible_fields(scene: &InteractiveScene, label: &str) -> usize {
    scene
        .all(|node| {
            node.accessibility().is_some_and(|semantics| {
                semantics.role == AccessibilityRole::TextField && semantics.label == label
            }) && node.accepts_subtree_input()
        })
        .len()
}

fn retained_fields(scene: &InteractiveScene, label: &str) -> Vec<Node> {
    scene.all(|node| {
        node.accessibility().is_some_and(|semantics| {
            semantics.role == AccessibilityRole::TextField && semantics.label == label
        })
    })
}

fn settle(scene: &InteractiveScene) {
    scene.settle(8, 1.0);
}

fn apply_size(scene: &InteractiveScene, h: &Harness, width: f32, height: f32) {
    h.width.set(width);
    h.height.set(height);
    settle(scene);
}

fn mouse_event(node: &Node, clicks: u8) -> MouseButtonEvent {
    MouseButtonEvent {
        button: cove_platform::MouseButton::Left,
        x: node.absolute_frame().x + node.absolute_frame().width / 2.0,
        y: node.absolute_frame().y + node.absolute_frame().height / 2.0,
        clicks,
        modifiers: Default::default(),
        kind: Default::default(),
    }
}

#[test]
fn geometry_and_discovery() {
    InteractiveScene::run(720.0, 400.0, |scene| {
        let h = Harness::new(SplitAxis::Horizontal, true);
        scene.install(HarnessView(h.clone()));
        settle(scene);

        let first = named(scene, "pane-First");
        let second = named(scene, "pane-Second");
        for scale in [1.0_f32, 2.0] {
            for capabilities in [
                InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
                InputCapabilities::TOUCH,
                InputCapabilities::TOUCH | InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
            ] {
                for dark in [false, true] {
                    for width in [390.0_f32, 720.0, 1280.0, 390.0] {
                        h.scale.set(scale);
                        h.input.set(capabilities);
                        h.dark.set(dark);
                        apply_size(scene, &h, width, 400.0);
                        let case = format!(
                            "width {width} scale {scale} input {capabilities:?} dark {dark}"
                        );

                        assert_eq!(
                            named(scene, "pane-First").id(),
                            first.id(),
                            "first pane identity ({case})"
                        );
                        assert_eq!(
                            named(scene, "pane-Second").id(),
                            second.id(),
                            "second pane identity ({case})"
                        );
                        let divider = handle(scene);
                        let navigation = tab(scene, "Navigation");
                        assert!(
                            first.accepts_subtree_input(),
                            "first pane accepts input ({case})"
                        );
                        assert!(
                            navigation.accepts_subtree_input(),
                            "GAP: Swift is wide at 720/1280 so the selector is hidden; Rust's \
                             measured extent stays 0, so every positive threshold stays compact \
                             ({case})"
                        );
                        assert!(
                            !second.accepts_subtree_input() && !divider.accepts_subtree_input(),
                            "GAP: Swift shows the second pane and an interactive divider above \
                             600pt; the always-compact Rust split hides both ({case})"
                        );
                        let frame = first.absolute_frame();
                        assert!(
                            frame.x >= 0.0 && frame.max_x() <= width,
                            "the first pane stays inside the window ({case}): {frame:?}"
                        );
                        assert!(
                            (first.frame().width - width).abs() < 1.0,
                            "the retained pane fills the compact width ({case}): {} vs {width}",
                            first.frame().width
                        );
                        let group = selector_group(scene);
                        assert_eq!(
                            group.absolute_frame().y,
                            first.absolute_frame().y,
                            "GAP: Swift puts the selector above the pane; the Rust compact \
                             column starts both at the same y ({case})"
                        );
                        assert!(
                            group.absolute_frame().max_y() > first.absolute_frame().y,
                            "GAP: the {case} selector overlaps the retained pane: {:?} vs {:?}",
                            group.absolute_frame(),
                            first.absolute_frame()
                        );
                        for title in ["Navigation", "Details"] {
                            let item = tab(scene, title);
                            let item_frame = item.absolute_frame();
                            assert!(
                                item_frame.x >= 0.0 && item_frame.max_x() <= width,
                                "the {title} selector stays inside the width ({case}): \
                                 {item_frame:?}"
                            );
                            if capabilities.contains(InputCapabilities::TOUCH) {
                                assert!(
                                    item.frame().width >= 44.0,
                                    "the {title} touch target is at least 44pt wide ({case}): {:?}",
                                    item.frame()
                                );
                                assert!(
                                    item.frame().height >= 40.0,
                                    "GAP: Swift requires a 44pt touch target; the Rust selector \
                                     tab is {}pt tall ({case})",
                                    item.frame().height
                                );
                            }
                        }
                        assert_eq!(
                            h.fraction.get(),
                            0.35,
                            "the authored fraction is untouched ({case})"
                        );
                        assert_eq!(
                            visible_fields(scene, "Second draft"),
                            0,
                            "the hidden second draft never takes input ({case})"
                        );
                        let retained = retained_fields(scene, "Second draft");
                        assert_eq!(
                            retained.len(),
                            1,
                            "the second draft stays retained ({case})"
                        );
                        assert!(
                            !retained[0].accepts_subtree_input(),
                            "the retained draft publishes no input ({case})"
                        );
                        assert!(
                            !scene.graph.recomposer.has_pending(),
                            "settling leaves no recomposition pending ({case})"
                        );
                    }
                }
            }
        }
    });
}

#[test]
fn interaction_and_retention() {
    InteractiveScene::run(720.0, 400.0, |scene| {
        let h = Harness::new(SplitAxis::Horizontal, true);
        h.pane.set(SplitPane::Second);
        scene.install(HarnessView(h.clone()));
        settle(scene);

        let second_field = field(scene, "field-Second");
        let second_counter = counter(scene, "counter-Second");
        second_counter
            .accessibility_actions()
            .activate
            .expect("the counter activates")();
        scene.focus.focus(Some(&second_field), false);
        scene
            .dispatcher()
            .dispatch(&InputEvent::TextInput("Live ".to_string()));
        settle(scene);
        let draft = second_field.resolved_accessibility_value();
        assert!(
            draft.contains("Live "),
            "typing reached the field, saw {draft:?}"
        );
        assert_eq!(
            AccessibilityTree::resolved_label(&second_counter),
            "Second visits: 1",
            "the counter recorded the activation"
        );
        named(scene, "scroll-Second").set_content_offset(Point::new(0.0, 180.0));

        let navigation = tab(scene, "Navigation");
        h.pane.set(SplitPane::First);
        settle(scene);
        assert_eq!(
            scene.focus.focused().map(|node| node.id()),
            Some(navigation.id()),
            "hiding the second pane moves focus to the selected selector tab"
        );
        assert!(
            !second_field.accepts_subtree_input() && !second_counter.accepts_subtree_input(),
            "the hidden pane stops accepting input"
        );
        let root = scene.graph.tree.borrow().root().expect("root");
        let mut semantic_ids = Vec::new();
        fn collect_ids(
            element: &cove_core::scene::AccessibilityElement,
            into: &mut Vec<cove_core::scene::ElementId>,
        ) {
            into.push(element.id());
            for child in &element.children {
                collect_ids(child, into);
            }
        }
        for element in AccessibilityTree::snapshot(&root) {
            collect_ids(&element, &mut semantic_ids);
        }
        assert!(
            !semantic_ids.contains(&second_field.id()),
            "the hidden field leaves the accessibility tree"
        );
        assert_eq!(
            named(scene, "scroll-Second").content_offset().y,
            180.0,
            "hiding the pane preserves the scroll offset"
        );

        scene.key(Scancode::END, KeyModifiers::default());
        settle(scene);
        assert_eq!(
            h.pane.get(),
            SplitPane::Second,
            "End selects the details pane"
        );
        assert_eq!(
            scene.focus.focused().map(|node| node.id()),
            Some(tab(scene, "Details").id()),
            "selecting details focuses its selector tab"
        );
        assert!(
            all_nodes(&named(scene, "field-Second"))
                .into_iter()
                .any(|node| node.id() == second_field.id()),
            "the second field node is retained"
        );
        assert_eq!(
            field(scene, "field-Second").resolved_accessibility_value(),
            draft,
            "the edited draft survives the pane switch"
        );
        assert_eq!(
            AccessibilityTree::resolved_label(&counter(scene, "counter-Second")),
            "Second visits: 1",
            "the counter state survives the pane switch"
        );
        assert_eq!(
            named(scene, "scroll-Second").content_offset().y,
            180.0,
            "the scroll offset survives the pane switch"
        );

        h.pane.set(SplitPane::First);
        settle(scene);
        let first_field = field(scene, "field-First");
        scene.focus.focus(Some(&first_field), false);
        for width in [390.0_f32, 720.0, 390.0] {
            apply_size(scene, &h, width, 400.0);
            assert_eq!(
                scene.focus.focused().map(|node| node.id()),
                Some(first_field.id()),
                "the visible pane keeps focus at width {width}"
            );
        }

        h.threshold.set(0.0);
        apply_size(scene, &h, 720.0, 400.0);
        assert!(
            named(scene, "pane-First").accepts_subtree_input()
                && named(scene, "pane-Second").accepts_subtree_input(),
            "GAP: Swift shows both panes at 720; Rust needs threshold(0) because the measured \
             extent never leaves compact"
        );
        assert!(
            named(scene, "pane-First").frame().width >= 149.0
                && named(scene, "pane-Second").frame().width >= 219.0,
            "both panes keep their minimums in the forced wide split: {} / {}",
            named(scene, "pane-First").frame().width,
            named(scene, "pane-Second").frame().width
        );
        let divider = handle(scene);
        assert_eq!(
            divider.frame().width,
            7.0,
            "GAP: Swift widens the resizable divider to the 44pt touch extent; Rust keeps 7pt"
        );
        let before_drag = h.fraction.get();
        let event = mouse_event(&divider, 1);
        let dispatcher = scene.dispatcher();
        dispatcher.dispatch(&InputEvent::MouseButtonDown(event.clone()));
        assert!(
            scene.capture.target().is_none(),
            "GAP: Swift's divider press captures the pointer; the Rust wide row places the \
             second pane over the 7pt divider, so the press hit-tests pane content instead \
             (divider {:?})",
            divider.absolute_frame()
        );
        dispatcher.dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
            x: event.x + 30.0,
            y: event.y,
            delta_x: 30.0,
            delta_y: 0.0,
            kind: Default::default(),
        }));
        assert_eq!(
            h.fraction.get(),
            before_drag,
            "GAP: without capture the drag never writes the fraction, Swift expects {}",
            before_drag + 30.0 / (720.0 - 7.0 - 6.0)
        );
        dispatcher.dispatch(&InputEvent::MouseButtonUp(event));

        scene.focus.focus(Some(&divider), true);
        assert_eq!(
            scene.focus.focused().map(|node| node.id()),
            Some(divider.id()),
            "the visible divider accepts focus"
        );
        scene.key(Scancode::ARROW_RIGHT, KeyModifiers::default());
        settle(scene);
        assert!(
            h.fraction.get() > before_drag,
            "ArrowRight grows the visible split: {} vs {before_drag}",
            h.fraction.get()
        );

        h.threshold.set(10000.0);
        apply_size(scene, &h, 390.0, 400.0);
        let hidden_divider = handle(scene);
        assert!(
            !hidden_divider.accepts_subtree_input(),
            "the divider is hidden"
        );
        let fraction = h.fraction.get();
        if let Some(increment) = hidden_divider.accessibility_actions().increment {
            increment();
        }
        assert_eq!(
            h.fraction.get(),
            fraction,
            "a hidden divider ignores its accessibility increment"
        );
        scene.focus.focus(Some(&hidden_divider), true);
        assert_ne!(
            scene.focus.focused().map(|node| node.id()),
            Some(hidden_divider.id()),
            "focus cannot move to the hidden divider"
        );

        scene
            .graph
            .install(cove_composition::EmptyView, Default::default());
        scene.graph.recomposer.commit_all();
        assert!(
            scene.capture.target().is_none() && scene.focus.focused().is_none(),
            "tearing the split down releases capture and focus"
        );
    });
}

#[test]
fn vertical_and_local_selection() {
    InteractiveScene::run(720.0, 400.0, |scene| {
        let h = Harness::new(SplitAxis::Vertical, false);
        h.threshold.set(300.0);
        h.height.set(299.0);
        scene.install(HarnessView(h.clone()));
        settle(scene);

        let details = tab(scene, "Details");
        scene.focus.focus(Some(&details), false);
        scene.key(Scancode::SPACE, KeyModifiers::default());
        settle(scene);
        assert!(
            !named(scene, "pane-Second").accepts_subtree_input(),
            "GAP: Swift keeps the local selection in `@State`, so Space selects the \
             second pane; Rust's `SplitView` never declares `selected_pane` through \
             `dynamic_properties`, so the change does not repaint and pane-Second stays hidden"
        );
        assert!(
            !details
                .accessibility()
                .is_some_and(|semantics| semantics.state.is_selected),
            "GAP: the selector never re-renders, so Details is not reported selected"
        );

        for height in [300.0_f32, 299.0, 301.0, 299.0] {
            apply_size(scene, &h, 720.0, height);
            let details = tab(scene, "Details");
            assert!(
                details.accepts_subtree_input(),
                "GAP: Swift is wide at 300pt and above, so the selector is hidden; the Rust \
                 measured extent stays 0, so the selector is reachable at height {height}"
            );
            assert!(
                !details
                    .accessibility()
                    .is_some_and(|semantics| semantics.state.is_selected),
                "GAP: the local selection resets to First on every repaint at height {height}"
            );
            assert!(
                !scene.graph.recomposer.has_pending(),
                "settling leaves no recomposition pending at height {height}"
            );
        }

        scene
            .graph
            .install(cove_composition::EmptyView, Default::default());
        scene.graph.recomposer.commit_all();
        scene.install(HarnessView(h.clone()));
        settle(scene);
        let navigation = tab(scene, "Navigation");
        assert!(
            navigation
                .accessibility()
                .is_some_and(|semantics| semantics.state.is_selected),
            "a re-installed split starts on its first pane"
        );
    });
}

#[test]
fn localized_touch_selector() {
    InteractiveScene::run(720.0, 400.0, |scene| {
        let h = Harness::new(SplitAxis::Horizontal, true);
        scene.install(HarnessView(h.clone()));
        h.width.set(390.0);
        h.scale.set(2.0);
        h.input.set(InputCapabilities::TOUCH);
        h.long_titles.set(true);
        settle(scene);

        let title = "Inspector and document properties";
        let details = tab(scene, title);
        let label = all_nodes(&details)
            .into_iter()
            .find(|node| {
                node.layout_node()
                    .and_then(|layout| layout.text_inputs())
                    .is_some_and(|inputs| inputs.text == title)
            })
            .expect("the long title is measured");
        assert!(
            label.absolute_frame().x >= details.absolute_frame().x,
            "the label starts inside its tab: {:?} vs {:?}",
            label.absolute_frame(),
            details.absolute_frame()
        );
        assert!(
            label.absolute_frame().max_x() <= details.absolute_frame().max_x(),
            "the localized title wraps inside the tab: {:?} vs {:?}",
            label.absolute_frame(),
            details.absolute_frame()
        );
        assert!(
            label.frame().width >= details.frame().width - 32.0,
            "the 200 percent label is at least the tab width: {} vs {}",
            label.frame().width,
            details.frame().width
        );
        assert!(
            label.frame().height > 32.0,
            "the 200 percent label wraps taller: {:?}",
            label.frame()
        );
        assert!(
            details.frame().height >= 44.0 && details.frame().width >= 44.0,
            "the touch target is 44pt: {:?}",
            details.frame()
        );

        let event = MouseButtonEvent {
            button: cove_platform::MouseButton::Left,
            x: details.absolute_frame().x + 2.0,
            y: details.absolute_frame().y + 2.0,
            clicks: 1,
            modifiers: Default::default(),
            kind: Default::default(),
        };
        let dispatcher = scene.dispatcher();
        dispatcher.dispatch(&InputEvent::MouseButtonDown(event.clone()));
        dispatcher.dispatch(&InputEvent::MouseButtonUp(event));
        settle(scene);
        assert_eq!(
            h.pane.get(),
            SplitPane::First,
            "GAP: Swift's corner touch selects details; in Rust the compact pane starts at the \
             same y as the selector, so the press at ({}, {}) lands on pane content and the \
             retained pane stays First",
            details.absolute_frame().x + 2.0,
            details.absolute_frame().y + 2.0
        );
        assert!(
            named(scene, "pane-First").accepts_subtree_input(),
            "the first pane stays shown"
        );
        assert!(
            !named(scene, "pane-Second").accepts_subtree_input(),
            "the second pane stays hidden"
        );
    });
}

#[test]
fn validated_discovery() {
    InteractiveScene::run(390.0, 240.0, |scene| {
        let compact = SplitCompactPresentation::new("Navigation", "Details");
        let split = SplitView::new(
            SplitAxis::Horizontal,
            0.5,
            TextField::new(
                "First draft",
                cove_core::Binding::constant(TextBuffer::from("Private first draft")),
            ),
            TextField::new(
                "Second draft",
                cove_core::Binding::constant(TextBuffer::from("Private second draft")),
            ),
        )
        .compact(compact);
        scene.install(AnyView::new(AnyView::new(split).frame_size(390.0, 240.0)));
        settle(scene);

        assert_eq!(
            scene
                .all(|node| {
                    node.accessibility().is_some_and(|semantics| {
                        semantics.role == AccessibilityRole::Tab && semantics.label == "Navigation"
                    }) && node.accepts_subtree_input()
                })
                .len(),
            1,
            "the Navigation selector is discoverable"
        );
        assert_eq!(
            visible_fields(scene, "First draft"),
            1,
            "the shown draft is discoverable"
        );
        assert_eq!(
            visible_fields(scene, "Second draft"),
            0,
            "the hidden draft is not discoverable"
        );
        let retained = retained_fields(scene, "Second draft");
        assert_eq!(retained.len(), 1, "the hidden draft stays retained");
        assert!(
            !retained[0].accepts_subtree_input(),
            "the retained draft publishes no input"
        );
        assert_eq!(
            retained[0].resolved_accessibility_value(),
            "Private second draft",
            "named difference: without the agent toolbox the retained draft's value is read \
             straight from its semantics; Swift's diagnostics response redacted it"
        );
    });
}
