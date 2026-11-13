//! Clearing the replay window.
//!
//! The clear is a real reset of the runtime's retained records, not a
//! client-side filter: `LogHistory` loses its records but keeps its capacity,
//! `LogRuntime::clear_history` also resets the loss count for the cleared
//! window, and the sequence counter keeps running so later records never reuse
//! a cleared sequence.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier};

use cove_logging::history::LogHistory;
use cove_logging::{
    Level, Location, LogEvent, LogQuery, LogRecord, LogRuntime, LoggingConfiguration, Process,
};

fn record(sequence: u64) -> LogRecord {
    LogRecord {
        sequence,
        timestamp: sequence as f64,
        level: Level::Info,
        label: "test".into(),
        message: format!("record {sequence}"),
        metadata: Default::default(),
        location: Location::default(),
        process: Process::default(),
    }
}

fn sequences(history: &LogHistory) -> Vec<u64> {
    history
        .records()
        .iter()
        .map(|record| record.sequence)
        .collect()
}

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

#[test]
fn clearing_history_empties_records_and_keeps_the_window() {
    let mut history = LogHistory::new(3);
    for sequence in 1..=3 {
        history.append(record(sequence));
    }
    assert_eq!(sequences(&history), vec![1, 2, 3]);

    assert_eq!(history.clear(), 3, "the clear reports what it removed");
    assert!(history.records().is_empty(), "no record survives the clear");
    assert_eq!(
        history.dropped_capacity(),
        3,
        "the storage capacity is unchanged"
    );

    // The window restarts at the front and wraps exactly as before.
    for sequence in 4..=6 {
        history.append(record(sequence));
    }
    assert_eq!(sequences(&history), vec![4, 5, 6]);
    history.append(record(7));
    assert_eq!(sequences(&history), vec![5, 6, 7]);
    assert_eq!(history.dropped_capacity(), 3);

    // A one-record window clears the same way.
    assert_eq!(history.clear(), 3);
    history.append(record(8));
    assert_eq!(sequences(&history), vec![8]);
    assert_eq!(history.clear(), 1);
    assert!(history.records().is_empty());
    assert_eq!(history.dropped_capacity(), 3);

    // Clearing an already empty window is a no-op.
    assert_eq!(history.clear(), 0);
    assert!(history.records().is_empty());
    assert_eq!(history.dropped_capacity(), 3);
}

#[test]
fn clearing_the_runtime_history_drops_records_and_the_cleared_loss_count() {
    let runtime = runtime(|configuration| {
        configuration.buffering.pending_records = 1;
        configuration.buffering.history_records = 4;
    });
    let logger = runtime.logger("test");
    logger.log(&LogEvent::new(Level::Info, "seed"));
    runtime.flush();

    // Hold the worker inside a replay callback so the flood overflows the
    // pending window deterministically instead of racing the drain.
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let blocked = Arc::new(AtomicBool::new(false));
    let observer = {
        let runtime = runtime.clone();
        let (entered, release, blocked) = (entered.clone(), release.clone(), blocked.clone());
        std::thread::spawn(move || {
            runtime.observe(
                LogQuery::default(),
                true,
                Arc::new(move |_record: &LogRecord| {
                    if !blocked.swap(true, Ordering::SeqCst) {
                        entered.wait();
                        release.wait();
                    }
                }),
            )
        })
    };
    entered.wait();
    for index in 0..16 {
        logger.log(&LogEvent::new(Level::Info, format!("flood {index}")));
    }
    release.wait();
    let observation = observer.join().expect("observer thread");
    runtime.flush();

    let before = runtime.snapshot(&LogQuery::default());
    assert!(
        before.dropped > 0,
        "the flood must overflow the pending window"
    );
    assert!(before.records.len() > 1);
    let last = before
        .records
        .last()
        .map(|record| record.sequence)
        .expect("retained records");

    let cleared = runtime.clear_history();
    assert_eq!(
        cleared,
        before.records.len(),
        "the clear reports the records it removed"
    );
    let snapshot = runtime.snapshot(&LogQuery::default());
    assert!(snapshot.records.is_empty(), "no record survives the clear");
    assert_eq!(
        snapshot.dropped, 0,
        "the cleared window's loss count is reset"
    );
    assert_eq!(snapshot.first_available_sequence, None);
    assert_eq!(runtime.status().dropped, 0);

    observation.cancel();
    for index in 0..5 {
        logger.log(&LogEvent::new(Level::Info, format!("after {index}")));
        runtime.flush();
    }
    let resumed = runtime.snapshot(&LogQuery::default());
    assert_eq!(
        resumed.records.len(),
        4,
        "history capacity survives the clear"
    );
    assert!(
        resumed.records.iter().all(|record| record.sequence > last),
        "sequences continue above the cleared window"
    );
    assert_eq!(resumed.dropped, 0, "no loss after the clear");
    assert_eq!(
        resumed.records.last().map(|record| record.message.as_str()),
        Some("after 4")
    );
}
