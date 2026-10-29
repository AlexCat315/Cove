//! Translation of `System/Sources/CoveFileSystem/FileLock.swift`.
use crate::files::FileLock as NativeLock;
use std::fs;
use std::io;
use std::path::Path;

pub enum FileLock {}

impl FileLock {
    /// An operating-system lock shared by threads and processes, released when
    /// `body` returns or unwinds.
    pub fn with_lock<T>(
        file: &Path,
        body: impl FnOnce() -> io::Result<T>,
    ) -> io::Result<T> {
        if let Some(parent) = file.parent().filter(|path| !path.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        let _lock = NativeLock::acquire(file)?;
        body()
    }
}
