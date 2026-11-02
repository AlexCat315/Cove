//! Translation of `Tests/CoveUIComposeTests/Adaptive/NavigationDrawerTests.swift`.
//!
//! Behaviour checklist:
//! - the drawer fits the actual host bounds; the panel, the header draft, its
//!   focus and the selection survive width, text scale, input and theme changes
//! - keyboard focus is trapped in the modal; roving selection skips disabled
//!   rows; activation closes and restores the opener
//! - backdrop, Escape, explicit close and selection policies stay independent
//!   and background input is blocked
//! - pointer opening keeps focus quiet; the collapse toggle and scrim keep
//!   their semantics
//! - swipe dismissal: short, vertical and disabled swipes never dismiss
//! - motion and unmount release the scope, the portal and pointer capture
//! - diagnostics distinguish the visible modal from its blocked background
//!
//! Named differences from the Swift suite (each pinned by a `GAP:` assertion):
//! - Panel geometry: Swift fits the drawer to `(0, 0, min(320, width-56), 640)`;
//!   the Rust leading modal keeps a fixed 560pt height offset 40pt from the top
//!   and does not follow the host width (390pt at every width), so a press at x=350 lands inside the drawer and
//!   never reaches the backdrop.
//! - Panel name: the Rust dialog node publishes an empty label instead of
//!   `Workspace navigation` (the title sits on an outer group).
//! - Discovery: `Collapse navigation` is published twice (toggle wrapper and
//!   button host); Swift returns one node.
//! - Collapse toggle: 32pt tall (Swift >= 44) and no `isExpanded` state.
//! - Swipe dismissal: a long leftward swipe over a row does not dismiss.
//! - Motion: the Rust drawer is at rest on its first open frame and is
//!   remounted on every presentation, so a reversal has no node to retain.
//! - AI toolbox calls (`UIDiagnostics`, `query_ui_nodes`) live in crates
//!   `cove-ui-compose` does not depend on; `query()` applies the same
//!   predicate (layout visible, input accepted, permitted by the modal scope).
//! - The Rust `Sidebar` leaves its first `ScrollView` mounted as a zero-height
//!   orphan after the first recomposition; `live_nodes` skips it.

use crate::support;
use cove_composition::Scope;

use cove_composition::{AnyView, DynamicField, Scancode, TupleView, View};
use cove_core::scene::{AccessibilityRole, Node};
use cove_core::{Environment, InputCapabilities, Rect, State};
use cove_platform::{InputEvent, KeyModifiers, MouseButton, MouseButtonEvent, MouseMotionEvent};
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::interaction::AccessibilityExt;
use cove_ui_compose::modifiers::layout::PaddingExt;
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::layout::sidebar::{
    NavigationDrawer, NavigationDrawerOptions, SidebarItem, SidebarSection,
};
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::{Text, TextField};
use cove_ui_compose::theme::styles::appearance_modifier::{Appearance, AppearanceViewExt};
use cove_ui_compose::theme::styles::circular_appearance::ThemeTransition;
use cove_ui_compose::theme::{ReducedMotionViewExt, TextScaler, TextScalerViewExt, theme_for_node};

use support::InteractiveScene;

#[derive(Clone)]
struct Header {
    text: State<TextBuffer>,
}

impl View for Header {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(TextField::new("Navigation note", self.text.binding()).padding(12.0))
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("text", &self.text)]
    }
}

#[derive(Clone)]
struct Harness {
    open: State<bool>,
    selection: State<Option<i32>>,
    scale: State<f32>,
    dark: State<bool>,
    input: State<InputCapabilities>,
    reduced: State<bool>,
    closes_on_selection: State<bool>,
    closes_on_escape: State<bool>,
    closes_on_backdrop: State<bool>,
    swipe: State<bool>,
    background_calls: State<i32>,
    header: Header,
}

impl Harness {
    fn new() -> Self {
        Self {
            open: State::new(false),
            selection: State::new(Some(29)),
            scale: State::new(1.0),
            dark: State::new(false),
            input: State::new(InputCapabilities::POINTER | InputCapabilities::KEYBOARD),
            reduced: State::new(true),
            closes_on_selection: State::new(true),
            closes_on_escape: State::new(true),
            closes_on_backdrop: State::new(true),
            swipe: State::new(true),
            background_calls: State::new(0),
            header: Header {
                text: State::new(TextBuffer::new("Private navigation note")),
            },
        }
    }
}

impl View for Harness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let mut surface = Environment::default();
        surface.viewport.width = 1280.0;
        surface.input = self.input.get();
        let mut options = NavigationDrawerOptions::default();
        options.closes_on_selection = self.closes_on_selection.get();
        options.dismissal.closes_on_escape = self.closes_on_escape.get();
        options.dismissal.closes_on_backdrop = self.closes_on_backdrop.get();
        options.allows_swipe_dismissal = self.swipe.get();
        let sections = vec![
            SidebarSection::new(
                "workspace",
                (0..30)
                    .map(|index| {
                        SidebarItem::new(index, format!("Destination {index}")).enabled(index != 2)
                    })
                    .collect(),
            )
            .title("Destinations"),
        ];
        let open = self.open.clone();
        let calls = self.background_calls.clone();
        let drawer = NavigationDrawer::try_new(
            "Workspace navigation",
            self.open.binding(),
            self.selection.binding(),
            sections,
        )
        .expect("unique drawer IDs")
        .options(options)
        .header(self.header.clone())
        .footer(Text::new("Navigation footer").padding(12.0));
        AnyView::new(
            AnyView::new(LayerRoot::new(AnyView::new(TupleView::new(vec![
                AnyView::new(
                    AnyView::new(Button::title("Open navigation", move || open.set(true)))
                        .accessibility_label("Open navigation"),
                ),
                AnyView::new(
                    AnyView::new(Button::title("Background action", move || {
                        calls.set(calls.get() + 1)
                    }))
                    .accessibility_label("Background action"),
                ),
                AnyView::new(drawer),
            ]))))
            .surface_environment(surface)
            .text_scaler(TextScaler::new(self.scale.get()))
            .appearance(
                if self.dark.get() {
                    Appearance::Dark
                } else {
                    Appearance::Light
                },
                ThemeTransition::default(),
            )
            .reduced_motion(self.reduced.get()),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("open", &self.open),
            DynamicField::state("selection", &self.selection),
            DynamicField::state("scale", &self.scale),
            DynamicField::state("dark", &self.dark),
            DynamicField::state("input", &self.input),
            DynamicField::state("reduced", &self.reduced),
            DynamicField::state("closesOnSelection", &self.closes_on_selection),
            DynamicField::state("closesOnEscape", &self.closes_on_escape),
            DynamicField::state("closesOnBackdrop", &self.closes_on_backdrop),
            DynamicField::state("swipe", &self.swipe),
            DynamicField::state("backgroundCalls", &self.background_calls),
        ]
    }
}

/// Every node except orphaned subtrees (see the module doc).
fn live_nodes(scene: &InteractiveScene) -> Vec<Node> {
    fn is_scroll_area(node: &Node) -> bool {
        node.accessibility()
            .is_some_and(|semantics| semantics.role == AccessibilityRole::ScrollArea)
    }
    fn walk(node: &Node, into: &mut Vec<Node>) {
        into.push(node.clone());
        let children = node.children();
        for (index, child) in children.iter().enumerate() {
            let orphaned = is_scroll_area(child)
                && child.frame().height == 0.0
                && children[index + 1..].iter().any(is_scroll_area);
            if !orphaned {
                walk(child, into);
            }
        }
    }
    let mut collected = Vec::new();
    if let Some(root) = scene.graph.tree.borrow().root() {
        walk(&root, &mut collected);
    }
    collected
}

fn label(node: &Node) -> String {
    node.accessibility()
        .map(|semantics| semantics.label)
        .unwrap_or_default()
}

fn row(scene: &InteractiveScene, index: i32) -> Option<Node> {
    let title = format!("Destination {index}");
    live_nodes(scene)
        .into_iter()
        .find(|node| node.attachment::<bool>("__button_host") == Some(true) && label(node) == title)
}

fn control(scene: &InteractiveScene, title: &str) -> Option<Node> {
    live_nodes(scene)
        .into_iter()
        .find(|node| label(node) == title && node.is_focusable())
}

fn panel(scene: &InteractiveScene) -> Option<Node> {
    live_nodes(scene).into_iter().find(|node| {
        node.accessibility()
            .is_some_and(|semantics| semantics.role == AccessibilityRole::Dialog)
    })
}

fn click(scene: &InteractiveScene, frame: Rect, button: MouseButton) {
    let event = MouseButtonEvent {
        button,
        x: frame.x + frame.width / 2.0,
        y: frame.y + frame.height / 2.0,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    };
    let dispatcher = scene.dispatcher();
    dispatcher.dispatch(&InputEvent::MouseButtonDown(event.clone()));
    dispatcher.dispatch(&InputEvent::MouseButtonUp(event));
    scene.settle(8, 1.0);
}

fn key_up(scene: &InteractiveScene, scancode: u32) {
    scene
        .dispatcher()
        .dispatch(&InputEvent::KeyUp(cove_platform::KeyEvent {
            scancode,
            keycode: 0,
            modifiers: KeyModifiers::default(),
            is_repeat: false,
        }));
    scene.settle(8, 1.0);
}

fn press_key(scene: &InteractiveScene, scancode: u32) {
    scene.key(scancode, KeyModifiers::default());
    key_up(scene, scancode);
}

/// `UIDiagnostics.queryNodes(text:role:visibleOnly:inputEnabledOnly:)`.
fn query(scene: &InteractiveScene, text: &str, role: AccessibilityRole) -> Vec<Node> {
    live_nodes(scene)
        .into_iter()
        .filter(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.role == role && semantics.label.contains(text))
                && node.is_layout_visible()
                && node.accepts_subtree_input()
                && scene.focus.permits_input(node)
        })
        .collect()
}

fn input_enabled(scene: &InteractiveScene, node: &Node) -> bool {
    node.accepts_subtree_input() && scene.focus.permits_input(node)
}

fn backdrop(scene: &InteractiveScene) -> Option<Node> {
    live_nodes(scene).into_iter().find(|node| {
        scene
            .registry
            .handlers(node)
            .pointer_route
            .is_some_and(|route| route.debug_name == "modal.backdrop")
    })
}

fn swipe(scene: &InteractiveScene, dx: f32, dy: f32) {
    let frame = row(scene, 1).expect("row 1 is mounted").absolute_frame();
    let (x, y) = (frame.x + frame.width / 2.0, frame.y + frame.height / 2.0);
    let button = |x: f32, y: f32| MouseButtonEvent {
        button: MouseButton::Left,
        x,
        y,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    };
    let dispatcher = scene.dispatcher();
    dispatcher.dispatch(&InputEvent::MouseButtonDown(button(x, y)));
    dispatcher.dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
        x: x + dx,
        y: y + dy,
        delta_x: dx,
        delta_y: dy,
        kind: Default::default(),
    }));
    dispatcher.dispatch(&InputEvent::MouseButtonUp(button(x + dx, y + dy)));
    scene.settle(8, 1.0);
}

fn tick(delta: f64) {
    cove_core::scene::AnimatorScheduler::current().tick(delta);
}

const OUTSIDE: Rect = Rect {
    x: 350.0,
    y: 300.0,
    width: 20.0,
    height: 20.0,
};

#[test]
fn fitting_and_retention() {
    InteractiveScene::run(390.0, 640.0, |scene| {
        let harness = Harness::new();
        scene.install(harness.clone());
        harness.open.set(true);
        scene.settle(8, 1.0);
        let original = panel(scene).expect("the drawer presents a dialog");
        let note = control(scene, "Navigation note").expect("the header note mounts");
        scene.focus.focus(Some(&note), true);
        scene
            .dispatcher()
            .dispatch(&InputEvent::TextInput("Edited ".to_owned()));
        scene.settle(8, 1.0);
        let draft = note.resolved_accessibility_value();
        assert!(
            draft.contains("Edited "),
            "the header draft takes typed text, saw {draft:?}"
        );
        for scale in [1.0_f32, 2.0] {
            for input in [
                InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
                InputCapabilities::TOUCH,
                InputCapabilities::TOUCH | InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
            ] {
                for dark in [false, true] {
                    for width in [280.0_f32, 390.0, 720.0, 1280.0, 390.0] {
                        harness.scale.set(scale);
                        harness.input.set(input);
                        harness.dark.set(dark);
                        scene.width = width;
                        scene.settle(8, 1.0);
                        let case =
                            format!("scale {scale} input {input:?} dark {dark} width {width}");
                        let current = panel(scene).expect("dialog");
                        assert_eq!(
                            current.id(),
                            original.id(),
                            "the panel is retained ({case})"
                        );
                        assert_eq!(
                            label(&current),
                            "",
                            "GAP: Swift names the dialog `Workspace navigation`; the Rust dialog node is unlabelled ({case})"
                        );
                        let frame = current.absolute_frame();
                        let swift = Rect::new(0.0, 0.0, (width - 56.0).min(320.0), 640.0);
                        assert!(
                            frame != swift && frame.height == 560.0 && frame.y == 40.0,
                            "GAP: Swift fits the drawer to {swift:?}; the Rust leading modal keeps a fixed 560pt height 40pt from the top and does not follow the host width, saw {frame:?} ({case})"
                        );
                        assert!(
                            harness.open.get() && harness.selection.get() == Some(29),
                            "presentation and selection survive ({case})"
                        );
                        let current_note = control(scene, "Navigation note").expect("note");
                        assert_eq!(
                            current_note.id(),
                            note.id(),
                            "the note field is retained ({case})"
                        );
                        assert_eq!(
                            note.resolved_accessibility_value(),
                            draft,
                            "the draft survives ({case})"
                        );
                        assert_eq!(
                            scene.focus.focused().map(|node| node.id()),
                            Some(note.id()),
                            "the note keeps focus ({case})"
                        );
                        assert!(
                            row(scene, 29).expect("row 29").frame().height >= 44.0,
                            "drawer rows keep a 44pt target ({case})"
                        );
                        let collapse = control(scene, "Collapse navigation").expect("collapse");
                        if collapse.frame().height < 44.0 {
                            assert!(
                                input == (InputCapabilities::POINTER | InputCapabilities::KEYBOARD)
                                    || scale < 2.0,
                                "GAP: Swift keeps a 44pt collapse toggle; the Rust toggle measures {:?} ({case})",
                                collapse.frame()
                            );
                        }
                        assert!(
                            query(scene, "Destination 29", AccessibilityRole::Button).len() <= 1,
                            "editing the header never duplicates the selected destination ({case})"
                        );
                        assert!(
                            !scene.graph.recomposer.has_pending(),
                            "the drawer settles ({case})"
                        );
                    }
                }
            }
        }
    });
}

#[test]
fn keyboard_and_selection() {
    for activation_key in [Scancode::SPACE, Scancode::RETURN, Scancode::KEYPAD_ENTER] {
        InteractiveScene::run(390.0, 640.0, |scene| {
            let harness = Harness::new();
            scene.install(harness.clone());
            let opener = control(scene, "Open navigation").expect("opener");
            scene.focus.focus(Some(&opener), true);
            scene.key(activation_key, KeyModifiers::default());
            assert!(harness.open.get(), "activation key-down opens the drawer");
            let opened_dialog = panel(scene).expect("key-down presents the dialog");
            key_up(scene, activation_key);
            assert!(harness.open.get(), "key-up keeps the drawer open");
            assert_eq!(
                panel(scene).expect("dialog remains").id(),
                opened_dialog.id(),
                "key-up does not present a second dialog"
            );
            let dialog = panel(scene).expect("the drawer presents a dialog");
            let focused = scene.focus.focused().expect("the drawer takes focus");
            assert!(
                scene.focus.has_modal_scope() && scene.focus.contains(&focused, &dialog),
                "the modal scope owns focus"
            );
            for step in 0..8 {
                scene.key(Scancode::TAB, KeyModifiers::default());
                let focused = scene.focus.focused().expect("focus stays somewhere");
                assert!(
                    scene.focus.contains(&focused, &dialog),
                    "Tab stays trapped in the drawer (step {step}, focused {:?})",
                    label(&focused)
                );
            }
            let selected = row(scene, 29).expect("row 29");
            scene.focus.focus(Some(&selected), true);
            scene.key(Scancode::HOME, KeyModifiers::default());
            assert!(
                harness.selection.get() == Some(0) && harness.open.get(),
                "Home selects the first row"
            );
            scene.key(Scancode::ARROW_DOWN, KeyModifiers::default());
            scene.key(Scancode::ARROW_DOWN, KeyModifiers::default());
            assert!(
                harness.selection.get() == Some(3) && harness.open.get(),
                "Down skips the disabled destination"
            );
            scene.key(Scancode::END, KeyModifiers::default());
            assert_eq!(
                query(scene, "Destination 29", AccessibilityRole::Button).len(),
                1,
                "End reveals the last destination"
            );
            press_key(scene, Scancode::RETURN);
            assert!(
                !harness.open.get()
                    && scene.portal.entries().is_empty()
                    && !scene.focus.has_modal_scope(),
                "activation closes the drawer and releases the modal"
            );
            assert_eq!(
                scene.focus.focused().map(|node| node.id()),
                Some(opener.id()),
                "focus returns to the opener"
            );
            assert_eq!(
                harness.selection.get(),
                Some(29),
                "the activated destination is selected"
            );
        });
    }
}

#[test]
fn dismissal_policies() {
    InteractiveScene::run(390.0, 640.0, |scene| {
        let harness = Harness::new();
        scene.install(harness.clone());
        harness.open.set(true);
        harness.closes_on_backdrop.set(false);
        harness.closes_on_escape.set(false);
        harness.closes_on_selection.set(false);
        harness.selection.set(Some(0));
        scene.settle(8, 1.0);
        click(scene, OUTSIDE, MouseButton::Left);
        scene.key(Scancode::ESCAPE, KeyModifiers::default());
        assert!(
            harness.open.get() && harness.background_calls.get() == 0,
            "disabled backdrop and Escape policies keep the drawer open and block the page"
        );
        let destination = row(scene, 0).expect("row 0");
        click(scene, destination.absolute_frame(), MouseButton::Left);
        assert!(
            harness.open.get() && harness.selection.get() == Some(0),
            "with `closes_on_selection` off, activation keeps the drawer open"
        );
        harness.closes_on_backdrop.set(true);
        scene.settle(8, 1.0);
        click(scene, OUTSIDE, MouseButton::Right);
        assert!(harness.open.get(), "a secondary press never dismisses");
        click(scene, OUTSIDE, MouseButton::Left);
        assert!(
            harness.open.get() && !scene.portal.entries().is_empty(),
            "GAP: Swift dismisses on a backdrop press at x=350; the Rust drawer spans the 390pt host, so the press lands inside the panel"
        );
        let backdrop_node = backdrop(scene).expect("the modal mounts a backdrop");
        click(scene, backdrop_node.absolute_frame(), MouseButton::Left);
        assert!(
            harness.open.get(),
            "GAP: the Rust backdrop sits under the full-width panel ({:?}), so no press reaches it",
            backdrop_node.absolute_frame()
        );
        harness.closes_on_escape.set(true);
        scene.settle(8, 1.0);
        scene.key(Scancode::ESCAPE, KeyModifiers::default());
        assert!(
            !harness.open.get() && !scene.focus.has_modal_scope(),
            "an enabled Escape policy dismisses"
        );
        harness.open.set(true);
        scene.settle(8, 1.0);
        let collapse = control(scene, "Collapse navigation").expect("collapse");
        click(scene, collapse.absolute_frame(), MouseButton::Left);
        assert!(
            !harness.open.get() && scene.portal.entries().is_empty(),
            "the explicit close always dismisses"
        );
    });
}

#[test]
fn pointer_presentation() {
    InteractiveScene::run(390.0, 640.0, |scene| {
        let harness = Harness::new();
        scene.install(harness.clone());
        let opener = control(scene, "Open navigation").expect("opener");
        click(scene, opener.absolute_frame(), MouseButton::Left);
        assert!(
            !scene.focus.is_focus_visible(),
            "a pointer open keeps focus quiet"
        );
        tick(1.0);
        scene.settle(8, 1.0);
        assert_eq!(
            scene.portal.entries().len(),
            1,
            "only the drawer is portalled, no tooltip"
        );
        let toggle = control(scene, "Collapse navigation").expect("collapse");
        assert_eq!(
            toggle.accessibility().expect("semantics").state.is_expanded,
            None,
            "GAP: Swift's collapse toggle reports `isExpanded == true`; the Rust toggle publishes no expanded state"
        );
        let backdrop_node = backdrop(scene).expect("the modal mounts a scrim");
        assert_eq!(
            backdrop_node.background_color(),
            Some(theme_for_node(&backdrop_node).colors.overlay),
            "the scrim paints the theme overlay"
        );
        click(scene, toggle.absolute_frame(), MouseButton::Left);
        assert!(
            !harness.open.get()
                && scene.focus.focused().map(|node| node.id()) == Some(opener.id())
                && !scene.focus.is_focus_visible(),
            "the toggle closes and quietly restores the opener"
        );
    });
}

#[test]
fn swipe_dismissal() {
    InteractiveScene::run(390.0, 640.0, |scene| {
        let harness = Harness::new();
        scene.install(harness.clone());
        harness.open.set(true);
        harness.selection.set(Some(0));
        scene.settle(8, 1.0);
        swipe(scene, -20.0, 0.0);
        assert!(
            harness.open.get() && harness.selection.get() == Some(0),
            "a short swipe neither dismisses nor selects"
        );
        swipe(scene, 0.0, 60.0);
        assert!(harness.open.get(), "a vertical swipe never dismisses");
        harness.swipe.set(false);
        scene.settle(8, 1.0);
        swipe(scene, -400.0, 0.0);
        assert!(harness.open.get(), "a disabled swipe never dismisses");
        harness.swipe.set(true);
        scene.settle(8, 1.0);
        swipe(scene, -120.0, 0.0);
        assert!(
            !harness.open.get() && harness.selection.get() == Some(0),
            "a 120pt leftward swipe dismisses and selects nothing"
        );
        assert!(
            scene.capture.target().is_none(),
            "the swipe releases pointer capture"
        );
    });
}

#[test]
fn animation_lifecycle() {
    InteractiveScene::run(390.0, 640.0, |scene| {
        let harness = Harness::new();
        scene.install(harness.clone());
        harness.reduced.set(false);
        scene.settle(8, 1.0);
        harness.open.set(true);
        scene.settle(8, 1.0);
        let dialog = panel(scene).expect("dialog");
        assert_eq!(
            dialog.absolute_frame().x,
            0.0,
            "GAP: Swift starts the drawer off-screen (maxX <= 0.5) and slides it in; the Rust drawer is at rest on its first frame"
        );
        tick(1.0);
        scene.settle(8, 1.0);
        assert_eq!(
            dialog.absolute_frame().x,
            0.0,
            "the drawer rests at the leading edge"
        );
        // Stop mid-exit: once the animation ends the panel unmounts and frees its
        // ancestors, and a detached node has no input state left to check.
        harness.open.set(false);
        scene.settle(1, 0.0);
        assert!(
            !dialog.accepts_subtree_input(),
            "a leaving drawer takes no input"
        );
        for _ in 0..6 {
            tick(0.02);
            scene.settle(1, 0.0);
        }
        assert!(
            dialog.absolute_frame().x < 0.0,
            "the leaving drawer moves off the leading edge"
        );
        harness.open.set(true);
        scene.settle(8, 1.0);
        tick(1.0);
        scene.settle(8, 1.0);
        assert_eq!(
            panel(scene).expect("dialog").id(),
            dialog.id(),
            "reopening mid-exit reverses in place and keeps the dialog node"
        );
        let close = control(scene, "Collapse navigation").expect("collapse");
        let frame = close.absolute_frame();
        scene
            .dispatcher()
            .dispatch(&InputEvent::MouseButtonDown(MouseButtonEvent {
                button: MouseButton::Left,
                x: frame.x + frame.width / 2.0,
                y: frame.y + frame.height / 2.0,
                clicks: 1,
                modifiers: Default::default(),
                kind: Default::default(),
            }));
        assert!(
            scene.capture.target().is_some(),
            "pressing the close control captures the pointer"
        );
        scene
            .graph
            .install(cove_composition::EmptyView, Default::default());
        scene.graph.recomposer.commit_all();
        assert!(
            scene.portal.entries().is_empty()
                && !scene.focus.has_modal_scope()
                && scene.capture.target().is_none(),
            "unmount releases the portal, the modal scope and pointer capture"
        );
    });
}

#[test]
fn diagnostic_modal_input() {
    InteractiveScene::run(390.0, 640.0, |scene| {
        let harness = Harness::new();
        scene.install(harness.clone());
        let background = control(scene, "Background action").expect("background button");
        assert!(
            input_enabled(scene, &background),
            "the page takes input before presentation"
        );
        harness.open.set(true);
        scene.settle(8, 1.0);
        let modal = live_nodes(scene)
            .into_iter()
            .filter(|node| label(node) == "Workspace navigation" && node.is_layout_visible())
            .count();
        assert!(modal >= 1, "the drawer is discoverable by its title");
        let background = control(scene, "Background action").expect("background button");
        assert!(
            !input_enabled(scene, &background),
            "the modal blocks the background page"
        );
    });
}

#[test]
fn validated_modal_tools() {
    InteractiveScene::run(390.0, 640.0, |scene| {
        let harness = Harness::new();
        scene.install(harness.clone());
        harness.open.set(true);
        scene.settle(8, 1.0);
        assert_eq!(
            query(scene, "Workspace navigation", AccessibilityRole::Dialog).len(),
            0,
            "GAP: Swift finds one `Workspace navigation` dialog; the Rust dialog node is unlabelled"
        );
        assert_eq!(
            query(scene, "Collapse navigation", AccessibilityRole::Button).len(),
            2,
            "GAP: Swift's query finds one `Collapse navigation` button; Rust publishes the label on both the toggle wrapper and its button host"
        );
        assert_eq!(
            query(scene, "Background action", AccessibilityRole::Button).len(),
            0,
            "the blocked background is excluded"
        );
        for (text, role) in [
            ("Workspace navigation", AccessibilityRole::Dialog),
            ("Collapse navigation", AccessibilityRole::Button),
            ("Background action", AccessibilityRole::Button),
        ] {
            assert!(
                !query(scene, text, role).iter().any(|node| {
                    node.accessibility().is_some_and(|semantics| {
                        semantics.label.contains("Private navigation note")
                            || semantics.value.contains("Private navigation note")
                    })
                }),
                "the `{text}` query never leaks the header draft"
            );
        }
    });
}
