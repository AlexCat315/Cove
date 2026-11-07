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
//! Rows retain identity between docked and rail layouts. Content state must
//! survive crossing the compact visibility boundary. Discovery uses the same
//! modal input predicate as event dispatch.

use crate::support;
use cove_composition::Scope;

use cove_composition::{AnyView, LayoutDebugExt, Scancode, View};
use cove_core::scene::{AccessibilityRole, ElementId, Node};
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_core::{Binding, Environment, InputCapabilities, Point, State};
use cove_platform::{InputEvent, KeyModifiers, MouseButton, MouseButtonEvent, MouseMotionEvent};
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::interaction::AccessibilityExt;
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt};
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::layout::ScrollView;
use cove_ui_compose::primitives::layout::sidebar::{
    AdaptiveSidebar, AdaptiveSidebarOptions, SidebarItem, SidebarLayoutPolicy, SidebarSection,
};
use cove_ui_compose::primitives::media::ui_icon_catalog::file::folder;
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::text::TextField;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::theme::styles::appearance_modifier::{Appearance, AppearanceViewExt};
use cove_ui_compose::theme::styles::circular_appearance::ThemeTransition;
use cove_ui_compose::theme::theme_for_node;
use cove_ui_compose::theme::{ReducedMotionViewExt, TextScaler, TextScalerViewExt};
use cove_ui_compose::{BoxView, TupleView};

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
    fn body(&self, _cx: &Scope) -> AnyView {
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
    fn body(&self, _cx: &Scope) -> AnyView {
        let mut options = AdaptiveSidebarOptions::default();
        options.layout.compact_maximum_width = self.compact.get();
        options.layout.expanded_minimum_width = self.expanded_minimum.get();
        options.layout.shows_rail_labels = self.rail_labels.get();
        options.allows_edge_swipe = self.edge.get();
        let selected = self.selected.clone();
        let sidebar = AdaptiveSidebar::try_new(
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
        .expect("unique sidebar IDs")
        .options(options)
        .header(Text::new("Workspace header").debug_name("adaptive-header"))
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
            AnyView::new(LayerRoot::new(AnyView::new(sidebar).flex_with(
                1.0,
                1.0,
                Some(0.0),
            )))
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

/// Every node except orphaned subtrees.
///
/// The Rust `Sidebar` body leaves its first `ScrollView` mounted as a
/// zero-height sibling after the first recomposition (see the module doc), so
/// lookups skip a zero-height `ScrollArea` that has a later `ScrollArea`
/// sibling.
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

fn live_first(scene: &InteractiveScene, predicate: impl Fn(&Node) -> bool) -> Option<Node> {
    live_nodes(scene).into_iter().find(|node| predicate(node))
}

fn live_all(scene: &InteractiveScene, predicate: impl Fn(&Node) -> bool) -> Vec<Node> {
    live_nodes(scene)
        .into_iter()
        .filter(|node| predicate(node))
        .collect()
}

fn live_named(scene: &InteractiveScene, name: &str) -> Option<Node> {
    live_first(scene, |node| {
        support::debug_name(node).as_deref() == Some(name)
    })
}

fn row(scene: &InteractiveScene, index: i32) -> Node {
    let label = format!("Destination {index}");
    live_first(scene, |node| {
        node.attachment::<bool>("__button_host") == Some(true)
            && node
                .accessibility()
                .is_some_and(|semantics| semantics.label == label)
    })
    .unwrap_or_else(|| panic!("row {index} is mounted"))
}

fn navigation(scene: &InteractiveScene) -> Node {
    live_first(scene, |node| {
        node.accessibility().is_some_and(|semantics| {
            semantics.label == "Workspace" && semantics.role != AccessibilityRole::StaticText
        })
    })
    .expect("navigation")
}

fn toggle(scene: &InteractiveScene, label: &str) -> Option<Node> {
    live_first(scene, |node| {
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

fn label(node: &Node) -> String {
    node.accessibility()
        .map(|semantics| semantics.label)
        .unwrap_or_default()
}

fn draft_field(scene: &InteractiveScene) -> Node {
    let host = live_named(scene, "adaptive-draft").expect("the draft field keeps its debug name");
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
    live_first(scene, |node| {
        scene
            .registry
            .handlers(node)
            .pointer_route
            .is_some_and(|route| route.debug_name == "sidebar.backdrop")
    })
}

/// `UIDiagnostics.queryNodes(text:role:visibleOnly:inputEnabledOnly:)`.
fn query(scene: &InteractiveScene, text: &str, role: Option<AccessibilityRole>) -> Vec<Node> {
    live_all(scene, |node| {
        node.accessibility().is_some_and(|semantics| {
            (semantics.label.contains(text) || semantics.value.contains(text))
                && role.is_none_or(|role| semantics.role == role)
        }) && node.is_layout_visible()
            && node.accepts_subtree_input()
            && scene.focus.permits_input(node)
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
                        let case =
                            format!("scale {scale} input {input:?} dark {dark} width {width}");
                        let docked = if width < 600.0 {
                            0.0
                        } else if width < 1000.0 {
                            72.0
                        } else {
                            240.0
                        };
                        let main = live_named(scene, "adaptive-content").expect("content");
                        assert_eq!(
                            main.frame().width,
                            width - docked,
                            "content fits beside navigation ({case})"
                        );
                        assert!(
                            main.frame().height > 400.0 && main.absolute_frame().max_x() <= width,
                            "content stays in bounds ({case})"
                        );
                        if width < 1000.0 {
                            let trigger = toggle_button(scene, "Open navigation")
                                .expect("compact and rail opener");
                            let frame = trigger.absolute_frame();
                            let minimum =
                                if input.contains(InputCapabilities::TOUCH) || scale >= 2.0 {
                                    44.0
                                } else {
                                    32.0
                                };
                            assert!(
                                frame.width >= 44.0 && frame.height >= minimum,
                                "opener meets its input target ({case}): {frame:?}"
                            );
                            if width < 600.0 {
                                assert!(
                                    query(scene, "Destination 29", Some(AccessibilityRole::Button))
                                        .is_empty(),
                                    "closed compact navigation is unavailable ({case})"
                                );
                            }
                            activate(&trigger);
                            scene.settle(10, 1.0);
                            let panel = navigation(scene);
                            assert_eq!(role_of(&panel), AccessibilityRole::Dialog);
                            assert_eq!(
                                panel.frame().width,
                                (width - 56.0).min(320.0),
                                "drawer width ({case})"
                            );
                            assert_eq!(
                                panel.frame().height,
                                640.0,
                                "drawer fills the height ({case})"
                            );
                            assert_eq!(
                                live_named(scene, "adaptive-content")
                                    .expect("content")
                                    .frame()
                                    .width,
                                width - docked
                            );
                            assert!(
                                row(scene, 29).frame().height >= 44.0,
                                "drawer row target ({case})"
                            );
                            assert_eq!(
                                query(scene, "Destination 29", Some(AccessibilityRole::Button))
                                    .len(),
                                1
                            );
                            assert!(
                                query(scene, "Background", Some(AccessibilityRole::Button))
                                    .is_empty(),
                                "modal blocks background discovery ({case})"
                            );
                            assert!(scene.focus.has_modal_scope());
                        } else {
                            assert_eq!(role_of(&navigation(scene)), AccessibilityRole::Group);
                            assert!(
                                row(scene, 29).frame().height
                                    >= if input.contains(InputCapabilities::TOUCH) {
                                        44.0
                                    } else {
                                        30.0
                                    }
                            );
                        }
                        assert!(
                            !scene.graph.recomposer.has_pending(),
                            "shell settles ({case})"
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
        let scroll = live_named(scene, "adaptive-scroll").expect("scroll keeps its name");
        assert!(
            live_named(scene, "adaptive-header").is_some(),
            "the public header builder mounts content"
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
            if width == 720.0 && !open {
                assert_eq!(
                    id(&row_node),
                    id(&row(scene, 29)),
                    "docked and rail rows retain identity ({case})"
                );
            }
            assert_eq!(
                id(&field),
                id(&draft_field(scene)),
                "the field is retained ({case})"
            );
            assert_eq!(
                field.resolved_accessibility_value(),
                draft,
                "the draft survives ({case})"
            );
            assert_eq!(
                live_named(scene, "adaptive-scroll")
                    .expect("scroll")
                    .content_offset()
                    .y,
                180.0,
                "content retains its scroll offset ({case})"
            );
            assert_eq!(
                harness.selected.get(),
                Some(29),
                "selection is untouched ({case})"
            );
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
        let rail_row = row(scene, 29);
        assert_eq!(
            scene.focus.focused().map(|node| id(&node)),
            Some(id(&rail_row)),
            "focus follows the row into the rail"
        );
        assert_eq!(
            id(&rail_row),
            id(&row_node),
            "the row is retained in the rail"
        );
        assert!(
            !scene.focus.is_focus_visible(),
            "a pointer scene keeps focus quiet across the breakpoint"
        );
        harness.set_width(scene, 390.0);
        let opener = toggle_button(scene, "Open navigation").expect("compact opener");
        assert_eq!(
            scene.focus.focused().map(|node| node.id()),
            Some(opener.id()),
            "focus moves to the compact opener"
        );
        assert!(!scene.focus.is_focus_visible(), "focus stays quiet");

        harness.open.set(true);
        scene.settle(10, 1.0);
        assert!(harness.open.get(), "the binding presents the drawer");
        assert!(
            scene.focus.has_modal_scope(),
            "the compact drawer owns a modal scope"
        );
        let background = live_named(scene, "adaptive-background").expect("background");
        let frame = background.absolute_frame();
        press(
            scene,
            frame.x + frame.width / 2.0,
            frame.y + frame.height / 2.0,
        );
        assert_eq!(
            background.accessibility().expect("semantics").label,
            "Background 0",
            "the modal blocks background input"
        );
        harness.open.set(true);
        scene.settle(10, 1.0);
        scene.focus.focus(Some(&row(scene, 29)), true);
        scene.key(Scancode::HOME, KeyModifiers::default());
        scene.key(Scancode::ARROW_DOWN, KeyModifiers::default());
        scene.key(Scancode::ARROW_DOWN, KeyModifiers::default());
        assert_eq!(
            harness.selected.get(),
            Some(3),
            "the roving group skips the disabled destination"
        );
        assert!(harness.open.get(), "arrowing does not dismiss the drawer");
        scene.key(Scancode::ESCAPE, KeyModifiers::default());
        assert!(
            !harness.open.get() && !scene.focus.has_modal_scope(),
            "Escape dismisses the drawer"
        );
        assert_eq!(
            scene.focus.focused().map(|node| node.id()),
            Some(opener.id()),
            "Escape restores the opener"
        );
        harness.set_width(scene, 1280.0);
        assert_eq!(
            scene.focus.focused().map(|node| node.id()),
            Some(row(scene, 3).id()),
            "docking focuses the selected destination"
        );
        let selected = row(scene, 3);
        let frame = selected.absolute_frame();
        press_down(
            scene,
            frame.x + frame.width / 2.0,
            frame.y + frame.height / 2.0,
        );
        assert!(
            scene.capture.target().is_some(),
            "pressing a docked row acquires pointer capture"
        );
        harness.set_width(scene, 390.0);
        assert!(
            scene.capture.target().is_none(),
            "a breakpoint change releases pointer capture"
        );
    });
}

#[test]
fn opening_and_closing_preserve_input_focus_visibility() {
    InteractiveScene::run(390.0, 640.0, |scene| {
        let harness = Harness::new(false);
        scene.install(harness.clone());
        let opener = toggle_button(scene, "Open navigation").expect("compact opener");
        scene.click(&opener, 1);
        assert!(harness.open.get() && !scene.focus.is_focus_visible());
        let collapse = toggle_button(scene, "Collapse navigation").expect("collapse");
        assert_eq!(
            toggle(scene, "Collapse navigation")
                .expect("toggle semantics")
                .accessibility()
                .expect("semantics")
                .state
                .is_expanded,
            Some(true)
        );
        let scrim = backdrop(scene).expect("backdrop");
        assert_eq!(
            scrim.background_color(),
            Some(theme_for_node(&scrim).colors.overlay)
        );
        scene.click(&collapse, 1);
        assert!(!harness.open.get() && !scene.focus.is_focus_visible());
        assert_eq!(
            scene.focus.focused().map(|node| node.id()),
            Some(opener.id())
        );
        scene.focus.focus(Some(&opener), true);
        scene.key(Scancode::RETURN, KeyModifiers::default());
        assert!(
            harness.open.get() && scene.focus.is_focus_visible(),
            "Return key-down opens with visible focus"
        );
        scene
            .dispatcher()
            .dispatch(&InputEvent::KeyUp(cove_platform::KeyEvent {
                scancode: Scancode::RETURN,
                keycode: 0,
                modifiers: Default::default(),
                is_repeat: false,
            }));
        scene.settle(5, 1.0);
        assert!(
            harness.open.get(),
            "key-up does not activate a second control"
        );
        scene.key(Scancode::ESCAPE, KeyModifiers::default());
        assert!(!harness.open.get() && scene.focus.is_focus_visible());
        assert_eq!(
            scene.focus.focused().map(|node| node.id()),
            Some(opener.id())
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
            for semantic in [false, true] {
                let trigger =
                    toggle_button(scene, "Open navigation").expect("the rail mounts an opener");
                scene.focus.clear();
                if semantic {
                    activate(&trigger);
                    scene.settle(10, 1.0);
                } else {
                    scene.click(&trigger, 1);
                }
                assert!(
                    harness.open.get(),
                    "activation presents the drawer (semantic {semantic})"
                );
                assert!(
                    scene.focus.has_modal_scope(),
                    "the drawer owns a modal scope (semantic {semantic})"
                );
                scene.key(Scancode::ESCAPE, KeyModifiers::default());
                assert!(
                    !harness.open.get() && !scene.focus.has_modal_scope(),
                    "Escape dismisses the drawer (semantic {semantic})"
                );
                assert!(
                    scene.focus.focused().is_some_and(|node| node.id()
                        == toggle_button(scene, "Open navigation")
                            .expect("opener")
                            .id()),
                    "Escape restores the opener (semantic {semantic})"
                );
            }
        }
    });
}

#[test]
fn edge_swipe_opens_only_in_the_enabled_touch_lane() {
    InteractiveScene::run(1280.0, 640.0, |scene| {
        let harness = Harness::new(false);
        scene.install(harness.clone());
        harness.input.set(InputCapabilities::TOUCH);
        harness.set_width(scene, 390.0);
        let background = live_named(scene, "adaptive-background").expect("background");
        let y = background.absolute_frame().y + background.absolute_frame().height / 2.0;
        swipe(scene, 10.0, y, 100.0, 0.0);
        assert!(harness.open.get(), "a touch edge swipe opens the drawer");
        assert_eq!(
            background.accessibility().expect("semantics").label,
            "Background 0",
            "the edge swipe cancels the underlying tap"
        );
        harness.open.set(true);
        scene.settle(10, 1.0);
        swipe(scene, 200.0, y, -160.0, 0.0);
        assert!(!harness.open.get(), "a leftward swipe dismisses the drawer");
        harness.open.set(false);
        scene.settle(10, 1.0);
        swipe(scene, 50.0, y, 100.0, 0.0);
        assert!(
            !harness.open.get(),
            "a swipe outside the leading lane opens nothing"
        );
        swipe(scene, 10.0, y, 10.0, 90.0);
        assert!(!harness.open.get(), "a mostly vertical swipe opens nothing");
        harness
            .input
            .set(InputCapabilities::POINTER | InputCapabilities::KEYBOARD);
        scene.settle(10, 1.0);
        swipe(scene, 10.0, y, 100.0, 0.0);
        assert!(
            !harness.open.get(),
            "a pointer surface never opens the drawer"
        );
    });
}

#[test]
fn local_session_and_policy_validation() {
    InteractiveScene::run(1280.0, 640.0, |scene| {
        let harness = Harness::new(true);
        scene.install(harness.clone());
        harness.set_width(scene, 390.0);
        let trigger = toggle_button(scene, "Open navigation").expect("compact local opener");
        activate(&trigger);
        scene.settle(10, 1.0);
        assert!(
            scene.focus.has_modal_scope(),
            "the local presentation opens without an external binding"
        );
        let row_node = row(scene, 29);
        scene.focus.focus(Some(&row_node), true);
        harness.set_width(scene, 1280.0);
        assert!(
            !scene.focus.has_modal_scope(),
            "the local session docks without a modal scope"
        );
        assert_eq!(
            scene.focus.focused().map(|node| label(&node)),
            Some("Destination 29".to_owned()),
            "the selected row keeps focus when docked"
        );
        harness.set_width(scene, 390.0);
        assert!(
            scene.focus.has_modal_scope(),
            "the local session reopens at compact width"
        );
        harness.compact.set(800.0);
        harness.expanded_minimum.set(900.0);
        harness.set_width(scene, 720.0);
        assert_eq!(
            navigation(scene).frame().width,
            320.0,
            "the drawer follows the updated layout policy"
        );
        harness.rail_labels.set(false);
        harness.set_width(scene, 850.0);
        assert_eq!(
            role_of(&navigation(scene)),
            AccessibilityRole::Dialog,
            "the local presentation survives a parent recomposition"
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
fn default_dismissal_policies_close_and_release_the_modal() {
    InteractiveScene::run(390.0, 640.0, |scene| {
        let harness = Harness::new(false);
        scene.install(harness.clone());
        for path in 0..4 {
            harness.present(scene, 390.0, true);
            assert!(scene.focus.has_modal_scope());
            match path {
                0 => press(scene, 370.0, 200.0),
                1 => scene.key(Scancode::ESCAPE, KeyModifiers::default()),
                2 => {
                    scene.focus.focus(Some(&row(scene, 29)), true);
                    scene.key(Scancode::RETURN, KeyModifiers::default());
                }
                _ => {
                    activate(&row(scene, 29));
                    scene.settle(10, 1.0);
                }
            }
            assert!(
                !harness.open.get() && !scene.focus.has_modal_scope(),
                "dismissal path {path} closes and releases the modal"
            );
        }
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
        // Recompose without advancing the clock so the entry transition is
        // still at its first frame.
        scene.settle(2, 0.0);
        let panel = navigation(scene);
        assert!(
            panel.absolute_frame().x < -0.5,
            "the drawer slides in from off the leading edge, first frame {:?}",
            panel.absolute_frame()
        );
        tick(1.0);
        scene.settle(10, 1.0);
        assert_eq!(
            panel.absolute_frame().x,
            0.0,
            "the panel reaches rest at the leading edge"
        );
        let row_before_exit = row(scene, 29);
        harness.open.set(false);
        scene.settle(1, 0.0);
        assert!(
            !panel.accepts_subtree_input(),
            "a leaving panel stops accepting input immediately"
        );
        tick(0.05);
        // Recompose without advancing the clock so the exit is still part-way.
        scene.settle(2, 0.0);
        let leaving_x = panel.absolute_frame().x;
        assert!(
            leaving_x < 0.0 && leaving_x > -320.0,
            "the departing drawer sits part-way off the leading edge, saw {leaving_x}"
        );
        harness.open.set(true);
        scene.settle(2, 0.0);
        assert!(
            (panel.absolute_frame().x - leaving_x).abs() < 0.5,
            "reopening mid-exit reverses in place, saw {:?} from {leaving_x}",
            panel.absolute_frame()
        );
        tick(1.0);
        scene.settle(10, 1.0);
        assert_eq!(
            id(&row(scene, 29)),
            id(&row_before_exit),
            "the reversal retains the row node"
        );
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
        let close =
            toggle_button(scene, "Collapse navigation").expect("the drawer mounts a close control");
        let frame = close.absolute_frame();
        press_down(
            scene,
            frame.x + frame.width / 2.0,
            frame.y + frame.height / 2.0,
        );
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
            0,
            "the modal blocks background discovery"
        );
        for (text, role) in [
            ("Workspace", AccessibilityRole::Dialog),
            ("Destination 29", AccessibilityRole::Button),
            ("Background", AccessibilityRole::Button),
        ] {
            let leaked = query(scene, text, Some(role)).into_iter().any(|node| {
                node.accessibility().is_some_and(|semantics| {
                    semantics.label.contains("Local draft")
                        || semantics.value.contains("Local draft")
                })
            });
            assert!(
                !leaked,
                "the `{text}` query never returns the page draft payload"
            );
        }
        harness.present(scene, 390.0, false);
        assert_eq!(
            query(scene, "Destination 29", Some(AccessibilityRole::Button)).len(),
            0,
            "dismissed compact navigation is unavailable"
        );
        assert_eq!(
            query(scene, "Background", Some(AccessibilityRole::Button)).len(),
            1,
            "the page is always discoverable once nothing is modal"
        );
    });
}
