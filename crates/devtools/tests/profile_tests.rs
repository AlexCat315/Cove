//! UI CPU profiles through the runtime probe.
//!
//! The wire shape is pinned here: bounded aggregated stacks instead of one
//! record per sample, aggregated self/inclusive hotspots with `function
//! file:line`, and `unavailable` with the platform reason instead of
//! synthesized samples.

use cove_core::scene::NodeTree;
use cove_devtools::protocol::control::ProbeCommand;
use cove_devtools::runtime_probe::RuntimeProbe;
use std::cell::RefCell;
use std::rc::Rc;

fn probe() -> RuntimeProbe {
    RuntimeProbe::new(Rc::new(RefCell::new(NodeTree::new())), None, None, None)
}

#[test]
fn profile_start_rejects_requests_outside_the_bounds() {
    let mut probe = probe();
    for (hz, duration_ms) in [(0, 100), (1_001, 100), (100, 0), (100, 10_001)] {
        let fault = probe
            .execute(ProbeCommand::ProfileStart { hz, duration_ms })
            .unwrap_err();
        assert_eq!(
            fault.code, "invalid_request",
            "{hz} Hz for {duration_ms} ms is outside the accepted bounds"
        );
    }
    let fault = probe
        .execute(ProbeCommand::ProfileHotspots { limit: 0 })
        .unwrap_err();
    assert_eq!(fault.code, "invalid_request");
    assert!(!probe.ui_thread_name().is_empty());
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::*;
    use std::time::{Duration, Instant};

    // Linux and Android install one process-wide sampling signal handler.
    static SAMPLER_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn profiles_the_probe_thread() {
        let _guard = SAMPLER_LOCK.lock().unwrap();
        std::thread::Builder::new()
            .name("cove-prof-probe".into())
            .spawn(verify_probe_profile)
            .unwrap()
            .join()
            .unwrap();
    }

    fn verify_probe_profile() {
        let mut probe = probe();
        if let cove_profiling::PlatformSupport::Unsupported { reason } =
            cove_profiling::platform_support()
        {
            verify_unsupported(&mut probe, reason);
            return;
        }
        let thread = probe.ui_thread_name().to_owned();
        assert!(
            probe
                .capabilities()
                .iter()
                .any(|name| name == "cpu_profile"),
            "the runtime advertises its sampler"
        );
        assert_eq!(
            probe.execute(ProbeCommand::ProfileRead).unwrap()["status"],
            "idle"
        );

        let started = probe
            .execute(ProbeCommand::ProfileStart {
                hz: 500,
                duration_ms: 300,
            })
            .unwrap();
        assert_eq!(started["status"], "running");
        assert_eq!(started["thread"], thread.as_str());
        assert_eq!(
            probe
                .execute(ProbeCommand::ProfileStart {
                    hz: 500,
                    duration_ms: 300
                })
                .unwrap_err()
                .code,
            "busy",
            "a second start must not run two samplers"
        );

        // The probe thread stays runnable while the sampler follows it.
        let deadline = Instant::now() + Duration::from_secs(10);
        let profile = loop {
            let profile = probe.execute(ProbeCommand::ProfileRead).unwrap();
            match profile["status"].as_str() {
                Some("ready") => break profile,
                Some("running") => {}
                other => panic!("unexpected profile status {other:?}: {profile}"),
            }
            assert!(
                Instant::now() < deadline,
                "the sampler never published a profile"
            );
            std::thread::sleep(Duration::from_millis(20));
        };

        assert_eq!(profile["thread"], thread.as_str());
        assert_eq!(profile["source"], "sampler");
        assert_eq!(profile["hz"], 500);
        assert_eq!(profile["sampleIntervalMs"], 2.0);
        let samples = profile["samplesTaken"].as_u64().unwrap();
        let retained = profile["retainedSamples"].as_u64().unwrap();
        assert!(samples > 0, "no live samples were taken: {profile}");
        assert!(retained > 0, "no samples were retained: {profile}");
        assert!(retained <= samples, "retained more samples than taken");
        assert_eq!(
            profile["samplesDropped"].as_u64(),
            Some(0),
            "a bounded run keeps every sample: {profile}"
        );
        let duration = profile["durationMs"].as_f64().unwrap();
        assert!(
            (250.0..3_000.0).contains(&duration),
            "the run ignored its bound: {duration} ms"
        );

        let stacks = profile["stacks"].as_array().unwrap();
        assert!(!stacks.is_empty(), "a retained run has no stacks");
        assert!(
            stacks.len() <= 24,
            "the wire carries a bounded number of stacks"
        );
        assert!(
            stacks
                .windows(2)
                .all(|pair| pair[0]["samples"].as_u64() >= pair[1]["samples"].as_u64()),
            "stacks are ordered by sample count"
        );
        let stack_samples: u64 = stacks
            .iter()
            .map(|stack| stack["samples"].as_u64().unwrap())
            .sum();
        assert!(
            stack_samples <= retained,
            "stack counts cannot exceed the retained samples"
        );
        for stack in stacks {
            let frames = stack["frames"].as_array().unwrap();
            assert!(!frames.is_empty(), "a stack has no frames");
            for frame in frames {
                assert!(
                    frame["label"]
                        .as_str()
                        .is_some_and(|label| !label.is_empty()),
                    "a frame has no label: {frame}"
                );
            }
        }

        let hotspots = probe
            .execute(ProbeCommand::ProfileHotspots { limit: 4 })
            .unwrap();
        assert_eq!(hotspots["status"], "ready");
        let entries = hotspots["hotspots"].as_array().unwrap();
        assert!(!entries.is_empty(), "a sampled run has no hotspots");
        assert!(entries.len() <= 4, "the query limit is bounded");
        assert_eq!(hotspots["retainedSamples"], retained);
        for entry in entries {
            assert!(
                entry["label"]
                    .as_str()
                    .is_some_and(|label| !label.is_empty()),
                "a hotspot has no label: {entry}"
            );
            let self_samples = entry["selfSamples"].as_u64().unwrap();
            let inclusive = entry["inclusiveSamples"].as_u64().unwrap();
            assert!(self_samples <= inclusive);
            assert!(inclusive <= retained);
            assert!(entry["selfMs"].as_f64().unwrap() >= 0.0);
            assert!(
                entry["selfPercent"].as_f64().unwrap() <= 100.0 + f64::EPSILON,
                "self time cannot exceed the profile: {entry}"
            );
        }
        assert!(
            entries
                .windows(2)
                .all(|pair| pair[0]["selfSamples"].as_u64() >= pair[1]["selfSamples"].as_u64()),
            "hotspots are ordered by self time"
        );

        let stopped = probe.execute(ProbeCommand::ProfileStop).unwrap();
        assert_eq!(stopped["status"], "ready");
        assert_eq!(stopped["samplesTaken"], profile["samplesTaken"]);
        assert_eq!(
            probe.execute(ProbeCommand::ProfileStop).unwrap_err().code,
            "no_profile",
            "a second stop has no active run"
        );
        assert_eq!(
            probe.execute(ProbeCommand::ProfileRead).unwrap()["status"],
            "ready",
            "the last profile stays readable after stop"
        );

        probe.disconnect();
        assert_eq!(
            probe.execute(ProbeCommand::ProfileRead).unwrap()["status"],
            "idle",
            "disconnecting stops sampling and forgets the run"
        );
    }

    #[test]
    fn stops_an_active_run_before_its_bound() {
        if !matches!(
            cove_profiling::platform_support(),
            cove_profiling::PlatformSupport::Supported
        ) {
            return;
        }
        let _guard = SAMPLER_LOCK.lock().unwrap();
        std::thread::Builder::new()
            .name("cove-prof-stop".into())
            .spawn(verify_early_stop)
            .unwrap()
            .join()
            .unwrap();
    }

    fn verify_early_stop() {
        let mut probe = probe();
        probe
            .execute(ProbeCommand::ProfileStart {
                hz: 500,
                duration_ms: 5_000,
            })
            .unwrap();
        std::thread::sleep(Duration::from_millis(200));
        let running = probe.execute(ProbeCommand::ProfileRead).unwrap();
        assert!(
            matches!(running["status"].as_str(), Some("running" | "ready")),
            "an active run reports itself: {running}"
        );
        let stopped = probe.execute(ProbeCommand::ProfileStop).unwrap();
        assert_eq!(stopped["status"], "ready");
        let duration = stopped["durationMs"].as_f64().unwrap();
        assert!(
            duration < 5_000.0,
            "stop waited for the bound instead of the request: {duration} ms"
        );
        assert_eq!(
            stopped["samplesDropped"].as_u64(),
            Some(0),
            "stopping early keeps the ring within its bound"
        );
    }
    fn verify_unsupported(probe: &mut RuntimeProbe, reason: &str) {
        assert!(
            !probe
                .capabilities()
                .iter()
                .any(|name| name == "cpu_profile"),
            "a runtime without a sampler must not advertise one"
        );
        let fault = probe
            .execute(ProbeCommand::ProfileStart {
                hz: 100,
                duration_ms: 100,
            })
            .unwrap_err();
        assert_eq!(fault.code, "unavailable");
        assert_eq!(fault.message, reason);
        for command in [
            ProbeCommand::ProfileStop,
            ProbeCommand::ProfileRead,
            ProbeCommand::ProfileHotspots { limit: 10 },
        ] {
            let fault = probe.execute(command).unwrap_err();
            assert_eq!(fault.code, "unavailable");
        }
    }
}
