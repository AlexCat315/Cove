//! Shared tree types and configuration.
//!
//! The small supporting enums (`TreeSearchFilterPolicy`, `TreeDropPosition`,
//! `TreeNodeKey`) are shared with the projection, so they live here rather than
//! inside the view.
pub(crate) mod check;
pub mod options;
pub mod reparent;
pub mod view;

pub use options::{
    TreeCheck, TreeDragOperations, TreeEdit, TreeEvents, TreeExpansion, TreeGuideStyle, TreeLayout,
    TreeLazy, TreeOptions, TreeSearch, TreeSearchMatch, TreeSelection, TreeSlots,
};
pub use reparent::TreeReparentPlan;
pub use view::{Tree, TreeEntry, TreeSearchMetadata, flatten_tree, flatten_tree_matching};

/// How a search box affects the tree.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TreeSearchFilterPolicy {
    /// Keep every row mounted; only mark the matches.
    HighlightOnly,
    /// Hide non-matches and expand the ancestors of every match.
    #[default]
    FilterAndAutoExpand,
}

/// Where a programmatic scroll places the target row. Virtualized stacks use
/// the exact alignment; the eager scroll view clamps the resulting offset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TreeScrollAlign {
    /// Reveal with the least movement.
    #[default]
    Auto,
    Start,
    Center,
    End,
}

/// Where a dragged row lands relative to the row under the pointer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TreeDropPosition {
    #[default]
    Before,
    /// Become a child of the target row.
    Inside,
    After,
}

/// Stable identity of a row: its model id plus the index path that locates it,
/// so the same id appearing in two branches stays distinct.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TreeNodeKey<ID: std::hash::Hash + Eq> {
    pub id: ID,
    pub path: Vec<usize>,
}

impl<ID: std::hash::Hash + Eq> TreeNodeKey<ID> {
    pub fn new(id: ID, path: Vec<usize>) -> Self {
        Self { id, path }
    }
}
