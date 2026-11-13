//! Fixed-size sample ring, written only by the sampler thread.

use std::time::Duration;

use crate::config::MAX_STACK_DEPTH;
use crate::Sample;

/// One captured stack before it is turned into a [`Sample`].
pub(crate) struct RawSample {
    pub(crate) offset: Duration,
    pub(crate) depth: u8,
    pub(crate) frames: [u64; MAX_STACK_DEPTH],
}

/// Keeps the newest `capacity` samples and counts the ones it overwrote.
pub(crate) struct Ring {
    slots: Vec<RawSample>,
    capacity: usize,
    next: usize,
    total: u64,
}

impl Ring {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            slots: Vec::with_capacity(capacity),
            capacity,
            next: 0,
            total: 0,
        }
    }

    pub(crate) fn push(&mut self, sample: RawSample) {
        self.total += 1;
        if self.slots.len() < self.capacity {
            self.slots.push(sample);
        } else {
            self.slots[self.next] = sample;
            self.next = (self.next + 1) % self.capacity;
        }
    }

    /// Returns the retained samples oldest first, and how many were dropped.
    pub(crate) fn finish(self) -> (Vec<Sample>, u64) {
        let len = self.slots.len();
        let dropped = self.total - len as u64;
        let mut ordered = Vec::with_capacity(len);
        if len == self.capacity {
            ordered.extend(self.slots[self.next..].iter());
            ordered.extend(self.slots[..self.next].iter());
        } else {
            ordered.extend(self.slots.iter());
        }
        let samples = ordered
            .into_iter()
            .map(|raw| Sample {
                offset: raw.offset,
                stack: raw.frames[..usize::from(raw.depth)].to_vec(),
            })
            .collect();
        (samples, dropped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(offset_ms: u64) -> RawSample {
        RawSample {
            offset: Duration::from_millis(offset_ms),
            depth: 1,
            frames: [0x1000; MAX_STACK_DEPTH],
        }
    }

    #[test]
    fn ring_keeps_the_newest_samples_in_order() {
        let mut ring = Ring::new(3);
        for offset in 0..5 {
            ring.push(raw(offset));
        }
        let (samples, dropped) = ring.finish();
        assert_eq!(dropped, 2);
        assert_eq!(
            samples.iter().map(|s| s.offset).collect::<Vec<_>>(),
            vec![
                Duration::from_millis(2),
                Duration::from_millis(3),
                Duration::from_millis(4)
            ]
        );
    }

    #[test]
    fn ring_does_not_report_drops_before_it_is_full() {
        let mut ring = Ring::new(4);
        ring.push(raw(0));
        ring.push(raw(1));
        let (samples, dropped) = ring.finish();
        assert_eq!(samples.len(), 2);
        assert_eq!(dropped, 0);
    }
}
