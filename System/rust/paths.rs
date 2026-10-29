//! Translation of `System/Sources/CoveFileSystem/Paths.swift`.
//!
//! Comparisons resolve the longest existing ancestor, including paths that do
//! not exist yet, then fall back to lexical components.
use crate::files::same_file;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub enum PathIdentity {}

impl PathIdentity {
    pub fn canonical_url(path: &Path) -> PathBuf {
        Self::resolved_url(path).unwrap_or_else(|| Self::normalized_url(path))
    }

    pub fn same_location(lhs: &Path, rhs: &Path) -> bool {
        let (Some(lhs), Some(rhs)) = (Self::resolved_url(lhs), Self::resolved_url(rhs)) else {
            return false;
        };
        let left_exists = lhs.exists();
        let right_exists = rhs.exists();
        if left_exists != right_exists {
            return false;
        }
        if left_exists {
            return same_file(&lhs, &rhs).unwrap_or(false);
        }
        Self::comparable_components(&lhs) == Self::comparable_components(&rhs)
    }

    pub fn contains(candidate: &Path, root: &Path, including_root: bool) -> bool {
        let (Some(candidate), Some(root)) = (Self::resolved_url(candidate), Self::resolved_url(root))
        else {
            return false;
        };
        if root.exists() {
            let directories = Self::ancestors(&candidate);
            return directories
                .iter()
                .skip(if including_root { 0 } else { 1 })
                .any(|directory| Self::same_location(directory, &root));
        }
        let parts = Self::comparable_components(&candidate);
        let base = Self::comparable_components(&root);
        parts.len() >= base.len()
            && (including_root || parts.len() > base.len())
            && parts[..base.len()] == base[..]
    }

    /// Includes the starting directory and terminates at POSIX, drive and UNC
    /// roots.
    pub fn ancestors(directory: &Path) -> Vec<PathBuf> {
        let mut current = Self::normalized_url(directory);
        let mut visited = HashSet::new();
        let mut result = Vec::new();
        loop {
            if !visited.insert(current.clone()) {
                break;
            }
            result.push(current.clone());
            #[cfg(windows)]
            if is_windows_root(&current) {
                break;
            }
            let Some(parent) = current.parent() else {
                break;
            };
            let parent = Self::normalized_url(parent);
            if parent.as_os_str().is_empty() || parent == current {
                break;
            }
            current = parent;
        }
        result
    }

    pub fn relative_path(file: &Path, root: &Path) -> String {
        let parts = Self::components(file);
        let base = Self::components(root);
        let comparable = Self::comparable_components(file);
        let comparable_base = Self::comparable_components(root);
        let mut common = 0;
        while common < parts.len().min(base.len())
            && comparable.get(common) == comparable_base.get(common)
        {
            common += 1;
        }
        let mut values = vec!["..".to_owned(); base.len().saturating_sub(common)];
        values.extend(parts.into_iter().skip(common));
        values.join("/")
    }

    /// Resolves the longest existing ancestor, following dangling symbolic
    /// links up to 40 hops, then reattaches the missing suffix.
    fn resolved_url(path: &Path) -> Option<PathBuf> {
        let mut ancestor = Self::normalized_url(path);
        let mut suffix: Vec<std::ffi::OsString> = Vec::new();
        let mut visited = HashSet::new();
        while !ancestor.exists() {
            if let Ok(destination) = fs::read_link(&ancestor) {
                if visited.len() >= 40 || !visited.insert(ancestor.clone()) {
                    return None;
                }
                let parent = ancestor
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from("/"));
                ancestor = Self::normalized_url(&parent.join(destination));
                continue;
            }
            #[cfg(windows)]
            if is_windows_root(&ancestor) {
                break;
            }
            let Some(name) = ancestor.file_name().map(std::ffi::OsStr::to_os_string) else {
                break;
            };
            let Some(parent) = ancestor.parent().map(Path::to_path_buf) else {
                break;
            };
            if parent == ancestor {
                break;
            }
            suffix.push(name);
            ancestor = parent;
        }
        let mut result = if ancestor.exists() {
            fs::canonicalize(&ancestor).unwrap_or(ancestor)
        } else {
            ancestor
        };
        for component in suffix.iter().rev() {
            result.push(component);
        }
        Some(Self::normalized_url(&result))
    }

    fn normalized_url(path: &Path) -> PathBuf {
        #[cfg(windows)]
        {
            PathBuf::from(normalize_windows(path))
        }
        #[cfg(not(windows))]
        {
            lexical(path)
        }
    }

    pub fn components(path: &Path) -> Vec<String> {
        #[cfg(windows)]
        {
            let value = Self::canonical_url(path);
            value
                .to_string_lossy()
                .replace('\\', "/")
                .split('/')
                .filter(|part| !part.is_empty())
                .map(str::to_owned)
                .collect()
        }
        #[cfg(not(windows))]
        {
            let value = Self::canonical_url(path);
            value
                .components()
                .filter_map(|component| match component {
                    std::path::Component::Normal(part) => {
                        Some(part.to_string_lossy().into_owned())
                    }
                    std::path::Component::Prefix(prefix) => {
                        Some(prefix.as_os_str().to_string_lossy().into_owned())
                    }
                    _ => None,
                })
                .collect()
        }
    }

    pub fn comparable_components(path: &Path) -> Vec<String> {
        #[cfg(windows)]
        {
            Self::components(path)
                .into_iter()
                .map(|part| part.to_lowercase())
                .collect()
        }
        #[cfg(not(windows))]
        {
            Self::components(path)
        }
    }
}

/// `standardizedFileURL` without touching the filesystem.
fn lexical(path: &Path) -> PathBuf {
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

#[cfg(windows)]
fn normalize_windows(path: &Path) -> String {
    let mut value = path.to_string_lossy().replace('\\', "/");
    if let Some(rest) = value
        .strip_prefix("//?/")
        .or_else(|| value.strip_prefix("/?/"))
    {
        value = rest.to_owned();
        if value.to_lowercase().starts_with("unc/") {
            value = format!("//{}", &value[4..]);
        }
    }
    let unc = value.starts_with("//");
    let root_count = if unc { 2 } else { 1 };
    let mut parts: Vec<&str> = Vec::new();
    for part in value.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.len() > root_count {
                    parts.pop();
                }
            }
            part => parts.push(part),
        }
    }
    let mut normalized = if unc {
        format!("//{}", parts.join("/"))
    } else {
        parts.join("/")
    };
    if is_windows_root(Path::new(&normalized)) {
        if !normalized.ends_with('/') {
            normalized.push('/');
        }
    }
    normalized
}

#[cfg(windows)]
fn is_windows_root(path: &Path) -> bool {
    let value = path.to_string_lossy().replace('\\', "/");
    let parts: Vec<&str> = value.split('/').filter(|part| !part.is_empty()).collect();
    if value.starts_with("//") {
        return parts.len() <= 2;
    }
    parts.len() == 1 && parts[0].len() == 2 && parts[0].ends_with(':')
}
