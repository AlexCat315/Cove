//! One sampler backend per platform; no backend fabricates samples.

mod fp;
mod ring;

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

#[cfg(all(
    any(target_os = "linux", target_os = "android"),
    any(target_arch = "aarch64", target_arch = "x86_64")
))]
mod linux;

#[cfg(all(
    any(target_os = "linux", target_os = "android"),
    any(target_arch = "aarch64", target_arch = "x86_64")
))]
pub(crate) use linux::{Sampler, Symbolizer, support, symbolizer};

#[cfg(all(
    target_os = "windows",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]
mod windows;

#[cfg(all(
    target_os = "windows",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]
pub(crate) use windows::{Sampler, Symbolizer, support, symbolizer};

#[cfg(not(any(
    all(
        target_os = "macos",
        any(target_arch = "aarch64", target_arch = "x86_64")
    ),
    all(
        any(target_os = "linux", target_os = "android"),
        any(target_arch = "aarch64", target_arch = "x86_64")
    ),
    all(
        target_os = "windows",
        any(target_arch = "aarch64", target_arch = "x86_64")
    )
)))]
mod unsupported;

#[cfg(not(any(
    all(
        target_os = "macos",
        any(target_arch = "aarch64", target_arch = "x86_64")
    ),
    all(
        any(target_os = "linux", target_os = "android"),
        any(target_arch = "aarch64", target_arch = "x86_64")
    ),
    all(
        target_os = "windows",
        any(target_arch = "aarch64", target_arch = "x86_64")
    )
)))]
pub(crate) use unsupported::{Sampler, Symbolizer, support, symbolizer};
