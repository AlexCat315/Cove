//! Translation of `Logging/Tests/CoveLoggingTests/StartupTests.swift`.
//!
//! Behaviour checklist:
//! - records written before `configure` carry the final process identity
//! - the startup history is replayed into the session file, so file contents
//!   equal the snapshot records
//! - reconfiguring with a new identity does not rewrite already-written records

use std::fs;
use std::path::{Path, PathBuf};

use cove_logging::{Level, LogEvent, LogQuery, LogRecord, LogRuntime, LoggingConfiguration};

/// A configuration that logs to `directory` with a silent console.
fn file_configuration(directory: &Path) -> LoggingConfiguration {
    let mut configuration = LoggingConfiguration::default();
    configuration.console.enabled = false;
    configuration.storage.directory = Some(directory.to_path_buf());
    configuration
}

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

fn read_records(path: &PathBuf) -> Vec<LogRecord> {
    let data = fs::read(path).expect("read log file");
    data.split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).expect("decode log record"))
        .collect()
}

#[test]
fn startup_records_receive_the_final_identity_before_file_replay() {
    let root = temporary_root("startup");
    let runtime = LogRuntime::new("provisional");
    runtime
        .logger("startup")
        .log(&LogEvent::new(Level::Info, "before bootstrap"));
    runtime.flush();

    let configuration = file_configuration(&root);
    runtime
        .configure(configuration.clone(), "real-app", Some("cli-run".into()))
        .expect("configure");
    runtime
        .logger("startup")
        .log(&LogEvent::new(Level::Error, "after bootstrap"));

    let snapshot = runtime.snapshot(&LogQuery::default());
    assert_eq!(
        snapshot.records.len(),
        2,
        "startup history joins the session"
    );
    assert!(
        snapshot
            .records
            .iter()
            .all(|record| record.process.name == "real-app" && record.process.run_id == "cli-run"),
        "every replayed record takes the identity installed by configure"
    );

    let file = runtime.status().file.expect("storage is open");
    let records = read_records(&file);
    assert_eq!(
        records, snapshot.records,
        "the file contains exactly the snapshot records"
    );

    runtime
        .configure(configuration, "another-app", Some("another-run".into()))
        .expect("reconfigure");
    assert_eq!(
        runtime
            .snapshot(&LogQuery::default())
            .records
            .first()
            .map(|record| record.process.run_id.as_str()),
        Some("cli-run"),
        "already written records keep the identity they were written with"
    );

    let _ = fs::remove_dir_all(&root);
}
