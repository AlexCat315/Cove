//! Explicit failure modes; unavailable work is reported, never simulated.

/// Why a sampling run could not start or continue.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SamplingError {
    /// The platform, or this build's architecture, has no sampler yet.
    #[error("CPU sampling is not available on {platform}: {reason}")]
    Unsupported {
        /// Operating system reported by `std::env::consts::OS`.
        platform: &'static str,
        /// What is missing and what to use instead.
        reason: &'static str,
    },
    /// No live thread in this process carries the requested name.
    #[error("no thread named {0:?} is running in this process")]
    ThreadNotFound(String),
    /// Sampling the calling thread would deadlock on its own suspension.
    #[error("the sampled thread must be different from the caller")]
    TargetIsCaller,
    /// A configuration value is outside the supported range.
    #[error("invalid sampling configuration: {0}")]
    InvalidConfig(&'static str),
    /// The sampler thread could not be spawned.
    #[error("the sampler thread could not be started: {0}")]
    ThreadSpawn(String),
    /// Thread enumeration failed; the value is a Mach `kern_return_t`.
    #[error("thread discovery failed with Mach error {0}")]
    ThreadDiscovery(i32),
    /// The target thread exited or its Mach port became invalid.
    #[error("the sampled thread became unavailable (Mach error {0})")]
    TargetUnavailable(i32),
    /// The sampler thread panicked; no trustworthy profile exists.
    #[error("the sampler thread panicked")]
    SamplerPanicked,
    /// Another sampler, or the application's own SIGPROF handler, is active.
    #[error("a CPU sampler or SIGPROF handler is already active in this process")]
    SamplerBusy,
    /// The sampling signal could not be installed; the value is an `errno`.
    #[error("installing the CPU sampling signal handler failed with errno {0}")]
    SignalSetup(i32),
}

/// Why symbolization is unavailable.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SymbolizationError {
    /// The platform has no compiled-image lookup in this milestone.
    #[error("symbolization is not available on {platform}: {reason}")]
    Unsupported {
        /// Operating system reported by `std::env::consts::OS`.
        platform: &'static str,
        /// What is missing and what to use instead.
        reason: &'static str,
    },
}
