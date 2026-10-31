//! Command application for the workspace document.
//!
//! Every mutation goes through one `dispatch` so observers, maximized/focused
//! reconciliation and persistence dirtiness stay in a single place. Commands
//! that would not change the document return `unchanged` and never notify.
use super::ids::{
    WorkspaceEdge, WorkspaceFloatingWindowID, WorkspacePanelID, WorkspacePlacementKind,
    WorkspaceSlotID, WorkspaceSplitAxis, WorkspaceSplitID, WorkspaceTabGroupID,
};
use super::model::{
    WorkspaceClosedPanel, WorkspaceCollapsedItem, WorkspaceDocument, WorkspaceFloatingWindow,
    WorkspaceLayoutNode, WorkspaceLayoutSplit, WorkspaceSlot, WorkspaceTabGroup,
};
use super::sizing::{WorkspaceRect, WorkspaceSizingResolver, WorkspaceSplitFractions};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq)]
pub struct WorkspaceTarget {
    pub slot: WorkspaceSlotID,
    pub group_id: Option<WorkspaceTabGroupID>,
    pub placement: WorkspacePlacementKind,
    pub edge: Option<WorkspaceEdge>,
    pub tab_index: Option<usize>,
    pub slot_index: Option<usize>,
    pub fraction: f32,
}

impl WorkspaceTarget {
    pub fn new(slot: WorkspaceSlotID) -> Self {
        Self {
            slot,
            group_id: None,
            placement: WorkspacePlacementKind::TabGroup,
            edge: None,
            tab_index: None,
            slot_index: None,
            fraction: 0.5,
        }
    }

    pub fn fraction(mut self, fraction: f32) -> Self {
        self.fraction = WorkspaceSplitFractions::clamp(fraction);
        self
    }

    pub fn tab_group(
        slot: WorkspaceSlotID,
        group_id: WorkspaceTabGroupID,
        tab_index: Option<usize>,
    ) -> Self {
        Self {
            group_id: Some(group_id),
            tab_index,
            ..Self::new(slot)
        }
    }

    pub fn slot_target(slot: WorkspaceSlotID, index: Option<usize>) -> Self {
        Self {
            placement: WorkspacePlacementKind::Slot,
            slot_index: index,
            ..Self::new(slot)
        }
    }

    pub fn split(
        slot: WorkspaceSlotID,
        anchor_group_id: WorkspaceTabGroupID,
        edge: WorkspaceEdge,
        fraction: f32,
    ) -> Self {
        Self {
            group_id: Some(anchor_group_id),
            placement: WorkspacePlacementKind::Split,
            edge: Some(edge),
            ..Self::new(slot).fraction(fraction)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorkspaceHitTarget {
    pub slot: WorkspaceSlotID,
    pub group_id: Option<WorkspaceTabGroupID>,
    pub panel_id: Option<WorkspacePanelID>,
    pub target: WorkspaceTarget,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WorkspaceCommand {
    ToggleMaximize(WorkspacePanelID),
    RestoreMaximized,
    ReplaceDocument(Box<WorkspaceDocument>),
    Collapse(WorkspaceTabGroupID),
    Expand(WorkspaceTabGroupID),
    ToggleCollapse(WorkspaceTabGroupID),
    SetActivePanel {
        group_id: WorkspaceTabGroupID,
        panel_id: WorkspacePanelID,
    },
    ReorderPanel {
        panel_id: WorkspacePanelID,
        group_id: WorkspaceTabGroupID,
        index: usize,
    },
    MovePanel {
        panel_id: WorkspacePanelID,
        target: WorkspaceTarget,
    },
    ClosePanel(WorkspacePanelID),
    CloseOthers {
        group_id: WorkspaceTabGroupID,
        keeping: WorkspacePanelID,
    },
    CloseToTheRight {
        group_id: WorkspaceTabGroupID,
        of: WorkspacePanelID,
    },
    ReopenLastClosed,
    ReopenPanel(WorkspacePanelID),
    SetPinned {
        panel_id: WorkspacePanelID,
        is_pinned: bool,
    },
    FloatGroup {
        group_id: WorkspaceTabGroupID,
        window_id: WorkspaceFloatingWindowID,
        frame: WorkspaceRect,
    },
    RedockFloatingWindow {
        window_id: WorkspaceFloatingWindowID,
        target: WorkspaceTarget,
    },
    MoveFloatingWindow {
        window_id: WorkspaceFloatingWindowID,
        frame: WorkspaceRect,
    },
    FocusFloatingWindow(WorkspaceFloatingWindowID),
    ResizeSplit {
        split_id: WorkspaceSplitID,
        fraction: f32,
    },
    ResizeLayoutSplit {
        address: WorkspaceLayoutSplit,
        fraction: f32,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkspaceTransactionResult {
    pub did_change: bool,
    pub changed_slots: BTreeSet<WorkspaceSlotID>,
    pub focus_panel_id: Option<WorkspacePanelID>,
    pub persistence_dirty: bool,
}

impl WorkspaceTransactionResult {
    pub fn unchanged() -> Self {
        Self::default()
    }
    fn changed(
        changed_slots: impl IntoIterator<Item = WorkspaceSlotID>,
        focus: Option<WorkspacePanelID>,
        persistence_dirty: bool,
    ) -> Self {
        Self {
            did_change: true,
            changed_slots: changed_slots.into_iter().collect(),
            focus_panel_id: focus,
            persistence_dirty,
        }
    }
    fn visual_only(focus: Option<WorkspacePanelID>) -> Self {
        Self {
            did_change: true,
            changed_slots: BTreeSet::new(),
            focus_panel_id: focus,
            persistence_dirty: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SubscriptionToken(u64);

struct LocatedPanel {
    slot_id: Option<WorkspaceSlotID>,
    floating_window_id: Option<WorkspaceFloatingWindowID>,
    group: WorkspaceTabGroup,
    index: usize,
}

struct Destination {
    slot: WorkspaceSlotID,
    anchor_group_id: Option<WorkspaceTabGroupID>,
    placement: WorkspacePlacementKind,
    split_edge: Option<WorkspaceEdge>,
    fraction: f32,
}

/// Owns the authored document and the transient focus/maximize state.
pub struct WorkspaceController {
    document: WorkspaceDocument,
    maximized_panel_id: Option<WorkspacePanelID>,
    focused_panel_id: Option<WorkspacePanelID>,
    version: u64,
    subscribers: BTreeMap<SubscriptionToken, Box<dyn FnMut(&WorkspaceController)>>,
    next_subscriber_id: u64,
}

impl WorkspaceController {
    pub fn new(document: WorkspaceDocument) -> Self {
        let mut controller = Self {
            document,
            maximized_panel_id: None,
            focused_panel_id: None,
            version: 0,
            subscribers: BTreeMap::new(),
            next_subscriber_id: 0,
        };
        controller.focused_panel_id = controller.active_center_panel();
        controller
    }

    pub fn document(&self) -> &WorkspaceDocument {
        &self.document
    }
    pub fn maximized_panel_id(&self) -> Option<&WorkspacePanelID> {
        self.maximized_panel_id.as_ref()
    }
    pub fn focused_panel_id(&self) -> Option<&WorkspacePanelID> {
        self.focused_panel_id.as_ref()
    }
    /// Monotonic document version. Observers compare it to decide re-render.
    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn subscribe(
        &mut self,
        handler: impl FnMut(&WorkspaceController) + 'static,
    ) -> SubscriptionToken {
        self.next_subscriber_id = self.next_subscriber_id.wrapping_add(1);
        let token = SubscriptionToken(self.next_subscriber_id);
        self.subscribers.insert(token, Box::new(handler));
        token
    }

    pub fn unsubscribe(&mut self, token: SubscriptionToken) {
        self.subscribers.remove(&token);
    }

    pub fn replace(&mut self, document: WorkspaceDocument) {
        self.dispatch(WorkspaceCommand::ReplaceDocument(Box::new(document)));
    }

    pub fn dispatch(&mut self, command: WorkspaceCommand) -> WorkspaceTransactionResult {
        let before = self.document.clone();
        let result = self.apply(command);
        if !result.did_change && before == self.document {
            return WorkspaceTransactionResult::unchanged();
        }
        if let Some(panel_id) = &self.maximized_panel_id
            && self.document.group_containing(panel_id).is_none()
        {
            self.maximized_panel_id = None;
        }
        if let Some(panel_id) = &self.focused_panel_id
            && self.document.group_containing(panel_id).is_none()
        {
            self.focused_panel_id = None;
        }
        self.version = self.version.wrapping_add(1);
        if let Some(panel_id) = result.focus_panel_id.clone() {
            self.focused_panel_id = Some(panel_id);
        }
        self.notify_change();
        result
    }

    fn apply(&mut self, command: WorkspaceCommand) -> WorkspaceTransactionResult {
        match command {
            WorkspaceCommand::ToggleMaximize(panel_id) => {
                if self.document.group_containing(&panel_id).is_none() {
                    return WorkspaceTransactionResult::unchanged();
                }
                self.maximized_panel_id =
                    (self.maximized_panel_id.as_ref() != Some(&panel_id)).then(|| panel_id.clone());
                WorkspaceTransactionResult::visual_only(Some(panel_id))
            }
            WorkspaceCommand::RestoreMaximized => {
                if self.maximized_panel_id.take().is_none() {
                    return WorkspaceTransactionResult::unchanged();
                }
                WorkspaceTransactionResult::visual_only(None)
            }
            WorkspaceCommand::ReplaceDocument(next) => {
                if *next == self.document && self.maximized_panel_id.is_none() {
                    return WorkspaceTransactionResult::unchanged();
                }
                self.maximized_panel_id = None;
                self.document = *next;
                let slots = self.document.slots.keys().cloned().collect::<Vec<_>>();
                WorkspaceTransactionResult::changed(slots, self.active_center_panel(), true)
            }
            WorkspaceCommand::Collapse(group_id) => self.set_collapsed(&group_id, true),
            WorkspaceCommand::Expand(group_id) => self.set_collapsed(&group_id, false),
            WorkspaceCommand::ToggleCollapse(group_id) => {
                let Some(group) = self.document.groups.get(&group_id) else {
                    return WorkspaceTransactionResult::unchanged();
                };
                let collapsed = !group.is_collapsed;
                self.set_collapsed(&group_id, collapsed)
            }
            WorkspaceCommand::SetActivePanel { group_id, panel_id } => {
                let Some(group) = self.document.groups.get(&group_id) else {
                    return WorkspaceTransactionResult::unchanged();
                };
                if !group.panels.contains(&panel_id)
                    || (group.active_panel_id.as_ref() == Some(&panel_id)
                        && self.focused_panel_id.as_ref() == Some(&panel_id))
                {
                    return WorkspaceTransactionResult::unchanged();
                }
                let document_changed = group.active_panel_id.as_ref() != Some(&panel_id);
                let slot = self.document.slot_containing(&group_id);
                let mut group = group.clone();
                group.active_panel_id = Some(panel_id.clone());
                self.document.groups.insert(group_id, group);
                let slots: Vec<_> = if document_changed {
                    slot.into_iter().collect()
                } else {
                    Vec::new()
                };
                if document_changed {
                    WorkspaceTransactionResult::changed(slots, Some(panel_id), true)
                } else {
                    WorkspaceTransactionResult::visual_only(Some(panel_id))
                }
            }
            WorkspaceCommand::ReorderPanel {
                panel_id,
                group_id,
                index,
            } => self.reorder_panel(&panel_id, &group_id, index),
            WorkspaceCommand::MovePanel { panel_id, target } => self.move_panel(&panel_id, target),
            WorkspaceCommand::ClosePanel(panel_id) => self.close_panel(&panel_id),
            WorkspaceCommand::CloseOthers { group_id, keeping } => {
                self.close_others(&group_id, &keeping)
            }
            WorkspaceCommand::CloseToTheRight { group_id, of } => {
                self.close_to_the_right(&group_id, &of)
            }
            WorkspaceCommand::ReopenLastClosed => self.reopen_last_closed(),
            WorkspaceCommand::ReopenPanel(panel_id) => self.reopen_panel(&panel_id),
            WorkspaceCommand::SetPinned {
                panel_id,
                is_pinned,
            } => self.set_pinned(&panel_id, is_pinned),
            WorkspaceCommand::FloatGroup {
                group_id,
                window_id,
                frame,
            } => self.float_group(&group_id, window_id, frame),
            WorkspaceCommand::RedockFloatingWindow { window_id, target } => {
                self.redock_floating_window(&window_id, target)
            }
            WorkspaceCommand::MoveFloatingWindow { window_id, frame } => {
                self.move_floating_window(&window_id, frame)
            }
            WorkspaceCommand::FocusFloatingWindow(window_id) => {
                self.focus_floating_window(&window_id)
            }
            WorkspaceCommand::ResizeSplit { split_id, fraction } => {
                self.resize_split(&split_id, fraction)
            }
            WorkspaceCommand::ResizeLayoutSplit { address, fraction } => {
                let mut slot = self.document.slot(&address.slot);
                if !fraction.is_finite() {
                    return WorkspaceTransactionResult::unchanged();
                }
                let Some(next) = slot
                    .layout
                    .as_ref()
                    .and_then(|layout| layout.resizing(&address, fraction))
                else {
                    return WorkspaceTransactionResult::unchanged();
                };
                slot.layout = Some(next);
                let slot_id = slot.id.clone();
                self.document.set_slot(slot);
                WorkspaceTransactionResult::changed(vec![slot_id], None, true)
            }
        }
    }

    fn set_collapsed(
        &mut self,
        group_id: &WorkspaceTabGroupID,
        collapsed: bool,
    ) -> WorkspaceTransactionResult {
        let Some(group) = self.document.groups.get(group_id) else {
            return WorkspaceTransactionResult::unchanged();
        };
        if group.is_collapsed == collapsed {
            return WorkspaceTransactionResult::unchanged();
        }
        let Some(slot_id) = self.document.slot_containing(group_id) else {
            return WorkspaceTransactionResult::unchanged();
        };
        if slot_id == WorkspaceSlotID::center() {
            return WorkspaceTransactionResult::unchanged();
        }
        if collapsed
            && !group.panels.iter().any(|id| {
                self.document
                    .panels
                    .get(id)
                    .is_some_and(|panel| panel.is_collapsible)
            })
        {
            return WorkspaceTransactionResult::unchanged();
        }
        let active = group.active_panel_id.clone();
        let mut group = group.clone();
        group.is_collapsed = collapsed;
        self.document.groups.insert(group_id.clone(), group);
        if collapsed {
            let edge = self.collapsed_edge(&slot_id);
            if !self
                .document
                .collapsed
                .iter()
                .any(|item| &item.group_id == group_id)
            {
                self.document.collapsed.push(WorkspaceCollapsedItem {
                    group_id: group_id.clone(),
                    slot_id: slot_id.clone(),
                    edge,
                });
            }
        } else {
            self.document
                .collapsed
                .retain(|item| &item.group_id != group_id);
        }
        let focus = if collapsed { None } else { active };
        WorkspaceTransactionResult::changed(vec![slot_id], focus, true)
    }

    fn move_panel(
        &mut self,
        panel_id: &WorkspacePanelID,
        target: WorkspaceTarget,
    ) -> WorkspaceTransactionResult {
        if !self.document.panels.contains_key(panel_id) {
            return WorkspaceTransactionResult::unchanged();
        }
        let mut changed_slots = BTreeSet::new();
        let located = self.locate(panel_id);
        let source_group_id = located.as_ref().map(|located| located.group.id.clone());
        let source_slot_id = located.as_ref().and_then(|located| located.slot_id.clone());
        changed_slots.extend(source_slot_id.clone());
        let destination = self.destination(target.clone());
        changed_slots.insert(destination.slot.clone());

        if let Some(source) = &source_group_id
            && Some(source) == target.group_id.as_ref()
            && source_slot_id.as_ref() == Some(&destination.slot)
            && destination.placement == WorkspacePlacementKind::TabGroup
        {
            let Some(group) = self.document.groups.get(source) else {
                return WorkspaceTransactionResult::unchanged();
            };
            let did_change = group.active_panel_id.as_ref() != Some(panel_id) || group.is_collapsed;
            let mut group = group.clone();
            group.active_panel_id = Some(panel_id.clone());
            group.is_collapsed = false;
            self.document.groups.insert(group.id.clone(), group);
            if !did_change {
                return WorkspaceTransactionResult::unchanged();
            }
            return WorkspaceTransactionResult::changed(
                changed_slots,
                Some(panel_id.clone()),
                true,
            );
        }

        if let Some(source) = &source_group_id {
            self.remove(panel_id, source);
        }

        let target_group_id = match (
            destination.placement,
            destination.split_edge,
            destination.anchor_group_id.clone(),
        ) {
            (WorkspacePlacementKind::Split, Some(edge), Some(anchor)) => self.make_group(
                &destination.slot,
                Some(anchor),
                Some(edge),
                destination.fraction,
            ),
            (_, _, Some(anchor)) if self.document.groups.contains_key(&anchor) => {
                self.ensure_group(&anchor, &destination.slot);
                anchor
            }
            _ => self
                .first_group_id(&destination.slot)
                .unwrap_or_else(|| self.make_primary_group(&destination.slot)),
        };

        let mut target_group = self
            .document
            .groups
            .get(&target_group_id)
            .cloned()
            .unwrap_or_else(|| WorkspaceTabGroup::new(target_group_id.clone(), Vec::new()));
        if !target_group.panels.contains(panel_id) {
            let index = target_group.panels.len();
            insert_panel(panel_id.clone(), &mut target_group, index);
        }
        target_group.active_panel_id = Some(panel_id.clone());
        target_group.is_collapsed = false;
        self.document.groups.insert(target_group_id, target_group);

        WorkspaceTransactionResult::changed(changed_slots, Some(panel_id.clone()), true)
    }

    fn reorder_panel(
        &mut self,
        panel_id: &WorkspacePanelID,
        group_id: &WorkspaceTabGroupID,
        index: usize,
    ) -> WorkspaceTransactionResult {
        let Some(group) = self.document.groups.get(group_id) else {
            return WorkspaceTransactionResult::unchanged();
        };
        let Some(old_index) = group.panels.iter().position(|id| id == panel_id) else {
            return WorkspaceTransactionResult::unchanged();
        };
        let pinned_boundary = group.pinned_panel_ids.len();
        let is_pinned = group.is_pinned(panel_id);
        let (low, high) = if is_pinned {
            (0, pinned_boundary)
        } else {
            (pinned_boundary, group.panels.len())
        };
        let clamped = index.clamp(low, high);
        let mut next = group.panels.clone();
        next.remove(old_index);
        let adjusted = if old_index < clamped {
            clamped - 1
        } else {
            clamped
        };
        let insert_index = adjusted.min(next.len());
        next.insert(insert_index, panel_id.clone());
        if next == group.panels {
            return WorkspaceTransactionResult::unchanged();
        }
        let mut group = group.clone();
        group.panels = next;
        group
            .pinned_panel_ids
            .retain(|id| group.panels.contains(id));
        let slot = self.document.slot_containing(group_id);
        self.document.groups.insert(group_id.clone(), group);
        if let Some(slot) = slot {
            WorkspaceTransactionResult::changed(vec![slot], Some(panel_id.clone()), true)
        } else {
            WorkspaceTransactionResult::visual_only(Some(panel_id.clone()))
        }
    }

    fn close_panel(&mut self, panel_id: &WorkspacePanelID) -> WorkspaceTransactionResult {
        if !self
            .document
            .panels
            .get(panel_id)
            .is_some_and(|panel| panel.is_closable)
        {
            return WorkspaceTransactionResult::unchanged();
        }
        let Some(located) = self.locate(panel_id) else {
            return WorkspaceTransactionResult::unchanged();
        };
        self.record_closed(
            panel_id.clone(),
            located.group.id.clone(),
            located.slot_id.clone(),
            located.floating_window_id.clone(),
            located.index,
        );
        self.remove(panel_id, &located.group.id);
        let focus = self
            .document
            .groups
            .get(&located.group.id)
            .and_then(|group| group.active_panel_id.clone());
        match located.slot_id {
            Some(slot) => WorkspaceTransactionResult::changed(vec![slot], focus, true),
            None => WorkspaceTransactionResult::visual_only(focus),
        }
    }

    fn close_others(
        &mut self,
        group_id: &WorkspaceTabGroupID,
        keep: &WorkspacePanelID,
    ) -> WorkspaceTransactionResult {
        let Some(group) = self.document.groups.get(group_id).cloned() else {
            return WorkspaceTransactionResult::unchanged();
        };
        if !group.panels.contains(keep) {
            return WorkspaceTransactionResult::unchanged();
        }
        let Some(slot_id) = self.document.slot_containing(group_id) else {
            return WorkspaceTransactionResult::unchanged();
        };
        let mut keep_set: BTreeSet<WorkspacePanelID> =
            group.pinned_panel_ids.iter().cloned().collect();
        keep_set.insert(keep.clone());
        let victims: Vec<(usize, WorkspacePanelID)> = group
            .panels
            .iter()
            .enumerate()
            .filter(|(_, id)| {
                !keep_set.contains(*id)
                    && self
                        .document
                        .panels
                        .get(*id)
                        .is_some_and(|panel| panel.is_closable)
            })
            .map(|(index, id)| (index, id.clone()))
            .collect();
        if victims.is_empty() {
            return WorkspaceTransactionResult::unchanged();
        }
        for (index, victim) in &victims {
            self.record_closed(
                victim.clone(),
                group_id.clone(),
                Some(slot_id.clone()),
                None,
                *index,
            );
        }
        let mut group = group.clone();
        group.panels.retain(|id| keep_set.contains(id));
        group.active_panel_id = Some(keep.clone());
        group
            .pinned_panel_ids
            .retain(|id| group.panels.contains(id));
        self.document.groups.insert(group_id.clone(), group);
        WorkspaceTransactionResult::changed(vec![slot_id], Some(keep.clone()), true)
    }

    fn close_to_the_right(
        &mut self,
        group_id: &WorkspaceTabGroupID,
        pivot_panel_id: &WorkspacePanelID,
    ) -> WorkspaceTransactionResult {
        let Some(group) = self.document.groups.get(group_id).cloned() else {
            return WorkspaceTransactionResult::unchanged();
        };
        let Some(pivot) = group.panels.iter().position(|id| id == pivot_panel_id) else {
            return WorkspaceTransactionResult::unchanged();
        };
        let Some(slot_id) = self.document.slot_containing(group_id) else {
            return WorkspaceTransactionResult::unchanged();
        };
        let victims: Vec<(usize, WorkspacePanelID)> = group
            .panels
            .iter()
            .enumerate()
            .filter(|(index, id)| {
                *index > pivot
                    && !group.is_pinned(id)
                    && self
                        .document
                        .panels
                        .get(*id)
                        .is_some_and(|panel| panel.is_closable)
            })
            .map(|(index, id)| (index, id.clone()))
            .collect();
        if victims.is_empty() {
            return WorkspaceTransactionResult::unchanged();
        }
        for (index, victim) in &victims {
            self.record_closed(
                victim.clone(),
                group_id.clone(),
                Some(slot_id.clone()),
                None,
                *index,
            );
        }
        let removed: BTreeSet<_> = victims.into_iter().map(|(_, id)| id).collect();
        let mut group = group.clone();
        group.panels.retain(|id| !removed.contains(id));
        if group
            .active_panel_id
            .as_ref()
            .is_some_and(|active| removed.contains(active))
        {
            group.active_panel_id = Some(pivot_panel_id.clone());
        }
        group
            .pinned_panel_ids
            .retain(|id| group.panels.contains(id));
        let focus = group.active_panel_id.clone();
        self.document.groups.insert(group_id.clone(), group);
        WorkspaceTransactionResult::changed(vec![slot_id], focus, true)
    }

    fn reopen_last_closed(&mut self) -> WorkspaceTransactionResult {
        while let Some(closed) = self.document.closed_history.pop() {
            if !self.document.panels.contains_key(&closed.panel_id)
                || self.locate(&closed.panel_id).is_some()
            {
                continue;
            }
            return self.restore_closed_panel(closed);
        }
        WorkspaceTransactionResult::unchanged()
    }

    fn reopen_panel(&mut self, panel_id: &WorkspacePanelID) -> WorkspaceTransactionResult {
        if self.locate(panel_id).is_some() || !self.document.panels.contains_key(panel_id) {
            return WorkspaceTransactionResult::unchanged();
        }
        let Some(index) = self
            .document
            .closed_history
            .iter()
            .rposition(|closed| &closed.panel_id == panel_id)
        else {
            return WorkspaceTransactionResult::unchanged();
        };
        let closed = self.document.closed_history.remove(index);
        self.restore_closed_panel(closed)
    }

    fn restore_closed_panel(&mut self, closed: WorkspaceClosedPanel) -> WorkspaceTransactionResult {
        let group_id = closed.group_id.clone();
        let restore_slot = closed
            .slot_id
            .clone()
            .unwrap_or_else(WorkspaceSlotID::center);
        if !self.document.groups.contains_key(&group_id) {
            self.document.groups.insert(
                group_id.clone(),
                WorkspaceTabGroup::new(group_id.clone(), Vec::new()),
            );
        }
        self.ensure_group(&group_id, &restore_slot);
        let mut group = self
            .document
            .groups
            .get(&group_id)
            .cloned()
            .unwrap_or_else(|| WorkspaceTabGroup::new(group_id.clone(), Vec::new()));
        insert_panel(closed.panel_id.clone(), &mut group, closed.index);
        group.active_panel_id = Some(closed.panel_id.clone());
        group.is_collapsed = false;
        self.document.groups.insert(group_id, group);
        WorkspaceTransactionResult::changed(vec![restore_slot], Some(closed.panel_id), true)
    }

    fn set_pinned(
        &mut self,
        panel_id: &WorkspacePanelID,
        is_pinned: bool,
    ) -> WorkspaceTransactionResult {
        let Some(located) = self.locate(panel_id) else {
            return WorkspaceTransactionResult::unchanged();
        };
        let was_pinned = located.group.is_pinned(panel_id);
        if was_pinned == is_pinned {
            return WorkspaceTransactionResult::unchanged();
        }
        let mut group = located.group;
        if is_pinned {
            group.pinned_panel_ids.push(panel_id.clone());
        } else {
            group.pinned_panel_ids.retain(|id| id != panel_id);
        }
        group.panels.retain(|id| id != panel_id);
        let index = if is_pinned {
            group.pinned_panel_ids.len().saturating_sub(1)
        } else {
            group.pinned_panel_ids.len()
        };
        let index = index.min(group.panels.len());
        group.panels.insert(index, panel_id.clone());
        let group_id = group.id.clone();
        self.document.groups.insert(group_id.clone(), group);
        match located.slot_id {
            Some(slot) => {
                WorkspaceTransactionResult::changed(vec![slot], Some(panel_id.clone()), true)
            }
            None => WorkspaceTransactionResult::visual_only(Some(panel_id.clone())),
        }
    }

    fn float_group(
        &mut self,
        group_id: &WorkspaceTabGroupID,
        window_id: WorkspaceFloatingWindowID,
        frame: WorkspaceRect,
    ) -> WorkspaceTransactionResult {
        let Some(group) = self.document.groups.get(group_id) else {
            return WorkspaceTransactionResult::unchanged();
        };
        if group.panels.is_empty()
            || self.document.floating_window_containing(group_id).is_some()
            || self.document.slot_containing(group_id).is_none()
        {
            return WorkspaceTransactionResult::unchanged();
        }
        let slot_id = self.document.slot_containing(group_id);
        let active = group.active_panel_id.clone();
        let first = group.panels.first().cloned();
        let group_id_owned = group_id.clone();
        self.remove_group_from_all_slots(group_id);
        self.document
            .collapsed
            .retain(|item| &item.group_id != group_id);
        let z_index = self.next_floating_z_index();
        let title = active
            .as_ref()
            .and_then(|id| self.document.panels.get(id))
            .map(|panel| panel.title.clone())
            .or_else(|| {
                first
                    .and_then(|id| self.document.panels.get(&id))
                    .map(|panel| panel.title.clone())
            })
            .unwrap_or_else(|| group_id.raw_value().to_string());
        let frame =
            WorkspaceSizingResolver::new(&self.document).floating_frame(frame, group_id, 30.0);
        self.document
            .floating_windows
            .push(WorkspaceFloatingWindow::new(
                window_id,
                group_id_owned,
                title,
                frame,
                z_index,
            ));
        match slot_id {
            Some(slot) => WorkspaceTransactionResult::changed(vec![slot], active, true),
            None => WorkspaceTransactionResult::visual_only(active),
        }
    }

    fn redock_floating_window(
        &mut self,
        window_id: &WorkspaceFloatingWindowID,
        target: WorkspaceTarget,
    ) -> WorkspaceTransactionResult {
        let Some(index) = self
            .document
            .floating_windows
            .iter()
            .position(|window| &window.id == window_id)
        else {
            return WorkspaceTransactionResult::unchanged();
        };
        let window = self.document.floating_windows.remove(index);
        if !self.document.groups.contains_key(&window.group_id) {
            return WorkspaceTransactionResult::changed(Vec::new(), None, true);
        }
        let destination = self.destination(target);
        self.insert_existing_group(&window.group_id, &destination);
        if let Some(group) = self.document.groups.get_mut(&window.group_id) {
            group.is_collapsed = false;
        }
        let focus = self
            .document
            .groups
            .get(&window.group_id)
            .and_then(|group| group.active_panel_id.clone());
        WorkspaceTransactionResult::changed(vec![destination.slot], focus, true)
    }

    fn move_floating_window(
        &mut self,
        window_id: &WorkspaceFloatingWindowID,
        frame: WorkspaceRect,
    ) -> WorkspaceTransactionResult {
        let Some(index) = self
            .document
            .floating_windows
            .iter()
            .position(|window| &window.id == window_id)
        else {
            return WorkspaceTransactionResult::unchanged();
        };
        let group_id = self.document.floating_windows[index].group_id.clone();
        let frame =
            WorkspaceSizingResolver::new(&self.document).floating_frame(frame, &group_id, 30.0);
        if self.document.floating_windows[index].frame == frame {
            return WorkspaceTransactionResult::unchanged();
        }
        self.document.floating_windows[index].frame = frame;
        let focus = self
            .document
            .groups
            .get(&group_id)
            .and_then(|group| group.active_panel_id.clone());
        WorkspaceTransactionResult::visual_only(focus)
    }

    fn focus_floating_window(
        &mut self,
        window_id: &WorkspaceFloatingWindowID,
    ) -> WorkspaceTransactionResult {
        let Some(index) = self
            .document
            .floating_windows
            .iter()
            .position(|window| &window.id == window_id)
        else {
            return WorkspaceTransactionResult::unchanged();
        };
        let z_index = self.next_floating_z_index();
        if self.document.floating_windows[index].z_index == z_index {
            return WorkspaceTransactionResult::unchanged();
        }
        self.document.floating_windows[index].z_index = z_index;
        let group_id = self.document.floating_windows[index].group_id.clone();
        let focus = self
            .document
            .groups
            .get(&group_id)
            .and_then(|group| group.active_panel_id.clone());
        WorkspaceTransactionResult::visual_only(focus)
    }

    fn resize_split(
        &mut self,
        split_id: &WorkspaceSplitID,
        fraction: f32,
    ) -> WorkspaceTransactionResult {
        if !fraction.is_finite() {
            return WorkspaceTransactionResult::unchanged();
        }
        let clamped = WorkspaceSplitFractions::clamp(fraction);
        let target = match split_id.raw_value() {
            "leading" => &mut self.document.split_fractions.leading,
            "centerTrailing" => &mut self.document.split_fractions.center_trailing,
            "topBottom" => &mut self.document.split_fractions.top_bottom,
            "top" => &mut self.document.split_fractions.top,
            _ => return WorkspaceTransactionResult::unchanged(),
        };
        if *target == Some(clamped) {
            return WorkspaceTransactionResult::unchanged();
        }
        *target = Some(clamped);
        let slots = self.document.slots.keys().cloned().collect::<Vec<_>>();
        WorkspaceTransactionResult::changed(slots, None, true)
    }

    fn destination(&self, target: WorkspaceTarget) -> Destination {
        Destination {
            slot: target.slot,
            anchor_group_id: target.group_id,
            placement: target.placement,
            split_edge: target.edge,
            fraction: target.fraction,
        }
    }

    fn remove(&mut self, panel_id: &WorkspacePanelID, group_id: &WorkspaceTabGroupID) {
        let Some(group) = self.document.groups.get(group_id) else {
            return;
        };
        let mut group = group.clone();
        group.panels.retain(|id| id != panel_id);
        group.pinned_panel_ids.retain(|id| id != panel_id);
        if group.panels.is_empty() {
            self.document.groups.remove(group_id);
            self.remove_group_from_all_slots(group_id);
            self.document
                .collapsed
                .retain(|item| &item.group_id != group_id);
            self.document
                .floating_windows
                .retain(|window| &window.group_id != group_id);
            return;
        }
        if group.active_panel_id.as_ref() == Some(panel_id) {
            group.active_panel_id = group.panels.first().cloned();
        }
        self.document.groups.insert(group_id.clone(), group);
    }

    fn locate(&mut self, panel_id: &WorkspacePanelID) -> Option<LocatedPanel> {
        for slot in self.document.slots.values() {
            let Some(layout) = &slot.layout else { continue };
            for group_id in layout.leaf_group_ids() {
                let Some(group) = self.document.groups.get(&group_id) else {
                    continue;
                };
                if let Some(index) = group.panels.iter().position(|id| id == panel_id) {
                    return Some(LocatedPanel {
                        slot_id: Some(slot.id.clone()),
                        floating_window_id: None,
                        group: group.clone(),
                        index,
                    });
                }
            }
        }
        for window in &self.document.floating_windows {
            let Some(group) = self.document.groups.get(&window.group_id) else {
                continue;
            };
            if let Some(index) = group.panels.iter().position(|id| id == panel_id) {
                return Some(LocatedPanel {
                    slot_id: None,
                    floating_window_id: Some(window.id.clone()),
                    group: group.clone(),
                    index,
                });
            }
        }
        None
    }

    fn record_closed(
        &mut self,
        panel_id: WorkspacePanelID,
        group_id: WorkspaceTabGroupID,
        slot_id: Option<WorkspaceSlotID>,
        floating_window_id: Option<WorkspaceFloatingWindowID>,
        index: usize,
    ) {
        self.document
            .closed_history
            .retain(|closed| closed.panel_id != panel_id);
        self.document.closed_history.push(WorkspaceClosedPanel::new(
            panel_id,
            group_id,
            slot_id,
            floating_window_id,
            index,
        ));
        if self.document.closed_history.len() > 64 {
            let excess = self.document.closed_history.len() - 64;
            self.document.closed_history.drain(..excess);
        }
    }

    fn first_group_id(&self, slot_id: &WorkspaceSlotID) -> Option<WorkspaceTabGroupID> {
        self.document
            .slot(slot_id)
            .layout
            .as_ref()
            .and_then(|layout| layout.leaf_group_ids().into_iter().next())
    }

    fn make_group(
        &mut self,
        slot_id: &WorkspaceSlotID,
        anchor_id: Option<WorkspaceTabGroupID>,
        edge: Option<WorkspaceEdge>,
        fraction: f32,
    ) -> WorkspaceTabGroupID {
        let id = WorkspaceTabGroupID::new(format!("{}-{}", slot_id.raw_value(), unique_suffix()));
        self.document
            .groups
            .insert(id.clone(), WorkspaceTabGroup::new(id.clone(), Vec::new()));
        self.insert_group(&id, slot_id, anchor_id, edge, fraction);
        id
    }

    fn make_primary_group(&mut self, slot_id: &WorkspaceSlotID) -> WorkspaceTabGroupID {
        let id = WorkspaceTabGroupID::new(slot_id.raw_value());
        if self.document.groups.contains_key(&id) {
            self.ensure_group(&id, slot_id);
            return id;
        }
        self.document
            .groups
            .insert(id.clone(), WorkspaceTabGroup::new(id.clone(), Vec::new()));
        self.ensure_group(&id, slot_id);
        id
    }

    fn ensure_group(&mut self, group_id: &WorkspaceTabGroupID, slot_id: &WorkspaceSlotID) {
        let mut slot = self.document.slot(slot_id);
        if slot.contains_group(group_id) {
            return;
        }
        slot.append_group(group_id.clone());
        self.document.set_slot(slot);
    }

    fn insert_group(
        &mut self,
        group_id: &WorkspaceTabGroupID,
        slot_id: &WorkspaceSlotID,
        anchor_id: Option<WorkspaceTabGroupID>,
        edge: Option<WorkspaceEdge>,
        fraction: f32,
    ) {
        let mut slot = self.document.slot(slot_id);
        slot.layout = slot
            .layout
            .as_ref()
            .and_then(|layout| layout.removing(group_id));
        match (&anchor_id, edge) {
            (Some(anchor), Some(edge))
                if slot
                    .layout
                    .as_ref()
                    .is_some_and(|layout| layout.contains_group(anchor)) =>
            {
                slot.layout = Some(split_at_anchor(
                    slot.layout.take(),
                    anchor,
                    group_id,
                    edge,
                    fraction,
                ));
            }
            _ => slot.append_group(group_id.clone()),
        }
        self.document.set_slot(slot);
    }

    fn insert_existing_group(&mut self, group_id: &WorkspaceTabGroupID, destination: &Destination) {
        self.remove_group_from_all_slots(group_id);
        self.document
            .collapsed
            .retain(|item| &item.group_id != group_id);
        let (anchor, edge) = match destination.placement {
            WorkspacePlacementKind::Split => {
                (destination.anchor_group_id.clone(), destination.split_edge)
            }
            _ => (None, None),
        };
        self.insert_group(
            group_id,
            &destination.slot,
            anchor,
            edge,
            destination.fraction,
        );
    }

    fn next_floating_z_index(&self) -> i32 {
        self.document
            .floating_windows
            .iter()
            .map(|window| window.z_index)
            .max()
            .unwrap_or(0)
            + 1
    }

    fn remove_group_from_all_slots(&mut self, group_id: &WorkspaceTabGroupID) {
        let slot_ids: Vec<_> = self.document.slots.keys().cloned().collect();
        for slot_id in slot_ids {
            if let Some(slot) = self.document.slots.get_mut(&slot_id) {
                slot.layout = slot
                    .layout
                    .as_ref()
                    .and_then(|layout| layout.removing(group_id));
            }
        }
    }

    fn active_center_panel(&self) -> Option<WorkspacePanelID> {
        self.document
            .slot(&WorkspaceSlotID::center())
            .layout
            .as_ref()
            .map(|layout| layout.leaf_group_ids())
            .unwrap_or_default()
            .into_iter()
            .find_map(|id| {
                self.document
                    .groups
                    .get(&id)
                    .and_then(|group| group.active_panel_id.clone())
            })
    }

    fn collapsed_edge(&self, slot_id: &WorkspaceSlotID) -> WorkspaceEdge {
        if let super::model::WorkspaceSlotKind::Chrome { edge, .. } =
            self.document.slot(slot_id).kind
        {
            return edge;
        }
        if slot_id == &WorkspaceSlotID::leading() {
            WorkspaceEdge::Leading
        } else if slot_id == &WorkspaceSlotID::trailing() {
            WorkspaceEdge::Trailing
        } else if slot_id == &WorkspaceSlotID::top() {
            WorkspaceEdge::Top
        } else {
            WorkspaceEdge::Bottom
        }
    }

    fn notify_change(&mut self) {
        // Move the callbacks out for the duration of the notification so a
        // handler that dispatches re-entrantly cannot hold two mutable borrows
        // of the same map. Handlers registered during the loop are appended to
        // the live map and run on the next change, like Swift's snapshot.
        let mut subscribers = std::mem::take(&mut self.subscribers);
        let tokens: Vec<SubscriptionToken> = subscribers.keys().copied().collect();
        for token in tokens {
            if let Some(callback) = subscribers.get_mut(&token) {
                callback(self);
            }
        }
        // Preserve any subscriber added re-entrantly while draining.
        for (token, callback) in subscribers {
            self.subscribers.entry(token).or_insert(callback);
        }
    }
}

/// Insert keeping pinned panels ahead of the requested index.
fn insert_panel(panel_id: WorkspacePanelID, group: &mut WorkspaceTabGroup, requested_index: usize) {
    group.panels.retain(|id| id != &panel_id);
    let pinned_boundary = group.pinned_panel_ids.len();
    let index = requested_index.clamp(pinned_boundary, group.panels.len());
    group.panels.insert(index, panel_id);
}

fn split_at_anchor(
    node: Option<WorkspaceLayoutNode>,
    anchor_group_id: &WorkspaceTabGroupID,
    inserted_group_id: &WorkspaceTabGroupID,
    edge: WorkspaceEdge,
    fraction: f32,
) -> WorkspaceLayoutNode {
    let Some(node) = node else {
        return WorkspaceLayoutNode::Group(inserted_group_id.clone());
    };
    match node {
        WorkspaceLayoutNode::Group(current) if &current == anchor_group_id => {
            let inserted = WorkspaceLayoutNode::Group(inserted_group_id.clone());
            let anchor = WorkspaceLayoutNode::Group(anchor_group_id.clone());
            if edge.inserts_before_anchor() {
                WorkspaceLayoutNode::split(edge.split_axis(), fraction, inserted, anchor)
            } else {
                WorkspaceLayoutNode::split(edge.split_axis(), 1.0 - fraction, anchor, inserted)
            }
        }
        other @ WorkspaceLayoutNode::Group(_) => other,
        WorkspaceLayoutNode::Split {
            axis,
            fraction: current,
            first,
            second,
        } => WorkspaceLayoutNode::split(
            axis,
            current,
            split_at_anchor(
                Some(*first),
                anchor_group_id,
                inserted_group_id,
                edge,
                fraction,
            ),
            split_at_anchor(
                Some(*second),
                anchor_group_id,
                inserted_group_id,
                edge,
                fraction,
            ),
        ),
    }
}

/// Monotonic process-local counter. Swift uses `UUID()` here only to keep new
/// group ids unique within one document; a counter is deterministic and
/// satisfies the same requirement.
fn unique_suffix() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::super::model::WorkspacePanel;
    use super::*;

    fn controller() -> WorkspaceController {
        let panel = WorkspacePanel::new(WorkspacePanelID::new("viewport"), "Viewport");
        let mut panels = BTreeMap::new();
        panels.insert(panel.id.clone(), panel);
        let group = WorkspaceTabGroup::new(
            WorkspaceTabGroupID::new("center"),
            vec![WorkspacePanelID::new("viewport")],
        );
        let mut groups = BTreeMap::new();
        groups.insert(group.id.clone(), group);
        let mut slots = BTreeMap::new();
        slots.insert(
            WorkspaceSlotID::center(),
            WorkspaceSlot::new(
                WorkspaceSlotID::center(),
                super::super::model::WorkspaceSlotKind::Content,
                Some(WorkspaceLayoutNode::Group(WorkspaceTabGroupID::new(
                    "center",
                ))),
            ),
        );
        WorkspaceController::new(WorkspaceDocument::new(panels, groups, slots))
    }

    #[test]
    fn center_cannot_collapse() {
        let mut controller = controller();
        assert_eq!(
            controller.dispatch(WorkspaceCommand::Collapse(WorkspaceTabGroupID::new(
                "center"
            ))),
            WorkspaceTransactionResult::unchanged()
        );
        assert_eq!(controller.version(), 0);
    }

    #[test]
    fn maximize_toggles_without_dirtying_persistence() {
        let mut controller = controller();
        let panel = WorkspacePanelID::new("viewport");
        let result = controller.dispatch(WorkspaceCommand::ToggleMaximize(panel.clone()));
        assert!(result.did_change && !result.persistence_dirty);
        assert_eq!(controller.maximized_panel_id(), Some(&panel));
        controller.dispatch(WorkspaceCommand::ToggleMaximize(panel));
        assert!(controller.maximized_panel_id().is_none());
        assert_eq!(controller.version(), 2);
    }

    #[test]
    fn closing_the_last_panel_removes_the_group_and_slot_reference() {
        let mut controller = controller();
        let result = controller.dispatch(WorkspaceCommand::ClosePanel(WorkspacePanelID::new(
            "viewport",
        )));
        assert!(result.did_change);
        assert!(controller.document().groups.is_empty());
        assert!(
            controller
                .document()
                .slot(&WorkspaceSlotID::center())
                .layout
                .is_none()
        );
        assert_eq!(controller.document().closed_history.len(), 1);
        controller.dispatch(WorkspaceCommand::ReopenLastClosed);
        assert!(
            controller
                .document()
                .groups
                .contains_key(&WorkspaceTabGroupID::new("center"))
        );
    }

    #[test]
    fn observers_fire_once_per_effective_dispatch() {
        use std::cell::Cell;
        use std::rc::Rc;
        let mut controller = controller();
        let hits = Rc::new(Cell::new(0));
        let counter = hits.clone();
        controller.subscribe(move |_| counter.set(counter.get() + 1));
        controller.dispatch(WorkspaceCommand::Collapse(WorkspaceTabGroupID::new(
            "center",
        )));
        assert_eq!(hits.get(), 0);
        controller.dispatch(WorkspaceCommand::ToggleMaximize(WorkspacePanelID::new(
            "viewport",
        )));
        assert_eq!(hits.get(), 1);
    }
}
