//! Translation of `Tests/CoveUIComposeTests/ListTreeTests.swift`.
//!
//! Behaviour checklist:
//! - a list row tap writes the single-selection binding and exactly one row
//!   paints the selection fill
//! - a caller-constrained list keeps its 260pt viewport, clips it and leaves
//!   the following caption outside it
//! - the navigation selection pill is inset inside its 30pt row slot, matches
//!   `SelectionAppearance`, and the data-row role keeps the same fill with no
//!   rounding
//! - tree disclosure toggles the expanded binding and reveals child rows
//! - tree child taps update the selection; cmd-click toggles in a multi selection
//! - tree rows register hover handlers and hover repaints without requiring a
//!   structural recompose
//! - dragging a row onto another row reports an inside drop resolved by row
//!   token, so duplicate ids stay deterministic
//! - key selection disambiguates duplicate ids by path; filtered search keeps
//!   the original paths
//! - indent guides are subtle 1.5pt vertical strokes with ancestor
//!   continuations, and the hovered row deepens its own columns
//!
//! Named differences from the Swift suite:
//! - Rust selection is configured through `TreeSelection` bindings rather than
//!   Swift's `configure` closure; the harness is otherwise the same.
//! - The Rust `Tree` row closure receives `(element, selected, expanded,
//!   depth)`; the tests do not assert row colours, which Rust derives from the
//!   row style rather than the closure.
//! - `List` takes its IDs and a `.selection(..)` builder instead of Swift's
//!   `List(items, selection:)` initializer.
//! - A caller-owned `TreeSelection::expanded` binding is read when the tree is
//!   built and rebuilds the visible projection on the next pass, so
//!   `tree_selection` mounts the tree already expanded through that binding;
//!   `tree_expansion` covers disclosure-driven growth with local state.

use crate::support;
use cove_composition::Scope;

use std::cell::RefCell;
use std::collections::{BTreeSet, HashSet};
use std::rc::Rc;

use cove_composition::{AnyView, LayoutDebugExt, Scancode, TupleView, View};
use cove_core::scene::{
    AccessibilityRole, Align as SceneAlign, EventPhase, FlexDirection, Justify, Node, PointerPhase,
    Wrap,
};
use cove_core::{Binding, DrawList, Point, State};
use cove_platform::{KeyModifiers, MouseButton, MouseButtonEvent};
use cove_ui_compose::BoxView;
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::data::lists::RowState;
use cove_ui_compose::primitives::data::lists::reorder_model::ReorderRowShift;
use cove_ui_compose::primitives::data::lists::tree::options::{
    TreeDragOperations, TreeSearch, TreeSelection,
};
use cove_ui_compose::primitives::data::lists::tree::options::{TreeLayout, TreeOptions};
use cove_ui_compose::primitives::data::lists::tree::{
    Tree, TreeDropPosition, TreeGuideStyle, TreeNodeKey, TreeSearchFilterPolicy,
};
use cove_ui_compose::primitives::data::lists::{List, Selection};
use cove_ui_compose::primitives::layout::{ScrollAxis, ScrollView};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::Theme;
use cove_ui_compose::theme::styles::components::selection_appearance::{
    SelectionAppearance, SelectionRole,
};

use support::InteractiveScene;

#[derive(Clone, PartialEq)]
struct Item {
    id: &'static str,
    title: &'static str,
    children: Vec<Item>,
}

fn item(id: &'static str, title: &'static str, children: Vec<Item>) -> Item {
    Item {
        id,
        title,
        children,
    }
}

fn bind<T: Clone + Send + 'static>(state: &State<T>) -> Binding<T> {
    let get = state.clone();
    let set = state.clone();
    Binding::new(move || get.get(), move |value| set.set(value))
}

fn tree_rows(scene: &InteractiveScene) -> Vec<Node> {
    let mut rows: Vec<Node> = scene.all(|node| {
        node.accessibility()
            .is_some_and(|semantics| semantics.role == AccessibilityRole::TreeItem)
    });
    rows.sort_by(|a, b| {
        let (a, b) = (a.absolute_frame(), b.absolute_frame());
        a.y.partial_cmp(&b.y)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
    });
    rows
}

fn disclosures(scene: &InteractiveScene) -> Vec<Node> {
    let mut nodes = scene.all(|node| {
        node.accessibility().is_some_and(|semantics| {
            semantics.role == AccessibilityRole::Button
                && (semantics.label == "Expand" || semantics.label == "Collapse")
        })
    });
    nodes.sort_by(|a, b| {
        let (a, b) = (a.absolute_frame(), b.absolute_frame());
        a.y.partial_cmp(&b.y)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
    });
    nodes
}

/// Dispatches a click straight at the node's pointer handler, as the Swift
/// helper does, so modifiers reach the collection model.
fn click(scene: &InteractiveScene, node: &Node, modifiers: KeyModifiers) {
    let frame = node.absolute_frame();
    let event = MouseButtonEvent {
        button: MouseButton::Left,
        x: frame.x + frame.width / 2.0,
        y: frame.y + frame.height / 2.0,
        clicks: 1,
        modifiers,
        kind: Default::default(),
    };
    let handlers = scene.registry.handlers(node);
    let pointer = handlers.pointer.expect("row installs a pointer handler");
    let _ = pointer(&event, PointerPhase::Down, EventPhase::Target);
    let _ = pointer(&event, PointerPhase::Up, EventPhase::Target);
    scene.settle(1, 1.0);
}

/// Drags one row onto another through real pointer events: press on the source,
/// move past the dead zone to the target's middle, then release.
fn drag(scene: &InteractiveScene, source: &Node, target: &Node) {
    let from = centre(source);
    let to = centre(target);
    pointer_down(scene, from);
    pointer_walk(scene, from, to);
    pointer_up(scene, to);
}

fn centre(node: &Node) -> Point {
    let frame = node.absolute_frame();
    Point::new(frame.x + frame.width / 2.0, frame.y + frame.height / 2.0)
}

fn pointer_down(scene: &InteractiveScene, at: Point) {
    scene
        .dispatcher()
        .dispatch(&cove_platform::InputEvent::MouseButtonDown(
            MouseButtonEvent {
                button: MouseButton::Left,
                x: at.x,
                y: at.y,
                clicks: 1,
                modifiers: KeyModifiers::default(),
                kind: Default::default(),
            },
        ));
    scene.settle(2, 1.0);
}

/// Walks the open pointer to `to` in six steps, settling after each so the tree
/// re-projects between moves.
fn pointer_walk(scene: &InteractiveScene, from: Point, to: Point) {
    let dispatcher = scene.dispatcher();
    for step in 1..=6 {
        let t = step as f32 / 6.0;
        dispatcher.dispatch(&cove_platform::InputEvent::MouseMotion(
            cove_platform::MouseMotionEvent {
                x: from.x + (to.x - from.x) * t,
                y: from.y + (to.y - from.y) * t,
                delta_x: 0.0,
                delta_y: 0.0,
                kind: Default::default(),
            },
        ));
        scene.settle(2, 1.0);
    }
}

fn pointer_up(scene: &InteractiveScene, at: Point) {
    scene
        .dispatcher()
        .dispatch(&cove_platform::InputEvent::MouseButtonUp(
            MouseButtonEvent {
                button: MouseButton::Left,
                x: at.x,
                y: at.y,
                clicks: 1,
                modifiers: KeyModifiers::default(),
                kind: Default::default(),
            },
        ));
    scene.settle(2, 1.0);
}

fn paints(node: &Node) -> bool {
    node.background_color().is_some_and(|color| color.a > 0.0)
}

fn subtree_paints(node: &Node) -> bool {
    paints(node) || node.children().iter().any(subtree_paints)
}

fn list_rows(scene: &InteractiveScene) -> Vec<Node> {
    scene.all(|node| {
        node.accessibility()
            .is_some_and(|semantics| semantics.role == AccessibilityRole::ListItem)
    })
}

fn list_harness(selection: State<Option<i32>>, rows: Vec<i32>, height: f32) -> AnyView {
    AnyView::new(ListTestHarness {
        selection,
        rows,
        height,
    })
}

struct ListTestHarness {
    selection: State<Option<i32>>,
    rows: Vec<i32>,
    height: f32,
}

impl View for ListTestHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let _ = self.selection.get();
        AnyView::new(
            List::new(self.rows.clone(), |id: &i32, _: RowState| {
                let id = *id;
                AnyView::new(Text::new(format!("Document {id}")))
            })
            .selection(Selection::single(bind(&self.selection)))
            .frame_size(240.0, self.height),
        )
    }
}

fn sized_list_harness(selection: State<Option<i32>>, rows: Vec<i32>) -> AnyView {
    AnyView::new(
        AnyView::new(BoxView::new(
            FlexDirection::Column,
            SceneAlign::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            12.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(
                    List::new(rows, |id: &i32, _: RowState| {
                        let id = *id;

                        AnyView::new(Text::new(format!("Document {id}")))
                    })
                    .selection(Selection::single(bind(&selection)))
                    .frame_height(260.0),
                ),
                AnyView::new(AnyView::new(Text::new("caption")).debug_name("sized-list-caption")),
            ])),
        ))
        .surface_environment(Default::default()),
    )
}

fn tree_harness(
    roots: Vec<Item>,
    selection: TreeSelection<String>,
    search: TreeSearch<Item>,
    drag: TreeDragOperations<Item>,
) -> AnyView {
    tree_harness_with(roots, selection, search, drag, TreeLayout::default())
}

fn tree_harness_with(
    roots: Vec<Item>,
    selection: TreeSelection<String>,
    search: TreeSearch<Item>,
    drag: TreeDragOperations<Item>,
    layout: TreeLayout,
) -> AnyView {
    let tree = Tree::new(
        roots,
        |item: &Item| item.id.to_string(),
        |item: &Item| item.children.clone(),
        |item: &Item, state: RowState| {
            let depth = state.depth;

            AnyView::new(Text::new(format!("{depth}: {}", item.title)))
        },
    )
    .options(TreeOptions {
        selection,
        search,
        drag,
        layout,
        ..TreeOptions::default()
    });
    AnyView::new(tree.frame_size(280.0, 220.0))
}

fn default_tree(roots: Vec<Item>) -> AnyView {
    tree_harness(
        roots,
        TreeSelection::default(),
        TreeSearch::default(),
        TreeDragOperations::default(),
    )
}

#[test]
fn list_selection() {
    let selection = State::new(None);
    let observed = selection.clone();
    InteractiveScene::run(240.0, 160.0, |scene| {
        scene.install(list_harness(selection.clone(), vec![1, 2, 3], 160.0));
        let rows = list_rows(scene);
        assert_eq!(rows.len(), 3, "three rows mount, got {}", rows.len());

        click(scene, &rows[1], KeyModifiers::default());
        assert_eq!(
            observed.get(),
            Some(2),
            "tapping the second row writes its id, got {:?}",
            observed.get()
        );

        scene.settle(10, 1.0);
        let fills = scene
            .nodes()
            .into_iter()
            .filter(|node| node.background_color().is_some_and(|color| color.a > 0.1))
            .count();
        assert_eq!(
            fills, 1,
            "the newly selected row repaints exactly one selection fill, got {fills}"
        );
    });
}

#[test]
fn list_respects_caller_height() {
    let selection = State::new(Some(2));
    InteractiveScene::run(600.0, 800.0, |scene| {
        scene.install(sized_list_harness(selection.clone(), (1..=30).collect()));
        let caption = scene
            .named("sized-list-caption")
            .expect("the caption below the list mounts");
        let viewport = scene
            .first(|node| node.clips_to_bounds() && node.frame().width > 200.0)
            .expect("the list mounts a clipping viewport wider than 200pt");
        let frame = viewport.absolute_frame();
        assert!(
            (frame.height - 260.0).abs() < 0.5,
            "the viewport is the caller's 260pt frame, got {}",
            frame.height
        );
        assert!(
            (caption.frame().y - 272.0).abs() < 0.5,
            "the caption sits 12pt below the 260pt list, got {}",
            caption.frame().y
        );
    });
}

#[test]
fn selected_row_fits_its_slot() {
    let theme = Theme::default_dark();
    let appearance = SelectionAppearance::resolve(SelectionRole::Navigation, &theme);
    let selection = State::new(Some(2));
    InteractiveScene::run(600.0, 800.0, |scene| {
        scene.install(sized_list_harness(selection.clone(), (1..=6).collect()));

        let fills: Vec<Node> = scene
            .nodes()
            .into_iter()
            .filter(|node| node.background_color().is_some_and(|color| color.a > 0.1))
            .collect();
        assert_eq!(
            fills.len(),
            1,
            "exactly the preselected row paints a selection, got {}",
            fills.len()
        );
        let fill = &fills[0];
        let expected_height = 30.0 - 2.0 * appearance.vertical_inset;
        assert!(
            (fill.frame().height - expected_height).abs() < 0.5,
            "the highlight stays inside the 30pt row (expected {expected_height}), got {}",
            fill.frame().height
        );
        assert!(
            (fill.corner_radius() - appearance.corner_radius).abs() < 0.01,
            "the highlight uses the navigation radius {}, got {}",
            appearance.corner_radius,
            fill.corner_radius()
        );
        let viewport = scene
            .first(|node| node.clips_to_bounds() && node.frame().width > 200.0)
            .expect("the list mounts a clipping viewport");
        let viewport_width = viewport.absolute_frame().width;
        assert!(
            (viewport_width - 2.0 * appearance.horizontal_inset - fill.absolute_frame().width)
                .abs()
                < 0.5,
            "the selection fill is inset by {} per side; viewport {} fill {}",
            appearance.horizontal_inset,
            viewport_width,
            fill.absolute_frame().width
        );
        // Swift paints `.background(fill)` before `.padding(horizontal:)`, so the
        // band spans the whole row slot and only the content is inset.
        assert!(
            (fill.absolute_frame().x - viewport.absolute_frame().x).abs() < 0.5,
            "the fill spans the row slot from its leading edge, got offset {}",
            fill.absolute_frame().x - viewport.absolute_frame().x
        );

        assert_eq!(
            SelectionAppearance::resolve(SelectionRole::Navigation, &theme),
            appearance,
            "the navigation role must resolve deterministically"
        );
        let grid = SelectionAppearance::resolve(SelectionRole::DataRow, &theme);
        assert_eq!(grid.fill, appearance.fill, "a data row keeps the same fill");
        assert_eq!(grid.corner_radius, 0.0, "a data row keeps a square band");
    });
}

#[test]
fn tree_expansion() {
    let roots = vec![
        item(
            "scene",
            "Scene",
            vec![
                item("camera", "Camera", vec![]),
                item("light", "Light", vec![]),
            ],
        ),
        item("console", "Console", vec![]),
    ];
    InteractiveScene::run(280.0, 220.0, |scene| {
        scene.install(default_tree(roots));
        assert_eq!(
            tree_rows(scene).len(),
            2,
            "collapsed tree shows two root rows"
        );

        let disclosure = disclosures(scene)
            .into_iter()
            .min_by(|a, b| {
                a.frame()
                    .width
                    .partial_cmp(&b.frame().width)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .expect("a disclosure button mounts");
        let disclosure_y = disclosure.absolute_frame().y;
        click(scene, &disclosure, KeyModifiers::default());

        let rows = tree_rows(scene);
        assert_eq!(
            rows.len(),
            4,
            "expanding Scene reveals its two children, got {}",
            rows.len()
        );
        assert!(
            rows.iter().any(|row| row.absolute_frame().y > disclosure_y),
            "a child row must follow the Scene row"
        );
        let indented_text = scene
            .all(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.label.starts_with("1: "))
            })
            .into_iter()
            .next();
        let root_text = scene
            .all(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.label.starts_with("0: "))
            })
            .into_iter()
            .next();
        assert!(
            indented_text
                .as_ref()
                .zip(root_text.as_ref())
                .is_some_and(|(child, root)| {
                    child.absolute_frame().x >= root.absolute_frame().x + 16.0
                }),
            "a depth-1 leaf keeps its indentation and disclosure gutter: child {:?}, root {:?}",
            indented_text.map(|node| node.absolute_frame()),
            root_text.map(|node| node.absolute_frame())
        );
    });
}

#[test]
fn tree_selection() {
    let roots = vec![item(
        "scene",
        "Scene",
        vec![
            item("camera", "Camera", vec![]),
            item("light", "Light", vec![]),
        ],
    )];
    let selection = State::new(Some("camera".to_string()));
    let expanded = State::new(HashSet::from(["scene".to_string()]));
    let tree_selection = TreeSelection {
        ids: Selection::single(bind(&selection)),
        expanded: Some(bind(&expanded)),
        ..TreeSelection::default()
    };
    InteractiveScene::run(280.0, 220.0, |scene| {
        scene.install(tree_harness(
            roots,
            tree_selection,
            TreeSearch::default(),
            TreeDragOperations::default(),
        ));
        let rows = tree_rows(scene);
        assert_eq!(rows.len(), 3, "the caller-owned expansion mounts expanded");
        let fill = scene
            .nodes()
            .into_iter()
            .find(|node| node.background_color().is_some_and(|color| color.a > 0.1))
            .expect("the selected row paints its fill");
        assert!(
            fill.absolute_frame().width > 200.0,
            "the selection band spans the row slot, got {}",
            fill.absolute_frame().width
        );

        let sibling = rows
            .into_iter()
            .nth(2)
            .expect("the expanded sibling row mounts");
        click(scene, &sibling, KeyModifiers::default());
        assert_eq!(
            selection.get(),
            Some("light".to_string()),
            "clicking Light writes primary selection, got {:?}",
            selection.get()
        );
    });
}

#[test]
fn tree_multi_selection_updates_primary_selection() {
    let roots = vec![
        item("camera", "Camera", vec![]),
        item("light", "Light", vec![]),
        item("mesh", "Mesh", vec![]),
    ];
    let multiple = State::new(BTreeSet::new());
    let tree_selection = TreeSelection {
        ids: Selection::multiple(bind(&multiple)),
        ..TreeSelection::default()
    };
    InteractiveScene::run(280.0, 220.0, |scene| {
        scene.install(tree_harness(
            roots,
            tree_selection,
            TreeSearch::default(),
            TreeDragOperations::default(),
        ));
        let rows = tree_rows(scene);
        assert_eq!(rows.len(), 3, "three rows mount");
        click(scene, &rows[1], KeyModifiers::default());
        assert_eq!(
            multiple.get(),
            BTreeSet::from(["light".to_string()]),
            "plain click replaces the multi selection"
        );
    });
}

#[test]
fn tree_multi_selection_toggle_off_does_not_reselect_removed_item() {
    let roots = vec![
        item("camera", "Camera", vec![]),
        item("light", "Light", vec![]),
        item("mesh", "Mesh", vec![]),
    ];
    let multiple = State::new(BTreeSet::new());
    let tree_selection = TreeSelection {
        ids: Selection::multiple(bind(&multiple)),
        ..TreeSelection::default()
    };
    InteractiveScene::run(280.0, 220.0, |scene| {
        scene.install(tree_harness(
            roots,
            tree_selection,
            TreeSearch::default(),
            TreeDragOperations::default(),
        ));
        let rows = tree_rows(scene);
        assert_eq!(rows.len(), 3, "three rows mount");

        click(scene, &rows[0], KeyModifiers::default());
        click(scene, &rows[1], KeyModifiers::LEFT_GUI);
        assert_eq!(
            multiple.get(),
            BTreeSet::from(["camera".to_string(), "light".to_string()]),
            "cmd-click adds Light to the multi selection"
        );

        click(scene, &rows[1], KeyModifiers::LEFT_GUI);
        assert_eq!(
            multiple.get(),
            BTreeSet::from(["camera".to_string()]),
            "cmd-clicking a selected row removes it"
        );
    });
}

#[test]
fn tree_rows_register_hover_handlers() {
    let roots = vec![
        item("scene", "Scene", vec![item("camera", "Camera", vec![])]),
        item("console", "Console", vec![]),
    ];
    InteractiveScene::run(280.0, 220.0, |scene| {
        scene.install(default_tree(roots));
        let disclosure = disclosures(scene)
            .into_iter()
            .min_by(|a, b| {
                a.frame()
                    .width
                    .partial_cmp(&b.frame().width)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .expect("a disclosure button mounts");
        let disclosure_frame = disclosure.frame();
        let row_hosts: Vec<Node> = tree_rows(scene)
            .into_iter()
            .filter(|row| row.frame() != disclosure_frame)
            .collect();
        assert_eq!(row_hosts.len(), 2, "two row hosts mount");
        for row in &row_hosts {
            assert!(
                scene.registry.handlers(row).hover.is_some(),
                "every tree row host registers a hover handler"
            );
        }
    });
}

#[test]
fn default_tree_row_hover_does_not_recompose_tree() {
    let roots = vec![
        item("scene", "Scene", vec![item("camera", "Camera", vec![])]),
        item("console", "Console", vec![]),
    ];
    InteractiveScene::run(280.0, 220.0, |scene| {
        scene.install(default_tree(roots));
        scene.settle(1, 1.0);

        let disclosure = disclosures(scene)
            .into_iter()
            .min_by(|a, b| {
                a.frame()
                    .width
                    .partial_cmp(&b.frame().width)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .expect("a disclosure button mounts");
        let disclosure_frame = disclosure.frame();
        let row = tree_rows(scene)
            .into_iter()
            .find(|row| row.frame() != disclosure_frame)
            .expect("a row host mounts");
        let hover = scene
            .registry
            .handlers(&row)
            .hover
            .expect("the row registers a hover handler");

        assert!(!paints(&row), "a resting row paints no background");
        hover(cove_core::scene::HoverPhase::Enter);
        assert!(
            !scene.graph.recomposer.has_pending(),
            "hover repaints without scheduling a structural recompose"
        );
        assert!(
            paints(&row),
            "entering paints the hover fill on the row host, got {:?}",
            row.background_color()
        );
        scene.settle(1, 1.0);
        assert!(
            paints(&row),
            "the hover fill survives a recompose, got {:?}",
            row.background_color()
        );
        hover(cove_core::scene::HoverPhase::Leave);
        scene.settle(1, 1.0);
        assert!(!paints(&row), "leaving clears the hover background");
    });
}

fn drag_tree(
    roots: Vec<Item>,
    probe: Rc<RefCell<Option<(String, String, TreeDropPosition)>>>,
) -> AnyView {
    let operations = TreeDragOperations {
        can_drop: Some(Rc::new(|_source: &Item, _target: &Item, _position| true)),
        on_drop: Some(Rc::new(move |source: Item, target: Item, position| {
            *probe.borrow_mut() =
                Some((source.title.to_string(), target.title.to_string(), position));
        })),
        ..TreeDragOperations::default()
    };
    tree_harness(
        roots,
        TreeSelection::default(),
        TreeSearch::default(),
        operations,
    )
}

#[test]
fn tree_drag_dispatches_inside_drop() {
    let done = Rc::new(RefCell::new(None));
    let probe = done.clone();
    InteractiveScene::run(280.0, 160.0, |scene| {
        scene.install(drag_tree(
            vec![item("a", "A", vec![]), item("b", "B", vec![])],
            probe.clone(),
        ));
        let rows = tree_rows(scene);
        assert_eq!(rows.len(), 2, "two rows mount");
        assert!(
            scene.registry.handlers(&rows[1]).motion.is_some(),
            "an armed tree row registers a pointer-motion handler"
        );

        drag(scene, &rows[1], &rows[0]);
        assert_eq!(
            done.borrow().as_ref().map(|(source, target, position)| (
                source.as_str(),
                target.as_str(),
                *position
            )),
            Some(("B", "A", TreeDropPosition::Inside)),
            "dragging B onto A's middle reports an inside drop, got {:?}",
            *done.borrow()
        );
    });
}

#[test]
fn tree_drag_survives_a_surrounding_scroll_view() {
    let done = Rc::new(RefCell::new(None));
    let probe = done.clone();
    InteractiveScene::run(320.0, 220.0, |scene| {
        let tree = drag_tree(
            vec![item("a", "A", vec![]), item("b", "B", vec![])],
            probe.clone(),
        );
        scene.install(AnyView::new(
            ScrollView::new(ScrollAxis::Vertical, AnyView::new(tree).frame_height(420.0))
                .frame_size(300.0, 200.0),
        ));
        let rows = tree_rows(scene);
        assert_eq!(rows.len(), 2, "two rows mount");
        drag(scene, &rows[1], &rows[0]);
        assert_eq!(
            done.borrow().as_ref().map(|(source, target, position)| (
                source.as_str(),
                target.as_str(),
                *position
            )),
            Some(("B", "A", TreeDropPosition::Inside)),
            "the row drag survives the surrounding scroll view"
        );
    });
}

#[test]
fn tree_drag_with_duplicate_ids_is_deterministic() {
    let done = Rc::new(RefCell::new(None));
    let probe = done.clone();
    InteractiveScene::run(280.0, 160.0, |scene| {
        scene.install(drag_tree(
            vec![item("dup", "First", vec![]), item("dup", "Second", vec![])],
            probe.clone(),
        ));
        let rows = tree_rows(scene);
        assert_eq!(rows.len(), 2, "two rows mount");

        // Both roots share the id "dup"; the drop resolves by row token, so the
        // second row is the source and the first is the target.
        drag(scene, &rows[1], &rows[0]);
        assert_eq!(
            done.borrow().as_ref().map(|(source, target, position)| (
                source.as_str(),
                target.as_str(),
                *position
            )),
            Some(("Second", "First", TreeDropPosition::Inside)),
            "duplicate ids resolve by row token, got {:?}",
            *done.borrow()
        );
    });
}

fn keyed_tree(
    roots: Vec<Item>,
    multiple: State<BTreeSet<TreeNodeKey<String>>>,
    search: TreeSearch<Item>,
) -> AnyView {
    tree_harness(
        roots,
        TreeSelection {
            keys: Some(Selection::multiple(bind(&multiple))),
            ..TreeSelection::default()
        },
        search,
        TreeDragOperations::default(),
    )
}

#[test]
fn tree_node_key_selection_disambiguates_duplicate_ids() {
    let multiple = State::new(BTreeSet::new());
    InteractiveScene::run(280.0, 220.0, |scene| {
        scene.install(keyed_tree(
            vec![item("dup", "First", vec![]), item("dup", "Second", vec![])],
            multiple.clone(),
            TreeSearch::default(),
        ));
        let rows = tree_rows(scene);
        assert_eq!(rows.len(), 2, "two duplicate-id rows mount");

        click(scene, &rows[1], KeyModifiers::default());
        let second = TreeNodeKey::new("dup".to_string(), vec![1]);
        assert_eq!(
            multiple.get(),
            BTreeSet::from([second.clone()]),
            "the selection stores the path"
        );

        click(scene, &rows[0], KeyModifiers::LEFT_GUI);
        let first = TreeNodeKey::new("dup".to_string(), vec![0]);
        assert_eq!(
            multiple.get(),
            BTreeSet::from([first, second]),
            "cmd-click adds the path-[0] sibling"
        );
    });
}

#[test]
fn filtered_tree_node_key_paths_remain_stable() {
    let multiple = State::new(BTreeSet::new());
    let roots = vec![
        item("unmatched-root", "Other", vec![]),
        item(
            "group",
            "Group",
            vec![
                item("first", "First", vec![]),
                item("second", "Second", vec![]),
            ],
        ),
    ];
    let search = TreeSearch {
        query: "Second".to_string(),
        text: Some(Rc::new(|item: &Item| item.title.to_string())),
        policy: TreeSearchFilterPolicy::FilterAndAutoExpand,
        ..TreeSearch::default()
    };
    InteractiveScene::run(280.0, 160.0, |scene| {
        scene.install(keyed_tree(roots, multiple.clone(), search));
        let rows = tree_rows(scene);
        // Swift's three pointer nodes are these two rows plus Group's
        // disclosure; the non-matching sibling is dropped in both suites.
        assert_eq!(
            rows.len(),
            2,
            "the filtered projection keeps Group and Second only, got {}",
            rows.len()
        );
        let last = rows.last().expect("rows exist");
        click(scene, last, KeyModifiers::default());
        let expected = TreeNodeKey::new("second".to_string(), vec![1, 1]);
        assert_eq!(multiple.get(), BTreeSet::from([expected]));
    });
}

fn guide_nodes(scene: &InteractiveScene) -> Vec<Node> {
    scene
        .nodes()
        .into_iter()
        .filter(|node| node.attachment::<bool>("__tree_guide") == Some(true))
        .collect()
}

fn stroke_rects(node: &Node) -> Vec<(f32, f32)> {
    guide_strokes(node)
        .into_iter()
        .map(|(rect, _)| rect)
        .collect()
}

/// Every rect a guide node paints as `(width, height)` plus its packed colour,
/// so tests can compare the ink a hovered row deepens against a resting one.
fn guide_strokes(node: &Node) -> Vec<((f32, f32), u32)> {
    let Some(draw) = node.draw() else {
        return Vec::new();
    };
    let mut list = DrawList::default();
    draw(&mut list, Point::default());
    let vertices = list.vertices();
    vertices
        .chunks(4)
        .filter(|quad| quad.len() == 4)
        .map(|quad| {
            let min_x = quad.iter().map(|vertex| vertex.x).fold(f32::MAX, f32::min);
            let max_x = quad.iter().map(|vertex| vertex.x).fold(f32::MIN, f32::max);
            let min_y = quad.iter().map(|vertex| vertex.y).fold(f32::MAX, f32::min);
            let max_y = quad.iter().map(|vertex| vertex.y).fold(f32::MIN, f32::max);
            ((max_x - min_x, max_y - min_y), quad[0].color)
        })
        .collect()
}

fn expand_two_levels(scene: &InteractiveScene) {
    let first = disclosures(scene)
        .into_iter()
        .next()
        .expect("the root disclosure mounts");
    click(scene, &first, KeyModifiers::default());
    let second = disclosures(scene)
        .into_iter()
        .nth(1)
        .expect("the child disclosure mounts after expanding the root");
    click(scene, &second, KeyModifiers::default());
}

#[test]
fn tree_guide_lines_are_pixel_aligned() {
    let roots = vec![item(
        "scene",
        "Scene",
        vec![item("group", "Group", vec![item("leaf", "Leaf", vec![])])],
    )];
    InteractiveScene::run(280.0, 220.0, |scene| {
        scene.install(default_tree(roots));
        expand_two_levels(scene);

        let guides = guide_nodes(scene);
        let rects: Vec<(f32, f32)> = guides.iter().flat_map(stroke_rects).collect();
        assert!(
            !rects.is_empty(),
            "expanding two levels draws guide strokes"
        );
        for (width, height) in &rects {
            assert_eq!(
                *width, 1.5,
                "every guide stroke is the default 1.5pt wide, got {width}"
            );
            assert!(
                *height > 1.0,
                "every guide stroke is vertical, got height {height}"
            );
        }
        for node in &guides {
            for (_, color) in guide_strokes(node) {
                let alpha = ((color >> 24) & 0xFF) as f32 / 255.0;
                assert!(
                    alpha <= 0.4,
                    "guide ink stays subtle (alpha <= 0.4), got {alpha}"
                );
            }
        }
    });
}

#[test]
fn tree_guide_lines_include_ancestor_continuations() {
    let roots = vec![
        item(
            "scene",
            "Scene",
            vec![
                item("group", "Group", vec![item("leaf", "Leaf", vec![])]),
                item("sibling", "Sibling", vec![]),
            ],
        ),
        item("console", "Console", vec![]),
    ];
    InteractiveScene::run(280.0, 220.0, |scene| {
        scene.install(default_tree(roots));
        expand_two_levels(scene);

        let rects: Vec<(f32, f32)> = guide_nodes(scene).iter().flat_map(stroke_rects).collect();
        assert!(
            rects
                .iter()
                .any(|(width, height)| *width == 1.5 && *height == 30.0),
            "an ancestor continuation stem spans the full 30pt row, got {rects:?}"
        );
    });
}

/// Every axis-aligned rect a guide node paints, as `(width, height)`.
fn guide_rects(scene: &InteractiveScene) -> Vec<(f32, f32)> {
    guide_nodes(scene).iter().flat_map(stroke_rects).collect()
}

/// Each row's columns as `(guide node, strongest ink alpha it paints)`, ordered
/// by row position so two reads of the same tree line up entry by entry.
fn guide_inks(scene: &InteractiveScene) -> Vec<(cove_core::scene::ElementId, f32)> {
    let mut inks: Vec<(f32, cove_core::scene::ElementId, f32)> = guide_nodes(scene)
        .into_iter()
        .map(|node| {
            let alpha = guide_strokes(&node)
                .into_iter()
                .map(|(_, color)| ((color >> 24) & 0xFF) as f32 / 255.0)
                .fold(0.0f32, f32::max);
            (node.absolute_frame().y, node.id(), alpha)
        })
        .collect();
    inks.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.1.cmp(&b.1))
    });
    inks.into_iter().map(|(_, id, alpha)| (id, alpha)).collect()
}

/// The widest horizontal span any single guide node paints, which is how the
/// elbow's connector is told apart from a plain column.
fn guide_max_span(scene: &InteractiveScene) -> f32 {
    guide_nodes(scene)
        .iter()
        .filter_map(|node| {
            let draw = node.draw()?;
            let mut list = DrawList::default();
            draw(&mut list, Point::default());
            let vertices = list.vertices();
            let first = vertices.first()?;
            let mut min_x = first.x;
            let mut max_x = first.x;
            for vertex in vertices {
                min_x = min_x.min(vertex.x);
                max_x = max_x.max(vertex.x);
            }
            Some(max_x - min_x)
        })
        .fold(0.0f32, f32::max)
}

/// Where the earliest row label begins, inside the indent gutter.
fn content_x(scene: &InteractiveScene) -> f32 {
    scene
        .all(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.role == AccessibilityRole::StaticText)
        })
        .into_iter()
        .map(|node| node.absolute_frame().x)
        .fold(f32::MAX, f32::min)
}

/// The leftmost pixel any mounted guide node paints.
fn guide_min_x(scene: &InteractiveScene) -> f32 {
    guide_nodes(scene)
        .iter()
        .filter_map(|node| {
            let draw = node.draw()?;
            let mut list = DrawList::default();
            draw(&mut list, Point::default());
            Some(
                list.vertices()
                    .iter()
                    .map(|vertex| vertex.x)
                    .fold(f32::MAX, f32::min),
            )
        })
        .fold(f32::MAX, f32::min)
}

fn guide_tree(roots: Vec<Item>, layout: TreeLayout) -> AnyView {
    tree_harness_with(
        roots,
        TreeSelection::default(),
        TreeSearch::default(),
        TreeDragOperations::default(),
        layout,
    )
}

/// One branch two levels deep, expanded level by level in the guide tests.
fn guided_roots() -> Vec<Item> {
    vec![item(
        "scene",
        "Scene",
        vec![
            item("group", "Group", vec![item("leaf", "Leaf", vec![])]),
            item("sibling", "Sibling", vec![]),
        ],
    )]
}

#[test]
fn tree_guide_style_none_draws_no_columns() {
    InteractiveScene::run(280.0, 220.0, |scene| {
        scene.install(guide_tree(
            guided_roots(),
            TreeLayout::default().guide_style(TreeGuideStyle::None),
        ));
        expand_two_levels(scene);
        assert!(
            guide_nodes(scene).is_empty(),
            "the None style mounts no guide surface"
        );
    });
}

#[test]
fn tree_guide_style_straight_draws_unbroken_columns() {
    InteractiveScene::run(280.0, 220.0, |scene| {
        scene.install(guide_tree(
            guided_roots(),
            TreeLayout::default().guide_style(TreeGuideStyle::Straight),
        ));
        expand_two_levels(scene);
        let rects = guide_rects(scene);
        assert!(!rects.is_empty(), "straight guides draw columns");
        let tallest = rects
            .iter()
            .map(|(_, height)| *height)
            .fold(0.0f32, f32::max);
        assert_eq!(tallest, 30.0, "an unbroken column spans the whole row");
        assert!(
            rects.iter().all(|(width, _)| *width == 1.5),
            "the default guide width is 1.5pt, got {rects:?}"
        );
    });
}

#[test]
fn tree_guide_hover_deepens_only_the_hovered_row() {
    InteractiveScene::run(280.0, 220.0, |scene| {
        scene.install(guide_tree(
            guided_roots(),
            TreeLayout::default().guide_style(TreeGuideStyle::Elbow),
        ));
        expand_two_levels(scene);

        let rows = tree_rows(scene);
        let deepest = rows
            .iter()
            .filter_map(|row| row.attachment::<f32>("__tree_row_gutter").map(|g| (row, g)))
            .max_by(|(_, left), (_, right)| {
                left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(row, _)| row.clone())
            .expect("the deepest row mounts");
        let hover = scene
            .registry
            .handlers(&deepest)
            .hover
            .expect("the row registers a hover handler");

        let resting = guide_inks(scene);
        assert!(
            resting.len() >= 2,
            "two expanded levels mount one column set per indented row, got {resting:?}"
        );
        assert!(
            resting.iter().all(|(_, alpha)| *alpha < 0.5),
            "resting columns keep the subtle ink, got {resting:?}"
        );

        let owned: Vec<cove_core::scene::ElementId> = deepest
            .attachment::<Rc<RefCell<Vec<cove_core::scene::NodeWeak>>>>("__tree_row_guides")
            .expect("the indented row owns its columns")
            .borrow()
            .iter()
            .filter_map(Node::upgrade)
            .map(|guide| guide.id())
            .collect();
        scene.graph.tree.borrow_mut().flush();
        hover(cove_core::scene::HoverPhase::Enter);
        assert!(
            !scene.graph.recomposer.has_pending(),
            "deepening the columns repaints without recomposing the tree"
        );
        let dirty: Vec<cove_core::scene::ElementId> = guide_nodes(scene)
            .into_iter()
            .filter(|node| node.render_dirty())
            .map(|node| node.id())
            .collect();
        assert_eq!(
            dirty, owned,
            "the hover handler invalidates the hovered row's columns and no others"
        );
        let hovered = guide_inks(scene);
        assert_eq!(
            hovered.len(),
            resting.len(),
            "hovering adds no guide surface"
        );
        let deepened: Vec<usize> = (0..resting.len())
            .filter(|index| hovered[*index].1 > resting[*index].1 + 0.05)
            .collect();
        assert_eq!(
            deepened.len(),
            1,
            "only the hovered row's own columns darken, got {hovered:?} against {resting:?}"
        );
        assert_eq!(
            owned,
            vec![hovered[deepened[0]].0],
            "the darkened column belongs to the hovered row"
        );

        hover(cove_core::scene::HoverPhase::Leave);
        assert_eq!(
            guide_inks(scene),
            resting,
            "leaving returns every column to the resting ink"
        );
    });
}

#[test]
fn tree_guide_style_dashed_breaks_each_column() {
    InteractiveScene::run(280.0, 220.0, |scene| {
        scene.install(guide_tree(
            guided_roots(),
            TreeLayout::default().guide_style(TreeGuideStyle::Dashed),
        ));
        expand_two_levels(scene);
        let rects = guide_rects(scene);
        assert!(
            rects.len() >= 6,
            "a dashed column is several segments per row, got {}",
            rects.len()
        );
        assert!(
            rects.iter().all(|(_, height)| *height <= 3.0),
            "a dash is at most 3pt long, got {rects:?}"
        );
    });
}

#[test]
fn tree_guide_style_dotted_uses_shorter_segments_than_dashes() {
    InteractiveScene::run(280.0, 220.0, |scene| {
        scene.install(guide_tree(
            guided_roots(),
            TreeLayout::default().guide_style(TreeGuideStyle::Dotted),
        ));
        expand_two_levels(scene);
        let rects = guide_rects(scene);
        assert!(
            rects.iter().all(|(_, height)| *height <= 2.0),
            "a dot is at most 2pt long, got {rects:?}"
        );
    });
}

#[test]
fn tree_guide_style_elbow_adds_a_horizontal_connector() {
    InteractiveScene::run(280.0, 220.0, |scene| {
        scene.install(guide_tree(
            guided_roots(),
            TreeLayout::default().guide_style(TreeGuideStyle::Elbow),
        ));
        expand_two_levels(scene);
        let span = guide_max_span(scene);
        assert!(
            span > 4.0,
            "the elbow reaches from its column towards the row content, got {span}"
        );
    });
}

#[test]
fn tree_guide_width_thickens_every_column() {
    InteractiveScene::run(280.0, 220.0, |scene| {
        scene.install(guide_tree(
            guided_roots(),
            TreeLayout::default().guide_width(3.0),
        ));
        expand_two_levels(scene);
        let rects = guide_rects(scene);
        assert!(
            !rects.is_empty() && rects.iter().all(|(width, _)| *width == 3.0),
            "a 3pt guide width paints 3pt columns, got {rects:?}"
        );
    });
}

#[test]
fn tree_guide_position_moves_the_column_inside_its_step() {
    let mut leading = 0.0f32;
    InteractiveScene::run(280.0, 220.0, |scene| {
        scene.install(guide_tree(
            guided_roots(),
            TreeLayout::default().guide_position(0.0),
        ));
        expand_two_levels(scene);
        leading = guide_min_x(scene);
    });
    let mut trailing = 0.0f32;
    InteractiveScene::run(280.0, 220.0, |scene| {
        scene.install(guide_tree(
            guided_roots(),
            TreeLayout::default().guide_position(1.0),
        ));
        expand_two_levels(scene);
        trailing = guide_min_x(scene);
    });
    assert!(
        (trailing - leading - 16.0).abs() < 0.01,
        "position 1.0 sits one 16pt indent step right of position 0.0, got {leading} -> {trailing}"
    );
}

#[test]
fn tree_leading_padding_insets_the_labels_and_their_guides() {
    let mut without = (0.0f32, 0.0f32);
    InteractiveScene::run(280.0, 220.0, |scene| {
        scene.install(guide_tree(guided_roots(), TreeLayout::default()));
        expand_two_levels(scene);
        without = (content_x(scene), guide_min_x(scene));
    });
    let mut with = (0.0f32, 0.0f32);
    InteractiveScene::run(280.0, 220.0, |scene| {
        scene.install(guide_tree(
            guided_roots(),
            TreeLayout::default().leading_padding(12.0),
        ));
        expand_two_levels(scene);
        with = (content_x(scene), guide_min_x(scene));
    });
    assert!(
        (with.0 - without.0 - 12.0).abs() < 0.01,
        "a 12pt leading padding insets the row label by 12pt, got {:?}",
        (without.0, with.0)
    );
    assert!(
        (with.1 - without.1 - 12.0).abs() < 0.01,
        "and the first guide column with it, got {:?}",
        (without.1, with.1)
    );
}

#[test]
fn tree_drag_paints_rows_at_the_projected_slots() {
    let done = Rc::new(RefCell::new(None));
    let probe = done.clone();
    InteractiveScene::run(280.0, 220.0, |scene| {
        scene.install(tree_harness_with(
            vec![
                item("a", "A", vec![]),
                item("b", "B", vec![]),
                item("c", "C", vec![]),
                item("d", "D", vec![]),
            ],
            TreeSelection::default(),
            TreeSearch::default(),
            TreeDragOperations {
                can_drop: Some(Rc::new(|_: &Item, _: &Item, _| true)),
                on_drop: Some(Rc::new(move |source: Item, target: Item, position| {
                    *probe.borrow_mut() =
                        Some((source.title.to_string(), target.title.to_string(), position));
                })),
                ..TreeDragOperations::default()
            },
            TreeLayout::default(),
        ));
        let rows = tree_rows(scene);
        assert_eq!(rows.len(), 4, "four rows mount");
        let stride = rows[1].absolute_frame().y - rows[0].absolute_frame().y;

        let from = centre(&rows[3]);
        let to = centre(&rows[1]);
        pointer_down(scene, from);
        pointer_walk(scene, from, to);

        // D becomes B's last child, so it is painted one slot up and C steps
        // down into the slot D vacated.
        assert!(
            (ReorderRowShift::target_offset(&rows[3]) + stride).abs() < 0.01,
            "the dragged row paints one slot above its own, got {}",
            ReorderRowShift::target_offset(&rows[3])
        );
        assert!(
            (ReorderRowShift::target_offset(&rows[2]) - stride).abs() < 0.01,
            "the row the block jumped over shifts down one slot, got {}",
            ReorderRowShift::target_offset(&rows[2])
        );
        assert_eq!(
            ReorderRowShift::target_offset(&rows[0]),
            0.0,
            "rows outside the travelled range stay put"
        );
        assert_eq!(
            ReorderRowShift::target_offset(&rows[1]),
            0.0,
            "the target row stays put; the block moves to it"
        );
        assert!(
            rows[3].opacity() < 1.0,
            "the dragged row is held back at {}",
            rows[3].opacity()
        );

        pointer_up(scene, to);
        assert_eq!(
            done.borrow().as_ref().map(|(source, target, position)| (
                source.as_str(),
                target.as_str(),
                *position
            )),
            Some(("D", "B", TreeDropPosition::Inside)),
            "the drop is reported once the pointer releases"
        );
        for row in &rows {
            assert_eq!(
                ReorderRowShift::target_offset(row),
                0.0,
                "releasing without a model change clears the projection"
            );
        }
    });
}

#[test]
fn tree_drag_holds_the_projection_over_its_own_branch() {
    let done = Rc::new(RefCell::new(None));
    let probe = done.clone();
    InteractiveScene::run(280.0, 220.0, |scene| {
        scene.install(tree_harness_with(
            vec![
                item("a", "A", vec![item("a1", "A1", vec![])]),
                item("b", "B", vec![]),
            ],
            TreeSelection::default(),
            TreeSearch::default(),
            TreeDragOperations {
                can_drop: Some(Rc::new(|_: &Item, _: &Item, _| true)),
                on_drop: Some(Rc::new(move |source: Item, target: Item, position| {
                    *probe.borrow_mut() =
                        Some((source.title.to_string(), target.title.to_string(), position));
                })),
                ..TreeDragOperations::default()
            },
            TreeLayout::default(),
        ));
        let disclosure = disclosures(scene)
            .into_iter()
            .next()
            .expect("the branch discloses");
        click(scene, &disclosure, KeyModifiers::default());
        let rows = tree_rows(scene);
        assert_eq!(rows.len(), 3, "expanding the branch mounts its child");
        let stride = rows[1].absolute_frame().y - rows[0].absolute_frame().y;

        let from = centre(&rows[0]);
        pointer_down(scene, from);
        pointer_walk(scene, from, centre(&rows[2]));
        let projected = ReorderRowShift::target_offset(&rows[0]);
        assert!(
            (projected - stride).abs() < 0.01,
            "A is painted below its child while aimed at B, got {projected}"
        );

        // Crossing the rows being moved cannot collapse the projection.
        pointer_walk(scene, centre(&rows[2]), centre(&rows[1]));
        assert_eq!(
            ReorderRowShift::target_offset(&rows[1]),
            projected,
            "the branch keeps the projection the pointer already earned"
        );
        pointer_up(scene, centre(&rows[1]));
    });
}

/// Detaches the row with `id` from `nodes`, keeping its branch intact.
fn take_item(nodes: &mut Vec<Item>, id: &str) -> Option<Item> {
    for index in 0..nodes.len() {
        if nodes[index].id == id {
            return Some(nodes.remove(index));
        }
        if let Some(found) = take_item(&mut nodes[index].children, id) {
            return Some(found);
        }
    }
    None
}

/// Inserts `node` next to (or inside) the row with `target_id`.
fn insert_relative(
    nodes: &mut Vec<Item>,
    target_id: &str,
    position: TreeDropPosition,
    node: &Item,
) -> bool {
    for index in 0..nodes.len() {
        if nodes[index].id == target_id {
            match position {
                TreeDropPosition::Inside => nodes[index].children.push(node.clone()),
                TreeDropPosition::Before => nodes.insert(index, node.clone()),
                TreeDropPosition::After => nodes.insert(index + 1, node.clone()),
            }
            return true;
        }
        if insert_relative(&mut nodes[index].children, target_id, position, node) {
            return true;
        }
    }
    false
}

/// A tree whose caller owns the model: the drop edits the hierarchy the same way
/// a host application would, so the projection can be compared with the result.
struct MoveHarness {
    roots: State<Vec<Item>>,
}

impl View for MoveHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let roots = self.roots.clone();
        let drop_roots = self.roots.clone();
        let drag = TreeDragOperations {
            can_drop: Some(Rc::new(|_: &Item, _: &Item, _| true)),
            on_drop: Some(Rc::new(move |source: Item, target: Item, position| {
                let mut model = drop_roots.get();
                let Some(moved) = take_item(&mut model, source.id) else {
                    return;
                };
                if insert_relative(&mut model, target.id, position, &moved) {
                    roots.set(model);
                }
            })),
            ..TreeDragOperations::default()
        };
        AnyView::new(tree_harness_with(
            self.roots.get(),
            TreeSelection::default(),
            TreeSearch::default(),
            drag,
            TreeLayout::default(),
        ))
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![cove_composition::DynamicField::state("roots", &self.roots)]
    }
}

/// The visible row labels, painted top to bottom.
fn row_labels(scene: &InteractiveScene) -> Vec<String> {
    let mut labels: Vec<(f32, String)> = tree_rows(scene)
        .into_iter()
        .map(|row| {
            let label = collect_labels(&row).into_iter().next().unwrap_or_default();
            (row.absolute_frame().y, label)
        })
        .collect();
    labels.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    labels.into_iter().map(|(_, label)| label).collect()
}

fn collect_labels(node: &Node) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(semantics) = node.accessibility()
        && semantics.role == AccessibilityRole::StaticText
    {
        out.push(semantics.label);
    }
    for child in node.children() {
        out.extend(collect_labels(&child));
    }
    out
}

#[test]
fn tree_drag_committing_a_drop_leaves_the_rows_where_they_were_painted() {
    let roots = State::new(vec![
        item("a", "A", vec![]),
        item("b", "B", vec![]),
        item("c", "C", vec![]),
        item("d", "D", vec![]),
    ]);
    InteractiveScene::run(280.0, 220.0, |scene| {
        scene.install(AnyView::new(MoveHarness {
            roots: roots.clone(),
        }));
        let rows = tree_rows(scene);
        assert_eq!(
            row_labels(scene),
            vec!["0: A", "0: B", "0: C", "0: D"],
            "the row label carries its depth"
        );

        let from = centre(&rows[3]);
        let to = Point::new(
            rows[1].absolute_frame().x + rows[1].absolute_frame().width / 2.0,
            rows[1].absolute_frame().y + 2.0,
        );
        pointer_down(scene, from);
        // The top band of B projects a drop *before* B: D is painted in B's
        // slot and B and C step down one.
        pointer_walk(scene, from, to);
        let stride = rows[1].absolute_frame().y - rows[0].absolute_frame().y;
        assert!(
            (ReorderRowShift::target_offset(&rows[3]) + stride).abs() < 0.01,
            "D is projected one slot up, got {}",
            ReorderRowShift::target_offset(&rows[3])
        );

        pointer_up(scene, to);
        let committed = roots.get();
        assert_eq!(
            committed.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec!["a", "d", "b", "c"],
            "the caller's model takes the drop"
        );
        assert_eq!(
            row_labels(scene),
            vec!["0: A", "0: D", "0: B", "0: C"],
            "the laid-out order matches what the drag painted"
        );
        for row in tree_rows(scene) {
            assert_eq!(
                ReorderRowShift::target_offset(&row),
                0.0,
                "no row springs a second time after the drop, got {:?}",
                row_labels(scene)
            );
        }
    });
}

#[test]
fn tree_drag_reveals_the_branch_an_inside_drop_creates() {
    let roots = State::new(vec![
        item("a", "A", vec![]),
        item("b", "B", vec![]),
        item("c", "C", vec![]),
    ]);
    InteractiveScene::run(280.0, 220.0, |scene| {
        scene.install(AnyView::new(MoveHarness {
            roots: roots.clone(),
        }));
        let rows = tree_rows(scene);
        assert_eq!(
            row_labels(scene),
            vec!["0: A", "0: B", "0: C"],
            "every node starts as a collapsed root"
        );

        // A lands in B's middle band, which turns the leaf into a branch.
        drag(scene, &rows[0], &rows[1]);
        assert_eq!(
            row_labels(scene),
            vec!["0: B", "1: A", "0: C"],
            "the row dropped into a leaf opens it and stays visible"
        );
    });
}

#[test]
fn tree_drag_paints_projected_rows_inside_their_slots() {
    fn clip_owner(node: &Node) -> Option<Node> {
        let mut current = node.parent()?;
        loop {
            if current.clips_to_bounds() {
                return Some(current);
            }
            current = current.parent()?;
        }
    }

    let roots = vec![
        item(
            "scene",
            "Scene",
            vec![
                item(
                    "lights",
                    "Lights",
                    vec![item("key", "Key", vec![]), item("fill", "Fill", vec![])],
                ),
                item("objects", "Objects", vec![]),
            ],
        ),
        item("camera", "Camera", vec![]),
    ];
    InteractiveScene::run(280.0, 220.0, |scene| {
        scene.install(tree_harness_with(
            roots,
            TreeSelection {
                is_local: true,
                default_expanded: vec!["scene".to_string(), "lights".to_string()],
                default_selected: vec!["camera".to_string()],
                ..TreeSelection::default()
            },
            TreeSearch::default(),
            TreeDragOperations {
                can_drop: Some(Rc::new(|_: &Item, _: &Item, _| true)),
                on_drop: Some(Rc::new(|_: Item, _: Item, _: TreeDropPosition| {})),
                ..TreeDragOperations::default()
            },
            TreeLayout::default(),
        ));
        let rows = tree_rows(scene);
        assert_eq!(rows.len(), 6, "six rows mount, got {}", rows.len());
        let stride = rows[1].absolute_frame().y - rows[0].absolute_frame().y;
        let slots: Vec<f32> = rows.iter().map(|row| row.absolute_frame().y).collect();

        let from = centre(&rows[5]);
        let to = Point::new(from.x, rows[3].absolute_frame().y + stride * 0.3);
        pointer_down(scene, from);
        pointer_walk(scene, from, to);

        let displaced = tree_rows(scene);
        assert_eq!(displaced.len(), 6, "no row leaves the tree mid-drag");
        let painted: Vec<f32> = displaced.iter().map(|row| row.absolute_frame().y).collect();
        for (index, row) in displaced.iter().enumerate() {
            let frame = row.absolute_frame();
            let Some(owner) = clip_owner(row) else {
                continue;
            };
            let clip = owner.absolute_frame();
            assert!(
                frame.y >= clip.y - 0.5 && frame.y + frame.height <= clip.y + clip.height + 0.5,
                "row {index} painted at y={} left its clipping slot at y={}..{}",
                frame.y,
                clip.y,
                clip.y + clip.height
            );
        }
        assert!(
            rows[5].absolute_frame().y < slots[5],
            "the dragged row lifts toward its target, got {}",
            rows[5].absolute_frame().y
        );
        let mut expected = slots.clone();
        expected.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mut actual = painted.clone();
        actual.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(
            actual, expected,
            "the projection permutes the slots instead of dropping rows, got {painted:?}"
        );
        pointer_up(scene, to);
    });
}

/// A focused row carries the tree's key handling, so Alt+arrows have to reach
/// the session's move contract instead of being swallowed as plain navigation.
#[test]
fn alt_arrow_on_a_focused_row_moves_the_branch() {
    let drops: Rc<RefCell<Vec<(String, String, TreeDropPosition)>>> =
        Rc::new(RefCell::new(Vec::new()));
    let roots = vec![
        item(
            "scene",
            "Scene",
            vec![
                item(
                    "lights",
                    "Lights",
                    vec![item("key", "Key", vec![]), item("fill", "Fill", vec![])],
                ),
                item("objects", "Objects", vec![]),
            ],
        ),
        item("camera", "Camera", vec![]),
    ];
    InteractiveScene::run(280.0, 220.0, |scene| {
        scene.install(tree_harness_with(
            roots,
            TreeSelection {
                is_local: true,
                default_expanded: vec!["scene".to_string(), "lights".to_string()],
                ..TreeSelection::default()
            },
            TreeSearch::default(),
            TreeDragOperations {
                can_drop: Some(Rc::new(|_: &Item, _: &Item, _| true)),
                on_drop: {
                    let drops = drops.clone();
                    Some(Rc::new(move |source: Item, target: Item, position| {
                        drops.borrow_mut().push((
                            source.title.to_string(),
                            target.title.to_string(),
                            position,
                        ));
                    }))
                },
                ..TreeDragOperations::default()
            },
            TreeLayout::default(),
        ));
        let rows = tree_rows(scene);
        assert_eq!(rows.len(), 6, "six rows mount, got {}", rows.len());

        // Press on "Key" so the row itself holds focus, as a tap does.
        let key_row = centre(&rows[2]);
        pointer_down(scene, key_row);
        pointer_up(scene, key_row);
        scene.settle(2, 1.0);

        scene.key(Scancode::ARROW_DOWN, KeyModifiers::LEFT_ALT);

        let after_move = drops.borrow().clone();
        assert_eq!(
            after_move,
            vec![(
                "Key".to_string(),
                "Fill".to_string(),
                TreeDropPosition::After
            )],
            "Alt+Down asks for the sibling move the drop contract describes"
        );

        // Plain arrows on the same focused row still navigate.
        drops.borrow_mut().clear();
        scene.key(Scancode::ARROW_DOWN, KeyModifiers(0));
        assert!(
            drops.borrow().is_empty(),
            "a plain arrow stays navigation rather than a move"
        );
    });
}

/// Collapsing the forest unmounts the focused row, and the keyboard has to stay
/// with the tree so the next shortcut still lands.
#[test]
fn collapse_all_keeps_the_tree_on_the_keyboard() {
    let roots = vec![item(
        "scene",
        "Scene",
        vec![
            item("lights", "Lights", vec![]),
            item("objects", "Objects", vec![]),
        ],
    )];
    InteractiveScene::run(280.0, 220.0, |scene| {
        scene.install(tree_harness_with(
            roots,
            TreeSelection {
                is_local: true,
                default_expanded: vec!["scene".to_string()],
                ..TreeSelection::default()
            },
            TreeSearch::default(),
            TreeDragOperations::default(),
            TreeLayout::default(),
        ));
        let rows = tree_rows(scene);
        assert_eq!(
            row_labels(scene),
            vec!["0: Scene", "1: Lights", "1: Objects"],
            "the branch starts open"
        );

        let focused = centre(&rows[1]);
        pointer_down(scene, focused);
        pointer_up(scene, focused);
        scene.settle(2, 1.0);

        scene.key(Scancode::ARROW_LEFT, KeyModifiers::LEFT_ALT);
        assert_eq!(
            row_labels(scene),
            vec!["0: Scene"],
            "Alt+Left collapses the whole forest"
        );

        scene.key(Scancode::ARROW_RIGHT, KeyModifiers::LEFT_ALT);
        assert_eq!(
            row_labels(scene),
            vec!["0: Scene", "1: Lights", "1: Objects"],
            "Alt+Right still reaches the tree after the focused row left"
        );
    });
}
