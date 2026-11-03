//! Translation of `Tests/CoveUIComposeTests/Adaptive/SidebarHierarchyTests.swift`.
//!
//! Behaviour checklist:
//! - clicking a branch discloses it without navigating; activation only fires for
//!   leaves
//! - programmatic selection, external collapse, data removal and disabled
//!   ancestors keep focus on a usable target
//! - collapsed descendants are out of the interaction graph; disabled ancestors
//!   disable every descendant
//! - local (uncontrolled) disclosure survives presentation changes
//! - horizontal keys follow RTL
//! - the adaptive shell retains rows, the document, its draft, its scroll offset,
//!   its disclosure and the row height floor across width, text scale, theme,
//!   modality and direction
//! - rail rows name the whole breadcrumb path
//! - AI discovery reports the current expansion only and omits collapsed
//!   descendants
//! - an item's actions open from the ellipsis and from the context menu
//! - every destination paints the same full-row surface
//!
//! Swift drives nine `@Test` cases; each maps to one `#[test]` below. Swift's
//! `adaptiveMatrix` is a single parameterised case; the four nested width/scale/
//! theme/direction/modality loops are kept inside `adaptive_matrix`, so the
//! `#[test]` count still matches one-to-one.
//!
//!
//! Harness notes:
//! - Rows are found by accessible label on the button host (rail rows carry
//!   breadcrumb paths) because Rust publishes no `sidebar-item-*` debug names;
//!   indentation is measured on the row's `StaticText`.
//! - The Rust `Sidebar` leaves its first `ScrollView` mounted as a zero-height
//!   orphan sibling after the first recomposition; `live_nodes` skips it.
//! - AI discovery in Swift ran through `UIDiagnostics.handleReadRequest` in the
//!   DevTools crate; `cove-ui-compose` does not depend on it, so `query()`
//!   applies the same predicate to the node tree.

use crate::support;
use cove_composition::Scope;

use std::collections::HashSet;

use cove_composition::layout::LayoutDirectionExt;
use cove_composition::{AnyView, DynamicField, LayoutDebugExt, Scancode, View};
use cove_core::scene::{
    AccessibilityRole, Align, Direction, ElementId, FlexDirection, Justify, Node, Wrap,
};
use cove_core::{Binding, Environment, InputCapabilities, Point, State};
use cove_platform::{InputEvent, KeyModifiers, MouseButton, MouseButtonEvent};
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::layout::{FlexExt, FrameDimension, LayoutFrameExt};
use cove_ui_compose::primitives::layout::ScrollView;
use cove_ui_compose::primitives::layout::sidebar::{
    AdaptiveSidebar, AdaptiveSidebarOptions, Sidebar, SidebarItem, SidebarPresentation,
    SidebarSection,
};
use cove_ui_compose::primitives::media::ui_icon_catalog::file::folder;
use cove_ui_compose::primitives::overlays::menu::{MenuEntry, MenuItem};
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::text::TextField;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::theme::styles::appearance_modifier::{Appearance, AppearanceViewExt};
use cove_ui_compose::theme::styles::circular_appearance::ThemeTransition;
use cove_ui_compose::theme::{ReducedMotionViewExt, TextScaler, TextScalerViewExt};
use cove_ui_compose::{BoxView, TupleView};

use support::{InteractiveScene, deepest_node};

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

fn binding<T: Clone + Send + 'static>(state: &State<T>) -> Binding<T> {
    state.binding()
}

fn set(ids: &[&str]) -> HashSet<String> {
    ids.iter().map(|id| (*id).to_owned()).collect()
}

#[derive(Clone)]
struct Document {
    draft: State<TextBuffer>,
}

impl Document {
    fn new() -> Self {
        Self {
            draft: State::new(TextBuffer::new("Retained document")),
        }
    }
}

impl View for Document {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(
                        AnyView::new(TextField::new("Draft", binding(&self.draft)))
                            .debug_name("hierarchy-draft"),
                    ),
                    AnyView::new(
                        AnyView::new(
                            AnyView::new(ScrollView::vertical(AnyView::new(
                                AnyView::new(Text::new("Document content")).frame_height(1200.0),
                            )))
                            .flex_with(1.0, 1.0, Some(0.0)),
                        )
                        .debug_name("hierarchy-scroll"),
                    ),
                ])),
            )
            .debug_name("hierarchy-document"),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("draft", &self.draft)]
    }
}

#[derive(Clone)]
struct Harness {
    adaptive: bool,
    selection: State<Option<String>>,
    expanded: State<HashSet<String>>,
    open: State<bool>,
    controlled: State<bool>,
    rail: State<bool>,
    disabled: State<bool>,
    removed: State<bool>,
    moved: State<bool>,
    activations: State<i32>,
    archived: State<i32>,
    width: State<f32>,
    scale: State<f32>,
    dark: State<bool>,
    rtl: State<bool>,
    input: State<InputCapabilities>,
}

impl Harness {
    fn new(adaptive: bool) -> Self {
        Self {
            adaptive,
            selection: State::new(Some("overview".to_owned())),
            expanded: State::new(HashSet::new()),
            open: State::new(false),
            controlled: State::new(true),
            rail: State::new(false),
            disabled: State::new(false),
            removed: State::new(false),
            moved: State::new(false),
            activations: State::new(0),
            archived: State::new(0),
            width: State::new(1280.0),
            scale: State::new(1.0),
            dark: State::new(false),
            rtl: State::new(false),
            input: State::new(InputCapabilities::POINTER | InputCapabilities::KEYBOARD),
        }
    }

    fn expand(&self, scene: &InteractiveScene, ids: &[&str]) {
        self.expanded.set(set(ids));
        scene.settle(10, 1.0);
    }

    fn set_width(&self, scene: &mut InteractiveScene, width: f32) {
        self.width.set(width);
        scene.width = width;
        scene.settle(10, 1.0);
    }
}

impl Harness {
    fn sections(&self) -> Vec<SidebarSection<String>> {
        let archive = {
            let archived = self.archived.clone();
            move || archived.set(archived.get() + 1)
        };
        let audit = SidebarItem::new("audit".to_owned(), "Audit trail")
            .badge("12")
            .context_menu(vec![MenuEntry::Item(MenuItem::new(7, "Archive", archive))]);
        let mut team_children = Vec::new();
        if !(self.removed.get() || self.moved.get()) {
            team_children.push(audit.clone());
        }
        team_children.push(SidebarItem::new("billing".to_owned(), "Billing").enabled(false));
        let team = SidebarItem::new("team".to_owned(), "Team").children(team_children);
        let locked = SidebarItem::new("locked".to_owned(), "Locked group")
            .enabled(false)
            .children(vec![SidebarItem::new("secret".to_owned(), "Secret")]);
        let mut projects_children = vec![
            SidebarItem::new("welcome".to_owned(), "Welcome"),
            team,
            locked,
        ];
        if self.moved.get() {
            projects_children.push(audit);
        }
        let projects = SidebarItem::new("projects".to_owned(), "Projects")
            .icon(folder())
            .enabled(!self.disabled.get())
            .children(projects_children);
        vec![
            SidebarSection::new(
                "workspace".to_owned(),
                vec![
                    SidebarItem::new("overview".to_owned(), "Overview"),
                    projects,
                    SidebarItem::new("settings".to_owned(), "Settings"),
                ],
            )
            .title("Workspace"),
        ]
    }
}

impl View for Harness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let width = self.width.get();
        let mut surface = Environment::default();
        surface.viewport.width = width;
        surface.viewport.height = 360.0;
        surface.input = self.input.get();
        let sections = self.sections();
        let sidebar = if self.adaptive {
            let mut options = AdaptiveSidebarOptions::default();
            options.layout.shows_rail_labels = false;
            let sidebar = AdaptiveSidebar::try_new("Workspace", binding(&self.selection), sections)
                .expect("unique sidebar IDs")
                .options(options)
                .presentation(binding(&self.open))
                .content(Document::new());
            let sidebar = if self.controlled.get() {
                sidebar.expanded(binding(&self.expanded))
            } else {
                sidebar
            };
            AnyView::new(sidebar)
        } else {
            let sidebar = Sidebar::try_new(binding(&self.selection), sections)
                .expect("unique sidebar IDs")
                .presentation(if self.rail.get() {
                    SidebarPresentation::Rail {
                        shows_labels: false,
                    }
                } else {
                    SidebarPresentation::Expanded
                })
                .on_activate({
                    let activations = self.activations.clone();
                    move |_| activations.set(activations.get() + 1)
                });
            let sidebar = if self.controlled.get() {
                sidebar.expanded(binding(&self.expanded))
            } else {
                sidebar
            };
            AnyView::new(AnyView::new(sidebar).frame(
                Some(FrameDimension::Points(if self.rail.get() {
                    56.0
                } else {
                    280.0
                })),
                Some(FrameDimension::Points(360.0)),
                None,
                None,
                None,
                None,
            ))
        };
        AnyView::new(
            AnyView::new(AnyView::new(LayerRoot::new(sidebar)).frame(
                Some(FrameDimension::Points(width)),
                Some(FrameDimension::Points(360.0)),
                None,
                None,
                None,
                None,
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
            .layout_direction(if self.rtl.get() {
                Direction::Rtl
            } else {
                Direction::Ltr
            })
            .reduced_motion(true),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("selection", &self.selection),
            DynamicField::state("expanded", &self.expanded),
            DynamicField::state("open", &self.open),
            DynamicField::state("controlled", &self.controlled),
            DynamicField::state("rail", &self.rail),
            DynamicField::state("disabled", &self.disabled),
            DynamicField::state("removed", &self.removed),
            DynamicField::state("moved", &self.moved),
            DynamicField::state("activations", &self.activations),
            DynamicField::state("archived", &self.archived),
            DynamicField::state("width", &self.width),
            DynamicField::state("scale", &self.scale),
            DynamicField::state("dark", &self.dark),
            DynamicField::state("rtl", &self.rtl),
            DynamicField::state("input", &self.input),
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

fn label(node: &Node) -> String {
    node.accessibility()
        .map(|semantics| semantics.label)
        .unwrap_or_default()
}

fn state_of(node: &Node) -> cove_core::scene::AccessibilityState {
    node.accessibility()
        .map(|semantics| semantics.state)
        .unwrap_or_default()
}

fn id(node: &Node) -> ElementId {
    node.id()
}

/// `try #require(named("sidebar-item-<id>"))` plus the button-host lookup.
fn row(scene: &InteractiveScene, names: &[&str]) -> Option<Node> {
    live_first(scene, |node| {
        node.attachment::<bool>("__button_host") == Some(true)
            && node
                .accessibility()
                .is_some_and(|semantics| names.iter().any(|name| semantics.label == *name))
    })
}

fn item(scene: &InteractiveScene, names: &[&str]) -> Node {
    row(scene, names).unwrap_or_else(|| panic!("{names:?} row is mounted"))
}

/// The trailing actions trigger for `title`. Rust publishes no
/// `sidebar-actions-<id>` debug name (it has no `Id` string form), so the
/// trigger is reached by the accessible name it publishes, the same way rows
/// are found here.
fn actions_trigger(scene: &InteractiveScene, title: &str) -> Option<Node> {
    let expected = format!("Actions for {title}");
    live_first(scene, |node| {
        node.accessibility()
            .is_some_and(|semantics| semantics.label == expected)
    })
}

fn title_node(scene: &InteractiveScene, names: &[&str], title: &str) -> Node {
    let host = item(scene, names);
    subtree(&host)
        .into_iter()
        .find(|node| {
            node.accessibility().is_some_and(|semantics| {
                semantics.role == AccessibilityRole::StaticText && semantics.label == title
            })
        })
        .unwrap_or_else(|| panic!("{names:?} row renders its title"))
}

fn field(scene: &InteractiveScene) -> Node {
    let host = live_named(scene, "hierarchy-draft").expect("the draft field keeps its debug name");
    subtree(&host)
        .into_iter()
        .find(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.role == AccessibilityRole::TextField)
        })
        .unwrap_or_else(|| deepest_node(&host))
}

fn has_expanded(state: cove_core::scene::AccessibilityState, value: bool) -> bool {
    state.is_expanded == Some(value)
}

/// `UIDiagnostics.query_nodes(role: "button", text:, visible_only:, input_enabled_only:)`.
fn query(scene: &InteractiveScene, text: &str, role: AccessibilityRole) -> Vec<Node> {
    live_all(scene, |node| {
        node.accessibility()
            .is_some_and(|semantics| semantics.role == role && semantics.label.contains(text))
            && node.is_layout_visible()
            && node.frame().width > 0.0
            && node.frame().height > 0.0
            && node.accepts_subtree_input()
            && scene.focus.permits_input(node)
    })
}

fn press(scene: &InteractiveScene, node: &Node, button: MouseButton) {
    let frame = node.absolute_frame();
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
    scene.settle(10, 1.0);
}

fn focused_label(scene: &InteractiveScene) -> Option<String> {
    scene.focus.focused().map(|node| label(&node))
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
    scene.settle(5, 1.0);
}

fn activate(node: &Node) {
    let action = node
        .accessibility_actions()
        .activate
        .unwrap_or_else(|| panic!("the row exposes an activate action"));
    action();
}

const PROJECTS: &str = "Projects";
const AUDIT: &str = "Audit trail";
const AUDIT_PATH: &str = "Projects / Team / Audit trail";
const WELCOME_PATH: &str = "Projects / Welcome";

#[test]
fn keyboard_and_pointer() {
    InteractiveScene::run(1280.0, 360.0, |scene| {
        let harness = Harness::new(false);
        scene.install(harness.clone());
        let projects = item(scene, &[PROJECTS]);
        assert!(
            row(scene, &[AUDIT]).is_some_and(|node| !node.accepts_subtree_input()),
            "a collapsed descendant stays mounted but outside the interaction graph"
        );
        assert_eq!(
            state_of(&projects).is_expanded,
            Some(false),
            "a collapsed branch reports isExpanded == false"
        );

        press(scene, &projects, MouseButton::Left);
        assert_eq!(
            harness.expanded.get(),
            set(&["projects"]),
            "clicking discloses"
        );
        assert_eq!(
            harness.selection.get().as_deref(),
            Some("overview"),
            "disclosure never navigates"
        );
        assert_eq!(
            harness.activations.get(),
            0,
            "a branch never fires onActivate"
        );

        scene.focus.focus(Some(&item(scene, &[PROJECTS])), true);
        scene.key(Scancode::ARROW_RIGHT, KeyModifiers::default());
        assert_eq!(
            (focused_label(scene), harness.selection.get()),
            (Some("Welcome".into()), Some("welcome".into())),
            "Right enters the expanded branch"
        );
        scene.key(Scancode::ARROW_DOWN, KeyModifiers::default());
        assert_eq!(focused_label(scene).as_deref(), Some("Team"));
        assert_eq!(
            harness.selection.get().as_deref(),
            Some("welcome"),
            "focusing a branch does not navigate"
        );
        scene.key(Scancode::ARROW_RIGHT, KeyModifiers::default());
        assert_eq!(harness.expanded.get(), set(&["projects", "team"]));
        scene.key(Scancode::ARROW_RIGHT, KeyModifiers::default());
        assert_eq!(harness.selection.get().as_deref(), Some("audit"));
        assert_eq!(focused_label(scene).as_deref(), Some(AUDIT));
        scene.key(Scancode::ARROW_LEFT, KeyModifiers::default());
        assert_eq!(focused_label(scene).as_deref(), Some("Team"));
        scene.key(Scancode::ARROW_LEFT, KeyModifiers::default());
        assert!(
            !harness.expanded.get().contains("team")
                && row(scene, &[AUDIT]).is_some_and(|node| !node.accepts_subtree_input()),
            "collapsing team keeps the audit row mounted but out of input"
        );
        assert_eq!(
            harness.selection.get().as_deref(),
            Some("audit"),
            "collapse preserves the selected page"
        );
        scene.settle(3, 0.05);
        assert!(
            scene
                .focus
                .focused()
                .expect("Team retains focus")
                .is_tab_stop(),
            "the focused branch remains the tab stop after later layout passes"
        );
        assert_eq!(
            live_all(scene, |node| node
                .attachment::<u64>("__roving.item")
                .is_some()
                && node.is_tab_stop()
                && node.accepts_subtree_input())
            .len(),
            1,
            "the navigation group has one tab stop"
        );
        scene.key(Scancode::ARROW_DOWN, KeyModifiers::default());
        assert_eq!(
            (harness.selection.get().as_deref(), harness.expanded.get()),
            (Some("settings"), set(&["projects"]))
        );
        scene.key(Scancode::HOME, KeyModifiers::default());
        assert_eq!(harness.selection.get().as_deref(), Some("overview"));
        scene.key(Scancode::END, KeyModifiers::default());
        assert_eq!(harness.selection.get().as_deref(), Some("settings"));
        scene.key(Scancode::RETURN, KeyModifiers::default());
        assert_eq!(
            harness.activations.get(),
            1,
            "Return activates the focused destination on key-down"
        );
        scene.focus.focus(Some(&item(scene, &[PROJECTS])), true);
        scene.key(Scancode::RETURN, KeyModifiers::default());
        assert!(!harness.expanded.get().contains("projects"));
        key_up(scene, Scancode::RETURN);
        assert!(
            !harness.expanded.get().contains("projects"),
            "key-up does not toggle twice"
        );
        assert_eq!(
            harness.activations.get(),
            1,
            "branches do not activate a page"
        );
        assert_eq!(harness.selection.get().as_deref(), Some("settings"));
    });
}

#[test]
fn controlled_disclosure() {
    InteractiveScene::run(1280.0, 360.0, |scene| {
        let harness = Harness::new(false);
        scene.install(harness.clone());
        harness.selection.set(Some("audit".to_owned()));
        scene.settle(10, 1.0);
        assert_eq!(
            harness.expanded.get(),
            set(&["projects", "team"]),
            "a programmatic selection reveals its ancestor chain once"
        );
        assert!(
            row(scene, &[AUDIT]).is_some_and(|node| node.accepts_subtree_input()),
            "the revealed row is mounted and takes input"
        );

        harness.expand(scene, &["projects", "team"]);
        let audit = item(scene, &[AUDIT]);
        let projects = item(scene, &[PROJECTS]);
        assert!(
            audit.accepts_subtree_input(),
            "the revealed row takes input"
        );
        assert!(
            !state_of(&projects).is_selected,
            "an expanded branch is not the selected destination"
        );
        scene.focus.focus(Some(&audit), false);
        let mut collapsed = harness.expanded.get();
        collapsed.remove("projects");
        harness.expanded.set(collapsed);
        scene.settle(10, 1.0);
        assert_eq!(
            harness.selection.get().as_deref(),
            Some("audit"),
            "collapsing the chain never changes the page"
        );
        assert!(
            !harness.expanded.get().contains("projects"),
            "collapse is caller owned"
        );
        assert!(
            row(scene, &[AUDIT]).is_some_and(|node| !node.accepts_subtree_input()),
            "the collapsed row stays mounted without input"
        );
        assert_eq!(
            focused_label(scene).as_deref(),
            Some(PROJECTS),
            "focus moves to the nearest visible ancestor"
        );
        assert!(
            !scene.focus.is_focus_visible(),
            "a programmatic collapse does not turn focus visible"
        );
        scene.settle(10, 1.0);
        assert!(
            !harness.expanded.get().contains("projects"),
            "collapse sticks"
        );

        harness.selection.set(Some("welcome".to_owned()));
        scene.settle(10, 1.0);
        assert_eq!(
            harness.expanded.get(),
            set(&["projects", "team"]),
            "a new selection re-reveals its whole ancestor chain"
        );

        harness.expand(scene, &["projects", "team"]);
        let title = title_node(scene, &[AUDIT], AUDIT);
        let projects_title = title_node(scene, &[PROJECTS], PROJECTS);
        assert_eq!(
            id(&item(scene, &[AUDIT])),
            id(&audit),
            "collapse and re-reveal retain the same row node"
        );
        assert!(
            title.absolute_frame().x > projects_title.absolute_frame().x,
            "a deeper row indents its title, saw {:?} vs {:?}",
            title.absolute_frame(),
            projects_title.absolute_frame()
        );
    });
}

#[test]
fn changing_data() {
    InteractiveScene::run(1280.0, 360.0, |scene| {
        let harness = Harness::new(false);
        scene.install(harness.clone());
        harness.selection.set(Some("audit".to_owned()));
        scene.settle(10, 1.0);
        assert_eq!(
            harness.expanded.get(),
            set(&["projects", "team"]),
            "the selected row's ancestors are revealed"
        );
        harness.expand(scene, &["projects", "team"]);
        let audit = item(scene, &[AUDIT]);
        scene.focus.focus(Some(&audit), true);
        harness.removed.set(true);
        scene.settle(10, 1.0);
        assert_eq!(
            harness.selection.get().as_deref(),
            Some("audit"),
            "removing the selected row never moves the selection"
        );
        assert!(
            row(scene, &[AUDIT]).is_none(),
            "the removed row leaves the tree"
        );
        assert_eq!(
            focused_label(scene).as_deref(),
            Some("Team"),
            "removing the focused row moves focus to the nearest visible enabled ancestor"
        );

        harness.disabled.set(true);
        scene.settle(10, 1.0);
        assert_eq!(
            focused_label(scene).as_deref(),
            Some("Overview"),
            "disabling the branch moves focus to the first usable row"
        );
        for name in [PROJECTS, "Welcome", "Team", "Billing", "Locked group"] {
            let row_node = item(scene, &[name]);
            assert!(
                !row_node.is_focusable() && !state_of(&row_node).is_enabled,
                "every descendant of a disabled branch shares its disabled state, saw {name}"
            );
        }
        let secret = item(scene, &["Secret"]);
        assert!(
            !secret.accepts_subtree_input() && !state_of(&secret).is_enabled,
            "secret stays mounted under the collapsed Locked group and reports disabled"
        );

        harness.disabled.set(false);
        harness.removed.set(false);
        harness.moved.set(true);
        harness.expanded.set(HashSet::new());
        scene.settle(10, 1.0);
        assert_eq!(
            harness.expanded.get(),
            set(&["projects"]),
            "the moved selection's new ancestors are revealed"
        );
        assert!(
            row(scene, &[AUDIT]).is_some_and(|node| node.accepts_subtree_input()),
            "the moved row is mounted and takes input"
        );
        harness.expand(scene, &["projects"]);
        let moved_audit = item(scene, &[AUDIT]);
        assert!(
            moved_audit.accepts_subtree_input(),
            "the moved row is reachable under its new parent"
        );

        scene
            .graph
            .install(cove_composition::EmptyView, Default::default());
        assert!(
            scene.focus.focused().is_none() && scene.capture.target().is_none(),
            "unmounting releases both the focus target and pointer capture"
        );
    });
}

#[test]
fn local_and_rtl() {
    InteractiveScene::run(1280.0, 360.0, |scene| {
        let harness = Harness::new(false);
        harness.controlled.set(false);
        harness.rtl.set(true);
        scene.install(harness.clone());
        let projects = item(scene, &[PROJECTS]);
        scene.focus.focus(Some(&projects), true);
        scene.key(Scancode::ARROW_LEFT, KeyModifiers::default());
        assert!(
            harness.expanded.get().is_empty(),
            "local disclosure never writes the caller's set"
        );
        assert_eq!(
            state_of(&item(scene, &[PROJECTS])).is_expanded,
            Some(true),
            "RTL Left discloses the branch locally"
        );
        scene.key(Scancode::ARROW_LEFT, KeyModifiers::default());
        assert_eq!(
            harness.selection.get().as_deref(),
            Some("welcome"),
            "RTL Left enters the expanded branch and selects welcome"
        );
        scene.key(Scancode::ARROW_RIGHT, KeyModifiers::default());
        assert_eq!(
            focused_label(scene).as_deref(),
            Some(PROJECTS),
            "Right under RTL leaves focus on the parent branch"
        );
        scene.key(Scancode::ARROW_RIGHT, KeyModifiers::LEFT_SHIFT);
        assert_eq!(
            state_of(&item(scene, &[PROJECTS])).is_expanded,
            Some(true),
            "modified arrows do not change disclosure"
        );

        press(scene, &item(scene, &[PROJECTS]), MouseButton::Left);
        assert!(
            row(scene, &["Welcome"]).is_some_and(|node| !node.accepts_subtree_input()),
            "the pointer collapses the disclosed branch"
        );
        press(scene, &item(scene, &[PROJECTS]), MouseButton::Left);
        assert!(
            row(scene, &["Welcome"]).is_some_and(|node| node.accepts_subtree_input())
                && harness.expanded.get().is_empty(),
            "an uncontrolled sidebar keeps local disclosure state: the press discloses the branch"
        );

        harness.rail.set(true);
        scene.settle(10, 1.0);
        let rail_projects = item(scene, &[PROJECTS]);
        assert_eq!(
            id(&rail_projects),
            id(&projects),
            "the row node survives the rail switch"
        );
        assert!(
            row(scene, &[WELCOME_PATH]).is_some(),
            "the rail row for the locally disclosed child is labelled `Projects / Welcome`"
        );
        assert_eq!(
            state_of(&rail_projects).is_expanded,
            Some(true),
            "the rail row reports the disclosed branch"
        );

        scene.focus.focus(Some(&rail_projects), true);
        scene.key(Scancode::ARROW_RIGHT, KeyModifiers::default());
        assert_eq!(
            state_of(&item(scene, &[PROJECTS])).is_expanded,
            Some(false),
            "RTL Right collapses the disclosed branch"
        );
    });
}

#[test]
fn adaptive_matrix() {
    InteractiveScene::run(1280.0, 360.0, |scene| {
        let harness = Harness::new(true);
        scene.install(harness.clone());
        harness.expand(scene, &["projects", "team"]);
        harness.selection.set(Some("audit".to_owned()));
        scene.settle(10, 1.0);
        let audit = item(scene, &[AUDIT, AUDIT_PATH]);
        let document =
            live_named(scene, "hierarchy-document").expect("the document keeps its debug name");
        let field = field(scene);
        let scroll =
            live_named(scene, "hierarchy-scroll").expect("the scroll view keeps its debug name");
        scene.focus.focus(Some(&field), true);
        scene
            .dispatcher()
            .dispatch(&InputEvent::TextInput(" edited".to_owned()));
        scene.settle(10, 1.0);
        scroll.set_content_offset(Point::new(0.0, 60.0));
        scene.settle(1, 1.0);
        let draft = field.resolved_accessibility_value();
        assert!(
            draft.contains("edited"),
            "the field typed into, saw {draft:?}"
        );

        let original_audit = id(&audit);
        let mut expected_docked_audit = original_audit;
        let mut previous_compact = false;
        for width in [1280.0_f32, 720.0, 390.0, 1280.0] {
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
                            let case = format!(
                                "width {width} scale {scale} dark {dark} rtl {rtl} modality {modality:?}"
                            );
                            harness.scale.set(scale);
                            harness.dark.set(dark);
                            harness.rtl.set(rtl);
                            harness.input.set(modality);
                            harness.open.set(false);
                            harness.set_width(scene, width);
                            if width == 390.0 {
                                assert!(
                                    row(scene, &[AUDIT, AUDIT_PATH])
                                        .is_none_or(|node| !node.accepts_subtree_input()),
                                    "collapsed compact navigation is outside input ({case})"
                                );
                                harness.open.set(true);
                                scene.settle(10, 1.0);
                            }
                            let audit = item(scene, &[AUDIT, AUDIT_PATH]);
                            if width != 390.0 {
                                if previous_compact {
                                    assert_eq!(
                                        id(&audit),
                                        original_audit,
                                        "navigation rows retain identity across compact presentation ({case})"
                                    );
                                    expected_docked_audit = id(&audit);
                                }
                                assert_eq!(
                                    id(&audit),
                                    expected_docked_audit,
                                    "rows are retained within the docked/rail presentation ({case})"
                                );
                            } else {
                                assert_eq!(
                                    id(&audit),
                                    original_audit,
                                    "compact navigation reuses the same row ({case})"
                                );
                            }
                            previous_compact = width == 390.0;
                            assert_eq!(
                                id(&live_named(scene, "hierarchy-document").expect("document")),
                                id(&document),
                                "the document is retained ({case})"
                            );
                            assert_eq!(
                                harness.expanded.get(),
                                set(&["projects", "team"]),
                                "disclosure survives ({case})"
                            );
                            assert_eq!(
                                harness.selection.get().as_deref(),
                                Some("audit"),
                                "selection survives ({case})"
                            );
                            assert_eq!(
                                field.resolved_accessibility_value(),
                                draft,
                                "the draft survives ({case})"
                            );
                            assert_eq!(
                                scene
                                    .named("hierarchy-scroll")
                                    .expect("scroll")
                                    .content_offset()
                                    .y,
                                60.0,
                                "the scroll offset survives ({case})"
                            );
                            if width < 1000.0 || modality.contains(InputCapabilities::TOUCH) {
                                assert!(
                                    audit.frame().height >= 44.0,
                                    "rows keep the 44pt floor for rail and touch, saw {:?} ({case})",
                                    audit.frame()
                                );
                            } else {
                                assert!(
                                    audit.frame().height >= 38.0,
                                    "docked rows keep the 38pt floor, saw {:?} ({case})",
                                    audit.frame()
                                );
                            }
                            if width == 390.0 {
                                assert!(
                                    audit.accepts_subtree_input(),
                                    "the open compact drawer accepts input ({case})"
                                );
                                harness.open.set(true);
                                scene.settle(10, 1.0);
                            }
                            assert!(
                                audit.accepts_subtree_input()
                                    && has_expanded(state_of(&item(scene, &[PROJECTS])), true,),
                                "the presented navigation is live and the branch reports expanded ({case})"
                            );
                            if width == 720.0 {
                                assert_eq!(
                                    label(&audit),
                                    AUDIT_PATH,
                                    "a rail row names the whole path ({case})"
                                );
                            }
                            if width == 390.0 {
                                harness.open.set(false);
                                scene.settle(10, 1.0);
                                scene.focus.focus(Some(&field), true);
                                // Focus recomposes the field's style host, the
                                // way Swift's @State invalidates its chrome.
                                scene.settle(10, 1.0);
                            }
                            assert_eq!(
                                scene.focus.focused().map(|node| id(&node)),
                                Some(id(&field)),
                                "the document field keeps focus across presentation changes ({case})"
                            );
                            assert!(
                                !scene.graph.recomposer.has_pending(),
                                "the shell settles without pending work ({case})"
                            );
                        }
                    }
                }
            }
        }
    });
}

#[test]
fn discovery() {
    InteractiveScene::run(1280.0, 360.0, |scene| {
        let harness = Harness::new(false);
        scene.install(harness.clone());
        assert_eq!(
            query(scene, AUDIT, AccessibilityRole::Button).len(),
            0,
            "a collapsed descendant is not discoverable"
        );
        activate(&item(scene, &[PROJECTS]));
        scene.settle(10, 1.0);
        activate(&item(scene, &["Team"]));
        scene.settle(10, 1.0);
        let found = query(scene, AUDIT, AccessibilityRole::Button);
        assert!(
            !found.is_empty() && harness.selection.get().as_deref() == Some("overview"),
            "disclosure alone makes the row discoverable without navigating, saw {}",
            found.len()
        );
        activate(&item(scene, &[AUDIT]));
        scene.settle(10, 1.0);
        assert_eq!(
            (
                harness.selection.get().as_deref(),
                harness.activations.get()
            ),
            (Some("audit"), 1),
            "activating the discovered row navigates once"
        );
        let mut collapsed = harness.expanded.get();
        collapsed.remove("projects");
        harness.expanded.set(collapsed);
        scene.settle(10, 1.0);
        assert_eq!(
            query(scene, AUDIT, AccessibilityRole::Button).len(),
            0,
            "collapsing takes the subtree out of discovery"
        );
        assert_eq!(
            state_of(&item(scene, &[PROJECTS])).is_expanded,
            Some(false),
            "the collapsed branch reports it"
        );
    });
}

#[test]
fn item_actions() {
    InteractiveScene::run(1280.0, 360.0, |scene| {
        let harness = Harness::new(false);
        scene.install(harness.clone());
        harness.expand(scene, &["projects", "team"]);
        harness.selection.set(Some("audit".to_owned()));
        scene.settle(10, 1.0);
        let actions = actions_trigger(scene, AUDIT)
            .expect("the row mounts its actions trigger");
        press(scene, &actions, MouseButton::Left);
        assert_eq!(
            harness.archived.get(),
            0,
            "opening the menu does not run an action"
        );
        let archive = scene
            .first(|node| label(node) == "Archive" && node.is_hit_testable())
            .expect("the authored context menu item is rendered");
        press(scene, &archive, MouseButton::Left);
        assert_eq!(
            harness.archived.get(),
            1,
            "the ellipsis reaches the authored action"
        );
        let audit = item(scene, &[AUDIT]);
        assert_eq!(
            audit.accessibility().map(|semantics| semantics.value),
            Some("12".to_owned()),
            "the authored badge still reaches the row's accessible value"
        );

        // The right-click path opens the same menu.
        press(scene, &audit, MouseButton::Right);
        let context_archive = scene
            .first(|node| label(node) == "Archive" && node.is_hit_testable())
            .expect("the row's context menu opens");
        press(scene, &context_archive, MouseButton::Left);
        assert_eq!(
            harness.archived.get(),
            2,
            "the right-click menu reaches the same action"
        );
    });
}

#[test]
fn compact_item_actions() {
    InteractiveScene::run(1280.0, 360.0, |scene| {
        let harness = Harness::new(true);
        scene.install(harness.clone());
        harness.expand(scene, &["projects", "team"]);
        harness.selection.set(Some("audit".to_owned()));
        harness.input.set(InputCapabilities::TOUCH);
        harness.open.set(true);
        harness.set_width(scene, 390.0);
        let actions = actions_trigger(scene, AUDIT)
            .expect("the compact drawer mounts the actions trigger");
        assert!(
            actions.frame().height >= 44.0,
            "the compact drawer's actions trigger keeps a 44pt target, saw {:?}",
            actions.frame()
        );
        let audit = item(scene, &[AUDIT, AUDIT_PATH]);
        assert!(
            audit.frame().height >= 44.0,
            "the row the trigger belongs to keeps the 44pt touch floor, saw {:?}",
            audit.frame()
        );
    });
}

#[test]
fn full_width_rows() {
    InteractiveScene::run(1280.0, 360.0, |scene| {
        let harness = Harness::new(false);
        scene.install(harness);
        let overview = item(scene, &["Overview"]);
        let settings = item(scene, &["Settings"]);
        let roving = |node: &Node| {
            let mut cursor = node.parent();
            while let Some(current) = cursor {
                if current.attachment::<u64>("__roving.item").is_some() {
                    return current;
                }
                cursor = current.parent();
            }
            panic!("a row sits inside its roving item")
        };
        assert_eq!(
            roving(&overview).frame().width,
            roving(&settings).frame().width,
            "every destination's row lane has the same width"
        );
        assert!(
            roving(&overview).frame().width >= 250.0,
            "the row lane spans the 280pt sidebar, saw {:?}",
            roving(&overview).frame()
        );
        assert!(
            overview.frame().width >= 250.0 && overview.frame().width == settings.frame().width,
            "the row button (the painted hover and selection surface) spans the full row, saw {:?} vs {:?}",
            overview.frame(),
            settings.frame()
        );
    });
}
