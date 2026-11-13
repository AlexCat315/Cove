//! Bounds and options for one sampling run.

use std::time::Duration;

use crate::SamplingError;

/// Deepest stack a sample can record.
pub const MAX_STACK_DEPTH: usize = 64;
/// Most samples a sampler keeps in its ring, regardless of run length.
pub const MAX_SAMPLES: usize = 65_536;
/// Fastest supported sampling interval.
pub const MIN_SAMPLE_INTERVAL: Duration = Duration::from_micros(100);
/// Slowest supported sampling interval.
pub const MAX_SAMPLE_INTERVAL: Duration = Duration::from_secs(10);

/// Options for [`CpuSampler::start`](crate::CpuSampler::start).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampleConfig {
    /// Nominal time between samples. The sampler sleeps for whatever remains
    /// of this period after a sample, so suspended-thread overhead is included.
    pub interval: Duration,
    /// Fixed ring size. Older samples are overwritten once the ring is full.
    pub max_samples: usize,
    /// Frames recorded per sample; deeper stacks are truncated.
    pub max_stack_depth: usize,
}

impl Default for SampleConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_millis(1),
            // Ten seconds of history at the default interval.
            max_samples: 10_000,
            max_stack_depth: 32,
        }
    }
}

impl SampleConfig {
    /// Rejects intervals and sizes the sampler cannot honour.
    pub fn validate(&self) -> Result<(), SamplingError> {
        if self.interval < MIN_SAMPLE_INTERVAL {
            return Err(SamplingError::InvalidConfig(
                "interval must be at least 100µs",
            ));
        }
        if self.interval > MAX_SAMPLE_INTERVAL {
            return Err(SamplingError::InvalidConfig("interval must be at most 10s"));
        }
        if self.max_samples == 0 || self.max_samples > MAX_SAMPLES {
            return Err(SamplingError::InvalidConfig(
                "max_samples must be between 1 and 65536",
            ));
        }
        if self.max_stack_depth == 0 || self.max_stack_depth > MAX_STACK_DEPTH {
            return Err(SamplingError::InvalidConfig(
                "max_stack_depth must be between 1 and 64",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        SampleConfig::default().validate().expect("default config");
    }

    #[test]
    fn rejects_unusable_bounds() {
        let config = SampleConfig {
            interval: Duration::from_micros(50),
            ..SampleConfig::default()
        };
        assert!(matches!(
            config.validate(),
            Err(SamplingError::InvalidConfig(_))
        ));

        let config = SampleConfig {
            max_samples: 0,
            ..SampleConfig::default()
        };
        assert!(matches!(
            config.validate(),
            Err(SamplingError::InvalidConfig(_))
        ));

        let config = SampleConfig {
            max_stack_depth: MAX_STACK_DEPTH + 1,
            ..SampleConfig::default()
        };
        assert!(matches!(
            config.validate(),
            Err(SamplingError::InvalidConfig(_))
        ));
    }
}
