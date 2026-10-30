//! Translation of Graphics/Tests/NativeRHITests/SubmissionPlannerTests.swift.
//!
//! Barrier synthesis, duplicate-transition merging and cross-queue
//! release/acquire splitting. These are CPU-side planner contracts: no device is
//! created, so the suite runs on every host.

use std::collections::HashSet;

use cove_graphics::{
    BarrierCommand, BarrierSyncAction, BindingResource, BindingSet, BindingSetEntry, Buffer,
    ComputeCommand, ComputePassRecord, PlannedCommand, PlannedSubmit, QueueClass, RecordedCommand,
    RenderColorTarget, RenderCommand, RenderPassDescriptor, RenderPassRecord, ResourceKind,
    ResourceRef, ResourceState, Sampler, SubmitDescriptor, SubmissionPlanner, Texture,
};

fn set_entries(set: BindingSet, entries: &[BindingSetEntry]) -> RenderCommand {
    RenderCommand::SetBindingSet { slot: 0, set, dynamic_offsets: vec![] }
}

fn compute_binding(set: BindingSet) -> ComputeCommand {
    ComputeCommand::SetBindingSet { slot: 0, set, dynamic_offsets: vec![] }
}

fn render_pass(target: Option<Texture>, body: Vec<RenderCommand>) -> RenderPassRecord {
    let color_targets = target
        .map(|texture| vec![RenderColorTarget::new(texture)])
        .unwrap_or_default();
    RenderPassRecord {
        descriptor: RenderPassDescriptor::new(color_targets),
        body,
    }
}

fn compute_pass(body: Vec<ComputeCommand>) -> ComputePassRecord {
    ComputePassRecord { body }
}

fn commands(submit: &PlannedSubmit) -> Vec<BarrierCommand> {
    submit
        .commands
        .iter()
        .filter_map(|command| match command {
            PlannedCommand::Barriers(barriers) => Some(barriers.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

fn barrier_blocks(submit: &PlannedSubmit) -> Vec<Vec<BarrierCommand>> {
    submit
        .commands
        .iter()
        .filter_map(|command| match command {
            PlannedCommand::Barriers(barriers) => Some(barriers.clone()),
            _ => None,
        })
        .collect()
}

fn register(planner: &mut SubmissionPlanner, set: BindingSet, entries: &[BindingSetEntry]) {
    planner.register_binding_set_entries(set.id(), entries, &HashSet::new());
}

/// Immutable binding sets keep their resolved description, which is what lets a
/// repeated read skip its barriers.
fn register_read_only(
    planner: &mut SubmissionPlanner,
    set: BindingSet,
    entries: &[BindingSetEntry],
    read_only: &[u32],
) {
    planner.register_binding_set_entries(
        set.id(),
        entries,
        &read_only.iter().copied().collect(),
    );
}

#[test]
fn cached_read_dependencies_keep_upload_queue_write_and_forget_boundaries() {
    let mut planner = SubmissionPlanner::new();
    let set = BindingSet::new(50);
    let resources = [
        ResourceRef::new(ResourceKind::Texture, 2),
        ResourceRef::new(ResourceKind::Buffer, 3),
        ResourceRef::new(ResourceKind::Buffer, 4),
    ];
    register_read_only(
        &mut planner,
        set,
        &[
            BindingSetEntry::new(0, BindingResource::Texture(Texture::new(2))),
            BindingSetEntry::new(1, BindingResource::uniform_buffer(Buffer::new(3))),
            BindingSetEntry::new(2, BindingResource::storage_buffer(Buffer::new(4))),
            BindingSetEntry::new(3, BindingResource::Sampler(Sampler::new(5))),
        ],
        &[2],
    );

    let render = RecordedCommand::RenderPass(render_pass(None, vec![set_entries(set, &[])]));
    for resource in resources {
        planner.record_immediate_write(resource);
    }

    // The first read after an upload must barrier every written resource.
    let first = planner
        .build_plan(QueueClass::Graphics, &[render.clone()], &SubmitDescriptor::default())
        .expect("plan");
    let first_resources: HashSet<ResourceRef> =
        commands(&first.submits[0]).into_iter().map(|b| b.resource).collect();
    assert_eq!(first_resources, resources.into_iter().collect());

    // Cached reads emit nothing.
    let steady = planner
        .build_plan(QueueClass::Graphics, &[render.clone()], &SubmitDescriptor::default())
        .expect("plan");
    assert!(
        barrier_blocks(&steady.submits[0]).is_empty(),
        "cached reads must not re-emit barriers"
    );

    // Re-writing one resource re-emits exactly that resource.
    planner.record_immediate_write(resources[2]);
    let uploaded = planner
        .build_plan(QueueClass::Graphics, &[render], &SubmitDescriptor::default())
        .expect("plan");
    let uploaded_resources: Vec<ResourceRef> =
        commands(&uploaded.submits[0]).into_iter().map(|b| b.resource).collect();
    assert_eq!(uploaded_resources, vec![resources[2]]);

    // Crossing to the compute queue splits release from acquire.
    let compute = RecordedCommand::ComputePass(compute_pass(vec![compute_binding(set)]));
    let handoff = planner
        .build_plan(QueueClass::Compute, &[compute.clone()], &SubmitDescriptor::default())
        .expect("plan");
    assert_eq!(handoff.submits.len(), 2, "cross-queue work must split");
    assert_eq!(
        handoff.submits[0].signal_semaphores, handoff.submits[1].wait_semaphores,
        "the acquire waits on the release signal"
    );
    let acquired = commands(&handoff.submits[1]);
    assert_eq!(
        acquired.iter().map(|b| b.resource).collect::<HashSet<_>>(),
        resources.into_iter().collect()
    );
    assert!(
        acquired.iter().all(|b| b.sync_action == BarrierSyncAction::Acquire),
        "every cross-queue barrier is an acquire"
    );

    // Forgetting a resource makes the next reader re-establish its barrier.
    planner.forget_resource(resources[0]);
    let forgotten = planner
        .build_plan(QueueClass::Compute, &[compute], &SubmitDescriptor::default())
        .expect("plan");
    let forgotten_resources: Vec<ResourceRef> =
        commands(&forgotten.submits[0]).into_iter().map(|b| b.resource).collect();
    assert_eq!(forgotten_resources, vec![resources[0]]);
}

#[test]
fn presentation_restores_external_state_before_image_reuse() {
    let mut planner = SubmissionPlanner::new();
    let target = Texture::new(1);
    let pass = RecordedCommand::RenderPass(render_pass(Some(target), vec![]));

    planner
        .build_plan(QueueClass::Graphics, &[pass.clone()], &SubmitDescriptor::default())
        .expect("first plan");
    planner.record_presentation(ResourceRef::new(ResourceKind::Texture, target.id()));

    let reused = planner
        .build_plan(QueueClass::Graphics, &[pass], &SubmitDescriptor::default())
        .expect("reused plan");
    let barrier = barrier_blocks(&reused.submits[0])
        .first()
        .and_then(|block| block.first())
        .copied()
        .expect("a barrier precedes image reuse");
    assert_eq!(barrier.source_state, ResourceState::PRESENT);
    assert_eq!(barrier.destination_state, ResourceState::RENDER_TARGET);
}

#[test]
fn repeated_render_bindings_preserve_pass_and_queue_dependencies() {
    let mut planner = SubmissionPlanner::new();
    let set = BindingSet::new(50);
    let storage = Buffer::new(4);
    register(
        &mut planner,
        set,
        &[
            BindingSetEntry::new(0, BindingResource::uniform_buffer(Buffer::new(3))),
            BindingSetEntry::new(1, BindingResource::Texture(Texture::new(2))),
            BindingSetEntry::new(2, BindingResource::storage_buffer(storage)),
        ],
    );

    // 100 identical bindings in one pass must merge to one transition each.
    let body: Vec<RenderCommand> = (0..100).map(|_| set_entries(set, &[])).collect();
    let pass = RecordedCommand::RenderPass(render_pass(Some(Texture::new(1)), body));
    let result = planner
        .build_plan(QueueClass::Graphics, &[pass.clone(), pass], &SubmitDescriptor::default())
        .expect("plan");

    let blocks = barrier_blocks(&result.submits[0]);
    assert_eq!(blocks.len(), 2, "one barrier block per render pass");
    assert_eq!(
        blocks[0].iter().map(|b| b.resource.id).collect::<HashSet<u32>>(),
        [1u32, 2, 3, 4].into_iter().collect(),
        "the pass covers its target and its bindings"
    );
    assert_eq!(
        blocks[0].iter().filter(|b| b.resource.id == storage.id()).count(),
        1,
        "duplicate transitions to one resource merge"
    );
    assert_eq!(
        blocks[1].iter().filter(|b| b.resource.id == storage.id()).count(),
        1,
        "each pass re-establishes its own storage dependency"
    );
    let ordered = blocks[1]
        .iter()
        .find(|b| b.resource.id == storage.id())
        .copied()
        .expect("storage barrier in the second pass");
    assert_eq!(ordered.source_state, ResourceState::UNORDERED_ACCESS);
    assert_eq!(ordered.destination_state, ResourceState::UNORDERED_ACCESS);

    let compute = planner
        .build_plan(
            QueueClass::Compute,
            &[RecordedCommand::ComputePass(compute_pass(vec![compute_binding(set)]))],
            &SubmitDescriptor::default(),
        )
        .expect("compute plan");
    assert_eq!(compute.submits.len(), 2);
    assert_eq!(
        compute.submits[1].wait_semaphores, compute.submits[0].signal_semaphores,
        "compute waits on the graphics release"
    );
    let acquire_ids: HashSet<u32> = commands(&compute.submits[1])
        .into_iter()
        .filter(|b| b.sync_action == BarrierSyncAction::Acquire)
        .map(|b| b.resource.id)
        .collect();
    assert_eq!(
        acquire_ids,
        [2u32, 3, 4].into_iter().collect(),
        "the sampler carries no memory dependency"
    );
}

#[test]
fn distinct_sets_sharing_reads_keep_upload_and_queue_handoffs() {
    let mut planner = SubmissionPlanner::new();
    let buffer = Buffer::new(1);
    let resource = ResourceRef::new(ResourceKind::Buffer, buffer.id());
    planner.record_immediate_write(resource);

    // 96 sets read the same buffer, each with its own texture.
    let sets: Vec<BindingSet> = (0..96).map(|index| BindingSet::new(50 + index)).collect();
    for (index, set) in sets.iter().enumerate() {
        register(
            &mut planner,
            *set,
            &[
                BindingSetEntry::new(0, BindingResource::uniform_buffer(buffer)),
                BindingSetEntry::new(
                    1,
                    BindingResource::Texture(Texture::new(1000 + index as u32)),
                ),
            ],
        );
    }

    let body: Vec<RenderCommand> = sets.iter().map(|set| set_entries(*set, &[])).collect();
    let graphics = planner
        .build_plan(
            QueueClass::Graphics,
            &[RecordedCommand::RenderPass(render_pass(None, body))],
            &SubmitDescriptor::default(),
        )
        .expect("graphics plan");
    let barriers = commands(&graphics.submits[0]);
    assert_eq!(barriers.len(), 97, "96 textures plus the shared buffer");
    let upload = barriers
        .iter()
        .find(|b| b.resource == resource)
        .copied()
        .expect("the immediate write is barriered");
    assert_eq!(upload.source_state, ResourceState::COPY_DESTINATION);
    assert_eq!(upload.destination_state, ResourceState::CONSTANT_BUFFER);

    let compute_body: Vec<ComputeCommand> =
        sets.iter().map(|set| compute_binding(*set)).collect();
    let compute = planner
        .build_plan(
            QueueClass::Compute,
            &[RecordedCommand::ComputePass(compute_pass(compute_body))],
            &SubmitDescriptor::default(),
        )
        .expect("compute plan");
    assert_eq!(compute.submits.len(), 2);
    assert_eq!(
        compute.submits[0].signal_semaphores, compute.submits[1].wait_semaphores,
        "the release signals and the acquire waits"
    );
    let acquired = commands(&compute.submits[1]);
    assert_eq!(acquired.len(), 97);
    assert!(acquired.iter().all(|b| b.sync_action == BarrierSyncAction::Acquire));

    // A writer on the same buffer demands unordered access.
    let writer = BindingSet::new(200);
    register(
        &mut planner,
        writer,
        &[BindingSetEntry::new(0, BindingResource::storage_buffer(buffer))],
    );
    let write = planner
        .build_plan(
            QueueClass::Compute,
            &[RecordedCommand::ComputePass(compute_pass(vec![compute_binding(writer)]))],
            &SubmitDescriptor::default(),
        )
        .expect("write plan");
    let state = barrier_blocks(&write.submits[0])
        .first()
        .and_then(|block| block.first())
        .map(|b| b.destination_state);
    assert_eq!(state, Some(ResourceState::UNORDERED_ACCESS));
}
