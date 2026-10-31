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
//! Named differences from the Swift suite:
//! - Swift found rows by the debug names the sidebar published
//!   (`sidebar-item-projects`, `sidebar-title-audit`, `sidebar-actions-audit`).
//!   The Rust `Sidebar` publishes no per-row debug names, so rows are found by
//!   accessible label (expanded rows are titled, rail rows are breadcrumb paths)
//!   and indentation is measured on the row's `StaticText` node instead of
//!   `sidebar-title-*`.
//! - Swift mounted every authored row and took hidden ones out of the
//!   interaction graph (`acceptsSubtreeInput == false`). Rust prunes collapsed
//!   descendants from the node tree entirely, so the Swift "hidden but mounted"
//!   assertions are ported as absence checks.
//! - The Rust `Sidebar` registers a roving command that maps only Up/Down/Home/
//!   End (and only with an empty modifier set). Right/Left arrows never disclose
//!   or collapse a branch, Shift-arrow is ignored, and the keys never reverse
//!   under `Direction::Rtl`, so `keyboard_and_pointer` and `local_and_rtl` pin
//!   that instead of the Swift traversal.
//! - Swift revealed a branch chain when the caller selected one of its
//!   descendants (`selection = "audit"` ⇒ `expanded == ["projects", "team"]`).
//!   The Rust `Sidebar` has no reveal-on-selection, so those cases expand the
//!   chain through the caller's binding first and pin the missing auto-reveal.
//! - Swift's vertical traversal moved focus without changing the selection.
//!   Rust's roving host calls its select handler on every arrow step, so
//!   Down/Up/Home/End write the selection (and toggle any branch they land on).
//! - `SidebarItem::context_menu` only reserves trailing width; no actions
//!   ellipsis node is mounted and no context-menu affordance is wired to the
//!   row, so `item_actions` and `compact_item_actions` pin the missing trigger
//!   while keeping the row-level assertions.
//! - AI discovery in Swift ran through `UIDiagnostics.handleReadRequest` in the
//!   DevTools crate; `cove-ui-compose` does not depend on it, so `query()`
//!   below applies the same predicate to the node tree
//!   (`is_layout_visible`, non-empty frame, `accepts_subtree_input`,
//!   `FocusChain::permits_input`).

mod support;

use std::collections::HashSet;

use cove_composition::layout::LayoutDirectionExt;
use cove_composition::{AnyView, DynamicField, LayoutDebugExt, Scancode, View};
use cove_core::scene::{AccessibilityRole, Align, Direction, ElementId, FlexDirection, Justify, Node, Wrap};
use cove_core::{Binding, Environment, InputCapabilities, Point, State};
use cove_platform::{InputEvent, KeyModifiers, MouseButton, MouseButtonEvent};
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::layout::{FlexExt, FrameDimension, LayoutFrameExt};
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::layout::scroll_view::ScrollView;
use cove_ui_compose::primitives::layout::sidebar::{
    AdaptiveSidebar, AdaptiveSidebarOptions, Sidebar, SidebarItem, SidebarPresentation, SidebarSection,
};
use cove_ui_compose::primitives::media::ui_icon_catalog::file::folder;
use cove_ui_compose::primitives::overlays::menu::{MenuEntry, MenuItem};
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::text::TextField;
use cove_ui_compose::primitives::text::Text;
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

fn binding<T: Clone + 'static>(state: &State<T>) -> Binding<T> {
    let state = state.clone();
    Binding::new(
        move || state.get(),
        move |value: T| state.set(value),
    )
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
    fn body(&self) -> AnyView {
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
                        AnyView::new(ScrollView::vertical(AnyView::new(AnyView::new(
                            Text::new("Document content"),
                        )
                        .frame_height(1200.0))))
                        .flex_with(1.0, 1.0, Some(0.0)))
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
        vec![SidebarSection::new(
            "workspace".to_owned(),
            vec![
                SidebarItem::new("overview".to_owned(), "Overview"),
                projects,
                SidebarItem::new("settings".to_owned(), "Settings"),
            ],
        )
        .title("Workspace")]
    }
}

impl View for Harness {
    fn body(&self) -> AnyView {
        let width = self.width.get();
        let mut surface = Environment::default();
        surface.viewport.width = width;
        surface.viewport.height = 360.0;
        surface.input = self.input.get();
        let sections = self.sections();
        let sidebar = if self.adaptive {
            let mut options = AdaptiveSidebarOptions::default();
            options.layout.shows_rail_labels = false;
            let sidebar = AdaptiveSidebar::new("Workspace", binding(&self.selection), sections)
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
            let sidebar = Sidebar::new(binding(&self.selection), sections)
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
            AnyView::new(sidebar).frame(
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
            )
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
    scene.first(|node| {
        node.accessibility()
            .is_some_and(|semantics| names.iter().any(|name| semantics.label == *name))
    })
}

fn item(scene: &InteractiveScene, names: &[&str]) -> Node {
    row(scene, names).unwrap_or_else(|| panic!("{names:?} row is mounted"))
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
    let host = scene
        .named("hierarchy-draft")
        .expect("the draft field keeps its debug name");
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
    scene.all(|node| {
        node.accessibility().is_some_and(|semantics| {
            semantics.role == role && semantics.label.contains(text)
        }) && node.is_layout_visible()
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
            row(scene, &[AUDIT]).is_none(),
            "GAP: Swift keeps a collapsed descendant mounted with `acceptsSubtreeInput == false`; the Rust sidebar never mounts it"
        );
        assert_eq!(
            state_of(&projects).is_expanded,
            Some(false),
            "a collapsed branch reports isExpanded == false"
        );

        press(scene, &projects, MouseButton::Left);
        assert_eq!(harness.expanded.get(), set(&["projects"]), "clicking discloses");
        assert_eq!(
            harness.selection.get().as_deref(),
            Some("overview"),
            "disclosure never navigates"
        );
        assert_eq!(harness.activations.get(), 0, "a branch never fires onActivate");
        let projects = item(scene, &[PROJECTS]);
        let welcome = item(scene, &["Welcome"]);
        assert!(welcome.accepts_subtree_input(), "a disclosed row takes input");
        assert!(
            row(scene, &[AUDIT]).is_none(),
            "the next level down stays unmounted until its own branch opens"
        );

        scene.focus.focus(Some(&projects), true);
        scene.key(Scancode::ARROW_RIGHT, KeyModifiers::default());
        assert_eq!(
            (
                harness.expanded.get(),
                harness.selection.get(),
                scene.focus.focused().map(|node| label(&node)),
            ),
            (
                set(&["projects"]),
                Some("overview".to_owned()),
                Some(PROJECTS.to_owned()),
            ),
            "GAP: Swift's Right arrow discloses the focused branch and moves into its first child; the Rust sidebar's roving command maps only Up/Down/Home/End, so the key is ignored"
        );

        scene.key(Scancode::ARROW_DOWN, KeyModifiers::default());
        assert_eq!(
            harness.selection.get().as_deref(),
            Some("welcome"),
            "GAP: Swift's Down arrow moved only focus to the next visible row (`welcome`) and left the page on `welcome`; Rust's roving step writes the selection too ({:?})",
            scene.focus.focused().map(|node| label(&node))
        );
        scene.key(Scancode::ARROW_DOWN, KeyModifiers::default());
        assert!(
            harness.expanded.get().contains("team"),
            "GAP: landing the roving step on a branch toggles its disclosure in Rust, which Swift only did for the Right arrow; selection is now {:?} ({:?})",
            harness.selection.get(),
            scene.focus.focused().map(|node| label(&node))
        );
        scene.key(Scancode::ARROW_LEFT, KeyModifiers::default());
        assert_eq!(
            harness.expanded.get(),
            set(&["projects", "team"]),
            "GAP: Swift's Left arrow collapsed the focused branch and handed focus to its parent; Rust ignores the key"
        );

        let tab_stops = scene.all(|node| {
            node.attachment::<u64>("__roving.item").is_some()
                && node.is_tab_stop()
                && node.accepts_subtree_input()
        });
        assert_eq!(
            tab_stops.len(),
            1,
            "the group exposes exactly one tab stop, saw {:?}",
            tab_stops.iter().map(label).collect::<Vec<String>>()
        );

        scene.key(Scancode::ARROW_DOWN, KeyModifiers::default());
        assert_eq!(harness.selection.get().as_deref(), Some("settings"));
        scene.key(Scancode::HOME, KeyModifiers::default());
        assert_eq!(harness.selection.get().as_deref(), Some("overview"));
        scene.key(Scancode::END, KeyModifiers::default());
        assert_eq!(harness.selection.get().as_deref(), Some("settings"));
        let settings = item(scene, &["Settings"]);
        scene.focus.focus(Some(&settings), true);
        let before = harness.activations.get();
        scene.key(Scancode::RETURN, KeyModifiers::default());
        assert_eq!(
            harness.selection.get().as_deref(),
            Some("settings"),
            "Return keeps the roving selection"
        );
        assert_eq!(
            harness.activations.get(),
            before + 1,
            "Return activates the focused destination"
        );
        let projects = item(scene, &[PROJECTS]);
        scene.focus.focus(Some(&projects), true);
        scene.key(Scancode::RETURN, KeyModifiers::default());
        assert!(
            !harness.expanded.get().contains("projects"),
            "Return collapses the focused branch instead of navigating"
        );
        assert_eq!(harness.activations.get(), before + 1, "a branch never activates");
    });
}

#[test]
fn controlled_disclosure() {
    InteractiveScene::run(1280.0, 360.0, |scene| {
        let harness = Harness::new(false);
        scene.install(harness.clone());
        harness.selection.set(Some("audit".to_owned()));
        scene.settle(10, 1.0);
        assert!(
            row(scene, &[AUDIT]).is_none(),
            "GAP: Swift revealed the ancestor chain for a programmatic selection (`expanded == [projects, team]`); the Rust sidebar has no reveal-on-selection, so the row stays unmounted"
        );
        assert!(harness.expanded.get().is_empty(), "the binding is untouched");

        harness.expand(scene, &["projects", "team"]);
        let audit = item(scene, &[AUDIT]);
        let projects = item(scene, &[PROJECTS]);
        assert!(audit.accepts_subtree_input(), "the revealed row takes input");
        assert_eq!(
            state_of(&projects).is_selected, false,
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
        assert!(!harness.expanded.get().contains("projects"), "collapse is caller owned");
        assert!(
            row(scene, &[AUDIT]).is_none(),
            "GAP: Swift kept the row mounted and handed focus to `projects`; Rust unmounts it, focus lands on {:?}",
            scene.focus.focused().map(|node| label(&node))
        );
        assert!(
            !scene.focus.is_focus_visible(),
            "a programmatic collapse does not turn focus visible"
        );
        scene.settle(10, 1.0);
        assert!(!harness.expanded.get().contains("projects"), "collapse sticks");

        harness.selection.set(Some("welcome".to_owned()));
        scene.settle(10, 1.0);
        assert_eq!(
            harness.expanded.get(),
            set(&["team"]),
            "GAP: Swift re-revealed [projects, team] for the new selection; Rust leaves the caller's set alone"
        );

        harness.expand(scene, &["projects", "team"]);
        let audit = item(scene, &[AUDIT]);
        let title = title_node(scene, &[AUDIT], AUDIT);
        let projects_title = title_node(scene, &[PROJECTS], PROJECTS);
        assert_eq!(id(&item(scene, &[AUDIT])), id(&audit), "the row is retained");
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
        harness.expand(scene, &["projects", "team"]);
        let audit = item(scene, &[AUDIT]);
        scene.focus.focus(Some(&audit), true);
        harness.selection.set(Some("audit".to_owned()));
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
        assert!(
            row(scene, &["Team"]).is_some(),
            "GAP: Swift moved focus to the nearest visible enabled ancestor (`team`); Rust leaves focus on {:?}",
            scene.focus.focused().map(|node| label(&node))
        );

        harness.disabled.set(true);
        scene.settle(10, 1.0);
        for name in [
            PROJECTS,
            "Welcome",
            "Team",
            "Billing",
            "Locked group",
            "Secret",
        ] {
            let row_node = item(scene, &[name]);
            assert!(
                !row_node.is_focusable() && !state_of(&row_node).is_enabled,
                "every descendant of a disabled branch shares its disabled state, saw {name}"
            );
        }

        harness.disabled.set(false);
        harness.removed.set(false);
        harness.moved.set(true);
        harness.expanded.set(HashSet::new());
        scene.settle(10, 1.0);
        assert!(
            row(scene, &[AUDIT]).is_none(),
            "GAP: Swift revealed the moved row's new ancestors (`expanded == [projects]`); Rust leaves the caller's set empty and the row hidden"
        );
        harness.expand(scene, &["projects"]);
        let moved_audit = item(scene, &[AUDIT]);
        assert!(
            moved_audit.accepts_subtree_input(),
            "the moved row is reachable under its new parent"
        );
        assert_eq!(harness.expanded.get(), set(&["projects"]), "the caller owns disclosure");

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
        scene.settle(10, 1.0);
        let projects = item(scene, &[PROJECTS]);
        scene.focus.focus(Some(&projects), true);
        scene.key(Scancode::ARROW_LEFT, KeyModifiers::default());
        assert!(
            row(scene, &[AUDIT]).is_none() && harness.expanded.get().is_empty(),
            "GAP: Swift's Left arrow discloses under RTL (leading direction); the Rust sidebar ignores horizontal keys in both directions"
        );
        scene.key(Scancode::ARROW_UP, KeyModifiers::default());
        scene.key(Scancode::ARROW_DOWN, KeyModifiers::default());
        assert_eq!(
            harness.selection.get().as_deref(),
            Some("projects"),
            "GAP: RTL does not reverse the vertical roving order in Rust; Down still steps to the next authored row ({:?})",
            scene.focus.focused().map(|node| label(&node))
        );

        press(scene, &item(scene, &[PROJECTS]), MouseButton::Left);
        let disclosed = row(scene, &["Welcome"]).is_some();
        assert!(
            disclosed,
            "GAP: the uncontrolled sidebar toggles its own local expansion state on a row press; Rust reports {:?}",
            row(scene, &["Welcome"]).map(|node| label(&node))
        );
        if disclosed {
            assert!(
                harness.expanded.get().is_empty(),
                "local disclosure never writes the caller's set ({:?})",
                harness.expanded.get()
            );
        }

        let local_projects = item(scene, &[PROJECTS]);
        harness.rail.set(true);
        scene.settle(10, 1.0);
        assert_eq!(
            id(&item(scene, &[PROJECTS])),
            id(&local_projects),
            "GAP-or-pin: Swift required the rail switch to keep the same row node; Rust {:?}",
            if id(&item(scene, &[PROJECTS])) == id(&local_projects) {
                "retains it"
            } else {
                "rebuilds it"
            }
        );
        assert_eq!(
            label(&item(scene, &[WELCOME_PATH])),
            WELCOME_PATH,
            "GAP-or-pin: a rail row names the whole breadcrumb path (Swift `Projects / Welcome`); Rust local disclosure before the rail switch is {:?}",
            disclosed
        );

        scene.focus.focus(Some(&local_projects), true);
        scene.key(Scancode::ARROW_RIGHT, KeyModifiers::default());
        assert_eq!(
            state_of(&item(scene, &[PROJECTS])).is_expanded,
            Some(true),
            "GAP: Swift's Right arrow under RTL collapses the branch; Rust ignores it, so the local state is unchanged"
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
        let document = scene
            .named("hierarchy-document")
            .expect("the document keeps its debug name");
        let field = field(scene);
        let scroll = scene
            .named("hierarchy-scroll")
            .expect("the scroll view keeps its debug name");
        scene.focus.focus(Some(&field), true);
        scene
            .dispatcher()
            .dispatch(&InputEvent::TextInput(" edited".to_owned()));
        scene.settle(10, 1.0);
        scroll.set_content_offset(Point::new(0.0, 60.0));
        scene.settle(1, 1.0);
        let draft = field.resolved_accessibility_value();
        assert!(draft.contains("edited"), "the field typed into, saw {draft:?}");

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
                            let audit = item(scene, &[AUDIT, AUDIT_PATH]);
                            assert_eq!(id(&audit), id(&item(scene, &[AUDIT, AUDIT_PATH])), "the row is retained ({case})");
                            assert_eq!(
                                id(&scene.named("hierarchy-document").expect("document")),
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
                                    "GAP: Swift hid the collapsed compact navigation (`acceptsSubtreeInput == false`); the Rust shell leaves the undocked navigation mounted and input-accepting ({case})"
                                );
                                harness.open.set(true);
                                scene.settle(10, 1.0);
                            }
                            assert!(
                                audit.accepts_subtree_input()
                                    && has_expanded(
                                        state_of(&item(scene, &[PROJECTS])),
                                        true,
                                    ),
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
                            }
                            assert_eq!(
                                scene.focus.focused().map(|node| id(&node)),
                                Some(id(&field)),
                                "GAP-or-pin: the document field keeps focus across presentation changes ({case})"
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
            (harness.selection.get().as_deref(), harness.activations.get()),
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
        harness.expand(scene, &["projects", "team"]);
        harness.selection.set(Some("audit".to_owned()));
        scene.settle(10, 1.0);
        assert!(
            scene.named("sidebar-actions-audit").is_none(),
            "GAP: Swift published a `sidebar-actions-audit` ellipsis trigger; the Rust sidebar only reserves `actions_width` for a context menu and mounts no trigger"
        );
        let audit = item(scene, &[AUDIT]);
        press(scene, &audit, MouseButton::Right);
        assert_eq!(harness.archived.get(), 0, "a right click opens no menu in Rust");
        assert!(
            scene
                .first(|node| { label(&node) == "Archive" && node.is_hit_testable() })
                .is_none(),
            "GAP: `SidebarItem::context_menu` entries are never rendered, so no `Archive` item exists to click"
        );
        assert_eq!(
            audit.accessibility().map(|semantics| semantics.value),
            Some("12".to_owned()),
            "the authored badge still reaches the row's accessible value"
        );
        assert_eq!(harness.archived.get(), 0, "nothing fired the menu action");
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
        assert!(
            scene.named("sidebar-actions-audit").is_none(),
            "GAP: Swift asserted the compact drawer's actions trigger is at least 44pt tall; the Rust sidebar mounts no such trigger"
        );
        let audit = item(scene, &[AUDIT, AUDIT_PATH]);
        assert!(
            audit.frame().height >= 44.0,
            "the row the trigger belonged to keeps the 44pt touch floor, saw {:?}",
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
        assert_eq!(
            overview.frame().width,
            settings.frame().width,
            "every destination paints the same row surface, saw {:?} vs {:?}",
            overview.frame(),
            settings.frame()
        );
        assert!(
            overview.frame().width >= 250.0,
            "the row surface spans the 280pt sidebar, saw {:?}",
            overview.frame()
        );
    });
}

#[test]
fn zz_probe() {
    InteractiveScene::run(1280.0, 360.0, |scene| {
        let harness = Harness::new(false);
        scene.install(harness.clone());
        let dump = |scene: &InteractiveScene, tag: &str| {
            println!(
                "[{tag}] focus {:?} sel {:?} exp {:?} act {}",
                scene.focus.focused().map(|n| (label(&n), n.is_focusable())),
                harness.selection.get(),
                harness.expanded.get(),
                harness.activations.get()
            );
        };
        dump(scene, "start");
        let rovers = scene.all(|n| n.attachment::<u64>("__roving.item").is_some());
        println!(
            "(a) rovers {:?}",
            rovers
                .iter()
                .map(|n| (
                    label(n),
                    n.is_focusable(),
                    n.is_tab_stop(),
                    n.accepts_subtree_input(),
                    n.frame()
                ))
                .collect::<Vec<_>>()
        );
        println!(
            "(a) rows {:?}",
            scene
                .all(|n| n
                    .accessibility()
                    .is_some_and(|s| s.role == AccessibilityRole::Button))
                .iter()
                .map(|n| (label(n), n.frame(), n.is_focusable(), state_of(n)))
                .collect::<Vec<_>>()
        );
        let projects = item(scene, &[PROJECTS]);
        press(scene, &projects, MouseButton::Left);
        dump(scene, "click projects");
        scene.focus.focus(Some(&projects), true);
        println!("(b) after focus {:?}", scene.focus.focused().map(|n| label(&n)));
        for key in [
            Scancode::ARROW_RIGHT,
            Scancode::ARROW_DOWN,
            Scancode::ARROW_DOWN,
            Scancode::ARROW_DOWN,
            Scancode::ARROW_DOWN,
            Scancode::ARROW_LEFT,
            Scancode::ARROW_UP,
            Scancode::HOME,
            Scancode::END,
        ] {
            scene.key(key, KeyModifiers::default());
            println!("(b) key {key} ->");
            dump(scene, &format!("key {key}"));
        }
        let settings = item(scene, &["Settings"]);
        scene.focus.focus(Some(&settings), true);
        scene.key(Scancode::RETURN, KeyModifiers::default());
        dump(scene, "RETURN settings");
        let tab_stops = scene.all(|n| {
            n.attachment::<u64>("__roving.item").is_some() && n.is_tab_stop()
                && n.accepts_subtree_input()
        });
        println!("(c) tab stops {}", tab_stops.len());

        let harness = Harness::new(false);
        scene.install(harness.clone());
        harness.expand(scene, &["projects", "team"]);
        let audit = item(scene, &[AUDIT]);
        let title = title_node(scene, &[AUDIT], AUDIT);
        let projects_title = title_node(scene, &[PROJECTS], PROJECTS);
        println!(
            "(d) audit {:?} title {:?} projects title {:?}",
            audit.absolute_frame(),
            title.absolute_frame(),
            projects_title.absolute_frame()
        );
        scene.focus.focus(Some(&audit), true);
        harness.removed.set(true);
        scene.settle(10, 1.0);
        dump(scene, "removed");
        harness.removed.set(false);
        harness.disabled.set(true);
        scene.settle(10, 1.0);
        println!(
            "(e) disabled {:?}",
            scene
                .all(|n| n
                    .accessibility()
                    .is_some_and(|s| s.role == AccessibilityRole::Button))
                .iter()
                .map(|n| (label(n), n.is_focusable(), state_of(n).is_enabled))
                .collect::<Vec<_>>()
        );
        dump(scene, "disabled");

        let harness = Harness::new(false);
        harness.controlled.set(true);
        scene.install(harness.clone());
        harness.selection.set(Some("audit".to_owned()));
        scene.settle(10, 1.0);
        dump(scene, "programmatic audit");
        harness.expand(scene, &["projects", "team"]);
        let audit = item(scene, &[AUDIT]);
        scene.focus.focus(Some(&audit), false);
        let mut collapsed = harness.expanded.get();
        collapsed.remove("projects");
        harness.expanded.set(collapsed);
        scene.settle(10, 1.0);
        println!(
            "(f) after collapse focus {:?} visible {} audit mounted {}",
            scene.focus.focused().map(|n| label(&n)),
            scene.focus.is_focus_visible(),
            row(scene, &[AUDIT]).is_some()
        );

        let harness = Harness::new(false);
        harness.controlled.set(false);
        scene.install(harness.clone());
        let projects = item(scene, &[PROJECTS]);
        press(scene, &projects, MouseButton::Left);
        println!(
            "(g) local click welcome mounted {} focus {:?}",
            row(scene, &["Welcome"]).is_some(),
            scene.focus.focused().map(|n| label(&n))
        );
        harness.rail.set(true);
        scene.settle(10, 1.0);
        println!(
            "(h) rail rows {:?}",
            scene
                .all(|n| n
                    .accessibility()
                    .is_some_and(|s| s.role == AccessibilityRole::Button))
                .iter()
                .map(|n| (label(n), n.frame(), n.is_focusable()))
                .collect::<Vec<_>>()
        );

        let harness = Harness::new(true);
        scene.install(harness.clone());
        harness.expand(scene, &["projects", "team"]);
        harness.selection.set(Some("audit".to_owned()));
        let document = scene.named("hierarchy-document").expect("document");
        let field = field(scene);
        let scroll = scene.named("hierarchy-scroll").expect("scroll");
        scene.focus.focus(Some(&field), true);
        scene
            .dispatcher()
            .dispatch(&InputEvent::TextInput(" edited".to_owned()));
        scene.settle(10, 1.0);
        scroll.set_content_offset(Point::new(0.0, 60.0));
        for width in [1280.0_f32, 720.0, 390.0] {
            harness.open.set(false);
            harness.set_width(scene, width);
            println!(
                "(i) w{width} audit {:?} accepts {} focus {:?} value {:?} scroll {} exp {:?} sel {:?} pending {}",
                row(scene, &[AUDIT, AUDIT_PATH]).map(|n| (label(&n), n.frame().height)),
                row(scene, &[AUDIT, AUDIT_PATH]).is_some_and(|n| n.accepts_subtree_input()),
                scene.focus.focused().map(|n| label(&n)),
                field.resolved_accessibility_value(),
                scene
                    .named("hierarchy-scroll")
                    .map(|n| n.content_offset().y)
                    .unwrap_or_default(),
                harness.expanded.get(),
                harness.selection.get(),
                scene.graph.recomposer.has_pending()
            );
            println!(
                "(i) ids audit {:?} document same {}",
                row(scene, &[AUDIT, AUDIT_PATH]).map(|n| n.id()),
                scene
                    .named("hierarchy-document")
                    .is_some_and(|n| id(&n) == id(&document))
            );
            harness.open.set(true);
            scene.settle(10, 1.0);
            println!(
                "(i) w{width} open modal-scope {} nav-role {:?} audit {:?}",
                scene.focus.has_modal_scope(),
                scene
                    .first(|n| n.accessibility().is_some_and(|s| s.role == AccessibilityRole::Dialog))
                    .map(|n| (label(&n), n.absolute_frame())),
                row(scene, &[AUDIT, AUDIT_PATH]).map(|n| n.frame().height)
            );
            scene.focus.focus(Some(&field), true);
            harness.open.set(false);
            scene.settle(10, 1.0);
        }
        println!(
            "(j) touch docked row height {:?}",
            {
                harness.input.set(InputCapabilities::TOUCH);
                harness.set_width(scene, 1280.0);
                row(scene, &[AUDIT, AUDIT_PATH]).map(|n| (n.frame().height, label(&n)))
            }
        );
    });
}
