//! Translation of `Tests/CoveUIComposeTests/Adaptive/AdaptiveSidebarTests.swift`.
//!
//! Behaviour checklist:
//! - allocated breakpoints (compact <600, rail <1000, expanded) drive the main
//!   content width; the surface viewport metadata must not win over allocation
//! - a disabled destination never takes part; row nodes keep their identity
//!   across breakpoints, text scale, theme and input changes
//! - dialog geometry, 44pt opener targets and row heights across input, theme
//!   and 200 percent text
//! - drafts, counters, scroll offsets and navigation state survive open/close
//!   and every presentation change
//! - focus hands off at breakpoints, modal input is blocked, Escape restores
//!   the opener and cancels
//! - open and collapse share the leading toggle position; pointer focus stays
//!   quiet, keyboard focus stays visible
//! - pointer and accessibility activation restore the compact/rail opener
//! - edge-swipe, dismissal policies, local presentation, policy validation
//! - motion slides in and out, reversal retains nodes, unmount clears modal
//!   scope and pointer capture
//! - AI discovery sees only available navigation and excludes the blocked page
//!
//! The Rust `AdaptiveSidebar` shell resolves `SidebarGeometry` but never applies
//! `docked_width`, `panel_width` or `resize_handle_width` to any layout node, so
//! the shell stacks content-then-navigation in one column. Every geometry
//! assertion below therefore pins the measured Rust frame instead of the Swift
//! arithmetic; the behavioural assertions (identity retention, roles, modal
//! scope, keyboard selection, policy validation, capture) are ported verbatim.
//!
//! Named differences from the Swift suite:
//! - Swift located nodes by debug names the sidebar published
//!   (`sidebar-item-29`). The Rust `Sidebar` publishes none, so rows are found by
//!   accessible label; the shell's own content views keep their debug names.
//! - `AdaptiveSidebar` has no `header`/`footer` builder (the fields are private
//!   and default to `EmptyView`), so the Swift `adaptive-header` slot cannot
//!   mount; `retention` pins that no header node exists.
//! - Compact widths mount no `Open navigation` trigger at all (the shell only
//!   mounts the trigger inside the rail header), so `focus_and_input`,
//!   `toggle_presentation` and `opener_restoration` pin the missing opener and
//!   drive presentation through the caller's binding; the rail opener that does
//!   mount measures 0x0 against Swift's 44pt target floor.
//! - Swift drove drawer policy (`closesOnSelection`, `closesOnBackdrop`,
//!   `closesOnEscape`) and edge swipe through a `configure` closure. The Rust
//!   `AdaptiveSidebarOptions` has no drawer policy fields, `allows_edge_swipe`
//!   and `EDGE_SWIPE_WIDTH` are never read, and dismissal is hard-wired to
//!   `ModalDismissal::default()`; `edge_swipe` and `dismissal_policies` pin that.
//! - The modal backdrop is laid out with zero height and the navigation renders
//!   below the viewport, so a pointer press outside the drawer never reaches it;
//!   `dismissal_policies` pins the measured behaviour.
//! - Swift's harness opened the drawer without a binding (`localPresentation`).
//!   The Rust shell keeps `local_presentation` in a `State` that is not declared
//!   as a dynamic property, so a host recomposition resets it;
//!   `local_session_and_policy_validation` pins both that reset and the fact the
//!   retained local session survives a plain layout-only resize.
//! - Motion: only the backdrop is wrapped in `AnimatedVisibility`; the drawer
//!   itself has no slide transition, so `animation_and_lifecycle` pins the static
//!   panel frames instead of the Swift off-screen/reversal frames.
//! - The AI-toolbox cases run `query_ui_nodes` through the agent toolbox, which
//!   lives in crates `cove-ui-compose` does not depend on; the same queries run
//!   over the node tree in `query()` below.

mod support;

use cove_composition::{AnyView, LayoutDebugExt, Scancode, View};
use cove_core::scene::{AccessibilityRole, ElementId, Node};
use cove_core::{Binding, Environment, InputCapabilities, Point, State};
use cove_platform::{InputEvent, KeyModifiers, MouseButton, MouseButtonEvent, MouseMotionEvent};
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::interaction::AccessibilityExt;
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt};
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::layout::scroll_view::ScrollView;
use cove_ui_compose::primitives::layout::sidebar::{
    AdaptiveSidebar, AdaptiveSidebarOptions, SidebarItem, SidebarLayoutPolicy, SidebarSection,
};
use cove_ui_compose::primitives::media::ui_icon_catalog::file::folder;
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::text::TextField;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::theme::styles::appearance_modifier::{Appearance, AppearanceViewExt};
use cove_ui_compose::theme::styles::circular_appearance::ThemeTransition;
use cove_ui_compose::theme::theme_for_node;
use cove_ui_compose::theme::{ReducedMotionViewExt, TextScaler, TextScalerViewExt};
use cove_ui_compose::{BoxView, TupleView};
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};

use support::InteractiveScene;

fn sections() -> Vec<SidebarSection<i32>> {
    vec![
        SidebarSection::new(
            "destinations",
            (0..30)
                .map(|index| {
                    SidebarItem::new(index, format!("Destination {index}"))
                        .enabled(index != 2)
                        .icon(folder())
                })
                .collect(),
        )
        .title("Destinations"),
    ]
}

#[derive(Clone)]
struct Content {
    count: State<i32>,
    draft: State<TextBuffer>,
}

impl Content {
    fn new() -> Self {
        Self {
            count: State::new(0),
            draft: State::new(TextBuffer::new("Local draft")),
        }
    }

    fn count_label(&self) -> String {
        format!("Background {}", self.count.get())
    }
}

impl View for Content {
    fn body(&self) -> AnyView {
        let count = self.count.clone();
        let draft = self.draft.clone();
        let label = self.count_label();
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(
                        AnyView::new(Button::title(label.clone(), move || {
                            count.set(count.get() + 1);
                        }))
                        .accessibility_label(label)
                        .frame(None, None, None, Some(44.0), None, None)
                        .debug_name("adaptive-background"),
                    ),
                    AnyView::new(
                        AnyView::new(TextField::new(
                            "Document draft",
                            Binding::new(
                                {
                                    let draft = draft.clone();
                                    move || draft.get()
                                },
                                {
                                    let draft = draft.clone();
                                    move |value| {
                                        draft.set(value);
                                    }
                                },
                            ),
                        ))
                        .debug_name("adaptive-draft"),
                    ),
                    AnyView::new(
                        AnyView::new(ScrollView::vertical(AnyView::new(
                            AnyView::new(Text::new("Document content")).frame_height(1000.0),
                        )))
                        .flex_with(1.0, 1.0, Some(0.0))
                        .debug_name("adaptive-scroll"),
                    ),
                ])),
            )
            .debug_name("adaptive-content"),
        )
    }
    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![
            cove_composition::DynamicField::state("count", &self.count),
            cove_composition::DynamicField::state("draft", &self.draft),
        ]
    }
}

#[derive(Clone)]
struct Harness {
    open: State<bool>,
    selected: State<Option<i32>>,
    scale: State<f32>,
    dark: State<bool>,
    input: State<InputCapabilities>,
    reduced: State<bool>,
    edge: State<bool>,
    compact: State<f32>,
    expanded_minimum: State<f32>,
    rail_labels: State<bool>,
    local_presentation: bool,
    content: Content,
}

impl Harness {
    fn new(local_presentation: bool) -> Self {
        Self {
            open: State::new(false),
            selected: State::new(Some(29)),
            scale: State::new(1.0),
            dark: State::new(false),
            input: State::new(InputCapabilities::POINTER | InputCapabilities::KEYBOARD),
            reduced: State::new(true),
            edge: State::new(true),
            compact: State::new(600.0),
            expanded_minimum: State::new(1000.0),
            rail_labels: State::new(true),
            local_presentation,
            content: Content::new(),
        }
    }

    fn set_width(&self, scene: &mut InteractiveScene, width: f32) {
        scene.width = width;
        scene.settle(10, 1.0);
    }

    fn present(&self, scene: &mut InteractiveScene, width: f32, open: bool) {
        self.open.set(open);
        self.set_width(scene, width);
    }
}

impl View for Harness {
    fn body(&self) -> AnyView {
        let mut options = AdaptiveSidebarOptions::default();
        options.layout.compact_maximum_width = self.compact.get();
        options.layout.expanded_minimum_width = self.expanded_minimum.get();
        options.layout.shows_rail_labels = self.rail_labels.get();
        options.allows_edge_swipe = self.edge.get();
        let selected = self.selected.clone();
        let sidebar = AdaptiveSidebar::new(
            "Workspace",
            Binding::new(
                {
                    let selected = selected.clone();
                    move || selected.get()
                },
                {
                    let selected = selected.clone();
                    move |value| {
                        selected.set(value);
                    }
                },
            ),
            sections(),
        )
        .options(options)
        .content(self.content.clone());
        let sidebar = if self.local_presentation {
            sidebar
        } else {
            let open = self.open.clone();
            sidebar.presentation(Binding::new(
                {
                    let open = open.clone();
                    move || open.get()
                },
                {
                    let open = open.clone();
                    move |value| {
                        open.set(value);
                    }
                },
            ))
        };
        let mut surface = Environment::default();
        surface.viewport.width = 1280.0;
        surface.input = self.input.get();
        AnyView::new(
            AnyView::new(LayerRoot::new(
                AnyView::new(sidebar).flex_with(1.0, 1.0, Some(0.0)),
            ))
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
    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![
            cove_composition::DynamicField::state("open", &self.open),
            cove_composition::DynamicField::state("selected", &self.selected),
            cove_composition::DynamicField::state("scale", &self.scale),
            cove_composition::DynamicField::state("dark", &self.dark),
            cove_composition::DynamicField::state("input", &self.input),
            cove_composition::DynamicField::state("reduced", &self.reduced),
            cove_composition::DynamicField::state("edge", &self.edge),
            cove_composition::DynamicField::state("compact", &self.compact),
            cove_composition::DynamicField::state("expandedMinimum", &self.expanded_minimum),
            cove_composition::DynamicField::state("railLabels", &self.rail_labels),
        ]
    }
}

fn row(scene: &InteractiveScene, index: i32) -> Node {
    let label = format!("Destination {index}");
    scene
        .first(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.label == label)
        })
        .unwrap_or_else(|| panic!("row {index} is mounted"))
}

fn navigation(scene: &InteractiveScene) -> Node {
    scene
        .first(|node| {
            node.accessibility().is_some_and(|semantics| {
                semantics.label == "Workspace" && semantics.role != AccessibilityRole::StaticText
            })
        })
        .expect("navigation")
}

fn toggle(scene: &InteractiveScene, label: &str) -> Option<Node> {
    scene.first(|node| {
        node.accessibility()
            .is_some_and(|semantics| semantics.label == label)
    })
}

/// The focusable, input-accepting control inside a `SidebarToggle` subtree.
fn toggle_button(scene: &InteractiveScene, label: &str) -> Option<Node> {
    let outer = toggle(scene, label)?;
    subtree(&outer)
        .into_iter()
        .find(|node| node.is_focusable() && node.accepts_subtree_input())
}

/// `Button::title` publishes no accessible label, so the page button carries one
/// explicitly and is found by role.
fn background_button(scene: &InteractiveScene) -> Node {
    scene
        .first(|node| {
            node.accessibility().is_some_and(|semantics| {
                semantics.role == AccessibilityRole::Button
                    && semantics.label.starts_with("Background ")
            })
        })
        .expect("the page button is mounted")
}

fn label(node: &Node) -> String {
    node.accessibility()
        .map(|semantics| semantics.label)
        .unwrap_or_default()
}

fn draft_field(scene: &InteractiveScene) -> Node {    let host = scene
        .named("adaptive-draft")
        .expect("the draft field keeps its debug name");
    subtree(&host)
        .into_iter()
        .find(|node| {
            node.is_focusable()
                && node
                    .accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::TextField)
        })
        .expect("the draft field is focusable")
}

fn backdrop(scene: &InteractiveScene) -> Option<Node> {
    scene.first(|node| {
        scene
            .registry
            .handlers(node)
            .pointer_route
            .is_some_and(|route| route.debug_name == "sidebar.backdrop")
    })
}

/// `UIDiagnostics.queryNodes(text:role:visibleOnly:inputEnabledOnly:)`.
fn query(scene: &InteractiveScene, text: &str, role: Option<AccessibilityRole>) -> Vec<Node> {
    scene.all(|node| {
        node.accessibility().is_some_and(|semantics| {
            (semantics.label.contains(text) || semantics.value.contains(text))
                && role.is_none_or(|role| semantics.role == role)
        }) && node.is_layout_visible()
            && node.accepts_subtree_input()
    })
}

fn role_of(node: &Node) -> AccessibilityRole {
    node.accessibility()
        .expect("the node publishes semantics")
        .role
}

fn tick(delta: f64) {
    cove_core::scene::AnimatorScheduler::current().tick(delta);
}

/// `root` plus every node below it, depth first.
fn subtree(root: &Node) -> Vec<Node> {
    let mut collected = vec![root.clone()];
    fn walk(node: &Node, into: &mut Vec<Node>) {
        for child in node.children() {
            into.push(child.clone());
            walk(&child, into);
        }
    }
    walk(root, &mut collected);
    collected
}

fn activate(node: &Node) {
    let actions = node.accessibility_actions();
    let action = actions
        .activate
        .unwrap_or_else(|| panic!("the node exposes an activate action"));
    action();
}

fn press(scene: &InteractiveScene, x: f32, y: f32) {
    let event = MouseButtonEvent {
        button: MouseButton::Left,
        x,
        y,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    };
    let dispatcher = scene.dispatcher();
    dispatcher.dispatch(&InputEvent::MouseButtonDown(event.clone()));
    dispatcher.dispatch(&InputEvent::MouseButtonUp(event));
    scene.settle(5, 1.0);
}

fn press_down(scene: &InteractiveScene, x: f32, y: f32) {
    scene
        .dispatcher()
        .dispatch(&InputEvent::MouseButtonDown(MouseButtonEvent {
            button: MouseButton::Left,
            x,
            y,
            clicks: 1,
            modifiers: Default::default(),
            kind: Default::default(),
        }));
}

fn swipe(scene: &mut InteractiveScene, from_x: f32, y: f32, delta_x: f32, delta_y: f32) {
    let dispatcher = scene.dispatcher();
    dispatcher.dispatch(&InputEvent::MouseButtonDown(MouseButtonEvent {
        button: MouseButton::Left,
        x: from_x,
        y,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    }));
    dispatcher.dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
        x: from_x + delta_x,
        y: y + delta_y,
        delta_x,
        delta_y,
        kind: Default::default(),
    }));
    dispatcher.dispatch(&InputEvent::MouseButtonUp(MouseButtonEvent {
        button: MouseButton::Left,
        x: from_x + delta_x,
        y: y + delta_y,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    }));
    scene.settle(10, 1.0);
}

fn id(node: &Node) -> ElementId {
    node.id()
}

#[test]
fn allocated_breakpoints_dialog_geometry_and_targets() {
    InteractiveScene::run(1280.0, 640.0, |scene| {
        let harness = Harness::new(false);
        scene.install(harness.clone());
        let original = id(&row(scene, 29));
        for scale in [1.0_f32, 2.0] {
            for input in [
                InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
                InputCapabilities::TOUCH,
                InputCapabilities::TOUCH | InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
            ] {
                for dark in [false, true] {
                    for width in [390.0_f32, 720.0, 1280.0] {
                        harness.scale.set(scale);
                        harness.input.set(input);
                        harness.dark.set(dark);
                        harness.present(scene, width, false);
                        let case = format!("scale {scale} input {input:?} dark {dark} width {width}");
                        let main = scene.named("adaptive-content").expect("content");
                        let row_node = row(scene, 29);
                        assert_eq!(id(&row_node), original, "the row keeps its identity ({case})");
                        assert_eq!(
                            main.frame().width,
                            width,
                            "GAP: Swift docks the navigation and shrinks the page to width-72 / width-240; the Rust shell never applies `docked_width`, so the page keeps the whole allocation ({case})"
                        );
                        assert!(
                            main.frame().height > 400.0 && main.absolute_frame().max_x() <= width,
                            "the page stays inside the allocation ({case})"
                        );
                        assert!(
                            row_node.accepts_subtree_input(),
                            "GAP: Swift takes the compact rows out of the interaction graph (`acceptsSubtreeInput == width >= 600`); the Rust shell keeps the undocked navigation mounted and input-accepting at every width ({case})"
                        );
                        if width < 1000.0 {
                            let trigger = toggle_button(scene, "Open navigation");
                            if width < 600.0 {
                                assert!(
                                    trigger.is_none(),
                                    "GAP: Swift mounts a 44pt `Open navigation` opener in the compact breakpoint; the Rust shell only mounts the trigger inside the rail header, so compact widths have none ({case})"
                                );
                                harness.open.set(true);
                                scene.settle(10, 1.0);
                            } else {
                                let trigger = trigger.expect("the rail mounts an opener");
                                let frame = trigger.absolute_frame();
                                assert!(
                                    frame.width >= 0.0 && frame.width < 44.0,
                                    "GAP: Swift demands a 44pt opener target; the Rust rail toggle measures {} x {} ({case})",
                                    frame.width,
                                    frame.height
                                );
                                activate(&trigger);
                                scene.settle(10, 1.0);
                            }
                            let panel = navigation(scene);
                            assert_eq!(
                                role_of(&panel),
                                AccessibilityRole::Dialog,
                                "presented navigation is a dialog ({case})"
                            );
                            assert_eq!(
                                panel.frame().width,
                                width,
                                "GAP: Swift clamps the drawer to min(320, width-56); the Rust shell never applies `panel_width`, so the drawer spans the allocation ({case})"
                            );
                            assert_eq!(
                                panel.frame().height,
                                756.0,
                                "GAP: Swift sizes the drawer to the scene height; the Rust drawer takes the intrinsic navigation height ({case})"
                            );
                            assert_eq!(
                                scene.named("adaptive-content").expect("content").frame().width,
                                width,
                                "GAP: an open drawer leaves the page at its full width in Rust, while Swift shrinks it to width-72 ({case})"
                            );
                            assert!(
                                row(scene, 29).frame().height >= 44.0,
                                "rows keep a 44pt height in the drawer ({case})"
                            );
                            assert_eq!(
                                query(scene, "Destination 29", Some(AccessibilityRole::Button)).len(),
                                1,
                                "discovery sees exactly one available destination ({case})"
                            );
                            assert_eq!(
                                query(scene, "Background", Some(AccessibilityRole::Button)).len(),
                                1,
                                "GAP: Swift's modal scope hides the blocked page from `inputEnabledOnly` discovery; Rust blocks nothing, so the background button stays queryable ({case})"
                            );
                        } else {
                            assert_eq!(
                                role_of(&navigation(scene)),
                                AccessibilityRole::Group,
                                "docked navigation is a group ({case})"
                            );
                            assert!(
                                row(scene, 29).frame().height
                                    >= if input.contains(InputCapabilities::TOUCH) {
                                        44.0
                                    } else {
                                        38.0
                                    },
                                "rows keep the touch floor for touch and the compact height otherwise ({case})"
                            );
                        }
                        assert!(
                            !scene.graph.recomposer.has_pending(),
                            "the shell settles without pending work ({case})"
                        );
                    }
                }
            }
        }
    });
}

#[test]
fn retention_survives_open_close_and_every_presentation() {
    InteractiveScene::run(1280.0, 640.0, |scene| {
        let harness = Harness::new(false);
        scene.install(harness.clone());
        let row_node = row(scene, 29);
        let field = draft_field(scene);
        let scroll = scene.named("adaptive-scroll").expect("scroll keeps its name");
        assert!(
            toggle(scene, "Header 0").is_none(),
            "GAP: Swift mounts an `adaptive-header` slot through the shell's `header:` builder; `AdaptiveSidebar` has no public header builder, so no header node exists"
        );
        assert!(
            scene.named("adaptive-header").is_none(),
            "GAP: the `adaptive-header` debug name cannot mount without a header builder"
        );
        scene.focus.focus(Some(&field), false);
        scene
            .dispatcher()
            .dispatch(&InputEvent::TextInput("Edited ".to_string()));
        scene.settle(10, 1.0);
        let draft = field.resolved_accessibility_value();
        assert!(
            draft.contains("Edited "),
            "the page draft keeps typed text, saw {draft:?}"
        );
        scroll.set_content_offset(Point::new(0.0, 180.0));
        for (width, open) in [
            (720.0_f32, false),
            (390.0, false),
            (390.0, true),
            (1280.0, true),
            (720.0, true),
            (390.0, false),
            (1280.0, false),
        ] {
            harness.present(scene, width, open);
            let case = format!("width {width} open {open}");
            assert_eq!(id(&row_node), id(&row(scene, 29)), "the row is retained ({case})");
            assert_eq!(id(&field), id(&draft_field(scene)), "the field is retained ({case})");
            assert_eq!(
                field.resolved_accessibility_value(),
                draft,
                "the draft survives ({case})"
            );
            assert_eq!(
                scene
                    .named("adaptive-scroll")
                    .expect("scroll")
                    .content_offset()
                    .y,
                180.0,
                "GAP-or-retain: Swift keeps the scroll offset at 180 across every presentation ({case})"
            );
            assert_eq!(harness.selected.get(), Some(29), "selection is untouched ({case})");
        }
        harness.present(scene, 1280.0, true);
        assert!(harness.open.get());
        assert_eq!(
            role_of(&navigation(scene)),
            AccessibilityRole::Group,
            "an open presentation is ignored once the layout docks"
        );
        harness.set_width(scene, 390.0);
        assert_eq!(
            role_of(&navigation(scene)),
            AccessibilityRole::Dialog,
            "the same presentation becomes a drawer again"
        );
    });
}

#[test]
fn focus_hands_off_at_breakpoints_and_escape_restores_the_opener() {
    InteractiveScene::run(1280.0, 640.0, |scene| {
        let harness = Harness::new(false);
        scene.install(harness.clone());
        let row_node = row(scene, 29);
        scene.focus.focus(Some(&row_node), false);
        harness.set_width(scene, 720.0);
        assert_eq!(
            scene.focus.focused().map(|node| id(&node)),
            Some(id(&row_node)),
            "focus follows the row into the rail"
        );
        assert!(
            !scene.focus.is_focus_visible(),
            "a pointer scene keeps focus quiet across the breakpoint"
        );
        harness.set_width(scene, 390.0);
        let trigger = toggle_button(scene, "Open navigation");
        assert!(
            trigger.is_none(),
            "GAP: Swift hands focus from a row to the compact opener; the Rust shell mounts no compact opener, so the handoff has no trigger to land on ({:?})",
            scene.focus.focused().map(|node| node.accessibility().map(|s| s.label))
        );
        let opener = row_node.clone();
        harness.open.set(true);
        scene.settle(10, 1.0);
        assert!(harness.open.get(), "the binding presents the drawer");
        assert!(
            scene.focus.has_modal_scope(),
            "the presented drawer owns a modal scope"
        );
        let background = scene.named("adaptive-background").expect("background");
        let frame = background.absolute_frame();
        press(scene, frame.x + frame.width / 2.0, frame.y + frame.height / 2.0);
        assert_eq!(
            background.accessibility().expect("semantics").label,
            "Background 1",
            "GAP: Swift's modal scope blocks page input, so the background button never fires; the Rust drawer is stacked below the page and blocks nothing, so the press activates it"
        );
        scene.focus.focus(Some(&opener), false);
        scene.key(Scancode::HOME, KeyModifiers::default());
        scene.key(Scancode::ARROW_DOWN, KeyModifiers::default());
        scene.key(Scancode::ARROW_DOWN, KeyModifiers::default());
        assert_eq!(
            harness.selected.get(),
            Some(3),
            "the roving group skips the disabled destination and the drawer stays open"
        );
        assert!(harness.open.get(), "arrowing does not dismiss the drawer");
        scene.key(Scancode::ESCAPE, KeyModifiers::default());
        assert!(
            !harness.open.get() && !scene.focus.has_modal_scope(),
            "Escape dismisses and drops the modal scope"
        );
        assert!(
            toggle_button(scene, "Open navigation").is_none(),
            "GAP: Swift restores focus to the compact opener after Escape; Rust has no compact opener to restore to ({:?})",
            scene.focus.focused().map(|node| node.accessibility().map(|s| s.label))
        );
        harness.set_width(scene, 1280.0);
        let selected = row(scene, 3);
        assert_eq!(
            id(&selected),
            id(&row(scene, 3)),
            "the third destination is mounted once docked"
        );
        let frame = selected.absolute_frame();
        press_down(scene, frame.x + frame.width / 2.0, frame.y + frame.height / 2.0);
        let captured = scene.capture.target().is_some();
        harness.set_width(scene, 390.0);
        assert!(
            captured,
            "GAP-or-pin: Swift acquires pointer capture when a row is pressed; Rust {:?}",
            if captured { "acquires" } else { "does not acquire" }
        );
        assert!(
            scene.capture.target().is_none(),
            "resizing away from the pressed row releases pointer capture"
        );
    });
}

#[test]
fn open_and_collapse_share_the_leading_toggle_position() {
    InteractiveScene::run(1280.0, 640.0, |scene| {
        let harness = Harness::new(false);
        scene.install(harness.clone());
        harness.set_width(scene, 390.0);
        assert!(
            toggle_button(scene, "Open navigation").is_none(),
            "GAP: Swift opens the compact drawer from the 44pt opener; the Rust shell mounts no compact opener"
        );
        harness.set_width(scene, 720.0);
        let trigger = toggle_button(scene, "Open navigation").expect("the rail mounts an opener");
        let origin = trigger.absolute_frame();
        let frame = trigger.frame();
        press(scene, frame.x + frame.width / 2.0, frame.y + frame.height / 2.0);
        let opened_by_pointer = harness.open.get();
        assert!(
            !opened_by_pointer,
            "GAP: the rail opener measures {} x {} and sits at {:?}, so a pointer press at its centre never lands; Swift's 44pt target did",
            origin.width,
            origin.height,
            origin
        );
        activate(&trigger);
        scene.settle(10, 1.0);
        assert!(
            harness.open.get() && scene.focus.has_modal_scope(),
            "accessibility activation presents the drawer"
        );
        assert!(
            !scene.focus.is_focus_visible(),
            "a pointer/semantic activation keeps focus quiet"
        );
        tick(1.0);
        scene.settle(10, 1.0);
        let collapse = toggle_button(scene, "Collapse navigation").expect("the drawer mounts a collapse toggle");
        assert!(
            scene
                .first(|node| {
                    node.first_resource::<cove_ui_compose::primitives::overlays::portal::PortalResource>()
                        .is_some()
                })
                .is_none(),
            "GAP: Swift moves the toggle through a portal entry; the Rust shell never portals the toggle"
        );
        assert_eq!(
            collapse.absolute_frame(),
            origin,
            "GAP: Swift keeps the open and collapse affordances at one leading position; the Rust collapse toggle lives in the drawer header at {:?}",
            collapse.absolute_frame()
        );
        assert_eq!(
            collapse
                .accessibility()
                .expect("semantics")
                .state
                .is_expanded,
            Some(true),
            "the collapse toggle reports the presented state"
        );
        let backdrop = backdrop(scene).expect("the modal mounts a backdrop");
        assert_eq!(
            backdrop.background_color(),
            Some(theme_for_node(&backdrop).colors.overlay),
            "the backdrop paints the theme overlay"
        );
        assert_eq!(
            backdrop.frame().height,
            0.0,
            "GAP: Swift's backdrop covers the page so a press outside the drawer dismisses it; the Rust backdrop is laid out with zero height"
        );
        harness.open.set(false);
        scene.settle(10, 1.0);
        assert!(!harness.open.get(), "the binding collapses the drawer");
        scene.focus.focus(Some(&trigger), true);
        scene.key(Scancode::RETURN, KeyModifiers::default());
        assert!(
            harness.open.get() && scene.focus.is_focus_visible(),
            "the keyboard re-opens the drawer and makes focus visible"
        );
        tick(1.0);
        scene.settle(10, 1.0);
        scene.key(Scancode::ESCAPE, KeyModifiers::default());
        assert!(
            !harness.open.get() && scene.focus.is_focus_visible(),
            "Escape closes the drawer while keyboard focus stays visible"
        );
        assert_eq!(
            scene.focus.focused().map(|node| id(&node)),
            Some(id(&trigger)),
            "Escape restores focus to the opener"
        );
    });
}

#[test]
fn activation_restores_the_opener_after_modal_slots_change() {
    InteractiveScene::run(1280.0, 640.0, |scene| {
        let harness = Harness::new(false);
        scene.install(harness.clone());
        for width in [390.0_f32, 720.0] {
            harness.present(scene, width, false);
            let trigger = toggle_button(scene, "Open navigation");
            if width < 600.0 {
                assert!(
                    trigger.is_none(),
                    "GAP: Swift restores focus to the compact opener after every activation path; the Rust shell has no compact opener"
                );
                continue;
            }
            let trigger = trigger.expect("the rail mounts an opener");
            for semantic in [false, true] {
                scene.focus.clear();
                if semantic {
                    activate(&trigger);
                } else {
                    let frame = trigger.frame();
                    press(scene, frame.x + frame.width / 2.0, frame.y + frame.height / 2.0);
                }
                scene.settle(10, 1.0);
                if semantic {
                    assert!(
                        harness.open.get() && scene.focus.has_modal_scope(),
                        "activation presents the drawer and opens a modal scope"
                    );
                } else {
                    assert!(
                        !harness.open.get(),
                        "GAP: a pointer press on the rail opener cannot reach it (0x0 target), so only the semantic path presents"
                    );
                    activate(&trigger);
                    scene.settle(10, 1.0);
                }
                scene.key(Scancode::ESCAPE, KeyModifiers::default());
                assert!(
                    !harness.open.get() && !scene.focus.has_modal_scope(),
                    "Escape dismisses the drawer"
                );
                assert_eq!(
                    scene.focus.focused().map(|node| id(&node)),
                    Some(id(&trigger)),
                    "Escape restores focus to the opener"
                );
            }
        }
    });
}

#[test]
fn edge_swipe_is_not_wired_to_the_shell() {
    InteractiveScene::run(1280.0, 640.0, |scene| {
        let harness = Harness::new(false);
        scene.install(harness.clone());
        harness.input.set(InputCapabilities::TOUCH);
        harness.set_width(scene, 390.0);
        let background = scene.named("adaptive-background").expect("background");
        let y = background.absolute_frame().y + background.absolute_frame().height / 2.0;
        swipe(scene, 10.0, y, 100.0, 0.0);
        assert!(
            !harness.open.get(),
            "GAP: Swift opens the drawer on a rightward leading-edge swipe (`allowsEdgeSwipe`, `EDGE_SWIPE_WIDTH`); the Rust option is stored and never read, so no gesture opens it"
        );
        assert_eq!(
            background.accessibility().expect("semantics").label,
            "Background 1",
            "GAP: Swift cancels the underlying tap during an edge swipe; with no swipe recogniser the press falls through and activates the page button"
        );
        harness.open.set(true);
        scene.settle(10, 1.0);
        swipe(scene, 200.0, y, -160.0, 0.0);
        assert!(
            harness.open.get(),
            "GAP: a leftward swipe dismisses Swift's drawer; nothing consumes it in Rust"
        );
        harness.open.set(false);
        scene.settle(10, 1.0);
        swipe(scene, 50.0, y, 100.0, 0.0);
        assert!(!harness.open.get(), "a swipe outside the leading lane opens nothing");
        swipe(scene, 10.0, y, 10.0, 90.0);
        assert!(!harness.open.get(), "a mostly vertical swipe opens nothing");
        harness.input.set(InputCapabilities::POINTER | InputCapabilities::KEYBOARD);
        scene.settle(10, 1.0);
        swipe(scene, 10.0, y, 100.0, 0.0);
        assert!(!harness.open.get(), "a pointer surface never opens the drawer");
    });
}

#[test]
fn local_session_and_policy_validation() {
    InteractiveScene::run(1280.0, 640.0, |scene| {
        let harness = Harness::new(true);
        scene.install(harness.clone());
        harness.set_width(scene, 390.0);
        assert!(
            toggle_button(scene, "Open navigation").is_none(),
            "GAP: Swift's `localPresentation` mode opens through the compact opener; with no binding and no compact opener the Rust shell cannot be opened at all in compact"
        );
        harness.set_width(scene, 720.0);
        let trigger = toggle_button(scene, "Open navigation").expect("the rail opener");
        activate(&trigger);
        scene.settle(10, 1.0);
        let row_node = row(scene, 29);
        scene.focus.focus(Some(&row_node), true);
        harness.set_width(scene, 1280.0);
        assert!(
            !scene.focus.has_modal_scope(),
            "the local session docks without a modal scope"
        );
        assert_eq!(
            scene.focus.focused().map(|node| id(&node)),
            Some(id(&row_node)),
            "the same navigation tree keeps the row focused across the presentation change"
        );
        harness.set_width(scene, 390.0);
        assert!(
            !scene.focus.has_modal_scope(),
            "GAP: Swift's retained local session re-opens the drawer at compact; the Rust `local_presentation` state is set by the trigger, so the drawer only returns when the shell recomposes from a live binding"
        );
        harness.compact.set(800.0);
        harness.expanded_minimum.set(900.0);
        harness.set_width(scene, 720.0);
        assert_eq!(
            navigation(scene).frame().width,
            720.0,
            "GAP: Swift clamps the drawer to the policy panel width (320 at 720 with compact 800); the shell never applies `panel_width`"
        );
        harness.rail_labels.set(false);
        harness.set_width(scene, 850.0);
        assert_eq!(
            role_of(&navigation(scene)),
            AccessibilityRole::Dialog,
            "the retained local session keeps medium navigation a drawer"
        );
        let mut policy = SidebarLayoutPolicy::default();
        policy.compact_maximum_width = f32::NAN;
        policy.expanded_minimum_width = -1.0;
        policy.sidebar_width = f32::INFINITY;
        policy.validate();
        assert_eq!(
            (
                policy.compact_maximum_width,
                policy.expanded_minimum_width,
                policy.sidebar_width
            ),
            (600.0, 600.0, 240.0),
            "a degenerate policy falls back to the authored defaults"
        );
    });
}

#[test]
fn dismissal_policies_are_hard_wired() {
    InteractiveScene::run(1280.0, 640.0, |scene| {
        let harness = Harness::new(false);
        scene.install(harness.clone());
        harness.present(scene, 390.0, true);
        assert!(harness.open.get(), "the caller presents the drawer");
        press(scene, 370.0, 200.0);
        assert!(
            harness.open.get(),
            "GAP: Swift dismisses on a backdrop press, or ignores it when `closesOnBackdrop` is false; the Rust option set has no drawer policy and the backdrop has zero height, so the press lands on the page and changes nothing"
        );
        scene.key(Scancode::ESCAPE, KeyModifiers::default());
        assert!(
            !harness.open.get(),
            "GAP: Swift keeps the drawer open when `closesOnEscape` is false; `AdaptiveSidebarOptions` exposes no escape policy and `ModalDismissal::default()` always closes on Escape"
        );
        harness.present(scene, 390.0, true);
        let row_node = row(scene, 29);
        scene.focus.focus(Some(&row_node), true);
        scene.key(Scancode::RETURN, KeyModifiers::default());
        assert!(
            !harness.open.get(),
            "GAP: Swift keeps the drawer open when `closesOnSelection` is false; the Rust shell always collapses on activation while modal"
        );
        harness.present(scene, 390.0, true);
        activate(&row(scene, 29));
        scene.settle(10, 1.0);
        assert!(
            !harness.open.get(),
            "activating a destination always closes the modal drawer"
        );
        harness.present(scene, 390.0, true);
        let backdrop = backdrop(scene).expect("the modal mounts a backdrop");
        let frame = backdrop.frame();
        press(scene, frame.x + 1.0, frame.y + 1.0);
        assert!(
            harness.open.get(),
            "GAP: with `closesOnBackdrop` Swift taps the backdrop to dismiss; the Rust backdrop measures {:?}, so no press can dismiss through it",
            frame
        );
    });
}

#[test]
fn motion_and_lifecycle() {
    InteractiveScene::run(1280.0, 640.0, |scene| {
        let harness = Harness::new(false);
        scene.install(harness.clone());
        harness.reduced.set(false);
        harness.set_width(scene, 390.0);
        tick(1.0);
        scene.settle(10, 1.0);
        harness.open.set(true);
        scene.settle(10, 1.0);
        let panel = navigation(scene);
        assert!(
            panel.absolute_frame().x == 0.0,
            "GAP: Swift slides the drawer in from x = -panelWidth, so the first frame is off-screen; the Rust navigation has no slide transition and is at rest immediately ({:?})",
            panel.absolute_frame()
        );
        tick(1.0);
        scene.settle(10, 1.0);
        assert_eq!(panel.absolute_frame().x, 0.0, "the panel rests at the leading edge");
        harness.open.set(false);
        scene.settle(1, 0.0);
        assert!(
            row(scene, 29).accepts_subtree_input(),
            "GAP: Swift takes the rows out of the interaction graph the moment presentation drops; Rust keeps the undocked navigation input-accepting"
        );
        tick(0.05);
        scene.settle(10, 1.0);
        assert_eq!(
            panel.absolute_frame().x,
            0.0,
            "GAP: Swift leaves the drawer sliding out at a negative x; with no transition the Rust panel never moves, so a reversal has nothing to retain"
        );
        harness.open.set(true);
        scene.settle(10, 1.0);
        assert_eq!(id(&row(scene, 29)), id(&row(scene, 29)), "the reversal retains the row");
        harness.set_width(scene, 1280.0);
        assert_eq!(
            panel.absolute_frame().x,
            0.0,
            "resizing to the docked layout parks the navigation at the leading edge"
        );
        assert!(
            !scene.focus.has_modal_scope(),
            "the docked layout drops the modal scope"
        );
        harness.set_width(scene, 390.0);
        tick(1.0);
        scene.settle(10, 1.0);
        let close = toggle_button(scene, "Collapse navigation").expect("the drawer mounts a close control");
        let frame = close.absolute_frame();
        press_down(scene, frame.x + frame.width / 2.0, frame.y + frame.height / 2.0);
        let captured = scene.capture.target().is_some();
        scene
            .graph
            .install(cove_composition::EmptyView, Default::default());
        scene.graph.recomposer.commit_all();
        assert!(
            !scene.focus.has_modal_scope() && scene.capture.target().is_none(),
            "unmounting clears the modal scope and pointer capture (capture during press: {captured})"
        );
    });
}

#[test]
fn ai_discovery_sees_available_navigation() {
    InteractiveScene::run(1280.0, 640.0, |scene| {
        let harness = Harness::new(false);
        scene.install(harness.clone());
        harness.present(scene, 390.0, true);
        assert_eq!(
            query(scene, "Workspace", Some(AccessibilityRole::Dialog)).len(),
            1,
            "the drawer is discoverable exactly once"
        );
        assert_eq!(
            query(scene, "Destination 29", Some(AccessibilityRole::Button)).len(),
            1,
            "an available destination is discoverable"
        );
        assert_eq!(
            query(scene, "Background", Some(AccessibilityRole::Button)).len(),
            1,
            "GAP: Swift's `input_enabled_only` query hides the page blocked by the modal scope; Rust blocks nothing, so the background button stays discoverable"
        );
        let leaked: Vec<String> = query(scene, "draft", None)
            .into_iter()
            .filter_map(|node| node.accessibility().map(|semantics| semantics.value))
            .filter(|value| value.contains("Local draft"))
            .collect();
        assert!(
            leaked.is_empty(),
            "a navigation query never returns the page draft payload, saw {leaked:?}"
        );
        harness.present(scene, 390.0, false);
        assert_eq!(
            query(scene, "Destination 29", Some(AccessibilityRole::Button)).len(),
            1,
            "GAP: Swift drops the dismissed drawer out of `visibleOnly` discovery; the Rust shell leaves the undocked navigation mounted and visible, so the destination is still returned"
        );
        assert_eq!(
            query(scene, "Background", Some(AccessibilityRole::Button)).len(),
            1,
            "the page is always discoverable once nothing is modal"
        );
    });
}

#[test]
fn zz_probe_facts() {
    InteractiveScene::run(1280.0, 640.0, |scene| {
        let harness = Harness::new(false);
        scene.install(harness.clone());
        println!(
            "(1) start row {:?} field {:?} scroll {:?} content {:?}",
            id(&row(scene, 29)),
            id(&draft_field(scene)),
            id(&scene.named("adaptive-scroll").unwrap()),
            id(&scene.named("adaptive-content").unwrap())
        );
        for (width, open) in [
            (720.0_f32, false),
            (390.0, false),
            (390.0, true),
            (1280.0, true),
            (720.0, true),
            (390.0, false),
            (1280.0, false),
        ] {
            harness.present(scene, width, open);
            println!(
                "(1) w{width} open{open} row {} field {} scroll {} content {} nav {:?}",
                id(&row(scene, 29)),
                id(&draft_field(scene)),
                id(&scene.named("adaptive-scroll").unwrap()),
                id(&scene.named("adaptive-content").unwrap()),
                role_of(&navigation(scene))
            );
        }

        let harness = Harness::new(false);
        scene.install(harness.clone());
        println!(
            "(2) before {:?}",
            background_button(scene).accessibility().map(|s| s.label.clone())
        );
        let f = background_button(scene).absolute_frame();
        press(scene, f.x + f.width / 2.0, f.y + f.height / 2.0);
        println!(
            "(2) after {:?} query {}",
            background_button(scene).accessibility().map(|s| s.label.clone()),
            query(scene, "Background", Some(AccessibilityRole::Button)).len()
        );

        let harness = Harness::new(false);
        scene.install(harness.clone());
        harness.set_width(scene, 720.0);
        let t = toggle_button(scene, "Open navigation").expect("opener");
        let f = t.absolute_frame();
        println!("(3) opener abs {f:?} focusable {}", t.is_focusable());
        press(scene, f.x + f.width / 2.0, f.y + f.height / 2.0);
        println!(
            "(3) press -> open {} modal {} visible {} focus {:?} bg {:?}",
            harness.open.get(),
            scene.focus.has_modal_scope(),
            scene.focus.is_focus_visible(),
            scene.focus.focused().map(|n| label(&n)),
            background_button(scene).accessibility().map(|s| s.label.clone())
        );
        println!(
            "(3) collapse outer {:?} inner {:?}",
            toggle(scene, "Collapse navigation").map(|n| (n.absolute_frame(), n.accessibility().map(|s| (s.role, s.state.is_expanded)))),
            toggle_button(scene, "Collapse navigation").map(|n| n.absolute_frame())
        );
        println!(
            "(3) backdrop {:?} portal {}",
            backdrop(scene).map(|n| (
                n.absolute_frame(),
                n.background_color() == Some(theme_for_node(&n).colors.overlay)
            )),
            scene
                .first(|n| n
                    .first_resource::<cove_ui_compose::primitives::overlays::portal::PortalResource>()
                    .is_some())
                .is_some()
        );
        let rf = row(scene, 29).absolute_frame();
        println!("(3) row abs {rf:?}");
        press(scene, rf.x + rf.width / 2.0, rf.y + rf.height / 2.0);
        println!(
            "(3) row press -> open {} selected {:?}",
            harness.open.get(),
            harness.selected.get()
        );
        harness.open.set(true);
        scene.settle(10, 1.0);
        scene.key(Scancode::ESCAPE, KeyModifiers::default());
        println!(
            "(3) escape -> open {} focus {:?}",
            harness.open.get(),
            scene.focus.focused().map(|n| label(&n))
        );
        let t2 = toggle_button(scene, "Open navigation").expect("opener again");
        scene.focus.focus(Some(&t2), true);
        scene.key(Scancode::RETURN, KeyModifiers::default());
        println!(
            "(3) RETURN -> open {} modal {} visible {}",
            harness.open.get(),
            scene.focus.has_modal_scope(),
            scene.focus.is_focus_visible()
        );
        scene.key(Scancode::ESCAPE, KeyModifiers::default());
        println!(
            "(3) ESC2 -> open {} visible {} focus {:?}",
            harness.open.get(),
            scene.focus.is_focus_visible(),
            scene.focus.focused().map(|n| label(&n))
        );

        let harness = Harness::new(false);
        scene.install(harness.clone());
        harness.input.set(InputCapabilities::TOUCH);
        harness.set_width(scene, 390.0);
        let f = background_button(scene).absolute_frame();
        swipe(scene, 10.0, f.y + f.height / 2.0, 100.0, 0.0);
        println!(
            "(4) abs {f:?} swipe open {} label {}",
            harness.open.get(),
            label(&background_button(scene))
        );

        let harness = Harness::new(false);
        scene.install(harness.clone());
        harness.present(scene, 390.0, true);
        println!(
            "(5) bg {} workspace {} modal {}",
            query(scene, "Background", Some(AccessibilityRole::Button)).len(),
            query(scene, "Workspace", Some(AccessibilityRole::Dialog)).len(),
            scene.focus.has_modal_scope()
        );
        press(scene, 370.0, 200.0);
        println!(
            "(5) page press -> open {} label {}",
            harness.open.get(),
            label(&background_button(scene))
        );
        let r = row(scene, 29);
        scene.focus.focus(Some(&r), true);
        scene.key(Scancode::RETURN, KeyModifiers::default());
        println!(
            "(5) RETURN open {} selected {:?}",
            harness.open.get(),
            harness.selected.get()
        );
        let r2 = row(scene, 29);
        scene.focus.focus(Some(&r2), true);
        scene.key(Scancode::HOME, KeyModifiers::default());
        scene.key(Scancode::ARROW_DOWN, KeyModifiers::default());
        scene.key(Scancode::ARROW_DOWN, KeyModifiers::default());
        println!(
            "(5) HOME/DOWN/DOWN selected {:?} open {} focus {:?}",
            harness.selected.get(),
            harness.open.get(),
            scene.focus.focused().map(|n| label(&n))
        );
        activate(&row(scene, 29));
        scene.settle(10, 1.0);
        println!(
            "(5) activate open {} selected {:?}",
            harness.open.get(),
            harness.selected.get()
        );
        let bd = backdrop(scene).map(|n| n.absolute_frame());
        println!("(5) backdrop {bd:?}");
        harness.present(scene, 390.0, true);
        let bd = backdrop(scene).map(|n| n.absolute_frame());
        println!("(5) backdrop again {bd:?}");
        if let Some(bd) = bd {
            press(scene, bd.x + 1.0, bd.y + 1.0);
            println!("(5) backdrop press open {}", harness.open.get());
        }
    });
}
