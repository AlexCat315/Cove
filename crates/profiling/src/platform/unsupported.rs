//! Honest gap reports for platforms without a sampler in this milestone.

use crate::sampler::Collected;
use crate::{PlatformSupport, SampleConfig, SamplingError, SymbolizationError, SymbolizedFrame};

/// Why the sampler is missing here.
pub(crate) fn reason() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "the Linux CPU sampler is not implemented in this milestone; use an external profiler such as perf until it lands"
    }
    #[cfg(target_os = "windows")]
    {
        "the Windows CPU sampler is not implemented in this milestone; use an external profiler such as ETW/WPA until it lands"
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        "no in-process CPU sampler is implemented for this platform yet"
    }
}

/// Why symbolization is missing here.
pub(crate) fn symbolization_reason() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "the Linux image and DWARF lookup is not implemented in this milestone; use addr2line or eu-addr2line on a profile until it lands"
    }
    #[cfg(target_os = "windows")]
    {
        "the Windows PDB/DWARF lookup is not implemented in this milestone; use a PDB-aware symbolizer until it lands"
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        "no loaded-image symbol lookup is implemented for this platform yet"
    }
}

pub(crate) fn support() -> PlatformSupport {
    PlatformSupport::Unsupported { reason: reason() }
}

pub(crate) fn symbolizer() -> Result<Symbolizer, SymbolizationError> {
    Err(SymbolizationError::Unsupported {
        platform: std::env::consts::OS,
        reason: symbolization_reason(),
    })
}

/// A sampler that cannot exist: [`Sampler::start`] always reports the gap.
pub(crate) enum Sampler {}

impl Sampler {
    pub(crate) fn start(_thread_name: &str, _config: &SampleConfig) -> Result<Self, SamplingError> {
        Err(SamplingError::Unsupported {
            platform: std::env::consts::OS,
            reason: reason(),
        })
    }

    pub(crate) fn samples_taken(&self) -> u64 {
        match *self {}
    }

    pub(crate) fn stop(&mut self) -> Result<Collected, SamplingError> {
        match *self {}
    }
}

/// A symbolizer that cannot exist: [`super::symbolizer`] always errors.
pub(crate) enum Symbolizer {}

impl Symbolizer {
    pub(crate) fn resolve(&mut self, _address: u64) -> SymbolizedFrame {
        match *self {}
    }
}
