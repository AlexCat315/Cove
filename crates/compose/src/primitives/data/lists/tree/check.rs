//! Index, check cascade and ancestry helpers shared by `Tree` and its actions.
//!
//! The projection (`flatten_visible`) only walks expanded branches, which is
//! what keeps a collapsed subtree cheap. Check state, path lookup and
//! "expand to node" need the opposite view — every node, collapsed or not — so
//! they share one eager index built on demand. Building it costs one walk of
//! the data the caller exposes through the child resolver; imperative calls
//! (`get_path`, `set_checked`, `scroll_to_key`, ...) are rare next to pointer
//! and keyboard input, and the per-frame projection never builds it unless
//! checkboxes are enabled.
use std::collections::{HashMap, HashSet};

use super::TreeNodeKey;

/// One node in the full-tree index.
pub(crate) struct IndexedNode<Element, Id: std::hash::Hash + Eq> {
    pub element: Element,
    pub id: Id,
    pub key: TreeNodeKey<Id>,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub is_leaf: bool,
}

/// Depth-first index of every node reachable from the roots. Repeated IDs keep
/// their distinct path keys; `by_id` points at the first occurrence so ID-based
/// bindings stay deterministic.
pub(crate) struct TreeIndex<Element, Id: std::hash::Hash + Eq> {
    pub nodes: Vec<IndexedNode<Element, Id>>,
    pub by_key: HashMap<TreeNodeKey<Id>, usize>,
    pub by_id: HashMap<Id, usize>,
}

impl<Element, Id> TreeIndex<Element, Id>
where
    Element: Clone,
    Id: Clone + std::hash::Hash + Eq,
{
    pub fn build(
        roots: &[Element],
        id_of: &dyn Fn(&Element) -> Id,
        children: &dyn Fn(&Element) -> Vec<Element>,
        has_children: Option<&dyn Fn(&Element) -> bool>,
    ) -> Self {
        let mut index = Self {
            nodes: Vec::new(),
            by_key: HashMap::new(),
            by_id: HashMap::new(),
        };
        let mut ancestry = HashSet::new();
        append(
            roots,
            None,
            &mut Vec::new(),
            0,
            id_of,
            children,
            has_children,
            &mut ancestry,
            &mut index,
        );
        index
    }

    pub fn node(&self, key: &TreeNodeKey<Id>) -> Option<&IndexedNode<Element, Id>> {
        self.by_key
            .get(key)
            .and_then(|&index| self.nodes.get(index))
    }

    pub fn node_by_id(&self, id: &Id) -> Option<&IndexedNode<Element, Id>> {
        self.by_id.get(id).and_then(|&index| self.nodes.get(index))
    }

    /// Root → node elements.
    pub fn path(&self, key: &TreeNodeKey<Id>) -> Vec<Element> {
        let mut index = self.by_key.get(key).copied();
        let mut out = Vec::new();
        while let Some(current) = index {
            out.push(self.nodes[current].element.clone());
            index = self.nodes[current].parent;
        }
        out.reverse();
        out
    }

    /// Keys of every ancestor, nearest first.
    pub fn ancestor_keys(&self, key: &TreeNodeKey<Id>) -> Vec<TreeNodeKey<Id>> {
        let Some(mut index) = self.by_key.get(key).copied() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        while let Some(parent) = self.nodes[index].parent {
            index = parent;
            out.push(self.nodes[index].key.clone());
        }
        out
    }

    /// The node and every descendant.
    pub fn subtree(&self, key: &TreeNodeKey<Id>) -> Vec<usize> {
        let Some(root) = self.by_key.get(key).copied() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut stack = vec![root];
        while let Some(index) = stack.pop() {
            out.push(index);
            stack.extend(self.nodes[index].children.iter().copied());
        }
        out
    }
}

#[allow(clippy::too_many_arguments)]
fn append<Element, Id>(
    nodes: &[Element],
    parent: Option<usize>,
    path_prefix: &mut Vec<usize>,
    depth: usize,
    id_of: &dyn Fn(&Element) -> Id,
    children: &dyn Fn(&Element) -> Vec<Element>,
    has_children: Option<&dyn Fn(&Element) -> bool>,
    ancestry: &mut HashSet<Id>,
    index: &mut TreeIndex<Element, Id>,
) where
    Element: Clone,
    Id: Clone + std::hash::Hash + Eq,
{
    for (position, element) in nodes.iter().enumerate() {
        let id = id_of(element);
        if !ancestry.insert(id.clone()) {
            continue;
        }
        let mut path = path_prefix.clone();
        path.push(position);
        let key = TreeNodeKey::new(id.clone(), path.clone());
        let declared_branch = has_children.map(|predicate| predicate(element));
        let branch = declared_branch.unwrap_or(true);
        let slot = index.nodes.len();
        index.nodes.push(IndexedNode {
            element: element.clone(),
            id: id.clone(),
            key: key.clone(),
            parent,
            children: Vec::new(),
            is_leaf: !branch,
        });
        index.by_key.insert(key, slot);
        index.by_id.entry(id.clone()).or_insert(slot);
        if branch {
            let kids = children(element);
            if declared_branch.is_none() && kids.is_empty() {
                index.nodes[slot].is_leaf = true;
            }
            if !kids.is_empty() {
                let count_before = index.nodes.len();
                let mut child_path = path;
                append(
                    &kids,
                    Some(slot),
                    &mut child_path,
                    depth + 1,
                    id_of,
                    children,
                    has_children,
                    ancestry,
                    index,
                );
                let child_slots: Vec<usize> = (count_before..index.nodes.len())
                    .filter(|&candidate| index.nodes[candidate].parent == Some(slot))
                    .collect();
                index.nodes[slot].children = child_slots;
            }
        }
        ancestry.remove(&id);
    }
}

/// Canonical check result: fully checked nodes plus indeterminate branches.
pub(crate) struct CheckSnapshot<Element, Id: std::hash::Hash + Eq> {
    pub checked_ids: HashSet<Id>,
    pub checked_keys: HashSet<TreeNodeKey<Id>>,
    pub half_checked_keys: HashSet<TreeNodeKey<Id>>,
    pub checked_elements: Vec<Element>,
}

impl<Element, Id: std::hash::Hash + Eq> Default for CheckSnapshot<Element, Id> {
    fn default() -> Self {
        Self {
            checked_ids: HashSet::new(),
            checked_keys: HashSet::new(),
            half_checked_keys: HashSet::new(),
            checked_elements: Vec::new(),
        }
    }
}

/// Resolve explicit checked sets into the canonical state a checkbox tree
/// reports. With cascade, a branch is fully checked when it is explicit or all
/// of its children are; an explicit branch forces its whole subtree. Without
/// cascade every node stands alone and nothing is half-checked.
pub(crate) fn snapshot<Element, Id>(
    index: &TreeIndex<Element, Id>,
    explicit_ids: &HashSet<Id>,
    explicit_keys: &HashSet<TreeNodeKey<Id>>,
    check_strictly: bool,
) -> CheckSnapshot<Element, Id>
where
    Element: Clone,
    Id: Clone + std::hash::Hash + Eq,
{
    let mut result = CheckSnapshot::default();
    if check_strictly {
        for node in &index.nodes {
            if explicit_ids.contains(&node.id) || explicit_keys.contains(&node.key) {
                result.checked_ids.insert(node.id.clone());
                result.checked_keys.insert(node.key.clone());
                result.checked_elements.push(node.element.clone());
            }
        }
        return result;
    }

    let mut all = vec![false; index.nodes.len()];
    let mut any = vec![false; index.nodes.len()];
    // Children are appended after their parent, so a reverse sweep resolves
    // every subtree before the node that owns it.
    for slot in (0..index.nodes.len()).rev() {
        let node = &index.nodes[slot];
        let explicit = explicit_ids.contains(&node.id) || explicit_keys.contains(&node.key);
        let all_children =
            !node.children.is_empty() && node.children.iter().all(|&child| all[child]);
        all[slot] = explicit || all_children;
        any[slot] = all[slot] || node.children.iter().any(|&child| any[child]);
    }

    fn collect<Element, Id>(
        index: &TreeIndex<Element, Id>,
        all: &[bool],
        any: &[bool],
        slot: usize,
        forced: bool,
        result: &mut CheckSnapshot<Element, Id>,
    ) where
        Element: Clone,
        Id: Clone + std::hash::Hash + Eq,
    {
        let node = &index.nodes[slot];
        let checked = forced || all[slot];
        if checked {
            result.checked_ids.insert(node.id.clone());
            result.checked_keys.insert(node.key.clone());
            result.checked_elements.push(node.element.clone());
        }
        for &child in &node.children {
            collect(index, all, any, child, checked, result);
        }
        if !checked && !node.children.is_empty() && node.children.iter().any(|&child| any[child]) {
            result.half_checked_keys.insert(node.key.clone());
        }
    }

    for (slot, node) in index.nodes.iter().enumerate() {
        if node.parent.is_none() {
            collect(index, &all, &any, slot, false, &mut result);
        }
    }
    result
}

/// Toggle one node and return the canonical sets to write back.
///
/// With cascade, checking inserts the whole subtree and then repairs ancestors;
/// unchecking removes the subtree and clears every ancestor, which is what makes
/// them indeterminate.
pub(crate) fn toggle<Element, Id>(
    index: &TreeIndex<Element, Id>,
    explicit_ids: &HashSet<Id>,
    explicit_keys: &HashSet<TreeNodeKey<Id>>,
    target: &TreeNodeKey<Id>,
    checked: bool,
    check_strictly: bool,
) -> (HashSet<Id>, HashSet<TreeNodeKey<Id>>)
where
    Element: Clone,
    Id: Clone + std::hash::Hash + Eq,
{
    let mut ids = explicit_ids.clone();
    let mut keys = explicit_keys.clone();
    let Some(slot) = index.by_key.get(target).copied() else {
        return (ids, keys);
    };

    if check_strictly {
        let node = &index.nodes[slot];
        if checked {
            ids.insert(node.id.clone());
            keys.insert(node.key.clone());
        } else {
            ids.remove(&node.id);
            keys.remove(&node.key);
        }
        return (ids, keys);
    }

    let subtree = index.subtree(target);
    for candidate in subtree {
        let node = &index.nodes[candidate];
        if checked {
            ids.insert(node.id.clone());
            keys.insert(node.key.clone());
        } else {
            ids.remove(&node.id);
            keys.remove(&node.key);
        }
    }

    let mut ancestor = index.nodes[slot].parent;
    while let Some(current) = ancestor {
        let node = &index.nodes[current];
        if checked {
            let complete = !node.children.is_empty()
                && node.children.iter().all(|&child| {
                    let child = &index.nodes[child];
                    ids.contains(&child.id)
                });
            if complete {
                ids.insert(node.id.clone());
                keys.insert(node.key.clone());
            } else {
                ids.remove(&node.id);
                keys.remove(&node.key);
            }
        } else {
            ids.remove(&node.id);
            keys.remove(&node.key);
        }
        ancestor = node.parent;
    }

    let canonical = snapshot(index, &ids, &keys, false);
    (canonical.checked_ids, canonical.checked_keys)
}

/// Resolve a caller-supplied checked set. `leaf_only` drops branch keys, which
/// matches `getCheckedKeys(leafOnly)`.
pub(crate) fn canonical_checked<Element, Id>(
    index: &TreeIndex<Element, Id>,
    ids: HashSet<Id>,
    keys: HashSet<TreeNodeKey<Id>>,
    check_strictly: bool,
    leaf_only: bool,
) -> (HashSet<Id>, HashSet<TreeNodeKey<Id>>)
where
    Element: Clone,
    Id: Clone + std::hash::Hash + Eq,
{
    let (mut ids, mut keys) = if leaf_only {
        let keep_slots: HashSet<usize> = index
            .by_key
            .values()
            .copied()
            .filter(|&slot| index.nodes[slot].is_leaf)
            .collect();
        let keys: HashSet<TreeNodeKey<Id>> = keys
            .into_iter()
            .filter(|key| {
                index
                    .by_key
                    .get(key)
                    .is_some_and(|slot| keep_slots.contains(slot))
            })
            .collect();
        let ids: HashSet<Id> = keys.iter().map(|key| key.id.clone()).collect();
        (ids, keys)
    } else {
        (ids, keys)
    };

    if !check_strictly {
        let canonical = snapshot(index, &ids, &keys, false);
        ids = canonical.checked_ids;
        keys = canonical.checked_keys;
    }
    (ids, keys)
}
