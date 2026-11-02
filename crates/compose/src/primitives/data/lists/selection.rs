//! Translation of Primitives/Data/Lists/Selection.swift.
//!
//! What every collection shares: the selection it writes to, the identity bound
//! on its rows, the state it hands each row and the keyboard and pointer rules
//! for moving through flat rows.
use std::collections::BTreeSet;
use std::rc::Rc;

use cove_core::{Binding, State};
use cove_platform::{KeyEvent, KeyModifiers};

/// The bound on row identity across `List`, `SectionedList`, `ReorderableList`,
/// `SwipeList`, `Tree` and `DataTable`.
pub trait CollectionId: Ord + std::hash::Hash + Clone + Send + Sync + 'static {}
impl<T: Ord + std::hash::Hash + Clone + Send + Sync + 'static> CollectionId for T {}

/// What a row closure knows about its row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RowState {
    pub is_selected: bool,
    pub is_expanded: bool,
    pub is_dragging: bool,
    /// Depth in a hierarchy; zero in a flat collection.
    pub depth: usize,
}

impl RowState {
    pub fn selected(is_selected: bool) -> Self {
        Self {
            is_selected,
            ..Self::default()
        }
    }
}

/// The selection a collection writes to: one optional ID or a set of IDs.
#[derive(Clone)]
pub enum Selection<Id: Ord + Clone> {
    Single(Binding<Option<Id>>),
    Multiple(Binding<BTreeSet<Id>>),
}

impl<Id: Ord + Clone + 'static> Selection<Id> {
    pub fn single(binding: Binding<Option<Id>>) -> Self {
        Self::Single(binding)
    }

    pub fn multiple(binding: Binding<BTreeSet<Id>>) -> Self {
        Self::Multiple(binding)
    }

    /// A collection nothing can select in.
    pub fn none() -> Self {
        Self::Single(Binding::constant(None))
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

    /// Every selected ID.
    pub fn ids(&self) -> BTreeSet<Id> {
        match self {
            Self::Single(binding) => binding.get().into_iter().collect(),
            Self::Multiple(binding) => binding.get(),
        }
    }
}

/// Flat rows, the selection and its anchor and cursor.
///
/// With multiple selection a plain click selects one row, Shift-click extends
/// from the anchor, Cmd/Ctrl-click toggles, Shift+Arrow extends, Space toggles
/// the cursor row and Cmd/Ctrl+A selects everything.
pub(crate) struct ListCore<Id: CollectionId> {
    pub ids: Rc<[Id]>,
    pub selection: Selection<Id>,
    pub anchor: State<Option<Id>>,
    pub cursor: State<Option<Id>>,
    pub on_activate: Option<Rc<dyn Fn(&Id)>>,
}

impl<Id: CollectionId> Clone for ListCore<Id> {
    fn clone(&self) -> Self {
        Self {
            ids: self.ids.clone(),
            selection: self.selection.clone(),
            anchor: self.anchor.clone(),
            cursor: self.cursor.clone(),
            on_activate: self.on_activate.clone(),
        }
    }
}

impl<Id: CollectionId> ListCore<Id> {
    fn index_of(&self, target: &Id) -> Option<usize> {
        self.ids.iter().position(|id| id == target)
    }

    fn ids_between(&self, first: &Id, second: &Id) -> Option<Vec<Id>> {
        let start = self.index_of(first)?;
        let end = self.index_of(second)?;
        let (low, high) = (start.min(end), start.max(end));
        Some(self.ids[low..=high].to_vec())
    }

    pub fn tap(&self, id: Id, modifiers: KeyModifiers) {
        if modifiers.has_shift()
            && let Some(anchor) = self.anchor.get()
            && let Some(range) = self.ids_between(&anchor, &id)
        {
            self.selection.replace(range);
        } else if modifiers.has_gui() || modifiers.has_ctrl() {
            self.selection.toggle(id.clone());
            self.anchor.set(Some(id.clone()));
        } else {
            self.selection.replace([id.clone()]);
            self.anchor.set(Some(id.clone()));
        }
        self.cursor.set(Some(id));
    }

    pub fn activate(&self, id: Id) {
        self.selection.replace([id.clone()]);
        self.anchor.set(Some(id.clone()));
        self.cursor.set(Some(id.clone()));
        if let Some(handler) = self.on_activate.as_ref() {
            handler(&id);
        }
    }

    /// The row keyboard movement starts from. Single selection follows the
    /// binding, so an outside write moves the keyboard too.
    fn current(&self) -> Option<usize> {
        let cursor = self.cursor.get().and_then(|id| self.index_of(&id));
        let selected = self.selection.first().and_then(|id| self.index_of(&id));
        if self.selection.allows_multiple() {
            cursor.or(selected)
        } else {
            selected.or(cursor)
        }
    }

    /// The row a scrolling list keeps in view.
    pub fn target(&self) -> Option<usize> {
        self.cursor
            .get()
            .or_else(|| self.selection.first())
            .and_then(|id| self.index_of(&id))
    }

    /// Keyboard handling for the cursor row.
    pub fn handle_key(&self, event: &KeyEvent) -> bool {
        if self.ids.is_empty() {
            return false;
        }
        let multiple = self.selection.allows_multiple();
        let current = self.current();
        if multiple
            && (event.modifiers.has_gui() || event.modifiers.has_ctrl())
            && event.scancode == crate::Scancode::A
        {
            self.selection.replace(self.ids.iter().cloned());
            self.anchor.set(self.ids.first().cloned());
            self.cursor.set(self.ids.last().cloned());
            return true;
        }
        if multiple
            && event.scancode == crate::Scancode::SPACE
            && let Some(current) = current
        {
            let value = self.ids[current].clone();
            self.selection.toggle(value.clone());
            self.anchor.set(Some(value.clone()));
            self.cursor.set(Some(value));
            return true;
        }
        let next = match event.scancode {
            crate::Scancode::ARROW_DOWN => {
                (current.map_or(0, |index| index + 1)).min(self.ids.len() - 1)
            }
            crate::Scancode::ARROW_UP => {
                current.map_or(self.ids.len() - 1, |index| index.saturating_sub(1))
            }
            crate::Scancode::HOME => 0,
            crate::Scancode::END => self.ids.len() - 1,
            crate::Scancode::RETURN | crate::Scancode::KEYPAD_ENTER => {
                let Some(current) = current else { return false };
                self.activate(self.ids[current].clone());
                return true;
            }
            _ => return false,
        };
        let value = self.ids[next].clone();
        if multiple
            && event.modifiers.has_shift()
            && let Some(anchor) = self.anchor.get()
            && let Some(range) = self.ids_between(&anchor, &value)
        {
            self.selection.replace(range);
        } else {
            self.selection.replace([value.clone()]);
            self.anchor.set(Some(value.clone()));
        }
        self.cursor.set(Some(value));
        true
    }
}
