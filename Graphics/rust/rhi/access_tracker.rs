//! Translation of NativeRHI/Validation/AccessTracker.swift: orthogonal
//! access-dependency (hazard) tracking.
//!
//! This layer is deliberately separate from `StateTracker`:
//!   * `StateTracker` answers "what state / image layout is a resource in?" and
//!     emits a transition barrier when the *state* changes (layout, binding
//!     kind).
//!   * `AccessTracker` answers "which prior memory accesses must complete before
//!     this one?" and emits an ordering dependency (RAW / WAR / WAW) even when
//!     the state is unchanged — e.g. two consecutive storage writes, or a write
//!     followed by a read of the same unordered-access resource.
//!
//! The `SubmissionPlanner` runs both; state transitions and access ordering are
//! orthogonal. Backends that order within an encoder (Metal tracked resources)
//! may consume the ordering as a no-op; backends that need explicit barriers
//! (Vulkan) map it to a pipeline barrier; cross-queue hazards ride the
//! release/acquire timeline path.
use std::collections::HashMap;

use super::{QueueClass, ResourceRef, ShaderStage};

// MARK: - Access vocabulary

/// Read or write.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AccessKind {
    Read,
    Write,
}

crate::rhi_option_set!(AccessStage, u8, {
    const VERTEX = 1 << 0;
    const FRAGMENT = 1 << 1;
    const COMPUTE = 1 << 2;
    const TRANSFER = 1 << 3;
    const TASK = 1 << 4;
    const MESH = 1 << 5;
});

impl AccessStage {
    /// Maps a shader stage to the matching access stage.
    pub const fn of(stage: ShaderStage) -> Self {
        match stage {
            ShaderStage::Vertex => Self::VERTEX,
            ShaderStage::Fragment => Self::FRAGMENT,
            ShaderStage::Compute => Self::COMPUTE,
            ShaderStage::Task => Self::TASK,
            ShaderStage::Mesh => Self::MESH,
        }
    }

    /// Whether this set covers every stage in `other`.
    pub const fn is_superset_of(self, other: Self) -> bool {
        self.contains(other)
    }
}
impl From<ShaderStage> for AccessStage {
    fn from(stage: ShaderStage) -> Self {
        Self::of(stage)
    }
}

/// A half-open byte interval within a buffer. [`BufferRange::WHOLE`] (or an
/// absent range on an access) spans the entire resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BufferRange {
    pub offset: usize,
    /// `usize::MAX` means "to the end of the resource".
    pub size: usize,
}
impl BufferRange {
    pub const WHOLE: Self = Self { offset: 0, size: usize::MAX };

    pub const fn new(offset: usize, size: usize) -> Self {
        Self { offset, size }
    }

    const fn is_whole(self) -> bool {
        self.size == usize::MAX
    }

    /// Whether two half-open intervals overlap.
    pub fn overlaps(self, other: Self) -> bool {
        match (self.is_whole(), other.is_whole()) {
            (true, true) => true,
            (true, false) => other.offset >= self.offset,
            (false, true) => self.offset >= other.offset,
            (false, false) => {
                let (Some(self_end), Some(other_end)) =
                    (self.offset.checked_add(self.size), other.offset.checked_add(other.size))
                else {
                    // A range that overflows the address space covers everything
                    // above its offset, so treat it as overlapping.
                    return true;
                };
                self_end > other.offset && other_end > self.offset
            }
        }
    }
}
impl Default for BufferRange {
    fn default() -> Self {
        Self::WHOLE
    }
}

/// One observed access to a resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceAccess {
    pub resource: ResourceRef,
    pub kind: AccessKind,
    pub stage: AccessStage,
    pub range: Option<BufferRange>,
    /// True for color/depth attachment writes. These are ordered by the render
    /// pass boundary (load/store), so a same-state attachment→attachment hazard
    /// does not require an extra barrier. Storage/unordered accesses are not
    /// attachment-ordered and still need explicit ordering.
    pub is_attachment: bool,
}
impl ResourceAccess {
    pub const fn new(resource: ResourceRef, kind: AccessKind, stage: AccessStage) -> Self {
        Self { resource, kind, stage, range: None, is_attachment: false }
    }
    pub const fn read(resource: ResourceRef, stage: AccessStage) -> Self {
        Self::new(resource, AccessKind::Read, stage)
    }
    pub const fn write(resource: ResourceRef, stage: AccessStage) -> Self {
        Self::new(resource, AccessKind::Write, stage)
    }
    pub const fn with_range(mut self, range: BufferRange) -> Self {
        self.range = Some(range);
        self
    }
    pub const fn as_attachment(mut self) -> Self {
        self.is_attachment = true;
        self
    }
}

// MARK: - Hazard

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HazardKind {
    /// RAW.
    ReadAfterWrite,
    /// WAR.
    WriteAfterRead,
    /// WAW.
    WriteAfterWrite,
}

/// An ordering dependency between a prior access and the current access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hazard {
    pub resource: ResourceRef,
    pub kind: HazardKind,
    pub source_stage: AccessStage,
    pub destination_stage: AccessStage,
    pub source_queue: QueueClass,
    pub destination_queue: QueueClass,
    pub range: Option<BufferRange>,
}

// MARK: - Tracker

#[derive(Clone, Copy, Debug)]
struct Record {
    stage: AccessStage,
    queue: QueueClass,
    range: Option<BufferRange>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct ReadKey {
    queue: QueueClass,
    range: BufferRange,
}

#[derive(Clone, Debug, Default)]
struct Memory {
    last_write: Option<Record>,
    /// Repeated read-only frames retain one dependency per queue/range,
    /// unioning stages rather than accumulating the entire access history.
    reads: HashMap<ReadKey, AccessStage>,
}

/// Per-resource active access window: the last write plus the reads since it.
#[derive(Clone, Debug, Default)]
pub struct AccessTracker {
    memories: HashMap<u64, Memory>,
}
impl AccessTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.memories.clear();
    }

    pub fn remove_resource(&mut self, resource: ResourceRef) {
        self.memories.remove(&resource.tracker_key());
    }

    pub fn tracked_resource_count(&self) -> usize {
        self.memories.len()
    }

    /// Records an access and returns the hazards against prior overlapping
    /// accesses. The ordering window is then pruned: a new write clears the
    /// reads and becomes the new last write.
    pub fn observe(&mut self, access: ResourceAccess, queue: QueueClass) -> Vec<Hazard> {
        let key = access.resource.tracker_key();
        let mut memory = self.memories.remove(&key).unwrap_or_default();
        let mut hazards: Vec<Hazard> = Vec::new();
        let range = access.range.unwrap_or(BufferRange::WHOLE);

        match access.kind {
            AccessKind::Read => {
                let read_key = ReadKey { queue, range };
                // The first read ordered the last write for this queue/range and
                // these stages. Later covered reads reuse that dependency. A new
                // write clears the window; new queues, ranges or stages still
                // require their own RAW dependency.
                if let Some(previous) = memory.reads.get(&read_key) {
                    if previous.is_superset_of(access.stage) {
                        self.memories.insert(key, memory);
                        return Vec::new();
                    }
                }
                if let Some(write) = memory.last_write {
                    if overlaps(write.range, range) {
                        hazards.push(Hazard {
                            resource: access.resource,
                            kind: HazardKind::ReadAfterWrite,
                            source_stage: write.stage,
                            destination_stage: access.stage,
                            source_queue: write.queue,
                            destination_queue: queue,
                            range: access.range,
                        });
                    }
                }
                memory.reads.entry(read_key).or_default().insert(access.stage);
            }
            AccessKind::Write => {
                if let Some(write) = memory.last_write {
                    if overlaps(write.range, range) {
                        hazards.push(Hazard {
                            resource: access.resource,
                            kind: HazardKind::WriteAfterWrite,
                            source_stage: write.stage,
                            destination_stage: access.stage,
                            source_queue: write.queue,
                            destination_queue: queue,
                            range: access.range,
                        });
                    }
                }
                for (read_key, stage) in &memory.reads {
                    if read_key.range.overlaps(range) {
                        hazards.push(Hazard {
                            resource: access.resource,
                            kind: HazardKind::WriteAfterRead,
                            source_stage: *stage,
                            destination_stage: access.stage,
                            source_queue: read_key.queue,
                            destination_queue: queue,
                            range: access.range,
                        });
                    }
                }
                memory.last_write = Some(Record { stage: access.stage, queue, range: access.range });
                memory.reads.clear();
            }
        }

        self.memories.insert(key, memory);
        hazards
    }
}

fn overlaps(prior: Option<BufferRange>, current: BufferRange) -> bool {
    prior.unwrap_or(BufferRange::WHOLE).overlaps(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rhi::ResourceKind;

    #[test]
    fn whole_ranges_overlap_everything() {
        assert!(BufferRange::WHOLE.overlaps(BufferRange::new(100, 4)));
        assert!(BufferRange::new(100, 4).overlaps(BufferRange::WHOLE));
        assert!(BufferRange::WHOLE.overlaps(BufferRange::WHOLE));
        assert!(BufferRange::new(0, 8).overlaps(BufferRange::new(7, 4)));
        assert!(!BufferRange::new(0, 8).overlaps(BufferRange::new(8, 4)));
    }

    #[test]
    fn a_write_after_a_read_is_a_hazard_and_prunes_reads() {
        let mut tracker = AccessTracker::new();
        let resource = ResourceRef::new(ResourceKind::Buffer, 1);
        let read = ResourceAccess::read(resource, AccessStage::COMPUTE);
        assert!(tracker.observe(read, QueueClass::Compute).is_empty());

        let write = ResourceAccess::write(resource, AccessStage::COMPUTE);
        let hazards = tracker.observe(write, QueueClass::Compute);
        assert_eq!(hazards.len(), 1);
        assert_eq!(hazards[0].kind, HazardKind::WriteAfterRead);

        // The write cleared the window, so a second write is a WAW.
        let second = tracker.observe(write, QueueClass::Compute);
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].kind, HazardKind::WriteAfterWrite);
    }

    #[test]
    fn covered_reads_do_not_repeat_the_raw_dependency() {
        let mut tracker = AccessTracker::new();
        let resource = ResourceRef::new(ResourceKind::Buffer, 2);
        tracker.observe(
            ResourceAccess::write(resource, AccessStage::TRANSFER),
            QueueClass::Transfer,
        );
        let read = ResourceAccess::read(resource, AccessStage::VERTEX);
        assert_eq!(tracker.observe(read, QueueClass::Graphics).len(), 1);
        // Same queue/range and a subset of stages: already ordered.
        assert!(tracker.observe(read, QueueClass::Graphics).is_empty());
        // A new stage in the same window still needs its own dependency.
        let wider = ResourceAccess::read(resource, AccessStage::VERTEX | AccessStage::FRAGMENT);
        assert_eq!(tracker.observe(wider, QueueClass::Graphics).len(), 1);
    }

    #[test]
    fn disjoint_ranges_do_not_conflict() {
        let mut tracker = AccessTracker::new();
        let resource = ResourceRef::new(ResourceKind::Buffer, 3);
        tracker.observe(
            ResourceAccess::write(resource, AccessStage::COMPUTE)
                .with_range(BufferRange::new(0, 16)),
            QueueClass::Compute,
        );
        let disjoint = ResourceAccess::write(resource, AccessStage::COMPUTE)
            .with_range(BufferRange::new(16, 16));
        assert!(tracker.observe(disjoint, QueueClass::Compute).is_empty());
        tracker.remove_resource(resource);
        assert_eq!(tracker.tracked_resource_count(), 0);
    }
}
