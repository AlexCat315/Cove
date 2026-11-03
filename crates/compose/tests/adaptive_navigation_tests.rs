//! Translation of `Tests/CoveUIComposeTests/AdaptiveNavigationTests.swift`.
//!
//! Behaviour checklist:
//! - an explicit `Top` placement keeps the strip above the panel
//! - the panel's own state survives, and activating its control relabels it
//! - compact placement puts the strip below the retained panel
//! - an explicit `Top` placement wins over compactness
//! - a touch surface gives the tab target at least 44pt of height
//! - no recomposition is left pending after settling
//!
//! Named differences from the Swift suite:
//! - Swift resolves `TabStripPlacement::Adaptive` from the surface
//!   (`SurfaceReader` plus `UIEnvironment.idiom` / `widthClass`). The Rust
//!   `TabView` instead exposes public `compact` and `mobile` flags that the
//!   caller sets and never reads the surface environment, so a width change
//!   alone cannot move the strip. `compact_placement_comes_from_caller_flags`
//!   pins that current behaviour.
//! - Swift marks each strip item `role = .tab`; the Rust strip item renders a
//!   plain `Button`, so assistive technology announces a button.
//!   `strip_items_are_buttons_not_tabs` records the gap.
//! - Swift sizes a scrolling strip to
//!   `max(40, touch ? minimumControlExtent : 0)`, so a touch strip is at least
//!   44pt tall. The Rust strip has a hard-coded 40pt scroller height and each
//!   item is 32pt, so no strip node reaches 44pt on a touch surface.
//!   `strip_height_ignores_the_touch_control_extent` records the gap.

use crate::support;
use cove_composition::Scope;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::{AnyView, LayoutDebugExt, View};
use cove_core::scene::{AccessibilityRole, AccessibilityTree, Node};
use cove_core::{Binding, Environment, InputCapabilities, State};
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::layout::{TabItem, TabOverflow, TabStripPlacement, TabView};
use cove_ui_compose::primitives::text::Text;

use support::GlobalTestLock;

/// Panel whose button counts its own activations, mirroring the Swift `Panel`
/// (`@State count` plus a body that reads it).
struct Panel {
    count: State<i32>,
    clicks: Rc<RefCell<i32>>,
}

impl View for Panel {
    fn body(&self, _cx: &Scope) -> AnyView {
        let count = self.count.clone();
        let clicks = self.clicks.clone();
        AnyView::new(
            Button::new(
                Text::new(format!("Count {}", self.count.get())),
                move || {
                    let next = count.get() + 1;
                    count.set(next);
                    *clicks.borrow_mut() = next;
                },
            )
            .debug_name("panel-counter"),
        )
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![cove_composition::DynamicField::state("count", &self.count)]
    }
}

fn nodes(root: &Node) -> Vec<Node> {
    let mut collected = vec![root.clone()];
    for child in root.children() {
        collected.extend(nodes(&child));
    }
    collected
}

/// The clickable strip item for `label`: the button that holds a static-text
/// descendant carrying that label.
fn strip_item(root: &Node, label: &str) -> Node {
    nodes(root)
        .into_iter()
        .find(|node| {
            node.accessibility_actions().activate.is_some()
                && nodes(node).iter().any(|child| {
                    child.accessibility().is_some_and(|semantics| {
                        semantics.role == AccessibilityRole::StaticText && semantics.label == label
                    })
                })
        })
        .unwrap_or_else(|| panic!("a strip item for {label:?} mounts"))
}

fn is_strip(node: &Node) -> bool {
    node.view_tag()
        .is_some_and(|tag| tag.rsplit("::").next() == Some("TabStrip"))
}

/// The strip container. Ordering is asserted against it rather than against an
/// item: `ScrollView` content keeps item-local frames, so an item's
/// `absolute_frame` is not offset by the scroller's origin.
fn strip_container(root: &Node, label: &str) -> Node {
    let target = strip_item(root, label);
    fn climb(node: &Node, target: &Node) -> Option<Node> {
        if is_strip(node) && nodes(node).iter().any(|child| child == target) {
            return Some(node.clone());
        }
        node.children()
            .iter()
            .find_map(|child| climb(child, target))
    }
    climb(root, &target).unwrap_or_else(|| panic!("the strip containing {label:?} mounts"))
}

fn panel_node(root: &Node) -> Node {
    nodes(root)
        .into_iter()
        .find(|node| support::debug_name(node).as_deref() == Some("panel-counter"))
        .unwrap_or_else(|| panic!("the panel mounts"))
}

fn harness(
    width: f32,
    placement: TabStripPlacement,
    compact: bool,
    count: State<i32>,
    clicks: Rc<RefCell<i32>>,
) -> AnyView {
    let mut tabs = TabView::try_new(
        Binding::constant(0_u8),
        vec![
            TabItem::new(0_u8, "Overview", Panel { count, clicks }),
            TabItem::new(1_u8, "Details", cove_composition::EmptyView),
        ],
    )
    .expect("unique tab IDs");
    tabs.placement = placement;
    tabs.overflow = TabOverflow::Scroll;
    tabs.compact = compact;

    let mut surface = Environment::default();
    surface.viewport.width = width;
    surface.input = InputCapabilities::TOUCH;

    AnyView::new(
        AnyView::new(AnyView::new(tabs).frame_width(width).frame_height(200.0))
            .surface_environment(surface),
    )
}

fn settle(graph: &Rc<cove_composition::ViewGraph>, width: f32) {
    for _ in 0..3 {
        graph.recomposer.commit_all();
        graph.compute_layout(width, 200.0);
    }
}

fn install(
    width: f32,
    placement: TabStripPlacement,
    compact: bool,
    count: State<i32>,
    clicks: Rc<RefCell<i32>>,
) -> Rc<cove_composition::ViewGraph> {
    let graph = support::graph();
    graph.install_view(
        harness(width, placement, compact, count, clicks),
        Default::default(),
    );
    settle(&graph, width);
    graph
}

#[test]
fn explicit_top_places_the_strip_above_the_panel() {
    GlobalTestLock::with_scheduler(|| {
        let count = State::new(0);
        let clicks = Rc::new(RefCell::new(0));
        let graph = install(
            800.0,
            TabStripPlacement::Top,
            false,
            count.clone(),
            clicks.clone(),
        );

        let root = graph.tree.borrow().root().expect("root");
        let strip = strip_container(&root, "Overview");
        let panel = panel_node(&root);
        assert!(
            strip.absolute_frame().max_y() <= panel.absolute_frame().y,
            "explicit Top keeps the strip above the panel: strip {:?} panel {:?}",
            strip.absolute_frame(),
            panel.absolute_frame()
        );

        strip_item(&root, "Overview")
            .accessibility_actions()
            .activate
            .expect("the tab exposes an activate action")();
        settle(&graph, 800.0);
        assert_eq!(
            count.get(),
            0,
            "activating the already-selected tab runs no panel action"
        );

        panel_node(&root)
            .accessibility_actions()
            .activate
            .expect("the panel button exposes an activate action")();
        settle(&graph, 800.0);
        assert_eq!(
            *clicks.borrow(),
            1,
            "activating the panel button increments it"
        );
        assert_eq!(count.get(), 1, "the panel state moved to 1");
        assert_eq!(
            AccessibilityTree::resolved_label(&panel_node(&root)),
            "Count 1",
            "the panel relabels after its state changes"
        );
        assert!(
            !graph.recomposer.has_pending(),
            "settling leaves no recomposition pending"
        );
    });
}

#[test]
fn compact_places_the_strip_below_the_retained_panel() {
    GlobalTestLock::with_scheduler(|| {
        let count = State::new(0);
        let clicks = Rc::new(RefCell::new(0));
        let graph = install(
            390.0,
            TabStripPlacement::Adaptive,
            true,
            count.clone(),
            clicks,
        );

        let root = graph.tree.borrow().root().expect("root");
        let panel_before = panel_node(&root);
        let strip = strip_container(&root, "Overview");
        assert!(
            panel_before.absolute_frame().max_y() <= strip.absolute_frame().y,
            "compact puts the strip below the panel: panel {:?} strip {:?}",
            panel_before.absolute_frame(),
            strip.absolute_frame()
        );

        panel_node(&root)
            .accessibility_actions()
            .activate
            .expect("activate")();
        settle(&graph, 390.0);
        assert_eq!(
            count.get(),
            1,
            "the panel state survives the compact layout"
        );
        assert_eq!(
            AccessibilityTree::resolved_label(&panel_node(&root)),
            "Count 1",
            "the retained panel reports the incremented label"
        );
        assert!(!graph.recomposer.has_pending());
    });
}

#[test]
fn explicit_top_wins_over_compactness() {
    GlobalTestLock::with_scheduler(|| {
        let graph = install(
            390.0,
            TabStripPlacement::Top,
            true,
            State::new(0),
            Rc::new(RefCell::new(0)),
        );

        let root = graph.tree.borrow().root().expect("root");
        let strip = strip_container(&root, "Overview");
        let panel = panel_node(&root);
        assert!(
            strip.absolute_frame().max_y() <= panel.absolute_frame().y,
            "an explicit Top placement overrides compactness: strip {:?} panel {:?}",
            strip.absolute_frame(),
            panel.absolute_frame()
        );
    });
}

#[test]
fn compact_placement_comes_from_caller_flags() {
    // Documents the current Rust contract: placement follows the caller's
    // `compact` flag, not the surface width.
    GlobalTestLock::with_scheduler(|| {
        let graph = install(
            390.0,
            TabStripPlacement::Adaptive,
            false,
            State::new(0),
            Rc::new(RefCell::new(0)),
        );
        let root = graph.tree.borrow().root().expect("root");
        let strip = strip_container(&root, "Overview");
        let panel = panel_node(&root);
        assert!(
            strip.absolute_frame().max_y() <= panel.absolute_frame().y,
            "a 390pt surface alone does not move the strip: the caller flag decides"
        );
    });
}

#[test]
fn strip_height_ignores_the_touch_control_extent() {
    // Gap: Swift reserves `max(40, minimumControlExtent)` for a scrolling strip
    // on a touch surface (44pt here). Rust frames the scroller at a fixed 40pt
    // and the item chrome at 32pt, so nothing reaches the 44pt touch extent.
    GlobalTestLock::with_scheduler(|| {
        let mut surface = Environment::default();
        surface.viewport.width = 390.0;
        surface.input = InputCapabilities::TOUCH;
        assert_eq!(
            surface.minimum_control_extent(),
            44.0,
            "a touch surface asks for a 44pt control extent"
        );

        let graph = install(
            390.0,
            TabStripPlacement::Top,
            false,
            State::new(0),
            Rc::new(RefCell::new(0)),
        );
        let root = graph.tree.borrow().root().expect("root");
        let item = strip_item(&root, "Overview");
        // Previously pinned as a gap: the scroller had a fixed 40pt frame and the
        // item chrome measured 32pt, so no strip node reached the 44pt touch
        // extent Swift uses. The minimum-size floor is now honoured, which raises
        // both to the touch extent, so this asserts the Swift behaviour.
        assert_eq!(
            item.absolute_frame().height,
            44.0,
            "the strip item reaches the touch control extent"
        );
        assert_eq!(
            strip_container(&root, "Overview").absolute_frame().height,
            44.0,
            "the scroller is raised to the touch control extent, not a fixed 40pt"
        );
        assert!(
            nodes(&strip_container(&root, "Overview"))
                .iter()
                .any(|node| node.absolute_frame().height >= 44.0),
            "at least one strip node carries the 44pt touch extent"
        );
    });
}

#[test]
fn strip_items_are_tabs() {
    // `TabView` publishes `AccessibilityRole::Tab` for each strip item, matching
    // `TabView.swift:221`.
    GlobalTestLock::with_scheduler(|| {
        let graph = install(
            800.0,
            TabStripPlacement::Top,
            false,
            State::new(0),
            Rc::new(RefCell::new(0)),
        );
        let root = graph.tree.borrow().root().expect("root");
        let item = strip_item(&root, "Overview");
        let semantics = item.accessibility().expect("the strip item is semantic");
        assert_eq!(
            semantics.role,
            AccessibilityRole::Tab,
            "a strip item is announced as a tab"
        );
        assert_eq!(
            semantics.label, "Overview",
            "the tab carries its own label"
        );
    });
}
