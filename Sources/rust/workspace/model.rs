//! Authored workspace document: panels, tab groups, slots and floating windows.
//!
//! The document is pure data. Nothing here measures, and nothing caches a
//! derived frame, so a decoded document behaves exactly like an authored one.
//!
//! Translation of `Sources/CoveUIWorkspace/Layout/Split.swift`: the
//! `WorkspaceLayoutSplit` address and the `WorkspaceLayoutNode::resizing` walk
//! live in this module.
use super::ids::{
    WorkspaceEdge, WorkspaceFloatingWindowID, WorkspacePanelID, WorkspaceSlotID, WorkspaceSplitAxis,
    WorkspaceTabGroupID,
};
use super::sizing::{WorkspacePanelSizing, WorkspaceRect};
use std::collections::BTreeMap;

/// Recursive layout of a slot. A split is located by its two children; the
/// common ancestor stays stable when hidden branches are compacted.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum WorkspaceLayoutNode {
    Group(WorkspaceTabGroupID),
    Split {
        axis: WorkspaceSplitAxis,
        fraction: f32,
        first: Box<WorkspaceLayoutNode>,
        second: Box<WorkspaceLayoutNode>,
    },
}

impl WorkspaceLayoutNode {
    pub fn group(id: WorkspaceTabGroupID) -> Self {
        Self::Group(id)
    }

    pub fn split(axis: WorkspaceSplitAxis, fraction: f32, first: Self, second: Self) -> Self {
        Self::Split { axis, fraction, first: Box::new(first), second: Box::new(second) }
    }

    pub fn leaf_group_ids(&self) -> Vec<WorkspaceTabGroupID> {
        match self {
            Self::Group(id) => vec![id.clone()],
            Self::Split { first, second, .. } => {
                let mut ids = first.leaf_group_ids();
                ids.extend(second.leaf_group_ids());
                ids
            }
        }
    }

    pub fn contains_group(&self, group_id: &WorkspaceTabGroupID) -> bool {
        self.leaf_group_ids().iter().any(|id| id == group_id)
    }

    /// Stack the provided groups vertically, left-heavy tree like Swift.
    pub fn stacked(group_ids: &[WorkspaceTabGroupID], axis: WorkspaceSplitAxis, fraction: f32) -> Option<Self> {
        let mut iterator = group_ids.iter();
        let mut root = Self::Group(iterator.next()?.clone());
        for next in iterator {
            root = Self::split(axis, fraction, root, Self::Group(next.clone()));
        }
        Some(root)
    }

    pub fn appending(&self, group_id: WorkspaceTabGroupID, axis: WorkspaceSplitAxis, fraction: f32) -> Self {
        Self::split(axis, fraction, self.clone(), Self::Group(group_id))
    }

    pub fn removing(&self, group_id: &WorkspaceTabGroupID) -> Option<Self> {
        match self {
            Self::Group(current) => (current != group_id).then(|| self.clone()),
            Self::Split { axis, fraction, first, second } => {
                match (first.removing(group_id), second.removing(group_id)) {
                    (None, None) => None,
                    (Some(remaining), None) | (None, Some(remaining)) => Some(remaining),
                    (Some(next_first), Some(next_second)) => {
                        Some(Self::split(*axis, *fraction, next_first, next_second))
                    }
                }
            }
        }
    }

    /// Re-target a split identified by the groups on each side. Addresses
    /// survive subtree compaction because they name content, not paths.
    pub fn resizing(&self, address: &WorkspaceLayoutSplit, fraction: f32) -> Option<Self> {
        let Self::Split { axis, fraction: current, first, second } = self else {
            return None;
        };
        if first.contains_group(&address.first_group) && second.contains_group(&address.second_group) {
            let next = super::sizing::WorkspaceSplitFractions::clamp(fraction);
            return (next != *current).then(|| Self::split(*axis, next, (**first).clone(), (**second).clone()));
        }
        if let Some(next) = first.resizing(address, fraction) {
            return Some(Self::split(*axis, *current, next, (**second).clone()));
        }
        if let Some(next) = second.resizing(address, fraction) {
            return Some(Self::split(*axis, *current, (**first).clone(), next));
        }
        None
    }
}

/// A split is located by the groups on opposite sides of it.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceLayoutSplit {
    pub slot: WorkspaceSlotID,
    pub first_group: WorkspaceTabGroupID,
    pub second_group: WorkspaceTabGroupID,
}

impl WorkspaceLayoutSplit {
    pub fn new(slot: WorkspaceSlotID, first_group: WorkspaceTabGroupID, second_group: WorkspaceTabGroupID) -> Self {
        Self { slot, first_group, second_group }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorkspacePanel {
    pub id: WorkspacePanelID,
    pub title: String,
    pub is_closable: bool,
    pub is_draggable: bool,
    pub is_collapsible: bool,
    pub icon_asset_key: Option<String>,
    /// A panel that draws its own tabs, such as a code editor with file tabs,
    /// hides the group tab bar while it is the only panel in its group.
    #[serde(default)]
    pub hides_tab_bar_when_alone: bool,
    #[serde(default)]
    pub sizing: WorkspacePanelSizing,
}

impl WorkspacePanel {
    pub fn new(id: WorkspacePanelID, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            is_closable: true,
            is_draggable: true,
            is_collapsible: true,
            icon_asset_key: None,
            hides_tab_bar_when_alone: false,
            sizing: WorkspacePanelSizing::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceTabGroup {
    pub id: WorkspaceTabGroupID,
    pub panels: Vec<WorkspacePanelID>,
    pub active_panel_id: Option<WorkspacePanelID>,
    pub is_collapsed: bool,
    pub pinned_panel_ids: Vec<WorkspacePanelID>,
}

impl WorkspaceTabGroup {
    pub fn new(id: WorkspaceTabGroupID, panels: Vec<WorkspacePanelID>) -> Self {
        let active = panels.first().cloned();
        Self { id, panels, active_panel_id: active, is_collapsed: false, pinned_panel_ids: Vec::new() }
    }

    pub fn is_pinned(&self, panel_id: &WorkspacePanelID) -> bool {
        self.pinned_panel_ids.contains(panel_id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum WorkspaceSlotSize {
    Fixed(f32),
    Fraction(f32),
}

impl WorkspaceSlotSize {
    pub fn fixed_value(self) -> Option<f32> {
        match self {
            Self::Fixed(value) => Some(value),
            Self::Fraction(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum WorkspaceSlotKind {
    Content,
    Chrome { edge: WorkspaceEdge, size: WorkspaceSlotSize },
    Overlay,
    Floating,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceSlot {
    pub id: WorkspaceSlotID,
    pub kind: WorkspaceSlotKind,
    pub layout: Option<WorkspaceLayoutNode>,
}

impl WorkspaceSlot {
    pub fn new(id: WorkspaceSlotID, kind: WorkspaceSlotKind, layout: Option<WorkspaceLayoutNode>) -> Self {
        Self { id, kind, layout }
    }

    pub fn content(id: WorkspaceSlotID) -> Self {
        Self::new(id, WorkspaceSlotKind::Content, None)
    }

    pub fn contains_group(&self, group_id: &WorkspaceTabGroupID) -> bool {
        self.layout.as_ref().is_some_and(|layout| layout.contains_group(group_id))
    }

    pub fn append_group(&mut self, group_id: WorkspaceTabGroupID) {
        if self.contains_group(&group_id) {
            return;
        }
        self.layout = Some(match self.layout.take() {
            Some(layout) => layout.appending(group_id, WorkspaceSplitAxis::Vertical, 0.5),
            None => WorkspaceLayoutNode::Group(group_id),
        });
    }

    pub fn remove_group(&mut self, group_id: &WorkspaceTabGroupID) {
        self.layout = self.layout.as_ref().and_then(|layout| layout.removing(group_id));
    }

    pub fn standard_editor_slots() -> BTreeMap<WorkspaceSlotID, WorkspaceSlot> {
        let mut slots = BTreeMap::new();
        for id in [
            WorkspaceSlotID::leading(),
            WorkspaceSlotID::center(),
            WorkspaceSlotID::trailing(),
            WorkspaceSlotID::bottom(),
        ] {
            slots.insert(id.clone(), Self::content(id));
        }
        slots.insert(WorkspaceSlotID::overlay(), Self::new(WorkspaceSlotID::overlay(), WorkspaceSlotKind::Overlay, None));
        slots.insert(WorkspaceSlotID::floating(), Self::new(WorkspaceSlotID::floating(), WorkspaceSlotKind::Floating, None));
        slots.extend(Self::standard_editor_chrome_rail_slots());
        slots
    }

    pub fn standard_editor_chrome_rail_slots() -> BTreeMap<WorkspaceSlotID, WorkspaceSlot> {
        let mut slots = BTreeMap::new();
        for (id, edge) in [
            (WorkspaceSlotID::chrome_leading_rail(), WorkspaceEdge::Leading),
            (WorkspaceSlotID::chrome_trailing_rail(), WorkspaceEdge::Trailing),
            (WorkspaceSlotID::chrome_bottom_rail(), WorkspaceEdge::Bottom),
        ] {
            slots.insert(
                id.clone(),
                Self::new(id, WorkspaceSlotKind::Chrome { edge, size: WorkspaceSlotSize::Fixed(40.0) }, None),
            );
        }
        slots
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceFloatingWindow {
    pub id: WorkspaceFloatingWindowID,
    pub group_id: WorkspaceTabGroupID,
    pub title: String,
    pub frame: WorkspaceRect,
    pub z_index: i32,
}

impl WorkspaceFloatingWindow {
    pub fn new(id: WorkspaceFloatingWindowID, group_id: WorkspaceTabGroupID, title: String, frame: WorkspaceRect, z_index: i32) -> Self {
        Self { id, group_id, title, frame, z_index }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceClosedPanel {
    pub panel_id: WorkspacePanelID,
    pub group_id: WorkspaceTabGroupID,
    pub slot_id: Option<WorkspaceSlotID>,
    pub floating_window_id: Option<WorkspaceFloatingWindowID>,
    pub index: usize,
}

impl WorkspaceClosedPanel {
    pub fn new(
        panel_id: WorkspacePanelID,
        group_id: WorkspaceTabGroupID,
        slot_id: Option<WorkspaceSlotID>,
        floating_window_id: Option<WorkspaceFloatingWindowID>,
        index: usize,
    ) -> Self {
        Self { panel_id, group_id, slot_id, floating_window_id, index }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceCollapsedItem {
    pub group_id: WorkspaceTabGroupID,
    pub slot_id: WorkspaceSlotID,
    pub edge: WorkspaceEdge,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceDocument {
    pub panels: BTreeMap<WorkspacePanelID, WorkspacePanel>,
    pub groups: BTreeMap<WorkspaceTabGroupID, WorkspaceTabGroup>,
    pub slots: BTreeMap<WorkspaceSlotID, WorkspaceSlot>,
    #[serde(default)]
    pub collapsed: Vec<WorkspaceCollapsedItem>,
    #[serde(default)]
    pub floating_windows: Vec<WorkspaceFloatingWindow>,
    #[serde(default)]
    pub split_fractions: super::sizing::WorkspaceSplitFractions,
    #[serde(default)]
    pub closed_history: Vec<WorkspaceClosedPanel>,
}

impl WorkspaceDocument {
    pub fn new(
        panels: BTreeMap<WorkspacePanelID, WorkspacePanel>,
        groups: BTreeMap<WorkspaceTabGroupID, WorkspaceTabGroup>,
        slots: BTreeMap<WorkspaceSlotID, WorkspaceSlot>,
    ) -> Self {
        Self {
            panels,
            groups,
            slots,
            collapsed: Vec::new(),
            floating_windows: Vec::new(),
            split_fractions: Default::default(),
            closed_history: Vec::new(),
        }
    }

    pub fn slot(&self, id: &WorkspaceSlotID) -> WorkspaceSlot {
        self.slots.get(id).cloned().unwrap_or_else(|| WorkspaceSlot::content(id.clone()))
    }

    pub fn set_slot(&mut self, slot: WorkspaceSlot) {
        self.slots.insert(slot.id.clone(), slot);
    }

    pub fn ensure_standard_editor_slot_schema(&mut self) {
        for (slot_id, slot) in WorkspaceSlot::standard_editor_slots() {
            self.slots.entry(slot_id).or_insert(slot);
        }
    }

    pub fn group(&self, id: &WorkspaceTabGroupID) -> Option<&WorkspaceTabGroup> {
        self.groups.get(id)
    }

    pub fn panel(&self, id: &WorkspacePanelID) -> Option<&WorkspacePanel> {
        self.panels.get(id)
    }

    /// Deterministic order: slots are ordered, then leaf groups in layout order.
    pub fn group_containing(&self, panel_id: &WorkspacePanelID) -> Option<&WorkspaceTabGroup> {
        self.slots
            .values()
            .flat_map(|slot| slot.layout.iter().flat_map(|layout| layout.leaf_group_ids()))
            .filter_map(|id| self.groups.get(&id))
            .find(|group| group.panels.contains(panel_id))
            .or_else(|| {
                self.floating_windows
                    .iter()
                    .filter_map(|window| self.groups.get(&window.group_id))
                    .find(|group| group.panels.contains(panel_id))
            })
            .or_else(|| self.groups.values().find(|group| group.panels.contains(panel_id)))
    }

    pub fn slot_containing(&self, group_id: &WorkspaceTabGroupID) -> Option<WorkspaceSlotID> {
        self.slots
            .values()
            .find(|slot| slot.contains_group(group_id))
            .map(|slot| slot.id.clone())
    }

    pub fn floating_window_containing(&self, group_id: &WorkspaceTabGroupID) -> Option<&WorkspaceFloatingWindow> {
        self.floating_windows.iter().find(|window| &window.group_id == group_id)
    }

    pub fn referenced_layout_group_ids(&self) -> std::collections::BTreeSet<WorkspaceTabGroupID> {
        self.slots
            .values()
            .flat_map(|slot| slot.layout.as_ref().map(|layout| layout.leaf_group_ids()).unwrap_or_default())
            .collect()
    }

    pub fn referenced_floating_group_ids(&self) -> std::collections::BTreeSet<WorkspaceTabGroupID> {
        self.floating_windows.iter().map(|window| window.group_id.clone()).collect()
    }

    pub fn has_valid_layout_references(&self) -> bool {
        let layout_ids = self.referenced_layout_group_ids();
        let floating_ids = self.referenced_floating_group_ids();
        if !layout_ids.iter().all(|id| self.groups.contains_key(id))
            || !floating_ids.iter().all(|id| self.groups.contains_key(id))
        {
            return false;
        }
        let attached: std::collections::BTreeSet<_> = layout_ids.union(&floating_ids).cloned().collect();
        self.groups
            .iter()
            .filter(|(_, group)| !group.panels.is_empty())
            .all(|(id, _)| attached.contains(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(name: &str) -> WorkspaceTabGroupID {
        WorkspaceTabGroupID::new(name)
    }

    #[test]
    fn removing_a_leaf_compacts_its_parent() {
        let tree = WorkspaceLayoutNode::split(
            WorkspaceSplitAxis::Vertical,
            0.5,
            WorkspaceLayoutNode::Group(group("a")),
            WorkspaceLayoutNode::Group(group("b")),
        );
        let remaining = tree.removing(&group("b")).unwrap();
        assert_eq!(remaining.leaf_group_ids(), vec![group("a")]);
        assert!(tree.removing(&group("a")).is_some());
        assert!(remaining.removing(&group("a")).is_none());
    }

    #[test]
    fn resizing_finds_the_address_from_either_side() {
        let tree = WorkspaceLayoutNode::split(
            WorkspaceSplitAxis::Vertical,
            0.5,
            WorkspaceLayoutNode::Group(group("a")),
            WorkspaceLayoutNode::Group(group("b")),
        );
        let address = WorkspaceLayoutSplit::new(WorkspaceSlotID::center(), group("a"), group("b"));
        let next = tree.resizing(&address, 0.25).unwrap();
        assert!(matches!(next, WorkspaceLayoutNode::Split { fraction, .. } if fraction == 0.25));
        assert!(tree.resizing(&address, 0.5).is_none());
    }
}
