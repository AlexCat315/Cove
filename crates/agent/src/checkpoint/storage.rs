use std::collections::{HashMap, HashSet};
use std::io;
use std::sync::{Condvar, Mutex, MutexGuard};

use super::{
    AgentCheckpoint, CheckpointError, CheckpointLease, CheckpointStore, MAX_CHECKPOINT_BYTES,
    validate_id,
};

#[derive(Default)]
pub struct MemoryCheckpointStore {
    state: Mutex<MemoryState>,
    released: Condvar,
}

#[derive(Default)]
struct MemoryState {
    records: HashMap<String, AgentCheckpoint>,
    leased: HashSet<String>,
}

impl MemoryCheckpointStore {
    fn state(&self) -> Result<MutexGuard<'_, MemoryState>, CheckpointError> {
        self.state.lock().map_err(|_| storage_poisoned())
    }
}

fn storage_poisoned() -> CheckpointError {
    io::Error::other("checkpoint store lock poisoned").into()
}

struct MemoryLease<'a> {
    store: &'a MemoryCheckpointStore,
    id: String,
}

impl CheckpointStore for MemoryCheckpointStore {
    fn load(&self, id: &str) -> Result<Option<AgentCheckpoint>, CheckpointError> {
        validate_id(id)?;
        Ok(self.state()?.records.get(id).cloned())
    }

    fn lease(&self, id: &str) -> Result<Box<dyn CheckpointLease + '_>, CheckpointError> {
        validate_id(id)?;
        let mut state = self
            .released
            .wait_while(self.state()?, |state| state.leased.contains(id))
            .map_err(|_| storage_poisoned())?;
        state.leased.insert(id.to_owned());
        Ok(Box::new(MemoryLease {
            store: self,
            id: id.to_owned(),
        }))
    }
}

impl CheckpointLease for MemoryLease<'_> {
    fn load(&self) -> Result<Option<AgentCheckpoint>, CheckpointError> {
        self.store.load(&self.id)
    }

    fn save(&mut self, checkpoint: &AgentCheckpoint) -> Result<(), CheckpointError> {
        serialize(&self.id, checkpoint)?;
        self.store
            .state()?
            .records
            .insert(self.id.clone(), checkpoint.clone());
        Ok(())
    }
}

impl Drop for MemoryLease<'_> {
    fn drop(&mut self) {
        let mut state = self
            .store
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state.leased.remove(&self.id);
        self.store.released.notify_all();
    }
}

fn serialize(id: &str, checkpoint: &AgentCheckpoint) -> Result<Vec<u8>, CheckpointError> {
    if checkpoint.id() != id {
        return Err(CheckpointError::Invalid("lease belongs to a different run"));
    }
    let bytes = serde_json::to_vec(checkpoint)
        .map_err(|_| CheckpointError::Invalid("could not encode checkpoint"))?;
    if bytes.len() as u64 > MAX_CHECKPOINT_BYTES {
        return Err(CheckpointError::Invalid("checkpoint exceeds 8 MiB"));
    }
    Ok(bytes)
}

#[cfg(feature = "checkpoint-files")]
mod files {
    use std::fs::{self, File};
    use std::io::Read;
    use std::path::{Path, PathBuf};

    use cove_system::files::{Access, FileLock, atomic_write};

    use super::*;

    /// Local JSON snapshots with owner-only data files and an OS lock per
    /// run. Open a directory dedicated to checkpoints in the app's data area.
    pub struct FileCheckpointStore {
        directory: PathBuf,
    }

    impl FileCheckpointStore {
        pub fn open(directory: impl AsRef<Path>) -> Result<Self, CheckpointError> {
            fs::create_dir_all(directory.as_ref())?;
            Ok(Self {
                directory: directory.as_ref().canonicalize()?,
            })
        }

        fn path(&self, id: &str, extension: &str) -> PathBuf {
            self.directory.join(format!("run-{id}.{extension}"))
        }
    }

    struct FileLease {
        path: PathBuf,
        id: String,
        _lock: FileLock,
    }

    impl CheckpointStore for FileCheckpointStore {
        fn load(&self, id: &str) -> Result<Option<AgentCheckpoint>, CheckpointError> {
            validate_id(id)?;
            load_file(&self.path(id, "json"))
        }

        fn lease(&self, id: &str) -> Result<Box<dyn CheckpointLease + '_>, CheckpointError> {
            validate_id(id)?;
            let lock = FileLock::acquire(&self.path(id, "lock"))?;
            Ok(Box::new(FileLease {
                path: self.path(id, "json"),
                id: id.to_owned(),
                _lock: lock,
            }))
        }
    }

    impl CheckpointLease for FileLease {
        fn load(&self) -> Result<Option<AgentCheckpoint>, CheckpointError> {
            load_file(&self.path)
        }

        fn save(&mut self, checkpoint: &AgentCheckpoint) -> Result<(), CheckpointError> {
            let bytes = serialize(&self.id, checkpoint)?;
            atomic_write(&self.path, &bytes, Access::OwnerOnly)?;
            Ok(())
        }
    }

    fn load_file(path: &Path) -> Result<Option<AgentCheckpoint>, CheckpointError> {
        let file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let mut bytes = Vec::new();
        file.take(MAX_CHECKPOINT_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_CHECKPOINT_BYTES {
            return Err(CheckpointError::Invalid("checkpoint exceeds 8 MiB"));
        }
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| CheckpointError::Invalid("could not decode checkpoint"))
    }
}

#[cfg(feature = "checkpoint-files")]
pub use files::FileCheckpointStore;
