//! Native allocation-site sampling behind `allocation_start`/`allocation_stop`.
//!
//! `cove_alloc` captures raw return addresses from the process allocator; this
//! module bounds, symbolizes and attributes them. Frames that belong to the
//! allocator itself are removed so a site starts at the code that requested the
//! block. Symbolization subtracts one from each return address, because the
//! captured value is the instruction after the call.

use cove_alloc::{AllocationSiteReport, MAX_SITE_FRAMES, SamplingError, SamplingSupport};
use cove_devtools_protocol::control::Fault;
use cove_devtools_protocol::{
    AllocationSite, AllocationSiteFrame, AllocationSiteStatus, MAX_ALLOCATION_RATE,
    MAX_ALLOCATION_SITES, UiAllocationSites,
};
use cove_profiling::Symbolizer;
use serde_json::{Value, json};

/// Frames kept per site: the capture depth `cove_alloc` retains.
const MAX_FRAMES_PER_SITE: usize = MAX_SITE_FRAMES;

/// `None` where real allocation stacks can be captured, else the reason.
pub(crate) fn unavailable_reason() -> Option<String> {
    match cove_alloc::sampling_support() {
        SamplingSupport::Supported => None,
        SamplingSupport::Unsupported { platform, reason } => Some(format!(
            "Allocation-site sampling is not available on {platform}: {reason}."
        )),
    }
}

fn invalid(message: String) -> Fault {
    Fault::new("invalid_request", message)
}

fn unavailable(reason: &str) -> Fault {
    Fault::new("unavailable", reason)
}

fn sampling_fault(error: &SamplingError) -> Fault {
    match error {
        SamplingError::Unsupported { .. } => Fault::new("unavailable", error.to_string()),
        SamplingError::InvalidRate => invalid(error.to_string()),
        SamplingError::AlreadySampling => Fault::new("busy", error.to_string()),
    }
}

/// Symbol resolution for the sampled addresses.
///
/// Linux can capture stacks but `cove_profiling` has no Linux symbolizer yet;
/// addresses are then returned unresolved with `symbolization_reason` set. The
/// probe never invents names.
struct Symbols {
    inner: Option<Symbolizer>,
    reason: Option<String>,
}

impl Symbols {
    fn new() -> Self {
        match Symbolizer::new() {
            Ok(symbolizer) => Self {
                inner: Some(symbolizer),
                reason: None,
            },
            Err(error) => Self {
                inner: None,
                reason: Some(error.to_string()),
            },
        }
    }

    /// Resolves one return address. `address` is the sampled value; the lookup
    /// uses `address - 1` so a return address maps to the call instruction.
    fn resolve(&mut self, address: u64) -> (Option<String>, Option<String>, Option<u32>) {
        let Some(symbolizer) = self.inner.as_mut() else {
            return (None, None, None);
        };
        let frame = symbolizer.resolve(address.saturating_sub(1));
        let (file_path, line) = match frame.location {
            Some(location) => (Some(location.file), location.line),
            None => (None, None),
        };
        (frame.function, file_path, line)
    }
}

/// Whether a resolved frame belongs to the allocator or its shims rather than
/// to the code that requested the block.
fn is_allocator_internal(frame: &AllocationSiteFrame) -> bool {
    if frame.file_path.as_deref().is_some_and(|path| {
        path.contains("crates/alloc/src/") || path.contains("crates\\alloc\\src\\")
    }) {
        return true;
    }
    let Some(function) = frame.function.as_deref() else {
        return false;
    };
    const NAMES: [&str; 21] = [
        "cove_alloc::",
        "cove_alloc::TrackingAllocator",
        "cove_alloc::sampling",
        "TrackingAllocator",
        "__rust_alloc",
        "__rust_alloc_zeroed",
        "__rust_realloc",
        "__rg_alloc",
        "__rg_alloc_zeroed",
        "__rg_realloc",
        "__rdl_alloc",
        "__rdl_alloc_zeroed",
        "__rdl_realloc",
        "alloc::alloc::alloc",
        "alloc::alloc::alloc_zeroed",
        "alloc::alloc::exchange_malloc",
        "libc::backtrace",
        "backtrace::",
        "RtlCaptureStackBackTrace",
        "_Unwind_Backtrace",
        "_Unwind_GetIP",
    ];
    NAMES.iter().any(|name| function.contains(name))
}

/// Drops allocator frames from one captured stack.
///
/// A stack that resolves entirely to allocator internals keeps its addresses:
/// they are real samples, and dropping them would hide the blocks.
fn visible_frames(frames: Vec<AllocationSiteFrame>) -> Vec<AllocationSiteFrame> {
    match frames
        .iter()
        .position(|frame| !is_allocator_internal(frame))
    {
        Some(first) => frames
            .into_iter()
            .skip(first)
            .filter(|frame| !is_allocator_internal(frame))
            .collect(),
        None => frames,
    }
}

/// A processed answer: bounded sites plus the sampling counters they came from.
struct Processed {
    rate: u32,
    sampled_bytes: u64,
    sampled_blocks: u64,
    samples_attempted: u64,
    samples_retained: u64,
    samples_dropped: u64,
    capture_failures: u64,
    total_sites: usize,
    symbolization_reason: Option<String>,
    sites: Vec<AllocationSite>,
}

impl Processed {
    fn empty(rate: u32, symbolization_reason: Option<String>) -> Self {
        Self {
            rate,
            sampled_bytes: 0,
            sampled_blocks: 0,
            samples_attempted: 0,
            samples_retained: 0,
            samples_dropped: 0,
            capture_failures: 0,
            total_sites: 0,
            symbolization_reason,
            sites: Vec::new(),
        }
    }

    fn payload(&self, status: AllocationSiteStatus, limit: usize) -> UiAllocationSites {
        let count = limit.min(self.sites.len());
        UiAllocationSites {
            status,
            rate: self.rate,
            sampled_bytes: self.sampled_bytes,
            sampled_blocks: self.sampled_blocks,
            samples_attempted: self.samples_attempted,
            samples_retained: self.samples_retained,
            samples_dropped: self.samples_dropped,
            capture_failures: self.capture_failures,
            sites_truncated: self.total_sites > count,
            site_count: self.total_sites,
            symbolization_reason: self.symbolization_reason.clone(),
            sites: self.sites[..count].to_vec(),
        }
    }
}

/// The allocation-site sampler owned by one runtime probe.
pub(crate) struct AllocationSites {
    symbols: Symbols,
    /// This probe started the session; disconnect only stops its own.
    session: bool,
    /// The last stopped report, kept until the next start or disconnect.
    last: Option<Processed>,
}

impl AllocationSites {
    pub(crate) fn new() -> Self {
        Self {
            symbols: Symbols::new(),
            session: false,
            last: None,
        }
    }

    /// Starts a fresh sampling window at 1-in-`rate`.
    ///
    /// Any session already running is stopped first; its report belongs to the
    /// previous window and is discarded.
    pub(crate) fn start(&mut self, rate: u32) -> Result<Value, Fault> {
        if !(1..=MAX_ALLOCATION_RATE).contains(&rate) {
            return Err(invalid(format!(
                "rate must be an integer from 1 to {MAX_ALLOCATION_RATE}."
            )));
        }
        if let Some(reason) = unavailable_reason() {
            return Err(unavailable(&reason));
        }
        if cove_alloc::sampling_enabled() {
            let _ = cove_alloc::stop_sampling();
        }
        self.session = false;
        self.last = None;
        cove_alloc::start_sampling(rate).map_err(|error| sampling_fault(&error))?;
        self.session = true;
        Ok(json!(
            Processed::empty(rate, self.symbols.reason.clone())
                .payload(AllocationSiteStatus::Running, 0)
        ))
    }

    /// Stops the real sampler and returns its bounded, symbolized sites.
    pub(crate) fn stop(&mut self) -> Result<Value, Fault> {
        if let Some(reason) = unavailable_reason() {
            return Err(unavailable(&reason));
        }
        let Some(report) = cove_alloc::stop_sampling() else {
            self.session = false;
            return Err(Fault::new(
                "no_allocation_profile",
                "Start allocation-site sampling before stopping it.",
            ));
        };
        self.session = false;
        let processed = self.process(report, MAX_ALLOCATION_SITES);
        let payload = processed.payload(AllocationSiteStatus::Ready, MAX_ALLOCATION_SITES);
        self.last = Some(processed);
        Ok(json!(payload))
    }

    /// The live session's sites, or the last stopped report.
    pub(crate) fn sites(&mut self, limit: usize) -> Result<Value, Fault> {
        if !(1..=MAX_ALLOCATION_SITES).contains(&limit) {
            return Err(invalid(format!(
                "limit must be an integer from 1 to {MAX_ALLOCATION_SITES}."
            )));
        }
        if let Some(reason) = unavailable_reason() {
            return Err(unavailable(&reason));
        }
        if let Some(report) = cove_alloc::allocation_sites() {
            // A live query reads a stable snapshot; capture pauses only while
            // the ring is copied.
            let processed = self.process(report, limit);
            return Ok(json!(
                processed.payload(AllocationSiteStatus::Running, limit)
            ));
        }
        if let Some(last) = &self.last {
            return Ok(json!(last.payload(AllocationSiteStatus::Ready, limit)));
        }
        Ok(json!(
            Processed::empty(0, self.symbols.reason.clone())
                .payload(AllocationSiteStatus::Idle, limit)
        ))
    }

    /// Stops this probe's session and forgets its report on disconnect.
    pub(crate) fn reset(&mut self) {
        if self.session {
            let _ = cove_alloc::stop_sampling();
        }
        self.session = false;
        self.last = None;
    }

    /// Bounds, symbolizes and attributes the report's sites.
    ///
    /// Only the first `limit` sites (already sorted by sampled bytes) are
    /// resolved, so one answer costs at most `limit * MAX_SITE_FRAMES`
    /// lookups.
    fn process(&mut self, report: AllocationSiteReport, limit: usize) -> Processed {
        let total_sites = report.sites.len();
        let mut sites = Vec::with_capacity(total_sites.min(limit));
        for site in report.sites.iter().take(limit) {
            let mut frames = Vec::with_capacity(site.frames.len());
            for address in site.frames.iter().take(MAX_FRAMES_PER_SITE) {
                let (function, file_path, line) = self.symbols.resolve(*address);
                frames.push(AllocationSiteFrame {
                    address: *address,
                    function,
                    file_path,
                    line,
                });
            }
            sites.push(AllocationSite {
                bytes: site.bytes,
                blocks: site.blocks,
                frames_truncated: site.frames.len() >= MAX_FRAMES_PER_SITE,
                frames: visible_frames(frames),
            });
        }
        Processed {
            rate: report.rate,
            sampled_bytes: report.sampled_bytes,
            sampled_blocks: report.sampled_blocks,
            samples_attempted: report.samples_attempted,
            samples_retained: report.samples_retained,
            samples_dropped: report.samples_dropped,
            capture_failures: report.capture_failures,
            total_sites,
            symbolization_reason: self.symbols.reason.clone(),
            sites,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(function: &str, file_path: Option<&str>) -> AllocationSiteFrame {
        AllocationSiteFrame {
            address: 0x1000,
            function: Some(function.to_owned()),
            file_path: file_path.map(str::to_owned),
            line: None,
        }
    }

    #[test]
    fn allocator_names_and_sources_are_internal() {
        for frame in [
            frame("cove_alloc::sampling::record_allocation_enabled", None),
            frame(
                "<cove_alloc::TrackingAllocator<A> as core::alloc::GlobalAlloc>::alloc",
                None,
            ),
            frame("__rust_alloc", None),
            frame("alloc::alloc::exchange_malloc", None),
            frame("libc::backtrace", None),
            frame("some_symbol", Some("/tmp/crates/alloc/src/lib.rs")),
        ] {
            assert!(
                is_allocator_internal(&frame),
                "{:?} belongs to the allocator",
                frame.function
            );
        }
    }

    #[test]
    fn caller_frames_are_not_internal() {
        for frame in [
            frame("gallery::paint", Some("/tmp/examples/gallery/src/paint.rs")),
            frame("alloc::vec::Vec<T>::push", None),
            frame("std::collections::HashMap::insert", None),
        ] {
            assert!(
                !is_allocator_internal(&frame),
                "{:?} is caller code",
                frame.function
            );
        }
    }

    #[test]
    fn the_captured_stack_starts_at_the_requester() {
        let frames = vec![
            frame("cove_alloc::sampling::record_allocation_enabled", None),
            frame("__rust_alloc", None),
            frame("gallery::paint", None),
            frame("gallery::frame", None),
        ];
        let visible = visible_frames(frames);
        assert_eq!(
            visible
                .iter()
                .map(|frame| frame.function.as_deref().unwrap_or(""))
                .collect::<Vec<_>>(),
            ["gallery::paint", "gallery::frame"]
        );
    }

    #[test]
    fn a_stack_without_a_caller_keeps_its_real_addresses() {
        let frames = vec![
            frame("cove_alloc::sampling::record_allocation_enabled", None),
            frame("__rust_alloc", None),
        ];
        let visible = visible_frames(frames.clone());
        assert_eq!(
            visible.len(),
            frames.len(),
            "no caller frame was captured, but the addresses are real"
        );
    }

    #[test]
    fn the_symbolizer_resolves_a_real_function_address() {
        let mut symbols = Symbols::new();
        if symbols.reason.is_some() {
            return;
        }
        // `resolve` subtracts the return-address bias, so pointing one byte
        // past the entry resolves the entry instruction itself.
        let address =
            the_symbolizer_resolves_a_real_function_address as *const () as usize as u64 + 1;
        let (function, _, _) = symbols.resolve(address);
        let function = function.expect("a debug executable resolves its own function");
        assert!(
            function.contains("the_symbolizer_resolves_a_real_function_address"),
            "resolved {function}"
        );
    }
}
