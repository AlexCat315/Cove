//! Translation of `Graphics/Tests/NativeRHITests/SwapchainLifetimeTests.swift`.
//!
//! Presentation ownership is the part of the RHI that must not leak or reuse
//! state: a swapchain's presentation mode follows only its own window, a retired
//! handle is rejected, frame sessions serialize preparation, a queued
//! presentation keeps its slot until its completion runs, an abandoned
//! acquisition is returned rather than lost, and an owned swapchain closes once.
//!
//! The Swift suite drives a purpose-built `PresentationBackend` that records
//! windows and holds presentation completions by hand. `support::RecordingBackend`
//! has that shape (`RecordingState` with `windows`, `pending_presentations`,
//! `reject_next_idle`, `reject_next_present`) and covers five of the seven cases.
//! It vends a *fresh* texture per acquisition, though, whereas Swift's backend
//! returned one pooled texture per window — and that pooling is exactly what the
//! "old ticket" case is about. Rather than weaken that assertion or change the
//! shared harness other Graphics suites depend on, this file defines its own
//! `PoolingBackend`, a direct port of Swift's `PresentationBackend`: same
//! per-window texture pool, same one-generation-per-window image, same
//! `RHIError.outOfMemory` for every resource operation the suite never uses.
//!
//! Behaviour checklist:
//! - presentation mode follows only its own window and honours a reconfigure
//! - a retired swapchain handle is rejected
//! - a nested frame session is refused and the outer one still closes
//! - a queued presentation keeps its window until the completion runs
//! - an abandoned acquisition is returned and tracked
//! - a re-acquired pooled texture yields a new ticket even when every other
//!   field matches, so the stale ticket is rejected
//! - a present refused before queueing can be retried exactly once
//! - an owned swapchain closes idempotently and survives a completed-work error

#![cfg(target_os = "macos")]

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use cove_graphics::{
    AdapterCapabilities, BindingLayout, BindingLayoutEntry, BindingSet, BindingSetEntry, Buffer,
    BufferDescriptor, Capabilities, Completion, ComputePipeline, ComputePipelineDescriptor, Device,
    DeviceConfig, FrameUploader, GraphicsApi, GraphicsPipeline, GraphicsPipelineDescriptor,
    PlannedSubmit, PresentationMode, RhiBackend, RhiError, RhiResult, Sampler, SamplerDescriptor,
    ShaderModule, ShaderModuleDescriptor, SubmissionCpuProfile, SurfaceDescriptor, Swapchain,
    SwapchainImage, SwapchainResource, Texture, TextureDescriptor, TextureSubresource,
    TextureUploadRegion, UploadLocation,
};

/// Direct port of the Swift suite's private `PresentationBackend`.
///
/// Holds presentation completions by hand and returns one pooled texture per
/// window, so a re-acquired image references the same texture with the same
/// generation while still being a distinct ticket.
struct PoolingBackend {
    state: Arc<Mutex<PoolingState>>,
}

#[derive(Default)]
struct PoolingState {
    windows: HashMap<u32, SurfaceDescriptor>,
    completions: Vec<Completion>,
    fail_next_idle: bool,
    fail_next_present: bool,
}

impl PoolingState {
    fn pending_count(&self) -> usize {
        self.completions.len()
    }

    fn window_count(&self) -> usize {
        self.windows.len()
    }

    fn descriptor_for(&self, handle: Swapchain) -> Option<SurfaceDescriptor> {
        self.windows.get(&handle.id()).copied()
    }

    fn complete(&mut self) {
        let bodies = std::mem::take(&mut self.completions);
        for body in bodies {
            body();
        }
    }
}

impl PoolingBackend {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(PoolingState::default())),
        }
    }

    fn state(&self) -> Arc<Mutex<PoolingState>> {
        Arc::clone(&self.state)
    }

    fn lock(&self) -> MutexGuard<'_, PoolingState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Every resource operation the suite never reaches reports out of memory, as
/// Swift's `PresentationBackend` did.
fn refused<T>() -> RhiResult<T> {
    Err(RhiError::OutOfMemory)
}

struct RefusingUploader;

impl FrameUploader for RefusingUploader {
    fn reset(&mut self) {}

    fn write(&mut self, _bytes: &[u8], _alignment: usize) -> RhiResult<UploadLocation> {
        refused()
    }
}

impl RhiBackend for PoolingBackend {
    fn api(&self) -> GraphicsApi {
        GraphicsApi::Metal
    }

    fn device_name(&self) -> &str {
        "controlled presentation"
    }

    fn query_capabilities(&self) -> Capabilities {
        Capabilities::default()
    }

    fn query_adapter_capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities::default()
    }

    fn configure_swapchain(
        &mut self,
        handle: Swapchain,
        descriptor: SurfaceDescriptor,
    ) -> RhiResult<()> {
        self.lock().windows.insert(handle.id(), descriptor);
        Ok(())
    }

    fn presentation_mode(&self, handle: Swapchain) -> RhiResult<PresentationMode> {
        let state = self.lock();
        let descriptor = state
            .descriptor_for(handle)
            .ok_or_else(|| RhiError::invalid_argument("unknown swapchain"))?;
        Ok(if descriptor.vsync_enabled {
            PresentationMode::Fifo
        } else {
            PresentationMode::Immediate
        })
    }

    fn destroy_swapchain(&mut self, handle: Swapchain) {
        self.lock().windows.remove(&handle.id());
    }

    fn acquire_swapchain_image(&mut self, handle: Swapchain) -> RhiResult<SwapchainImage> {
        let state = self.lock();
        let descriptor = state
            .descriptor_for(handle)
            .ok_or_else(|| RhiError::invalid_argument("unknown swapchain"))?;
        // One pooled texture per window, and one generation per window: a later
        // acquisition reuses both, which is what makes the stale-ticket case
        // meaningful.
        Ok(SwapchainImage::new(
            handle,
            1,
            Texture::new(0x8000_0000 + handle.id()),
            descriptor.width,
            descriptor.height,
        ))
    }

    fn present(&mut self, _image: SwapchainImage, completion: Completion) -> RhiResult<()> {
        let mut state = self.lock();
        if state.fail_next_present {
            state.fail_next_present = false;
            return Err(RhiError::present_failed("before queueing"));
        }
        state.completions.push(completion);
        Ok(())
    }

    fn create_buffer(&mut self, _handle: Buffer, _descriptor: BufferDescriptor) -> RhiResult<()> {
        refused()
    }

    fn create_texture(
        &mut self,
        _handle: Texture,
        _descriptor: TextureDescriptor,
    ) -> RhiResult<()> {
        refused()
    }

    fn create_sampler(
        &mut self,
        _handle: Sampler,
        _descriptor: SamplerDescriptor,
    ) -> RhiResult<()> {
        refused()
    }

    fn create_shader_module(
        &mut self,
        _handle: ShaderModule,
        _descriptor: ShaderModuleDescriptor,
    ) -> RhiResult<()> {
        refused()
    }

    fn create_graphics_pipeline(
        &mut self,
        _handle: GraphicsPipeline,
        _descriptor: GraphicsPipelineDescriptor,
    ) -> RhiResult<()> {
        refused()
    }

    fn create_compute_pipeline(
        &mut self,
        _handle: ComputePipeline,
        _descriptor: ComputePipelineDescriptor,
    ) -> RhiResult<()> {
        refused()
    }

    fn register_binding_set(
        &mut self,
        _handle: BindingSet,
        _layout: BindingLayout,
        _layout_entries: Vec<BindingLayoutEntry>,
        _set_entries: Vec<BindingSetEntry>,
    ) -> RhiResult<()> {
        refused()
    }

    fn unregister_binding_set(&mut self, _handle: BindingSet) {}

    fn upload_buffer_data(
        &mut self,
        _buffer: Buffer,
        _offset: usize,
        _data: &[u8],
    ) -> RhiResult<()> {
        refused()
    }

    fn upload_texture_data(
        &mut self,
        _texture: Texture,
        _data: &[u8],
        _region: TextureUploadRegion,
        _bytes_per_row: usize,
        _subresource: TextureSubresource,
    ) -> RhiResult<()> {
        refused()
    }

    fn read_texture_data(
        &mut self,
        _texture: Texture,
        _width: usize,
        _height: usize,
        _bytes_per_row: usize,
        _subresource: TextureSubresource,
        _destination: &mut [u8],
    ) -> RhiResult<()> {
        refused()
    }

    fn make_frame_uploader(&mut self, _slot: usize) -> RhiResult<Box<dyn FrameUploader>> {
        Ok(Box::new(RefusingUploader))
    }

    fn submit(
        &mut self,
        _submit: &PlannedSubmit,
        _cpu_profile: Option<&mut SubmissionCpuProfile>,
        completion: Completion,
    ) -> RhiResult<()> {
        completion();
        Ok(())
    }

    fn destroy_buffer(&mut self, _handle: Buffer) {}

    fn destroy_texture(&mut self, _handle: Texture) {}

    fn destroy_sampler(&mut self, _handle: Sampler) {}

    fn destroy_shader_module(&mut self, _handle: ShaderModule) {}

    fn destroy_graphics_pipeline(&mut self, _handle: GraphicsPipeline) {}

    fn destroy_compute_pipeline(&mut self, _handle: ComputePipeline) {}

    fn wait_until_idle(&mut self) -> RhiResult<()> {
        let mut state = self.lock();
        if state.fail_next_idle {
            state.fail_next_idle = false;
            return Err(RhiError::submit_failed(
                "completed GPU work reported an error",
            ));
        }
        Ok(())
    }
}

fn context() -> (Arc<Device>, Arc<Mutex<PoolingState>>) {
    let backend = PoolingBackend::new();
    let state = backend.state();
    let device = Device::new(
        Box::new(backend),
        DeviceConfig {
            preferred_backends: vec![GraphicsApi::Metal],
            frames_in_flight: 1,
            ..DeviceConfig::default()
        },
    )
    .expect("the controlled-presentation device builds");
    (Arc::new(device), state)
}

fn complete(state: &Arc<Mutex<PoolingState>>) {
    state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .complete();
}

fn reject_next_idle(state: &Arc<Mutex<PoolingState>>) {
    state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .fail_next_idle = true;
}

fn reject_next_present(state: &Arc<Mutex<PoolingState>>) {
    state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .fail_next_present = true;
}

fn descriptor(id: usize) -> SurfaceDescriptor {
    let mut descriptor = SurfaceDescriptor::new(id);
    descriptor.width = 16;
    descriptor.height = 16;
    descriptor
}

#[test]
fn presentation_mode_follows_only_its_window_and_rejects_retired_handle() {
    let (device, state) = context();
    let first = device
        .make_swapchain(descriptor(1))
        .expect("first swapchain");
    let second = device
        .make_swapchain(descriptor(2))
        .expect("second swapchain");

    assert_eq!(
        device.presentation_mode(first).expect("first mode"),
        PresentationMode::Fifo,
        "a vsynced window presents fifo",
    );

    let mut unsynchronized = descriptor(1);
    unsynchronized.vsync_enabled = false;
    device
        .configure_swapchain(first, unsynchronized)
        .expect("the first window reconfigures");
    assert_eq!(
        device.presentation_mode(first).expect("reconfigured mode"),
        PresentationMode::Immediate,
        "the reconfigured window presents immediate",
    );
    assert_eq!(
        device.presentation_mode(second).expect("second mode"),
        PresentationMode::Fifo,
        "the other window is unaffected",
    );

    device.destroy_swapchain(first);
    assert!(
        device.presentation_mode(first).is_err(),
        "a retired handle has no presentation mode",
    );
    device.destroy_swapchain(second);
    device.wait_until_idle().expect("the device drains");
    assert_eq!(
        state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .window_count(),
        0,
        "both windows are released",
    );
}

#[test]
fn frame_sessions_serialize_preparation_and_release_after_errors() {
    let (device, _state) = context();

    // A failed preparation must not leave the session held: the error propagates
    // and a later session still acquires.
    let failed = device.with_frame_session(|| {
        Err::<(), _>(cove_graphics::RhiError::invalid_argument(
            "failed preparation",
        ))
    });
    assert!(failed.is_err(), "the preparation error reaches the caller",);

    // A nested session is refused while the outer one is active, and the outer
    // one still closes cleanly afterwards.
    device
        .with_frame_session(|| {
            device.begin_frame()?;
            assert!(
                device
                    .with_frame_session(|| {
                        device.begin_frame()?;
                        Ok(())
                    })
                    .is_err(),
                "a nested frame session is refused",
            );
            device.end_frame();
            Ok(())
        })
        .expect("the outer session completes");
}

#[test]
fn presentation_completion_protects_slot_reuse_and_owned_window_retirement() {
    let (device, state) = context();
    let resource = SwapchainResource::new(Arc::clone(&device), descriptor(1))
        .expect("the owned swapchain builds");
    let handle = resource.swapchain();

    device.begin_frame().expect("the frame opens");
    let image = device
        .acquire_swapchain_image(handle)
        .expect("the image acquires");
    device.present(image).expect("the presentation queues");
    drop(resource);
    device.end_frame();

    // The completion is still outstanding, so the window must stay alive.
    assert_eq!(
        state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .window_count(),
        1,
        "a queued presentation keeps its window",
    );
    assert_eq!(
        state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pending_count(),
        1,
        "the presentation is pending",
    );

    complete(&state);
    device.wait_until_idle().expect("the device drains");
    assert_eq!(
        state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .window_count(),
        0,
        "the window is released once its presentation completes",
    );
}

#[test]
fn abandoned_acquisition_is_returned_and_tracked() {
    let (device, state) = context();
    let handle = device
        .make_swapchain(descriptor(1))
        .expect("the swapchain builds");

    device.begin_frame().expect("the frame opens");
    let _abandoned = device
        .acquire_swapchain_image(handle)
        .expect("the image acquires");
    device.end_frame();
    assert_eq!(
        state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pending_count(),
        1,
        "the abandoned acquisition is tracked rather than leaked",
    );

    complete(&state);
    device.begin_frame().expect("the next frame opens");
    let next = device
        .acquire_swapchain_image(handle)
        .expect("the next image acquires");
    device.present(next).expect("the next presentation queues");
    device.end_frame();
    complete(&state);
    device.wait_until_idle().expect("the device drains");

    device.destroy_swapchain(handle);
    assert_eq!(
        state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .window_count(),
        0,
        "the window is released after both presentations completed",
    );
}

#[test]
fn old_ticket_is_rejected_when_the_same_pool_texture_is_acquired_again() {
    let (device, state) = context();
    let handle = device
        .make_swapchain(descriptor(1))
        .expect("the swapchain builds");

    device.begin_frame().expect("the first frame opens");
    let old = device
        .acquire_swapchain_image(handle)
        .expect("the first image acquires");
    device.present(old).expect("the first presentation queues");
    device.end_frame();
    complete(&state);

    device.begin_frame().expect("the second frame opens");
    let current = device
        .acquire_swapchain_image(handle)
        .expect("the pooled image is acquired again");
    assert_eq!(
        old.texture, current.texture,
        "the backend hands back the same pooled texture",
    );
    assert_eq!(
        old.generation, current.generation,
        "the recording backend keeps one generation per window",
    );

    // Swift's `SwapchainImage` compared by reference identity, so a re-acquired
    // image was a new ticket even though it names the same pooled texture with
    // the same per-window generation. Rust restores that with a per-acquisition
    // `serial` that participates in equality.
    assert_ne!(
        old, current,
        "a re-acquired image is a new ticket even when every other field matches",
    );
    assert!(
        device.present(old).is_err(),
        "the stale ticket from the previous frame is rejected",
    );

    device
        .present(current)
        .expect("the current ticket presents");
    device.end_frame();
    complete(&state);
    device.wait_until_idle().expect("the device drains");
    device.destroy_swapchain(handle);
}

#[test]
fn rejected_present_can_retry_without_duplicating_the_completion() {
    let (device, state) = context();
    let handle = device
        .make_swapchain(descriptor(1))
        .expect("the swapchain builds");

    device.begin_frame().expect("the frame opens");
    let image = device
        .acquire_swapchain_image(handle)
        .expect("the image acquires");

    // A present refused before queueing must leave the ticket usable, so the
    // caller can retry without having lost the frame's image.
    reject_next_present(&state);
    assert!(
        device.present(image).is_err(),
        "the refused present reports its error",
    );
    assert_eq!(
        state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pending_count(),
        0,
        "a refused present queues no completion",
    );

    device
        .present(image)
        .expect("the same ticket presents after the retry");
    assert!(
        device.present(image).is_err(),
        "a second present of the same ticket is refused",
    );
    assert!(
        device.acquire_swapchain_image(handle).is_err(),
        "a window is acquired at most once per frame",
    );
    device.end_frame();
    assert_eq!(
        state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pending_count(),
        1,
        "the retry queued exactly one completion",
    );
    complete(&state);
    device.wait_until_idle().expect("the device drains");
    device.destroy_swapchain(handle);
}

#[test]
fn owned_close_is_idempotent_and_cleans_up_after_completed_work_errors() {
    let (device, state) = context();
    let resource = SwapchainResource::new(Arc::clone(&device), descriptor(1))
        .expect("the owned swapchain builds");

    // Closing while a frame is open cannot retire the window yet, so it reports
    // that and leaves the window in place.
    device.begin_frame().expect("the frame opens");
    let mut resource = resource;
    assert!(resource.close().is_err(), "closing mid-frame is deferred",);
    assert_eq!(
        state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .window_count(),
        1,
        "the deferred close keeps the window",
    );
    device.end_frame();

    // A completed-work error surfaces from `wait_until_idle` but must not block
    // the eventual close.
    reject_next_idle(&state);
    let idle = device.wait_until_idle();
    assert!(idle.is_err(), "the completed-work error is reported");
    resource.close().expect("the close succeeds afterwards");
    resource.close().expect("a second close is idempotent");
    assert_eq!(
        state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .window_count(),
        0,
        "the window is released exactly once",
    );
    device.wait_until_idle().expect("the device drains");
}
