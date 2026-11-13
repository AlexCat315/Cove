//! One sampler backend per platform; no backend fabricates samples.

#[cfg(all(
    target_os = "macos",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]
mod macos;

#[cfg(all(
    target_os = "macos",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]
pub(crate) use macos::{Sampler, Symbolizer, support, symbolizer};

#[cfg(not(all(
    target_os = "macos",
    any(target_arch = "aarch64", target_arch = "x86_64")
)))]
mod unsupported;

#[cfg(not(all(
    target_os = "macos",
    any(target_arch = "aarch64", target_arch = "x86_64")
)))]
pub(crate) use unsupported::{Sampler, Symbolizer, support, symbolizer};
