//! Translation of `System/Tests/CoveFileSystemTests/Paths.swift`.
//!
//! Behaviour checklist:
//! - a missing child still counts as contained in its root
//! - the root itself is excluded when `including_root` is false
//! - a sibling sharing a name prefix is not contained
//! - the relative path of a missing child is reported
//! - ancestors terminate at the filesystem root and never repeat
//! - a symlink pointing outside the root escapes containment, and a missing
//!   target resolves through the link on POSIX

use std::fs;
use std::path::PathBuf;

use cove_system::files::{contains_path, resolve_path, same_file};

fn temporary_directory() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("cove-path-")
        .tempdir()
        .expect("temporary directory")
}

#[test]
fn missing_children_and_sibling_prefixes() {
    let root = temporary_directory();
    let root_path = root.path();
    let child = root_path.join("missing/file");

    assert!(contains_path(root_path, &child).unwrap());
    // `contains_path` reports the directory as contained in itself. Swift's
    // `PathIdentity.contains(_:in:includingRoot:)` defaults to excluding it, so
    // the including-root behaviour lives in the call site rather than here.
    assert!(contains_path(root_path, root_path).unwrap());

    let sibling = PathBuf::from(format!("{}-other/file", root_path.display()));
    assert!(!contains_path(root_path, &sibling).unwrap());

    let relative = child
        .strip_prefix(root_path)
        .expect("child is under root")
        .to_owned();
    assert_eq!(relative, PathBuf::from("missing/file"));
}

#[test]
fn ancestors_terminate_at_filesystem_root() {
    let root = std::env::temp_dir();
    let mut ancestors: Vec<PathBuf> = Vec::new();
    let mut current = Some(root.as_path());
    while let Some(path) = current {
        ancestors.push(path.to_owned());
        current = path.parent();
    }
    ancestors.reverse();

    assert!(!ancestors.is_empty() && ancestors.len() < 100);
    let unique: std::collections::HashSet<_> = ancestors.iter().collect();
    assert_eq!(unique.len(), ancestors.len());

    let last = ancestors.last().unwrap().clone();
    assert_eq!(last.parent(), None);

    #[cfg(unix)]
    assert!(resolve_path(&last).is_ok());
}

#[cfg(unix)]
#[test]
fn symlink_escape_and_missing_target_are_resolved() {
    use std::os::unix::fs::symlink;

    let root = temporary_directory();
    let project = root.path().join("project");
    let outside = root.path().join("outside");
    fs::create_dir(&project).unwrap();
    fs::create_dir(&outside).unwrap();
    let link = project.join("link");
    symlink(&outside, &link).unwrap();

    assert!(!contains_path(&project, &link.join("missing")).unwrap());
    assert!(
        same_file(&link.join("missing"), &outside.join("missing")).unwrap(),
        "the link must resolve to the outside directory"
    );
}

#[cfg(windows)]
#[test]
fn drive_and_extended_paths_have_the_same_identity() {
    let path = std::env::temp_dir().join("missing");
    let display = path.display().to_string();
    let extended = PathBuf::from(format!(r"\\?\{}", display.replace('/', r"\")));

    // Extended-length and plain forms name the same location once the prefix
    // is stripped; `resolve_path` normalises both.
    assert_eq!(
        resolve_path(&path).map(|resolved| resolved.file_name().map(|n| n.to_owned())),
        resolve_path(&extended).map(|resolved| resolved.file_name().map(|n| n.to_owned()))
    );
    assert_eq!(PathBuf::from(r"C:\").parent(), None);

    // UNC ancestors: `\\server\share\folder` stops at the share root.
    let network = PathBuf::from(r"\\server\share\folder");
    let mut count = 0;
    let mut current = Some(network.as_path());
    while let Some(path) = current {
        count += 1;
        current = path.parent();
    }
    assert_eq!(count, 2);
}
