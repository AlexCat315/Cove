//! Authored workspace document: panels, tab groups, slots and floating windows.
//!
//! The document is pure data. Nothing here measures, and nothing caches a
//! derived frame, so a decoded document behaves exactly like an authored one.
//!
//! Translation of `Sources/CoveUIWorkspace/Layout/Split.swift`: the
//! `WorkspaceLayoutSplit` address and the `WorkspaceLayoutNode::resizing` walk
//! live in this module.
use super::ids::{
    WorkspaceEdge, WorkspaceFloatingWindowID, WorkspacePanelID, WorkspaceSlotID,
    WorkspaceSplitAxis, WorkspaceTabGroupID,
};
use super::sizing::{WorkspacePanelSizing, WorkspaceRect};
use cove_composition::{AnyView, EmptyView};
use std::collections::BTreeMap;
use std::rc::Rc;

/// Recursive layout of a slot. A split is located by its two children; the
/// common ancestor stays stable when hidden branches are compacted.
///
/// Encoded as Swift's synthesized `Codable`: `{"group":{"_0":id}}` and
/// `{"split":{"axis":…,"fraction":…,"first":…,"second":…}}`.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(from = "LayoutNodeCoding", into = "LayoutNodeCoding")]
pub enum WorkspaceLayoutNode {
    Group(WorkspaceTabGroupID),
    Split {
        axis: WorkspaceSplitAxis,
        fraction: f32,
        first: Box<WorkspaceLayoutNode>,
        second: Box<WorkspaceLayoutNode>,
    },
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
enum LayoutNodeCoding {
    Group {
        #[serde(rename = "_0")]
        id: WorkspaceTabGroupID,
    },
    Split {
        axis: WorkspaceSplitAxis,
        fraction: f32,
        first: Box<WorkspaceLayoutNode>,
        second: Box<WorkspaceLayoutNode>,
    },
}

impl From<LayoutNodeCoding> for WorkspaceLayoutNode {
    fn from(coding: LayoutNodeCoding) -> Self {
        match coding {
            LayoutNodeCoding::Group { id } => Self::Group(id),
            LayoutNodeCoding::Split {
                axis,
                fraction,
                first,
                second,
            } => Self::Split {
                axis,
                fraction,
                first,
                second,
            },
        }
    }
}

impl From<WorkspaceLayoutNode> for LayoutNodeCoding {
    fn from(node: WorkspaceLayoutNode) -> Self {
        match node {
            WorkspaceLayoutNode::Group(id) => Self::Group { id },
            WorkspaceLayoutNode::Split {
                axis,
                fraction,
                first,
                second,
            } => Self::Split {
                axis,
                fraction,
                first,
                second,
            },
        }
    }
}

impl WorkspaceLayoutNode {
    pub fn group(id: WorkspaceTabGroupID) -> Self {
        Self::Group(id)
    }

    pub fn split(axis: WorkspaceSplitAxis, fraction: f32, first: Self, second: Self) -> Self {
        Self::Split {
            axis,
            fraction,
            first: Box::new(first),
            second: Box::new(second),
        }
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
    pub fn stacked(
        group_ids: &[WorkspaceTabGroupID],
        axis: WorkspaceSplitAxis,
        fraction: f32,
    ) -> Option<Self> {
        let mut iterator = group_ids.iter();
        let mut root = Self::Group(iterator.next()?.clone());
        for next in iterator {
            root = Self::split(axis, fraction, root, Self::Group(next.clone()));
        }
        Some(root)
    }

    pub fn appending(
        &self,
        group_id: WorkspaceTabGroupID,
        axis: WorkspaceSplitAxis,
        fraction: f32,
    ) -> Self {
        Self::split(axis, fraction, self.clone(), Self::Group(group_id))
    }

    pub fn removing(&self, group_id: &WorkspaceTabGroupID) -> Option<Self> {
        match self {
            Self::Group(current) => (current != group_id).then(|| self.clone()),
            Self::Split {
                axis,
                fraction,
                first,
                second,
            } => match (first.removing(group_id), second.removing(group_id)) {
                (None, None) => None,
                (Some(remaining), None) | (None, Some(remaining)) => Some(remaining),
                (Some(next_first), Some(next_second)) => {
                    Some(Self::split(*axis, *fraction, next_first, next_second))
                }
            },
        }
    }

    /// Re-target a split identified by the groups on each side. Addresses
    /// survive subtree compaction because they name content, not paths.
    pub fn resizing(&self, address: &WorkspaceLayoutSplit, fraction: f32) -> Option<Self> {
        let Self::Split {
            axis,
            fraction: current,
            first,
            second,
        } = self
        else {
            return None;
        };
        if first.contains_group(&address.first_group)
            && second.contains_group(&address.second_group)
        {
            let next = super::sizing::WorkspaceSplitFractions::clamp(fraction);
            return (next != *current)
                .then(|| Self::split(*axis, next, (**first).clone(), (**second).clone()));
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
#[serde(rename_all = "camelCase")]
pub struct WorkspaceLayoutSplit {
    pub slot: WorkspaceSlotID,
    pub first_group: WorkspaceTabGroupID,
    pub second_group: WorkspaceTabGroupID,
}

impl WorkspaceLayoutSplit {
    pub fn new(
        slot: WorkspaceSlotID,
        first_group: WorkspaceTabGroupID,
        second_group: WorkspaceTabGroupID,
    ) -> Self {
        Self {
            slot,
            first_group,
            second_group,
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePanel {
    pub id: WorkspacePanelID,
    pub title: String,
    #[serde(default = "super::coding::yes")]
    pub is_closable: bool,
    #[serde(default = "super::coding::yes")]
    pub is_draggable: bool,
    #[serde(default = "super::coding::yes")]
    pub is_collapsible: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTabGroup {
    pub id: WorkspaceTabGroupID,
    pub panels: Vec<WorkspacePanelID>,
    #[serde(
        rename = "activePanelID",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub active_panel_id: Option<WorkspacePanelID>,
    pub is_collapsed: bool,
    #[serde(rename = "pinnedPanelIDs")]
    pub pinned_panel_ids: Vec<WorkspacePanelID>,
}

impl WorkspaceTabGroup {
    pub fn new(id: WorkspaceTabGroupID, panels: Vec<WorkspacePanelID>) -> Self {
        let active = panels.first().cloned();
        Self {
            id,
            panels,
            active_panel_id: active,
            is_collapsed: false,
            pinned_panel_ids: Vec::new(),
        }
    }

    pub fn is_pinned(&self, panel_id: &WorkspacePanelID) -> bool {
        self.pinned_panel_ids.contains(panel_id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkspaceSlotSize {
    Fixed(#[serde(with = "unlabelled")] f32),
    Fraction(#[serde(with = "unlabelled")] f32),
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
#[serde(from = "SlotKindCoding", into = "SlotKindCoding")]
pub enum WorkspaceSlotKind {
    Content,
    Chrome {
        edge: WorkspaceEdge,
        size: WorkspaceSlotSize,
    },
    Overlay,
    Floating,
}

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
enum SlotKindCoding {
    Content {},
    Chrome {
        edge: WorkspaceEdge,
        size: WorkspaceSlotSize,
    },
    Overlay {},
    Floating {},
}

impl From<SlotKindCoding> for WorkspaceSlotKind {
    fn from(coding: SlotKindCoding) -> Self {
        match coding {
            SlotKindCoding::Content {} => Self::Content,
            SlotKindCoding::Chrome { edge, size } => Self::Chrome { edge, size },
            SlotKindCoding::Overlay {} => Self::Overlay,
            SlotKindCoding::Floating {} => Self::Floating,
        }
    }
}

impl From<WorkspaceSlotKind> for SlotKindCoding {
    fn from(kind: WorkspaceSlotKind) -> Self {
        match kind {
            WorkspaceSlotKind::Content => Self::Content {},
            WorkspaceSlotKind::Chrome { edge, size } => Self::Chrome { edge, size },
            WorkspaceSlotKind::Overlay => Self::Overlay {},
            WorkspaceSlotKind::Floating => Self::Floating {},
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSlot {
    pub id: WorkspaceSlotID,
    pub kind: WorkspaceSlotKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<WorkspaceLayoutNode>,
}

impl WorkspaceSlot {
    pub fn new(
        id: WorkspaceSlotID,
        kind: WorkspaceSlotKind,
        layout: Option<WorkspaceLayoutNode>,
    ) -> Self {
        Self { id, kind, layout }
    }

    pub fn content(id: WorkspaceSlotID) -> Self {
        Self::new(id, WorkspaceSlotKind::Content, None)
    }

    pub fn contains_group(&self, group_id: &WorkspaceTabGroupID) -> bool {
        self.layout
            .as_ref()
            .is_some_and(|layout| layout.contains_group(group_id))
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
        self.layout = self
            .layout
            .as_ref()
            .and_then(|layout| layout.removing(group_id));
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
        slots.insert(
            WorkspaceSlotID::overlay(),
            Self::new(WorkspaceSlotID::overlay(), WorkspaceSlotKind::Overlay, None),
        );
        slots.insert(
            WorkspaceSlotID::floating(),
            Self::new(
                WorkspaceSlotID::floating(),
                WorkspaceSlotKind::Floating,
                None,
            ),
        );
        slots.extend(Self::standard_editor_chrome_rail_slots());
        slots
    }

    pub fn standard_editor_chrome_rail_slots() -> BTreeMap<WorkspaceSlotID, WorkspaceSlot> {
        let mut slots = BTreeMap::new();
        for (id, edge) in [
            (
                WorkspaceSlotID::chrome_leading_rail(),
                WorkspaceEdge::Leading,
            ),
            (
                WorkspaceSlotID::chrome_trailing_rail(),
                WorkspaceEdge::Trailing,
            ),
            (WorkspaceSlotID::chrome_bottom_rail(), WorkspaceEdge::Bottom),
        ] {
            slots.insert(
                id.clone(),
                Self::new(
                    id,
                    WorkspaceSlotKind::Chrome {
                        edge,
                        size: WorkspaceSlotSize::Fixed(40.0),
                    },
                    None,
                ),
            );
        }
        slots
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFloatingWindow {
    pub id: WorkspaceFloatingWindowID,
    #[serde(rename = "groupID")]
    pub group_id: WorkspaceTabGroupID,
    pub title: String,
    pub frame: WorkspaceRect,
    pub z_index: i32,
}

impl WorkspaceFloatingWindow {
    pub fn new(
        id: WorkspaceFloatingWindowID,
        group_id: WorkspaceTabGroupID,
        title: String,
        frame: WorkspaceRect,
        z_index: i32,
    ) -> Self {
        Self {
            id,
            group_id,
            title,
            frame,
            z_index,
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceClosedPanel {
    #[serde(rename = "panelID")]
    pub panel_id: WorkspacePanelID,
    #[serde(rename = "groupID")]
    pub group_id: WorkspaceTabGroupID,
    #[serde(rename = "slotID", default, skip_serializing_if = "Option::is_none")]
    pub slot_id: Option<WorkspaceSlotID>,
    #[serde(
        rename = "floatingWindowID",
        default,
        skip_serializing_if = "Option::is_none"
    )]
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
        Self {
            panel_id,
            group_id,
            slot_id,
            floating_window_id,
            index,
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCollapsedItem {
    #[serde(rename = "groupID")]
    pub group_id: WorkspaceTabGroupID,
    #[serde(rename = "slotID")]
    pub slot_id: WorkspaceSlotID,
    pub edge: WorkspaceEdge,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDocument {
    #[serde(with = "map")]
    pub panels: BTreeMap<WorkspacePanelID, WorkspacePanel>,
    #[serde(with = "map")]
    pub groups: BTreeMap<WorkspaceTabGroupID, WorkspaceTabGroup>,
    #[serde(with = "map")]
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
        self.slots
            .get(id)
            .cloned()
            .unwrap_or_else(|| WorkspaceSlot::content(id.clone()))
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
            .flat_map(|slot| {
                slot.layout
                    .iter()
                    .flat_map(|layout| layout.leaf_group_ids())
            })
            .filter_map(|id| self.groups.get(&id))
            .find(|group| group.panels.contains(panel_id))
            .or_else(|| {
                self.floating_windows
                    .iter()
                    .filter_map(|window| self.groups.get(&window.group_id))
                    .find(|group| group.panels.contains(panel_id))
            })
            .or_else(|| {
                self.groups
                    .values()
                    .find(|group| group.panels.contains(panel_id))
            })
    }

    pub fn slot_containing(&self, group_id: &WorkspaceTabGroupID) -> Option<WorkspaceSlotID> {
        self.slots
            .values()
            .find(|slot| slot.contains_group(group_id))
            .map(|slot| slot.id.clone())
    }

    pub fn floating_window_containing(
        &self,
        group_id: &WorkspaceTabGroupID,
    ) -> Option<&WorkspaceFloatingWindow> {
        self.floating_windows
            .iter()
            .find(|window| &window.group_id == group_id)
    }

    pub fn referenced_layout_group_ids(&self) -> std::collections::BTreeSet<WorkspaceTabGroupID> {
        self.slots
            .values()
            .flat_map(|slot| {
                slot.layout
                    .as_ref()
                    .map(|layout| layout.leaf_group_ids())
                    .unwrap_or_default()
            })
            .collect()
    }

    pub fn referenced_floating_group_ids(&self) -> std::collections::BTreeSet<WorkspaceTabGroupID> {
        self.floating_windows
            .iter()
            .map(|window| window.group_id.clone())
            .collect()
    }

    pub fn has_valid_layout_references(&self) -> bool {
        let layout_ids = self.referenced_layout_group_ids();
        let floating_ids = self.referenced_floating_group_ids();
        if !layout_ids.iter().all(|id| self.groups.contains_key(id))
            || !floating_ids.iter().all(|id| self.groups.contains_key(id))
        {
            return false;
        }
        let attached: std::collections::BTreeSet<_> =
            layout_ids.union(&floating_ids).cloned().collect();
        self.groups
            .iter()
            .filter(|(_, group)| !group.panels.is_empty())
            .all(|(id, _)| attached.contains(id))
    }
}

/// Authored panel metadata has one representation; the factory is transient.
pub struct WorkspacePanelDescriptor {
    pub panel: WorkspacePanel,
    pub default_slot: WorkspaceSlotID,
    factory: Rc<dyn Fn() -> AnyView>,
}

impl WorkspacePanelDescriptor {
    pub fn new(id: WorkspacePanelID, title: impl Into<String>) -> Self {
        Self::with(Self::panel_for(id, title))
    }

    fn panel_for(id: WorkspacePanelID, title: impl Into<String>) -> WorkspacePanel {
        WorkspacePanel::new(id, title)
    }

    pub fn with(panel: WorkspacePanel) -> Self {
        Self {
            panel,
            default_slot: WorkspaceSlotID::center(),
            factory: Rc::new(|| AnyView::new(EmptyView)),
        }
    }

    pub fn default_slot(mut self, slot: WorkspaceSlotID) -> Self {
        self.default_slot = slot;
        self
    }

    pub fn configure(mut self, configure: impl FnOnce(&mut WorkspacePanel)) -> Self {
        configure(&mut self.panel);
        self
    }

    pub fn content(mut self, content: impl Fn() -> AnyView + 'static) -> Self {
        self.factory = Rc::new(content);
        self
    }

    /// The view for this panel. A descriptor without content renders empty,
    /// like Swift's `AnyView(EmptyView())` fallback.
    pub fn make(&self) -> AnyView {
        (self.factory)()
    }
}

impl Clone for WorkspacePanelDescriptor {
    fn clone(&self) -> Self {
        Self {
            panel: self.panel.clone(),
            default_slot: self.default_slot.clone(),
            factory: self.factory.clone(),
        }
    }
}

/// Descriptors registered by identity, in registration order.
///
/// Updating a descriptor cannot change its panel identity: the identity is the
/// map key, so a descriptor carrying another panel id replaces nothing.
#[derive(Default)]
pub struct WorkspacePanelRegistry {
    by_id: BTreeMap<WorkspacePanelID, WorkspacePanelDescriptor>,
    order: Vec<WorkspacePanelID>,
}

impl WorkspacePanelRegistry {
    pub fn new(descriptors: impl IntoIterator<Item = WorkspacePanelDescriptor>) -> Self {
        let mut registry = Self::default();
        for descriptor in descriptors {
            registry.register(descriptor);
        }
        registry
    }

    pub fn register(&mut self, descriptor: WorkspacePanelDescriptor) {
        let id = descriptor.panel.id.clone();
        if !self.by_id.contains_key(&id) {
            self.order.push(id.clone());
        }
        self.by_id.insert(id, descriptor);
    }

    /// Re-author a registered descriptor. An update that would change the
    /// panel identity is rejected, matching the Swift precondition.
    pub fn update_descriptor(
        &mut self,
        id: &WorkspacePanelID,
        update: impl FnOnce(&mut WorkspacePanelDescriptor),
    ) {
        let Some(mut descriptor) = self.by_id.get(id).cloned() else {
            return;
        };
        update(&mut descriptor);
        if &descriptor.panel.id != id {
            return;
        }
        self.by_id.insert(id.clone(), descriptor);
    }

    pub fn descriptor(&self, id: &WorkspacePanelID) -> Option<&WorkspacePanelDescriptor> {
        self.by_id.get(id)
    }

    pub fn make(&self, id: &WorkspacePanelID) -> AnyView {
        self.by_id
            .get(id)
            .map_or_else(|| AnyView::new(EmptyView), WorkspacePanelDescriptor::make)
    }

    /// A content closure for [`WorkspaceView`], so the registry can be handed to
    /// a view without the caller rebuilding the indirection.
    pub fn content(&self) -> Rc<dyn Fn(WorkspacePanelID) -> AnyView> {
        let snapshot: BTreeMap<WorkspacePanelID, AnyView> = self
            .order
            .iter()
            .filter_map(|id| self.by_id.get(id).map(|d| (id.clone(), d.make())))
            .collect();
        Rc::new(move |id| {
            snapshot
                .get(&id)
                .cloned()
                .unwrap_or_else(|| AnyView::new(EmptyView))
        })
    }

    pub fn unregister(&mut self, id: &WorkspacePanelID) {
        self.by_id.remove(id);
        self.order.retain(|candidate| candidate != id);
    }

    pub fn count(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    pub fn ids(&self) -> &[WorkspacePanelID] {
        &self.order
    }

    pub fn descriptors(&self) -> Vec<WorkspacePanelDescriptor> {
        self.order
            .iter()
            .filter_map(|id| self.by_id.get(id).cloned())
            .collect()
    }

    /// The panels of every registered descriptor, in registration order.
    pub fn panels(&self) -> Vec<WorkspacePanel> {
        self.descriptors()
            .into_iter()
            .map(|descriptor| descriptor.panel)
            .collect()
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

mod map {
    pub use super::super::coding::{deserialize_map as deserialize, serialize_map as serialize};
}

mod unlabelled {
    use super::super::coding::Unlabelled;

    pub fn serialize<S: serde::Serializer>(value: &f32, serializer: S) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(&Unlabelled { value: *value }, serializer)
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
        <Unlabelled<f32> as serde::Deserialize>::deserialize(deserializer).map(|coded| coded.value)
    }
}

#[cfg(test)]
mod coding_tests {
    use super::*;

    #[test]
    fn documents_use_the_swift_codable_shape() {
        let id = WorkspaceTabGroupID::new("center");
        let panel = WorkspacePanelID::new("viewport");
        let document = WorkspaceDocument::new(
            BTreeMap::from([(panel.clone(), WorkspacePanel::new(panel.clone(), "Viewport"))]),
            BTreeMap::from([(id.clone(), WorkspaceTabGroup::new(id.clone(), vec![panel]))]),
            BTreeMap::from([(
                WorkspaceSlotID::center(),
                WorkspaceSlot::new(
                    WorkspaceSlotID::center(),
                    WorkspaceSlotKind::Chrome {
                        edge: WorkspaceEdge::Leading,
                        size: WorkspaceSlotSize::Fixed(40.0),
                    },
                    Some(WorkspaceLayoutNode::split(
                        WorkspaceSplitAxis::Horizontal,
                        0.5,
                        WorkspaceLayoutNode::group(id.clone()),
                        WorkspaceLayoutNode::group(id),
                    )),
                ),
            )]),
        );
        let value = serde_json::to_value(&document).expect("encodes");
        assert_eq!(value["panels"][0], "viewport", "ID-keyed dictionaries are flat pairs");
        assert_eq!(value["panels"][1]["isClosable"], true);
        assert_eq!(value["groups"][1]["activePanelID"], "viewport");
        assert_eq!(
            value["slots"][1]["kind"],
            serde_json::json!({ "chrome": { "edge": "leading", "size": { "fixed": { "_0": 40.0 } } } })
        );
        assert_eq!(
            value["slots"][1]["layout"]["split"]["first"],
            serde_json::json!({ "group": { "_0": "center" } })
        );
        assert_eq!(value["splitFractions"], serde_json::json!({}));
        let decoded: WorkspaceDocument = serde_json::from_value(value).expect("decodes");
        assert_eq!(decoded, document);

        let minimal: WorkspacePanel =
            serde_json::from_str(r#"{"id":"a","title":"A"}"#).expect("Swift defaults apply");
        assert!(minimal.is_closable && minimal.is_draggable && minimal.is_collapsible);
        assert!(!minimal.hides_tab_bar_when_alone);
    }
}
