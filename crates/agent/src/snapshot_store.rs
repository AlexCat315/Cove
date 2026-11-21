//! Internal storage for run-scoped compare-and-swap snapshots.

use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex};

pub(crate) struct SnapshotMap<T> {
    entries: Mutex<HashMap<String, Arc<T>>>,
}

impl<T> Default for SnapshotMap<T> {
    fn default() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }
}

impl<T> SnapshotMap<T> {
    pub fn load(&self, key: &str) -> io::Result<Option<Arc<T>>> {
        let entries = self
            .entries
            .lock()
            .map_err(|_| io::Error::other("snapshot store lock poisoned"))?;
        Ok(entries.get(key).cloned())
    }

    pub fn compare_exchange(
        &self,
        key: &str,
        expected: u64,
        next: Arc<T>,
        revision: impl Fn(&T) -> u64,
    ) -> io::Result<Option<u64>> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| io::Error::other("snapshot store lock poisoned"))?;
        let actual = entries.get(key).map_or(0, |current| revision(current));
        if expected != actual {
            return Ok(Some(actual));
        }
        entries.insert(key.to_owned(), next);
        Ok(None)
    }
}

#[cfg(any(feature = "memory-files", feature = "workflow-files"))]
mod files {
    use std::fs::{self, File};
    use std::io::{self, BufReader};
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use cove_system::files::{Access, FileLock, atomic_write};
    use serde::{Serialize, de::DeserializeOwned};

    pub(crate) struct FileSnapshots {
        directory: PathBuf,
        prefix: &'static str,
    }

    impl FileSnapshots {
        pub fn open(directory: &Path, prefix: &'static str) -> io::Result<Self> {
            fs::create_dir_all(directory)?;
            Ok(Self {
                directory: directory.canonicalize()?,
                prefix,
            })
        }

        pub fn load<T: DeserializeOwned>(&self, key: &str) -> io::Result<Option<Arc<T>>> {
            let file = match File::open(self.path(key, "json")) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(error),
            };
            let record = serde_json::from_reader(BufReader::new(file))
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            Ok(Some(Arc::new(record)))
        }

        pub fn compare_exchange<T: Serialize + DeserializeOwned>(
            &self,
            key: &str,
            expected: u64,
            next: &T,
            read_revision: impl Fn(&T) -> io::Result<u64>,
        ) -> io::Result<Option<u64>> {
            let _lock = FileLock::acquire(&self.path(key, "lock"))?;
            let actual = self
                .load(key)?
                .map_or(Ok(0), |current| read_revision(&current))?;
            if expected != actual {
                return Ok(Some(actual));
            }
            let bytes = serde_json::to_vec(next)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            atomic_write(&self.path(key, "json"), &bytes, Access::OwnerOnly)?;
            Ok(None)
        }

        fn path(&self, key: &str, extension: &str) -> PathBuf {
            debug_assert!(crate::portable_storage_id(key));
            self.directory
                .join(format!("{}-{key}.{extension}", self.prefix))
        }
    }
}

#[cfg(any(feature = "memory-files", feature = "workflow-files"))]
pub(crate) use files::FileSnapshots;
