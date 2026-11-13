//! Public sampler handle; delegates the platform work to one backend.

use std::time::{Duration, Instant};

use crate::platform;
use crate::{Sample, SampleConfig, SampledProfile, SamplingError};

/// Collected by a backend when its sampler thread finishes.
pub(crate) struct Collected {
    pub(crate) duration: Duration,
    pub(crate) samples_taken: u64,
    pub(crate) samples_dropped: u64,
    pub(crate) samples: Vec<Sample>,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub(crate) failure: Option<SamplingError>,
}

/// Samples one named thread of the current process at a bounded rate.
///
/// The sampler runs on its own thread. Call [`stop`](Self::stop) to obtain the
/// profile; dropping the handle stops and joins the thread as well.
pub struct CpuSampler {
    thread_name: String,
    config: SampleConfig,
    started: Instant,
    inner: Option<platform::Sampler>,
}

impl CpuSampler {
    /// Starts following `thread_name`.
    ///
    /// Must be called from a different thread than the target. Fails with
    /// [`SamplingError::Unsupported`] where no real sampler exists.
    pub fn start(thread_name: &str, config: SampleConfig) -> Result<Self, SamplingError> {
        if thread_name.is_empty() {
            return Err(SamplingError::InvalidConfig(
                "thread name must not be empty",
            ));
        }
        config.validate()?;
        let inner = platform::Sampler::start(thread_name, &config)?;
        Ok(Self {
            thread_name: thread_name.to_owned(),
            config,
            started: Instant::now(),
            inner: Some(inner),
        })
    }

    /// Name passed to [`start`](Self::start).
    pub fn thread_name(&self) -> &str {
        &self.thread_name
    }

    /// Options this run started with.
    pub fn config(&self) -> &SampleConfig {
        &self.config
    }

    /// Every sample captured so far, including those the ring will overwrite.
    pub fn samples_taken(&self) -> u64 {
        self.inner
            .as_ref()
            .map_or(0, platform::Sampler::samples_taken)
    }

    /// Wall time since the sampler thread was started.
    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    /// Stops sampling, joins the sampler thread and returns the retained ring.
    pub fn stop(mut self) -> Result<SampledProfile, SamplingError> {
        let Some(mut inner) = self.inner.take() else {
            return Err(SamplingError::SamplerPanicked);
        };
        let collected = inner.stop()?;
        Ok(SampledProfile {
            thread_name: std::mem::take(&mut self.thread_name),
            interval: self.config.interval,
            duration: collected.duration,
            samples_taken: collected.samples_taken,
            samples_dropped: collected.samples_dropped,
            samples: collected.samples,
        })
    }
}

impl Drop for CpuSampler {
    fn drop(&mut self) {
        if let Some(mut inner) = self.inner.take() {
            let _ = inner.stop();
        }
    }
}

impl std::fmt::Debug for CpuSampler {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CpuSampler")
            .field("thread_name", &self.thread_name)
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}
