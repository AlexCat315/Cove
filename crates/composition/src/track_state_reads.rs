//! Translation of CoveUIComposeCore/TrackStateReads.swift.
//!
//! `#[track_state_reads(field, ...)]` on an `impl View` rewrites direct
//! `self.field.get().member` reads to `State::read_field`, exposes those state
//! members to the composition scope, and opts the view into precise tracking.
use crate::View;

/// Marker emitted by [`crate::track_state_reads`] for an instrumented view.
pub trait StateReadTrackedView: View {}
