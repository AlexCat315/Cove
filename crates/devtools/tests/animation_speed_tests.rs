//! The runtime probe's global animation speed control.
//!
//! Flutter DevTools' slow-animations toggle changes the app's real animation
//! clock. Cove stores that scale in `cove-core`, so these tests drive the probe
//! command the controller sends and pin three facts: the reply is the clamped
//! value that actually took effect, invalid scales are refused, and probe
//! teardown returns the runtime to real time.

use cove_core::scene::{
    MAX_ANIMATION_TIME_SCALE, MIN_ANIMATION_TIME_SCALE, Node, NodeTree, animation_time_scale,
    reset_animation_time_scale, scale_animation_delta,
};
use cove_devtools::protocol::control::ProbeCommand;
use cove_devtools::runtime_probe::RuntimeProbe;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Mutex, MutexGuard};

/// The scale is process-global, so tests that touch it must not overlap.
static SCALE: Mutex<()> = Mutex::new(());

fn exclusive() -> MutexGuard<'static, ()> {
    SCALE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Restores real time even when an assertion panics mid-test.
struct RealTime;
impl Drop for RealTime {
    fn drop(&mut self) {
        reset_animation_time_scale();
    }
}

fn probe() -> RuntimeProbe {
    let mut nodes = NodeTree::new();
    nodes.set_root(Some(Node::new()));
    RuntimeProbe::new(Rc::new(RefCell::new(nodes)), None, None, None)
}

#[test]
fn the_probe_advertises_animation_speed() {
    assert!(
        probe()
            .capabilities()
            .contains(&"animation_speed".to_owned())
    );
}

#[test]
fn animation_scale_round_trips_and_reports_the_clamped_value() {
    let _serial = exclusive();
    let _real_time = RealTime;
    let mut probe = probe();

    let applied = probe
        .execute(ProbeCommand::AnimationScale { scale: 0.5 })
        .unwrap();
    assert_eq!(applied, serde_json::json!({"scale": 0.5}));
    assert_eq!(animation_time_scale(), 0.5);
    assert_eq!(scale_animation_delta(0.5), 0.25);

    let fast = probe
        .execute(ProbeCommand::AnimationScale { scale: 100.0 })
        .unwrap();
    assert_eq!(
        fast,
        serde_json::json!({"scale": MAX_ANIMATION_TIME_SCALE}),
        "the runtime reports the scale it stored"
    );
    assert_eq!(animation_time_scale(), MAX_ANIMATION_TIME_SCALE);

    let slow = probe
        .execute(ProbeCommand::AnimationScale { scale: 0.0 })
        .unwrap();
    assert_eq!(slow, serde_json::json!({"scale": MIN_ANIMATION_TIME_SCALE}));
    assert_eq!(animation_time_scale(), MIN_ANIMATION_TIME_SCALE);

    let fault = probe
        .execute(ProbeCommand::AnimationScale { scale: f64::NAN })
        .unwrap_err();
    assert_eq!(fault.code, "invalid_request");
    assert_eq!(
        animation_time_scale(),
        MIN_ANIMATION_TIME_SCALE,
        "a refused scale leaves the previous one in effect"
    );
}

#[test]
fn disconnect_returns_every_animation_to_real_time() {
    let _serial = exclusive();
    let _real_time = RealTime;
    let mut probe = probe();
    probe
        .execute(ProbeCommand::AnimationScale { scale: 0.25 })
        .unwrap();
    assert_eq!(animation_time_scale(), 0.25);

    probe.disconnect();

    assert_eq!(animation_time_scale(), 1.0);
    assert_eq!(scale_animation_delta(0.5), 0.5);
}
