//! Translation of Primitives/Data/Lists/Selection.swift.
//!
//! The selection a collection writes to: one optional ID or a set of IDs.
//! Callers never see this type; they pass the matching binding to the
//! collection's initializer.
use std::collections::BTreeSet;

use cove_core::Binding;

/// Single or multiple collection selection.
#[derive(Clone)]
pub enum CollectionSelection<Id: Ord + Clone> {
    Single(Binding<Option<Id>>),
    Multiple(Binding<BTreeSet<Id>>),
}

impl<Id: Ord + Clone> CollectionSelection<Id> {
    pub fn single(binding: Binding<Option<Id>>) -> Self {
        Self::Single(binding)
    }

    pub fn multiple(binding: Binding<BTreeSet<Id>>) -> Self {
        Self::Multiple(binding)
    }

    /// Multiple selection enables Shift ranges, Cmd/Ctrl toggles, Space and
    /// select-all.
    pub fn allows_multiple(&self) -> bool {
        matches!(self, Self::Multiple(_))
    }

    pub fn contains(&self, id: &Id) -> bool {
        match self {
            Self::Single(binding) => binding.get().as_ref() == Some(id),
            Self::Multiple(binding) => binding.get().contains(id),
        }
    }

    /// The single selected ID, or any member of a multiple selection.
    pub fn first(&self) -> Option<Id> {
        match self {
            Self::Single(binding) => binding.get(),
            Self::Multiple(binding) => binding.get().into_iter().next(),
        }
    }

    pub fn replace(&self, ids: impl IntoIterator<Item = Id>) {
        match self {
            Self::Single(binding) => {
                let last = ids.into_iter().last();
                if binding.get() != last {
                    binding.set(last);
                }
            }
            Self::Multiple(binding) => {
                let next: BTreeSet<Id> = ids.into_iter().collect();
                if binding.get() != next {
                    binding.set(next);
                }
            }
        }
    }

    pub fn toggle(&self, id: Id) {
        match self {
            Self::Single(binding) => binding.set(Some(id)),
            Self::Multiple(binding) => {
                let mut next = binding.get();
                if !next.remove(&id) {
                    next.insert(id);
                }
                binding.set(next);
            }
        }
    }

    pub fn all(&self) -> Vec<Id> {
        match self {
            Self::Single(_) => Vec::new(),
            Self::Multiple(binding) => binding.get().into_iter().collect(),
        }
    }
}
