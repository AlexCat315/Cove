//! Sample statistics kernel: one pass for minimum, maximum and sum of squares.
//!
//! The waveform and meter paths call it over every decoded sample, so the
//! 4-lane body is hand-written for AArch64 (NEON) and x86-64 (SSE). Other
//! targets and the sub-block tail use the scalar loop.

/// Minimum, maximum and sum of squares of a sample run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SampleStats {
    pub min: f32,
    pub max: f32,
    pub sum_squares: f32,
    pub count: usize,
}

impl SampleStats {
    pub const EMPTY: Self = Self {
        min: 0.0,
        max: 0.0,
        sum_squares: 0.0,
        count: 0,
    };

    /// Root mean square, or zero for an empty run.
    pub fn rms(&self) -> f32 {
        if self.count == 0 {
            0.0
        } else {
            (self.sum_squares / self.count as f32).sqrt()
        }
    }

    /// Largest absolute sample.
    pub fn peak(&self) -> f32 {
        self.min.abs().max(self.max.abs())
    }

    fn merge(self, other: Self) -> Self {
        if self.count == 0 {
            return other;
        }
        if other.count == 0 {
            return self;
        }
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
            sum_squares: self.sum_squares + other.sum_squares,
            count: self.count + other.count,
        }
    }
}

/// Statistics of a contiguous run of samples.
pub fn sample_stats(samples: &[f32]) -> SampleStats {
    let blocks = samples.len() / 4;
    let head = if blocks > 0 {
        vector_stats(&samples[..blocks * 4])
    } else {
        SampleStats::EMPTY
    };
    head.merge(scalar_stats(&samples[blocks * 4..]))
}

/// Per-channel statistics of interleaved samples.
pub fn channel_stats(interleaved: &[f32], channels: usize) -> Vec<SampleStats> {
    let channels = channels.max(1);
    if channels == 1 {
        return vec![sample_stats(interleaved)];
    }
    let mut stats = vec![SampleStats::EMPTY; channels];
    for frame in interleaved.chunks_exact(channels) {
        for (slot, &sample) in stats.iter_mut().zip(frame) {
            *slot = slot.merge(SampleStats {
                min: sample,
                max: sample,
                sum_squares: sample * sample,
                count: 1,
            });
        }
    }
    stats
}

pub(crate) fn scalar_stats(samples: &[f32]) -> SampleStats {
    let Some((&first, rest)) = samples.split_first() else {
        return SampleStats::EMPTY;
    };
    let mut stats = SampleStats {
        min: first,
        max: first,
        sum_squares: first * first,
        count: samples.len(),
    };
    for &sample in rest {
        stats.min = stats.min.min(sample);
        stats.max = stats.max.max(sample);
        stats.sum_squares += sample * sample;
    }
    stats
}

/// `samples.len()` is a positive multiple of four.
#[cfg(target_arch = "aarch64")]
fn vector_stats(samples: &[f32]) -> SampleStats {
    debug_assert!(!samples.is_empty() && samples.len().is_multiple_of(4));
    let (min, max, sum): (f32, f32, f32);
    // SAFETY: the pointer covers `blocks * 16` readable bytes of the borrowed slice,
    // the loop reads exactly that many, and only the declared registers are written.
    unsafe {
        std::arch::asm!(
            "ld1 {{{vmin:v}.4s}}, [{ptr}], #16",
            "mov {vmax:v}.16b, {vmin:v}.16b",
            "fmul {vsum:v}.4s, {vmin:v}.4s, {vmin:v}.4s",
            "subs {n}, {n}, #1",
            "b.eq 3f",
            "2:",
            "ld1 {{{t:v}.4s}}, [{ptr}], #16",
            "fmin {vmin:v}.4s, {vmin:v}.4s, {t:v}.4s",
            "fmax {vmax:v}.4s, {vmax:v}.4s, {t:v}.4s",
            "fmla {vsum:v}.4s, {t:v}.4s, {t:v}.4s",
            "subs {n}, {n}, #1",
            "b.ne 2b",
            "3:",
            "fminv {vmin:s}, {vmin:v}.4s",
            "fmaxv {vmax:s}, {vmax:v}.4s",
            "faddp {vsum:v}.4s, {vsum:v}.4s, {vsum:v}.4s",
            "faddp {vsum:s}, {vsum:v}.2s",
            ptr = inout(reg) samples.as_ptr() => _,
            n = inout(reg) samples.len() / 4 => _,
            vmin = out(vreg) min,
            vmax = out(vreg) max,
            vsum = out(vreg) sum,
            t = out(vreg) _,
            options(nostack, readonly),
        );
    }
    SampleStats {
        min,
        max,
        sum_squares: sum,
        count: samples.len(),
    }
}

/// `samples.len()` is a positive multiple of four.
#[cfg(target_arch = "x86_64")]
fn vector_stats(samples: &[f32]) -> SampleStats {
    debug_assert!(!samples.is_empty() && samples.len().is_multiple_of(4));
    let (min, max, sum): (f32, f32, f32);
    // SAFETY: SSE is part of the x86-64 baseline; the pointer covers `blocks * 16`
    // readable bytes of the borrowed slice and the loop reads exactly that many.
    unsafe {
        std::arch::asm!(
            "movups {vmin}, xmmword ptr [{ptr}]",
            "movaps {vmax}, {vmin}",
            "movaps {vsum}, {vmin}",
            "mulps {vsum}, {vsum}",
            "add {ptr}, 16",
            "dec {n}",
            "jz 3f",
            "2:",
            "movups {t}, xmmword ptr [{ptr}]",
            "minps {vmin}, {t}",
            "maxps {vmax}, {t}",
            "mulps {t}, {t}",
            "addps {vsum}, {t}",
            "add {ptr}, 16",
            "dec {n}",
            "jnz 2b",
            "3:",
            "movhlps {t}, {vmin}",
            "minps {vmin}, {t}",
            "movaps {t}, {vmin}",
            "shufps {t}, {t}, 0x55",
            "minss {vmin}, {t}",
            "movhlps {t}, {vmax}",
            "maxps {vmax}, {t}",
            "movaps {t}, {vmax}",
            "shufps {t}, {t}, 0x55",
            "maxss {vmax}, {t}",
            "movhlps {t}, {vsum}",
            "addps {vsum}, {t}",
            "movaps {t}, {vsum}",
            "shufps {t}, {t}, 0x55",
            "addss {vsum}, {t}",
            ptr = inout(reg) samples.as_ptr() => _,
            n = inout(reg) samples.len() / 4 => _,
            vmin = out(xmm_reg) min,
            vmax = out(xmm_reg) max,
            vsum = out(xmm_reg) sum,
            t = out(xmm_reg) _,
            options(nostack, readonly),
        );
    }
    SampleStats {
        min,
        max,
        sum_squares: sum,
        count: samples.len(),
    }
}

#[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
fn vector_stats(samples: &[f32]) -> SampleStats {
    scalar_stats(samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_and_scalar_paths_agree() {
        for length in [0usize, 1, 3, 4, 5, 8, 15, 64, 1023] {
            let samples: Vec<f32> = (0..length)
                .map(|i| ((i as f32) * 0.37).sin() * 0.8 - 0.1)
                .collect();
            let fast = sample_stats(&samples);
            let slow = scalar_stats(&samples);
            assert_eq!(fast.count, slow.count);
            assert_eq!(fast.min, slow.min, "min for {length}");
            assert_eq!(fast.max, slow.max, "max for {length}");
            assert!(
                (fast.sum_squares - slow.sum_squares).abs() <= 1e-3 * slow.sum_squares.max(1.0),
                "sum for {length}: {} vs {}",
                fast.sum_squares,
                slow.sum_squares
            );
        }
    }

    #[test]
    fn interleaved_channels_are_measured_separately() {
        let samples = [0.5, -0.25, -0.5, 0.25, 0.5, -0.25];
        let stats = channel_stats(&samples, 2);
        assert_eq!(stats[0].peak(), 0.5);
        assert_eq!(stats[1].peak(), 0.25);
        assert!((stats[0].rms() - 0.5).abs() < 1e-6);
    }
}
