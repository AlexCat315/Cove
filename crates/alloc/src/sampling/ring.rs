//! Fixed-size, lock-free ring of captured allocation stacks.
//!
//! Producers are allocator hooks, so a producer must never block. A claim is a
//! single `fetch_add` on a process-wide counter; the slot it selects is written
//! under a seqlock-style version: an odd version means a writer is active and an
//! even, non-zero version marks a complete sample. A reader re-checks the
//! version after copying, so a slot that was overwritten mid-read is discarded
//! rather than reported as a torn stack.
//!
//! The ring is bounded: a slot is reused once `CAPACITY` further samples have
//! been claimed, and the oldest sample is the one that disappears. Readers that
//! need an exact set (stop and query) halt producers first, so overwrites cannot
//! race with aggregation.

use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

/// Most instruction addresses retained for one allocation site.
pub(crate) const MAX_FRAMES: usize = 8;

/// Number of samples the ring can hold before it starts overwriting.
pub(crate) const CAPACITY: usize = 1024;

const SLOT_MASK: usize = CAPACITY - 1;

/// A bounded ring of raw stack samples.
pub(crate) struct SampleRing {
    claims: AtomicUsize,
    slots: [Slot; CAPACITY],
}

struct Slot {
    /// Even: a complete sample at this generation; odd: a writer is active;
    /// zero: never written (or cleared).
    version: AtomicUsize,
    depth: AtomicU8,
    frames: [AtomicUsize; MAX_FRAMES],
    bytes: AtomicUsize,
    blocks: AtomicUsize,
}

impl Slot {
    const fn new() -> Self {
        Self {
            version: AtomicUsize::new(0),
            depth: AtomicU8::new(0),
            frames: [const { AtomicUsize::new(0) }; MAX_FRAMES],
            bytes: AtomicUsize::new(0),
            blocks: AtomicUsize::new(0),
        }
    }
}

impl SampleRing {
    pub(crate) const fn new() -> Self {
        Self {
            claims: AtomicUsize::new(0),
            slots: [const { Slot::new() }; CAPACITY],
        }
    }

    /// Stores one sample, returning `false` when another writer held the slot.
    ///
    /// The hook never retries: a lost sample is preferable to time spent in the
    /// allocator, and the report accounts for it as dropped.
    pub(crate) fn push(&self, frames: &[usize], bytes: usize, blocks: usize) -> bool {
        if frames.is_empty() || frames.len() > MAX_FRAMES {
            return false;
        }
        let claim = self.claims.fetch_add(1, Ordering::Relaxed);
        let slot = &self.slots[claim & SLOT_MASK];
        let base = slot.version.load(Ordering::Acquire);
        // An odd version is a writer in progress; a slot never written starts
        // at zero and is claimable.
        if base & 1 != 0 {
            return false;
        }
        // Mark the slot busy only if nobody else claimed this generation. A
        // failed compare-exchange means a racing producer owns the slot and
        // this sample is dropped.
        if slot
            .version
            .compare_exchange(
                base,
                base.wrapping_add(1),
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_err()
        {
            return false;
        }
        slot.depth.store(frames.len() as u8, Ordering::Relaxed);
        for (index, frame) in frames.iter().enumerate() {
            slot.frames[index].store(*frame, Ordering::Relaxed);
        }
        slot.bytes.store(bytes, Ordering::Relaxed);
        slot.blocks.store(blocks, Ordering::Relaxed);
        // Release publishes the sample; a reader that observes this version
        // also observes every field above.
        slot.version.store(base.wrapping_add(2), Ordering::Release);
        true
    }

    /// Visits every complete sample and returns how many were retained.
    ///
    /// This is safe while producers run, but a sample overwritten during the
    /// scan is skipped. Callers that need an exact snapshot halt producers
    /// first.
    pub(crate) fn visit(&self, mut visit: impl FnMut(&[usize], usize, usize)) -> u64 {
        let mut retained = 0;
        let mut frames = [0usize; MAX_FRAMES];
        for slot in &self.slots {
            let before = slot.version.load(Ordering::Acquire);
            if before == 0 || before & 1 != 0 {
                continue;
            }
            let depth = (slot.depth.load(Ordering::Relaxed) as usize).min(MAX_FRAMES);
            if depth == 0 {
                continue;
            }
            for (index, frame) in frames.iter_mut().enumerate().take(depth) {
                *frame = slot.frames[index].load(Ordering::Relaxed);
            }
            let bytes = slot.bytes.load(Ordering::Relaxed);
            let blocks = slot.blocks.load(Ordering::Relaxed);
            // A changed version means a writer reused the slot while it was
            // being copied; the stack is not trustworthy.
            if slot.version.load(Ordering::Acquire) != before {
                continue;
            }
            // A null address is never a real instruction pointer. Skipping it
            // keeps a partially captured stack from being published as a site.
            if frames[..depth].contains(&0) {
                continue;
            }
            visit(&frames[..depth], bytes, blocks);
            retained += 1;
        }
        retained
    }

    /// Drops every sample. The caller must have halted producers.
    pub(crate) fn clear(&self) {
        for slot in &self.slots {
            slot.version.store(0, Ordering::Release);
        }
    }
}
