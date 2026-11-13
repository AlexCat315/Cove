//! Samples, profiles and their symbolized form.

use std::time::Duration;

use crate::Symbolizer;

/// One stack captured from the sampled thread, innermost frame first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sample {
    /// Time since the sampler started.
    pub offset: Duration,
    /// Instruction addresses. Empty only if the stack could not be read at all.
    pub stack: Vec<u64>,
}

/// Raw, unsymbolized result of a sampling run.
///
/// `samples` holds the newest `min(samples_taken, max_samples)` samples in
/// chronological order; `samples_dropped` counts the ones the ring overwrote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampledProfile {
    /// Thread the sampler followed.
    pub thread_name: String,
    /// Interval requested in [`SampleConfig`](crate::SampleConfig).
    pub interval: Duration,
    /// Wall time the sampler thread was actually running.
    pub duration: Duration,
    /// Every sample captured, including ones later overwritten.
    pub samples_taken: u64,
    /// Samples overwritten by the bounded ring.
    pub samples_dropped: u64,
    /// The retained samples, oldest first.
    pub samples: Vec<Sample>,
}

impl SampledProfile {
    /// Observed samples per second, or `None` when the run was too short.
    pub fn samples_per_second(&self) -> Option<f64> {
        let seconds = self.duration.as_secs_f64();
        (seconds > 0.0).then(|| self.samples_taken as f64 / seconds)
    }

    /// Resolves every retained frame through `symbolizer`.
    pub fn symbolize(&self, symbolizer: &mut Symbolizer) -> SymbolizedProfile {
        SymbolizedProfile {
            thread_name: self.thread_name.clone(),
            interval: self.interval,
            duration: self.duration,
            samples_taken: self.samples_taken,
            samples_dropped: self.samples_dropped,
            samples: self
                .samples
                .iter()
                .map(|sample| SymbolizedSample {
                    offset: sample.offset,
                    frames: symbolizer.resolve_stack(&sample.stack),
                })
                .collect(),
        }
    }
}

/// Source position for a resolved frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceLocation {
    /// File path as recorded by the compiler.
    pub file: String,
    /// One-based line, when the debug information has line tables.
    pub line: Option<u32>,
}

/// An instruction address with whatever symbols could be resolved for it.
///
/// `function` stays `None` when neither DWARF nor the dynamic symbol table
/// knows the address — for example inside the dyld shared cache. The address
/// is still the real sampled one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolizedFrame {
    /// Normalized instruction address.
    pub address: u64,
    /// Demangled function name when available.
    pub function: Option<String>,
    /// DWARF file and line when available.
    pub location: Option<SourceLocation>,
}

impl SymbolizedFrame {
    /// `function file:line`, `function file` or `0xaddress`, in that order of
    /// detail.
    pub fn label(&self) -> String {
        let location = self.location.as_ref().map(|location| match location.line {
            Some(line) => format!("{}:{}", location.file, line),
            None => location.file.clone(),
        });
        match (&self.function, location) {
            (Some(function), Some(location)) => format!("{function} {location}"),
            (Some(function), None) => function.clone(),
            (None, Some(location)) => location,
            (None, None) => format!("{:#x}", self.address),
        }
    }
}

/// One symbolized stack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolizedSample {
    /// Time since the sampler started.
    pub offset: Duration,
    /// Innermost frame first.
    pub frames: Vec<SymbolizedFrame>,
}

/// A sampled profile whose retained stacks have been resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolizedProfile {
    /// Thread the sampler followed.
    pub thread_name: String,
    /// Interval requested in [`SampleConfig`](crate::SampleConfig).
    pub interval: Duration,
    /// Wall time the sampler thread was actually running.
    pub duration: Duration,
    /// Every sample captured, including ones later overwritten.
    pub samples_taken: u64,
    /// Samples overwritten by the bounded ring.
    pub samples_dropped: u64,
    /// The retained samples, oldest first.
    pub samples: Vec<SymbolizedSample>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(function: Option<&str>, file: Option<&str>, line: Option<u32>) -> SymbolizedFrame {
        SymbolizedFrame {
            address: 0x1000,
            function: function.map(str::to_owned),
            location: file.map(|file| SourceLocation {
                file: file.to_owned(),
                line,
            }),
        }
    }

    #[test]
    fn label_uses_every_available_detail() {
        assert_eq!(
            frame(Some("app::run"), Some("src/main.rs"), Some(12)).label(),
            "app::run src/main.rs:12"
        );
        assert_eq!(
            frame(Some("app::run"), Some("src/main.rs"), None).label(),
            "app::run src/main.rs"
        );
        assert_eq!(frame(Some("app::run"), None, None).label(), "app::run");
        assert_eq!(frame(None, None, None).label(), "0x1000");
    }

    #[test]
    fn samples_per_second_is_absent_for_empty_runs() {
        let profile = SampledProfile {
            thread_name: "worker".into(),
            interval: Duration::from_millis(1),
            duration: Duration::ZERO,
            samples_taken: 0,
            samples_dropped: 0,
            samples: Vec::new(),
        };
        assert_eq!(profile.samples_per_second(), None);
    }
}
