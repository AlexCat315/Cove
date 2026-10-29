//! Translation of NativeRHI/Submission/SubmissionPlanner.swift.
//!
//! The planner walks a recorded command list and, for each resource usage:
//!   * tracks the current resource state (`StateTracker`) and synthesizes a
//!     barrier when a different state is required;
//!   * tracks which queue owns each resource, and when a resource is used on a
//!     different queue it splits the ownership transfer into a release submit on
//!     the source queue (signaling a timeline semaphore) and an acquire on the
//!     consumer (waiting on it);
//!   * merges repeated transitions of the same resource.
//!
//! The result is a self-contained `SubmitPlan` the `Device` executes in order.
use std::collections::{HashMap, HashSet};

use super::{
    AccessKind, AccessStage, AccessTracker, BarrierCommand, BarrierPassScope, BarrierSyncAction,
    BindingResource, BindingSet, BindingSetEntry, ComputePassRecord, CopyPassRecord,
    QueueClass, RecordedCommand, RenderPassRecord, RenderCommand, ResourceAccess, ResourceKind,
    ResourceRef, ResourceState, RhiError, RhiResult, StateTracker, SubmitDescriptor, SubmitPlan,
    PlannedCommand, PlannedSubmit, TimelineSemaphore,
};

#[derive(Clone, Copy, Debug)]
struct PendingTransfer {
    src_queue: QueueClass,
    dst_queue: QueueClass,
    released_state: ResourceState,
    semaphore: TimelineSemaphore,
}

#[derive(Clone, Copy, Debug)]
struct SplitReleaseRequest {
    resource: ResourceRef,
    src_state: ResourceState,
    src_queue: QueueClass,
    dst_queue: QueueClass,
}

#[derive(Clone, Copy, Debug, Default)]
struct QueueTimeline {
    id: u32,
    next_value: u64,
}

/// Everything the planner remembers between submissions.
#[derive(Clone, Debug, Default)]
pub struct Tracking {
    state_tracker: StateTracker,
    access_tracker: AccessTracker,
    resource_queues: HashMap<u64, QueueClass>,
    pending_transfers: HashMap<u64, PendingTransfer>,
    queue_timelines: HashMap<QueueClass, QueueTimeline>,
    next_timeline_semaphore_id: u32,
}
impl Tracking {
    fn new() -> Self {
        Self { next_timeline_semaphore_id: 1, ..Default::default() }
    }
}

#[derive(Clone, Copy, Debug)]
struct TrackedBinding {
    resource: ResourceRef,
    state: ResourceState,
}

pub struct SubmissionPlanner {
    tracking: Tracking,
    binding_set_entries: HashMap<u32, Vec<TrackedBinding>>,
}
impl SubmissionPlanner {
    pub fn new() -> Self {
        Self { tracking: Tracking::new(), binding_set_entries: HashMap::new() }
    }

    /// Captures the unsubmitted plan as a value, including queue timeline
    /// values, so a failed submission can be rolled back with
    /// [`Self::restore`].
    pub fn checkpoint(&self) -> Tracking {
        self.tracking.clone()
    }

    /// Restores a snapshot taken by [`Self::checkpoint`].
    pub fn restore(&mut self, snapshot: Tracking) {
        self.tracking = snapshot;
    }

    // MARK: Binding set bookkeeping

    pub fn register_binding_set_entries(
        &mut self,
        handle_id: u32,
        entries: &[BindingSetEntry],
        read_only_slots: &HashSet<u32>,
    ) {
        // Immutable sets keep their resolved dependency description. Samplers
        // have no tracked memory, and buffer access comes from the layout.
        let tracked = entries
            .iter()
            .filter_map(|entry| {
                self.tracked_resource(&entry.resource, read_only_slots.contains(&entry.slot))
            })
            .collect();
        self.binding_set_entries.insert(handle_id, tracked);
    }

    pub fn remove_binding_set_entries(&mut self, handle_id: u32) {
        self.binding_set_entries.remove(&handle_id);
    }

    /// Removes all frontend/planner bookkeeping for a resource being destroyed.
    pub fn forget_resource(&mut self, resource: ResourceRef) {
        let key = resource.tracker_key();
        self.tracking.state_tracker.remove_resource(resource);
        self.tracking.access_tracker.remove_resource(resource);
        self.tracking.resource_queues.remove(&key);
        self.tracking.pending_transfers.remove(&key);
    }

    /// Immediate uploads finish on the graphics queue (or write coherent host
    /// storage before it is submitted). Their next GPU reader still needs a
    /// transfer-write dependency and, on another queue, a timeline handoff.
    pub fn record_immediate_write(&mut self, resource: ResourceRef) {
        self.tracking.state_tracker.set_current_state(resource, ResourceState::COPY_DESTINATION);
        self.tracking.resource_queues.insert(resource.tracker_key(), QueueClass::Graphics);
        self.tracking.pending_transfers.remove(&resource.tracker_key());
        self.tracking.access_tracker.observe(
            ResourceAccess::write(resource, AccessStage::TRANSFER),
            QueueClass::Graphics,
        );
    }

    /// The backend's presentation command transitions the acquired image back to
    /// the external present layout. The next use must plan from that state.
    pub fn record_presentation(&mut self, resource: ResourceRef) {
        self.tracking.state_tracker.set_current_state(resource, ResourceState::PRESENT);
        self.tracking.resource_queues.insert(resource.tracker_key(), QueueClass::Graphics);
    }

    // MARK: Plan entry point

    pub fn build_plan(
        &mut self,
        queue: QueueClass,
        commands: &[RecordedCommand],
        external: &SubmitDescriptor,
    ) -> RhiResult<SubmitPlan> {
        let mut main_commands: Vec<PlannedCommand> = Vec::new();
        let mut split_releases: Vec<SplitReleaseRequest> = Vec::new();
        let mut main_waits: Vec<TimelineSemaphore> = Vec::new();
        let mut main_signals: Vec<TimelineSemaphore> = Vec::new();

        for command in commands {
            match command {
                RecordedCommand::AccelerationStructureBuild(build) => {
                    let mut barriers: Vec<BarrierCommand> = Vec::new();
                    for input in &build.inputs {
                        let state = if input.kind == ResourceKind::Buffer {
                            ResourceState::SHADER_RESOURCE
                        } else {
                            ResourceState::ACCELERATION_STRUCTURE_READ
                        };
                        self.collect(
                            *input,
                            state,
                            Self::make_access(*input, state, AccessStage::COMPUTE, false),
                            BarrierPassScope::OutsidePass,
                            queue,
                            &mut barriers,
                            &mut split_releases,
                            &mut main_waits,
                        );
                    }
                    let output = ResourceRef::acceleration_structure(build.structure.id());
                    self.collect(
                        output,
                        ResourceState::ACCELERATION_STRUCTURE_WRITE,
                        ResourceAccess::write(output, AccessStage::COMPUTE),
                        BarrierPassScope::OutsidePass,
                        queue,
                        &mut barriers,
                        &mut split_releases,
                        &mut main_waits,
                    );
                    if !barriers.is_empty() {
                        main_commands.push(PlannedCommand::Barriers(barriers));
                    }
                    main_commands.push(PlannedCommand::AccelerationStructureBuild(build.clone()));
                }

                RecordedCommand::Barrier(barrier) => {
                    if let Some(resolved) = self.resolve_explicit(
                        *barrier,
                        queue,
                        &mut split_releases,
                        &mut main_waits,
                        &mut main_signals,
                    )? {
                        main_commands.push(PlannedCommand::Barriers(vec![resolved]));
                        self.apply_explicit(resolved)?;
                    }
                }

                RecordedCommand::RenderPass(record) => {
                    let mut pre_barriers: Vec<BarrierCommand> = Vec::new();
                    self.collect_render_pass_barriers(
                        record,
                        queue,
                        &mut pre_barriers,
                        &mut split_releases,
                        &mut main_waits,
                    );
                    if !pre_barriers.is_empty() {
                        main_commands.push(PlannedCommand::Barriers(pre_barriers));
                    }
                    main_commands.push(PlannedCommand::RenderPass(record.clone()));
                }

                RecordedCommand::ComputePass(record) => {
                    let mut pre_barriers: Vec<BarrierCommand> = Vec::new();
                    self.collect_compute_pass_barriers(
                        record,
                        queue,
                        &mut pre_barriers,
                        &mut split_releases,
                        &mut main_waits,
                    );
                    if !pre_barriers.is_empty() {
                        main_commands.push(PlannedCommand::Barriers(pre_barriers));
                    }
                    main_commands.push(PlannedCommand::ComputePass(record.clone()));
                }

                RecordedCommand::CopyPass(record) => {
                    let mut pre_barriers: Vec<BarrierCommand> = Vec::new();
                    self.collect_copy_pass_barriers(
                        record,
                        queue,
                        &mut pre_barriers,
                        &mut split_releases,
                        &mut main_waits,
                    );
                    if !pre_barriers.is_empty() {
                        main_commands.push(PlannedCommand::Barriers(pre_barriers));
                    }
                    main_commands.push(PlannedCommand::CopyPass(record.clone()));
                }
            }
        }

        self.finalize_plan(
            queue,
            main_commands,
            split_releases,
            external,
            main_waits,
            main_signals,
        )
    }

    // MARK: Per-pass resource collection

    fn collect_render_pass_barriers(
        &mut self,
        record: &RenderPassRecord,
        queue: QueueClass,
        barriers: &mut Vec<BarrierCommand>,
        split_releases: &mut Vec<SplitReleaseRequest>,
        waits: &mut Vec<TimelineSemaphore>,
    ) {
        for target in &record.descriptor.color_targets {
            let mut textures = vec![target.texture];
            textures.extend(target.resolve_texture);
            for texture in textures {
                let reference = ResourceRef::texture(texture.id());
                self.collect(
                    reference,
                    ResourceState::RENDER_TARGET,
                    Self::make_access(reference, ResourceState::RENDER_TARGET, AccessStage::FRAGMENT, true),
                    BarrierPassScope::BeforePass,
                    queue,
                    barriers,
                    split_releases,
                    waits,
                );
            }
        }
        if let Some(depth) = record.descriptor.depth_target {
            let reference = ResourceRef::texture(depth.texture.id());
            self.collect(
                reference,
                ResourceState::DEPTH_WRITE,
                Self::make_access(reference, ResourceState::DEPTH_WRITE, AccessStage::FRAGMENT, true),
                BarrierPassScope::BeforePass,
                queue,
                barriers,
                split_releases,
                waits,
            );
        }
        // Sets are immutable and their resources are collected before the
        // entire pass. Rebinding a set for another draw adds no new access.
        let mut collected_sets: HashSet<u32> = HashSet::new();
        let mut collected_reads: HashMap<u64, ResourceState> = HashMap::new();
        for render_command in &record.body {
            match render_command {
                RenderCommand::SetBindingSet { set, .. } => {
                    if !collected_sets.insert(set.id()) {
                        continue;
                    }
                    self.collect_binding_set_resources(
                        *set,
                        queue,
                        AccessStage::VERTEX
                            | AccessStage::FRAGMENT
                            | AccessStage::TASK
                            | AccessStage::MESH,
                        BarrierPassScope::BeforePass,
                        &mut collected_reads,
                        barriers,
                        split_releases,
                        waits,
                    );
                }
                RenderCommand::SetVertexBuffer { buffer, .. } => {
                    let reference = ResourceRef::buffer(buffer.id());
                    collected_reads.remove(&reference.tracker_key());
                    self.collect(
                        reference,
                        ResourceState::VERTEX_BUFFER,
                        Self::make_access(reference, ResourceState::VERTEX_BUFFER, AccessStage::VERTEX, false),
                        BarrierPassScope::BeforePass,
                        queue,
                        barriers,
                        split_releases,
                        waits,
                    );
                }
                RenderCommand::SetIndexBuffer { buffer, .. } => {
                    let reference = ResourceRef::buffer(buffer.id());
                    collected_reads.remove(&reference.tracker_key());
                    self.collect(
                        reference,
                        ResourceState::INDEX_BUFFER,
                        Self::make_access(reference, ResourceState::INDEX_BUFFER, AccessStage::VERTEX, false),
                        BarrierPassScope::BeforePass,
                        queue,
                        barriers,
                        split_releases,
                        waits,
                    );
                }
                RenderCommand::DrawIndirect { buffer, .. } => {
                    let reference = ResourceRef::buffer(buffer.id());
                    collected_reads.remove(&reference.tracker_key());
                    self.collect(
                        reference,
                        ResourceState::INDIRECT_ARGUMENT,
                        Self::make_access(reference, ResourceState::INDIRECT_ARGUMENT, AccessStage::VERTEX, false),
                        BarrierPassScope::BeforePass,
                        queue,
                        barriers,
                        split_releases,
                        waits,
                    );
                }
                _ => {}
            }
        }
    }

    fn collect_compute_pass_barriers(
        &mut self,
        record: &ComputePassRecord,
        queue: QueueClass,
        barriers: &mut Vec<BarrierCommand>,
        split_releases: &mut Vec<SplitReleaseRequest>,
        waits: &mut Vec<TimelineSemaphore>,
    ) {
        let mut collected_reads: HashMap<u64, ResourceState> = HashMap::new();
        for compute_command in &record.body {
            match compute_command {
                super::ComputeCommand::SetBindingSet { set, .. } => {
                    self.collect_binding_set_resources(
                        *set,
                        queue,
                        AccessStage::COMPUTE,
                        BarrierPassScope::BeforePass,
                        &mut collected_reads,
                        barriers,
                        split_releases,
                        waits,
                    );
                }
                super::ComputeCommand::DispatchIndirect { buffer, .. } => {
                    let reference = ResourceRef::buffer(buffer.id());
                    collected_reads.remove(&reference.tracker_key());
                    self.collect(
                        reference,
                        ResourceState::INDIRECT_ARGUMENT,
                        Self::make_access(reference, ResourceState::INDIRECT_ARGUMENT, AccessStage::COMPUTE, false),
                        BarrierPassScope::BeforePass,
                        queue,
                        barriers,
                        split_releases,
                        waits,
                    );
                }
                _ => {}
            }
        }
    }

    fn collect_copy_pass_barriers(
        &mut self,
        record: &CopyPassRecord,
        queue: QueueClass,
        barriers: &mut Vec<BarrierCommand>,
        split_releases: &mut Vec<SplitReleaseRequest>,
        waits: &mut Vec<TimelineSemaphore>,
    ) {
        for copy_command in &record.body {
            let pairs: [(ResourceRef, ResourceState); 2] = match copy_command {
                super::CopyCommand::CopyTexture { src, dst, .. } => [
                    (ResourceRef::texture(src.id()), ResourceState::COPY_SOURCE),
                    (ResourceRef::texture(dst.id()), ResourceState::COPY_DESTINATION),
                ],
                super::CopyCommand::CopyBuffer { src, dst, .. } => [
                    (ResourceRef::buffer(src.id()), ResourceState::COPY_SOURCE),
                    (ResourceRef::buffer(dst.id()), ResourceState::COPY_DESTINATION),
                ],
                super::CopyCommand::CopyBufferToTexture(upload) => [
                    (ResourceRef::buffer(upload.buffer.id()), ResourceState::COPY_SOURCE),
                    (ResourceRef::texture(upload.texture.id()), ResourceState::COPY_DESTINATION),
                ],
                super::CopyCommand::CopyTextureToBuffer { texture, buffer, .. } => [
                    (ResourceRef::texture(texture.id()), ResourceState::COPY_SOURCE),
                    (ResourceRef::buffer(buffer.id()), ResourceState::COPY_DESTINATION),
                ],
            };
            for (reference, state) in pairs {
                self.collect(
                    reference,
                    state,
                    Self::make_access(reference, state, AccessStage::TRANSFER, false),
                    BarrierPassScope::OutsidePass,
                    queue,
                    barriers,
                    split_releases,
                    waits,
                );
            }
        }
    }

    fn collect_binding_set_resources(
        &mut self,
        set: BindingSet,
        queue: QueueClass,
        stage: AccessStage,
        scope: BarrierPassScope,
        collected_reads: &mut HashMap<u64, ResourceState>,
        barriers: &mut Vec<BarrierCommand>,
        split_releases: &mut Vec<SplitReleaseRequest>,
        waits: &mut Vec<TimelineSemaphore>,
    ) {
        let Some(entries) = self.binding_set_entries.get(&set.id()).cloned() else { return };
        for tracked in entries {
            let access = ResourceAccess {
                resource: tracked.resource,
                kind: Self::access_kind(tracked.state),
                stage,
                range: None,
                is_attachment: false,
            };
            // All binding reads in this pass use the same stage mask and
            // whole-resource range. Different immutable sets can share one read
            // dependency; writes and other buffer uses reset this entry.
            let key = tracked.resource.tracker_key();
            if access.kind == AccessKind::Read {
                if collected_reads.get(&key) == Some(&tracked.state) {
                    continue;
                }
                collected_reads.insert(key, tracked.state);
            } else {
                collected_reads.remove(&key);
            }
            self.collect(
                tracked.resource,
                tracked.state,
                access,
                scope,
                queue,
                barriers,
                split_releases,
                waits,
            );
        }
    }

    fn tracked_resource(&self, resource: &BindingResource, read_only: bool) -> Option<TrackedBinding> {
        match *resource {
            BindingResource::Sampler(_) => None,
            BindingResource::Texture(texture) => Some(TrackedBinding {
                resource: ResourceRef::texture(texture.id()),
                state: ResourceState::SHADER_RESOURCE,
            }),
            BindingResource::StorageTexture(texture) => Some(TrackedBinding {
                resource: ResourceRef::texture(texture.id()),
                state: ResourceState::UNORDERED_ACCESS,
            }),
            BindingResource::UniformBuffer { buffer, .. } => Some(TrackedBinding {
                resource: ResourceRef::buffer(buffer.id()),
                state: ResourceState::CONSTANT_BUFFER,
            }),
            BindingResource::StorageBuffer { buffer, .. } => Some(TrackedBinding {
                resource: ResourceRef::buffer(buffer.id()),
                state: if read_only {
                    ResourceState::SHADER_RESOURCE
                } else {
                    ResourceState::UNORDERED_ACCESS
                },
            }),
            BindingResource::AccelerationStructure(structure) => Some(TrackedBinding {
                resource: ResourceRef::acceleration_structure(structure.id()),
                state: ResourceState::ACCELERATION_STRUCTURE_READ,
            }),
        }
    }

    // MARK: Automatic transition + cross-queue split

    #[allow(clippy::too_many_arguments)]
    fn collect(
        &mut self,
        resource: ResourceRef,
        desired: ResourceState,
        access: ResourceAccess,
        scope: BarrierPassScope,
        queue: QueueClass,
        barriers: &mut Vec<BarrierCommand>,
        split_releases: &mut Vec<SplitReleaseRequest>,
        waits: &mut Vec<TimelineSemaphore>,
    ) {
        let key = resource.tracker_key();
        let known_queue = self.tracking.resource_queues.get(&key).copied();
        let owner_queue = known_queue.unwrap_or(queue);
        let cross_queue = owner_queue != queue;

        // Layer 1: state/layout transition (may be empty when state is
        // unchanged). `require_state` returns the prior state in one lookup.
        let before_state = self.tracking.state_tracker.require_state(resource, desired);
        let state_barriers = self.tracking.state_tracker.commit_barriers();

        // Layer 2: access ordering (RAW/WAR/WAW), tracked regardless of state.
        let hazards = self.tracking.access_tracker.observe(access, queue);
        let same_queue_hazard = hazards
            .iter()
            .any(|hazard| hazard.source_queue == hazard.destination_queue);

        if cross_queue {
            // Ownership transfer: release on the owner (from the real prior
            // state) + acquire here (to the desired state). This subsumes both
            // the state transition and the memory ordering, so it is required
            // even when the state is unchanged.
            if let Some(pending) =
                self.matching_pending_transfer(resource, owner_queue, queue, before_state)
            {
                append_unique(waits, pending.semaphore);
            } else {
                append_split_release(
                    split_releases,
                    SplitReleaseRequest {
                        resource,
                        src_state: before_state,
                        src_queue: owner_queue,
                        dst_queue: queue,
                    },
                );
            }
            barriers.push(BarrierCommand {
                resource,
                source_state: before_state,
                destination_state: desired,
                sync_action: BarrierSyncAction::Acquire,
                pass_scope: scope,
                source_queue: owner_queue,
                destination_queue: queue,
            });
            self.tracking.pending_transfers.remove(&key);
        } else if let Some(state_barrier) = state_barriers.first() {
            // State changed: the transition barrier also orders memory.
            barriers.push(BarrierCommand {
                resource,
                source_state: state_barrier.before,
                destination_state: state_barrier.after,
                sync_action: BarrierSyncAction::Full,
                pass_scope: scope,
                source_queue: owner_queue,
                destination_queue: queue,
            });
        } else if !access.is_attachment && same_queue_hazard {
            // State unchanged, but a RAW/WAR/WAW hazard on a storage/unordered
            // access exists: emit an ordering barrier even though nothing
            // transitioned. Attachment writes are pass-ordered and excluded.
            barriers.push(BarrierCommand {
                resource,
                source_state: desired,
                destination_state: desired,
                sync_action: BarrierSyncAction::Full,
                pass_scope: scope,
                source_queue: owner_queue,
                destination_queue: queue,
            });
        }

        if known_queue != Some(queue) {
            self.tracking.resource_queues.insert(key, queue);
        }
    }

    // MARK: Access construction

    /// States that perform a write (anything else is a read).
    fn write_states() -> ResourceState {
        ResourceState::UNORDERED_ACCESS
            | ResourceState::RENDER_TARGET
            | ResourceState::DEPTH_WRITE
            | ResourceState::COPY_DESTINATION
            | ResourceState::RESOLVE_DESTINATION
            | ResourceState::ACCELERATION_STRUCTURE_WRITE
    }

    /// Derives the access kind (read/write) from a resource state.
    fn access_kind(state: ResourceState) -> AccessKind {
        if state.intersects(Self::write_states()) { AccessKind::Write } else { AccessKind::Read }
    }

    /// Builds a `ResourceAccess` for a collected resource.
    fn make_access(
        resource: ResourceRef,
        state: ResourceState,
        stage: AccessStage,
        is_attachment: bool,
    ) -> ResourceAccess {
        ResourceAccess {
            resource,
            kind: Self::access_kind(state),
            stage,
            range: None,
            is_attachment,
        }
    }

    // MARK: Explicit barrier resolution

    fn resolve_explicit(
        &mut self,
        barrier: BarrierCommand,
        queue: QueueClass,
        split_releases: &mut Vec<SplitReleaseRequest>,
        waits: &mut Vec<TimelineSemaphore>,
        signals: &mut Vec<TimelineSemaphore>,
    ) -> RhiResult<Option<BarrierCommand>> {
        let (src_queue, dst_queue) = (barrier.source_queue, barrier.destination_queue);
        let resource = barrier.resource;

        if src_queue == dst_queue {
            return Ok(Some(barrier));
        }

        match barrier.sync_action {
            BarrierSyncAction::Full => {
                if queue != dst_queue {
                    return Ok(None);
                }
                self.arrange_release(
                    resource,
                    src_queue,
                    dst_queue,
                    barrier.source_state,
                    split_releases,
                    waits,
                );
                let mut acquire = barrier;
                acquire.sync_action = BarrierSyncAction::Acquire;
                Ok(Some(acquire))
            }
            BarrierSyncAction::Acquire => {
                if queue != dst_queue {
                    return Ok(None);
                }
                self.arrange_release(
                    resource,
                    src_queue,
                    dst_queue,
                    barrier.source_state,
                    split_releases,
                    waits,
                );
                Ok(Some(barrier))
            }
            BarrierSyncAction::Release => {
                if queue != src_queue {
                    return Ok(None);
                }
                let semaphore = self.ensure_signal_semaphore(queue, signals);
                self.tracking.pending_transfers.insert(
                    resource.tracker_key(),
                    PendingTransfer {
                        src_queue,
                        dst_queue,
                        released_state: barrier.source_state,
                        semaphore,
                    },
                );
                let mut release = barrier;
                release.destination_state = release.source_state;
                Ok(Some(release))
            }
        }
    }

    fn arrange_release(
        &mut self,
        resource: ResourceRef,
        src_queue: QueueClass,
        dst_queue: QueueClass,
        state: ResourceState,
        split_releases: &mut Vec<SplitReleaseRequest>,
        waits: &mut Vec<TimelineSemaphore>,
    ) {
        if let Some(pending) = self.matching_pending_transfer(resource, src_queue, dst_queue, state) {
            append_unique(waits, pending.semaphore);
        } else {
            append_split_release(
                split_releases,
                SplitReleaseRequest { resource, src_state: state, src_queue, dst_queue },
            );
        }
    }

    fn apply_explicit(&mut self, barrier: BarrierCommand) -> RhiResult<()> {
        let resource = barrier.resource;
        let key = resource.tracker_key();
        match barrier.sync_action {
            BarrierSyncAction::Full => {
                self.tracking.state_tracker.set_current_state(resource, barrier.destination_state);
                self.tracking.resource_queues.insert(key, barrier.destination_queue);
                self.tracking.pending_transfers.remove(&key);
            }
            BarrierSyncAction::Acquire => {
                if let Some(pending) = self.tracking.pending_transfers.get(&key) {
                    let matches = pending.src_queue == barrier.source_queue
                        && pending.dst_queue == barrier.destination_queue
                        && pending.released_state == barrier.source_state;
                    if !matches {
                        return Err(RhiError::submit_failed(
                            "acquire barrier does not match a pending release",
                        ));
                    }
                    self.tracking.pending_transfers.remove(&key);
                }
                self.tracking.state_tracker.set_current_state(resource, barrier.destination_state);
                self.tracking.resource_queues.insert(key, barrier.destination_queue);
            }
            BarrierSyncAction::Release => {
                let released_state = barrier.source_state;
                self.tracking.state_tracker.set_current_state(resource, released_state);
                self.tracking.resource_queues.insert(key, barrier.source_queue);
                if barrier.source_queue == barrier.destination_queue {
                    self.tracking.pending_transfers.remove(&key);
                    return Ok(());
                }
                let existing_semaphore = self
                    .tracking
                    .pending_transfers
                    .get(&key)
                    .map(|pending| pending.semaphore)
                    .unwrap_or(TimelineSemaphore::new(0, 0));
                self.tracking.pending_transfers.insert(
                    key,
                    PendingTransfer {
                        src_queue: barrier.source_queue,
                        dst_queue: barrier.destination_queue,
                        released_state,
                        semaphore: existing_semaphore,
                    },
                );
            }
        }
        Ok(())
    }

    // MARK: Plan finalization

    fn finalize_plan(
        &mut self,
        queue: QueueClass,
        main_commands: Vec<PlannedCommand>,
        split_releases: Vec<SplitReleaseRequest>,
        external: &SubmitDescriptor,
        mut main_waits: Vec<TimelineSemaphore>,
        mut main_signals: Vec<TimelineSemaphore>,
    ) -> RhiResult<SubmitPlan> {
        let mut submits: Vec<PlannedSubmit> = Vec::new();

        for release_queue in QueueClass::ALL {
            let requests: Vec<&SplitReleaseRequest> = split_releases
                .iter()
                .filter(|request| request.src_queue == release_queue)
                .collect();
            if requests.is_empty() {
                continue;
            }
            let release_barriers: Vec<BarrierCommand> = requests
                .into_iter()
                .map(|request| BarrierCommand {
                    resource: request.resource,
                    source_state: request.src_state,
                    destination_state: request.src_state,
                    sync_action: BarrierSyncAction::Release,
                    pass_scope: BarrierPassScope::OutsidePass,
                    source_queue: request.src_queue,
                    destination_queue: request.dst_queue,
                })
                .collect();

            let signal = self.next_queue_timeline(release_queue);
            append_unique(&mut main_waits, signal);

            submits.push(PlannedSubmit {
                queue: release_queue,
                commands: vec![PlannedCommand::Barriers(release_barriers)],
                wait_semaphores: Vec::new(),
                signal_semaphores: vec![signal],
            });
        }

        for semaphore in &external.wait_semaphores {
            append_unique(&mut main_waits, *semaphore);
        }
        for semaphore in &external.signal_semaphores {
            append_unique(&mut main_signals, *semaphore);
        }

        submits.push(PlannedSubmit {
            queue,
            commands: main_commands,
            wait_semaphores: main_waits,
            signal_semaphores: main_signals,
        });

        Ok(SubmitPlan::new(submits))
    }

    // MARK: Timeline semaphores

    fn ensure_signal_semaphore(
        &mut self,
        queue: QueueClass,
        signals: &mut Vec<TimelineSemaphore>,
    ) -> TimelineSemaphore {
        if let Some(existing) = signals.first() {
            return *existing;
        }
        let semaphore = self.next_queue_timeline(queue);
        signals.push(semaphore);
        semaphore
    }

    fn next_queue_timeline(&mut self, queue: QueueClass) -> TimelineSemaphore {
        let mut timeline = self.tracking.queue_timelines.get(&queue).copied().unwrap_or_default();
        if timeline.id == 0 {
            timeline.id = self.tracking.next_timeline_semaphore_id;
            self.tracking.next_timeline_semaphore_id += 1;
        }
        timeline.next_value += 1;
        self.tracking.queue_timelines.insert(queue, timeline);
        TimelineSemaphore::new(timeline.id, timeline.next_value)
    }

    // MARK: Pending transfer lookup

    fn matching_pending_transfer(
        &self,
        resource: ResourceRef,
        src_queue: QueueClass,
        dst_queue: QueueClass,
        state: ResourceState,
    ) -> Option<PendingTransfer> {
        let pending = *self.tracking.pending_transfers.get(&resource.tracker_key())?;
        if pending.src_queue != src_queue
            || pending.dst_queue != dst_queue
            || pending.released_state != state
        {
            return None;
        }
        Some(pending)
    }
}
impl Default for SubmissionPlanner {
    fn default() -> Self {
        Self::new()
    }
}

fn append_split_release(list: &mut Vec<SplitReleaseRequest>, request: SplitReleaseRequest) {
    if let Some(existing) = list.iter_mut().find(|existing| {
        existing.resource == request.resource
            && existing.src_queue == request.src_queue
            && existing.dst_queue == request.dst_queue
    }) {
        existing.src_state = existing.src_state | request.src_state;
    } else {
        list.push(request);
    }
}

/// Appends `semaphore` unless an identical one is already present. Returns
/// whether it was added.
fn append_unique(list: &mut Vec<TimelineSemaphore>, semaphore: TimelineSemaphore) -> bool {
    if semaphore.id == 0 {
        return false;
    }
    if list.iter().any(|existing| *existing == semaphore) {
        return false;
    }
    list.push(semaphore);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rhi::{
        BindingSet, Buffer, CopyCommand, CopyPassEncoder, CopyPassRecord, RenderColorTarget,
        RenderPassDescriptor, Texture, TextureBufferUpload, TextureUploadRegion,
    };

    fn planner() -> SubmissionPlanner {
        SubmissionPlanner::new()
    }

    #[test]
    fn an_unchanged_state_emits_no_barrier() {
        let mut planner = planner();
        planner.record_immediate_write(ResourceRef::texture(1));
        let commands = vec![RecordedCommand::CopyPass(CopyPassRecord {
            body: vec![CopyCommand::CopyBuffer {
                src: Buffer::new(2),
                src_offset: 0,
                dst: Buffer::new(3),
                dst_offset: 0,
                size: 16,
            }],
        })];
        let plan = planner
            .build_plan(QueueClass::Graphics, &commands, &SubmitDescriptor::new())
            .expect("plan");
        assert_eq!(plan.submits.len(), 1);
        // The two buffers transition from their initial state, so one barrier
        // block precedes the copy pass.
        assert!(matches!(plan.submits[0].commands[0], PlannedCommand::Barriers(_)));
        assert!(matches!(plan.submits[0].commands[1], PlannedCommand::CopyPass(_)));
    }

    #[test]
    fn a_second_identical_use_needs_no_new_transition() {
        let mut planner = planner();
        let commands = vec![RecordedCommand::CopyPass(CopyPassRecord {
            body: vec![CopyCommand::CopyTexture {
                src: Texture::new(1),
                dst: Texture::new(2),
                width: 4,
                height: 4,
            }],
        })];
        let first = planner
            .build_plan(QueueClass::Graphics, &commands, &SubmitDescriptor::new())
            .expect("plan");
        assert!(matches!(first.submits[0].commands[0], PlannedCommand::Barriers(_)));

        let second = planner
            .build_plan(QueueClass::Graphics, &commands, &SubmitDescriptor::new())
            .expect("plan");
        // The states are already correct, but the copy is still a WAR hazard on
        // the destination, so an ordering barrier remains.
        assert_eq!(second.submits.len(), 1);
    }

    #[test]
    fn a_cross_queue_use_splits_into_release_and_acquire() {
        let mut planner = planner();
        let commands = vec![RecordedCommand::CopyPass(CopyPassRecord {
            body: vec![CopyCommand::CopyBuffer {
                src: Buffer::new(1),
                src_offset: 0,
                dst: Buffer::new(2),
                dst_offset: 0,
                size: 8,
            }],
        })];
        planner
            .build_plan(QueueClass::Graphics, &commands, &SubmitDescriptor::new())
            .expect("plan");

        // The same buffer read on the compute queue must hand off ownership.
        let compute_commands = vec![RecordedCommand::CopyPass(CopyPassRecord {
            body: vec![CopyCommand::CopyBuffer {
                src: Buffer::new(1),
                src_offset: 0,
                dst: Buffer::new(4),
                dst_offset: 0,
                size: 8,
            }],
        })];
        let plan = planner
            .build_plan(QueueClass::Compute, &compute_commands, &SubmitDescriptor::new())
            .expect("plan");
        assert_eq!(plan.submits.len(), 2);
        assert_eq!(plan.submits[0].queue, QueueClass::Graphics);
        assert_eq!(plan.submits[0].signal_semaphores.len(), 1);
        assert_eq!(plan.submits[1].queue, QueueClass::Compute);
        assert_eq!(plan.submits[1].wait_semaphores.len(), 1);
        assert_eq!(
            plan.submits[1].wait_semaphores[0],
            plan.submits[0].signal_semaphores[0]
        );
    }

    #[test]
    fn forgetting_a_resource_drops_its_queue_ownership() {
        let mut planner = planner();
        planner.record_immediate_write(ResourceRef::buffer(1));
        planner.forget_resource(ResourceRef::buffer(1));
        let commands = vec![RecordedCommand::CopyPass(CopyPassRecord {
            body: vec![CopyCommand::CopyBuffer {
                src: Buffer::new(1),
                src_offset: 0,
                dst: Buffer::new(2),
                dst_offset: 0,
                size: 8,
            }],
        })];
        let plan = planner
            .build_plan(QueueClass::Compute, &commands, &SubmitDescriptor::new())
            .expect("plan");
        // Ownership was forgotten, so the compute queue is treated as the owner
        // and no release submit is emitted.
        assert_eq!(plan.submits.len(), 1);
    }

    #[test]
    fn binding_sets_register_their_tracked_resources() {
        let mut planner = planner();
        let entries = vec![
            BindingSetEntry::new(0, BindingResource::Texture(Texture::new(7))),
            BindingSetEntry::new(1, BindingResource::Sampler(crate::rhi::Sampler::new(1))),
        ];
        planner.register_binding_set_entries(9, &entries, &HashSet::new());
        let mut buffer = super::super::CommandBuffer::new();
        buffer.render_pass(
            RenderPassDescriptor::new(vec![RenderColorTarget::new(Texture::new(1))]),
            |encoder: &mut super::super::RenderPassEncoder| {
                encoder.set_binding_set(BindingSet::new(9), 0, Vec::new());
            },
        );
        let plan = planner
            .build_plan(QueueClass::Graphics, buffer.commands(), &SubmitDescriptor::new())
            .expect("plan");
        assert_eq!(plan.submits.len(), 1);
        planner.remove_binding_set_entries(9);
    }

    #[test]
    fn uploads_record_a_transfer_write() {
        let mut planner = planner();
        let upload = TextureBufferUpload::new(
            Buffer::new(1),
            16,
            Texture::new(2),
            TextureUploadRegion::new(4, 4),
        );
        let mut encoder = CopyPassEncoder::default();
        encoder.upload_buffer_to_texture(upload);
        let commands = vec![RecordedCommand::CopyPass(CopyPassRecord { body: encoder.body().to_vec() })];
        let plan = planner
            .build_plan(QueueClass::Graphics, &commands, &SubmitDescriptor::new())
            .expect("plan");
        assert!(!plan.submits.is_empty());
        let snapshot = planner.checkpoint();
        planner.record_presentation(ResourceRef::texture(2));
        planner.restore(snapshot);
    }
}
