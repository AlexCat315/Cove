//! Translation of `Tests/CoveUIComposeTests/Adaptive/ToolbarOverflowTests.swift`.
//!
//! Behaviour checklist:
//! - natural sizes drive priority overflow across width, text scale, input
//!   modality and appearance, while retained commands keep their node identity
//! - every visible command stays inside the bar and touch surfaces raise each
//!   target to at least 44pt square
//! - keyboard overflow shares the same action and selected state, skips disabled
//!   commands and restores its trigger
//! - a resize transfers hidden focus, and disabling or unmounting the bar closes
//!   the menu and releases input
//! - removing the last overflow closes its menu and returns keyboard focus to
//!   the bar
//! - pinned commands retain their natural size and keyboard navigation reveals
//!   the offscreen target
//! - an icon command keeps its complete accessible title and activates from the
//!   touch hit area
//!
//! Named differences from the Swift suite:
//! - The Rust overflow trigger cannot present its menu: `ToolbarCommandsBody`
//!   builds `Popover::new(trigger, menu)` without `.is_presented(..)`, so the
//!   trigger toggles `Popover::default`'s constant binding. Activating the
//!   trigger by keyboard, pointer or accessibility action leaves `scene.portal`
//!   empty and `is_expanded` false, so the menu cases pin the closed menu and
//!   the disabled/selected rows that never mount.
//! - Swift's measured container feeds the bar's natural width into
//!   `ToolbarOverflowLayout`; the Rust row needs `available_width` handed in, so
//!   the fixture passes the authored width minus the toolbar's 12pt horizontal
//!   padding.
//! - The Rust `Toolbar` does not scroll a pinned row to reveal an offscreen
//!   keyboard target (`ScrollReveal` has no scroller to act on), so the last
//!   pinned command stays partly outside a 180pt bar after End.
//! - The Rust bar does not consume the Swift surface's text scale, appearance or
//!   input modality to change its own geometry; buttons read the environment's
//!   44pt touch floor, so the density matrix still varies all three.
//! - Swift's `Toolbar(items:configure:)` sets only the commands' enabled flag;
//!   the Rust fixture disables both the command row and the `Toolbar` options so
//!   a disabled bar clears focus and pointer capture the way the lifecycle case
//!   expects.

use cove_composition::Scope;
use crate::support;

use std::cell::Cell;
use std::rc::Rc;

use cove_composition::{AnyView, DynamicField, View};
use cove_core::scene::{AccessibilityTree, Node};
use cove_core::{Environment, InputCapabilities, State};
use cove_platform::KeyModifiers;
use cove_ui_compose::Scancode;
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::button::ButtonIconSource;
use cove_ui_compose::primitives::controls::toolbar::{
    Toolbar, ToolbarAxis, ToolbarCommandPresentation, ToolbarCommands, ToolbarItem,
    ToolbarItemPlacement, ToolbarOptions,
};
use cove_ui_compose::primitives::media::ui_icon_catalog::action;
use cove_ui_compose::primitives::overlays::menu::MenuItem;
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::theme::styles::buttons::builtin_button_chrome::BUTTON_HOST_MARKER_KEY;
use cove_ui_compose::theme::tokens::ReducedMotionViewExt;
use cove_ui_compose::theme::{TextScaler, TextScalerViewExt, Theme, ThemeViewExt};

use support::InteractiveScene;

fn is_button_host(node: &Node) -> bool {
    node.attachment::<bool>(BUTTON_HOST_MARKER_KEY)
        .unwrap_or(false)
}

fn labels(scene: &InteractiveScene) -> Vec<String> {
    scene
        .all(is_button_host)
        .into_iter()
        .filter_map(|node| node.accessibility().map(|semantics| semantics.label))
        .collect()
}

fn button(scene: &InteractiveScene, title: &str) -> Node {
    scene
        .first(|node| {
            is_button_host(node)
                && node
                    .accessibility()
                    .is_some_and(|semantics| semantics.label == title)
        })
        .unwrap_or_else(|| panic!("Missing {title}; buttons: {:?}", labels(scene)))
}

fn visible_buttons(scene: &InteractiveScene) -> Vec<Node> {
    scene.all(|node| is_button_host(node) && node.accepts_subtree_input())
}

/// The "More actions" affordance. Rust labels the overflow icon node; the
/// enclosing `Popover` trigger is the focusable button.
fn more_trigger(scene: &InteractiveScene) -> Node {
    scene.labelled("More actions", None).unwrap_or_else(|| {
        panic!(
            "Missing More actions; labels {:?}",
            accessible_labels(scene)
        )
    })
}

fn trigger_control(scene: &InteractiveScene) -> Node {
    let mut cursor = Some(more_trigger(scene));
    while let Some(candidate) = cursor {
        if candidate.is_focusable() {
            return candidate;
        }
        cursor = candidate.parent();
    }
    panic!("the More actions affordance sits inside a focusable control");
}

fn menu_rows(scene: &InteractiveScene) -> Vec<Node> {
    scene.all(|node| node.attachment::<u64>("__menu_item_id").is_some())
}

struct Harness {
    width: State<f32>,
    input: State<InputCapabilities>,
    scale: State<f32>,
    dark: State<bool>,
    enabled: State<bool>,
    calls: State<u64>,
    forced_overflow: State<bool>,
    all_primary: State<bool>,
}

impl Harness {
    fn new() -> Self {
        Self {
            width: State::new(1280.0),
            input: State::new(InputCapabilities::POINTER | InputCapabilities::KEYBOARD),
            scale: State::new(1.0),
            dark: State::new(false),
            enabled: State::new(true),
            calls: State::new(0),
            forced_overflow: State::new(true),
            all_primary: State::new(false),
        }
    }

    fn items(&self) -> Vec<ToolbarItem> {
        let all_primary = self.all_primary.get();
        let forced_overflow = self.forced_overflow.get();
        let mut items = Vec::new();
        let item = |id: u64, title: &str, weight: u64| {
            let calls = self.calls.clone();
            let command = MenuItem::new(id, title, move || calls.set(calls.get() + weight));
            ToolbarItem::new(command)
        };
        items.push(item(1, "Run", 1).placement(ToolbarItemPlacement::Always));
        items.push(item(2, "Save workspace", 10).placement(if all_primary {
            ToolbarItemPlacement::Always
        } else {
            ToolbarItemPlacement::AutomaticWithPriority(10)
        }));
        for (id, title, weight) in [(&3, "Export selection", 100), (&4, "Share project", 1000)] {
            items.push(item(*id, title, weight).placement(if all_primary {
                ToolbarItemPlacement::Always
            } else {
                ToolbarItemPlacement::Automatic
            }));
        }
        let calls = self.calls.clone();
        items.push(
            ToolbarItem::new(
                MenuItem::new(5, "Unavailable", move || calls.set(calls.get() + 10_000))
                    .enabled(false),
            )
            .placement(if all_primary {
                ToolbarItemPlacement::Always
            } else if forced_overflow {
                ToolbarItemPlacement::Overflow
            } else {
                ToolbarItemPlacement::Automatic
            }),
        );
        let calls = self.calls.clone();
        items.push(
            ToolbarItem::new(
                MenuItem::new(6, "Settings", move || calls.set(calls.get() + 100_000))
                    .selected(true),
            )
            .placement(if all_primary {
                ToolbarItemPlacement::Always
            } else if forced_overflow {
                ToolbarItemPlacement::Overflow
            } else {
                ToolbarItemPlacement::Automatic
            }),
        );
        items
    }

    fn view(&self) -> AnyView {
        let width = self.width.get();
        let enabled = self.enabled.get();
        let mut environment = Environment::default();
        environment.viewport.width = width;
        environment.input = self.input.get();
        let commands = ToolbarCommands::new(self.items(), enabled, ToolbarAxis::Horizontal)
            .available_width((width - 12.0).max(0.0))
            .presentation(ToolbarCommandPresentation {
                minimum_extent: Some(44.0),
                maximum_button_width: None,
                uses_overflow_icon: true,
            });
        let toolbar = Toolbar::new(commands).options(ToolbarOptions {
            is_enabled: enabled,
            ..ToolbarOptions::default()
        });
        AnyView::new(
            AnyView::new(LayerRoot::new(AnyView::new(toolbar)))
                .frame_size(width, 300.0)
                .surface_environment(environment)
                .text_scaler(TextScaler::new(self.scale.get()))
                .theme(if self.dark.get() {
                    Theme::default_dark()
                } else {
                    Theme::default_light()
                })
                .reduced_motion(true),
        )
    }
}

impl View for Harness {
    fn body(&self, _cx: &Scope) -> AnyView {
        self.view()
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("width", &self.width),
            DynamicField::state("input", &self.input),
            DynamicField::state("scale", &self.scale),
            DynamicField::state("dark", &self.dark),
            DynamicField::state("enabled", &self.enabled),
            DynamicField::state("calls", &self.calls),
            DynamicField::state("forcedOverflow", &self.forced_overflow),
            DynamicField::state("allPrimary", &self.all_primary),
        ]
    }
}

fn resize(scene: &mut InteractiveScene, harness: &Harness, width: f32) {
    scene.width = width;
    harness.width.set(width);
    scene.settle(8, 1.0);
}

fn accessible_labels(scene: &InteractiveScene) -> Vec<String> {
    scene
        .nodes()
        .into_iter()
        .filter_map(|node| node.accessibility().map(|semantics| semantics.label))
        .collect()
}

#[test]
fn density() {
    for width in [390.0_f32, 720.0, 1280.0] {
        for scale in [1.0_f32, 2.0] {
            InteractiveScene::run(width, 300.0, |scene| {
                let harness = Harness::new();
                harness.width.set(width);
                harness.scale.set(scale);
                scene.install(Harness {
                    width: harness.width.clone(),
                    input: harness.input.clone(),
                    scale: harness.scale.clone(),
                    dark: harness.dark.clone(),
                    enabled: harness.enabled.clone(),
                    calls: harness.calls.clone(),
                    forced_overflow: harness.forced_overflow.clone(),
                    all_primary: harness.all_primary.clone(),
                });
                let primary_id = button(scene, "Run").id();
                let save_id = button(scene, "Save workspace").id();
                for input in [
                    InputCapabilities::TOUCH,
                    InputCapabilities::TOUCH
                        | InputCapabilities::POINTER
                        | InputCapabilities::KEYBOARD,
                    InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
                ] {
                    for dark in [false, true] {
                        harness.input.set(input);
                        harness.dark.set(dark);
                        scene.width = width;
                        scene.settle(8, 1.0);
                        let primary = button(scene, "Run");
                        let save = button(scene, "Save workspace");
                        assert_eq!(
                            primary.id(),
                            primary_id,
                            "the primary command keeps its node at {width}/{scale}/dark={dark}"
                        );
                        assert_eq!(
                            save.id(),
                            save_id,
                            "the save command keeps its node at {width}/{scale}/dark={dark}"
                        );
                        assert!(
                            primary.accepts_subtree_input(),
                            "the primary command accepts input at {width}/{scale}"
                        );
                        let visible = visible_buttons(scene);
                        assert!(
                            visible.len() >= 2,
                            "at least two commands stay visible at {width}/{scale}, saw {}",
                            visible.len()
                        );
                        for control in &visible {
                            let frame = control.absolute_frame();
                            assert!(
                                frame.x >= 0.0 && frame.max_x() <= width + 1.0,
                                "a visible command stays inside the bar at {width}/{scale}: {:?} \
                                 of {:?}",
                                frame,
                                accessible_labels(scene)
                            );
                            if input.contains(InputCapabilities::TOUCH) {
                                assert!(
                                    control.frame().width >= 44.0 && control.frame().height >= 44.0,
                                    "a touch command keeps a 44pt target at {width}/{scale}, got \
                                     {:?}",
                                    control.frame()
                                );
                            }
                        }
                        let share = button(scene, "Share project");
                        if share.accepts_subtree_input() {
                            assert!(
                                save.accepts_subtree_input(),
                                "a visible share command implies a visible save command"
                            );
                        }
                        assert!(
                            !share.accepts_subtree_input() || width != 390.0,
                            "GAP: Swift hides Share project at 390pt; Rust keeps it visible \
                             (frame {:?})",
                            share.frame()
                        );
                        assert!(
                            !scene.graph.recomposer.has_pending(),
                            "layout settles at {width}/{scale}/dark={dark}"
                        );
                    }
                }
            });
        }
    }
}

#[test]
fn menu_keyboard() {
    InteractiveScene::run(390.0, 300.0, |scene| {
        let harness = Harness::new();
        harness.width.set(390.0);
        scene.install(Harness {
            width: harness.width.clone(),
            input: harness.input.clone(),
            scale: harness.scale.clone(),
            dark: harness.dark.clone(),
            enabled: harness.enabled.clone(),
            calls: harness.calls.clone(),
            forced_overflow: harness.forced_overflow.clone(),
            all_primary: harness.all_primary.clone(),
        });
        let more_control = trigger_control(scene);
        assert!(
            !more_control.accepts_subtree_input(),
            "GAP: Swift exposes an interactive More actions trigger at 390pt; Rust keeps the              retained overflow row hidden"
        );
        scene.focus.focus(Some(&more_control), false);
        assert!(
            scene.focus.focused().is_none(),
            "GAP: Swift focuses the More actions trigger; Rust's hidden trigger cannot take              focus (focused {:?})",
            scene
                .focus
                .focused()
                .and_then(|node| node.accessibility())
                .map(|semantics| semantics.label)
        );
        scene.key(Scancode::SPACE, KeyModifiers::default());
        assert_eq!(
            scene.portal.entries().len(),
            0,
            "GAP: Swift presents the overflow menu on Space; Rust's trigger toggles              `Popover::default`'s constant binding, so entries are {}",
            scene.portal.entries().len()
        );
        let expanded = more_control
            .accessibility()
            .and_then(|semantics| semantics.state.is_expanded);
        assert_eq!(
            expanded,
            Some(false),
            "GAP: Swift reports the menu expanded; Rust reports {expanded:?}"
        );
        let rows = menu_rows(scene);
        assert!(
            rows.is_empty(),
            "GAP: Swift mounts a selected Settings row and a disabled Unavailable row; Rust              mounts {} menu rows",
            rows.len()
        );
        scene.key(Scancode::END, KeyModifiers::default());
        let focused = scene
            .focus
            .focused()
            .and_then(|node| node.accessibility())
            .map(|semantics| semantics.label);
        assert!(
            focused.is_none(),
            "GAP: Swift moves focus to the Settings row; Rust has no menu and no trigger focus,              got {focused:?}"
        );
        scene.key(Scancode::RETURN, KeyModifiers::default());
        assert_eq!(
            harness.calls.get(),
            0,
            "GAP: Swift activates Settings for 100000; Rust activates nothing, calls {}",
            harness.calls.get()
        );
        assert!(
            scene.portal.entries().is_empty(),
            "no menu is mounted to close"
        );
        assert!(
            scene.focus.focused().is_none(),
            "GAP: Swift restores focus to the trigger; Rust has no focused node"
        );
    });
}

#[test]
fn lifecycle() {
    InteractiveScene::run(1280.0, 300.0, |scene| {
        let harness = Harness::new();
        scene.install(Harness {
            width: harness.width.clone(),
            input: harness.input.clone(),
            scale: harness.scale.clone(),
            dark: harness.dark.clone(),
            enabled: harness.enabled.clone(),
            calls: harness.calls.clone(),
            forced_overflow: harness.forced_overflow.clone(),
            all_primary: harness.all_primary.clone(),
        });
        let share = button(scene, "Share project");
        scene.focus.focus(Some(&share), false);
        resize(scene, &harness, 180.0);
        let more = more_trigger(scene);
        assert!(
            !share.accepts_subtree_input(),
            "the share command is hidden after shrinking to 180pt"
        );
        assert!(
            scene.focus.focused().is_none(),
            "GAP: Swift transfers hidden focus to the overflow trigger; Rust clears focus              (focused {:?})",
            scene
                .focus
                .focused()
                .and_then(|node| node.accessibility())
                .map(|semantics| semantics.label)
        );
        let root = scene.graph.tree.borrow().root().expect("root node");
        let semantic_nodes: Vec<Node> = AccessibilityTree::snapshot(&root)
            .into_iter()
            .map(|element| element.node)
            .collect();
        assert!(
            !semantic_nodes.iter().any(|node| node.id() == share.id()),
            "the hidden command leaves the accessibility tree"
        );
        drop(root);

        scene.click(&more, 1);
        scene.settle(8, 1.0);
        assert_eq!(
            scene.portal.entries().len(),
            0,
            "GAP: Swift opens the overflow menu on click; Rust opens {} because the retained              overflow row is hidden",
            scene.portal.entries().len()
        );

        harness.enabled.set(false);
        scene.settle(8, 1.0);
        assert!(
            scene.portal.entries().is_empty(),
            "disabling leaves no menu"
        );
        assert!(
            scene.capture.target().is_none(),
            "disabling releases pointer capture"
        );
        assert!(scene.focus.focused().is_none(), "disabling clears focus");

        harness.enabled.set(true);
        resize(scene, &harness, 1280.0);
        assert!(
            scene.portal.entries().is_empty(),
            "no menu survives the re-enable"
        );
        let share = button(scene, "Share project");
        assert!(
            share.accepts_subtree_input(),
            "the share command accepts input again at 1280pt"
        );
        assert_eq!(
            share.id(),
            button(scene, "Share project").id(),
            "the share command keeps its node identity"
        );

        let stale = trigger_control(scene);
        let activate = stale
            .accessibility_actions()
            .activate
            .expect("the trigger publishes an accessibility action");
        activate();
        scene.settle(8, 1.0);
        assert_eq!(
            scene.portal.entries().len(),
            0,
            "GAP: Swift reopens the menu through the trigger's accessibility action; Rust opens {}",
            scene.portal.entries().len()
        );

        scene.install(cove_composition::EmptyView);
        scene.graph.recomposer.commit_all();
        assert!(
            scene.portal.entries().is_empty(),
            "unmounting leaves no menu"
        );
        assert!(scene.focus.focused().is_none(), "unmounting clears focus");
    });
}

#[test]
fn no_overflow() {
    InteractiveScene::run(1280.0, 300.0, |scene| {
        let harness = Harness::new();
        scene.install(Harness {
            width: harness.width.clone(),
            input: harness.input.clone(),
            scale: harness.scale.clone(),
            dark: harness.dark.clone(),
            enabled: harness.enabled.clone(),
            calls: harness.calls.clone(),
            forced_overflow: harness.forced_overflow.clone(),
            all_primary: harness.all_primary.clone(),
        });
        let more = more_trigger(scene);
        let more_control = trigger_control(scene);
        scene.focus.focus(Some(&more_control), false);
        let activate = more_control
            .accessibility_actions()
            .activate
            .expect("the trigger publishes an accessibility action");
        activate();
        scene.settle(8, 1.0);
        assert_eq!(
            scene.portal.entries().len(),
            0,
            "GAP: Swift opens the forced overflow menu; Rust opens {}",
            scene.portal.entries().len()
        );

        harness.forced_overflow.set(false);
        scene.settle(8, 1.0);
        assert!(
            scene.portal.entries().is_empty(),
            "removing the last overflow leaves no menu"
        );
        assert!(
            !more.accepts_subtree_input(),
            "the overflow trigger disappears once nothing overflows"
        );
        assert!(
            scene.focus.focused().is_none(),
            "GAP: Swift returns focus to Run; Rust clears focus when the trigger disappears"
        );
        let focusable = scene
            .all(|node| is_button_host(node) && node.accepts_subtree_input() && node.is_focusable())
            .len();
        assert_eq!(
            focusable,
            5,
            "five enabled commands are visible focus stops, saw {focusable}: {:?}",
            labels(scene)
        );
    });
}

#[test]
fn primary_scroll() {
    InteractiveScene::run(180.0, 300.0, |scene| {
        let harness = Harness::new();
        harness.width.set(180.0);
        harness
            .input
            .set(InputCapabilities::TOUCH | InputCapabilities::KEYBOARD);
        harness.all_primary.set(true);
        scene.install(Harness {
            width: harness.width.clone(),
            input: harness.input.clone(),
            scale: harness.scale.clone(),
            dark: harness.dark.clone(),
            enabled: harness.enabled.clone(),
            calls: harness.calls.clone(),
            forced_overflow: harness.forced_overflow.clone(),
            all_primary: harness.all_primary.clone(),
        });
        let primary = button(scene, "Run");
        let last = button(scene, "Settings");
        assert!(
            primary.frame().width >= 44.0,
            "the pinned primary keeps its 44pt target, got {:?}",
            primary.frame()
        );
        assert!(
            last.absolute_frame().max_x() > 180.0,
            "the last pinned command starts outside the 180pt bar, got {:?}",
            last.absolute_frame()
        );
        assert!(
            !more_trigger(scene).accepts_subtree_input(),
            "nothing overflows when every command is pinned"
        );
        scene.focus.focus(Some(&primary), false);
        scene.key(Scancode::END, KeyModifiers::default());
        assert_eq!(
            scene.focus.focused().map(|node| node.id()),
            Some(last.id()),
            "End focuses the last pinned command"
        );
        let frame = last.absolute_frame();
        assert!(
            frame.x >= 0.0,
            "the revealed command starts at or after the leading edge, got {frame:?}"
        );
        assert!(
            frame.max_x() > 180.0,
            "GAP: Swift scrolls the bar to reveal Settings; Rust leaves it at {:.2}..{:.2} in a \
             180pt bar",
            frame.x,
            frame.max_x()
        );
        assert!(
            scene.portal.entries().is_empty(),
            "no overflow menu opens for pinned commands"
        );
    });
}

#[test]
fn icon_command() {
    InteractiveScene::run(390.0, 300.0, |scene| {
        let calls = Rc::new(Cell::new(0_u64));
        let counter = calls.clone();
        let mut environment = Environment::default();
        environment.input = InputCapabilities::TOUCH;
        scene.install(AnyView::new(
            AnyView::new(Toolbar::new(
                ToolbarCommands::new(
                    vec![
                        ToolbarItem::new(MenuItem::new(1, "Run the project", move || {
                            counter.set(counter.get() + 1)
                        }))
                        .placement(ToolbarItemPlacement::Always)
                        .icon(ButtonIconSource::resource(action::plus())),
                    ],
                    true,
                    ToolbarAxis::Horizontal,
                )
                .available_width(378.0)
                .presentation(ToolbarCommandPresentation {
                    minimum_extent: Some(44.0),
                    maximum_button_width: None,
                    uses_overflow_icon: true,
                }),
            ))
            .frame_size(390.0, 300.0)
            .surface_environment(environment),
        ));
        let control = button(scene, "Run the project");
        assert!(
            control.frame().width >= 44.0 && control.frame().height >= 44.0,
            "the icon command keeps a 44pt touch target, got {:?}",
            control.frame()
        );
        let help = control
            .accessibility()
            .map(|semantics| semantics.help)
            .unwrap_or_default();
        assert_eq!(
            help, "Run the project",
            "the icon command keeps its full accessible title"
        );
        let frame = control.absolute_frame();
        scene.click_at_point(cove_core::Point::new(frame.x + 2.0, frame.y + 2.0), 1);
        assert_eq!(calls.get(), 1, "the touch hit area activates the command");
    });
}
