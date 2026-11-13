//! Turns sampled instruction addresses into function and file:line symbols.

use crate::{SymbolizationError, SymbolizedFrame};

/// Resolves instruction addresses from the current process.
///
/// On macOS this reads the loaded image list from dyld and looks addresses up
/// in the on-disk Mach-O (including split-DWARF object files and dSYM
/// bundles) with `addr2line`. On Linux and Android it reads the executable
/// mappings from `/proc/self/maps`, derives each ELF image's load bias and
/// resolves with `addr2line`, falling back to `dladdr`. On Windows it resolves
/// through dbghelp, which reads PDB and DWARF alike. Results are cached per
/// address.
///
/// Targets without a backend return [`SymbolizationError::Unsupported`] from
/// [`Symbolizer::new`]; no symbol names are invented.
pub struct Symbolizer {
    inner: crate::platform::Symbolizer,
}

impl Symbolizer {
    /// Creates a symbolizer over the images loaded right now. Images loaded
    /// later are picked up when an address cannot be attributed.
    pub fn new() -> Result<Self, SymbolizationError> {
        Ok(Self {
            inner: crate::platform::symbolizer()?,
        })
    }

    /// Resolves one address. Unknown images or missing debug information yield
    /// a frame with no `function` and no `location`.
    pub fn resolve(&mut self, address: u64) -> SymbolizedFrame {
        self.inner.resolve(address)
    }

    /// Resolves a stack, preserving its order.
    pub fn resolve_stack(&mut self, addresses: &[u64]) -> Vec<SymbolizedFrame> {
        addresses
            .iter()
            .map(|address| self.inner.resolve(*address))
            .collect()
    }
}

impl std::fmt::Debug for Symbolizer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Symbolizer").finish_non_exhaustive()
    }
}
