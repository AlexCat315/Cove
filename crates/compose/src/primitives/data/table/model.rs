//! Translation of `Primitives/Data/Table/{TableModel,TableSortWorker}.swift`
//! (the model half) and the row-storage core of `Table.swift`.
//!
//! UI-owned row storage and a persistent display projection. Rendering reads
//! rows and ID positions in O(1); a sort builds a projection once per command
//! and caches the last two.
//!
//! Behaviour checklist:
//! - a new model starts unsorted and reads rows in source order
//! - replacing rows resets the projection and reapplies an active sort
//! - updating a row keeps its identity and rejects a replacement ID
//! - an equal comparison retains source order in both directions
//! - a repeated sort command is a no-op
//! - a cached projection is reused instead of rebuilt
//! - every accepted mutation bumps the revision

use crate::primitives::DuplicateId;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use cove_core::scene::{ObservableObject, ObservablePublisher};

use super::row_metrics::TableProjection;
use super::table_column::{ComparisonResult, TableSort, TableSortDirection};

/// Background-sort telemetry, surfaced so a caller can show progress.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TableSortingStatus {
    pub is_pending: bool,
    pub completed_builds: usize,
    pub cache_hits: usize,
    pub last_duration: f64,
}

struct RowStorage<Record, Id: Eq + std::hash::Hash + Clone> {
    records: Vec<Record>,
    id: Rc<dyn Fn(&Record) -> Id>,
    ids: Vec<Id>,
    positions: HashMap<Id, usize>,
}

/// UI-owned row storage with a persistent display projection.
pub struct TableModel<Record: 'static, Id: Eq + std::hash::Hash + Clone + 'static> {
    storage: RefCell<RowStorage<Record, Id>>,
    projection: RefCell<Option<TableProjection<Id>>>,
    cache: RefCell<Vec<(TableSort, TableProjection<Id>)>>,
    generation: RefCell<u64>,
    comparator: RefCell<Option<Rc<dyn Fn(&Record, &Record) -> ComparisonResult>>>,
    sort: RefCell<Option<TableSort>>,
    sorting: RefCell<TableSortingStatus>,
    revision: RefCell<u64>,
    publisher: ObservablePublisher,
}

impl<Record: 'static, Id: Eq + std::hash::Hash + Clone + 'static> ObservableObject
    for TableModel<Record, Id>
{
    fn register_observer(&self, handler: Rc<dyn Fn()>) -> u64 {
        self.publisher.register_observer(handler)
    }
    fn unregister_observer(&self, token: u64) {
        self.publisher.unregister_observer(token);
    }
}

impl<Record: 'static, Id: Eq + std::hash::Hash + Clone + 'static> TableModel<Record, Id> {
    pub fn try_new(
        records: Vec<Record>,
        id: impl Fn(&Record) -> Id + 'static,
    ) -> Result<Rc<Self>, DuplicateId> {
        let storage = RowStorage::try_new(records, Rc::new(id))?;
        Ok(Rc::new(Self {
            storage: RefCell::new(storage),
            projection: RefCell::new(None),
            cache: RefCell::new(Vec::new()),
            generation: RefCell::new(0),
            comparator: RefCell::new(None),
            sort: RefCell::new(None),
            sorting: RefCell::new(TableSortingStatus::default()),
            revision: RefCell::new(0),
            publisher: ObservablePublisher::default(),
        }))
    }

    pub fn count(&self) -> usize {
        self.storage.borrow().records.len()
    }

    pub fn record_at(&self, index: usize) -> Option<Record>
    where
        Record: Clone,
    {
        let storage = self.storage.borrow();
        let source = self.source_index(&storage, index)?;
        storage.records.get(source).cloned()
    }

    pub fn row_id_at(&self, index: usize) -> Option<Id> {
        let storage = self.storage.borrow();
        let source = self.source_index(&storage, index)?;
        storage.ids.get(source).cloned()
    }

    /// Display position of `id`, in either the projection or the source order.
    pub fn index_for(&self, id: &Id) -> Option<usize> {
        if let Some(projection) = self.projection.borrow().as_ref()
            && let Some(position) = projection.positions.get(id)
        {
            return Some(*position);
        }
        self.storage.borrow().positions.get(id).copied()
    }

    pub fn record_for(&self, id: &Id) -> Option<Record>
    where
        Record: Clone,
    {
        let source = *self.storage.borrow().positions.get(id)?;
        self.storage.borrow().records.get(source).cloned()
    }

    pub fn sort(&self) -> Option<TableSort> {
        self.sort.borrow().clone()
    }

    pub fn sorting(&self) -> TableSortingStatus {
        *self.sorting.borrow()
    }

    pub fn revision(&self) -> u64 {
        *self.revision.borrow()
    }

    /// Replaces every row. An active sort is reapplied to the new rows.
    pub fn replace_rows(&self, records: Vec<Record>) -> Result<(), DuplicateId> {
        let id = self.storage.borrow().id.clone();
        let next = RowStorage::try_new(records, id)?;
        *self.storage.borrow_mut() = next;
        *self.projection.borrow_mut() = None;
        self.cache.borrow_mut().clear();
        *self.generation.borrow_mut() += 1;
        self.rebuild_projection();
        Ok(())
    }

    /// Mutates one row in place. Returns `false` when the ID is unknown or the edit changes it.
    ///
    /// The updater must preserve the row's identity; that is what keeps
    /// selection, viewport and an open draft pointing at the same row.
    pub fn update_row(&self, id: &Id, update: impl FnOnce(&mut Record)) -> bool
    where
        Record: Clone,
    {
        let source = match self.storage.borrow().positions.get(id).copied() {
            Some(source) => source,
            None => return false,
        };
        {
            let mut storage = self.storage.borrow_mut();
            let id_fn = storage.id.clone();
            let Some(record) = storage.records.get_mut(source) else {
                return false;
            };
            let mut next = record.clone();
            update(&mut next);
            if id_fn(&next) != *id {
                return false;
            }
            *record = next;
        }
        self.cache.borrow_mut().clear();
        *self.generation.borrow_mut() += 1;
        self.rebuild_projection();
        true
    }

    /// Sets the active sort, reusing a cached projection when one matches.
    pub fn set_sort(
        &self,
        next: Option<TableSort>,
        compare: Option<Rc<dyn Fn(&Record, &Record) -> ComparisonResult>>,
    ) {
        if *self.sort.borrow() == next {
            return;
        }
        *self.generation.borrow_mut() += 1;
        *self.sort.borrow_mut() = next.clone();
        *self.comparator.borrow_mut() = compare.clone();
        let (Some(next), Some(compare)) = (next, compare) else {
            *self.projection.borrow_mut() = None;
            self.sorting.borrow_mut().is_pending = false;
            self.publish();
            return;
        };
        if let Some(projection) = self
            .cache
            .borrow()
            .iter()
            .find(|(sort, _)| *sort == next)
            .map(|(_, projection)| projection.clone())
        {
            *self.projection.borrow_mut() = Some(projection);
            let mut sorting = self.sorting.borrow_mut();
            sorting.cache_hits += 1;
            sorting.is_pending = false;
            drop(sorting);
            self.publish();
            return;
        }
        self.schedule(next, compare);
    }

    fn schedule(&self, sort: TableSort, compare: Rc<dyn Fn(&Record, &Record) -> ComparisonResult>) {
        self.sorting.borrow_mut().is_pending = true;
        self.publish();
        let started = std::time::Instant::now();
        let projection = {
            let storage = self.storage.borrow();
            project(&storage.records, &storage.ids, &sort, &compare)
        };
        *self.projection.borrow_mut() = Some(projection.clone());
        {
            let mut cache = self.cache.borrow_mut();
            cache.retain(|(cached, _)| *cached != sort);
            cache.push((sort, projection));
            if cache.len() > 2 {
                cache.remove(0);
            }
        }
        let mut sorting = self.sorting.borrow_mut();
        sorting.is_pending = false;
        sorting.completed_builds += 1;
        sorting.last_duration = started.elapsed().as_secs_f64();
        drop(sorting);
        self.publish();
    }

    fn rebuild_projection(&self) {
        match (self.sort.borrow().clone(), self.comparator.borrow().clone()) {
            (Some(sort), Some(compare)) => self.schedule(sort, compare),
            _ => {
                self.sorting.borrow_mut().is_pending = false;
                self.publish();
            }
        }
    }

    fn source_index(&self, storage: &RowStorage<Record, Id>, index: usize) -> Option<usize> {
        match self.projection.borrow().as_ref() {
            Some(projection) => projection.order.get(index).copied(),
            None if index < storage.records.len() => Some(index),
            None => None,
        }
    }

    fn publish(&self) {
        *self.revision.borrow_mut() += 1;
        self.publisher.send();
    }
}

impl<Record, Id: Eq + std::hash::Hash + Clone> RowStorage<Record, Id> {
    fn try_new(records: Vec<Record>, id: Rc<dyn Fn(&Record) -> Id>) -> Result<Self, DuplicateId> {
        let ids: Vec<Id> = records.iter().map(|record| id(record)).collect();
        let mut positions = HashMap::with_capacity(records.len());
        for (index, value) in ids.iter().enumerate() {
            if positions.insert(value.clone(), index).is_some() {
                return Err(DuplicateId("Data table row"));
            }
        }
        Ok(Self {
            records,
            id,
            ids,
            positions,
        })
    }
}

/// Builds the display order for one sort command.
///
/// Equal comparisons fall back to the source index in both directions, so a
/// descending sort does not reverse ties.
/// The source indices in sorted order.
///
/// Equal comparisons retain source order in both directions, so a stable sort
/// never reshuffles rows the comparator cannot separate. Shared by the async
/// `Table` projection and the synchronous `Table`.
pub fn sorted_order<Record>(
    records: &[Record],
    sort: &TableSort,
    compare: &dyn Fn(&Record, &Record) -> ComparisonResult,
) -> Vec<usize> {
    let wanted = match sort.direction {
        TableSortDirection::Ascending => ComparisonResult::Ascending,
        TableSortDirection::Descending => ComparisonResult::Descending,
    };
    let mut order: Vec<usize> = (0..records.len()).collect();
    order.sort_by(|&left, &right| {
        let result = compare(&records[left], &records[right]);
        if result == ComparisonResult::Same {
            left.cmp(&right)
        } else if result == wanted {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        }
    });
    order
}

fn project<Record, Id: Eq + std::hash::Hash + Clone>(
    records: &[Record],
    ids: &[Id],
    sort: &TableSort,
    compare: &Rc<dyn Fn(&Record, &Record) -> ComparisonResult>,
) -> TableProjection<Id> {
    let order = sorted_order(records, sort, compare.as_ref());
    let positions = order
        .iter()
        .enumerate()
        .filter_map(|(position, source)| ids.get(*source).map(|id| (id.clone(), position)))
        .collect();
    TableProjection { order, positions }
}

/// Whether a sort is currently ascending.
pub fn sorts_ascending(sort: &TableSort) -> bool {
    sort.direction == TableSortDirection::Ascending
}
