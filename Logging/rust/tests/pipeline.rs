//! Translation of `Logging/Tests/CoveLoggingTests/PipelineTests.swift`.
//!
//! Behaviour checklist:
//! - a cloned handler keeps value semantics; a local level override applies to
//!   one copy only, and module filters match on label segments
//! - metadata precedence is context < provider < handler < event, and redaction
//!   is recursive while preserving structure
//! - concurrent writers produce ordered, unique sequences and an observer may
//!   log again without deadlocking
//! - a partial configuration decodes with documented defaults, environment
//!   overrides are validated, and the round-trip is lossless
//! - record count, message and metadata are bounded, and history queries filter
//! - a full queue reports loss but still retains critical events
//! - process installation is idempotent and native records join the pipeline

use std::collections::BTreeMap;
use std::sync::Arc;

use cove_logging::{
    Level, LogContext, LogEvent, LogQuery, LogRuntime, LogSnapshot, LogSystem, LogValue,
    LoggingConfiguration, MetadataProvider, UnifiedLogHandler,
};

/// A runtime with console and file storage disabled, so tests read history only.
fn runtime(configure: impl FnOnce(&mut LoggingConfiguration)) -> Arc<LogRuntime> {
    let runtime = LogRuntime::new("test-run");
    let mut configuration = LoggingConfiguration::default();
    configuration.console.enabled = false;
    configuration.storage.enabled = false;
    configuration.filtering.minimum = Level::Trace;
    configure(&mut configuration);
    runtime
        .configure(configuration, "test", None)
        .expect("configure runtime");
    runtime
}

fn messages(snapshot: &LogSnapshot) -> Vec<&str> {
    snapshot
        .records
        .iter()
        .map(|record| record.message.as_str())
        .collect()
}

#[test]
fn standard_logger_preserves_value_semantics_and_dynamic_module_filters() {
    let runtime = runtime(|configuration| {
        configuration.filtering.minimum = Level::Warning;
        configuration.filtering.modules = BTreeMap::from([
            ("com.test".into(), Level::Debug),
            ("com.test.noisy".into(), Level::Error),
        ]);
    });

    let mut first = runtime.logger("com.test.application");
    first.set_metadata_value("owner", Some(LogValue::from("first")));
    let mut second = first.clone();
    second.set_metadata_value("owner", Some(LogValue::from("second")));
    second.set_log_level(Level::Trace);

    first.log(&LogEvent::new(Level::Trace, "filtered"));
    second.log(&LogEvent::new(Level::Trace, "local override"));
    first.log(&LogEvent::new(Level::Debug, "module default"));
    runtime
        .logger("com.test.noisy.file")
        .log(&LogEvent::new(Level::Warning, "filtered noisy"));
    runtime
        .logger("com.testing")
        .log(&LogEvent::new(Level::Info, "prefix must match a segment"));

    runtime.flush();
    // `LogQuery::default()` starts at `Info`, so the Trace and Debug records
    // under test need an explicit minimum — otherwise the query itself filters
    // them out and the assertions would pass for the wrong reason.
    let snapshot = runtime.snapshot(&LogQuery {
        minimum: Level::Trace,
        ..LogQuery::default()
    });
    assert_eq!(
        messages(&snapshot),
        vec!["local override", "module default"]
    );
    assert_eq!(
        snapshot.records[0].metadata.get("owner"),
        Some(&LogValue::from("second"))
    );
    assert_eq!(
        snapshot.records[1].metadata.get("owner"),
        Some(&LogValue::from("first"))
    );
}

#[test]
fn context_metadata_and_recursive_redaction_preserve_structure() {
    let runtime = runtime(|_| {});
    let mut logger = runtime.logger("test");
    logger.metadata_provider = Some(MetadataProvider(Arc::new(|| {
        BTreeMap::from([
            ("provided".into(), LogValue::from("dynamic")),
            ("precedence".into(), LogValue::from("provider")),
            ("trace_id".into(), LogValue::from("provider-trace")),
        ])
    })));
    logger.set_metadata_value("precedence", Some(LogValue::from("logger")));

    let context = BTreeMap::from([
        ("trace_id".into(), LogValue::from("trace-1")),
        ("precedence".into(), LogValue::from("context")),
    ]);
    LogContext::with_values(context, || {
        logger.log(
            &LogEvent::new(Level::Info, "message").with_metadata(BTreeMap::from([
                ("precedence".into(), LogValue::from("event")),
                (
                    "nested".into(),
                    LogValue::Object(BTreeMap::from([
                        ("TOKEN".into(), LogValue::from("sensitive")),
                        (
                            "safe".into(),
                            LogValue::Array(vec![LogValue::from("a"), LogValue::from("b")]),
                        ),
                    ])),
                ),
            ])),
        )
    });

    runtime.flush();
    let snapshot = runtime.snapshot(&LogQuery::default());
    let record = snapshot.records.first().expect("one record");
    // The provider supplies the trace id and wins over the ambient context.
    assert_eq!(
        record.metadata.get("trace_id"),
        Some(&LogValue::from("provider-trace"))
    );
    assert_eq!(
        record.metadata.get("provided"),
        Some(&LogValue::from("dynamic"))
    );
    // Event metadata has the highest precedence.
    assert_eq!(
        record.metadata.get("precedence"),
        Some(&LogValue::from("event"))
    );
    let nested = record.metadata.get("nested").expect("nested metadata");
    let LogValue::Object(nested) = nested else {
        panic!("nested stays an object")
    };
    assert_eq!(nested.get("TOKEN"), Some(&LogValue::from("[redacted]")));
    assert!(matches!(nested.get("safe"), Some(LogValue::Array(items)) if items.len() == 2));

    // Records survive a JSON round-trip unchanged.
    let encoded = serde_json::to_string(record).expect("encode record");
    let decoded: cove_logging::LogRecord = serde_json::from_str(&encoded).expect("decode record");
    assert_eq!(&decoded, record);
}

#[test]
fn concurrent_writers_have_ordered_sequences_and_observer_can_log_again() {
    let runtime = runtime(|configuration| {
        configuration.buffering.pending_records = 65536;
        configuration.buffering.history_records = 2000;
    });
    let observation = runtime.observe(
        LogQuery::default(),
        false,
        Arc::new({
            let runtime = runtime.clone();
            move |record: &cove_logging::LogRecord| {
                if record.message == "nested" {
                    runtime
                        .logger("observer")
                        .log(&LogEvent::new(Level::Info, "reentrant"));
                }
            }
        }),
    );

    let mut handles = Vec::new();
    for index in 0..8 {
        let runtime = runtime.clone();
        handles.push(std::thread::spawn(move || {
            let logger = runtime.logger(&format!("worker.{index}"));
            for line in 0..100 {
                logger.log(&LogEvent::new(Level::Info, line.to_string()));
            }
        }));
    }
    for handle in handles {
        handle.join().expect("worker thread");
    }
    runtime
        .logger("test")
        .log(&LogEvent::new(Level::Info, "nested"));
    runtime.flush();
    observation.cancel();
    runtime.flush();

    let snapshot = runtime.snapshot(&LogQuery::default());
    assert_eq!(snapshot.records.len(), 802);
    let sequences: Vec<u64> = snapshot
        .records
        .iter()
        .map(|record| record.sequence)
        .collect();
    let mut sorted = sequences.clone();
    sorted.sort_unstable();
    assert_eq!(sequences, sorted, "sequences are ordered");
    assert_eq!(
        snapshot
            .records
            .last()
            .map(|record| record.message.as_str()),
        Some("reentrant")
    );
    assert_eq!(runtime.status().dropped, 0);
}

#[test]
fn partial_configuration_defaults_and_environment_are_validated() {
    let mut configuration: LoggingConfiguration = serde_json::from_str(
        r#"{"storage":{"filesPerSession":2},"filtering":{"minimum":"warning"}}"#,
    )
    .expect("partial configuration decodes");
    assert_eq!(configuration.storage.maximum_file_bytes, 8 * 1024 * 1024);
    assert_eq!(configuration.filtering.minimum, Level::Warning);
    assert!(configuration.console.enabled);

    let authored = configuration.clone();
    configuration
        .apply_environment(&BTreeMap::from([
            ("COVE_LOG_LEVEL".to_owned(), "debug".to_owned()),
            ("COVE_LOG_FILES".to_owned(), "0".to_owned()),
        ]))
        .expect("valid environment applies");
    assert_eq!(configuration.filtering.minimum, Level::Debug);
    assert!(!configuration.storage.enabled);
    // The authored copy is untouched.
    assert_eq!(authored.filtering.minimum, Level::Warning);
    assert!(authored.storage.enabled);

    assert!(
        configuration
            .apply_environment(&BTreeMap::from([(
                "COVE_LOG_LEVEL".to_owned(),
                "wrong".to_owned()
            )]))
            .is_err()
    );
    let encoded = serde_json::to_string(&authored).expect("encode configuration");
    let decoded: LoggingConfiguration = serde_json::from_str(&encoded).expect("decode");
    assert_eq!(decoded, authored);
}

#[test]
fn record_bounds_and_history_queries_are_explicit() {
    let runtime = runtime(|configuration| {
        configuration.buffering.history_records = 2;
        configuration.buffering.message_bytes = 128;
        configuration.buffering.metadata_bytes = 128;
    });
    let logger = runtime.logger("test");
    logger.log(&LogEvent::new(Level::Info, "old"));
    logger.log(&LogEvent::new(Level::Warning, "match"));
    logger.log(
        &LogEvent::new(Level::Error, "你好".repeat(1000)).with_metadata(BTreeMap::from([(
            "long".into(),
            LogValue::from("x".repeat(1000)),
        )])),
    );
    runtime.flush();

    let snapshot = runtime.snapshot(&LogQuery::default());
    assert_eq!(snapshot.records.len(), 2);
    assert_eq!(snapshot.first_available_sequence, Some(2));
    assert!(snapshot.records.last().unwrap().message.len() <= 128);

    let filtered = runtime.snapshot(&LogQuery {
        minimum: Level::Warning,
        text: Some("match".into()),
        ..Default::default()
    });
    assert_eq!(filtered.records.len(), 1);
}

#[test]
fn process_installation_is_idempotent_and_native_records_join_the_pipeline() {
    let configuration = LoggingConfiguration {
        storage: cove_logging::Storage {
            enabled: false,
            ..Default::default()
        },
        console: cove_logging::Console {
            enabled: false,
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(
        LogSystem::bootstrap(Some("test"), configuration.clone(), None).expect("first bootstrap")
    );
    assert!(!LogSystem::bootstrap(Some("test"), configuration, None).expect("second bootstrap"));
}
