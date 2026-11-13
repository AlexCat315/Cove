//! `clear_logs`: the probe empties the runtime's retained log history.
//!
//! This runs against the process-wide `LogSystem`, so the file holds a single
//! test and configures the runtime itself. The clear must remove the records
//! and the loss count for that window, while the sequence counter keeps running
//! so the next record continues above the cleared ones.

use cove_core::scene::{Node, NodeTree};
use cove_devtools::protocol::control::ProbeCommand;
use cove_devtools::runtime_probe::RuntimeProbe;
use cove_logging::{Level, LogEvent, LogSystem, LoggingConfiguration};
use serde_json::json;
use std::cell::RefCell;
use std::rc::Rc;

fn probe() -> RuntimeProbe {
    let mut nodes = NodeTree::new();
    nodes.set_root(Some(Node::new()));
    RuntimeProbe::new(Rc::new(RefCell::new(nodes)), None, None, None)
}

fn configure() {
    let mut configuration = LoggingConfiguration::default();
    configuration.console.enabled = false;
    configuration.storage.enabled = false;
    configuration.filtering.minimum = Level::Trace;
    configuration.buffering.history_records = 8;
    LogSystem::bootstrap(Some("devtools-clear"), configuration, None).expect("bootstrap logging");
}

#[test]
fn clearing_logs_empties_the_replay_window_and_keeps_sequences_running() {
    configure();
    let mut probe = probe();
    let logger = LogSystem::logger("test.devtools");
    for index in 0..3 {
        logger.log(&LogEvent::new(Level::Info, format!("before {index}")));
    }
    LogSystem::flush();

    let before = probe
        .execute(ProbeCommand::Logs { after: None })
        .expect("read logs");
    let records = before["records"].as_array().expect("records");
    assert_eq!(records.len(), 3);
    let last = records
        .last()
        .and_then(|record| record["sequence"].as_u64())
        .expect("last sequence");

    assert_eq!(
        probe.execute(ProbeCommand::ClearLogs).unwrap(),
        json!({"cleared": 3, "dropped": 0, "firstAvailableSequence": null}),
        "the probe acknowledges the clear with the removed count"
    );

    let cleared = probe
        .execute(ProbeCommand::Logs { after: None })
        .expect("read cleared logs");
    assert_eq!(
        cleared["records"],
        json!([]),
        "no record survives the clear"
    );
    assert_eq!(cleared["dropped"], 0, "the cleared loss count is reset");
    assert!(cleared["firstAvailableSequence"].is_null());

    logger.log(&LogEvent::new(Level::Info, "after clear"));
    LogSystem::flush();
    let resumed = probe
        .execute(ProbeCommand::Logs { after: None })
        .expect("read resumed logs");
    let records = resumed["records"].as_array().expect("records");
    assert_eq!(records.len(), 1, "the record after the clear is retained");
    assert_eq!(records[0]["message"], "after clear");
    assert!(
        records[0]["sequence"].as_u64().unwrap() > last,
        "sequences continue above the cleared window"
    );
    assert_eq!(resumed["firstAvailableSequence"], records[0]["sequence"]);

    // A window with records still reports them and leaves an empty one behind.
    assert_eq!(
        probe.execute(ProbeCommand::ClearLogs).unwrap(),
        json!({"cleared": 1, "dropped": 0, "firstAvailableSequence": null})
    );
    let empty = probe
        .execute(ProbeCommand::Logs { after: None })
        .expect("read logs after a second clear");
    assert_eq!(empty["records"], json!([]));
}
