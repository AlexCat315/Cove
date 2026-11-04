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

use super::super::selection::{CollectionId, RowState};
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
    flatten_visible(
        roots,
        id_of,
        children,
        has_children,
        expanded,
        expanded_keys,
        search,
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
    search: Option<(&dyn Fn(&Element) -> String, &str)>,
    filters: bool,
    mut seen: Option<&mut HashSet<Id>>,
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

/// Hierarchical, single-selection tree.
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
        self
    }

    /// A cheap branch predicate, so a synchronous provider can defer child
    /// construction until expansion.
    pub fn has_children(mut self, has_children: impl Fn(&Element) -> bool + 'static) -> Self {
        self.has_children = Some(Rc::new(has_children));
        self
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
        TreeActions {
            entries: Rc::new(RefCell::new(self.entries())),
            expanded: self.local_expanded.clone(),
            expanded_keys: self.options.selection.expanded_keys.clone(),
            selection: self.options.selection.clone(),
            anchor: self.anchor.clone(),
            cursor: self.cursor.clone(),
            events: super::options::TreeEvents {
                on_key_command: self.options.events.on_key_command.clone(),
                on_select: self.options.events.on_select.clone(),
            },
            modifiers: Rc::new(RefCell::new(*self.modifiers.borrow())),
            roots: Rc::new(self.roots.clone()),
            id_of: self.id_of.clone(),
            children: self.children.clone(),
            has_children: self.has_children.clone(),
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
                let content = (self.row_content)(
                    &entry.element,
                    RowState {
                        is_selected,
                        is_expanded: entry.is_expanded,
                        is_dragging: false,
                        depth: entry.depth,
                    },
                );

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
                let row_drag = self.options.drag.on_drop.as_ref().map(|_| {
                    let tree = self.session_actions();
                    let registry = self.drag_registry.clone();
                    let entry = entry.clone();
                    TreeRowDrag {
                        register: {
                            let key = key.clone();
                            let registry = registry.clone();
                            Rc::new(move |node: &Node| registry.register(node, key.clone()))
                        },
                        on_start: {
                            let key = key.clone();
                            let tree = tree.clone();
                            Rc::new(move || tree.drag_source.set(Some(key.clone())))
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
                    drag: row_drag,
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
        vec![
            crate::DynamicField::state("expansion", &self.local_expanded),
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
    expanded_keys: Option<Binding<HashSet<TreeNodeKey<Id>>>>,
    selection: TreeSelection<Id>,
    anchor: State<Option<TreeNodeKey<Id>>>,
    cursor: State<Option<TreeNodeKey<Id>>>,
    events: super::options::TreeEvents<Element, Id>,
    modifiers: Rc<RefCell<KeyModifiers>>,
    roots: Rc<Vec<Element>>,
    id_of: Rc<dyn Fn(&Element) -> Id>,
    children: Rc<dyn Fn(&Element) -> Vec<Element>>,
    has_children: Option<Rc<dyn Fn(&Element) -> bool>>,
    drag: super::options::TreeDragOperations<Element>,
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
            expanded_keys: self.expanded_keys.clone(),
            selection: self.selection.clone(),
            anchor: self.anchor.clone(),
            cursor: self.cursor.clone(),
            events: super::options::TreeEvents {
                on_key_command: self.events.on_key_command.clone(),
                on_select: self.events.on_select.clone(),
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

    fn allows_multiple(&self) -> bool {
        self.selection.ids.allows_multiple()
            || self
                .selection
                .keys
                .as_ref()
                .is_some_and(|keys| keys.allows_multiple())
    }

    fn select(&self, entry: &TreeEntry<Element, Id>, modifiers: KeyModifiers) {
        *self.modifiers.borrow_mut() = modifiers;
        let multiple = self.allows_multiple();
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
        self.cursor.set(Some(entry.key.clone()));
        if let Some(handler) = &self.events.on_select {
            handler(entry.element.clone());
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
        let target = registry.hit(Point::new(x, y));
        let entries = self.entries.borrow();
        let source_entry = entries.iter().find(|entry| &entry.key == source).cloned();
        let target_entry = target
            .as_ref()
            .and_then(|(key, _)| entries.iter().find(|entry| &entry.key == key).cloned());
        drop(entries);
        let (Some(source_entry), Some((target_key, frame))) = (source_entry, target) else {
            self.drag_target.set(None);
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
        self.drag_target.set(Some((target_key, position)));
    }

    /// Reports the drop for the current target, then clears the session. A
    /// release with no target reports nothing.
    fn commit_drag(&self, source: &TreeEntry<Element, Id>) {
        let target = self.drag_target.get();
        self.drag_source.set(None);
        self.drag_target.set(None);
        let Some((target_key, position)) = target else {
            return;
        };
        let entries = self.entries.borrow();
        let target_entry = entries
            .iter()
            .find(|entry| entry.key == target_key)
            .cloned();
        drop(entries);
        let Some(target_entry) = target_entry else {
            return;
        };
        if let Some(on_drop) = &self.drag.on_drop {
            on_drop(
                source.element.clone(),
                target_entry.element.clone(),
                position,
            );
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
        // Single selection follows the binding, so an outside write moves the
        // keyboard too.
        let by_cursor = self
            .cursor
            .get()
            .and_then(|key| entries.iter().find(|entry| entry.key == key).cloned());
        let by_selection = self
            .selection
            .keys
            .as_ref()
            .and_then(|keys| keys.first())
            .and_then(|key| entries.iter().find(|entry| entry.key == key).cloned())
            .or_else(|| {
                self.selection
                    .ids
                    .first()
                    .and_then(|id| entries.iter().find(|entry| entry.id == id).cloned())
            });
        let current = if self.allows_multiple() {
            by_cursor.or(by_selection)
        } else {
            by_selection.or(by_cursor)
        }
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
    /// The armed drag surface, `None` when `on_drop` is unset.
    drag: Option<TreeRowDrag>,
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
            paint_node.set_corner_radius(theme.radius.sm);
            paint_node.set_background_color(Some(if hovered && !is_selected && !is_search_hit {
                theme.colors.state_layer_hover
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
                                    capture.acquire(&pointer_node, false);
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
            Some(guides) => vec![AnyView::new(
                crate::primitives::layout::ZStack::new(AnyView::new(TupleView::new(vec![
                    AnyView::new(guides),
                    row,
                ])))
                .alignment(crate::Alignment::Leading),
            )],
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
