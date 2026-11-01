//! Translation of `Tests/CoveUIDevToolsTests/TimingPublisherTests.swift`.
//!
//! Behaviour checklist:
//! - a published frame keeps skipped and presented frames distinct
//! - delivery only starts after a sink is installed
//! - frame numbers advance per emitted record, and a skipped frame keeps the
//!   reported draw cost while `presented` is false

use std::cell::RefCell;
use std::rc::Rc;

use cove_devtools::devtools::mirror::TimingPublisher;
use cove_devtools_protocol::TimingFrame;

#[test]
fn timing_publisher_keeps_skipped_frames_distinct_from_presented_frames() {
    let publisher = TimingPublisher::new();
    let frames: Rc<RefCell<Vec<TimingFrame>>> = Rc::new(RefCell::new(Vec::new()));

    publisher.record(1.0, 2.0, 0.0, 3.0, 4, 1, true);
    assert!(
        frames.borrow().is_empty(),
        "records before a sink is installed are not delivered"
    );

    let sink = frames.clone();
    publisher.set_deliver(Some(Rc::new(move |frame: TimingFrame| {
        sink.borrow_mut().push(frame);
    })));

    publisher.record(1.0, 2.0, 1.0, 4.0, 4, 1, true);
    publisher.record(1.0, 2.0, 0.0, 3.0, 4, 1, false);

    let delivered = frames.borrow();
    assert_eq!(
        delivered
            .iter()
            .map(|frame| frame.frame)
            .collect::<Vec<_>>(),
        vec![1, 2],
        "frames are numbered from one and never reuse a number"
    );
    assert_eq!(
        delivered
            .iter()
            .map(|frame| frame.presented)
            .collect::<Vec<_>>(),
        vec![Some(true), Some(false)],
        "a skipped frame is reported as not presented"
    );
    assert_eq!(
        delivered[1].present_ms, 0.0,
        "a skipped frame presents nothing"
    );
    assert_eq!(
        delivered[1].draw_ms, 2.0,
        "skipping presentation must not erase the draw cost"
    );
}
