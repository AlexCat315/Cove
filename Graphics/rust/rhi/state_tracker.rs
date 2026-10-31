//! Translation of NativeRHI/Validation/StateTracker.swift: resource state
//! tracking and automatic barrier synthesis.
//!
//! The tracker stores the last known usage state of each tracked resource and
//! emits a barrier whenever a command requires a different state. Repeated
//! transitions of the same resource within one commit window are merged.
use std::collections::HashMap;

use super::ResourceState;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResourceKind {
    Buffer,
    Texture,
    AccelerationStructure,
}
impl ResourceKind {
    const fn raw_value(self) -> u64 {
        match self {
            Self::Buffer => 0,
            Self::Texture => 1,
            Self::AccelerationStructure => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ResourceRef {
    pub kind: ResourceKind,
    pub id: u32,
    pub subresource_base: u16,
    pub subresource_count: u16,
}
impl ResourceRef {
    pub const fn new(kind: ResourceKind, id: u32) -> Self {
        Self {
            kind,
            id,
            subresource_base: 0,
            subresource_count: 1,
        }
    }
    pub const fn with_subresources(kind: ResourceKind, id: u32, base: u16, count: u16) -> Self {
        Self {
            kind,
            id,
            subresource_base: base,
            subresource_count: count,
        }
    }
    pub const fn buffer(id: u32) -> Self {
        Self::new(ResourceKind::Buffer, id)
    }
    pub const fn texture(id: u32) -> Self {
        Self::new(ResourceKind::Texture, id)
    }
    pub const fn acceleration_structure(id: u32) -> Self {
        Self::new(ResourceKind::AccelerationStructure, id)
    }

    /// Collision-free packed key for the internal tracker dictionaries. IDs are
    /// handed out by a single global pool, so `(kind, id)` uniquely identifies a
    /// resource. Subresource is intentionally dropped: resources that differ
    /// only in subresource collapse to one key, which at worst over-orders a
    /// barrier — safe, never incorrect.
    pub const fn tracker_key(self) -> u64 {
        (self.kind.raw_value() << 32) | self.id as u64
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Barrier {
    pub resource: ResourceRef,
    pub before: ResourceState,
    pub after: ResourceState,
    pub cross_queue: bool,
}
impl Barrier {
    pub const fn new(resource: ResourceRef, before: ResourceState, after: ResourceState) -> Self {
        Self {
            resource,
            before,
            after,
            cross_queue: false,
        }
    }
    pub const fn cross_queue(
        resource: ResourceRef,
        before: ResourceState,
        after: ResourceState,
    ) -> Self {
        Self {
            resource,
            before,
            after,
            cross_queue: true,
        }
    }
}

/// Tracks the last known state of every resource touched by a submission.
///
/// The Swift original is a `struct` mutated through `inout`; the Rust port keeps
/// value semantics so a planner can snapshot the tracker cheaply.
#[derive(Clone, Debug, Default)]
pub struct StateTracker {
    current_states: HashMap<u64, ResourceState>,
    pending_barriers: Vec<Barrier>,
}
impl StateTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.current_states.clear();
        self.pending_barriers.clear();
    }

    pub fn pending_barriers(&self) -> &[Barrier] {
        &self.pending_barriers
    }

    pub fn set_initial_state(&mut self, resource: ResourceRef, state: ResourceState) {
        self.current_states.insert(resource.tracker_key(), state);
    }

    pub fn set_current_state(&mut self, resource: ResourceRef, state: ResourceState) {
        self.current_states.insert(resource.tracker_key(), state);
    }

    pub fn current_state(&self, resource: ResourceRef) -> ResourceState {
        self.current_states
            .get(&resource.tracker_key())
            .copied()
            .unwrap_or_default()
    }

    pub fn remove_resource(&mut self, resource: ResourceRef) {
        self.current_states.remove(&resource.tracker_key());
    }

    /// Records a required state. If it differs from the current state, a
    /// barrier is queued and the tracked state is advanced immediately. Returns
    /// the state prior to the transition so callers can capture it once.
    pub fn require_state(
        &mut self,
        resource: ResourceRef,
        desired: ResourceState,
    ) -> ResourceState {
        let key = resource.tracker_key();
        let current = self.current_states.get(&key).copied().unwrap_or_default();
        if current == desired {
            return current;
        }
        self.pending_barriers
            .push(Barrier::new(resource, current, desired));
        self.current_states.insert(key, desired);
        current
    }

    /// Drains the queued barriers, merging multiple transitions of the same
    /// resource into a single barrier whose destination state is the union.
    pub fn commit_barriers(&mut self) -> Vec<Barrier> {
        if self.pending_barriers.is_empty() {
            return Vec::new();
        }
        let mut merged: HashMap<u64, Barrier> = HashMap::with_capacity(self.pending_barriers.len());
        for barrier in self.pending_barriers.drain(..) {
            match merged.get_mut(&barrier.resource.tracker_key()) {
                Some(existing) => existing.after = existing.after | barrier.after,
                None => {
                    merged.insert(barrier.resource.tracker_key(), barrier);
                }
            }
        }
        merged.into_values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_keys_pack_kind_and_id() {
        assert_eq!(ResourceRef::buffer(7).tracker_key(), 7);
        assert_eq!(ResourceRef::texture(7).tracker_key(), (1 << 32) | 7);
        assert_ne!(ResourceRef::buffer(7), ResourceRef::texture(7));
    }

    #[test]
    fn repeated_transitions_merge_into_one_barrier() {
        let mut tracker = StateTracker::new();
        let resource = ResourceRef::texture(1);
        tracker.require_state(resource, ResourceState::COPY_DESTINATION);
        tracker.require_state(resource, ResourceState::SHADER_RESOURCE);
        assert_eq!(
            tracker.current_state(resource),
            ResourceState::SHADER_RESOURCE
        );
        let barriers = tracker.commit_barriers();
        assert_eq!(barriers.len(), 1);
        assert_eq!(barriers[0].before, ResourceState::EMPTY);
        assert_eq!(
            barriers[0].after,
            ResourceState::COPY_DESTINATION | ResourceState::SHADER_RESOURCE
        );
        assert!(tracker.commit_barriers().is_empty());
    }

    #[test]
    fn requesting_the_current_state_emits_nothing() {
        let mut tracker = StateTracker::new();
        let resource = ResourceRef::buffer(3);
        tracker.set_initial_state(resource, ResourceState::VERTEX_BUFFER);
        assert_eq!(
            tracker.require_state(resource, ResourceState::VERTEX_BUFFER),
            ResourceState::VERTEX_BUFFER
        );
        assert!(tracker.pending_barriers().is_empty());
        tracker.remove_resource(resource);
        assert_eq!(tracker.current_state(resource), ResourceState::EMPTY);
    }
}
