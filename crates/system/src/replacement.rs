//! Translation of `System/Sources/CoveFileSystem/Replacement.swift`.
use crate::files::replace_file;
use std::io;
use std::path::{Path, PathBuf};

pub enum FileReplacement {}

impl FileReplacement {
    /// Publishes a staged file on the same filesystem. Failure never deletes the
    /// destination. The native bridge performs `rename` on POSIX and
    /// `MoveFileExW(REPLACE_EXISTING | WRITE_THROUGH)` with bounded retries on
    /// Windows.
    pub fn replace(destination: &Path, staging: &Path) -> io::Result<()> {
        if standardized(staging) == standardized(destination) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "staging and destination identify the same file",
            ));
        }
        replace_file(staging, destination)
    }
}

/// Foundation's `standardizedFileURL` is lexical: make the path absolute and
/// collapse `.`/`..` without touching the filesystem.
fn standardized(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()
            .map(|directory| directory.join(path))
            .unwrap_or_else(|_| path.to_owned())
    };
    let mut result = PathBuf::new();
    for component in absolute.components() {
        use std::path::Component;
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            other => result.push(other.as_os_str()),
        }
    }
    result
}
