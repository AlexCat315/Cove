//! Runs allocation-site sampling through the runtime probe against the real
//! process allocator.
//!
//! `harness = false` keeps libtest from allocating on background threads, so a
//! sampling window started and stopped here sees only the allocations this
//! binary makes. Counters, stacks and symbols are the probe's real answer:
//! nothing is synthesized. A hang instead of a result would mean the sampling
//! hook deadlocked.

use cove_alloc::{SamplingSupport, TrackingAllocator};
use cove_core::scene::NodeTree;
use cove_devtools::protocol::control::ProbeCommand;
use cove_devtools::runtime_probe::RuntimeProbe;
use std::alloc::{Layout, alloc, dealloc};
use std::cell::RefCell;
use std::rc::Rc;

#[global_allocator]
static ALLOC: TrackingAllocator = TrackingAllocator::new();

/// Allocations the busy loop makes; each one is sampled when `rate` allows.
const BUSY_ALLOCATIONS: usize = 512;
const BUSY_SIZE: usize = 256;

fn main() {
    let mut probe = RuntimeProbe::new(Rc::new(RefCell::new(NodeTree::new())), None, None, None);
    let capabilities = probe.capabilities();
    let supported = matches!(cove_alloc::sampling_support(), SamplingSupport::Supported);
    assert_eq!(
        capabilities.iter().any(|name| name == "allocation_sites"),
        supported,
        "the capability is advertised exactly where stacks can be captured"
    );

    rate_bounds(&mut probe);
    if supported {
        real_sampling(&mut probe);
        disconnect_stops_sampling(&mut probe);
    } else {
        unsupported_path(&mut probe);
    }
    println!("allocation-site probe checks passed");
}

fn rate_bounds(probe: &mut RuntimeProbe) {
    for rate in [0, cove_devtools_protocol::MAX_ALLOCATION_RATE + 1] {
        let fault = probe
            .execute(ProbeCommand::AllocationStart { rate })
            .unwrap_err();
        assert_eq!(
            fault.code, "invalid_request",
            "rate {rate} is outside the accepted bounds"
        );
    }
    let fault = probe
        .execute(ProbeCommand::AllocationSites { limit: 0 })
        .unwrap_err();
    assert_eq!(fault.code, "invalid_request");
    let fault = probe
        .execute(ProbeCommand::AllocationSites {
            limit: cove_devtools_protocol::MAX_ALLOCATION_SITES + 1,
        })
        .unwrap_err();
    assert_eq!(fault.code, "invalid_request");
    assert!(
        !cove_alloc::sampling_enabled(),
        "a rejected request must not start a session"
    );
}

fn real_sampling(probe: &mut RuntimeProbe) {
    // A second start replaces the first session: the report must describe only
    // allocations after the replacement.
    let started = probe
        .execute(ProbeCommand::AllocationStart { rate: 1 })
        .expect("capture is supported");
    assert_eq!(started["status"], "running");
    assert_eq!(started["rate"], 1);
    busy_allocations();

    let replaced = probe
        .execute(ProbeCommand::AllocationStart { rate: 1 })
        .expect("a start replaces the prior session");
    assert_eq!(replaced["status"], "running");
    assert_eq!(
        replaced["samplesAttempted"], 0,
        "a new window starts with cleared counters"
    );
    busy_allocations();

    let stopped = probe
        .execute(ProbeCommand::AllocationStop)
        .expect("a session was running");
    assert_eq!(stopped["status"], "ready");
    assert_eq!(stopped["rate"], 1);
    let attempted = stopped["samplesAttempted"].as_u64().unwrap();
    let retained = stopped["samplesRetained"].as_u64().unwrap();
    let dropped = stopped["samplesDropped"].as_u64().unwrap();
    let sampled_blocks = stopped["sampledBlocks"].as_u64().unwrap();
    let sampled_bytes = stopped["sampledBytes"].as_u64().unwrap();
    assert!(
        attempted >= BUSY_ALLOCATIONS as u64,
        "the busy loop's allocations were attempted: {attempted}"
    );
    assert_eq!(
        retained + dropped + stopped["captureFailures"].as_u64().unwrap(),
        attempted,
        "every attempted sample is retained, dropped or a capture failure"
    );
    assert!(
        retained > 0,
        "the ring retained real samples from the busy loop"
    );
    assert!(
        sampled_blocks >= BUSY_ALLOCATIONS as u64,
        "the sampled blocks include the busy loop: {sampled_blocks}"
    );
    assert!(
        sampled_bytes >= (BUSY_ALLOCATIONS * BUSY_SIZE) as u64,
        "the sampled bytes include the busy loop: {sampled_bytes}"
    );

    let sites = stopped["sites"].as_array().expect("sites is an array");
    assert!(!sites.is_empty(), "real allocations produced real sites");
    assert!(
        sites.len() <= cove_devtools_protocol::MAX_ALLOCATION_SITES,
        "one answer is bounded"
    );
    let site_count = stopped["siteCount"].as_u64().unwrap() as usize;
    assert!(site_count >= sites.len());
    assert_eq!(
        stopped["sitesTruncated"].as_bool().unwrap(),
        site_count > sites.len(),
        "the truncation flag matches the cut"
    );

    let symbols_available = stopped["symbolizationReason"].is_null();
    let mut busy_site_seen = false;
    for site in sites {
        assert!(
            site["bytes"].as_u64().unwrap() > 0,
            "a sampled site allocates real bytes"
        );
        assert!(site["blocks"].as_u64().unwrap() > 0);
        assert!(site["framesTruncated"].is_boolean());
        let frames = site["frames"]
            .as_array()
            .expect("every site carries its captured frames");
        assert!(!frames.is_empty(), "a site carries at least one frame");
        // The filtered stack begins at the requester: an allocator frame may
        // only appear when the whole captured stack belongs to the allocator.
        let first_caller = frames.iter().position(|frame| !internal(frame));
        if let Some(index) = first_caller {
            assert_eq!(
                index, 0,
                "allocator frames must not precede the requesting frame: {frames:?}"
            );
        }
        for frame in frames {
            assert!(
                frame["address"].as_u64().unwrap() > 0,
                "a captured address is real"
            );
        }
        if symbols_available
            && frames
                .iter()
                .any(|frame| names_busy_allocation(frame) || is_this_test_file(frame))
        {
            busy_site_seen = true;
            assert!(
                site["bytes"].as_u64().unwrap() >= (BUSY_ALLOCATIONS * BUSY_SIZE) as u64,
                "the busy site's bytes come from the busy loop"
            );
        }
    }
    if symbols_available {
        assert!(
            busy_site_seen,
            "the busy loop's allocation site is among the resolved sites"
        );
        assert!(
            sites.iter().any(|site| {
                site["frames"]
                    .as_array()
                    .unwrap()
                    .first()
                    .is_some_and(|frame| {
                        frame["function"].is_string() || frame["filePath"].is_string()
                    })
            }),
            "the macOS symbolizer resolves at least one real function or file"
        );
    }

    // The last report is queryable after the stop, with the same counters.
    let queried = probe
        .execute(ProbeCommand::AllocationSites { limit: 1 })
        .expect("the last report is retained");
    assert_eq!(queried["status"], "ready");
    assert_eq!(queried["rate"], 1);
    assert_eq!(queried["sampledBytes"], stopped["sampledBytes"]);
    assert_eq!(queried["samplesAttempted"], stopped["samplesAttempted"]);
    assert_eq!(queried["siteCount"], stopped["siteCount"]);
    assert!(queried["sites"].as_array().unwrap().len() <= 1);
    assert_eq!(
        queried["sitesTruncated"].as_bool().unwrap(),
        site_count > 1,
        "the query reports the cut it applied"
    );

    // A live query reads the running session instead of the stopped report.
    probe
        .execute(ProbeCommand::AllocationStart { rate: 4 })
        .expect("capture is supported");
    busy_allocations();
    let live = probe
        .execute(ProbeCommand::AllocationSites { limit: 4 })
        .expect("a live session answers");
    assert_eq!(live["status"], "running");
    assert_eq!(live["rate"], 4);
    assert!(live["sites"].as_array().unwrap().len() <= 4);
    assert!(live["siteCount"].as_u64().unwrap() >= 1);
    probe
        .execute(ProbeCommand::AllocationStop)
        .expect("the live session stops");

    let fault = probe.execute(ProbeCommand::AllocationStop).unwrap_err();
    assert_eq!(
        fault.code, "no_allocation_profile",
        "stopping without a session is explicit"
    );
}

fn disconnect_stops_sampling(probe: &mut RuntimeProbe) {
    probe
        .execute(ProbeCommand::AllocationStart { rate: 64 })
        .expect("capture is supported");
    assert!(
        cove_alloc::sampling_enabled(),
        "the real sampler is running"
    );
    probe.disconnect();
    assert!(
        !cove_alloc::sampling_enabled(),
        "disconnect stops the sampler instead of leaving it running"
    );
    let idle = probe
        .execute(ProbeCommand::AllocationSites { limit: 4 })
        .expect("a supported runtime answers");
    assert_eq!(idle["status"], "idle");
    assert_eq!(idle["siteCount"], 0);
    assert!(idle["sites"].as_array().unwrap().is_empty());
}

fn unsupported_path(probe: &mut RuntimeProbe) {
    let reason = match cove_alloc::sampling_support() {
        SamplingSupport::Unsupported { reason, .. } => reason,
        SamplingSupport::Supported => unreachable!("this path is for unsupported targets"),
    };
    for command in [
        ProbeCommand::AllocationStart { rate: 1 },
        ProbeCommand::AllocationStop,
        ProbeCommand::AllocationSites { limit: 4 },
    ] {
        let fault = probe.execute(command).unwrap_err();
        assert_eq!(fault.code, "unavailable");
        assert!(
            fault.message.contains(reason) || fault.message.contains("not available"),
            "the gap names the platform reason: {}",
            fault.message
        );
    }
    assert!(
        !cove_alloc::sampling_enabled(),
        "an unsupported target never starts a sampler"
    );
}

fn internal(frame: &serde_json::Value) -> bool {
    if frame["filePath"]
        .as_str()
        .is_some_and(|path| path.contains("crates/alloc/src/"))
    {
        return true;
    }
    let Some(function) = frame["function"].as_str() else {
        return false;
    };
    [
        "cove_alloc::",
        "TrackingAllocator",
        "__rust_alloc",
        "__rg_alloc",
        "__rdl_alloc",
        "alloc::alloc::alloc",
        "alloc::alloc::exchange_malloc",
        "libc::backtrace",
        "backtrace::",
    ]
    .iter()
    .any(|name| function.contains(name))
}

fn names_busy_allocation(frame: &serde_json::Value) -> bool {
    frame["function"]
        .as_str()
        .is_some_and(|function| function.contains("busy_allocations"))
}

fn is_this_test_file(frame: &serde_json::Value) -> bool {
    frame["filePath"]
        .as_str()
        .is_some_and(|path| path.ends_with("tests/allocation_sites.rs"))
}

#[inline(never)]
fn busy_allocations() {
    let layout = Layout::from_size_align(BUSY_SIZE, 8).expect("a valid layout");
    for _ in 0..BUSY_ALLOCATIONS {
        // SAFETY: the layout has non-zero size and a valid alignment; each
        // block is released with the same layout before the next allocation.
        let pointer = unsafe { alloc(layout) };
        assert!(!pointer.is_null(), "the system allocator returned null");
        // SAFETY: `pointer` came from `alloc` with exactly `layout`.
        unsafe { dealloc(pointer, layout) };
    }
}
