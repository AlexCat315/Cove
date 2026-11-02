//! Primitive widgets grouped by category.
pub mod columns;
pub mod controls;
pub mod data;
pub mod feedback;
pub mod interaction;
pub mod layout;
pub mod media;
pub mod navigation;
pub mod overlays;
pub mod text;

/// An authored collection contains the same identity twice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DuplicateId(pub &'static str);

impl std::fmt::Display for DuplicateId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} IDs must be unique", self.0)
    }
}

impl std::error::Error for DuplicateId {}

pub(crate) fn check_ids<T: Eq + std::hash::Hash>(
    mut ids: impl Iterator<Item = T>,
    collection: &'static str,
) -> Result<(), DuplicateId> {
    let mut seen = std::collections::HashSet::with_capacity(ids.size_hint().0);
    if ids.all(|id| seen.insert(id)) {
        Ok(())
    } else {
        Err(DuplicateId(collection))
    }
}
