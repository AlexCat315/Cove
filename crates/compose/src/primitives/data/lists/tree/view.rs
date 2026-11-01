//! Translation of Primitives/Data/Lists/Tree.swift.
//!
//! Hierarchical, single-selection tree. Visual chrome (selection fill,
//! indentation, disclosure chevron) is delegated to the active `TreeRowStyle`;
//! the default is `DefaultTreeRowStyle`.
//!
//! The configuration groups live in [`tree::options`], the flattening and
//! keyboard rules here. Behaviour checklist:
//! - only expanded branches are walked, so a collapsed subtree costs nothing
//! - paths are identity tokens for selection, expansion and drag state;
//!   filtering must not renumber them, so callers' keys stay valid while
//!   search merely hides non-matching siblings
//! - a cheap branch predicate lets a synchronous provider defer child
//!   construction until expansion
//! - searching filters and auto-expands ancestors of every match
//! - Down/Up move by one visible row; Left collapses or selects the parent;
//!   Right expands or selects the first child; Alt+Left/Right collapse/expand
//!   everything
//! - Shift extends from the anchor, Cmd/Ctrl toggles, anything else replaces
//! - indent guides draw a branch line per depth with a continuation column
//!   only where an ancestor still has a following sibling

use crate::Scope;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use super::options::{TreeOptions, TreeSelection};
use super::{TreeDropPosition, TreeNodeKey, TreeSearchFilterPolicy};
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::AppearanceExt;
use crate::primitives::controls::Button;
use crate::primitives::layout::virtualization::FixedVirtualStack;
use crate::primitives::media::{Icon, ui_icon_catalog};
use crate::primitives::overlays::selection_key_host::SelectionKeyHost;
use crate::theme::styles::buttons::ButtonStyleViewExt;
use crate::theme::styles::components::{TreeRowStyleConfiguration, TreeRowStyleEnvironment};
use crate::theme::{SemanticColorRef, theme_for_node};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View};
use cove_composition::Scancode;
use cove_core::scene::{
    AccessibilityActions, AccessibilityRole, AccessibilitySemantics, Align, DrawHook, EventPhase,
    EventResult, FlexDirection, HoverPhase, InputHandlerRoute, InteractionRegistryHolder, Justify,
    LayoutNode, Node, PointerPhase, Wrap,
};
use cove_core::{Binding, Color, Rect, State};
use cove_platform::{KeyEvent, KeyModifiers, MouseButton, SystemCursor};

const PRESSED: &str = "__tree_row_pressed";
const HOVERED: &str = "__tree_row_hovered";
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
    pub is_expanded: bool,
}

/// The search match bookkeeping for one projection.
#[derive(Clone, Default)]
pub struct TreeSearchMetadata<Id> {
    pub self_matches: HashMap<Id, bool>,
    pub subtree_matches: HashMap<Id, bool>,
}

/// Flattens the expanded branches of a tree.
///
/// Children of collapsed branches are never visited, so cost follows the
/// visible rows rather than the whole forest. Paths stay keyed to the
/// unfiltered scene so the caller's keys remain valid while search merely
/// hides non-matching siblings.
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
    let query = search
        .map(|(_, query)| query.trim().to_lowercase())
        .unwrap_or_default();
    let filter_active = filters && !query.is_empty() && search.is_some();

    // A blank query matches nothing to mark, and walking it would resolve lazy
    // children the tree has not asked for yet.
    let metadata = search.filter(|_| !query.is_empty()).map(|(text, _)| {
        let mut self_matches = HashMap::new();
        let mut subtree_matches = HashMap::new();
        fn walk<Element, Id: std::hash::Hash + Eq + Clone>(
            node: &Element,
            id_of: &dyn Fn(&Element) -> Id,
            children: &dyn Fn(&Element) -> Vec<Element>,
            has_children: Option<&dyn Fn(&Element) -> bool>,
            text: &dyn Fn(&Element) -> String,
            query: &str,
            self_matches: &mut HashMap<Id, bool>,
            subtree_matches: &mut HashMap<Id, bool>,
        ) -> bool {
            let id = id_of(node);
            let own = text(node).to_lowercase().contains(query);
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
                        text,
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
                text,
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
                is_expanded,
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
        &mut out,
    );
    out
}

/// Hierarchical, single-selection tree.
pub struct Tree<Element, Id, RowContent>
where
    Element: Clone + 'static,
    Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    RowContent: Fn(&Element, bool, bool, usize) -> AnyView + 'static,
{
    pub roots: Vec<Element>,
    pub options: TreeOptions<Element, Id>,
    pub row_content: RowContent,
    id_of: Rc<dyn Fn(&Element) -> Id>,
    children: Rc<dyn Fn(&Element) -> Vec<Element>>,
    has_children: Option<Rc<dyn Fn(&Element) -> bool>>,
    local_expanded: State<HashSet<Id>>,
    hovered: State<Option<TreeNodeKey<Id>>>,
    modifiers: RefCell<KeyModifiers>,
    drag_source: State<Option<TreeNodeKey<Id>>>,
    drag_target: State<Option<(TreeNodeKey<Id>, TreeDropPosition)>>,
}

impl<Element, Id, RowContent> Tree<Element, Id, RowContent>
where
    Element: Clone + 'static,
    Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    RowContent: Fn(&Element, bool, bool, usize) -> AnyView + 'static,
{
    pub fn new(
        roots: Vec<Element>,
        id_of: impl Fn(&Element) -> Id + 'static,
        children: impl Fn(&Element) -> Vec<Element> + 'static,
        has_children: Option<Rc<dyn Fn(&Element) -> bool>>,
        options: TreeOptions<Element, Id>,
        row_content: RowContent,
    ) -> Self {
        let mut options = options;
        options.layout.validate();
        Self {
            roots,
            options,
            row_content,
            id_of: Rc::new(id_of),
            children: Rc::new(children),
            has_children,
            local_expanded: State::new(HashSet::new()),
            hovered: State::new(None),
            modifiers: RefCell::new(KeyModifiers(0)),
            drag_source: State::new(None),
            drag_target: State::new(None),
        }
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
            None => HashSet::new(),
        }
    }

    fn set_expanded(&self, ids: HashSet<Id>, keys: HashSet<TreeNodeKey<Id>>) {
        if let Some(binding) = &self.options.selection.expanded_keys {
            binding.set(keys);
        }
        if let Some(binding) = &self.options.selection.expanded {
            binding.set(ids);
        } else if self.options.selection.expanded_keys.is_none() {
            self.local_expanded.set(ids);
        }
    }

    /// The visible projection for the current expansion and search state.
    pub fn entries(&self) -> Vec<TreeEntry<Element, Id>> {
        let expanded = self.expanded_ids();
        let keys = self.expanded_keys();
        let query = self.options.search.query.clone();
        // The search text always runs, so matches are marked under either
        // policy; only FilterAndAutoExpand also hides non-matches and
        // auto-expands ancestors.
        let search = self
            .options
            .search
            .text
            .as_ref()
            .map(|text| (text.as_ref(), query.as_str()));
        flatten_tree_matching(
            &self.roots,
            &*self.id_of,
            &*self.children,
            self.has_children.as_deref(),
            &expanded,
            &keys,
            search,
            self.options.search.policy == TreeSearchFilterPolicy::FilterAndAutoExpand,
        )
    }

    fn selection_state(&self) -> (HashSet<Id>, HashSet<TreeNodeKey<Id>>) {
        let selection: &TreeSelection<Id> = &self.options.selection;
        if let Some(binding) = &selection.multiple_keys {
            let keys = binding.get();
            let ids = keys.iter().map(|key| key.id.clone()).collect();
            return (ids, keys);
        }
        if let Some(binding) = &selection.multiple {
            return (binding.get(), HashSet::new());
        }
        if let Some(key) = selection.primary_key.get() {
            return (HashSet::from([key.id.clone()]), HashSet::from([key]));
        }
        match selection.primary.get() {
            Some(id) => (HashSet::from([id]), HashSet::new()),
            None => (HashSet::new(), HashSet::new()),
        }
    }

    fn uses_key_selection(&self) -> bool {
        self.options.selection.multiple_keys.is_some()
            || self.options.selection.primary_key.get().is_some()
    }

    /// The row actions, cloned so row closures do not borrow `self`.
    fn session_actions(&self) -> TreeActions<Element, Id> {
        TreeActions {
            entries: Rc::new(RefCell::new(self.entries())),
            expanded: self.local_expanded.clone(),
            expanded_keys: self.options.selection.expanded_keys.clone(),
            selection: TreeSelection {
                primary: self.options.selection.primary.clone(),
                multiple: self.options.selection.multiple.clone(),
                expanded: self.options.selection.expanded.clone(),
                primary_key: self.options.selection.primary_key.clone(),
                multiple_keys: self.options.selection.multiple_keys.clone(),
                expanded_keys: self.options.selection.expanded_keys.clone(),
            },
            events: super::options::TreeEvents {
                on_key_command: self.options.events.on_key_command.clone(),
                on_select: self.options.events.on_select.clone(),
            },
            modifiers: Rc::new(RefCell::new(*self.modifiers.borrow())),
            roots: Rc::new(self.roots.clone()),
            id_of: self.id_of.clone(),
            children: self.children.clone(),
            has_children: self.has_children.clone(),
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

fn ids_between<Element, Id>(a: &Id, b: &Id, entries: &[TreeEntry<Element, Id>]) -> HashSet<Id>
where
    Element: Clone,
    Id: std::hash::Hash + Eq + Clone,
{
    let Some(ia) = entries.iter().position(|entry| &entry.id == a) else {
        return HashSet::new();
    };
    let Some(ib) = entries.iter().position(|entry| &entry.id == b) else {
        return HashSet::new();
    };
    let (lower, upper) = if ia <= ib { (ia, ib) } else { (ib, ia) };
    entries[lower..=upper]
        .iter()
        .map(|entry| entry.id.clone())
        .collect()
}

impl<Element, Id, RowContent> View for Tree<Element, Id, RowContent>
where
    Element: Clone + 'static,
    Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    RowContent: Fn(&Element, bool, bool, usize) -> AnyView + 'static,
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
                let id = entry.id.clone();
                let has_next_sibling = entry.has_next_sibling;
                let ancestor = entry.ancestor_has_next_siblings.clone();
                let content =
                    (self.row_content)(&entry.element, is_selected, entry.is_expanded, entry.depth);

                let drop_position = drag_target
                    .as_ref()
                    .filter(|(target, _)| target == &key)
                    .map(|(_, position)| *position);
                let is_drag_source = drag_source.as_ref() == Some(&key);

                let on_toggle: Rc<dyn Fn()> = {
                    let entry = entry.clone();
                    let tree = self.session_actions();
                    Rc::new(move || tree.toggle(&entry))
                };
                let on_select: Rc<dyn Fn(KeyModifiers)> = {
                    let entry = entry.clone();
                    let tree = self.session_actions();
                    Rc::new(move |modifiers| tree.select(&entry, modifiers))
                };
                let on_move: Rc<dyn Fn(i64)> = {
                    let entry = entry.clone();
                    let tree = self.session_actions();
                    Rc::new(move |delta| tree.step(&entry, delta))
                };
                let on_collapse: Rc<dyn Fn()> = {
                    let entry = entry.clone();
                    let tree = self.session_actions();
                    Rc::new(move || tree.collapse(&entry))
                };
                let on_expand: Rc<dyn Fn()> = {
                    let entry = entry.clone();
                    let tree = self.session_actions();
                    Rc::new(move || tree.expand(&entry))
                };
                let on_hover: Rc<dyn Fn(bool)> = {
                    let hovered = hovered.clone();
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
                                hovered.get().as_ref() == Some(&entry.key),
                                entry.depth,
                            ))
                            .padding_edges(4.0, 0.0)
                            .frame_width(trailing_width),
                        )
                        .clipped(),
                    ))
                });
                let _ = id;

                AnyView::new(TreeRow {
                    is_enabled,
                    has_children: entry.has_children,
                    is_expanded: entry.is_expanded,
                    is_search_hit: entry.is_search_hit,
                    is_selected,
                    is_hovered: hovered.get().as_ref() == Some(&entry.key),
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
                    trailing_slot: trailing.clone(),
                    on_toggle,
                    on_select,
                    on_move,
                    on_collapse,
                    on_expand,
                    on_hover,
                })
            })
            .collect();

        let ids: Vec<TreeNodeKey<Id>> = entries.iter().map(|entry| entry.key.clone()).collect();
        let scroll_to = entries.iter().position(|entry| {
            if uses_key_selection {
                selected_keys.contains(&entry.key)
            } else {
                selected_ids.contains(&entry.id)
            }
        });

        let mut stack = FixedVirtualStack::new(ids, layout.row_height, {
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
            stack = stack.scroll_to_index(index);
        }

        let tree = AnyView::new(
            AnyView::new(stack)
                .frame(None, None, Some(0.0), Some(0.0), None, None)
                .flex_with(1.0, 1.0, Some(0.0)),
        );

        let host = SelectionKeyHost::new(tree, is_enabled, {
            let actions = self.session_actions();
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
        vec![crate::DynamicField::state(
            "expansion",
            &self.local_expanded,
        )]
    }
}

/// The tree's actions, cloned so row closures can call them without borrowing
/// the view for the node's lifetime.
struct TreeActions<Element, Id>
where
    Element: Clone + 'static,
    Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
{
    entries: Rc<RefCell<Vec<TreeEntry<Element, Id>>>>,
    expanded: State<HashSet<Id>>,
    expanded_keys: Option<Binding<HashSet<TreeNodeKey<Id>>>>,
    selection: TreeSelection<Id>,
    events: super::options::TreeEvents<Element, Id>,
    modifiers: Rc<RefCell<KeyModifiers>>,
    roots: Rc<Vec<Element>>,
    id_of: Rc<dyn Fn(&Element) -> Id>,
    children: Rc<dyn Fn(&Element) -> Vec<Element>>,
    has_children: Option<Rc<dyn Fn(&Element) -> bool>>,
}

impl<Element, Id> Clone for TreeActions<Element, Id>
where
    Element: Clone + 'static,
    Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            entries: self.entries.clone(),
            expanded: self.expanded.clone(),
            expanded_keys: self.expanded_keys.clone(),
            selection: TreeSelection {
                primary: self.selection.primary.clone(),
                multiple: self.selection.multiple.clone(),
                expanded: self.selection.expanded.clone(),
                primary_key: self.selection.primary_key.clone(),
                multiple_keys: self.selection.multiple_keys.clone(),
                expanded_keys: self.selection.expanded_keys.clone(),
            },
            events: super::options::TreeEvents {
                on_key_command: self.events.on_key_command.clone(),
                on_select: self.events.on_select.clone(),
            },
            modifiers: self.modifiers.clone(),
            roots: self.roots.clone(),
            id_of: self.id_of.clone(),
            children: self.children.clone(),
            has_children: self.has_children.clone(),
        }
    }
}

impl<Element, Id> TreeActions<Element, Id>
where
    Element: Clone + 'static,
    Id: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
{
    fn toggle(&self, entry: &TreeEntry<Element, Id>) {
        if let Some(binding) = &self.selection.expanded_keys {
            let mut next = binding.get();
            if !next.remove(&entry.key) {
                next.insert(entry.key.clone());
            }
            binding.set(next.clone());
            if let Some(expanded) = &self.selection.expanded {
                expanded.set(next.iter().map(|key| key.id.clone()).collect());
            }
            return;
        }
        let mut next = match &self.selection.expanded {
            Some(binding) => binding.get(),
            None => self.expanded.get(),
        };
        if !next.remove(&entry.id) {
            next.insert(entry.id.clone());
        }
        match &self.selection.expanded {
            Some(binding) => binding.set(next),
            None => self.expanded.set(next),
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

    fn set_expanded(&self, ids: HashSet<Id>, keys: HashSet<TreeNodeKey<Id>>) {
        if let Some(binding) = &self.selection.expanded_keys {
            binding.set(keys.clone());
        }
        if let Some(binding) = &self.selection.expanded {
            binding.set(ids.clone());
        } else if self.selection.expanded_keys.is_none() {
            self.expanded.set(ids);
        }
    }

    fn select(&self, entry: &TreeEntry<Element, Id>, modifiers: KeyModifiers) {
        *self.modifiers.borrow_mut() = modifiers;
        let entries = self.entries.borrow();
        let single = |entry: &TreeEntry<Element, Id>| {
            if let Some(binding) = &self.selection.multiple_keys {
                binding.set(HashSet::from([entry.key.clone()]));
            }
            if let Some(binding) = &self.selection.multiple {
                binding.set(HashSet::from([entry.id.clone()]));
            }
            self.selection.primary.set(Some(entry.id.clone()));
            self.selection.primary_key.set(Some(entry.key.clone()));
        };
        if self.selection.multiple.is_none() && self.selection.multiple_keys.is_none() {
            single(entry);
            if let Some(handler) = &self.events.on_select {
                handler(entry.element.clone());
            }
            return;
        }
        let extending = modifiers.has_shift();
        let toggling = modifiers.has_gui() || modifiers.has_ctrl();
        if let Some(binding) = &self.selection.multiple_keys {
            let mut next = binding.get();
            if extending {
                let anchor = self
                    .selection
                    .primary_key
                    .get()
                    .unwrap_or_else(|| entry.key.clone());
                let keys = keys_between(&anchor, &entry.key, &entries);
                next = if keys.is_empty() {
                    HashSet::from([entry.key.clone()])
                } else {
                    keys
                };
            } else if toggling {
                if !next.remove(&entry.key) {
                    next.insert(entry.key.clone());
                }
            } else {
                next = HashSet::from([entry.key.clone()]);
            }
            binding.set(next);
        }
        if let Some(binding) = &self.selection.multiple {
            let mut next = binding.get();
            if extending {
                let anchor = self
                    .selection
                    .primary
                    .get()
                    .unwrap_or_else(|| entry.id.clone());
                let ids = ids_between(&anchor, &entry.id, &entries);
                next = if ids.is_empty() {
                    HashSet::from([entry.id.clone()])
                } else {
                    ids
                };
            } else if toggling {
                if !next.remove(&entry.id) {
                    next.insert(entry.id.clone());
                }
            } else {
                next = HashSet::from([entry.id.clone()]);
            }
            binding.set(next);
        }
        self.selection.primary.set(Some(entry.id.clone()));
        self.selection.primary_key.set(Some(entry.key.clone()));
        if let Some(handler) = &self.events.on_select {
            handler(entry.element.clone());
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
        let current = self
            .selection
            .primary_key
            .get()
            .and_then(|key| entries.iter().find(|entry| entry.key == key).cloned())
            .or_else(|| {
                self.selection
                    .primary
                    .get()
                    .and_then(|id| entries.iter().find(|entry| entry.id == id).cloned())
            })
            .or_else(|| entries.first().cloned());
        let Some(current) = current else { return false };
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
            Scancode::RETURN | Scancode::KEYPAD_ENTER | Scancode::SPACE => {
                self.select(&current, event.modifiers);
                true
            }
            _ => false,
        }
    }
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
    trailing_slot: Option<AnyView>,
    on_toggle: Rc<dyn Fn()>,
    on_select: Rc<dyn Fn(KeyModifiers)>,
    on_move: Rc<dyn Fn(i64)>,
    on_collapse: Rc<dyn Fn()>,
    on_expand: Rc<dyn Fn()>,
    on_hover: Rc<dyn Fn(bool)>,
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
            state: cove_core::scene::AccessibilityState {
                is_enabled: self.is_enabled,
                is_selected: self.is_selected,
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
        let paint: Rc<dyn Fn()> = Rc::new(move || {
            let hovered = paint_node.attachment::<bool>(HOVERED) == Some(true);
            let theme = theme_for_node(&paint_node);
            paint_node.set_background_color(Some(if hovered && !is_selected && !is_search_hit {
                theme.colors.state_layer_hover
            } else {
                Color::CLEAR
            }));
        });
        node.set_attachment(HOVERED, self.is_hovered);
        paint();

        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };

        let hover_paint = paint.clone();
        let hover_action = self.on_hover.clone();
        let hover_enabled = self.is_enabled;
        registry.set_hover(
            node,
            Rc::new(move |phase: HoverPhase| {
                if !hover_enabled {
                    return;
                }
                hover_action(phase == HoverPhase::Enter);
                hover_paint();
            }),
        );

        let pointer_select = self.on_select.clone();
        let pointer_node = node.clone();
        registry.set_pointer(
            node,
            InputHandlerRoute::control(),
            Rc::new(
                move |event: &cove_platform::MouseButtonEvent,
                      phase: PointerPhase,
                      delivery: EventPhase| {
                    if delivery == EventPhase::Capture || event.button != MouseButton::Left {
                        return EventResult::Ignored;
                    }
                    match phase {
                        PointerPhase::Down => {
                            pointer_node.set_attachment(PRESSED, true);
                            EventResult::Handled
                        }
                        PointerPhase::Up => {
                            let was = pointer_node.attachment::<bool>(PRESSED) == Some(true);
                            pointer_node.set_attachment(PRESSED, false);
                            if was {
                                pointer_select(event.modifiers);
                                return EventResult::Handled;
                            }
                            EventResult::Ignored
                        }
                    }
                },
            ),
        );

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
            is_enabled: self.is_enabled,
            theme: theme_for_node(node),
        };
        let body = style.make_body(configuration);

        let disclosure = if self.has_children {
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
                .accessibility(move |semantics: &mut AccessibilitySemantics| {
                    semantics.label = label.to_string();
                }),
            )
        } else {
            AnyView::new(
                AnyView::new(crate::EmptyView).frame_size(self.disclosure_width, row_height),
            )
        };

        let mut row = vec![
            AnyView::new(AnyView::new(crate::EmptyView).frame_size(indent.max(0.0), row_height)),
            disclosure,
            AnyView::new(
                AnyView::new(BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    body,
                ))
                .flex_with(1.0, 1.0, Some(0.0))
                .frame(None, None, Some(0.0), None, None, None),
            ),
        ];

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

        let row = AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(row)),
            ))
            .frame_height(row_height),
        );

        match guides {
            Some(guides) => vec![AnyView::new(crate::primitives::layout::ZStack::new(
                crate::Alignment::Leading,
                AnyView::new(TupleView::new(vec![AnyView::new(guides), row])),
            ))],
            None => vec![row],
        }
    }
}

impl TreeRow {
    fn on_key_command(&self) -> Rc<dyn Fn(&KeyEvent) -> bool> {
        let select = self.on_select.clone();
        let move_key = self.on_move.clone();
        let collapse = self.on_collapse.clone();
        let expand = self.on_expand.clone();
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
                Scancode::RETURN | Scancode::KEYPAD_ENTER | Scancode::SPACE => {
                    select(event.modifiers);
                    true
                }
                _ => false,
            }
        })
    }
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
