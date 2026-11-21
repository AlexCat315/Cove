use std::sync::Arc;

use crate::snapshot_store::SnapshotMap;

use super::{
    MemoryError, MemorySnapshot, MemoryStore, check_revision, next_revision, validate_namespace,
};

#[derive(Default)]
pub struct InMemoryStore {
    snapshots: SnapshotMap<MemorySnapshot>,
}

impl MemoryStore for InMemoryStore {
    fn load(&self, namespace: &str) -> Result<Option<Arc<MemorySnapshot>>, MemoryError> {
        validate_namespace(namespace)?;
        Ok(self.snapshots.load(namespace)?)
    }

    fn compare_exchange(
        &self,
        namespace: &str,
        expected_revision: u64,
        snapshot: Arc<MemorySnapshot>,
    ) -> Result<(), MemoryError> {
        validate_update(namespace, expected_revision, &snapshot)?;
        let actual = self.snapshots.compare_exchange(
            namespace,
            expected_revision,
            snapshot,
            MemorySnapshot::revision,
        )?;
        check_revision(expected_revision, actual.unwrap_or(expected_revision))
    }
}

fn validate_update(
    namespace: &str,
    expected_revision: u64,
    snapshot: &MemorySnapshot,
) -> Result<(), MemoryError> {
    validate_namespace(namespace)?;
    snapshot.validate_namespace(namespace)?;
    if snapshot.revision() != next_revision(expected_revision)? {
        return Err(MemoryError::Invalid(
            "replacement must be the next revision",
        ));
    }
    Ok(())
}

#[cfg(feature = "memory-files")]
mod files {
    use std::{io, path::Path};

    use crate::snapshot_store::FileSnapshots;

    use super::*;

    /// Owner-only JSON snapshots with a file lock per namespace.
    pub struct FileMemoryStore {
        snapshots: FileSnapshots,
    }

    impl FileMemoryStore {
        pub fn open(directory: impl AsRef<Path>) -> Result<Self, MemoryError> {
            Ok(Self {
                snapshots: FileSnapshots::open(directory.as_ref(), "memory")?,
            })
        }
    }

    impl MemoryStore for FileMemoryStore {
        fn load(&self, namespace: &str) -> Result<Option<Arc<MemorySnapshot>>, MemoryError> {
            validate_namespace(namespace)?;
            let snapshot: Option<Arc<MemorySnapshot>> = self.snapshots.load(namespace)?;
            if let Some(snapshot) = &snapshot {
                snapshot.validate_namespace(namespace)?;
            }
            Ok(snapshot)
        }

        fn compare_exchange(
            &self,
            namespace: &str,
            expected_revision: u64,
            snapshot: Arc<MemorySnapshot>,
        ) -> Result<(), MemoryError> {
            validate_update(namespace, expected_revision, &snapshot)?;
            let actual = self.snapshots.compare_exchange(
                namespace,
                expected_revision,
                &*snapshot,
                |previous| {
                    previous
                        .validate_namespace(namespace)
                        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
                    Ok(previous.revision())
                },
            )?;
            check_revision(expected_revision, actual.unwrap_or(expected_revision))
        }
    }
}

#[cfg(feature = "memory-files")]
pub use files::FileMemoryStore;
