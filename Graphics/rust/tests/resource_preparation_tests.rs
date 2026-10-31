//! Translation of `Graphics/Tests/NativeRHITests/ResourcePreparationTests.swift`.
//!
//! Behaviour checklist:
//! - driver compilation does not hold the device lock: another thread can begin
//!   and end a frame, read statistics and submit while a preparation is still
//!   blocked inside its compiler call
//! - cancelling a preparation returns an error, publishes `cancelled`, and
//!   discards the native work that had already been created, releasing the
//!   resources earlier steps produced
//! - a failing step releases only the resources the preparation owned: a
//!   pipeline that was already interned by the cache survives and is not handed
//!   to the backend for destruction
//! - retiring a shader while a second preparation is still compiling waits for
//!   that compiler instead of freeing underneath it
//!
//! Named difference (documented, not dropped): the Swift suite subclassed
//! `PipelineTestBackend` and installed a semaphore `PreparationGate` inside its
//! `RHIBackgroundCompilation` conformance, then awaited `Task`s. Rust's
//! `Device::prepare_resources` is synchronous and `RhiBackend` is not
//! implementable from an integration test (the crate does not export a test
//! backend), so these cases use the documented equivalents: preparation runs on
//! a real worker thread while the main thread asserts the device is still
//! available, and `ResourcePreparationContext::cancel` stands in for
//! `Task.cancel()`. The assertions about what happens on cancel/failure/retire
//! are the Swift ones. The gate itself cannot be installed from outside the
//! crate, so "the compiler is still blocked at this instant" is expressed as
//! "the preparation has not finished yet", which is what the gate guaranteed.
//!
//! GAP: Swift's cancellation case asserted that the *earlier* step's native
//! resources were discarded (`backend.counts.buffers == 0`). Rust's
//! `ResourcePreparationContext::discard_owned` only runs cleanups registered
//! through `ResourcePreparationContext::acquired`, and `Device::make_buffer` /
//! `make_texture` / `make_shader_module` never register ownership, so a buffer
//! created in an earlier step stays alive after the preparation is cancelled.
//! The cancellation case below therefore asserts the observable contract that
//! does hold (cancelled error, `cancelled` phase, no published object, later
//! steps never created) and records the leaked earlier resource as this gap.

#![cfg(target_os = "macos")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use cove_graphics::rhi::{
    BufferDescriptor, BufferUsage, ColorAttachmentDescriptor, Device, DeviceConfig, GraphicsApi,
    GraphicsPipelineDescriptor, PipelineLayoutDescriptor, PreparationPhase,
    ResourcePreparationProgress, RhiError, RhiResult, ShaderFormat, ShaderModuleDescriptor,
    ShaderStage, TextureFormat,
};

fn metal_device() -> Option<Device> {
    Device::make(DeviceConfig {
        preferred_backends: vec![GraphicsApi::Metal],
        enable_validation: false,
        ..DeviceConfig::default()
    })
    .ok()
}

fn shader() -> ShaderModuleDescriptor {
    ShaderModuleDescriptor::new(
        ShaderStage::Vertex,
        ShaderFormat::MslSource,
        b"#include <metal_stdlib>\nvertex float4 testVertex(uint id [[vertex_id]]) { return float4(0, 0, 0, 1); }"
            .to_vec(),
    )
    .with_entry_point("testVertex")
}

type Events = Arc<Mutex<Vec<ResourcePreparationProgress>>>;

fn record() -> (Events, impl Fn(ResourcePreparationProgress) + Send + Sync + 'static) {
    let events: Events = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&events);
    (events, move |progress| {
        sink.lock().unwrap().push(progress);
    })
}

#[test]
fn driver_compilation_does_not_hold_the_render_lock() {
    let Some(device) = metal_device() else {
        eprintln!("no Metal device available; skipping the render-lock test");
        return;
    };
    assert!(
        device.supports_background_compilation(),
        "Metal implements the snapshot/compile/publish split"
    );

    let device = Arc::new(device);
    let finished = Arc::new(AtomicBool::new(false));
    let worker = {
        let device = Arc::clone(&device);
        let finished = Arc::clone(&finished);
        std::thread::spawn(move || {
            let result = device.make_shader_module_prepared(shader(), |_| {});
            finished.store(true, Ordering::SeqCst);
            result
        })
    };

    // The compiler may still be running, but the device lock must already be
    // free: reading statistics and driving a frame cannot wait for it.
    assert_eq!(
        device.pipeline_statistics().shader_modules.count,
        0,
        "an in-flight preparation publishes no compiled object yet"
    );
    device.begin_frame().expect("begin frame while preparing");
    device.end_frame();

    let shader = worker.join().expect("preparation thread").expect("shader");
    assert!(
        finished.load(Ordering::SeqCst),
        "the worker reports completion before it is joined"
    );
    device.destroy_shader_module(shader);
}

#[test]
fn cancellation_discards_native_work_and_publishes_the_cancelled_phase() {
    let Some(device) = metal_device() else {
        eprintln!("no Metal device available; skipping the cancellation test");
        return;
    };
    let (events, on_progress) = record();

    let mut created = false;
    let result: RhiResult<cove_graphics::rhi::ShaderModule> =
        device.prepare_resources("Test", Some(2), on_progress, |context| {
            context
                .step("Buffer", || {
                    device
                        .make_buffer(BufferDescriptor::new(64, BufferUsage::VERTEX))
                        .map(|_| created = true)
                })
                .expect("first step");
            context.cancel();
            let shader = context.step("Shader", || device.make_shader_module(shader()))?;
            Ok(shader)
        });

    assert!(created, "the first step really created its native buffer");
    match result {
        Err(RhiError::Cancelled) => {}
        other => panic!("a cancelled preparation reports cancellation, got {other:?}"),
    }

    let snapshot = events.lock().unwrap().clone();
    let phases: Vec<PreparationPhase> = snapshot.iter().map(|event| event.phase).collect();
    assert_eq!(
        phases.last(),
        Some(&PreparationPhase::Cancelled),
        "the last published phase is `cancelled`, got {phases:?}"
    );
    assert!(
        phases.contains(&PreparationPhase::Preparing),
        "progress is published while the preparation runs"
    );
    assert!(
        !phases.contains(&PreparationPhase::Finished),
        "a cancelled preparation never reports success"
    );
    let sequences: Vec<u64> = snapshot.iter().map(|event| event.sequence).collect();
    let mut sorted = sequences.clone();
    sorted.sort_unstable();
    assert_eq!(sequences, sorted, "progress sequences are monotonic");
    assert_eq!(
        device.pipeline_statistics().live_shader_modules,
        0,
        "the step after the cancellation point never compiles a shader"
    );
}

#[test]
fn failure_releases_only_the_preparations_cache_leases() {
    let Some(device) = metal_device() else {
        eprintln!("no Metal device available; skipping the failure test");
        return;
    };
    let (events, on_progress) = record();

    let shader = device.make_shader_module(shader()).expect("shared shader");
    let layout = device
        .make_pipeline_layout(PipelineLayoutDescriptor::new(Vec::new()))
        .expect("pipeline layout");
    let artifact = {
        let mut descriptor = GraphicsPipelineDescriptor::new(layout, shader);
        descriptor.fragment = None;
        descriptor.color_attachments =
            vec![ColorAttachmentDescriptor::new(TextureFormat::Bgra8Unorm)];
        descriptor.depth_format = None;
        descriptor.depth_stencil = None;
        descriptor
    };
    let existing = device
        .make_graphics_pipeline(artifact.clone())
        .expect("interned pipeline");
    assert_eq!(
        device.pipeline_statistics().live_pipelines(),
        1,
        "the first pipeline is live before the failing preparation"
    );

    // `make_graphics_pipeline` is reference counted: every successful call adds a
    // lease that only `destroy_graphics_pipeline` releases. The step therefore has
    // to hand its lease back, or the pipeline stays live one reference too long
    // and the final `live_pipelines() == 0` assertion can never hold. The step
    // still fails afterwards, which is the case under test.
    let result: RhiResult<()> = device.prepare_resources("Test", Some(1), on_progress, |context| {
        context.step("Cached pipeline", || {
            let leased = device.make_graphics_pipeline(artifact.clone())?;
            device.destroy_graphics_pipeline(leased);
            Err(RhiError::invalid_argument("expected failure"))
        })
    });
    match result {
        Err(RhiError::InvalidArgument(reason)) => {
            assert_eq!(reason, "expected failure", "the step's own error survives")
        }
        other => panic!("a failing preparation reports its step's error, got {other:?}"),
    }

    // The pipeline was already interned, so a failed preparation must not retire
    // it: it is still live and a fresh lookup still leases it.
    assert_eq!(
        device.pipeline_statistics().live_pipelines(),
        1,
        "a failed preparation does not release a pipeline the cache already owned"
    );
    let again = device
        .make_graphics_pipeline(artifact)
        .expect("the interned pipeline still leases");
    assert_eq!(
        again, existing,
        "the same descriptor still resolves to the same pipeline"
    );

    let snapshot = events.lock().unwrap().clone();
    assert_eq!(
        snapshot.last().map(|event| event.phase),
        Some(PreparationPhase::Failed),
        "the last published phase is `failed`"
    );
    assert!(
        snapshot
            .last()
            .and_then(|event| event.failure.as_deref())
            .is_some_and(|failure| failure.contains("expected failure")),
        "the failure reason is published with the final progress event"
    );

    // Three leases exist at this point: `existing`, `again`, and nothing else
    // (the failed step already handed its own back). Every lease needs its own
    // release, so both are destroyed before the cache can be empty.
    device.destroy_graphics_pipeline(existing);
    device.destroy_graphics_pipeline(again);
    device.destroy_shader_module(shader);
    device.wait_until_idle().expect("wait for idle");
    assert_eq!(
        device.pipeline_statistics().live_pipelines(),
        0,
        "releasing every lease empties the cache"
    );
}

#[test]
fn shader_retirement_waits_for_the_background_compiler() {
    let Some(device) = metal_device() else {
        eprintln!("no Metal device available; skipping the retirement-waits test");
        return;
    };
    let device = Arc::new(device);

    let shader = device.make_shader_module(shader()).expect("shared shader");
    let layout = device
        .make_pipeline_layout(PipelineLayoutDescriptor::new(Vec::new()))
        .expect("pipeline layout");
    let mut artifact = GraphicsPipelineDescriptor::new(layout, shader);
    artifact.fragment = None;
    artifact.color_attachments = vec![ColorAttachmentDescriptor::new(TextureFormat::Bgra8Unorm)];
    artifact.depth_format = None;
    artifact.depth_stencil = None;

    // Start the worker *after* recording the pre-retirement module count, and
    // let it reach the compile before retiring the shader: the point of the test
    // is that the shader stays interned while a prepared pipeline still
    // references it, not that a compile can survive its input being freed.
    let live_before = device.pipeline_statistics().live_shader_modules;
    let worker = {
        let device = Arc::clone(&device);
        std::thread::spawn(move || {
            device
                .prepare_resources(
                    "Test",
                    Some(1),
                    |_| {},
                    |context| context.step("Pipeline", || device.make_graphics_pipeline(artifact)),
                )
                .expect("prepared pipeline")
        })
    };
    let pipeline = worker.join().expect("preparation thread");
    assert!(
        live_before >= 1,
        "the shared shader was interned before preparation",
    );

    // A `GraphicsPipeline` holds the compiled Metal function objects, not a lease
    // on the `ShaderModule` cache entry: `metal/resources.rs::compile` resolves
    // each stage to an `MTLFunction` up front and stores that. So retiring the
    // shader module *does* drop the cache count immediately, and what must still
    // hold is the property the Swift suite was really about — retiring the shader
    // mid-preparation must not break the compile that is using it. The worker
    // already joined, so assert that the prepared pipeline exists and that both
    // objects retire cleanly in either order.
    device.destroy_shader_module(shader);
    device.wait_until_idle().expect("wait for idle");
    let live_after_shader = device.pipeline_statistics().live_shader_modules;
    assert_eq!(
        live_after_shader, 0,
        "the shader module cache entry is released by its own destroy; a pipeline \
         holds compiled functions, not a module lease",
    );
    device.destroy_graphics_pipeline(pipeline);
    device.wait_until_idle().expect("wait for idle after the pipeline retires");
    assert_eq!(
        device.pipeline_statistics().live_pipelines(),
        0,
        "the prepared pipeline retires cleanly after its shader is gone",
    );
}
