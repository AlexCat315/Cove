//! Translation of `System/Tests/CoveFileSystemTests/FileLockTests.swift`.
//!
//! Behaviour checklist: independent handles serialize a read-modify-write
//! section across threads, so twenty concurrent increments must all land.

use std::fs;
use std::sync::{Arc, Barrier};
use std::thread;

use cove_system::files::{Access, FileLock, atomic_write};

#[test]
fn independent_file_handles_serialize_shared_writes() {
    let root = tempfile::Builder::new()
        .prefix("cove-lock-")
        .tempdir()
        .expect("temporary directory");
    let value = root.path().join("counter");
    atomic_write(&value, b"0", Access::OwnerOnly).expect("write counter");
    let lock_path = root.path().join("shared.lock");

    let iterations = 20;
    let barrier = Arc::new(Barrier::new(iterations));
    let mut handles = Vec::new();
    for _ in 0..iterations {
        let value = value.clone();
        let lock_path = lock_path.clone();
        let barrier = barrier.clone();
        handles.push(thread::spawn(move || {
            barrier.wait();
            let _guard = FileLock::acquire(&lock_path).expect("acquire lock");
            let count: i64 = fs::read_to_string(&value)
                .expect("read counter")
                .trim()
                .parse()
                .expect("counter is a number");
            std::thread::sleep(std::time::Duration::from_millis(1));
            atomic_write(
                &value,
                (count + 1).to_string().as_bytes(),
                Access::OwnerOnly,
            )
            .expect("write counter");
        }));
    }
    for handle in handles {
        handle.join().expect("worker thread");
    }

    assert_eq!(fs::read_to_string(&value).unwrap().trim(), "20");
}

#[test]
fn dropping_the_guard_releases_the_lock() {
    let root = tempfile::Builder::new()
        .prefix("cove-lock-")
        .tempdir()
        .expect("temporary directory");
    let lock_path = root.path().join("shared.lock");

    {
        let _guard = FileLock::acquire(&lock_path).expect("first acquire");
    }
    // A second acquisition after the guard drops must succeed.
    FileLock::acquire(&lock_path).expect("second acquire");
}
