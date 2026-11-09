//! What a folder tree looked like, and which paths differ between two looks.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Entry {
    modified: Option<SystemTime>,
    len: u64,
    directory: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    entries: HashMap<PathBuf, Entry>,
}

impl Snapshot {
    /// Walks `root`. Hidden entries (names starting with `.`) are skipped
    /// when `ignore_hidden` is set, together with everything inside them.
    pub fn scan(root: &Path, ignore_hidden: bool) -> Self {
        let mut snapshot = Self::default();
        snapshot.walk(root, ignore_hidden, 0);
        snapshot
    }

    fn walk(&mut self, directory: &Path, ignore_hidden: bool, depth: usize) {
        // Symlink loops cannot recurse forever: links are not followed, and
        // the depth bound stops pathological trees.
        if depth > 64 {
            return;
        }
        let Ok(entries) = std::fs::read_dir(directory) else {
            return;
        };
        for entry in entries.filter_map(Result::ok) {
            if ignore_hidden && entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            let path = entry.path();
            let directory = metadata.is_dir();
            self.entries.insert(
                path.clone(),
                Entry {
                    modified: if directory { None } else { metadata.modified().ok() },
                    len: if directory { 0 } else { metadata.len() },
                    directory,
                },
            );
            if directory && !metadata.file_type().is_symlink() {
                self.walk(&path, ignore_hidden, depth + 1);
            }
        }
    }

    /// Paths created, removed or modified since `self`, sorted.
    pub fn changes(&self, next: &Self) -> Vec<PathBuf> {
        let mut changed: Vec<PathBuf> = next
            .entries
            .iter()
            .filter(|(path, entry)| self.entries.get(*path) != Some(entry))
            .map(|(path, _)| path.clone())
            .chain(
                self.entries
                    .keys()
                    .filter(|path| !next.entries.contains_key(*path))
                    .cloned(),
            )
            .collect();
        changed.sort();
        changed
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
