//! Translation of the configuration groups in Primitives/Data/Lists/TreeOptions.swift.
//!
//! The groups mirror the capability families of the reference tree APIs:
//!
//! - [`TreeSelection`] — node selection (single or multiple, controlled or local)
//! - [`TreeCheck`] — checkbox state with optional parent/child cascade
//! - [`TreeExpansion`] — default expansion, auto-expanded parents and accordion
//! - [`TreeLazy`] — lazy child loading and loaded-node bookkeeping
//! - [`TreeEdit`] — structural edits delegated to the caller's model
//! - [`TreeSearch`] — filtering, matching rules and custom predicates
//! - [`TreeDragOperations`] — drop permissions plus the drag lifecycle
use std::collections::HashSet;
use std::rc::Rc;

use cove_composition::AnyView;
use cove_core::{Binding, Point};
use cove_platform::{KeyEvent, KeyModifiers};

use super::super::selection::{CollectionId, Selection};
use super::{TreeDropPosition, TreeNodeKey};
use crate::theme::TextScaler;

/// Caller-owned selection and expansion, independent of transient pointer/drag state.
pub struct TreeSelection<ID: CollectionId> {
    /// The selected rows by ID.
    pub ids: Selection<ID>,
    pub expanded: Option<Binding<HashSet<ID>>>,
    /// The selected rows by path, for a hierarchy that repeats IDs. When present
    /// it decides which occurrences are selected.
    pub keys: Option<Selection<TreeNodeKey<ID>>>,
    pub expanded_keys: Option<Binding<HashSet<TreeNodeKey<ID>>>>,
    /// Whether tapping a row selects it. When false the row still moves the
    /// keyboard cursor, so an explicit check-only tree stays navigable.
    pub is_selectable: bool,
    /// Use a tree-owned multiple selection when no `ids`/`keys` binding is
    /// supplied. The initial value comes from `default_selected`.
    pub is_local: bool,
    pub local_multiple: bool,
    /// IDs seeded into the local selection the first time the tree is built.
    pub default_selected: Vec<ID>,
    /// IDs seeded into tree-owned expansion when no binding is supplied.
    pub default_expanded: Vec<ID>,
    /// Path keys seeded into tree-owned expansion.
    pub default_expanded_keys: Vec<TreeNodeKey<ID>>,
}

impl<ID: CollectionId> Default for TreeSelection<ID> {
    fn default() -> Self {
        Self {
            ids: Selection::none(),
            expanded: None,
            keys: None,
            expanded_keys: None,
            is_selectable: true,
            is_local: false,
            local_multiple: false,
            default_selected: Vec::new(),
            default_expanded: Vec::new(),
            default_expanded_keys: Vec::new(),
        }
    }
}

impl<ID: CollectionId> Clone for TreeSelection<ID> {
    fn clone(&self) -> Self {
        Self {
            ids: self.ids.clone(),
            expanded: self.expanded.clone(),
            keys: self.keys.clone(),
            expanded_keys: self.expanded_keys.clone(),
            is_selectable: self.is_selectable,
            is_local: self.is_local,
            local_multiple: self.local_multiple,
            default_selected: self.default_selected.clone(),
            default_expanded: self.default_expanded.clone(),
            default_expanded_keys: self.default_expanded_keys.clone(),
        }
    }
}

impl<ID: CollectionId> TreeSelection<ID> {
    /// A tree-owned selection: no binding, state survives recomposition.
    pub fn local(default_selected: impl IntoIterator<Item = ID>) -> Self {
        Self {
            is_local: true,
            default_selected: default_selected.into_iter().collect(),
            ..Self::default()
        }
    }

    /// A tree-owned multiple selection.
    pub fn local_multiple(default_selected: impl IntoIterator<Item = ID>) -> Self {
        Self {
            is_local: true,
            local_multiple: true,
            default_selected: default_selected.into_iter().collect(),
            ..Self::default()
        }
    }

    /// Turns selection off while leaving checkboxes and expansion active.
    pub fn disabled(mut self) -> Self {
        self.is_selectable = false;
        self
    }
}

/// Checkbox state. Cascade is on by default; `check_strictly` makes every node
/// independent, as the reference implementations do.
pub struct TreeCheck<Element: Clone + 'static, ID: CollectionId> {
    pub is_enabled: bool,
    /// Fully checked rows by ID.
    pub checked: Option<Binding<HashSet<ID>>>,
    /// Fully checked rows by path, for repeated IDs.
    pub checked_keys: Option<Binding<HashSet<TreeNodeKey<ID>>>>,
    /// IDs seeded into the tree-owned check state.
    pub default_checked: Vec<ID>,
    /// Disable parent/child cascade.
    pub check_strictly: bool,
    /// Receives `(node, is_checked, checked_keys, half_checked_keys)`.
    pub on_change:
        Option<Rc<dyn Fn(&Element, bool, &HashSet<TreeNodeKey<ID>>, &HashSet<TreeNodeKey<ID>>)>>,
}

impl<Element: Clone + 'static, ID: CollectionId> Default for TreeCheck<Element, ID> {
    fn default() -> Self {
        Self {
            is_enabled: false,
            checked: None,
            checked_keys: None,
            default_checked: Vec::new(),
            check_strictly: false,
            on_change: None,
        }
    }
}

impl<Element: Clone + 'static, ID: CollectionId> Clone for TreeCheck<Element, ID> {
    fn clone(&self) -> Self {
        Self {
            is_enabled: self.is_enabled,
            checked: self.checked.clone(),
            checked_keys: self.checked_keys.clone(),
            default_checked: self.default_checked.clone(),
            check_strictly: self.check_strictly,
            on_change: self.on_change.clone(),
        }
    }
}

/// Expansion policy beyond the explicit `expanded` binding.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TreeExpansion {
    /// Expand every branch when the tree mounts.
    pub default_expand_all: bool,
    /// Add every ancestor of a newly revealed, selected or scrolled-to row.
    pub auto_expand_parent: bool,
    /// Only one sibling stays open at each level.
    pub accordion: bool,
}

/// Lazy child resolution.
///
/// `load_data` runs once per node the first time it expands; the host loads the
/// children into its own model and records the node in `loaded_keys`. The
/// optional `is_loading` predicate drives the row's progress ring, and
/// `is_leaf` marks explicit leaves so a not-yet-loaded branch is still drawn
/// with a disclosure control.
pub struct TreeLazy<Element: Clone + 'static, ID: CollectionId> {
    pub is_enabled: bool,
    pub is_leaf: Option<Rc<dyn Fn(&Element) -> bool>>,
    pub load_data: Option<Rc<dyn Fn(&Element)>>,
    pub loaded_keys: Option<Binding<HashSet<TreeNodeKey<ID>>>>,
    pub is_loading: Option<Rc<dyn Fn(&Element) -> bool>>,
}

impl<Element: Clone + 'static, ID: CollectionId> Default for TreeLazy<Element, ID> {
    fn default() -> Self {
        Self {
            is_enabled: true,
            is_leaf: None,
            load_data: None,
            loaded_keys: None,
            is_loading: None,
        }
    }
}

impl<Element: Clone + 'static, ID: CollectionId> Clone for TreeLazy<Element, ID> {
    fn clone(&self) -> Self {
        Self {
            is_enabled: self.is_enabled,
            is_leaf: self.is_leaf.clone(),
            load_data: self.load_data.clone(),
            loaded_keys: self.loaded_keys.clone(),
            is_loading: self.is_loading.clone(),
        }
    }
}

/// Structural edits are delegated to the caller's model: the tree has no
/// ownership of the node storage. Leaving a callback empty makes the matching
/// imperative method a no-op.
pub struct TreeEdit<Element: Clone + 'static> {
    /// `(parent, child)`; a `None` parent appends at the root.
    pub on_append: Option<Rc<dyn Fn(Option<&Element>, &Element)>>,
    pub on_insert_before: Option<Rc<dyn Fn(&Element, &Element)>>,
    pub on_insert_after: Option<Rc<dyn Fn(&Element, &Element)>>,
    pub on_remove: Option<Rc<dyn Fn(&Element)>>,
    pub on_update_children: Option<Rc<dyn Fn(&Element, Vec<Element>)>>,
}

impl<Element: Clone + 'static> Default for TreeEdit<Element> {
    fn default() -> Self {
        Self {
            on_append: None,
            on_insert_before: None,
            on_insert_after: None,
            on_remove: None,
            on_update_children: None,
        }
    }
}

impl<Element: Clone + 'static> Clone for TreeEdit<Element> {
    fn clone(&self) -> Self {
        Self {
            on_append: self.on_append.clone(),
            on_insert_before: self.on_insert_before.clone(),
            on_insert_after: self.on_insert_after.clone(),
            on_remove: self.on_remove.clone(),
            on_update_children: self.on_update_children.clone(),
        }
    }
}

/// How the indent guide columns are drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TreeGuideStyle {
    /// No columns: hierarchy reads from indentation alone.
    None,
    /// One column per level. The deepest column stops at the row's centre when
    /// the row has no following sibling, so a branch closes visually.
    #[default]
    Straight,
    /// Like [`Self::Straight`], except the deepest column bends into the row.
    Elbow,
    /// Straight columns broken into dashes.
    Dashed,
    /// Straight columns broken into dots.
    Dotted,
}

/// Geometry consumed by virtualization, hit testing, guides and disclosure slots.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TreeLayout {
    pub row_height: f32,
    pub row_spacing: f32,
    pub indentation: f32,
    pub disclosure_width: f32,
    /// Which guide columns are drawn.
    pub guide_style: TreeGuideStyle,
    /// Guide stroke width in points, 1.5 by default. It stays unscaled so the
    /// columns remain crisp at every text size.
    pub guide_width: f32,
    /// Where a column sits inside its indent step: `0.0` hugs the step's
    /// leading edge, `0.5` centres it, `1.0` the trailing edge.
    pub guide_position: f32,
    pub trailing_slot_width: f32,
    /// Space before the first level's indent.
    pub leading_padding: f32,
    /// Space after the trailing slot.
    pub trailing_padding: f32,
    /// Fill the row slot with the selection band. When false the band hugs the
    /// row's content, matching `blockNode: false` in the reference APIs.
    pub block_node: bool,
    /// Virtualize rows inside the scroll viewport. Turning it off mounts every
    /// visible row, which is useful when rows need to keep live editors.
    pub virtual_scroll: bool,
}

impl Default for TreeLayout {
    fn default() -> Self {
        Self {
            row_height: 30.0,
            row_spacing: 0.0,
            indentation: 16.0,
            disclosure_width: 18.0,
            guide_style: TreeGuideStyle::Straight,
            guide_width: 1.5,
            guide_position: 0.5,
            trailing_slot_width: 64.0,
            leading_padding: 0.0,
            trailing_padding: 0.0,
            block_node: true,
            virtual_scroll: true,
        }
    }
}

impl TreeLayout {
    /// Draw the vertical indent lines.
    pub fn show_line(mut self, show: bool) -> Self {
        self.guide_style = if show {
            TreeGuideStyle::Straight
        } else {
            TreeGuideStyle::None
        };
        self
    }

    /// Choose the guide column style.
    pub fn guide_style(mut self, style: TreeGuideStyle) -> Self {
        self.guide_style = style;
        self
    }

    /// Guide stroke width in points.
    pub fn guide_width(mut self, width: f32) -> Self {
        self.guide_width = width.clamp(0.5, 4.0);
        self
    }

    /// Position of a guide column inside its indent step, from `0.0` to `1.0`.
    pub fn guide_position(mut self, position: f32) -> Self {
        self.guide_position = position.clamp(0.0, 1.0);
        self
    }

    /// Row height before the platform minimums are applied.
    pub fn row_height(mut self, row_height: f32) -> Self {
        self.row_height = row_height.max(1.0);
        self
    }

    /// Gap between rows.
    pub fn row_spacing(mut self, row_spacing: f32) -> Self {
        self.row_spacing = row_spacing.max(0.0);
        self
    }

    /// Width of the disclosure slot. Zero hides it.
    pub fn disclosure_width(mut self, disclosure_width: f32) -> Self {
        self.disclosure_width = disclosure_width.max(0.0);
        self
    }

    /// Width of one indent step, and the spacing between guide columns.
    pub fn indentation(mut self, indentation: f32) -> Self {
        self.indentation = indentation.max(0.0);
        self
    }

    /// Width reserved for the trailing slot.
    pub fn trailing_slot_width(mut self, trailing_slot_width: f32) -> Self {
        self.trailing_slot_width = trailing_slot_width.max(0.0);
        self
    }

    /// Space before the first level's indent.
    pub fn leading_padding(mut self, padding: f32) -> Self {
        self.leading_padding = padding.max(0.0);
        self
    }

    /// Space after the trailing slot.
    pub fn trailing_padding(mut self, padding: f32) -> Self {
        self.trailing_padding = padding.max(0.0);
        self
    }

    /// Fill the whole row with the selection band.
    pub fn block_node(mut self, block: bool) -> Self {
        self.block_node = block;
        self
    }

    /// Horizontal step per level.
    pub fn indent(mut self, indentation: f32) -> Self {
        self.indentation = indentation.max(0.0);
        self
    }

    /// Virtualize the row stack.
    pub fn virtual_scroll(mut self, virtual_scroll: bool) -> Self {
        self.virtual_scroll = virtual_scroll;
        self
    }
}

/// Inputs to `TreeLayout::resolved` that the platform host supplies.
#[derive(Clone, Copy)]
pub struct TreeLayoutEnvironment {
    /// Small icon edge length.
    pub icon_small: f32,
    /// Icon scaling factor.
    pub icon_factor: f32,
    /// Whether the surface is touch-driven.
    pub is_touch: bool,
    /// Minimum control extent on a touch surface.
    pub minimum_control_extent: f32,
}

impl TreeLayout {
    /// Resolve one geometry so scaling cannot drift between the subsystems.
    pub fn resolved(&self, text_scaler: &TextScaler, environment: TreeLayoutEnvironment) -> Self {
        let mut result = *self;
        result.row_height = text_scaler
            .scale(self.row_height)
            .max(environment.icon_small + 4.0);
        result.row_spacing = text_scaler.scale(self.row_spacing);
        result.indentation = self.indentation * text_scaler.factor().max(environment.icon_factor);
        result.disclosure_width = if self.disclosure_width > 0.0 {
            self.disclosure_width.max(environment.icon_small)
        } else {
            0.0
        };
        result.trailing_slot_width = text_scaler.scale(self.trailing_slot_width);
        result.leading_padding = text_scaler.scale(self.leading_padding);
        result.trailing_padding = text_scaler.scale(self.trailing_padding);
        if environment.is_touch {
            result.row_height = result.row_height.max(environment.minimum_control_extent);
            if result.disclosure_width > 0.0 {
                result.disclosure_width = result
                    .disclosure_width
                    .max(environment.minimum_control_extent);
            }
        }
        result
    }

    pub fn validate(&mut self) {
        self.row_height = self.row_height.max(1.0);
        self.row_spacing = self.row_spacing.max(0.0);
        self.indentation = self.indentation.max(0.0);
        self.disclosure_width = self.disclosure_width.max(0.0);
        self.trailing_slot_width = self.trailing_slot_width.max(0.0);
        self.guide_width = self.guide_width.clamp(0.5, 4.0);
        self.guide_position = self.guide_position.clamp(0.0, 1.0);
        self.leading_padding = self.leading_padding.max(0.0);
        self.trailing_padding = self.trailing_padding.max(0.0);
    }
}

/// Caller-supplied row chrome.
pub struct TreeSlots<Element: 'static> {
    /// Receives whether the row is expanded. Replaces the default chevron.
    pub disclosure: Option<Rc<dyn Fn(bool) -> AnyView>>,
    /// Leading glyph before the title, after the disclosure and checkbox.
    pub icon: Option<Rc<dyn Fn(&Element) -> AnyView>>,
    /// Receives the element, selected/focused/expanded/disabled flags and depth.
    pub trailing: Option<Rc<dyn Fn(Element, bool, bool, bool, bool, usize) -> AnyView>>,
}

impl<Element: 'static> Default for TreeSlots<Element> {
    fn default() -> Self {
        Self {
            disclosure: None,
            icon: None,
            trailing: None,
        }
    }
}

/// How a row's text is compared with the search query.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TreeSearchMatch {
    #[default]
    Contains,
    Prefix,
    Exact,
}

/// Search configuration.
pub struct TreeSearch<Element: 'static> {
    pub query: String,
    pub text: Option<Rc<dyn Fn(&Element) -> String>>,
    pub policy: super::TreeSearchFilterPolicy,
    /// Overrides the text comparison entirely. Receives the node and the
    /// normalised query.
    pub filter: Option<Rc<dyn Fn(&Element, &str) -> bool>>,
    pub ignore_case: bool,
    pub match_mode: TreeSearchMatch,
}

impl<Element: 'static> Default for TreeSearch<Element> {
    fn default() -> Self {
        Self {
            query: String::new(),
            text: None,
            policy: super::TreeSearchFilterPolicy::FilterAndAutoExpand,
            filter: None,
            ignore_case: true,
            match_mode: TreeSearchMatch::Contains,
        }
    }
}

/// Selection, expansion, check and pointer events.
pub struct TreeEvents<Element: 'static, ID: std::hash::Hash + Eq + 'static> {
    pub on_key_command: Option<Rc<dyn Fn(&KeyEvent, &HashSet<ID>) -> bool>>,
    /// Legacy single-argument selection callback.
    pub on_select: Option<Rc<dyn Fn(Element)>>,
    /// Receives `(node, is_selected, selected_keys)`.
    pub on_select_change: Option<Rc<dyn Fn(&Element, bool, &HashSet<TreeNodeKey<ID>>)>>,
    /// Receives `(node, is_expanded, expanded_keys)`.
    pub on_expand_change: Option<Rc<dyn Fn(&Element, bool, &HashSet<TreeNodeKey<ID>>)>>,
    /// Receives `(node, modifiers)` for every pointer tap.
    pub on_node_click: Option<Rc<dyn Fn(&Element, KeyModifiers)>>,
    /// Receives `(node, window_point)`.
    pub on_node_context_menu: Option<Rc<dyn Fn(&Element, Point)>>,
    pub on_double_click: Option<Rc<dyn Fn(&Element)>>,
}

impl<Element: 'static, ID: std::hash::Hash + Eq + 'static> Default for TreeEvents<Element, ID> {
    fn default() -> Self {
        Self {
            on_key_command: None,
            on_select: None,
            on_select_change: None,
            on_expand_change: None,
            on_node_click: None,
            on_node_context_menu: None,
            on_double_click: None,
        }
    }
}

/// Drag-and-drop operations between rows.
#[derive(Clone)]
pub struct TreeDragOperations<Element: Clone + 'static> {
    /// Arm drag handling at all.
    pub is_enabled: bool,
    /// Per-row drag permission.
    pub can_drag: Option<Rc<dyn Fn(&Element) -> bool>>,
    pub can_drop: Option<Rc<dyn Fn(&Element, &Element, TreeDropPosition) -> bool>>,
    pub on_drop: Option<Rc<dyn Fn(Element, Element, TreeDropPosition)>>,
    pub on_drag_start: Option<Rc<dyn Fn(&Element)>>,
    /// Called when the row under the pointer changes. The second element is the
    /// current target.
    pub on_drag_enter: Option<Rc<dyn Fn(&Element, &Element, TreeDropPosition)>>,
    /// Called when the pointer leaves every valid target.
    pub on_drag_leave: Option<Rc<dyn Fn(&Element)>>,
    /// Called on release with the source and the accepted target, if any.
    pub on_drag_end: Option<Rc<dyn Fn(&Element, Option<&Element>)>>,
}

impl<Element: Clone + 'static> Default for TreeDragOperations<Element> {
    fn default() -> Self {
        Self {
            is_enabled: true,
            can_drag: None,
            can_drop: None,
            on_drop: None,
            on_drag_start: None,
            on_drag_enter: None,
            on_drag_leave: None,
            on_drag_end: None,
        }
    }
}

/// Configuration consists of independently reusable groups.
pub struct TreeOptions<Element: Clone + 'static, ID: CollectionId> {
    pub is_enabled: bool,
    pub selection: TreeSelection<ID>,
    pub check: TreeCheck<Element, ID>,
    pub expansion: TreeExpansion,
    pub lazy: TreeLazy<Element, ID>,
    pub edit: TreeEdit<Element>,
    pub layout: TreeLayout,
    pub slots: TreeSlots<Element>,
    pub search: TreeSearch<Element>,
    pub events: TreeEvents<Element, ID>,
    pub drag: TreeDragOperations<Element>,
}

impl<Element: Clone + 'static, ID: CollectionId> Default for TreeOptions<Element, ID> {
    fn default() -> Self {
        Self {
            is_enabled: true,
            selection: TreeSelection::default(),
            check: TreeCheck::default(),
            expansion: TreeExpansion::default(),
            lazy: TreeLazy::default(),
            edit: TreeEdit::default(),
            layout: TreeLayout::default(),
            slots: TreeSlots::default(),
            search: TreeSearch::default(),
            events: TreeEvents::default(),
            drag: TreeDragOperations::default(),
        }
    }
}
