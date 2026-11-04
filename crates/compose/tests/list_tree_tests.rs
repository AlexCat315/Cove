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
//! - indent guides are subtle 1pt vertical strokes with ancestor continuations
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

use cove_composition::{AnyView, LayoutDebugExt, TupleView, View};
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
use cove_ui_compose::primitives::data::lists::tree::options::{
    TreeDragOperations, TreeSearch, TreeSelection,
};
use cove_ui_compose::primitives::data::lists::tree::options::{TreeLayout, TreeOptions};
use cove_ui_compose::primitives::data::lists::tree::{
    Tree, TreeDropPosition, TreeNodeKey, TreeSearchFilterPolicy,
};
use cove_ui_compose::primitives::data::lists::{List, Selection};
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
    let start = source.absolute_frame();
    let end = target.absolute_frame();
    let dispatcher = scene.dispatcher();
    dispatcher.dispatch(&cove_platform::InputEvent::MouseButtonDown(
        MouseButtonEvent {
            button: MouseButton::Left,
            x: start.x + start.width / 2.0,
            y: start.y + start.height / 2.0,
            clicks: 1,
            modifiers: KeyModifiers::default(),
            kind: Default::default(),
        },
    ));
    scene.settle(2, 1.0);
    let to = Point::new(end.x + end.width / 2.0, end.y + end.height / 2.0);
    for step in 1..=6 {
        let t = step as f32 / 6.0;
        dispatcher.dispatch(&cove_platform::InputEvent::MouseMotion(
            cove_platform::MouseMotionEvent {
                x: start.x + start.width / 2.0 + (to.x - (start.x + start.width / 2.0)) * t,
                y: start.y + start.height / 2.0 + (to.y - (start.y + start.height / 2.0)) * t,
                delta_x: 0.0,
                delta_y: 0.0,
                kind: Default::default(),
            },
        ));
        scene.settle(2, 1.0);
    }
    dispatcher.dispatch(&cove_platform::InputEvent::MouseButtonUp(
        MouseButtonEvent {
            button: MouseButton::Left,
            x: to.x,
            y: to.y,
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
                let id = id.clone();
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
                        let id = id.clone();

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
        layout: TreeLayout::default(),
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
        assert!(
            indented_text
                .as_ref()
                .is_some_and(|node| node.absolute_frame().x > 0.0),
            "a depth-1 row label is indented past the root, found {:?}",
            indented_text.map(|node| node.absolute_frame())
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
    let selection = State::new(None);
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
        let child = rows
            .into_iter()
            .nth(1)
            .expect("the expanded child row mounts");
        click(scene, &child, KeyModifiers::default());
        assert_eq!(
            selection.get(),
            Some("camera".to_string()),
            "clicking Camera writes primary selection, got {:?}",
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
            (max_x - min_x, max_y - min_y)
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
                *width, 1.0,
                "every guide stroke is one pixel wide, got {width}"
            );
            assert!(
                *height > 1.0,
                "every guide stroke is vertical, got height {height}"
            );
        }
        for node in &guides {
            let Some(draw) = node.draw() else { continue };
            let mut list = DrawList::default();
            draw(&mut list, Point::default());
            for vertex in list.vertices() {
                let alpha = ((vertex.color >> 24) & 0xFF) as f32 / 255.0;
                assert!(
                    alpha <= 0.25,
                    "guide ink stays subtle (alpha <= 0.25), got {alpha}"
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
                .any(|(width, height)| *width == 1.0 && *height == 30.0),
            "an ancestor continuation stem spans the full 30pt row, got {rects:?}"
        );
    });
}
