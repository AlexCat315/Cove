//! Translation of `System/Tests/CoveFileSystemTests/Files.swift`.
//!
//! Behaviour checklist:
//! - replacement publishes the new bytes and removes the staging file
//! - replacing from a missing staging file or onto itself fails and preserves
//!   the destination
//! - a failed atomic write cleans every staging file it created
//! - owner-only files stay readable and writable, and the restriction is applied
//!   to files and directories
//! - on Windows a sharing violation preserves the old file and cleans staging

use std::fs;

use cove_system::files::{Access, atomic_write, replace_file, restrict_path};

fn temporary_directory() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("cove-system-")
        .tempdir()
        .expect("temporary directory")
}

#[test]
fn replacement_publishes_and_preserves_destination_on_failure() {
    let root = temporary_directory();
    let destination = root.path().join("file");
    let staging = root.path().join("staging");
    atomic_write(&destination, b"old", Access::OwnerOnly).expect("write old");
    fs::write(&staging, b"new").expect("write staging");

    replace_file(&staging, &destination).expect("replace");
    assert_eq!(fs::read_to_string(&destination).unwrap(), "new");
    assert!(!staging.exists(), "staging file must be consumed");

    // The staging file is gone, so a second replacement must fail.
    assert!(replace_file(&staging, &destination).is_err());
    assert_eq!(fs::read_to_string(&destination).unwrap(), "new");

    // Replacing a file with itself is rejected rather than truncating it.
    assert!(replace_file(&destination, &destination).is_err());
    assert_eq!(fs::read_to_string(&destination).unwrap(), "new");
}

#[test]
fn failed_atomic_write_cleans_staging_files() {
    let root = temporary_directory();
    let destination = root.path().join("directory");
    fs::create_dir(&destination).expect("create directory");
    fs::write(destination.join("child"), b"keep").expect("write child");

    assert!(atomic_write(&destination, b"new", Access::Inherit).is_err());
    assert_eq!(fs::read_to_string(destination.join("child")).unwrap(), "keep");

    let entries: Vec<_> = fs::read_dir(root.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(entries, vec![std::ffi::OsString::from("directory")]);
}

#[test]
fn owner_only_files_remain_readable_and_writable() {
    let root = temporary_directory();
    restrict_path(root.path(), true).expect("restrict directory");
    let file = root.path().join("secret");
    atomic_write(&file, b"secret", Access::OwnerOnly).expect("write secret");
    restrict_path(&file, false).expect("restrict file");
    assert_eq!(fs::read_to_string(&file).unwrap(), "secret");

    atomic_write(&file, b"updated", Access::OwnerOnly).expect("rewrite secret");
    assert_eq!(fs::read_to_string(&file).unwrap(), "updated");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(
            fs::metadata(root.path()).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
}

#[cfg(windows)]
#[test]
fn sharing_violation_preserves_old_file_and_cleans_staging() {
    use std::os::windows::fs::OpenOptionsExt;

    let root = temporary_directory();
    let file = root.path().join("locked");
    atomic_write(&file, b"old", Access::OwnerOnly).expect("write old");

    // Share read/write but not delete, which blocks the replace.
    let handle = fs::OpenOptions::new()
        .read(true)
        .share_mode(0x0000_0001 | 0x0000_0002)
        .open(&file)
        .expect("open locked file");

    assert!(atomic_write(&file, b"new", Access::OwnerOnly).is_err());
    assert_eq!(fs::read_to_string(&file).unwrap(), "old");

    let entries: Vec<_> = fs::read_dir(root.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(entries, vec![std::ffi::OsString::from("locked")]);
    drop(handle);
}
