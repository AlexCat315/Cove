use cove_watch::{DirectoryWatcher, Snapshot, WatchBackend, WatchOptions};
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::time::Duration;

fn options(native: bool) -> WatchOptions {
    WatchOptions {
        poll_interval: Duration::from_millis(100),
        // Long enough that a native test only passes through notifications.
        safety_interval: Duration::from_secs(60),
        debounce: Duration::from_millis(30),
        ignore_hidden: true,
        native,
    }
}

fn expect_change(native: bool) {
    let root = tempfile::tempdir().expect("temp root");
    std::fs::create_dir(root.path().join("folder")).expect("folder");
    let (sender, changes) = channel::<Vec<PathBuf>>();
    let watcher = DirectoryWatcher::new(root.path(), options(native), move |paths| {
        let _ = sender.send(paths);
    })
    .expect("watcher");
    let expected = if native
        && cfg!(any(
            target_os = "macos",
            target_os = "linux",
            target_os = "android",
            windows
        )) {
        WatchBackend::Native
    } else {
        WatchBackend::Polling
    };
    assert_eq!(watcher.backend(), expected);
    std::thread::sleep(Duration::from_millis(150));
    let file = root.path().join("folder").join("note.md");
    std::fs::write(&file, "hello").expect("write");
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        let paths = changes
            .recv_timeout(remaining)
            .expect("a change before the deadline");
        if paths.iter().any(|path| path.ends_with("folder/note.md")) {
            break;
        }
    }
}

#[test]
fn native_notifications_report_a_new_file() {
    expect_change(true);
}

#[test]
fn polling_reports_a_new_file() {
    expect_change(false);
}

#[test]
fn hidden_entries_are_ignored() {
    let root = tempfile::tempdir().expect("temp root");
    let before = Snapshot::scan(root.path(), true);
    std::fs::create_dir(root.path().join(".cache")).expect("hidden");
    std::fs::write(root.path().join(".cache/state.json"), "{}").expect("write");
    let after = Snapshot::scan(root.path(), true);
    assert!(before.changes(&after).is_empty());
    std::fs::write(root.path().join("visible.md"), "x").expect("write");
    let last = Snapshot::scan(root.path(), true);
    assert_eq!(after.changes(&last), vec![root.path().join("visible.md")]);
}

#[test]
fn removals_and_edits_are_reported() {
    let root = tempfile::tempdir().expect("temp root");
    let file = root.path().join("a.md");
    std::fs::write(&file, "one").expect("write");
    let before = Snapshot::scan(root.path(), true);
    std::fs::write(&file, "longer text").expect("edit");
    let edited = Snapshot::scan(root.path(), true);
    assert_eq!(before.changes(&edited), vec![file.clone()]);
    std::fs::remove_file(&file).expect("remove");
    let removed = Snapshot::scan(root.path(), true);
    assert_eq!(edited.changes(&removed), vec![file]);
}

#[test]
fn a_missing_folder_is_an_error() {
    let root = tempfile::tempdir().expect("temp root");
    assert!(DirectoryWatcher::new(root.path().join("missing"), options(true), |_| {}).is_err());
}

#[test]
fn dropping_stops_promptly() {
    let root = tempfile::tempdir().expect("temp root");
    let watcher =
        DirectoryWatcher::new(root.path(), WatchOptions::default(), |_| {}).expect("watcher");
    let started = std::time::Instant::now();
    drop(watcher);
    assert!(started.elapsed() < Duration::from_secs(1));
}
