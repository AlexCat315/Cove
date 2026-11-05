//! Translation of Primitives/Data/Lists/Tree.swift.
//!
//! Hierarchical, virtualized tree. Visual chrome (selection fill, indentation,
//! disclosure chevron) is delegated to the active `TreeRowStyle`; the default
//! is `DefaultTreeRowStyle`.
//!
//! The configuration groups live in [`super::options`], the flattening,
//! cascade and keyboard rules here. Behaviour checklist:
//! - only expanded branches are projected; a collapsed subtree is never mounted
//! - paths are identity tokens for selection, expansion, check and drag state;
//!   filtering must not renumber them, so callers' keys stay valid while
//!   search merely hides non-matching siblings
//! - a cheap branch predicate lets a synchronous or lazy provider defer child
//!   construction until expansion
//! - searching filters and auto-expands ancestors of every match, or only
//!   highlights them under `HighlightOnly`
//! - checkbox cascade keeps parents fully checked only when every child is,
//!   and reports indeterminate branches separately; `checkStrictly` disables it
//! - `accordion` keeps one sibling open, `auto_expand_parent` repairs the path
//!   before a programmatic scroll or cursor move
//! - Down/Up move by one visible row; Left collapses or selects the parent;
//!   Right expands or selects the first child; Space checks when checkboxes
//!   are enabled; Alt+Left/Right collapse/expand everything
//! - Shift extends from the anchor, Cmd/Ctrl toggles, anything else replaces
//! - indent guides draw a branch line per depth with a continuation column
//!   only where an ancestor still has a following sibling

use crate::Scope;
use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::rc::Rc;

use super::super::selection::{CollectionId, RowState};
use super::check::{self, CheckSnapshot, TreeIndex};
use super::options::{
    TreeCheck, TreeDragOperations, TreeEdit, TreeEvents, TreeExpansion, TreeLazy, TreeOptions,
    TreeSearchMatch, TreeSelection,
};
use super::{TreeDropPosition, TreeNodeKey, TreeScrollAlign, TreeSearchFilterPolicy};
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::AppearanceExt;
use crate::primitives::controls::{Button, Checkbox, CheckboxState};
use crate::primitives::feedback::progress_circle::ProgressCircle;
use crate::primitives::layout::virtualization::{FixedVirtualStack, VirtualScrollAlign};
use crate::primitives::media::{Icon, ui_icon_catalog};
use crate::primitives::overlays::selection_key_host::SelectionKeyHost;
use crate::theme::styles::buttons::ButtonStyleViewExt;
use crate::theme::styles::components::{TreeRowStyleConfiguration, TreeRowStyleEnvironment};
use crate::theme::{SemanticColorRef, theme_for_node};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View};
use cove_composition::Scancode;
use cove_core::scene::NodeWeak;
use cove_core::scene::{
    AccessibilityActions, AccessibilityRole, AccessibilitySemantics, Align, DrawHook, EventPhase,
    EventResult, FlexDirection, HoverPhase, InputHandlerRoute, InteractionRegistryHolder, Justify,
    LayoutNode, Node, PointerCaptureHolder, PointerPhase, Wrap,
};
use cove_core::{Binding, Color, Point, Rect, State};
use cove_platform::{KeyEvent, KeyModifiers, MouseButton, SystemCursor};

const PRESSED: &str = "__tree_row_pressed";
const HOVERED: &str = "__tree_row_hovered";
const DRAG_PRESS: &str = "__tree_row_drag_press";
const ROW_GUTTER: &str = "__tree_row_gutter";
const GUIDE_MARKER: &str = "__tree_guide";

/// One visible row: the element plus its place in the hierarchy.
#[derive(Clone, Debug, PartialEq)]
pub struct TreeEntry<Element, Id: std::hash::Hash + Eq> {
    pub element: Element,
    pub id: Id,
    pub key: TreeNodeKey<Id>,
    pub depth: usize,
    pub parent_id: Option<Id>,
    pub parent_key: Option<TreeNodeKey<Id>>,
    pub ancestor_has_next_siblings: Vec<bool>,
    pub has_next_sibling: bool,
    pub is_search_hit: bool,
    pub has_children: bool,
    pub is_leaf: bool,
    pub is_expanded: bool,
    pub is_checked: bool,
    pub is_half_checked: bool,
    pub is_loading: bool,
}

/// The search match bookkeeping for one projection.
#[derive(Clone, Default)]
pub struct TreeSearchMetadata<Id> {
    pub self_matches: HashMap<Id, bool>,
    pub subtree_matches: HashMap<Id, bool>,
}

/// Everything a projection needs to decide whether a node matches: the query,
/// the text extractor, an optional predicate that replaces it, and the
/// comparison rules.
pub(crate) struct SearchSpec<'a, Element> {
    pub query: &'a str,
    pub text: Option<&'a dyn Fn(&Element) -> String>,
    pub filter: Option<&'a dyn Fn(&Element, &str) -> bool>,
    pub ignore_case: bool,
    pub match_mode: TreeSearchMatch,
}

impl<'a, Element> Clone for SearchSpec<'a, Element> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, Element> Copy for SearchSpec<'a, Element> {}

impl<Element> SearchSpec<'_, Element> {
    pub fn is_active(&self) -> bool {
        !self.query.trim().is_empty() && (self.text.is_some() || self.filter.is_some())
    }

    pub fn normalized(&self) -> String {
        let query = self.query.trim();
        if self.ignore_case {
            query.to_lowercase()
        } else {
            query.to_string()
        }
    }

    pub fn matches(&self, element: &Element, query: &str) -> bool {
        if let Some(filter) = self.filter {
            return filter(element, query);
        }
        let Some(text) = self.text else {
            return false;
        };
        let haystack = text(element);
        let haystack = if self.ignore_case {
            haystack.to_lowercase()
        } else {
            haystack
        };
        match self.match_mode {
            TreeSearchMatch::Contains => haystack.contains(query),
            TreeSearchMatch::Prefix => haystack.starts_with(query),
            TreeSearchMatch::Exact => haystack == query,
        }
    }
}

/// Flattens the expanded branches of a tree.
///
/// Children of collapsed branches are never projected, so cost follows the
/// visible rows rather than the whole forest; a branch predicate lets the
/// provider avoid constructing them at all. Paths stay keyed to the unfiltered
/// scene so the caller's keys remain valid while search merely hides
/// non-matching siblings.
pub fn flatten_tree<Element, Id>(
    roots: &[Element],
    id_of: &dyn Fn(&Element) -> Id,
    children: &dyn Fn(&Element) -> Vec<Element>,
    has_children: Option<&dyn Fn(&Element) -> bool>,
    expanded: &HashSet<Id>,
    expanded_keys: &HashSet<TreeNodeKey<Id>>,
    search: Option<(&dyn Fn(&Element) -> String, &str)>,
) -> Vec<TreeEntry<Element, Id>>
where
    Element: Clone,
    Id: std::hash::Hash + Eq + Clone,
{
    flatten_tree_matching(
        roots,
        id_of,
        children,
        has_children,
        expanded,
        expanded_keys,
        search,
        true,
    )
}

/// [`flatten_tree`] with the filter decision left to the caller.
///
/// A search marks matches in both policies; only `FilterAndAutoExpand` also
/// hides non-matches and auto-expands their ancestors. `filters` separates the
/// two, so a highlight-only search keeps every expanded row mounted and still
/// reports `is_search_hit`.
pub fn flatten_tree_matching<Element, Id>(
    roots: &[Element],
    id_of: &dyn Fn(&Element) -> Id,
    children: &dyn Fn(&Element) -> Vec<Element>,
    has_children: Option<&dyn Fn(&Element) -> bool>,
    expanded: &HashSet<Id>,
    expanded_keys: &HashSet<TreeNodeKey<Id>>,
    search: Option<(&dyn Fn(&Element) -> String, &str)>,
    filters: bool,
) -> Vec<TreeEntry<Element, Id>>
where
    Element: Clone,
    Id: std::hash::Hash + Eq + Clone,
{
    let spec = search.map(|(text, query)| SearchSpec {
        query,
        text: Some(text),
        filter: None,
        ignore_case: true,
        match_mode: TreeSearchMatch::Contains,
    });
    flatten_visible(
        roots,
        id_of,
        children,
        has_children,
        expanded,
        expanded_keys,
        spec,
        filters,
        None,
    )
}

/// The one depth-first projection behind `Tree` and `TreeTable`. With `seen`, a
/// repeated ID refers to its first visible occurrence, which also stops a cycle
/// in a dynamic child provider.
#[allow(clippy::too_many_arguments)]
pub(crate) fn flatten_visible<Element, Id>(
    roots: &[Element],
    id_of: &dyn Fn(&Element) -> Id,
    children: &dyn Fn(&Element) -> Vec<Element>,
    has_children: Option<&dyn Fn(&Element) -> bool>,
    expanded: &HashSet<Id>,
    expanded_keys: &HashSet<TreeNodeKey<Id>>,
    search: Option<SearchSpec<'_, Element>>,
    filters: bool,
    mut seen: Option<&mut HashSet<Id>>,
) -> Vec<TreeEntry<Element, Id>>
where
    Element: Clone,
    Id: std::hash::Hash + Eq + Clone,
{
    let query = search.map(|spec| spec.normalized()).unwrap_or_default();
    let filter_active = filters && search.is_some_and(|spec| spec.is_active());

    // A blank query matches nothing to mark, and walking it would resolve lazy
    // children the tree has not asked for yet.
    let metadata = search.filter(|spec| spec.is_active()).map(|spec| {
        let mut self_matches = HashMap::new();
        let mut subtree_matches = HashMap::new();
        fn walk<Element, Id: std::hash::Hash + Eq + Clone>(
            node: &Element,
            id_of: &dyn Fn(&Element) -> Id,
            children: &dyn Fn(&Element) -> Vec<Element>,
            has_children: Option<&dyn Fn(&Element) -> bool>,
            spec: SearchSpec<'_, Element>,
            query: &str,
            self_matches: &mut HashMap<Id, bool>,
            subtree_matches: &mut HashMap<Id, bool>,
        ) -> bool {
            let id = id_of(node);
            let own = spec.matches(node, query);
            self_matches.insert(id.clone(), own);
            let mut any = own;
            if has_children
                .map(|predicate| predicate(node))
                .unwrap_or(true)
            {
                for child in children(node) {
                    if walk(
                        &child,
                        id_of,
                        children,
                        has_children,
                        spec,
                        query,
                        self_matches,
                        subtree_matches,
                    ) {
                        any = true;
                    }
                }
            }
            subtree_matches.insert(id, any);
            any
        }
        for root in roots {
            walk(
                root,
                id_of,
                children,
                has_children,
                spec,
                &query,
                &mut self_matches,
                &mut subtree_matches,
            );
        }
        TreeSearchMetadata {
            self_matches,
            subtree_matches,
        }
    });

    fn append<Element, Id>(
        nodes: &[Element],
        depth: usize,
        path_prefix: &[usize],
        ancestor_has_next_siblings: &[bool],
        parent_id: Option<Id>,
        parent_key: Option<&TreeNodeKey<Id>>,
        id_of: &dyn Fn(&Element) -> Id,
        children: &dyn Fn(&Element) -> Vec<Element>,
        has_children: Option<&dyn Fn(&Element) -> bool>,
        expanded: &HashSet<Id>,
        expanded_keys: &HashSet<TreeNodeKey<Id>>,
        metadata: Option<&TreeSearchMetadata<Id>>,
        filter_active: bool,
        seen: &mut Option<&mut HashSet<Id>>,
        out: &mut Vec<TreeEntry<Element, Id>>,
    ) where
        Element: Clone,
        Id: std::hash::Hash + Eq + Clone,
    {
        let visible: Vec<(usize, Element)> = nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| {
                if !filter_active {
                    return Some((index, node.clone()));
                }
                let id = id_of(node);
                let matches = metadata
                    .and_then(|metadata| metadata.subtree_matches.get(&id))
                    .copied()
                    .unwrap_or(false);
                matches.then_some((index, node.clone()))
            })
            .collect();

        for (visible_index, (original_index, node)) in visible.iter().enumerate() {
            let id = id_of(node);
            if let Some(seen) = seen.as_mut()
                && !seen.insert(id.clone())
            {
                continue;
            }
            let mut path = path_prefix.to_vec();
            path.push(*original_index);
            let key = TreeNodeKey::new(id.clone(), path.clone());
            let explicitly_expanded = expanded.contains(&id) || expanded_keys.contains(&key);
            let branch = has_children.map(|predicate| predicate(node));
            let child_nodes = if branch == Some(false) {
                Vec::new()
            } else {
                children(node)
            };
            // Filtering auto-expands the ancestors of every match, so a hit
            // deep in a collapsed branch is reachable without the caller
            // pre-expanding the path.
            let child_subtree_matches = filter_active
                && child_nodes.iter().any(|child| {
                    metadata
                        .and_then(|metadata| metadata.subtree_matches.get(&id_of(child)))
                        .copied()
                        .unwrap_or(false)
                });
            let is_expanded = explicitly_expanded || child_subtree_matches;
            let has_next_sibling = visible_index < visible.len() - 1;
            let self_matches = metadata
                .and_then(|metadata| metadata.self_matches.get(&id))
                .copied()
                .unwrap_or(false);
            let is_leaf = match branch {
                Some(value) => !value,
                None => child_nodes.is_empty(),
            };
            out.push(TreeEntry {
                element: node.clone(),
                id: id.clone(),
                key: key.clone(),
                depth,
                parent_id: parent_id.clone(),
                parent_key: parent_key.cloned(),
                ancestor_has_next_siblings: ancestor_has_next_siblings.to_vec(),
                has_next_sibling,
                is_search_hit: self_matches,
                has_children: branch.unwrap_or(!child_nodes.is_empty()),
                is_leaf,
                is_expanded,
                is_checked: false,
                is_half_checked: false,
                is_loading: false,
            });
            if is_expanded && !child_nodes.is_empty() {
                let mut child_guide = ancestor_has_next_siblings.to_vec();
                // Guide columns map to depth>=1 path siblings. Root-level
                // sibling state has no dedicated guide column and would shift
                // descendant columns by one, creating depth-2+ breaks.
                if depth > 0 {
                    child_guide.push(has_next_sibling);
                }
                append(
                    &child_nodes,
                    depth + 1,
                    &path,
                    &child_guide,
                    Some(id),
                    Some(&key),
                    id_of,
                    children,
                    has_children,
                    expanded,
                    expanded_keys,
                    metadata,
                    filter_active,
                    seen,
                    out,
                );
            }
        }
    }

    let mut out = Vec::new();
    append(
        roots,
        0,
        &[],
        &[],
        None,
        None,
        id_of,
        children,
        has_children,
        expanded,
        expanded_keys,
        metadata.as_ref(),
        filter_active,
        &mut seen,
        &mut out,
    );
    out
}

/// Hierarchical tree with selection, check cascade, filtering, lazy loading,
/// drag and drop and virtual or eager scrolling.
pub struct Tree<Element, Id, RowContent>
where
    Element: Clone + 'static,
    Id: CollectionId,
    RowContent: Fn(&Element, RowState) -> AnyView + 'static,
{
    pub roots: Vec<Element>,
    pub options: TreeOptions<Element, Id>,
    pub row_content: RowContent,
    id_of: Rc<dyn Fn(&Element) -> Id>,
    children: Rc<dyn Fn(&Element) -> Vec<Element>>,
    has_children: Option<Rc<dyn Fn(&Element) -> bool>>,
    anchor: State<Option<TreeNodeKey<Id>>>,
    cursor: State<Option<TreeNodeKey<Id>>>,
    local_expanded: State<HashSet<Id>>,
    local_expanded_keys: State<HashSet<TreeNodeKey<Id>>>,
    local_checked: State<HashSet<Id>>,
    local_checked_keys: State<HashSet<TreeNodeKey<Id>>>,
    local_loaded_keys: State<HashSet<TreeNodeKey<Id>>>,
    local_selected: State<Option<Id>>,
    local_multi_selected: State<BTreeSet<Id>>,
    local_query: State<String>,
    scroll_request: State<Option<(TreeNodeKey<Id>, TreeScrollAlign)>>,
    hovered: State<Option<TreeNodeKey<Id>>>,
    modifiers: RefCell<KeyModifiers>,
    drag_source: State<Option<TreeNodeKey<Id>>>,
    drag_target: State<Option<(TreeNodeKey<Id>, TreeDropPosition)>>,
    drag_registry: Rc<TreeDragRegistry<Id>>,
}

impl<Element, Id, RowContent> Tree<Element, Id, RowContent>
where
    Element: Clone + 'static,
    Id: CollectionId,
    RowContent: Fn(&Element, RowState) -> AnyView + 'static,
{
    pub fn new(
        roots: Vec<Element>,
        id_of: impl Fn(&Element) -> Id + 'static,
        children: impl Fn(&Element) -> Vec<Element> + 'static,
        row_content: RowContent,
    ) -> Self {
        Self {
            roots,
            options: TreeOptions::default(),
            row_content,
            id_of: Rc::new(id_of),
            children: Rc::new(children),
            has_children: None,
            anchor: State::new(None),
            cursor: State::new(None),
            local_expanded: State::new(HashSet::new()),
            local_expanded_keys: State::new(HashSet::new()),
            local_checked: State::new(HashSet::new()),
            local_checked_keys: State::new(HashSet::new()),
            local_loaded_keys: State::new(HashSet::new()),
            local_selected: State::new(None),
            local_multi_selected: State::new(BTreeSet::new()),
            local_query: State::new(String::new()),
            scroll_request: State::new(None),
            hovered: State::new(None),
            modifiers: RefCell::new(KeyModifiers(0)),
            drag_source: State::new(None),
            drag_target: State::new(None),
            drag_registry: Rc::new(TreeDragRegistry::default()),
        }
    }

    pub fn options(mut self, options: TreeOptions<Element, Id>) -> Self {
        self.options = options;
        self.options.layout.validate();
        self.seed_local_state();
        if self.options.expansion.default_expand_all
            && self.options.selection.expanded.is_none()
            && self.options.selection.expanded_keys.is_none()
        {
            self.expand_all();
        }
        self
    }

    /// Seeds the tree-owned fallbacks the first time options are applied.
    fn seed_local_state(&self) {
        let selection = &self.options.selection;
        if selection.is_local {
            if selection.local_multiple {
                self.local_multi_selected
                    .set(selection.default_selected.iter().cloned().collect());
            } else {
                self.local_selected
                    .set(selection.default_selected.first().cloned());
            }
        }
        if selection.expanded.is_none() && selection.expanded_keys.is_none() {
            if !selection.default_expanded.is_empty() {
                self.local_expanded
                    .set(selection.default_expanded.iter().cloned().collect());
            }
            if !selection.default_expanded_keys.is_empty() {
                self.local_expanded_keys
                    .set(selection.default_expanded_keys.iter().cloned().collect());
            }
        }
        let check = &self.options.check;
        if check.is_enabled && check.checked.is_none() && check.checked_keys.is_none() {
            self.local_checked
                .set(check.default_checked.iter().cloned().collect());
        }
    }

    /// A cheap branch predicate, so a synchronous provider can defer child
    /// construction until expansion.
    pub fn has_children(mut self, has_children: impl Fn(&Element) -> bool + 'static) -> Self {
        self.has_children = Some(Rc::new(has_children));
        self
    }

    /// Explicit leaf marker for a lazy tree; overrides [`Tree::has_children`].
    pub fn is_leaf(mut self, is_leaf: impl Fn(&Element) -> bool + 'static) -> Self {
        self.options.lazy.is_leaf = Some(Rc::new(is_leaf));
        self
    }

    fn branch_predicate(&self) -> Option<Rc<dyn Fn(&Element) -> bool>> {
        if let Some(is_leaf) = &self.options.lazy.is_leaf {
            let is_leaf = is_leaf.clone();
            return Some(Rc::new(move |element: &Element| !is_leaf(element)));
        }
        self.has_children.clone()
    }

    fn full_index(&self) -> TreeIndex<Element, Id> {
        let predicate = self.branch_predicate();
        TreeIndex::build(
            &self.roots,
            &*self.id_of,
            &*self.children,
            predicate.as_deref(),
        )
    }

    fn expanded_ids(&self) -> HashSet<Id> {
        match &self.options.selection.expanded {
            Some(binding) => binding.get(),
            None => self.local_expanded.get(),
        }
    }

    fn expanded_keys(&self) -> HashSet<TreeNodeKey<Id>> {
        match &self.options.selection.expanded_keys {
            Some(binding) => binding.get(),
            None => self.local_expanded_keys.get(),
        }
    }

    /// Expansion keys including those derived from an ID-only binding.
    fn expanded_key_snapshot(&self) -> HashSet<TreeNodeKey<Id>> {
        let mut keys = self.expanded_keys();
        let ids = self.expanded_ids();
        if ids.is_empty() {
            return keys;
        }
        for node in &self.full_index().nodes {
            if ids.contains(&node.id) {
                keys.insert(node.key.clone());
            }
        }
        keys
    }

    fn set_expanded(&self, ids: HashSet<Id>, keys: HashSet<TreeNodeKey<Id>>) {
        let use_local = self.options.selection.expanded.is_none()
            && self.options.selection.expanded_keys.is_none();
        if let Some(binding) = &self.options.selection.expanded_keys {
            binding.set(keys.clone());
        } else if use_local {
            self.local_expanded_keys.set(keys);
        }
        if let Some(binding) = &self.options.selection.expanded {
            binding.set(ids);
        } else if use_local {
            self.local_expanded.set(ids);
        }
    }

    fn checked_state(&self) -> (HashSet<Id>, HashSet<TreeNodeKey<Id>>) {
        let check = &self.options.check;
        if let Some(binding) = &check.checked_keys {
            let keys = binding.get();
            let ids = keys.iter().map(|key| key.id.clone()).collect();
            return (ids, keys);
        }
        if let Some(binding) = &check.checked {
            return (binding.get(), HashSet::new());
        }
        (self.local_checked.get(), self.local_checked_keys.get())
    }

    fn set_checked_state(&self, ids: HashSet<Id>, keys: HashSet<TreeNodeKey<Id>>) {
        let check = &self.options.check;
        if let Some(binding) = &check.checked_keys {
            binding.set(keys.clone());
        } else {
            self.local_checked_keys.set(keys.clone());
        }
        if let Some(binding) = &check.checked {
            binding.set(ids.clone());
        } else {
            self.local_checked.set(ids);
        }
    }

    fn check_snapshot(&self) -> CheckSnapshot<Element, Id> {
        if !self.options.check.is_enabled {
            return CheckSnapshot::default();
        }
        let index = self.full_index();
        let (ids, keys) = self.checked_state();
        check::snapshot(&index, &ids, &keys, self.options.check.check_strictly)
    }

    fn loaded_keys(&self) -> HashSet<TreeNodeKey<Id>> {
        match &self.options.lazy.loaded_keys {
            Some(binding) => binding.get(),
            None => self.local_loaded_keys.get(),
        }
    }

    /// The active query: a programmatic [`Tree::filter`] overrides the option
    /// string until it is cleared.
    fn effective_query(&self) -> String {
        let local = self.local_query.get();
        if local.trim().is_empty() {
            self.options.search.query.clone()
        } else {
            local
        }
    }

    /// The visible projection for the current expansion and search state.
    pub fn entries(&self) -> Vec<TreeEntry<Element, Id>> {
        let expanded = self.expanded_ids();
        let keys = self.expanded_keys();
        let query = self.effective_query();
        let search = &self.options.search;
        let text: Option<&dyn Fn(&Element) -> String> = search.text.as_deref();
        let filter: Option<&dyn Fn(&Element, &str) -> bool> = search.filter.as_deref();
        let has_matcher = text.is_some() || filter.is_some();
        let spec = has_matcher.then_some(SearchSpec {
            query: query.as_str(),
            text,
            filter,
            ignore_case: search.ignore_case,
            match_mode: search.match_mode,
        });
        let predicate = self.branch_predicate();
        let mut entries = flatten_visible(
            &self.roots,
            &*self.id_of,
            &*self.children,
            predicate.as_deref(),
            &expanded,
            &keys,
            spec,
            search.policy == TreeSearchFilterPolicy::FilterAndAutoExpand,
            None,
        );

        if self.options.check.is_enabled {
            let snapshot = self.check_snapshot();
            for entry in &mut entries {
                entry.is_checked = snapshot.checked_keys.contains(&entry.key)
                    || snapshot.checked_ids.contains(&entry.id);
                entry.is_half_checked = snapshot.half_checked_keys.contains(&entry.key);
            }
        }
        if let Some(is_loading) = &self.options.lazy.is_loading {
            for entry in &mut entries {
                entry.is_loading = is_loading(&entry.element);
            }
        }
        entries
    }

    fn selection_state(&self) -> (HashSet<Id>, HashSet<TreeNodeKey<Id>>) {
        let selection: &TreeSelection<Id> = &self.options.selection;
        if selection.is_local {
            if selection.local_multiple {
                return (
                    self.local_multi_selected.get().into_iter().collect(),
                    HashSet::new(),
                );
            }
            return (
                self.local_selected.get().into_iter().collect(),
                HashSet::new(),
            );
        }
        if let Some(keys) = &selection.keys {
            let keys: HashSet<TreeNodeKey<Id>> = keys.ids().into_iter().collect();
            let ids = keys.iter().map(|key| key.id.clone()).collect();
            return (ids, keys);
        }
        (selection.ids.ids().into_iter().collect(), HashSet::new())
    }

    fn uses_key_selection(&self) -> bool {
        self.options.selection.keys.is_some()
    }

    /// The row actions, cloned so row closures do not borrow `self`.
    fn session_actions(&self) -> TreeActions<Element, Id> {
        self.session_actions_with(self.entries())
    }

    /// [`Tree::session_actions`] reusing a projection the caller already
    /// computed, so the body does not walk the tree twice.
    fn session_actions_with(
        &self,
        entries: Vec<TreeEntry<Element, Id>>,
    ) -> TreeActions<Element, Id> {
        TreeActions {
            entries: Rc::new(RefCell::new(entries)),
            expanded: self.local_expanded.clone(),
            local_expanded_keys: self.local_expanded_keys.clone(),
            expanded_keys: self.options.selection.expanded_keys.clone(),
            selection: self.options.selection.clone(),
            check: self.options.check.clone(),
            expansion: self.options.expansion,
            lazy: self.options.lazy.clone(),
            edit: self.options.edit.clone(),
            anchor: self.anchor.clone(),
            cursor: self.cursor.clone(),
            local_checked: self.local_checked.clone(),
            local_checked_keys: self.local_checked_keys.clone(),
            local_loaded_keys: self.local_loaded_keys.clone(),
            local_selected: self.local_selected.clone(),
            local_multi_selected: self.local_multi_selected.clone(),
            scroll_request: self.scroll_request.clone(),
            events: TreeEvents {
                on_key_command: self.options.events.on_key_command.clone(),
                on_select: self.options.events.on_select.clone(),
                on_select_change: self.options.events.on_select_change.clone(),
                on_expand_change: self.options.events.on_expand_change.clone(),
                on_node_click: self.options.events.on_node_click.clone(),
                on_node_context_menu: self.options.events.on_node_context_menu.clone(),
                on_double_click: self.options.events.on_double_click.clone(),
            },
            modifiers: Rc::new(RefCell::new(*self.modifiers.borrow())),
            roots: Rc::new(self.roots.clone()),
            id_of: self.id_of.clone(),
            children: self.children.clone(),
            has_children: self.branch_predicate(),
            drag: self.options.drag.clone(),
            drag_source: self.drag_source.clone(),
            drag_target: self.drag_target.clone(),
        }
    }

    /// Applies one selection with Shift/Cmd/Ctrl semantics.
    pub fn select(&self, entry: &TreeEntry<Element, Id>, modifiers: KeyModifiers) {
        *self.modifiers.borrow_mut() = modifiers;
        self.session_actions().select(entry, modifiers);
    }

    pub fn collapse_all(&self) {
        self.set_expanded(HashSet::new(), HashSet::new());
    }

    /// Expands every node that has children.
    pub fn expand_all(&self) {
        let mut keys: HashSet<TreeNodeKey<Id>> = HashSet::new();
        fn visit<Element, Id: std::hash::Hash + Eq + Clone>(
            nodes: &[Element],
            path: &[usize],
            id_of: &dyn Fn(&Element) -> Id,
            children: &dyn Fn(&Element) -> Vec<Element>,
            has_children: Option<&dyn Fn(&Element) -> bool>,
            keys: &mut HashSet<TreeNodeKey<Id>>,
        ) {
            for (index, element) in nodes.iter().enumerate() {
                if has_children.map(|predicate| predicate(element)) == Some(false) {
                    continue;
                }
                let mut next_path = path.to_vec();
                next_path.push(index);
                let kids = children(element);
                if kids.is_empty() {
                    continue;
                }
                keys.insert(TreeNodeKey::new(id_of(element), next_path.clone()));
                visit(&kids, &next_path, id_of, children, has_children, keys);
            }
        }
        let predicate = self.branch_predicate();
        visit(
            &self.roots,
            &[],
            &*self.id_of,
            &*self.children,
            predicate.as_deref(),
            &mut keys,
        );
        let ids: HashSet<Id> = keys.iter().map(|key| key.id.clone()).collect();
        self.set_expanded(ids, keys);
    }

    /// Programmatic filter, the equivalent of `filter(value)` in the reference
    /// APIs. It overrides the option query until [`Tree::clear_filter`].
    pub fn filter(&self, query: impl Into<String>) {
        self.local_query.set(query.into());
    }

    pub fn clear_filter(&self) {
        self.local_query.set(String::new());
    }

    pub fn get_expanded_keys(&self) -> HashSet<TreeNodeKey<Id>> {
        self.expanded_key_snapshot()
    }

    pub fn get_expanded_ids(&self) -> HashSet<Id> {
        self.expanded_ids()
    }

    /// The selected rows as path keys. With an ID-only selection this resolves
    /// the visible occurrences.
    pub fn get_selected_keys(&self) -> HashSet<TreeNodeKey<Id>> {
        let (ids, keys) = self.selection_state();
        if !keys.is_empty() {
            return keys;
        }
        self.entries()
            .into_iter()
            .filter(|entry| ids.contains(&entry.id))
            .map(|entry| entry.key)
            .collect()
    }

    pub fn get_selected_ids(&self) -> HashSet<Id> {
        self.selection_state().0
    }

    /// Opens one visible branch.
    pub fn expand_key(&self, key: &TreeNodeKey<Id>) {
        if let Some(entry) = self.entries().into_iter().find(|entry| &entry.key == key)
            && entry.has_children
            && !entry.is_expanded
        {
            self.session_actions().toggle(&entry);
        }
    }

    /// Closes one visible branch.
    pub fn collapse_key(&self, key: &TreeNodeKey<Id>) {
        if let Some(entry) = self.entries().into_iter().find(|entry| &entry.key == key)
            && entry.is_expanded
        {
            self.session_actions().toggle(&entry);
        }
    }

    /// Toggles one visible branch.
    pub fn toggle_key(&self, key: &TreeNodeKey<Id>) {
        if let Some(entry) = self.entries().into_iter().find(|entry| &entry.key == key) {
            self.session_actions().toggle(&entry);
        }
    }

    pub fn get_checked_keys(&self) -> HashSet<TreeNodeKey<Id>> {
        self.check_snapshot().checked_keys
    }

    pub fn get_half_checked_keys(&self) -> HashSet<TreeNodeKey<Id>> {
        self.check_snapshot().half_checked_keys
    }

    pub fn get_checked_nodes(&self) -> Vec<Element> {
        self.check_snapshot().checked_elements
    }

    /// Checked nodes with the reference API's `leafOnly` / `includeHalfChecked`
    /// filters.
    pub fn get_checked_nodes_filtered(
        &self,
        leaf_only: bool,
        include_half_checked: bool,
    ) -> Vec<Element> {
        if !self.options.check.is_enabled {
            return Vec::new();
        }
        let index = self.full_index();
        let (ids, keys) = self.checked_state();
        let snapshot = check::snapshot(&index, &ids, &keys, self.options.check.check_strictly);
        index
            .nodes
            .iter()
            .filter(|node| {
                let checked = snapshot.checked_keys.contains(&node.key)
                    || snapshot.checked_ids.contains(&node.id);
                let half = snapshot.half_checked_keys.contains(&node.key);
                (checked || (include_half_checked && half)) && (!leaf_only || node.is_leaf)
            })
            .map(|node| node.element.clone())
            .collect()
    }

    pub fn get_loaded_keys(&self) -> HashSet<TreeNodeKey<Id>> {
        self.loaded_keys()
    }

    pub fn is_expanded(&self, key: &TreeNodeKey<Id>) -> bool {
        self.expanded_ids().contains(&key.id) || self.expanded_keys().contains(key)
    }

    pub fn is_checked(&self, key: &TreeNodeKey<Id>) -> bool {
        let snapshot = self.check_snapshot();
        snapshot.checked_keys.contains(key) || snapshot.checked_ids.contains(&key.id)
    }

    pub fn is_half_checked(&self, key: &TreeNodeKey<Id>) -> bool {
        self.check_snapshot().half_checked_keys.contains(key)
    }

    /// Replaces the check state. `leaf_only` keeps only leaf keys, matching
    /// `getCheckedKeys(leafOnly)`.
    pub fn set_checked_keys(
        &self,
        keys: impl IntoIterator<Item = TreeNodeKey<Id>>,
        leaf_only: bool,
    ) {
        if !self.options.check.is_enabled {
            return;
        }
        let index = self.full_index();
        let keys: HashSet<TreeNodeKey<Id>> = keys.into_iter().collect();
        let ids: HashSet<Id> = keys.iter().map(|key| key.id.clone()).collect();
        let (ids, keys) = check::canonical_checked(
            &index,
            ids,
            keys,
            self.options.check.check_strictly,
            leaf_only,
        );
        self.set_checked_state(ids, keys);
    }

    /// Sets one node's check state. `deep` overrides the cascade for this call:
    /// `Some(false)` is the `checkStrictly` behaviour, `None` follows options.
    pub fn set_checked(&self, key: &TreeNodeKey<Id>, checked: bool, deep: Option<bool>) {
        if !self.options.check.is_enabled {
            return;
        }
        let index = self.full_index();
        let Some(node) = index.node(key) else {
            return;
        };
        let element = node.element.clone();
        self.apply_checked(&index, key, &element, checked, deep);
    }

    pub fn set_checked_element(&self, element: &Element, checked: bool, deep: Option<bool>) {
        if !self.options.check.is_enabled {
            return;
        }
        let index = self.full_index();
        let id = (self.id_of)(element);
        let Some(node) = index.node_by_id(&id) else {
            return;
        };
        let key = node.key.clone();
        self.apply_checked(&index, &key, element, checked, deep);
    }

    fn apply_checked(
        &self,
        index: &TreeIndex<Element, Id>,
        key: &TreeNodeKey<Id>,
        element: &Element,
        checked: bool,
        deep: Option<bool>,
    ) {
        let strict = deep.unwrap_or(self.options.check.check_strictly);
        let (ids, keys) = self.checked_state();
        let (ids, keys) = check::toggle(index, &ids, &keys, key, checked, strict);
        self.set_checked_state(ids, keys);
        if let Some(on_change) = &self.options.check.on_change {
            let (ids, keys) = self.checked_state();
            let snapshot = check::snapshot(index, &ids, &keys, strict);
            on_change(
                element,
                checked,
                &snapshot.checked_keys,
                &snapshot.half_checked_keys,
            );
        }
    }

    /// Checks every node, branches included.
    pub fn check_all(&self) {
        if !self.options.check.is_enabled {
            return;
        }
        let index = self.full_index();
        let ids = index.nodes.iter().map(|node| node.id.clone()).collect();
        let keys = index.nodes.iter().map(|node| node.key.clone()).collect();
        self.set_checked_state(ids, keys);
    }

    pub fn uncheck_all(&self) {
        self.set_checked_state(HashSet::new(), HashSet::new());
    }

    /// The current keyboard/selection row.
    pub fn get_current_key(&self) -> Option<TreeNodeKey<Id>> {
        self.cursor
            .get()
            .or_else(|| self.get_selected_keys().into_iter().next())
    }

    pub fn get_current_node(&self) -> Option<Element> {
        let key = self.get_current_key()?;
        self.full_index()
            .node(&key)
            .map(|node| node.element.clone())
    }

    /// Moves the current row. With `auto_expand_parent` its ancestors are
    /// revealed before the cursor moves.
    pub fn set_current_key(&self, key: Option<TreeNodeKey<Id>>) {
        self.cursor.set(key.clone());
        let Some(key) = key else { return };
        if self.options.expansion.auto_expand_parent {
            self.expand_to(&key);
        }
        if self.options.selection.is_selectable
            && let Some(entry) = self.entries().into_iter().find(|entry| entry.key == key)
        {
            self.select(&entry, KeyModifiers(0));
        }
    }

    /// Root → node elements.
    pub fn get_path(&self, key: &TreeNodeKey<Id>) -> Vec<Element> {
        self.full_index().path(key)
    }

    /// Root → node keys.
    pub fn get_key_path(&self, key: &TreeNodeKey<Id>) -> Vec<TreeNodeKey<Id>> {
        let index = self.full_index();
        let mut slot = index.by_key.get(key).copied();
        let mut out = Vec::new();
        while let Some(current) = slot {
            out.push(index.nodes[current].key.clone());
            slot = index.nodes[current].parent;
        }
        out.reverse();
        out
    }

    /// Expands every ancestor of `key`, including its own branch.
    pub fn expand_to(&self, key: &TreeNodeKey<Id>) {
        let index = self.full_index();
        let Some(node) = index.node(key) else { return };
        let mut ids = self.expanded_ids();
        let mut keys = self.expanded_keys();
        for ancestor in index.ancestor_keys(key) {
            keys.insert(ancestor.clone());
            ids.insert(ancestor.id);
        }
        if !node.children.is_empty() {
            keys.insert(node.key.clone());
            ids.insert(node.id.clone());
        }
        self.set_expanded(ids, keys);
    }

    /// Releases the next projection at the row for `key`, expanding its path
    /// first so a collapsed target is reachable.
    pub fn scroll_to_key(&self, key: TreeNodeKey<Id>) {
        self.scroll_to_key_aligned(key, TreeScrollAlign::Auto);
    }

    /// [`Tree::scroll_to_key`] with an explicit alignment.
    pub fn scroll_to_key_aligned(&self, key: TreeNodeKey<Id>, align: TreeScrollAlign) {
        self.expand_to(&key);
        self.scroll_request.set(Some((key, align)));
    }

    pub fn clear_current_key(&self) {
        self.cursor.set(None);
    }

    /// Delegated structural edits. A missing callback makes these no-ops.
    pub fn append_at(&self, parent: Option<&TreeNodeKey<Id>>, data: Element) {
        let index = self.full_index();
        let parent_element = parent
            .and_then(|key| index.node(key))
            .map(|node| node.element.clone());
        if let Some(callback) = &self.options.edit.on_append {
            callback(parent_element.as_ref(), &data);
        }
    }

    pub fn remove_key(&self, key: &TreeNodeKey<Id>) {
        let index = self.full_index();
        if let Some(node) = index.node(key)
            && let Some(callback) = &self.options.edit.on_remove
        {
            callback(&node.element);
        }
    }

    pub fn update_children_at(&self, key: &TreeNodeKey<Id>, children: Vec<Element>) {
        let index = self.full_index();
        if let Some(node) = index.node(key)
            && let Some(callback) = &self.options.edit.on_update_children
        {
            callback(&node.element, children);
        }
    }

    pub fn append(&self, data: Element, parent: Option<&Element>) {
        if let Some(callback) = &self.options.edit.on_append {
            callback(parent, &data);
        }
    }

    pub fn insert_before(&self, reference: &Element, data: Element) {
        if let Some(callback) = &self.options.edit.on_insert_before {
            callback(reference, &data);
        }
    }

    pub fn insert_after(&self, reference: &Element, data: Element) {
        if let Some(callback) = &self.options.edit.on_insert_after {
            callback(reference, &data);
        }
    }

    pub fn remove(&self, element: &Element) {
        if let Some(callback) = &self.options.edit.on_remove {
            callback(element);
        }
    }

    pub fn update_children(&self, parent: &Element, children: Vec<Element>) {
        if let Some(callback) = &self.options.edit.on_update_children {
            callback(parent, children);
        }
    }

    /// The tree's keyboard behaviour. Returns whether the event was consumed.
    pub fn handle_key(&self, event: &KeyEvent) -> bool {
        self.session_actions().handle_key(event)
    }
}

fn keys_between<Element, Id>(
    a: &TreeNodeKey<Id>,
    b: &TreeNodeKey<Id>,
    entries: &[TreeEntry<Element, Id>],
) -> HashSet<TreeNodeKey<Id>>
where
    Element: Clone,
    Id: std::hash::Hash + Eq + Clone,
{
    let Some(ia) = entries.iter().position(|entry| &entry.key == a) else {
        return HashSet::new();
    };
    let Some(ib) = entries.iter().position(|entry| &entry.key == b) else {
        return HashSet::new();
    };
    let (lower, upper) = if ia <= ib { (ia, ib) } else { (ib, ia) };
    entries[lower..=upper]
        .iter()
        .map(|entry| entry.key.clone())
        .collect()
}

impl<Element, Id, RowContent> View for Tree<Element, Id, RowContent>
where
    Element: Clone + 'static,
    Id: CollectionId,
    RowContent: Fn(&Element, RowState) -> AnyView + 'static,
{
    fn body(&self, _cx: &Scope) -> AnyView {
        let entries = self.entries();
        let (selected_ids, selected_keys) = self.selection_state();
        let uses_key_selection = self.uses_key_selection();
        let layout = self.options.layout;
        let is_enabled = self.options.is_enabled;
        let hovered = self.hovered.clone();
        let drag_source = self.drag_source.get();
        let drag_target = self.drag_target.get();
        let cursor = self.cursor.get();
        let actions = self.session_actions_with(entries.clone());
        let check_enabled = self.options.check.is_enabled;

        let trailing_width = layout.trailing_slot_width;
        let rows: Vec<AnyView> = entries
            .iter()
            .map(|entry| {
                let is_selected = if uses_key_selection {
                    selected_keys.contains(&entry.key)
                } else {
                    selected_ids.contains(&entry.id)
                };
                let key = entry.key.clone();
                let has_next_sibling = entry.has_next_sibling;
                let ancestor = entry.ancestor_has_next_siblings.clone();
                let is_current = cursor.as_ref() == Some(&key);
                let is_hovered = hovered.get().as_ref() == Some(&entry.key);
                let content = (self.row_content)(
                    &entry.element,
                    RowState {
                        is_selected,
                        is_expanded: entry.is_expanded,
                        is_dragging: drag_source.as_ref() == Some(&key),
                        depth: entry.depth,
                        is_checked: entry.is_checked,
                        is_half_checked: entry.is_half_checked,
                        is_loading: entry.is_loading,
                        is_current,
                        is_leaf: entry.is_leaf,
                    },
                );

                let drop_position = drag_target
                    .as_ref()
                    .filter(|(target, _)| target == &key)
                    .map(|(_, position)| *position);
                let is_drag_source = drag_source.as_ref() == Some(&key);

                let on_toggle: Rc<dyn Fn()> = {
                    let entry = entry.clone();
                    let tree = actions.clone();
                    Rc::new(move || tree.toggle(&entry))
                };
                let on_select: Rc<dyn Fn(KeyModifiers)> = {
                    let entry = entry.clone();
                    let tree = actions.clone();
                    Rc::new(move |modifiers| tree.select(&entry, modifiers))
                };
                let on_move: Rc<dyn Fn(i64)> = {
                    let entry = entry.clone();
                    let tree = actions.clone();
                    Rc::new(move |delta| tree.step(&entry, delta))
                };
                let on_collapse: Rc<dyn Fn()> = {
                    let entry = entry.clone();
                    let tree = actions.clone();
                    Rc::new(move || tree.collapse(&entry))
                };
                let on_expand: Rc<dyn Fn()> = {
                    let entry = entry.clone();
                    let tree = actions.clone();
                    Rc::new(move || tree.expand(&entry))
                };
                let on_toggle_check: Option<Rc<dyn Fn()>> = check_enabled.then(|| {
                    let entry = entry.clone();
                    let tree = actions.clone();
                    let toggle: Rc<dyn Fn()> = Rc::new(move || tree.toggle_checked(&entry));
                    toggle
                });
                let on_node_click: Option<Rc<dyn Fn(KeyModifiers)>> =
                    self.options.events.on_node_click.as_ref().map(|handler| {
                        let handler = handler.clone();
                        let element = entry.element.clone();
                        let closure: Rc<dyn Fn(KeyModifiers)> =
                            Rc::new(move |modifiers: KeyModifiers| handler(&element, modifiers));
                        closure
                    });
                let on_context_menu: Option<Rc<dyn Fn(Point)>> = self
                    .options
                    .events
                    .on_node_context_menu
                    .as_ref()
                    .map(|handler| {
                        let handler = handler.clone();
                        let element = entry.element.clone();
                        let closure: Rc<dyn Fn(Point)> =
                            Rc::new(move |point: Point| handler(&element, point));
                        closure
                    });
                let on_double_click: Option<Rc<dyn Fn()>> =
                    self.options.events.on_double_click.as_ref().map(|handler| {
                        let handler = handler.clone();
                        let element = entry.element.clone();
                        let closure: Rc<dyn Fn()> = Rc::new(move || handler(&element));
                        closure
                    });
                let drag_enabled = self.options.drag.is_enabled
                    && (self.options.drag.on_drop.is_some()
                        || self.options.drag.on_drag_start.is_some()
                        || self.options.drag.on_drag_enter.is_some()
                        || self.options.drag.on_drag_end.is_some());
                let can_drag = self
                    .options
                    .drag
                    .can_drag
                    .as_ref()
                    .is_none_or(|allowed| allowed(&entry.element));
                let row_drag = (drag_enabled && can_drag).then(|| {
                    let tree = actions.clone();
                    let registry = self.drag_registry.clone();
                    let entry = entry.clone();
                    TreeRowDrag {
                        register: {
                            let key = key.clone();
                            let registry = registry.clone();
                            Rc::new(move |node: &Node| registry.register(node, key.clone()))
                        },
                        on_start: {
                            let entry = entry.clone();
                            let tree = tree.clone();
                            Rc::new(move || tree.drag_start(&entry))
                        },
                        on_move: {
                            let key = key.clone();
                            let registry = registry.clone();
                            let tree = tree.clone();
                            Rc::new(move |x: f32, y: f32| tree.update_drag(&key, x, y, &registry))
                        },
                        on_end: {
                            let tree = tree.clone();
                            Rc::new(move || tree.commit_drag(&entry))
                        },
                    }
                });
                let on_hover: Rc<dyn Fn(bool)> = {
                    let hovered = hovered.clone();
                    let key = key.clone();
                    Rc::new(move |entered| {
                        if entered {
                            hovered.set(Some(key.clone()));
                        } else if hovered.get().as_ref() == Some(&key) {
                            hovered.set(None);
                        }
                    })
                };
                let trailing: Option<AnyView> = self.options.slots.trailing.as_ref().map(|slot| {
                    AnyView::new(AnyView::new(
                        AnyView::new(
                            AnyView::new(slot(
                                entry.element.clone(),
                                is_selected,
                                entry.is_expanded,
                                entry.is_search_hit,
                                is_hovered,
                                entry.depth,
                            ))
                            .padding_edges(4.0, 0.0)
                            .frame_width(trailing_width),
                        )
                        .clipped(),
                    ))
                });
                let icon: Option<AnyView> = self.options.slots.icon.as_ref().map(|slot| {
                    AnyView::new(slot(&entry.element).frame_size(16.0, layout.row_height))
                });

                let checkbox: Option<AnyView> = check_enabled.then(|| {
                    let state = if entry.is_checked {
                        CheckboxState::On
                    } else if entry.is_half_checked {
                        CheckboxState::Mixed
                    } else {
                        CheckboxState::Off
                    };
                    let tree = actions.clone();
                    let entry = entry.clone();
                    let key = key.clone();
                    let binding = Binding::new(
                        move || state,
                        move |value: CheckboxState| {
                            tree.apply_checked(
                                &key,
                                &entry.element,
                                value == CheckboxState::On,
                                None,
                            );
                        },
                    );
                    AnyView::new(
                        AnyView::new(Checkbox::new(binding).enabled(is_enabled))
                            .frame_size(16.0, 16.0),
                    )
                });

                AnyView::new(TreeRow {
                    is_enabled,
                    has_children: entry.has_children,
                    is_expanded: entry.is_expanded,
                    is_search_hit: entry.is_search_hit,
                    is_selected,
                    is_hovered,
                    is_checked: entry.is_checked,
                    is_half_checked: entry.is_half_checked,
                    is_loading: entry.is_loading,
                    is_current,
                    width_filling: layout.block_node,
                    depth: entry.depth,
                    row_height: layout.row_height,
                    indentation: layout.indentation,
                    disclosure_width: layout.disclosure_width,
                    shows_indent_guides: layout.shows_indent_guides,
                    ancestor_has_next_siblings: ancestor,
                    has_next_sibling,
                    drop_position,
                    is_drag_source,
                    content,
                    checkbox,
                    icon,
                    disclosure_slot: self.options.slots.disclosure.clone(),
                    trailing_slot: trailing,
                    drag: row_drag,
                    on_toggle,
                    on_select,
                    on_move,
                    on_collapse,
                    on_expand,
                    on_toggle_check,
                    on_hover,
                    on_node_click,
                    on_context_menu,
                    on_double_click,
                })
            })
            .collect();

        let keys: Vec<TreeNodeKey<Id>> = entries.iter().map(|entry| entry.key.clone()).collect();
        let scroll_request = self.scroll_request.get();
        let scroll_to = scroll_request
            .as_ref()
            .and_then(|(key, _)| entries.iter().position(|entry| &entry.key == key))
            .or_else(|| {
                entries.iter().position(|entry| {
                    if uses_key_selection {
                        selected_keys.contains(&entry.key)
                    } else {
                        selected_ids.contains(&entry.id)
                    }
                })
            });
        let scroll_align = scroll_request
            .as_ref()
            .map(|(_, align)| *align)
            .unwrap_or_default();

        let tree = if layout.virtual_scroll {
            let mut stack = FixedVirtualStack::new(keys, layout.row_height, {
                let rows = rows.clone();
                let entries = entries.clone();
                move |key: &TreeNodeKey<Id>| {
                    entries
                        .iter()
                        .position(|entry| &entry.key == key)
                        .and_then(|index| rows.get(index).cloned())
                        .map(AnyView::new)
                        .unwrap_or_else(|| AnyView::new(crate::EmptyView))
                }
            })
            .spacing(layout.row_spacing);
            if let Some(index) = scroll_to {
                stack = stack.scroll_to_index_aligned(index, virtual_align(scroll_align));
            }
            AnyView::new(
                AnyView::new(stack)
                    .frame(None, None, Some(0.0), Some(0.0), None, None)
                    .flex_with(1.0, 1.0, Some(0.0)),
            )
        } else {
            let content = AnyView::new(
                AnyView::new(BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    layout.row_spacing,
                    AnyView::new(TupleView::new(rows.clone())),
                ))
                .frame(None, None, Some(0.0), None, None, None),
            );
            let mut scroll = crate::primitives::layout::ScrollView::new(
                crate::primitives::layout::ScrollAxis::Vertical,
                content,
            );
            if let Some(index) = scroll_to {
                let stride = layout.row_height + layout.row_spacing;
                let top = index as f32 * stride;
                let offset = match scroll_align {
                    TreeScrollAlign::End => top + layout.row_height,
                    _ => top,
                };
                scroll = scroll.scroll_request(crate::primitives::layout::ScrollRequest::new(
                    Point::new(0.0, offset),
                    0.0,
                ));
            }
            AnyView::new(
                AnyView::new(scroll)
                    .frame(None, None, Some(0.0), Some(0.0), None, None)
                    .flex_with(1.0, 1.0, Some(0.0)),
            )
        };

        let host = SelectionKeyHost::new(tree, is_enabled, {
            let actions = actions.clone();
            move |event: &KeyEvent| actions.handle_key(event)
        });

        AnyView::new(AnyView::new(host).accessibility(
            move |semantics: &mut AccessibilitySemantics| {
                semantics.role = AccessibilityRole::Tree;
                semantics.state.is_enabled = is_enabled;
            },
        ))
    }

    fn dynamic_properties(&self) -> Vec<crate::DynamicField> {
        vec![
            crate::DynamicField::state("expansion", &self.local_expanded),
            crate::DynamicField::state("expansionKeys", &self.local_expanded_keys),
            crate::DynamicField::state("checked", &self.local_checked),
            crate::DynamicField::state("checkedKeys", &self.local_checked_keys),
            crate::DynamicField::state("loadedKeys", &self.local_loaded_keys),
            crate::DynamicField::state("filter", &self.local_query),
            crate::DynamicField::state("selected", &self.local_selected),
            crate::DynamicField::state("multiSelected", &self.local_multi_selected),
            crate::DynamicField::state("cursor", &self.cursor),
            crate::DynamicField::state("scrollRequest", &self.scroll_request),
            crate::DynamicField::state("dragSource", &self.drag_source),
            crate::DynamicField::state("dragTarget", &self.drag_target),
        ]
    }
}

/// The tree's actions, cloned so row closures can call them without borrowing
/// the view for the node's lifetime.
struct TreeActions<Element, Id>
where
    Element: Clone + 'static,
    Id: CollectionId,
{
    entries: Rc<RefCell<Vec<TreeEntry<Element, Id>>>>,
    expanded: State<HashSet<Id>>,
    local_expanded_keys: State<HashSet<TreeNodeKey<Id>>>,
    expanded_keys: Option<Binding<HashSet<TreeNodeKey<Id>>>>,
    selection: TreeSelection<Id>,
    check: TreeCheck<Element, Id>,
    expansion: TreeExpansion,
    lazy: TreeLazy<Element, Id>,
    edit: TreeEdit<Element>,
    anchor: State<Option<TreeNodeKey<Id>>>,
    cursor: State<Option<TreeNodeKey<Id>>>,
    local_checked: State<HashSet<Id>>,
    local_checked_keys: State<HashSet<TreeNodeKey<Id>>>,
    local_loaded_keys: State<HashSet<TreeNodeKey<Id>>>,
    local_selected: State<Option<Id>>,
    local_multi_selected: State<BTreeSet<Id>>,
    scroll_request: State<Option<(TreeNodeKey<Id>, TreeScrollAlign)>>,
    events: TreeEvents<Element, Id>,
    modifiers: Rc<RefCell<KeyModifiers>>,
    roots: Rc<Vec<Element>>,
    id_of: Rc<dyn Fn(&Element) -> Id>,
    children: Rc<dyn Fn(&Element) -> Vec<Element>>,
    has_children: Option<Rc<dyn Fn(&Element) -> bool>>,
    drag: TreeDragOperations<Element>,
    drag_source: State<Option<TreeNodeKey<Id>>>,
    drag_target: State<Option<(TreeNodeKey<Id>, TreeDropPosition)>>,
}

impl<Element, Id> Clone for TreeActions<Element, Id>
where
    Element: Clone + 'static,
    Id: CollectionId,
{
    fn clone(&self) -> Self {
        Self {
            entries: self.entries.clone(),
            expanded: self.expanded.clone(),
            local_expanded_keys: self.local_expanded_keys.clone(),
            expanded_keys: self.expanded_keys.clone(),
            selection: self.selection.clone(),
            check: self.check.clone(),
            expansion: self.expansion,
            lazy: self.lazy.clone(),
            edit: self.edit.clone(),
            anchor: self.anchor.clone(),
            cursor: self.cursor.clone(),
            local_checked: self.local_checked.clone(),
            local_checked_keys: self.local_checked_keys.clone(),
            local_loaded_keys: self.local_loaded_keys.clone(),
            local_selected: self.local_selected.clone(),
            local_multi_selected: self.local_multi_selected.clone(),
            scroll_request: self.scroll_request.clone(),
            events: TreeEvents {
                on_key_command: self.events.on_key_command.clone(),
                on_select: self.events.on_select.clone(),
                on_select_change: self.events.on_select_change.clone(),
                on_expand_change: self.events.on_expand_change.clone(),
                on_node_click: self.events.on_node_click.clone(),
                on_node_context_menu: self.events.on_node_context_menu.clone(),
                on_double_click: self.events.on_double_click.clone(),
            },
            modifiers: self.modifiers.clone(),
            roots: self.roots.clone(),
            id_of: self.id_of.clone(),
            children: self.children.clone(),
            has_children: self.has_children.clone(),
            drag: self.drag.clone(),
            drag_source: self.drag_source.clone(),
            drag_target: self.drag_target.clone(),
        }
    }
}

impl<Element, Id> TreeActions<Element, Id>
where
    Element: Clone + 'static,
    Id: CollectionId,
{
    fn index(&self) -> TreeIndex<Element, Id> {
        TreeIndex::build(
            &self.roots,
            &*self.id_of,
            &*self.children,
            self.has_children.as_deref(),
        )
    }

    fn expanded_ids(&self) -> HashSet<Id> {
        match &self.selection.expanded {
            Some(binding) => binding.get(),
            None => self.expanded.get(),
        }
    }

    fn expanded_keys(&self) -> HashSet<TreeNodeKey<Id>> {
        match &self.selection.expanded_keys {
            Some(binding) => binding.get(),
            None => self.local_expanded_keys.get(),
        }
    }

    fn expanded_snapshot(&self) -> HashSet<TreeNodeKey<Id>> {
        let mut keys = self.expanded_keys();
        let ids = self.expanded_ids();
        if ids.is_empty() {
            return keys;
        }
        for node in &self.index().nodes {
            if ids.contains(&node.id) {
                keys.insert(node.key.clone());
            }
        }
        keys
    }

    fn set_expanded(&self, ids: HashSet<Id>, keys: HashSet<TreeNodeKey<Id>>) {
        let use_local = self.selection.expanded.is_none() && self.selection.expanded_keys.is_none();
        if let Some(binding) = &self.selection.expanded_keys {
            binding.set(keys.clone());
        } else if use_local {
            self.local_expanded_keys.set(keys);
        }
        if let Some(binding) = &self.selection.expanded {
            binding.set(ids.clone());
        } else if use_local {
            self.expanded.set(ids);
        }
    }

    fn checked_state(&self) -> (HashSet<Id>, HashSet<TreeNodeKey<Id>>) {
        if let Some(binding) = &self.check.checked_keys {
            let keys = binding.get();
            let ids = keys.iter().map(|key| key.id.clone()).collect();
            return (ids, keys);
        }
        if let Some(binding) = &self.check.checked {
            return (binding.get(), HashSet::new());
        }
        (self.local_checked.get(), self.local_checked_keys.get())
    }

    fn set_checked_state(&self, ids: HashSet<Id>, keys: HashSet<TreeNodeKey<Id>>) {
        if let Some(binding) = &self.check.checked_keys {
            binding.set(keys.clone());
        } else {
            self.local_checked_keys.set(keys);
        }
        if let Some(binding) = &self.check.checked {
            binding.set(ids);
        } else {
            self.local_checked.set(ids);
        }
    }

    fn allow_multiple(&self) -> bool {
        if self.selection.is_local {
            return self.selection.local_multiple;
        }
        self.selection.ids.allows_multiple()
            || self
                .selection
                .keys
                .as_ref()
                .is_some_and(|keys| keys.allows_multiple())
    }

    fn selected_key_snapshot(&self) -> HashSet<TreeNodeKey<Id>> {
        let entries = self.entries.borrow();
        if let Some(keys) = &self.selection.keys {
            return keys.ids().into_iter().collect();
        }
        if self.selection.is_local {
            if self.selection.local_multiple {
                let ids = self.local_multi_selected.get();
                return entries
                    .iter()
                    .filter(|entry| ids.contains(&entry.id))
                    .map(|entry| entry.key.clone())
                    .collect();
            }
            let selected = self.local_selected.get();
            return entries
                .iter()
                .filter(|entry| Some(&entry.id) == selected.as_ref())
                .map(|entry| entry.key.clone())
                .collect();
        }
        let ids = self.selection.ids.ids();
        entries
            .iter()
            .filter(|entry| ids.contains(&entry.id))
            .map(|entry| entry.key.clone())
            .collect()
    }

    fn toggle(&self, entry: &TreeEntry<Element, Id>) {
        let expanding = !entry.is_expanded;
        let mut ids = self.expanded_ids();
        let mut keys = self.expanded_keys();
        if expanding {
            if self.expansion.accordion {
                let siblings: Vec<TreeNodeKey<Id>> = self
                    .entries
                    .borrow()
                    .iter()
                    .filter(|other| {
                        other.key != entry.key
                            && other.parent_key == entry.parent_key
                            && other.is_expanded
                    })
                    .map(|other| other.key.clone())
                    .collect();
                for sibling in siblings {
                    ids.remove(&sibling.id);
                    keys.remove(&sibling);
                }
            }
            if self.expansion.auto_expand_parent {
                let mut ancestor = entry.parent_key.clone();
                while let Some(key) = ancestor {
                    keys.insert(key.clone());
                    ids.insert(key.id.clone());
                    ancestor = self
                        .entries
                        .borrow()
                        .iter()
                        .find(|other| other.key == key)
                        .and_then(|other| other.parent_key.clone());
                }
            }
            ids.insert(entry.id.clone());
            keys.insert(entry.key.clone());
        } else {
            ids.remove(&entry.id);
            keys.remove(&entry.key);
        }
        self.set_expanded(ids, keys);
        if let Some(handler) = &self.events.on_expand_change {
            handler(&entry.element, expanding, &self.expanded_snapshot());
        }
        if expanding {
            self.load_if_needed(entry);
        }
    }

    fn load_if_needed(&self, entry: &TreeEntry<Element, Id>) {
        if !self.lazy.is_enabled || !entry.has_children {
            return;
        }
        let already_loaded = match &self.lazy.loaded_keys {
            Some(binding) => binding.get().contains(&entry.key),
            None => self.local_loaded_keys.get().contains(&entry.key),
        };
        if already_loaded {
            return;
        }
        if let Some(load) = &self.lazy.load_data {
            load(&entry.element);
        }
        match &self.lazy.loaded_keys {
            Some(binding) => {
                let mut next = binding.get();
                if next.insert(entry.key.clone()) {
                    binding.set(next);
                }
            }
            None => {
                let mut next = self.local_loaded_keys.get();
                if next.insert(entry.key.clone()) {
                    self.local_loaded_keys.set(next);
                }
            }
        }
    }

    fn collapse_all(&self) {
        self.set_expanded(HashSet::new(), HashSet::new());
    }

    /// Expands every branch reachable through the child resolver.
    fn expand_all(&self) {
        let mut keys: HashSet<TreeNodeKey<Id>> = HashSet::new();
        fn visit<Element: Clone, Id: std::hash::Hash + Eq + Clone>(
            nodes: &[Element],
            path: &[usize],
            id_of: &dyn Fn(&Element) -> Id,
            children: &dyn Fn(&Element) -> Vec<Element>,
            has_children: Option<&dyn Fn(&Element) -> bool>,
            keys: &mut HashSet<TreeNodeKey<Id>>,
        ) {
            for (index, element) in nodes.iter().enumerate() {
                if has_children.is_some_and(|predicate| !predicate(element)) {
                    continue;
                }
                let kids = children(element);
                if kids.is_empty() {
                    continue;
                }
                let mut next_path = path.to_vec();
                next_path.push(index);
                keys.insert(TreeNodeKey::new(id_of(element), next_path.clone()));
                visit(&kids, &next_path, id_of, children, has_children, keys);
            }
        }
        visit(
            &self.roots,
            &[],
            &*self.id_of,
            &*self.children,
            self.has_children.as_deref(),
            &mut keys,
        );
        let ids: HashSet<Id> = keys.iter().map(|key| key.id.clone()).collect();
        self.set_expanded(ids, keys);
    }

    fn select(&self, entry: &TreeEntry<Element, Id>, modifiers: KeyModifiers) {
        *self.modifiers.borrow_mut() = modifiers;
        if !self.selection.is_selectable {
            self.cursor.set(Some(entry.key.clone()));
            return;
        }
        let multiple = self.allow_multiple();
        if self.selection.is_local {
            if multiple && modifiers.has_shift() {
                let anchor = self.anchor.get().unwrap_or_else(|| entry.key.clone());
                let range = keys_between(&anchor, &entry.key, &self.entries.borrow());
                if self.selection.local_multiple {
                    let ids: BTreeSet<Id> = range.iter().map(|key| key.id.clone()).collect();
                    self.local_multi_selected.set(ids);
                }
            } else if multiple && (modifiers.has_gui() || modifiers.has_ctrl()) {
                let id = entry.id.clone();
                if self.selection.local_multiple {
                    let mut next = self.local_multi_selected.get();
                    if !next.remove(&id) {
                        next.insert(id);
                    }
                    self.local_multi_selected.set(next);
                }
                self.anchor.set(Some(entry.key.clone()));
            } else if self.selection.local_multiple {
                self.local_multi_selected
                    .set([entry.id.clone()].into_iter().collect());
                self.anchor.set(Some(entry.key.clone()));
            } else {
                self.local_selected.set(Some(entry.id.clone()));
                self.anchor.set(Some(entry.key.clone()));
            }
        } else {
            let ids = &self.selection.ids;
            let keys = self.selection.keys.as_ref();
            if multiple && modifiers.has_shift() {
                let anchor = self.anchor.get().unwrap_or_else(|| entry.key.clone());
                let mut range = keys_between(&anchor, &entry.key, &self.entries.borrow());
                if range.is_empty() {
                    range.insert(entry.key.clone());
                }
                ids.replace(range.iter().map(|key| key.id.clone()));
                if let Some(keys) = keys {
                    keys.replace(range);
                }
            } else if multiple && (modifiers.has_gui() || modifiers.has_ctrl()) {
                ids.toggle(entry.id.clone());
                if let Some(keys) = keys {
                    keys.toggle(entry.key.clone());
                }
                self.anchor.set(Some(entry.key.clone()));
            } else {
                ids.replace([entry.id.clone()]);
                if let Some(keys) = keys {
                    keys.replace([entry.key.clone()]);
                }
                self.anchor.set(Some(entry.key.clone()));
            }
        }
        self.cursor.set(Some(entry.key.clone()));
        if let Some(handler) = &self.events.on_select {
            handler(entry.element.clone());
        }
        if let Some(handler) = &self.events.on_select_change {
            let selected = self.selected_key_snapshot();
            handler(&entry.element, true, &selected);
        }
    }

    fn toggle_checked(&self, entry: &TreeEntry<Element, Id>) {
        if !self.check.is_enabled {
            return;
        }
        let (ids, keys) = self.checked_state();
        let checked = !(keys.contains(&entry.key) || ids.contains(&entry.id));
        self.apply_checked(&entry.key, &entry.element, checked, None);
    }

    fn apply_checked(
        &self,
        key: &TreeNodeKey<Id>,
        element: &Element,
        checked: bool,
        deep: Option<bool>,
    ) {
        let index = self.index();
        let strict = deep.unwrap_or(self.check.check_strictly);
        let (ids, keys) = self.checked_state();
        let (ids, keys) = check::toggle(&index, &ids, &keys, key, checked, strict);
        self.set_checked_state(ids, keys);
        if let Some(handler) = &self.check.on_change {
            let (ids, keys) = self.checked_state();
            let snapshot = check::snapshot(&index, &ids, &keys, strict);
            handler(
                element,
                checked,
                &snapshot.checked_keys,
                &snapshot.half_checked_keys,
            );
        }
    }

    /// Records the row the pointer is over and the band it lands in. Invalid or
    /// self targets keep the source row marked but clear the drop target.
    fn update_drag(
        &self,
        source: &TreeNodeKey<Id>,
        x: f32,
        y: f32,
        registry: &TreeDragRegistry<Id>,
    ) {
        let previous = self.drag_target.get();
        let target = registry.hit(Point::new(x, y));
        let entries = self.entries.borrow();
        let source_entry = entries.iter().find(|entry| &entry.key == source).cloned();
        let target_entry = target
            .as_ref()
            .and_then(|(key, _)| entries.iter().find(|entry| &entry.key == key).cloned());
        drop(entries);
        let Some(source_entry) = source_entry else {
            self.drag_target.set(None);
            return;
        };
        let Some((target_key, frame)) = target else {
            self.drag_target.set(None);
            if previous.is_some()
                && let Some(handler) = &self.drag.on_drag_leave
            {
                handler(&source_entry.element);
            }
            return;
        };
        let Some(target_entry) = target_entry else {
            self.drag_target.set(None);
            return;
        };
        if &target_key == source {
            self.drag_target.set(None);
            return;
        }
        let position = drop_position(y, frame);
        if self.drag.can_drop.as_ref().is_some_and(|can_drop| {
            !can_drop(&source_entry.element, &target_entry.element, position)
        }) {
            self.drag_target.set(None);
            return;
        }
        if previous.as_ref().is_some_and(|(key, _)| key != &target_key)
            && let Some(handler) = &self.drag.on_drag_enter
        {
            handler(&source_entry.element, &target_entry.element, position);
        }
        self.drag_target.set(Some((target_key, position)));
    }

    fn drag_start(&self, entry: &TreeEntry<Element, Id>) {
        self.drag_source.set(Some(entry.key.clone()));
        if let Some(handler) = &self.drag.on_drag_start {
            handler(&entry.element);
        }
    }

    /// Reports the drop for the current target, then clears the session. A
    /// release with no target reports nothing.
    fn commit_drag(&self, source: &TreeEntry<Element, Id>) {
        let target = self.drag_target.get();
        self.drag_source.set(None);
        self.drag_target.set(None);
        if let Some((target_key, position)) = target {
            let entries = self.entries.borrow();
            let target_entry = entries
                .iter()
                .find(|entry| entry.key == target_key)
                .cloned();
            drop(entries);
            if let Some(target_entry) = target_entry {
                if let Some(on_drop) = &self.drag.on_drop {
                    on_drop(
                        source.element.clone(),
                        target_entry.element.clone(),
                        position,
                    );
                }
                if let Some(handler) = &self.drag.on_drag_end {
                    handler(&source.element, Some(&target_entry.element));
                }
                return;
            }
        }
        if let Some(handler) = &self.drag.on_drag_end {
            handler(&source.element, None);
        }
    }

    fn step(&self, entry: &TreeEntry<Element, Id>, delta: i64) {
        let entries = self.entries.borrow();
        let Some(index) = entries.iter().position(|other| other.key == entry.key) else {
            return;
        };
        let target = (index as i64 + delta).clamp(0, entries.len() as i64 - 1) as usize;
        if target != index {
            let target = entries[target].clone();
            drop(entries);
            let modifiers = *self.modifiers.borrow();
            self.select(&target, modifiers);
        }
    }

    fn collapse(&self, entry: &TreeEntry<Element, Id>) {
        if entry.has_children && entry.is_expanded {
            self.toggle(entry);
            return;
        }
        let entries = self.entries.borrow();
        let parent = entry
            .parent_key
            .as_ref()
            .and_then(|key| entries.iter().find(|other| &other.key == key))
            .cloned();
        drop(entries);
        if let Some(parent) = parent {
            let modifiers = *self.modifiers.borrow();
            self.select(&parent, modifiers);
        }
    }

    fn expand(&self, entry: &TreeEntry<Element, Id>) {
        if !entry.has_children {
            return;
        }
        if !entry.is_expanded {
            self.toggle(entry);
            return;
        }
        let entries = self.entries.borrow();
        let child = entries
            .iter()
            .find(|other| other.parent_key.as_ref() == Some(&entry.key))
            .cloned();
        drop(entries);
        if let Some(child) = child {
            let modifiers = *self.modifiers.borrow();
            self.select(&child, modifiers);
        }
    }

    fn current_entry(&self, entries: &[TreeEntry<Element, Id>]) -> Option<TreeEntry<Element, Id>> {
        let by_cursor = self
            .cursor
            .get()
            .and_then(|key| entries.iter().find(|entry| entry.key == key).cloned());
        let by_selection = {
            if self.selection.is_local {
                let first_id = if self.selection.local_multiple {
                    self.local_multi_selected.get().into_iter().next()
                } else {
                    self.local_selected.get()
                };
                first_id.and_then(|id| entries.iter().find(|entry| entry.id == id).cloned())
            } else {
                self.selection
                    .keys
                    .as_ref()
                    .and_then(|keys| keys.first())
                    .and_then(|key| entries.iter().find(|entry| entry.key == key).cloned())
                    .or_else(|| {
                        self.selection
                            .ids
                            .first()
                            .and_then(|id| entries.iter().find(|entry| entry.id == id).cloned())
                    })
            }
        };
        if self.allow_multiple() {
            by_cursor.or(by_selection)
        } else {
            by_selection.or(by_cursor)
        }
        .or_else(|| entries.first().cloned())
    }

    fn handle_key(&self, event: &KeyEvent) -> bool {
        let entries = self.entries.borrow().clone();
        if entries.is_empty() {
            return false;
        }
        // Alt+Right expands every descendant and Alt+Left collapses the whole
        // forest, whatever the current row is.
        if event.modifiers.has_alt() {
            match event.scancode {
                Scancode::ARROW_RIGHT => {
                    self.expand_all();
                    return true;
                }
                Scancode::ARROW_LEFT => {
                    self.collapse_all();
                    return true;
                }
                _ => {}
            }
        }
        let Some(current) = self.current_entry(&entries) else {
            return false;
        };
        *self.modifiers.borrow_mut() = event.modifiers;

        match event.scancode {
            Scancode::ARROW_DOWN => {
                let index = entries
                    .iter()
                    .position(|entry| entry.key == current.key)
                    .unwrap_or(0);
                if let Some(next) = entries.get(index + 1).cloned() {
                    self.select(&next, event.modifiers);
                }
                true
            }
            Scancode::ARROW_UP => {
                let index = entries
                    .iter()
                    .position(|entry| entry.key == current.key)
                    .unwrap_or(0);
                if index > 0
                    && let Some(previous) = entries.get(index - 1).cloned()
                {
                    self.select(&previous, event.modifiers);
                }
                true
            }
            Scancode::ARROW_LEFT => {
                self.collapse(&current);
                true
            }
            Scancode::ARROW_RIGHT => {
                self.expand(&current);
                true
            }
            Scancode::HOME => {
                if let Some(first) = entries.first().cloned() {
                    self.select(&first, event.modifiers);
                }
                true
            }
            Scancode::END => {
                if let Some(last) = entries.last().cloned() {
                    self.select(&last, event.modifiers);
                }
                true
            }
            Scancode::SPACE if self.check.is_enabled => {
                self.toggle_checked(&current);
                true
            }
            Scancode::RETURN | Scancode::KEYPAD_ENTER | Scancode::SPACE => {
                self.select(&current, event.modifiers);
                true
            }
            _ => false,
        }
    }
}

fn virtual_align(align: TreeScrollAlign) -> VirtualScrollAlign {
    match align {
        TreeScrollAlign::Auto => VirtualScrollAlign::Auto,
        TreeScrollAlign::Start => VirtualScrollAlign::Start,
        TreeScrollAlign::Center => VirtualScrollAlign::Center,
        TreeScrollAlign::End => VirtualScrollAlign::End,
    }
}

/// A fixed-size empty slot. `EmptyView` has no layout node, so a frame applied
/// to it reserves no width; leaf rows then lose their indentation and
/// disclosure gutter.
fn fixed_spacer(width: f32, height: f32) -> AnyView {
    AnyView::new(
        AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(TupleView::new(Vec::new())),
        ))
        .frame_size(width.max(0.0), height),
    )
}

/// One visible row: indentation, disclosure, indent guides, style-driven body
/// and the trailing slot.
struct TreeRow {
    is_enabled: bool,
    has_children: bool,
    is_expanded: bool,
    is_search_hit: bool,
    is_selected: bool,
    is_hovered: bool,
    is_checked: bool,
    is_half_checked: bool,
    is_loading: bool,
    is_current: bool,
    width_filling: bool,
    depth: usize,
    row_height: f32,
    indentation: f32,
    disclosure_width: f32,
    shows_indent_guides: bool,
    ancestor_has_next_siblings: Vec<bool>,
    has_next_sibling: bool,
    drop_position: Option<TreeDropPosition>,
    is_drag_source: bool,
    content: AnyView,
    checkbox: Option<AnyView>,
    icon: Option<AnyView>,
    disclosure_slot: Option<Rc<dyn Fn(bool) -> AnyView>>,
    trailing_slot: Option<AnyView>,
    /// The armed drag surface, `None` when `on_drop` is unset.
    drag: Option<TreeRowDrag>,
    on_toggle: Rc<dyn Fn()>,
    on_select: Rc<dyn Fn(KeyModifiers)>,
    on_move: Rc<dyn Fn(i64)>,
    on_collapse: Rc<dyn Fn()>,
    on_expand: Rc<dyn Fn()>,
    on_toggle_check: Option<Rc<dyn Fn()>>,
    on_hover: Rc<dyn Fn(bool)>,
    on_node_click: Option<Rc<dyn Fn(KeyModifiers)>>,
    on_context_menu: Option<Rc<dyn Fn(Point)>>,
    on_double_click: Option<Rc<dyn Fn()>>,
}

impl PrimitiveView for TreeRow {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(FlexDirection::Row);
        layout.set_align_items(Align::Center);
        layout.set_height(Some(self.row_height));
    }

    fn update_node(&self, node: &Node) {
        node.set_is_hit_testable(self.is_enabled);
        node.set_cursor(Some(SystemCursor::Pointer));
        // The drag hit zone covers the indent gutter and disclosure slot, so a
        // drop over indented columns still resolves this row.
        node.set_attachment(
            ROW_GUTTER,
            (self.depth as f32 * self.indentation + self.disclosure_width).max(0.0),
        );
        if let Some(drag) = &self.drag {
            (drag.register)(node);
        }
        node.set_opacity(if self.is_drag_source { 0.38 } else { 1.0 });
        node.set_corner_radius(4.0);
        if let Some(position) = self.drop_position {
            let gutter = self.depth as f32 * self.indentation + self.disclosure_width;
            let weak = node.downgrade();
            let overlay: DrawHook = Rc::new(move |list, origin| {
                let Some(node) = Node::upgrade(&weak) else {
                    return;
                };
                let frame = node.frame();
                draw_drop_indicator(
                    position,
                    Rect::new(
                        origin.x - gutter,
                        origin.y,
                        frame.width + gutter,
                        frame.height,
                    ),
                    theme_for_node(&node).colors.accent,
                    list,
                );
            });
            node.set_overlay_draw(Some(overlay));
        } else {
            node.set_overlay_draw(None);
        }
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::TreeItem,
            value: if self.checkbox.is_some() {
                if self.is_half_checked {
                    "mixed".to_string()
                } else if self.is_checked {
                    "checked".to_string()
                } else {
                    "unchecked".to_string()
                }
            } else {
                String::new()
            },
            state: cove_core::scene::AccessibilityState {
                is_enabled: self.is_enabled,
                is_selected: self.is_selected || self.is_checked,
                is_expanded: self.has_children.then_some(self.is_expanded),
                ..Default::default()
            },
            combines_children: true,
            ..Default::default()
        }));
        if self.is_enabled {
            let select = self.on_select.clone();
            node.set_accessibility_actions(AccessibilityActions {
                activate: Some(Rc::new(move || select(KeyModifiers(0)))),
                ..Default::default()
            });
        }

        let paint_node = node.clone();
        let is_selected = self.is_selected;
        let is_search_hit = self.is_search_hit;
        let is_current = self.is_current;
        let paint: Rc<dyn Fn()> = Rc::new(move || {
            let hovered = paint_node.attachment::<bool>(HOVERED) == Some(true);
            let theme = theme_for_node(&paint_node);
            paint_node.set_corner_radius(theme.radius.sm);
            paint_node.set_background_color(Some(if is_selected || is_search_hit {
                Color::CLEAR
            } else if hovered {
                theme.colors.state_layer_hover
            } else if is_current {
                theme.colors.state_layer_selected.multiplied_alpha(0.45)
            } else {
                Color::CLEAR
            }));
        });
        // A custom style watches its own hover and has to rebuild on a change; the
        // default one keeps the chrome on this node. A caller-supplied trailing
        // slot also reads the flag, so it keeps the state in sync either way.
        let style = node.composition_value(&TreeRowStyleEnvironment::key());
        let propagates_hover = self.trailing_slot.is_some() || style.requires_hover_recompose();
        if propagates_hover {
            node.set_attachment(HOVERED, self.is_hovered);
        } else if node.attachment::<bool>(HOVERED).is_none() {
            node.set_attachment(HOVERED, false);
        }
        paint();

        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };

        let hover_node = node.downgrade();
        let hover_paint = paint.clone();
        let hover_action = self.on_hover.clone();
        let hover_enabled = self.is_enabled;
        registry.set_hover(
            node,
            Rc::new(move |phase: HoverPhase| {
                if !hover_enabled {
                    return;
                }
                let entered = phase == HoverPhase::Enter;
                // Paint from the handler so the row answers the pointer in the
                // same frame; the default style never needs a recompose for it.
                if let Some(owner) = Node::upgrade(&hover_node) {
                    owner.set_attachment(HOVERED, entered);
                }
                hover_paint();
                if propagates_hover {
                    hover_action(entered);
                }
            }),
        );

        let pointer_select = self.on_select.clone();
        let pointer_node = node.clone();
        let pointer_drag = self.drag.clone();
        let pointer_click = self.on_node_click.clone();
        let pointer_context = self.on_context_menu.clone();
        let pointer_double = self.on_double_click.clone();
        registry.set_pointer(
            node,
            InputHandlerRoute::control(),
            Rc::new(
                move |event: &cove_platform::MouseButtonEvent,
                      phase: PointerPhase,
                      delivery: EventPhase| {
                    if delivery == EventPhase::Capture {
                        return EventResult::Ignored;
                    }
                    if event.button == MouseButton::Right {
                        if phase == PointerPhase::Up
                            && let Some(context) = &pointer_context
                        {
                            context(Point::new(event.x, event.y));
                            return EventResult::Handled;
                        }
                        return EventResult::Ignored;
                    }
                    if event.button != MouseButton::Left {
                        return EventResult::Ignored;
                    }
                    match phase {
                        PointerPhase::Down => {
                            pointer_node.set_attachment(PRESSED, true);
                            // An armed drag surface captures the pointer so motion
                            // keeps arriving after the row recomposes.
                            if pointer_drag.is_some() {
                                pointer_node.set_attachment(
                                    DRAG_PRESS,
                                    TreeDragPress {
                                        down_x: event.x,
                                        down_y: event.y,
                                        did_drag: false,
                                    },
                                );
                                if let Some(capture) = PointerCaptureHolder::current() {
                                    // Keep the gesture transferable until the
                                    // drag threshold: pressing a row must still
                                    // let a surrounding scroll view scroll.
                                    capture.acquire(&pointer_node, true);
                                }
                            }
                            EventResult::Handled
                        }
                        PointerPhase::Up => {
                            let was = pointer_node.attachment::<bool>(PRESSED) == Some(true);
                            pointer_node.set_attachment(PRESSED, false);
                            let press = pointer_node.attachment::<TreeDragPress>(DRAG_PRESS);
                            pointer_node.remove_attachment(DRAG_PRESS);
                            if pointer_drag.is_some() {
                                PointerCaptureHolder::release_current();
                            }
                            if press.is_some_and(|press| press.did_drag) {
                                if let Some(drag) = &pointer_drag {
                                    (drag.on_end)();
                                }
                                return EventResult::Handled;
                            }
                            if was {
                                pointer_select(event.modifiers);
                                if let Some(click) = &pointer_click {
                                    click(event.modifiers);
                                }
                                if event.clicks >= 2
                                    && let Some(double) = &pointer_double
                                {
                                    double();
                                }
                                return EventResult::Handled;
                            }
                            EventResult::Ignored
                        }
                    }
                },
            ),
        );

        if let Some(drag) = &self.drag {
            let drag = drag.clone();
            let motion_node = node.downgrade();
            registry.set_motion(
                node,
                InputHandlerRoute::control(),
                Rc::new(
                    move |event: &cove_platform::MouseMotionEvent, phase: EventPhase| {
                        if phase == EventPhase::Capture {
                            return EventResult::Ignored;
                        }
                        let Some(owner) = Node::upgrade(&motion_node) else {
                            return EventResult::Ignored;
                        };
                        let Some(target) = PointerCaptureHolder::current_target() else {
                            return EventResult::Ignored;
                        };
                        if target != owner {
                            return EventResult::Ignored;
                        }
                        let mut press = owner.attachment::<TreeDragPress>(DRAG_PRESS).unwrap_or(
                            TreeDragPress {
                                down_x: event.x,
                                down_y: event.y,
                                did_drag: false,
                            },
                        );
                        let dx = event.x - press.down_x;
                        let dy = event.y - press.down_y;
                        // A small dead zone keeps a click from becoming a drag.
                        if !press.did_drag && dx.abs().max(dy.abs()) >= 4.0 {
                            press.did_drag = true;
                            owner.set_attachment(PRESSED, false);
                            if let Some(capture) = PointerCaptureHolder::current() {
                                // Exclusive from here: the drag owns the pointer.
                                capture.acquire(&owner, false);
                            }
                            (drag.on_start)();
                        }
                        if press.did_drag {
                            owner.set_attachment(DRAG_PRESS, press);
                            (drag.on_move)(event.x, event.y);
                        } else {
                            owner.set_attachment(DRAG_PRESS, press);
                        }
                        EventResult::Handled
                    },
                ),
            );
        }

        let key_node = node.clone();
        let on_key = self.on_key_command();
        registry.set_key(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event: &KeyEvent, phase: EventPhase| {
                if phase == EventPhase::Capture || event.is_repeat {
                    return EventResult::Ignored;
                }
                if on_key(event) {
                    EventResult::Handled
                } else {
                    EventResult::Ignored
                }
            }),
        );
        let _ = key_node;

        let clear_node = node.downgrade();
        node.set_attachment(
            cove_core::scene::PointerCaptureAttachmentKey::DID_LOSE_CAPTURE,
            Rc::new(move || {
                if let Some(owner) = Node::upgrade(&clear_node) {
                    owner.set_attachment(PRESSED, false);
                    owner.remove_attachment(DRAG_PRESS);
                }
            }) as Rc<dyn Fn()>,
        );
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let indent = self.depth as f32 * self.indentation;
        let row_height = self.row_height;
        let style = node.composition_value(&TreeRowStyleEnvironment::key());
        let configuration = TreeRowStyleConfiguration {
            content: self.content.clone(),
            depth: self.depth,
            indentation: self.indentation,
            disclosure_width: self.disclosure_width,
            has_children: self.has_children,
            is_expanded: self.is_expanded,
            is_search_hit: self.is_search_hit,
            is_selected: self.is_selected,
            is_hovered: self.is_hovered,
            is_checked: self.is_checked,
            is_half_checked: self.is_half_checked,
            is_current: self.is_current,
            is_enabled: self.is_enabled,
            fills_width: self.width_filling,
            theme: theme_for_node(node),
        };
        let body = style.make_body(configuration);

        let disclosure = if self.is_loading {
            AnyView::new(
                AnyView::new(ProgressCircle::new(None).diameter(14.0).thickness(1.5))
                    .frame_size(self.disclosure_width, row_height),
            )
        } else if self.has_children {
            if let Some(slot) = &self.disclosure_slot {
                let slot = slot.clone();
                let expanded = self.is_expanded;
                AnyView::new(
                    AnyView::new(slot(expanded)).frame_size(self.disclosure_width, row_height),
                )
            } else {
                let icon = if self.is_expanded {
                    ui_icon_catalog::navigation::chevron_down()
                } else {
                    ui_icon_catalog::navigation::chevron_right()
                };
                let label = if self.is_expanded {
                    "Collapse"
                } else {
                    "Expand"
                };
                let toggle = self.on_toggle.clone();
                let enabled = self.is_enabled;
                AnyView::new(
                    AnyView::new(
                        Button::new(
                            Icon::with_semantic(icon, 16.0, SemanticColorRef::on_surface_variant()),
                            move || toggle(),
                        )
                        .enabled(enabled),
                    )
                    .button_style(crate::theme::styles::buttons::plain())
                    .frame_size(self.disclosure_width, row_height)
                    .accessibility(
                        move |semantics: &mut AccessibilitySemantics| {
                            semantics.label = label.to_string();
                        },
                    ),
                )
            }
        } else {
            fixed_spacer(self.disclosure_width, row_height)
        };

        let mut row = vec![fixed_spacer(indent, row_height), disclosure];
        if let Some(checkbox) = &self.checkbox {
            row.push(AnyView::new(checkbox.clone().padding_edges(0.0, 2.0)));
            row.push(fixed_spacer(6.0, row_height));
        }
        if let Some(icon) = &self.icon {
            row.push(icon.clone());
            row.push(fixed_spacer(4.0, row_height));
        }
        let content = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            body,
        ))
        .frame(None, None, Some(0.0), None, None, None);
        row.push(if self.width_filling {
            AnyView::new(AnyView::new(content).flex_with(1.0, 1.0, Some(0.0)))
        } else {
            AnyView::new(content)
        });

        if let Some(trailing) = &self.trailing_slot {
            row.push(trailing.clone());
        }

        let guides = if self.shows_indent_guides && self.depth > 0 {
            Some(TreeGuideOverlay {
                depth: self.depth,
                row_height,
                indentation: self.indentation,
                ancestor_has_next_siblings: self.ancestor_has_next_siblings.clone(),
                has_next_sibling: self.has_next_sibling,
            })
        } else {
            None
        };

        // The virtual stack gives the row a full-width slot. The row itself and
        // the guide overlay's ZStack must both take that width, or the selection
        // band shrinks to the label's intrinsic width.
        let row = AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(row)),
            ))
            .frame(
                Some(crate::modifiers::layout::FrameDimension::Percent(100.0)),
                Some(crate::modifiers::layout::FrameDimension::Points(row_height)),
                None,
                None,
                None,
                None,
            ),
        );

        match guides {
            Some(guides) => vec![AnyView::new(
                AnyView::new(
                    crate::primitives::layout::ZStack::new(AnyView::new(TupleView::new(vec![
                        AnyView::new(guides),
                        row,
                    ])))
                    .alignment(crate::Alignment::Leading),
                )
                .flex(1.0),
            )],
            None => vec![AnyView::new(AnyView::new(row).flex(1.0))],
        }
    }
}

impl TreeRow {
    fn on_key_command(&self) -> Rc<dyn Fn(&KeyEvent) -> bool> {
        let select = self.on_select.clone();
        let move_key = self.on_move.clone();
        let collapse = self.on_collapse.clone();
        let expand = self.on_expand.clone();
        let toggle_check = self.on_toggle_check.clone();
        let enabled = self.is_enabled;
        Rc::new(move |event: &KeyEvent| {
            if !enabled || event.is_repeat {
                return false;
            }
            match event.scancode {
                Scancode::ARROW_DOWN => {
                    move_key(1);
                    true
                }
                Scancode::ARROW_UP => {
                    move_key(-1);
                    true
                }
                Scancode::ARROW_LEFT => {
                    collapse();
                    true
                }
                Scancode::ARROW_RIGHT => {
                    expand();
                    true
                }
                Scancode::SPACE if toggle_check.is_some() => {
                    if let Some(toggle_check) = &toggle_check {
                        toggle_check();
                    }
                    true
                }
                Scancode::RETURN | Scancode::KEYPAD_ENTER | Scancode::SPACE => {
                    select(event.modifiers);
                    true
                }
                _ => false,
            }
        })
    }
}

/// The pointer-drag callbacks armed on a row: start when the dead zone is
/// crossed, move with the pointer, and commit on release.
#[derive(Clone)]
struct TreeRowDrag {
    /// Registers this row's node under its token so pointer motion can resolve
    /// the row and the drop band it is over.
    register: Rc<dyn Fn(&Node)>,
    on_start: Rc<dyn Fn()>,
    on_move: Rc<dyn Fn(f32, f32)>,
    on_end: Rc<dyn Fn()>,
}

/// Press bookkeeping that turns a click into a drag only after the dead zone.
#[derive(Clone, Copy)]
struct TreeDragPress {
    down_x: f32,
    down_y: f32,
    did_drag: bool,
}

impl View for TreeRow {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Indent guides: one branch line per depth, with a continuation column only
/// where an ancestor still has a following sibling.
struct TreeGuideOverlay {
    depth: usize,
    row_height: f32,
    indentation: f32,
    ancestor_has_next_siblings: Vec<bool>,
    has_next_sibling: bool,
}

impl PrimitiveView for TreeGuideOverlay {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.set_attachment(GUIDE_MARKER, true);
        node
    }

    fn update_node(&self, node: &Node) {
        if self.depth == 0 || self.row_height <= 0.0 || self.indentation <= 0.0 {
            node.set_draw(None);
            return;
        }
        let depth = self.depth;
        let row_height = self.row_height;
        let indentation = self.indentation;
        let ancestor = self.ancestor_has_next_siblings.clone();
        let has_next_sibling = self.has_next_sibling;
        let weak = node.downgrade();
        let draw: DrawHook = Rc::new(move |list, origin| {
            let Some(node) = Node::upgrade(&weak) else {
                return;
            };
            let theme = theme_for_node(&node);
            let ink = theme
                .colors
                .on_surface_muted
                .multiplied_alpha(0.24 * node.opacity());
            let base_x = origin.x;
            let base_y = origin.y;
            let center_lead = (((indentation.max(2.0).floor() - 1.0) * 0.5).floor()).max(0.0);
            let center_y = (((row_height.max(2.0).floor() - 1.0) * 0.5).floor()).max(0.0);
            let top = base_y.floor();
            let bottom = (base_y + row_height).ceil().max(top + 1.0);

            for level in 0..depth {
                let stroke_x = (base_x + level as f32 * indentation + center_lead).floor();
                if level == depth - 1 {
                    let vertical_bottom = if has_next_sibling {
                        bottom
                    } else {
                        base_y + center_y + 1.0
                    };
                    let _ = list.add_rect(
                        Rect::new(stroke_x, top, 1.0, (vertical_bottom - top).max(1.0)),
                        ink,
                    );
                    continue;
                }
                if ancestor.get(level).copied().unwrap_or(false) {
                    let _ =
                        list.add_rect(Rect::new(stroke_x, top, 1.0, (bottom - top).max(1.0)), ink);
                }
            }
        });
        node.set_draw(Some(draw));
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
        Some(layout)
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        Vec::new()
    }
}

impl View for TreeGuideOverlay {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Top and bottom sixth-scale bands drop before/after; the middle drops inside,
/// matching the Swift `dropPosition(for:frame:)` bands.
fn drop_position(pointer_y: f32, frame: Rect) -> TreeDropPosition {
    let local_y = pointer_y - frame.y;
    let top_band = (frame.height * 0.28).max(6.0);
    let bottom_band_start = frame.height - top_band;
    if local_y <= top_band {
        TreeDropPosition::Before
    } else if local_y >= bottom_band_start {
        TreeDropPosition::After
    } else {
        TreeDropPosition::Inside
    }
}

/// The drop affordance: an insertion rule before/after, or a highlighted row
/// for a drop inside.
fn draw_drop_indicator(
    position: TreeDropPosition,
    frame: Rect,
    accent: Color,
    list: &mut cove_core::DrawList,
) {
    match position {
        TreeDropPosition::Inside => {
            let _ = list.add_rounded_rect(
                Rect::new(
                    frame.x + 1.0,
                    frame.y + 1.0,
                    (frame.width - 2.0).max(2.0),
                    (frame.height - 2.0).max(2.0),
                ),
                4.0,
                accent.multiplied_alpha(0.12),
            );
            let _ = list.add_rounded_rect_stroke(
                Rect::new(
                    frame.x + 1.0,
                    frame.y + 1.0,
                    (frame.width - 2.0).max(2.0),
                    (frame.height - 2.0).max(2.0),
                ),
                4.0,
                1.0,
                accent.multiplied_alpha(0.9),
            );
        }
        TreeDropPosition::Before => {
            let _ = list.add_rect(
                Rect::new(frame.x, frame.y + 1.0, frame.width, 2.0),
                accent.multiplied_alpha(0.95),
            );
            let _ = list.add_rect(Rect::new(frame.x, frame.y, 6.0, 4.0), accent);
        }
        TreeDropPosition::After => {
            let y = frame.y + frame.height - 3.0;
            let _ = list.add_rect(
                Rect::new(frame.x, y, frame.width, 2.0),
                accent.multiplied_alpha(0.95),
            );
            let _ = list.add_rect(Rect::new(frame.x, y - 1.0, 6.0, 4.0), accent);
        }
    }
}

/// Rows registered for drag hit-testing, keyed by their row token. The row
/// itself is a weak handle so a released row drops out on the next lookup.
struct TreeDragRegistry<Id: std::hash::Hash + Eq> {
    entries: RefCell<Vec<(NodeWeak, TreeNodeKey<Id>)>>,
}

impl<Id: std::hash::Hash + Eq + Clone> Default for TreeDragRegistry<Id> {
    fn default() -> Self {
        Self {
            entries: RefCell::new(Vec::new()),
        }
    }
}

impl<Id: std::hash::Hash + Eq + Clone> TreeDragRegistry<Id> {
    fn register(&self, node: &Node, key: TreeNodeKey<Id>) {
        let mut entries = self.entries.borrow_mut();
        let weak = node.downgrade();
        if let Some(existing) = entries.iter_mut().find(|(other, _)| {
            other.strong_count() > 0 && Node::upgrade(other).as_ref() == Some(node)
        }) {
            *existing = (weak, key);
        } else {
            entries.push((weak, key));
        }
    }

    /// The row token under `point`, with the row's absolute frame extended left
    /// by its indent gutter so a drop over indented columns still resolves a
    /// target.
    fn hit(&self, point: Point) -> Option<(TreeNodeKey<Id>, Rect)> {
        let entries = self.entries.borrow();
        let mut found = None;
        for (weak, key) in entries.iter() {
            let Some(node) = Node::upgrade(weak) else {
                continue;
            };
            let frame = node.absolute_frame();
            let gutter = node.attachment::<f32>(ROW_GUTTER).unwrap_or(0.0);
            let expanded = Rect::new(
                frame.x - gutter,
                frame.y,
                frame.width + gutter,
                frame.height,
            );
            if expanded.contains(point) {
                found = Some((key.clone(), expanded));
            }
        }
        found
    }
}
