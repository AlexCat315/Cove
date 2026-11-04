//! Capability coverage for the tree: check cascade, expansion policy,
//! filtering, lazy loading and the imperative methods.
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use cove_composition::AnyView;
use cove_core::Binding;
use cove_platform::KeyModifiers;
use cove_ui_compose::primitives::data::lists::RowState;
use cove_ui_compose::primitives::data::lists::tree::{
    Tree, TreeCheck, TreeExpansion, TreeLazy, TreeNodeKey, TreeOptions, TreeSearch, TreeSelection,
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Item {
    id: usize,
}

fn item(id: usize) -> Item {
    Item { id }
}

/// Root 0 → {1 → {3}, 2}.
fn roots() -> Vec<Item> {
    vec![item(0)]
}

fn children(node: &Item) -> Vec<Item> {
    match node.id {
        0 => vec![item(1), item(2)],
        1 => vec![item(3)],
        _ => Vec::new(),
    }
}

fn is_branch(item: &Item) -> bool {
    matches!(item.id, 0 | 1)
}

fn build_tree(
    options: TreeOptions<Item, usize>,
) -> Tree<Item, usize, impl Fn(&Item, RowState) -> AnyView> {
    Tree::new(
        roots(),
        |item: &Item| item.id,
        children,
        |_: &Item, _: RowState| AnyView::new(cove_composition::EmptyView),
    )
    .options(options)
    .has_children(is_branch)
}

fn key_of(
    tree: &Tree<Item, usize, impl Fn(&Item, RowState) -> AnyView>,
    id: usize,
) -> TreeNodeKey<usize> {
    tree.entries()
        .into_iter()
        .find(|entry| entry.id == id)
        .map(|entry| entry.key)
        .unwrap_or_else(|| panic!("row {id} is not visible"))
}

fn expand_all_branches(tree: &Tree<Item, usize, impl Fn(&Item, RowState) -> AnyView>) {
    let root = key_of(tree, 0);
    tree.expand_key(&root);
    let branch = key_of(tree, 1);
    tree.expand_key(&branch);
}

fn expanding_options() -> TreeOptions<Item, usize> {
    TreeOptions {
        selection: TreeSelection {
            expanded: Some(Binding::constant(HashSet::from([0usize, 1]))),
            ..TreeSelection::default()
        },
        ..TreeOptions::default()
    }
}

#[test]
fn check_cascade_reports_checked_and_half_checked_nodes() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let recorded = events.clone();
    type Keys = HashSet<TreeNodeKey<usize>>;
    let on_change: Rc<dyn Fn(&Item, bool, &Keys, &Keys)> =
        Rc::new(move |node, checked, checked_keys, half_keys| {
            recorded
                .borrow_mut()
                .push((node.id, checked, checked_keys.len(), half_keys.len()));
        });
    let options = TreeOptions {
        check: TreeCheck {
            is_enabled: true,
            on_change: Some(on_change),
            ..TreeCheck::default()
        },
        ..TreeOptions::default()
    };
    let tree = build_tree(options);
    expand_all_branches(&tree);
    let branch = key_of(&tree, 1);
    tree.set_checked(&branch, true, None);

    let checked: Vec<usize> = tree
        .get_checked_keys()
        .into_iter()
        .map(|key| key.id)
        .collect();
    assert_eq!(
        checked.len(),
        2,
        "branch and child is checked, got {checked:?}"
    );
    assert!(checked.contains(&1) && checked.contains(&3));
    let half: Vec<usize> = tree
        .get_half_checked_keys()
        .into_iter()
        .map(|key| key.id)
        .collect();
    assert_eq!(half, vec![0], "the root is half checked");

    let sibling = key_of(&tree, 2);
    tree.set_checked(&sibling, true, None);
    let all: Vec<usize> = tree
        .get_checked_keys()
        .into_iter()
        .map(|key| key.id)
        .collect();
    assert_eq!(
        all.len(),
        4,
        "a fully checked root now reports the whole subtree: {all:?}"
    );
    assert!(tree.get_half_checked_keys().is_empty());

    let leaf = key_of(&tree, 3);
    tree.set_checked(&leaf, false, None);
    let checked: Vec<usize> = tree
        .get_checked_keys()
        .into_iter()
        .map(|key| key.id)
        .collect();
    assert_eq!(checked, vec![2], "only the untouched sibling stays checked");
    let half: Vec<usize> = tree
        .get_half_checked_keys()
        .into_iter()
        .map(|key| key.id)
        .collect();
    assert_eq!(
        half,
        vec![0],
        "the root is indeterminate; the emptied branch is plain unchecked, got {half:?}"
    );

    let events = events.borrow();
    assert_eq!(events.len(), 3, "every change reports one event");
    assert_eq!(
        events[0],
        (1, true, 2, 1),
        "checked node, keys and half keys"
    );
    assert_eq!(events[2].0, 3);
    assert!(!events[2].1);
}

#[test]
fn check_strictly_keeps_every_node_independent() {
    let options = TreeOptions {
        check: TreeCheck {
            is_enabled: true,
            check_strictly: true,
            ..TreeCheck::default()
        },
        ..TreeOptions::default()
    };
    let tree = build_tree(options);
    let root = key_of(&tree, 0);
    tree.set_checked(&root, true, None);

    let checked: Vec<usize> = tree
        .get_checked_keys()
        .into_iter()
        .map(|key| key.id)
        .collect();
    assert_eq!(checked, vec![0], "no cascade with checkStrictly");
    assert!(tree.get_half_checked_keys().is_empty());
}

#[test]
fn set_checked_keys_honours_leaf_only() {
    let options = TreeOptions {
        check: TreeCheck {
            is_enabled: true,
            ..TreeCheck::default()
        },
        ..TreeOptions::default()
    };
    let tree = build_tree(options);
    expand_all_branches(&tree);
    let branch = key_of(&tree, 1);
    let leaf = key_of(&tree, 2);
    tree.set_checked_keys([branch, leaf], true);

    let checked: Vec<usize> = tree
        .get_checked_keys()
        .into_iter()
        .map(|key| key.id)
        .collect();
    assert_eq!(checked, vec![2], "branch keys are dropped for leaf-only");
    assert_eq!(
        tree.get_checked_nodes()
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        vec![2]
    );
}

#[test]
fn default_expand_all_reveals_every_branch() {
    let options = TreeOptions {
        expansion: TreeExpansion {
            default_expand_all: true,
            ..TreeExpansion::default()
        },
        ..TreeOptions::default()
    };
    let tree = build_tree(options);
    assert_eq!(
        tree.entries()
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>(),
        vec![0, 1, 3, 2]
    );
}

#[test]
fn accordion_keeps_one_sibling_open() {
    #[derive(Clone)]
    struct Node {
        id: usize,
    }
    fn kids(node: &Node) -> Vec<Node> {
        match node.id {
            0 => vec![Node { id: 10 }, Node { id: 11 }],
            1 => vec![Node { id: 12 }, Node { id: 13 }],
            _ => Vec::new(),
        }
    }
    let tree = Tree::new(
        vec![Node { id: 0 }, Node { id: 1 }],
        |node: &Node| node.id,
        kids,
        |_: &Node, _: RowState| AnyView::new(cove_composition::EmptyView),
    )
    .has_children(|node: &Node| matches!(node.id, 0 | 1))
    .options(TreeOptions {
        expansion: TreeExpansion {
            accordion: true,
            ..TreeExpansion::default()
        },
        ..TreeOptions::default()
    });

    let first = tree
        .entries()
        .into_iter()
        .find(|entry| entry.id == 0)
        .expect("first branch");
    tree.expand_key(&first.key);
    assert!(tree.get_expanded_ids().contains(&0));

    let second = tree
        .entries()
        .into_iter()
        .find(|entry| entry.id == 1)
        .expect("sibling branch");
    tree.expand_key(&second.key);
    let expanded = tree.get_expanded_ids();
    assert!(expanded.contains(&1));
    assert!(
        !expanded.contains(&0),
        "the accordion closed the other sibling, got {expanded:?}"
    );
}

#[test]
fn scroll_to_key_expands_the_path_and_get_path_returns_it() {
    let options = TreeOptions {
        expansion: TreeExpansion {
            auto_expand_parent: true,
            ..TreeExpansion::default()
        },
        ..TreeOptions::default()
    };
    let tree = build_tree(options);
    expand_all_branches(&tree);
    let leaf = key_of(&tree, 3);
    let path = tree.get_path(&leaf);
    assert_eq!(
        path.iter().map(|item| item.id).collect::<Vec<_>>(),
        vec![0, 1, 3],
        "get_path walks from the root to the node"
    );

    tree.collapse_all();
    tree.scroll_to_key(leaf.clone());
    assert_eq!(
        tree.entries()
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>(),
        vec![0, 1, 3, 2],
        "scrolling expands the ancestors so the target is reachable"
    );
}

#[test]
fn programmatic_filter_overrides_the_option_query() {
    let options = TreeOptions {
        search: TreeSearch {
            text: Some(Rc::new(|item: &Item| format!("Node {}", item.id))),
            query: "Node 2".to_string(),
            ..TreeSearch::default()
        },
        ..TreeOptions::default()
    };
    let tree = build_tree(options);
    assert_eq!(
        tree.entries()
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>(),
        vec![0, 2]
    );

    tree.filter("Node 3");
    let visible: Vec<usize> = tree.entries().iter().map(|entry| entry.id).collect();
    assert_eq!(visible, vec![0, 1, 3], "the programmatic query wins");
    assert!(
        tree.entries()
            .iter()
            .any(|entry| entry.id == 3 && entry.is_search_hit)
    );

    tree.clear_filter();
    assert_eq!(
        tree.entries()
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>(),
        vec![0, 2],
        "clearing returns to the option query"
    );
}

#[test]
fn a_custom_filter_predicate_replaces_text_matching() {
    let options = TreeOptions {
        search: TreeSearch {
            query: "anything".to_string(),
            filter: Some(Rc::new(|item: &Item, _query: &str| item.id == 3)),
            ..TreeSearch::default()
        },
        ..TreeOptions::default()
    };
    let tree = build_tree(options);
    let entries = tree.entries();
    assert_eq!(
        entries.iter().map(|entry| entry.id).collect::<Vec<_>>(),
        vec![0, 1, 3],
        "the predicate decides the visible branch"
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.id == 3 && entry.is_search_hit)
    );
}

#[test]
fn lazy_load_runs_once_per_node() {
    let loads = Rc::new(RefCell::new(Vec::new()));
    let recorded = loads.clone();
    let options = TreeOptions {
        lazy: TreeLazy {
            is_enabled: true,
            load_data: Some(Rc::new(move |item: &Item| {
                recorded.borrow_mut().push(item.id);
            })),
            is_leaf: Some(Rc::new(|item: &Item| matches!(item.id, 2 | 3))),
            ..TreeLazy::default()
        },
        ..TreeOptions::default()
    };
    let tree = build_tree(options);
    let root = key_of(&tree, 0);
    tree.expand_key(&root);
    assert_eq!(
        loads.borrow().as_slice(),
        &[0],
        "expanding the branch asks the host to load it"
    );
    assert_eq!(tree.get_loaded_keys().len(), 1);

    tree.collapse_key(&root);
    tree.expand_key(&root);
    assert_eq!(
        loads.borrow().as_slice(),
        &[0],
        "a loaded branch is not requested again"
    );
    let leaf = tree
        .entries()
        .into_iter()
        .find(|entry| entry.id == 2)
        .unwrap();
    assert!(leaf.is_leaf);
    assert!(
        !tree
            .entries()
            .into_iter()
            .find(|entry| entry.id == 1)
            .unwrap()
            .is_leaf
    );
}

#[test]
fn local_selection_supports_single_and_multiple_modes() {
    let options = TreeOptions {
        selection: TreeSelection::local([1usize]),
        ..TreeOptions::default()
    };
    let tree = build_tree(options);
    expand_all_branches(&tree);
    assert_eq!(tree.get_selected_ids(), HashSet::from([1]));

    let second = tree
        .entries()
        .into_iter()
        .find(|entry| entry.id == 2)
        .unwrap();
    tree.select(&second, KeyModifiers(0));
    assert_eq!(tree.get_selected_ids(), HashSet::from([2]));

    let multiple_options = TreeOptions {
        selection: TreeSelection::local_multiple([1usize]),
        ..TreeOptions::default()
    };
    let multiple = build_tree(multiple_options);
    expand_all_branches(&multiple);
    let second = multiple
        .entries()
        .into_iter()
        .find(|entry| entry.id == 2)
        .unwrap();
    multiple.select(&second, KeyModifiers::LEFT_GUI);
    assert_eq!(
        multiple.get_selected_ids(),
        HashSet::from([1, 2]),
        "the tree-owned multiple selection toggles"
    );
    let first = multiple
        .entries()
        .into_iter()
        .find(|entry| entry.id == 1)
        .unwrap();
    multiple.select(&first, KeyModifiers(0));
    assert_eq!(
        multiple.get_selected_ids(),
        HashSet::from([1]),
        "a plain click replaces the tree-owned multiple selection"
    );
}

#[test]
fn disabling_selection_still_moves_the_cursor() {
    let options = TreeOptions {
        selection: TreeSelection::local([1usize]).disabled(),
        ..TreeOptions::default()
    };
    let tree = build_tree(options);
    expand_all_branches(&tree);
    let second = tree
        .entries()
        .into_iter()
        .find(|entry| entry.id == 2)
        .unwrap();
    tree.select(&second, KeyModifiers(0));
    assert_eq!(
        tree.get_selected_ids(),
        HashSet::from([1]),
        "selection is unchanged"
    );
    assert_eq!(
        tree.get_current_key().map(|key| key.id),
        Some(2),
        "the keyboard cursor still moved"
    );
}

#[test]
fn expanding_options_fixture_is_expanded() {
    let tree = build_tree(expanding_options());
    assert_eq!(tree.entries().len(), 4);
}

#[test]
fn checkbox_rows_drive_the_check_binding() {
    use crate::support::InteractiveScene;
    use cove_composition::Scope;
    use cove_composition::View;
    use cove_core::State;
    use cove_core::scene::{AccessibilityRole, EventPhase, PointerPhase};
    use cove_platform::{MouseButton, MouseButtonEvent};
    use cove_ui_compose::modifiers::layout::LayoutFrameExt;

    struct Harness {
        checked: State<HashSet<usize>>,
        selected: State<Option<usize>>,
    }

    impl View for Harness {
        fn body(&self, _cx: &Scope) -> AnyView {
            let _ = self.checked.get();
            let checked = self.checked.clone();
            let binding = Binding::new(
                {
                    let checked = checked.clone();
                    move || checked.get()
                },
                move |value: HashSet<usize>| checked.set(value),
            );
            let expanded: HashSet<usize> = [0usize, 1].into_iter().collect();
            let selected = self.selected.clone();
            let selection = Binding::new(
                {
                    let selected = selected.clone();
                    move || selected.get()
                },
                move |value: Option<usize>| selected.set(value),
            );
            let options = TreeOptions {
                check: TreeCheck {
                    is_enabled: true,
                    checked: Some(binding),
                    ..TreeCheck::default()
                },
                selection: TreeSelection {
                    ids: cove_ui_compose::primitives::data::lists::Selection::single(selection),
                    expanded: Some(Binding::constant(expanded)),
                    ..TreeSelection::default()
                },
                ..TreeOptions::default()
            };
            AnyView::new(
                Tree::new(
                    roots(),
                    |item: &Item| item.id,
                    children,
                    |_: &Item, _: RowState| AnyView::new(cove_composition::EmptyView),
                )
                .options(options)
                .has_children(is_branch)
                .frame_size(280.0, 220.0),
            )
        }
        fn tag(&self) -> &'static str {
            "TreeCheckHarness"
        }
    }

    fn click(scene: &InteractiveScene, node: &cove_core::scene::Node) {
        let frame = node.absolute_frame();
        let event = MouseButtonEvent {
            button: MouseButton::Left,
            x: frame.x + frame.width / 2.0,
            y: frame.y + frame.height / 2.0,
            clicks: 1,
            modifiers: KeyModifiers::default(),
            kind: Default::default(),
        };
        let handlers = scene.registry.handlers(node);
        let pointer = handlers
            .pointer
            .expect("the checkbox installs a pointer handler");
        let _ = pointer(&event, PointerPhase::Down, EventPhase::Target);
        let _ = pointer(&event, PointerPhase::Up, EventPhase::Target);
        scene.settle(2, 1.0);
    }

    let checked = State::new(HashSet::new());
    let observed = checked.clone();
    let selected = State::new(None);
    let observed_selection = selected.clone();
    InteractiveScene::run(320.0, 240.0, |scene| {
        scene.install(AnyView::new(Harness {
            checked: checked.clone(),
            selected: selected.clone(),
        }));
        let mut boxes: Vec<cove_core::scene::Node> = scene
            .all(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::Checkbox)
            })
            .into_iter()
            .collect();
        assert_eq!(boxes.len(), 4, "one checkbox per expanded row");
        boxes.sort_by(|a, b| {
            a.absolute_frame()
                .y
                .partial_cmp(&b.absolute_frame().y)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        click(scene, &boxes[0]);
        assert_eq!(
            observed.get(),
            HashSet::from([0, 1, 2, 3]),
            "checking the root cascades to the whole subtree"
        );
        assert_eq!(
            observed_selection.get(),
            None,
            "clicking the checkbox must not also select the row"
        );
    });
}

#[test]
fn default_expanded_ids_seed_tree_owned_expansion() {
    let options = TreeOptions {
        selection: TreeSelection {
            default_expanded: vec![0, 1],
            ..TreeSelection::default()
        },
        ..TreeOptions::default()
    };
    let tree = build_tree(options);
    assert_eq!(
        tree.entries()
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>(),
        vec![0, 1, 3, 2],
        "uncontrolled defaultExpandedKeys expands the authored paths"
    );
    assert_eq!(tree.get_expanded_ids(), HashSet::from([0, 1]));
}

#[test]
fn space_checks_the_current_row_when_checkboxes_are_on() {
    use cove_composition::Scancode;
    use cove_platform::KeyEvent;

    let options = TreeOptions {
        check: TreeCheck {
            is_enabled: true,
            ..TreeCheck::default()
        },
        ..TreeOptions::default()
    };
    let tree = build_tree(options);
    let space = KeyEvent {
        scancode: Scancode::SPACE,
        keycode: 0,
        modifiers: KeyModifiers(0),
        is_repeat: false,
    };
    assert!(tree.handle_key(&space), "Space is consumed");
    let checked: Vec<usize> = tree
        .get_checked_keys()
        .into_iter()
        .map(|key| key.id)
        .collect();
    assert_eq!(
        checked.len(),
        4,
        "Space checks the current root and cascades to its subtree: {checked:?}"
    );
}
