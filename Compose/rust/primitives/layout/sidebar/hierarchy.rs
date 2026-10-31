//! Translation of Primitives/Layout/Sidebar/Hierarchy.swift.
//!
//! A transient projection of authored navigation, shared by drawing and input.
use std::collections::HashSet;

use super::items::{SidebarItem, SidebarSection};

/// A flattened navigation entry with its ancestor path.
#[derive(Clone)]
pub struct SidebarEntry<Id: PartialEq + Eq + std::hash::Hash + Clone> {
    pub item: SidebarItem<Id>,
    pub ancestors: Vec<Id>,
    pub is_enabled: bool,
}

impl<Id: PartialEq + Eq + std::hash::Hash + Clone> SidebarEntry<Id> {
    pub fn is_branch(&self) -> bool {
        !self.item.children.is_empty()
    }

    pub fn is_visible(&self, expanded: &HashSet<Id>) -> bool {
        self.ancestors
            .iter()
            .all(|ancestor| expanded.contains(ancestor))
    }
}

/// Flattened projection of the authored sections.
#[derive(Clone, Default)]
pub struct SidebarHierarchy<Id: PartialEq + Eq + std::hash::Hash + Clone> {
    pub groups: Vec<Vec<SidebarEntry<Id>>>,
}

impl<Id: PartialEq + Eq + std::hash::Hash + Clone> SidebarHierarchy<Id> {
    pub fn new(sections: &[SidebarSection<Id>]) -> Self {
        fn flatten<Id: PartialEq + Eq + std::hash::Hash + Clone>(
            items: &[SidebarItem<Id>],
            ancestors: &[Id],
            enabled: bool,
        ) -> Vec<SidebarEntry<Id>> {
            let mut result = Vec::new();
            for item in items {
                let entry = SidebarEntry {
                    item: item.clone(),
                    ancestors: ancestors.to_vec(),
                    is_enabled: enabled && item.is_enabled,
                };
                let mut child_ancestors = ancestors.to_vec();
                child_ancestors.push(item.id.clone());
                let child_enabled = entry.is_enabled;
                result.push(entry);
                result.extend(flatten(&item.children, &child_ancestors, child_enabled));
            }
            result
        }
        Self {
            groups: sections
                .iter()
                .map(|section| flatten(&section.items, &[], true))
                .collect(),
        }
    }

    pub fn entries(&self) -> Vec<SidebarEntry<Id>> {
        self.groups.iter().flatten().cloned().collect()
    }

    pub fn entry(&self, id: &Id) -> Option<SidebarEntry<Id>> {
        self.entries()
            .into_iter()
            .find(|entry| &entry.item.id == id)
    }

    /// Ancestor path plus the id itself.
    pub fn path_to(&self, id: &Id) -> Vec<Id> {
        match self.entry(id) {
            Some(entry) => {
                let mut path = entry.ancestors.clone();
                path.push(entry.item.id.clone());
                path
            }
            None => Vec::new(),
        }
    }

    /// Human-readable breadcrumb for a row.
    pub fn context(&self, entry: &SidebarEntry<Id>) -> String {
        let mut parts: Vec<String> = entry
            .ancestors
            .iter()
            .filter_map(|ancestor| self.entry(ancestor).map(|found| found.item.title.clone()))
            .collect();
        parts.push(entry.item.title.clone());
        parts.join(" / ")
    }

    /// Where focus should go when `id` disappears: the nearest visible enabled
    /// ancestor, else the first visible enabled entry.
    pub fn fallback(&self, id: &Id, expanded: &HashSet<Id>, previous: Option<&Self>) -> Option<Id> {
        let entry = self
            .entry(id)
            .or_else(|| previous.and_then(|previous| previous.entry(id)));
        if let Some(entry) = entry {
            for parent in entry.ancestors.iter().rev() {
                if let Some(found) = self.entry(parent)
                    && found.is_enabled
                    && found.is_visible(expanded)
                {
                    return Some(parent.clone());
                }
            }
        }
        self.entries()
            .into_iter()
            .find(|candidate| candidate.is_enabled && candidate.is_visible(expanded))
            .map(|candidate| candidate.item.id)
    }

    /// Enabled entries that are currently visible in the expanded tree.
    pub fn visible_enabled(&self, expanded: &HashSet<Id>) -> Vec<Id> {
        self.entries()
            .into_iter()
            .filter(|entry| entry.is_enabled && entry.is_visible(expanded))
            .map(|entry| entry.item.id)
            .collect()
    }
}
