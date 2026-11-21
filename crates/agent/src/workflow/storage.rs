use std::sync::Arc;

use crate::snapshot_store::SnapshotMap;

use super::{WorkflowError, WorkflowSnapshot, check_revision, validate_run_id};

/// A missing record has revision zero. Stores must atomically check the
/// current revision and publish its successor, or return `WorkflowError::Stale`.
pub trait WorkflowStore: Send + Sync {
    fn load(&self, id: &str) -> Result<Option<Arc<WorkflowSnapshot>>, WorkflowError>;

    fn compare_exchange(
        &self,
        id: &str,
        expected: u64,
        next: Arc<WorkflowSnapshot>,
    ) -> Result<(), WorkflowError>;
}

#[derive(Default)]
pub struct InMemoryWorkflowStore {
    snapshots: SnapshotMap<WorkflowSnapshot>,
}

impl WorkflowStore for InMemoryWorkflowStore {
    fn load(&self, id: &str) -> Result<Option<Arc<WorkflowSnapshot>>, WorkflowError> {
        validate_run_id(id)?;
        Ok(self.snapshots.load(id)?)
    }

    fn compare_exchange(
        &self,
        id: &str,
        expected: u64,
        next: Arc<WorkflowSnapshot>,
    ) -> Result<(), WorkflowError> {
        validate_update(id, expected, &next)?;
        let actual =
            self.snapshots
                .compare_exchange(id, expected, next, WorkflowSnapshot::revision)?;
        check_revision(expected, actual.unwrap_or(expected))
    }
}

fn validate_update(id: &str, expected: u64, next: &WorkflowSnapshot) -> Result<(), WorkflowError> {
    next.validate(id)?;
    if expected.checked_add(1) != Some(next.revision()) {
        return Err(WorkflowError::Invalid(
            "replacement must be the next revision".into(),
        ));
    }
    Ok(())
}

#[cfg(feature = "workflow-files")]
mod files {
    use std::{io, path::Path};

    use crate::snapshot_store::FileSnapshots;

    use super::*;

    /// Owner-only JSON snapshots with a file lock per workflow run.
    pub struct FileWorkflowStore {
        snapshots: FileSnapshots,
    }

    impl FileWorkflowStore {
        pub fn open(directory: impl AsRef<Path>) -> Result<Self, WorkflowError> {
            Ok(Self {
                snapshots: FileSnapshots::open(directory.as_ref(), "workflow")?,
            })
        }
    }

    impl WorkflowStore for FileWorkflowStore {
        fn load(&self, id: &str) -> Result<Option<Arc<WorkflowSnapshot>>, WorkflowError> {
            validate_run_id(id)?;
            let snapshot: Option<Arc<WorkflowSnapshot>> = self.snapshots.load(id)?;
            if let Some(snapshot) = &snapshot {
                snapshot.validate(id)?;
            }
            Ok(snapshot)
        }

        fn compare_exchange(
            &self,
            id: &str,
            expected: u64,
            next: Arc<WorkflowSnapshot>,
        ) -> Result<(), WorkflowError> {
            validate_update(id, expected, &next)?;
            let actual = self
                .snapshots
                .compare_exchange(id, expected, &*next, |previous| {
                    previous
                        .validate(id)
                        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
                    Ok(previous.revision())
                })?;
            check_revision(expected, actual.unwrap_or(expected))
        }
    }
}

#[cfg(feature = "workflow-files")]
pub use files::FileWorkflowStore;
