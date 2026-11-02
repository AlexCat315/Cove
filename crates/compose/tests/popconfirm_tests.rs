//! Translation of `Tests/CoveUIComposeTests/PopconfirmTests.swift`.
//!
//! Behaviour checklist:
//! - confirm executes once, cancel and dismissal do not, and keyboard focus
//!   returns to the trigger
//! - every placement respects logical direction, flips at edges only when asked
//!   and stays inside the viewport
//! - three widths, scaled text, themes, direction and input keep real buttons
//!   inside an opaque painted bubble
//! - open content updates immediately; disabling and teardown remove portals
//!   and focus
//! - AI tools discover the dialog and action labels and follow its dismissal
//! - every explicit side is retained by the actual portal
//! - a generic `Popover` uses the same bubble surface
//!
//! Named differences:
//! - Diagnostics: Swift queried `UIDiagnostics` (`query_ui_nodes`); the Compose
//!   crate does not depend on the agent toolbox, so discovery counts dialog
//!   accessibility nodes directly.
//! - Surface: Swift's bubble is a `anchored-surface` debug-named node with its
//!   own painter; the Rust generic `Popover` portal is named `portal-N` and the
//!   slot has no painter of its own.
//! - Trigger and action buttons carry no accessibility label of their own; they
//!   are found by `AccessibilityTree::resolved_label`.

use cove_composition::Scope;
mod support;

use std::cell::Cell;
use std::rc::Rc;

use cove_composition::layout::LayoutDirectionExt;
use cove_composition::{AnyView, DynamicField, TupleView, View, ViewExt};
use cove_core::scene::{AccessibilityRole, Direction, Node};
use cove_core::{Environment, InputCapabilities, Rect, Size, State};
use cove_platform::KeyModifiers;
use cove_ui_compose::Scancode;
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::layout::{AbsolutePositionModifier, LayoutFrameExt};
use cove_ui_compose::primitives::controls::button::{Button, ButtonRole};
use cove_ui_compose::primitives::media::ui_icon_catalog;
use cove_ui_compose::primitives::overlays::anchored::{
    PopoverEdge, PopoverGeometry, PopoverOverflow, PopoverPlacement,
};
use cove_ui_compose::primitives::overlays::popconfirm::Popconfirm;
use cove_ui_compose::primitives::overlays::popover::Popover;
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::TextScaler;
use cove_ui_compose::theme::styles::appearance_modifier::{Appearance, AppearanceViewExt};
use cove_ui_compose::theme::tokens::ReducedMotionViewExt;
use cove_ui_compose::theme::tokens::text_scaler::TextScalerViewExt;

use support::InteractiveScene;

/// The Swift `Harness`: every knob is observed state.
struct Harness {
    count: Rc<Cell<u32>>,
    mounted: State<bool>,
    enabled: State<bool>,
    placement: State<PopoverPlacement>,
    width: State<f32>,
    height: State<f32>,
    x: State<f32>,
    y: State<f32>,
    scale: State<f32>,
    dark: State<bool>,
    rtl: State<bool>,
    input: State<InputCapabilities>,
    title: State<String>,
}

impl Harness {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            count: Rc::new(Cell::new(0)),
            mounted: State::new(true),
            enabled: State::new(true),
            placement: State::new(PopoverPlacement::Bottom),
            width: State::new(640.0),
            height: State::new(480.0),
            x: State::new(220.0),
            y: State::new(200.0),
            scale: State::new(1.0),
            dark: State::new(false),
            rtl: State::new(false),
            input: State::new(InputCapabilities::POINTER | InputCapabilities::KEYBOARD),
            title: State::new("Delete this project?".to_string()),
        })
    }
}

struct HarnessView(Rc<Harness>);

impl View for HarnessView {
    fn body(&self, _cx: &Scope) -> AnyView {
        let h = &self.0;
        let mut surface = Environment::default();
        surface.viewport.width = h.width.get();
        surface.viewport.height = h.height.get();
        surface.input = h.input.get();
        let content = if h.mounted.get() {
            let count = h.count.clone();
            AnyView::new(
                Popconfirm::new(
                    "Delete project",
                    Some(ui_icon_catalog::action::trash()),
                    h.title.get(),
                    "This removes the project and its local history.",
                    "Remove",
                    "Keep",
                    ButtonRole::Destructive,
                    h.placement.get(),
                    h.enabled.get(),
                    move || count.set(count.get() + 1),
                )
                .modifier(AbsolutePositionModifier::new(
                    Some(h.x.get()),
                    Some(h.y.get()),
                    None,
                    None,
                )),
            )
        } else {
            AnyView::new(TupleView::new(Vec::new()))
        };
        AnyView::new(
            AnyView::new(LayerRoot::new(content))
                .frame_size(h.width.get(), h.height.get())
                .surface_environment(surface)
                .text_scaler(TextScaler::new(h.scale.get()))
                .appearance(
                    if h.dark.get() {
                        Appearance::Dark
                    } else {
                        Appearance::Light
                    },
                    Default::default(),
                )
                .layout_direction(if h.rtl.get() {
                    Direction::Rtl
                } else {
                    Direction::Ltr
                })
                .reduced_motion(true),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        let h = &self.0;
        vec![
            DynamicField::state("mounted", &h.mounted),
            DynamicField::state("enabled", &h.enabled),
            DynamicField::state("placement", &h.placement),
            DynamicField::state("width", &h.width),
            DynamicField::state("height", &h.height),
            DynamicField::state("x", &h.x),
            DynamicField::state("y", &h.y),
            DynamicField::state("scale", &h.scale),
            DynamicField::state("dark", &h.dark),
            DynamicField::state("rtl", &h.rtl),
            DynamicField::state("input", &h.input),
            DynamicField::state("title", &h.title),
        ]
    }
}

fn button(scene: &InteractiveScene, label: &str) -> Option<Node> {
    scene.first(|node| {
        node.accessibility()
            .is_some_and(|semantics| semantics.role == AccessibilityRole::Button)
            && cove_core::scene::AccessibilityTree::resolved_label(node) == label
    })
}

fn panel(scene: &InteractiveScene) -> Option<Node> {
    scene.first(|node| {
        matches!(
            support::debug_name(node).as_deref(),
            Some("anchored-surface") | Some("touch-overlay-sheet")
        )
    })
}

fn settle(scene: &InteractiveScene) {
    scene.settle(8, 1.0);
}

/// Swift's suite lays out at the harness's own window size.
fn settle_window(scene: &InteractiveScene, h: &Harness) {
    for _ in 0..8 {
        scene.graph.recomposer.commit_all();
        scene.graph.compute_layout(h.width.get(), h.height.get());
        cove_core::scene::AnimatorScheduler::current().tick(1.0);
    }
    scene.graph.recomposer.commit_all();
    scene.graph.compute_layout(h.width.get(), h.height.get());
}

fn with_harness(body: impl FnOnce(&InteractiveScene, &Rc<Harness>)) {
    InteractiveScene::run(640.0, 480.0, |scene| {
        let harness = Harness::new();
        scene.install(HarnessView(harness.clone()));
        settle(scene);
        body(scene, &harness);
    });
}

fn trigger(scene: &InteractiveScene) -> Node {
    button(scene, "Delete project").expect("the popconfirm trigger is mounted")
}

fn dialog_count(scene: &InteractiveScene, title: &str) -> usize {
    scene
        .all(|node| {
            node.accessibility().is_some_and(|semantics| {
                semantics.role == AccessibilityRole::Dialog && semantics.label == title
            })
        })
        .len()
}

#[test]
fn lifecycle() {
    with_harness(|scene, h| {
        let trigger = trigger(scene);
        let focused = || scene.focus.focused().map(|node| node.id());
        let expanded = |trigger: &Node| {
            trigger
                .accessibility()
                .and_then(|semantics| semantics.state.is_expanded)
        };
        assert_eq!(
            expanded(&trigger),
            Some(false),
            "a closed trigger reports collapsed"
        );

        scene.focus.focus(Some(&trigger), true);
        scene.key(Scancode::RETURN, KeyModifiers::default());
        settle(scene);
        let keep = button(scene, "Keep").expect("Return opens the bubble");
        let remove = button(scene, "Remove").expect("the bubble offers Remove");
        assert_eq!(expanded(&trigger), Some(true));
        assert_eq!(focused(), Some(keep.id()), "focus moves into the bubble");
        scene.key(Scancode::TAB, KeyModifiers::default());
        settle(scene);
        assert_eq!(focused(), Some(remove.id()), "Tab cycles to Remove");
        scene.key(Scancode::TAB, KeyModifiers::default());
        settle(scene);
        assert_eq!(focused(), Some(keep.id()), "Tab cycles back to Keep");
        scene.key(Scancode::RETURN, KeyModifiers::default());
        settle(scene);
        assert_eq!(h.count.get(), 0, "Keep cancels");
        assert!(scene.portal.entries().is_empty());
        assert_eq!(
            focused(),
            Some(trigger.id()),
            "cancel returns focus to the trigger"
        );

        scene.click(&trigger, 1);
        settle(scene);
        let confirm = button(scene, "Remove")
            .and_then(|node| node.accessibility_actions().activate)
            .expect("Remove exposes activate");
        confirm();
        confirm();
        settle(scene);
        assert_eq!(h.count.get(), 1, "confirm runs exactly once");
        assert!(scene.portal.entries().is_empty());
        assert_eq!(expanded(&trigger), Some(false));

        scene.click(&trigger, 1);
        settle(scene);
        scene.key(Scancode::ESCAPE, KeyModifiers::default());
        settle(scene);
        assert_eq!(h.count.get(), 1);
        assert!(scene.portal.entries().is_empty(), "Escape dismisses");
        assert_eq!(
            focused(),
            Some(trigger.id()),
            "Escape returns focus to the trigger"
        );

        scene.click(&trigger, 1);
        settle(scene);
        scene.click_at_point(cove_core::Point::new(12.0, 12.0), 1);
        settle(scene);
        assert!(
            scene.portal.entries().is_empty(),
            "an outside click dismisses"
        );
        assert_eq!(h.count.get(), 1);

        scene.click(&trigger, 1);
        settle(scene);
        scene.click(&trigger, 1);
        settle(scene);
        assert_eq!(
            scene.portal.entries().len(),
            0,
            "pressing the open bubble's trigger dismisses it"
        );
        assert_eq!(h.count.get(), 1);
    });
}

#[test]
fn placements() {
    let window = Rect::new(0.0, 0.0, 1280.0, 900.0);
    let size = Size {
        width: 300.0,
        height: 180.0,
    };
    let inset = Rect::new(8.0, 8.0, 1264.0, 884.0);
    let contains = |outer: Rect, inner: Rect| {
        inner.x >= outer.x - 0.01
            && inner.y >= outer.y - 0.01
            && inner.x + inner.width <= outer.x + outer.width + 0.01
            && inner.y + inner.height <= outer.y + outer.height + 0.01
    };
    let geometry =
        |anchor: Rect, placement: PopoverPlacement, rtl: bool, overflow: PopoverOverflow| {
            PopoverGeometry::new(
                anchor,
                size,
                window,
                placement,
                rtl,
                overflow,
                PopoverGeometry::MARGIN,
                PopoverGeometry::GAP,
            )
        };
    let center = Rect::new(600.0, 400.0, 80.0, 44.0);
    for placement in PopoverPlacement::ALL {
        for rtl in [false, true] {
            let layout = geometry(center, placement, rtl, PopoverOverflow::Constrain);
            let expected = if rtl && !placement.edge().is_vertical() {
                placement.edge().opposite()
            } else {
                placement.edge()
            };
            assert_eq!(layout.edge, expected, "{placement:?} rtl={rtl}");
            assert!(
                contains(inset, layout.frame),
                "{placement:?} rtl={rtl}: {:?}",
                layout.frame
            );
            for anchor in [
                Rect::new(8.0, 8.0, 44.0, 44.0),
                Rect::new(1228.0, 848.0, 44.0, 44.0),
            ] {
                let edge = geometry(anchor, placement, rtl, PopoverOverflow::Constrain);
                assert!(
                    contains(inset, edge.frame),
                    "{placement:?} rtl={rtl} at {anchor:?}: {:?}",
                    edge.frame
                );
            }
        }
    }
    let bottom = geometry(
        Rect::new(220.0, 850.0, 80.0, 44.0),
        PopoverPlacement::Bottom,
        false,
        PopoverOverflow::Constrain,
    );
    assert!(
        bottom.edge == PopoverEdge::Bottom && bottom.frame.height == 0.0,
        "{bottom:?}"
    );
    let right = geometry(
        Rect::new(1228.0, 400.0, 44.0, 44.0),
        PopoverPlacement::Trailing,
        false,
        PopoverOverflow::Constrain,
    );
    assert!(
        right.edge == PopoverEdge::Right && right.frame.width == 0.0,
        "{right:?}"
    );
    let adaptive = geometry(
        Rect::new(220.0, 850.0, 80.0, 44.0),
        PopoverPlacement::Bottom,
        false,
        PopoverOverflow::Flip,
    );
    assert!(
        adaptive.edge == PopoverEdge::Top && adaptive.frame.y + adaptive.frame.height < 850.0,
        "{adaptive:?}"
    );
}

#[test]
fn matrix_and_painting() {
    with_harness(|scene, h| {
        (trigger(scene)
            .accessibility_actions()
            .activate
            .expect("activate"))();
        settle(scene);
        for width in [390.0_f32, 720.0, 1280.0] {
            for scale in [1.0_f32, 2.0] {
                for dark in [false, true] {
                    for rtl in [false, true] {
                        for modality in [
                            InputCapabilities::TOUCH,
                            InputCapabilities::TOUCH
                                | InputCapabilities::POINTER
                                | InputCapabilities::KEYBOARD,
                            InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
                        ] {
                            h.width.set(width);
                            h.height.set(820.0);
                            h.x.set(width - 160.0);
                            h.y.set(40.0);
                            h.scale.set(scale);
                            h.dark.set(dark);
                            h.rtl.set(rtl);
                            h.input.set(modality);
                            h.placement.set(PopoverPlacement::BottomTrailing);
                            settle_window(scene, h);
                            let case = format!(
                                "width {width}, scale {scale}, dark {dark}, rtl {rtl}, input {modality:?}"
                            );
                            let surface = panel(scene).unwrap_or_else(|| {
                                panic!("the bubble surface is mounted ({case})")
                            });
                            let keep = button(scene, "Keep").expect("Keep");
                            let remove = button(scene, "Remove").expect("Remove");
                            let frame = surface.absolute_frame();
                            let sheet = modality.contains(InputCapabilities::TOUCH)
                                && !modality.contains(InputCapabilities::POINTER);
                            let inset = if sheet { 0.0 } else { 8.0 };
                            assert!(
                                frame.x >= inset
                                    && frame.max_x() <= width - inset
                                    && frame.max_y() <= if sheet { 820.0 } else { 812.0 },
                                "the bubble stays inside the window ({case}): {frame:?}"
                            );
                            let body = Rect::new(
                                frame.x + 10.0,
                                frame.y + 10.0,
                                frame.width - 20.0,
                                frame.height - 20.0,
                            );
                            let (keep_frame, remove_frame) =
                                (keep.absolute_frame(), remove.absolute_frame());
                            assert!(
                                contains(body, keep_frame) && contains(body, remove_frame),
                                "both actions sit inside the bubble ({case})"
                            );
                            assert!(!intersects(keep_frame, remove_frame), "{case}");
                            if (keep_frame.y - remove_frame.y).abs() < 0.5 {
                                if rtl {
                                    assert!(keep_frame.x >= remove_frame.max_x(), "{case}");
                                } else {
                                    assert!(keep_frame.max_x() <= remove_frame.x, "{case}");
                                }
                            } else {
                                assert!(keep_frame.max_y() <= remove_frame.y, "{case}");
                            }
                            let target = if modality.contains(InputCapabilities::TOUCH) {
                                44.0
                            } else {
                                32.0
                            };
                            assert!(keep.frame().height >= target, "{case}");
                            assert!(remove.frame().height >= target, "{case}");
                            assert_eq!(dialog_count(scene, &h.title.get()), 1, "{case}");
                            assert_eq!(scene.portal.entries().len(), 1, "{case}");
                            assert!(!scene.graph.recomposer.has_pending(), "{case}");
                            if !sheet {
                                assert!(
                                    surface.draw().is_some(),
                                    "the bubble paints its card ({case})"
                                );
                            }
                        }
                    }
                }
            }
        }
    });
}

fn contains(outer: Rect, inner: Rect) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.max_x() <= outer.max_x()
        && inner.max_y() <= outer.max_y()
}

fn intersects(a: Rect, b: Rect) -> bool {
    a.x < b.max_x() && b.x < a.max_x() && a.y < b.max_y() && b.y < a.max_y()
}

#[test]
fn updates_and_teardown() {
    with_harness(|scene, h| {
        let trigger = trigger(scene);
        scene.click(&trigger, 1);
        settle(scene);
        h.title.set("Remove the archived workspace?".to_string());
        h.dark.set(true);
        h.scale.set(2.0);
        settle(scene);
        assert_eq!(
            dialog_count(scene, &h.title.get()),
            1,
            "open content follows the new title"
        );
        assert_eq!(scene.portal.entries().len(), 1);
        h.enabled.set(false);
        settle(scene);
        let trigger = button(scene, "Delete project").expect("trigger");
        assert!(
            scene.portal.entries().is_empty(),
            "disabling closes the bubble"
        );
        assert_eq!(
            trigger
                .accessibility()
                .and_then(|semantics| semantics.state.is_expanded),
            Some(false)
        );
        assert!(
            !trigger.is_focusable(),
            "a disabled trigger leaves the Tab order"
        );
        h.enabled.set(true);
        settle(scene);
        scene.click(&button(scene, "Delete project").expect("trigger"), 1);
        settle(scene);
        assert_eq!(
            scene.portal.entries().len(),
            1,
            "re-enabling allows reopening"
        );
        h.mounted.set(false);
        settle(scene);
        assert!(scene.portal.entries().is_empty());
        assert!(
            scene.capture.target().is_none(),
            "teardown releases capture"
        );
        assert!(scene.focus.focused().is_none(), "teardown releases focus");
    });
}

#[test]
fn discovery() {
    with_harness(|scene, h| {
        assert_eq!(
            dialog_count(scene, &h.title.get()),
            0,
            "a closed popconfirm exposes no dialog"
        );
        (trigger(scene)
            .accessibility_actions()
            .activate
            .expect("activate"))();
        settle(scene);
        assert_eq!(
            dialog_count(scene, &h.title.get()),
            1,
            "the open dialog is discoverable"
        );
        assert!(
            button(scene, "Remove").is_some(),
            "its actions are discoverable"
        );
        (button(scene, "Keep")
            .and_then(|node| node.accessibility_actions().activate)
            .expect("Keep exposes activate"))();
        settle(scene);
        assert_eq!(
            dialog_count(scene, &h.title.get()),
            0,
            "dismissal removes the dialog"
        );
        assert_eq!(h.count.get(), 0);
    });
}

#[test]
fn named_sides() {
    with_harness(|scene, h| {
        (trigger(scene)
            .accessibility_actions()
            .activate
            .expect("activate"))();
        settle(scene);
        for placement in PopoverPlacement::ALL {
            for rtl in [false, true] {
                h.placement.set(placement);
                h.rtl.set(rtl);
                settle(scene);
                let anchor = trigger(scene).absolute_frame();
                let surface = panel(scene)
                    .unwrap_or_else(|| panic!("the bubble is mounted for {placement:?} rtl {rtl}"))
                    .absolute_frame();
                let edge = if rtl && !placement.edge().is_vertical() {
                    placement.edge().opposite()
                } else {
                    placement.edge()
                };
                let placed = match edge {
                    PopoverEdge::Top => surface.max_y() <= anchor.y - 3.0,
                    PopoverEdge::Bottom => surface.y >= anchor.max_y() + 3.0,
                    PopoverEdge::Left => surface.max_x() <= anchor.x - 3.0,
                    PopoverEdge::Right => surface.x >= anchor.max_x() + 3.0,
                };
                assert!(
                    placed,
                    "{placement:?} rtl {rtl} keeps its side: {surface:?} vs {anchor:?}"
                );
                let inset = PopoverGeometry::MARGIN;
                assert!(
                    surface.x >= inset - 0.5 && surface.max_x() <= h.width.get() - inset + 0.5,
                    "the constrained bubble fits horizontally: {surface:?}"
                );
                if !edge.is_vertical() {
                    let available = if edge == PopoverEdge::Left {
                        anchor.x - inset
                    } else {
                        h.width.get() - inset - anchor.max_x()
                    };
                    assert!(
                        surface.width <= available - PopoverGeometry::GAP + 0.5,
                        "the side bubble narrows to the available width: {surface:?}, {available}"
                    );
                }
            }
        }
    });
}

#[test]
fn shared_card() {
    InteractiveScene::run(640.0, 480.0, |scene| {
        scene.install(SharedCardScene {
            open: State::new(true),
        });
        settle(scene);
        let surface = panel(scene).expect("the open popover mounts its bubble surface");
        let frame = surface.absolute_frame();
        assert_eq!(
            surface.frame().width,
            300.0,
            "the bubble keeps its authored width"
        );
        assert!(
            frame.max_y() <= 237.0,
            "the top placement sits above the trigger: {frame:?}"
        );
        assert!(
            surface.draw().is_some(),
            "the bubble surface paints its own card"
        );
        assert_eq!(scene.portal.entries().len(), 1);
        let done = button(scene, "Done").expect("the Done action");
        (done.accessibility_actions().activate.expect("activate"))();
        settle(scene);
        assert!(scene.portal.entries().is_empty(), "Done closes the popover");
    });
}

struct SharedCardScene {
    open: State<bool>,
}

impl View for SharedCardScene {
    fn body(&self, _cx: &Scope) -> AnyView {
        let open = self.open.clone();
        let popover = Popover::new(
            Text::new("Open details"),
            TupleView::new(vec![
                AnyView::new(Text::new("Shared surface")),
                AnyView::new(Button::title("Done", move || open.set(false))),
            ]),
        )
        .is_presented(self.open.binding())
        .width(300.0)
        .placement(PopoverPlacement::Top)
        .surface(cove_ui_compose::primitives::overlays::anchored::PopoverSurface::Bubble);
        AnyView::new(
            AnyView::new(LayerRoot::new(AnyView::new(popover.modifier(
                AbsolutePositionModifier::new(Some(220.0), Some(240.0), None, None),
            ))))
            .frame_size(640.0, 480.0)
            .reduced_motion(true),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("open", &self.open)]
    }
}
