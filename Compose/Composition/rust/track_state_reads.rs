//! Translation of CoveUIComposeCore/TrackStateReads.swift.
//!
//! Swift synthesizes `_coveTrackedBody` with a member macro so dependencies can
//! be inferred from a view body at compile time. Rust has no attached macros on
//! stable toolchains, so a view opts in by implementing [`StateReadTrackedView`]
//! directly. The runtime boundary is identical: recomposition reads the tracked
//! body and `View::precise_state_reads` reports `true` for the scope.
use crate::{AnyView, View};

/// Runtime entry point corresponding to the macro-synthesized protocol.
///
/// Implementors also override `View::precise_state_reads` to return `true`,
/// mirroring the Swift conformance check.
pub trait StateReadTrackedView: View {
    /// The body expression whose state reads are tracked per field.
    fn cove_tracked_body(&self) -> AnyView;
}
