//! Translation of `Logging/Tests/CoveLoggingTests/StorageTests.swift`.
//!
//! Behaviour checklist:
//! - rotation keeps at most `files_per_session` readable JSONL files, each
//!   within `maximum_file_bytes`
//! - session retention keeps `retained_sessions` directories per application
//! - a storage failure is counted in status while console history survives
//! - the active session and directories not owned by the logger survive
//!   retention

use std::fs;
use std::path::{Path, PathBuf};

use cove_logging::{Level, LogEvent, LogQuery, LogRecord, LogRuntime, LoggingConfiguration};

fn temporary_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "cove-logging-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&root);
    root
}

fn configuration(root: &Path, configure: impl FnOnce(&mut LoggingConfiguration)) -> LoggingConfiguration {
    let mut configuration = LoggingConfiguration::default();
    configuration.console.enabled = false;
    configuration.storage.directory = Some(root.to_path_buf());
    configure(&mut configuration);
    configuration
}

fn jsonl_files(directory: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(directory)
        .expect("read session directory")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "jsonl"))
        .collect();
    files.sort();
    files
}

fn decode_all(data: &[u8]) -> Vec<LogRecord> {
    data.split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).expect("decode log record"))
        .collect()
}

#[test]
fn rotation_retention_and_flush_produce_readable_json_lines() {
    let root = temporary_root("rotation");
    let configuration = configuration(&root, |configuration| {
        configuration.storage.maximum_file_bytes = 65536;
        configuration.storage.files_per_session = 2;
        configuration.storage.retained_sessions = 2;
        configuration.buffering.pending_records = 65536;
        configuration.buffering.message_bytes = 1024;
        configuration.buffering.metadata_bytes = 128;
    });
    let message = "x".repeat(1000);

    for _ in 0..4 {
        let runtime = LogRuntime::new("test-run");
        runtime
            .configure(configuration.clone(), "app", None)
            .expect("configure");
        {
            let logger = runtime.logger("test");
            for _ in 0..200 {
                logger.log(&LogEvent::new(Level::Info, message.clone()));
            }
        }
        runtime.flush();

        let file = runtime.status().file.expect("storage is open");
        let files = jsonl_files(file.parent().expect("session directory"));
        assert_eq!(files.len(), 2, "rotation keeps exactly files_per_session files");
        for file in files {
            let data = fs::read(&file).expect("read rotated file");
            assert!(
                data.len() <= 65536,
                "rotated file {} is {} bytes, above the 65536 limit",
                file.display(),
                data.len()
            );
            for record in decode_all(&data) {
                assert_eq!(record.message.len(), 1000, "each line decodes intact");
            }
        }
    }

    let sessions = fs::read_dir(root.join("app")).expect("application directory");
    assert_eq!(
        sessions.filter_map(|entry| entry.ok()).count(),
        2,
        "retained_sessions bounds the stored session directories"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn file_failure_keeps_console_history_and_status_available() {
    let root = temporary_root("file-failure");
    let _ = fs::remove_dir_all(&root);
    fs::write(&root, b"not a directory").expect("create blocking file");
    let configuration = configuration(&root, |_| {});

    let runtime = LogRuntime::new("test-run");
    // Swift's `configure` reports a storage failure through `status.fileFailures`
    // rather than throwing, so the console keeps working either way.
    let _ = runtime.configure(configuration, "test", None);

    runtime
        .logger("test")
        .log(&LogEvent::new(Level::Error, "still available"));
    runtime.flush();

    assert_eq!(
        runtime.status().file_failures,
        1,
        "the storage failure is reported exactly once"
    );
    assert!(
        runtime
            .snapshot(&LogQuery::default())
            .records
            .iter()
            .any(|record| record.message == "still available"),
        "history remains readable after a storage failure"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn active_sessions_and_unowned_directories_survive_retention() {
    let root = temporary_root("retention");
    let configuration = configuration(&root, |configuration| {
        configuration.storage.retained_sessions = 1;
    });

    let first = LogRuntime::new("test-run");
    first
        .configure(configuration.clone(), "retention", None)
        .expect("configure first runtime");
    first
        .logger("test")
        .log(&LogEvent::new(Level::Error, "durable without explicit flush"));
    let active = first.status().file.expect("first session is open");

    let application_directory = active
        .parent()
        .and_then(Path::parent)
        .expect("application directory");
    let unowned = application_directory.join(format!(
        "session-{}",
        "00000000-0000-4000-8000-000000000000"
    ));
    fs::create_dir_all(&unowned).expect("create unowned directory");

    let second = LogRuntime::new("test-run");
    second
        .configure(configuration, "retention", None)
        .expect("configure second runtime");

    assert!(active.exists(), "the active session must not be pruned");
    assert!(unowned.exists(), "a directory without a lease is not logger-owned");

    let data = fs::read(&active).expect("read active session");
    let last = decode_all(&data).pop().expect("a durable record is on disk");
    assert_eq!(last.message, "durable without explicit flush");

    drop(first);
    drop(second);
    let _ = fs::remove_dir_all(&root);
}
