//! Translation of NativeRHI/Submission/{Plan,FrameRing,UploadRing,
//! SubmissionCPUProfile,GPUTimestampProfile}.swift.
//!
//! The submission planner turns a recorded command list into one or more
//! `PlannedSubmit`s. The frame ring tracks which frames-in-flight slots are
//! still owned by the GPU, and the upload ring suballocates GPU-visible storage
//! per slot.
use std::collections::HashMap;
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::Instant;

use super::{
    AccelerationStructureBuild, BarrierCommand, Buffer, ComputePassRecord, CopyPassRecord,
    QueueClass, RenderPassRecord, RhiError, RhiResult, Swapchain, Texture, TimelineSemaphore,
};

// MARK: - Plan

/// A buffer/slot pair produced by the per-frame upload ring.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UploadLocation {
    pub buffer: Buffer,
    pub offset: usize,
}
impl UploadLocation {
    pub const fn new(buffer: Buffer, offset: usize) -> Self {
        Self { buffer, offset }
    }
}

/// Per-slot transient storage the backend vends to the frontend. Each
/// frames-in-flight slot owns one uploader; it is reset only when that slot is
/// reused (after the GPU finished the prior frame), so allocations stay alive
/// for exactly the frame that uses them.
///
/// Swift declares two `write` overloads (one taking `Data`, one taking an
/// `UnsafeRawBufferPointer` that defaults to wrapping the bytes). A Rust `&[u8]`
/// covers both: the backend copies from the slice before returning.
pub trait FrameUploader: Send {
    /// Returns the uploader to a zero-used state. Called when a frame begins.
    fn reset(&mut self);
    /// Suballocates from GPU-visible ring storage and copies `bytes` there.
    fn write(&mut self, bytes: &[u8], alignment: usize) -> RhiResult<UploadLocation>;
}

/// A distinct immutable acquisition ticket. The borrowed `texture` is valid
/// through this frame; retaining a ticket does not make it a persistent image.
/// Backend texture IDs may be recycled, but old tickets never become current.
///
/// Swift distinguishes tickets by *reference identity* (`Plan.swift:51`,
/// `lhs === rhs`). `generation` alone cannot stand in for that: every backend
/// sets it per *window*, not per acquisition, so two acquisitions of the same
/// pooled texture were fully field-equal and a stale ticket could pass the
/// presentation guard and consume the current frame. `serial` is unique per
/// acquisition and participates in equality, which restores the Swift semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SwapchainImage {
    pub swapchain: Swapchain,
    pub generation: u64,
    pub texture: Texture,
    pub width: usize,
    pub height: usize,
    /// Per-acquisition identity. Never compared against a saved value; only two
    /// tickets created by the same acquisition share it.
    pub serial: u64,
}
impl SwapchainImage {
    pub fn new(
        swapchain: Swapchain,
        generation: u64,
        texture: Texture,
        width: usize,
        height: usize,
    ) -> Self {
        Self {
            swapchain,
            generation,
            texture,
            width,
            height,
            serial: next_acquisition_serial(),
        }
    }
}

/// Monotonic per-process acquisition counter. Distinct acquisitions never share
/// a serial, which is what `SwapchainImage` equality relies on.
fn next_acquisition_serial() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// One linear command stream for a single queue submission. Barriers are
/// represented as explicit `Barriers` blocks interspersed with passes.
#[derive(Clone, Debug, PartialEq)]
pub enum PlannedCommand {
    AccelerationStructureBuild(AccelerationStructureBuild),
    RenderPass(RenderPassRecord),
    ComputePass(ComputePassRecord),
    CopyPass(CopyPassRecord),
    Barriers(Vec<BarrierCommand>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlannedSubmit {
    pub queue: QueueClass,
    pub commands: Vec<PlannedCommand>,
    pub wait_semaphores: Vec<TimelineSemaphore>,
    pub signal_semaphores: Vec<TimelineSemaphore>,
}
impl PlannedSubmit {
    pub fn new(queue: QueueClass, commands: Vec<PlannedCommand>) -> Self {
        Self {
            queue,
            commands,
            wait_semaphores: Vec::new(),
            signal_semaphores: Vec::new(),
        }
    }
}

/// The complete result of planning one frontend submission. It is ordered for
/// execution: release submits (cross-queue) first, then the primary submit.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SubmitPlan {
    pub submits: Vec<PlannedSubmit>,
}
impl SubmitPlan {
    pub fn new(submits: Vec<PlannedSubmit>) -> Self {
        Self { submits }
    }
    pub fn is_empty(&self) -> bool {
        self.submits.is_empty()
    }
}

// MARK: - Frame ring

struct SlotState {
    in_flight: bool,
    pending_command_buffers: usize,
    sealed: bool,
    epoch: u64,
    completed_epoch: u64,
    timing: Option<FrameMeasurement>,
}

/// One frame's GPU duration, filled asynchronously when its slot retires.
#[derive(Clone, Debug, Default)]
pub struct GpuFrameTiming(std::sync::Arc<Mutex<Option<Option<u64>>>>);

impl GpuFrameTiming {
    pub fn result(&self) -> Option<Option<u64>> {
        *self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[derive(Default)]
struct FrameMeasurement {
    result: GpuFrameTiming,
    elapsed: u64,
    has_sample: bool,
    incomplete: bool,
}

/// A deferred-destruction request expressed only as the set of frame-slot
/// epochs that must complete before it fires. The actual (backend-touching)
/// closure lives on `Device`, which is single-threaded for Metal, so it must
/// not be stored here: `FrameRing` is `Sync` and shared with the GPU completion
/// thread, which only updates slot state.
struct RetirementRecord {
    epochs: HashMap<usize, u64>,
    id: u64,
}

struct FrameRingState {
    slots: Vec<SlotState>,
    current: Option<usize>,
    epoch: u64,
    completed_frames: u64,
    retirements: Vec<RetirementRecord>,
}

/// Frame completion requires both `end` and completion of every submission.
/// A condition variable avoids accumulating stale semaphore signals on slot
/// reuse. The Swift original guards this with `NSCondition`; the Rust port uses
/// a `Mutex` plus `Condvar` with the same wait predicates.
pub struct FrameRing {
    state: Mutex<FrameRingState>,
    condition: Condvar,
}
impl FrameRing {
    pub fn new(slot_count: usize) -> Self {
        assert!(slot_count >= 1, "a frame ring needs at least one slot");
        let slots = (0..slot_count)
            .map(|_| SlotState {
                in_flight: false,
                pending_command_buffers: 0,
                sealed: false,
                epoch: 0,
                completed_epoch: 0,
                timing: None,
            })
            .collect();
        Self {
            state: Mutex::new(FrameRingState {
                slots,
                current: None,
                epoch: 0,
                completed_frames: 0,
                retirements: Vec::new(),
            }),
            condition: Condvar::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, FrameRingState> {
        // A poisoned lock means another thread panicked while holding it. The
        // frame ring holds no invariants that a panic could corrupt beyond the
        // slot counters, so recovering keeps the render thread alive.
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn slot_count(&self) -> usize {
        self.lock().slots.len()
    }

    pub fn current_slot_index(&self) -> Option<usize> {
        self.lock().current
    }

    pub fn completed_frames(&self) -> u64 {
        self.lock().completed_frames
    }

    /// Opens the next frame and returns its slot index plus the retirement
    /// identifiers that became runnable while opening it.
    pub fn begin(&self) -> (usize, Vec<u64>) {
        self.begin_slot(true).expect("blocking frame begin has a slot")
    }

    pub fn try_begin(&self) -> Option<(usize, Vec<u64>)> {
        self.begin_slot(false)
    }

    fn begin_slot(&self, wait: bool) -> Option<(usize, Vec<u64>)> {
        let mut state = self.lock();
        assert!(
            state.current.is_none(),
            "end the current frame before beginning another"
        );
        let slot_count = state.slots.len();
        let index = (state.epoch % slot_count as u64) as usize;
        while state.slots[index].in_flight {
            if !wait {
                return None;
            }
            state = self
                .condition
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        let ready = drain_retirements(&mut state);
        state.epoch += 1;
        let epoch = state.epoch;
        let slot = &mut state.slots[index];
        slot.epoch = epoch;
        slot.in_flight = true;
        slot.sealed = false;
        slot.pending_command_buffers = 0;
        state.current = Some(index);
        drop(state);
        Some((index, ready))
    }

    /// Seals the current frame and returns retirements that became runnable.
    pub fn end(&self) -> Vec<u64> {
        let mut state = self.lock();
        let Some(index) = state.current else {
            return Vec::new();
        };
        state.slots[index].sealed = true;
        state.current = None;
        finish_if_ready(&mut state, index);
        self.condition.notify_all();
        let ready = drain_retirements(&mut state);
        drop(state);
        ready
    }

    /// Registers a deferred-destruction request identified by `id`, keyed to
    /// every slot currently in flight, and returns any retirements that became
    /// runnable (possibly including `id` itself when nothing is in flight).
    pub fn register_retirement(&self, id: u64) -> Vec<u64> {
        let mut state = self.lock();
        let epochs = state
            .slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.in_flight)
            .map(|(index, slot)| (index, slot.epoch))
            .collect();
        state.retirements.push(RetirementRecord { epochs, id });
        let ready = drain_retirements(&mut state);
        drop(state);
        ready
    }

    pub fn register_command_buffer(&self, slot_index: usize) {
        let mut state = self.lock();
        let slot = &state.slots[slot_index];
        assert!(
            slot.in_flight && !slot.sealed,
            "register a command buffer inside an open frame"
        );
        state.slots[slot_index].pending_command_buffers += 1;
    }

    pub(crate) fn measure(&self, slot_index: usize) -> GpuFrameTiming {
        let mut state = self.lock();
        let slot = &mut state.slots[slot_index];
        slot.timing.get_or_insert_with(FrameMeasurement::default).result.clone()
    }

    pub(crate) fn measurement_epoch(&self, slot_index: usize) -> Option<u64> {
        let state = self.lock();
        let slot = &state.slots[slot_index];
        slot.timing.as_ref().map(|_| slot.epoch)
    }

    pub(crate) fn note_gpu_sample(&self, slot_index: usize, epoch: u64, nanos: Option<u64>) {
        let mut state = self.lock();
        let slot = &mut state.slots[slot_index];
        if slot.epoch == epoch && let Some(timing) = &mut slot.timing {
            if let Some(elapsed) = nanos.and_then(|value| timing.elapsed.checked_add(value)) {
                timing.elapsed = elapsed;
                timing.has_sample = true;
            } else {
                timing.incomplete = true;
            }
        }
    }

    pub fn unregister_command_buffer(&self, slot_index: usize) -> Vec<u64> {
        self.complete_command_buffer(slot_index);
        drain_retirements(&mut self.lock())
    }

    /// Notes GPU completion of a command buffer and returns retirements that
    /// became runnable. Safe to call from the GPU completion thread: it only
    /// mutates slot state under the ring lock and never runs closures.
    pub fn command_buffer_completed(&self, slot_index: usize) -> Vec<u64> {
        self.complete_command_buffer(slot_index)
    }

    fn complete_command_buffer(&self, slot_index: usize) -> Vec<u64> {
        let mut state = self.lock();
        assert!(
            state.slots[slot_index].pending_command_buffers > 0,
            "no command buffer is registered for this slot"
        );
        state.slots[slot_index].pending_command_buffers -= 1;
        finish_if_ready(&mut state, slot_index);
        self.condition.notify_all();
        // Retirements remain in the ring until the API thread drains them.
        // Observing completed slots and extracting IDs therefore use one lock.
        drop(state);
        Vec::new()
    }

    /// Blocks until every slot has retired, returning retirements that became
    /// runnable.
    pub fn wait_until_idle(&self) -> Vec<u64> {
        let mut state = self.lock();
        assert!(
            state.current.is_none(),
            "end the active frame before waiting for idle"
        );
        while state.slots.iter().any(|slot| slot.in_flight) {
            state = self
                .condition
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        let ready = drain_retirements(&mut state);
        drop(state);
        ready
    }

    pub fn is_idle(&self) -> bool {
        !self.lock().slots.iter().any(|slot| slot.in_flight)
    }
}

fn finish_if_ready(state: &mut FrameRingState, index: usize) {
    let slot = &state.slots[index];
    if !(slot.in_flight && slot.sealed && slot.pending_command_buffers == 0) {
        return;
    }
    // GPU callbacks only update completion state. Destruction is drained on the
    // API thread under the device's lock, never against live registries.
    let epoch = slot.epoch;
    let slot = &mut state.slots[index];
    if let Some(timing) = slot.timing.take() {
        let nanos = (timing.has_sample && !timing.incomplete).then_some(timing.elapsed);
        *timing.result.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(nanos);
    }
    slot.completed_epoch = epoch;
    slot.in_flight = false;
    state.completed_frames += 1;
}

/// Removes every retirement whose slots have all completed and returns the
/// actions to run. The caller releases the lock before invoking them so a
/// destruction callback cannot deadlock against the ring.
fn drain_retirements(state: &mut FrameRingState) -> Vec<u64> {
    let mut ready = Vec::new();
    let mut pending = Vec::with_capacity(state.retirements.len());
    for retirement in state.retirements.drain(..) {
        let complete = retirement
            .epochs
            .iter()
            .all(|(index, epoch)| state.slots[*index].completed_epoch >= *epoch);
        if complete {
            ready.push(retirement.id);
        } else {
            pending.push(retirement);
        }
    }
    state.retirements = pending;
    ready
}

// MARK: - Upload ring

/// Backend-provided chunk of GPU-visible storage. On Metal this wraps a
/// shared-storage `MTLBuffer` whose contents pointer is directly writable.
pub trait UploadRingChunk {
    fn capacity(&self) -> usize;
    /// Base pointer of the chunk's CPU-visible mapping.
    ///
    /// # Safety
    /// The pointer is valid for `capacity()` bytes for as long as the chunk is
    /// alive. Callers must not write past `capacity()` and must not hold the
    /// pointer after the chunk is dropped.
    unsafe fn cpu_pointer(&self) -> *mut u8;
}

/// One suballocation inside a ring chunk.
pub struct UploadAllocation<C> {
    chunk: C,
    offset: usize,
}
impl<C: UploadRingChunk> UploadAllocation<C> {
    pub fn chunk(&self) -> &C {
        &self.chunk
    }
    pub const fn offset(&self) -> usize {
        self.offset
    }
    /// Writable pointer at this allocation's start.
    ///
    /// # Safety
    /// The caller must stay within the chunk's remaining capacity and must not
    /// outlive the chunk.
    pub unsafe fn pointer(&self) -> *mut u8 {
        // SAFETY: the chunk is alive for as long as this allocation, and the
        // allocator only hands out offsets whose end is within `capacity()`.
        unsafe { self.chunk.cpu_pointer().add(self.offset) }
    }
}

struct ChunkEntry<C> {
    chunk: C,
    used: usize,
    capacity: usize,
}

/// Per-frame chunked upload allocator. Chunks are reset when the slot is reused
/// (which only happens after the GPU has finished with that slot), so every
/// suballocation is automatically kept alive until the frame that uses it
/// completes.
pub struct ChunkUploadAllocator<C> {
    factory: Box<dyn FnMut(usize) -> RhiResult<C> + Send>,
    default_chunk_size: usize,
    entries: Vec<ChunkEntry<C>>,
    current_index: Option<usize>,
}
impl<C: UploadRingChunk + Clone> ChunkUploadAllocator<C> {
    pub fn new(
        default_chunk_size: usize,
        factory: impl FnMut(usize) -> RhiResult<C> + Send + 'static,
    ) -> Self {
        Self {
            factory: Box::new(factory),
            default_chunk_size: default_chunk_size.max(64 * 1024),
            entries: Vec::new(),
            current_index: None,
        }
    }

    /// Called when the owning frame slot begins (GPU work from the previous use
    /// of the slot has already been waited on).
    pub fn reset(&mut self) {
        for entry in &mut self.entries {
            entry.used = 0;
        }
        self.current_index = if self.entries.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    pub fn total_chunk_capacity(&self) -> usize {
        self.entries.iter().map(|entry| entry.capacity).sum()
    }

    pub fn chunk_count(&self) -> usize {
        self.entries.len()
    }

    pub fn allocate(&mut self, size: usize, alignment: usize) -> RhiResult<UploadAllocation<C>> {
        if size == 0 {
            return Err(RhiError::invalid_argument("upload size must be > 0"));
        }
        let alignment = alignment.max(1);

        if let Some(index) = self.current_index
            && let Some(allocation) = self.attempt(index, size, alignment)
        {
            return Ok(allocation);
        }
        for index in 0..self.entries.len() {
            if Some(index) == self.current_index {
                continue;
            }
            if let Some(allocation) = self.attempt(index, size, alignment) {
                self.current_index = Some(index);
                return Ok(allocation);
            }
        }

        let capacity = self.default_chunk_size.max(align(size, alignment));
        let chunk = (self.factory)(capacity)?;
        if chunk.capacity() < capacity {
            return Err(RhiError::invalid_argument(
                "upload chunk smaller than requested",
            ));
        }
        self.entries.push(ChunkEntry {
            chunk,
            used: 0,
            capacity,
        });
        let index = self.entries.len() - 1;
        self.current_index = Some(index);
        self.attempt(index, size, alignment)
            .ok_or_else(|| RhiError::invalid_argument("upload chunk smaller than requested"))
    }

    /// Copies `bytes` straight into ring storage. Callers that already own
    /// contiguous memory use this to skip building an intermediate buffer.
    pub fn write(&mut self, bytes: &[u8], alignment: usize) -> RhiResult<UploadAllocation<C>> {
        let allocation = self.allocate(bytes.len(), alignment)?;
        if !bytes.is_empty() {
            // SAFETY: `allocate` returned a range of `bytes.len()` writable
            // bytes inside a live chunk, and `bytes` is a distinct source
            // region, so the two ranges cannot overlap.
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), allocation.pointer(), bytes.len());
            }
        }
        Ok(allocation)
    }

    fn attempt(
        &mut self,
        index: usize,
        size: usize,
        alignment: usize,
    ) -> Option<UploadAllocation<C>> {
        let entry = &mut self.entries[index];
        let aligned_offset = align(entry.used, alignment);
        let end = aligned_offset.checked_add(size)?;
        if end > entry.capacity {
            return None;
        }
        entry.used = end;
        Some(UploadAllocation {
            chunk: entry.chunk.clone(),
            offset: aligned_offset,
        })
    }
}

/// Rounds `size` up to the next multiple of `alignment`.
pub fn align(size: usize, alignment: usize) -> usize {
    let alignment = alignment.max(1);
    size.div_ceil(alignment) * alignment
}

// MARK: - CPU profiling

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CpuProfilePhase {
    Validation,
    Planning,
    Encoding,
    QueueSubmit,
    PassSetup,
    EncoderCreation,
    CommandReplay,
    BindingApply,
}

/// Optional synchronous CPU instrumentation for one or more submissions.
/// Use a fresh instance per sample; read it after `Device::submit` returns.
/// These durations exclude GPU completion and frame-slot waits. Completed
/// phases accumulate; a phase that throws may have no sample.
///
/// The encoding sub-phases stay zero unless the profile is created with
/// `records_encoding_detail`, so ordinary sampling keeps its original clocks.
#[derive(Clone, Debug)]
pub struct SubmissionCpuProfile {
    pub validation_nanoseconds: u64,
    pub planning_nanoseconds: u64,
    pub encoding_nanoseconds: u64,
    pub queue_submit_nanoseconds: u64,
    /// Attachment validation and native pass-descriptor construction.
    pub pass_setup_nanoseconds: u64,
    /// Creating native command encoders.
    pub encoder_creation_nanoseconds: u64,
    /// Replaying recorded commands into native encoders.
    pub command_replay_nanoseconds: u64,
    /// Applying binding sets inside `command_replay`.
    pub binding_apply_nanoseconds: u64,
    pub records_encoding_detail: bool,
}
impl SubmissionCpuProfile {
    pub fn new(records_encoding_detail: bool) -> Self {
        Self {
            validation_nanoseconds: 0,
            planning_nanoseconds: 0,
            encoding_nanoseconds: 0,
            queue_submit_nanoseconds: 0,
            pass_setup_nanoseconds: 0,
            encoder_creation_nanoseconds: 0,
            command_replay_nanoseconds: 0,
            binding_apply_nanoseconds: 0,
            records_encoding_detail,
        }
    }

    pub fn begin(&self) -> Option<Instant> {
        Some(Instant::now())
    }

    pub fn end(&mut self, phase: CpuProfilePhase, since: Option<Instant>) {
        let Some(start) = since else { return };
        self.end_elapsed(phase, start.elapsed().as_nanos() as u64);
    }

    /// Detail clocks are opt-in: reads zero when detail recording is off.
    pub fn begin_detail(&self, _phase: CpuProfilePhase) -> Option<Instant> {
        if self.records_encoding_detail {
            Some(Instant::now())
        } else {
            None
        }
    }

    pub fn end_detail(&mut self, phase: CpuProfilePhase, since: Option<Instant>) {
        self.end(phase, since);
    }

    fn end_elapsed(&mut self, phase: CpuProfilePhase, elapsed: u64) {
        let slot = match phase {
            CpuProfilePhase::Validation => &mut self.validation_nanoseconds,
            CpuProfilePhase::Planning => &mut self.planning_nanoseconds,
            CpuProfilePhase::Encoding => &mut self.encoding_nanoseconds,
            CpuProfilePhase::QueueSubmit => &mut self.queue_submit_nanoseconds,
            CpuProfilePhase::PassSetup => &mut self.pass_setup_nanoseconds,
            CpuProfilePhase::EncoderCreation => &mut self.encoder_creation_nanoseconds,
            CpuProfilePhase::CommandReplay => &mut self.command_replay_nanoseconds,
            CpuProfilePhase::BindingApply => &mut self.binding_apply_nanoseconds,
        };
        *slot = slot.saturating_add(elapsed);
    }
}

// MARK: - GPU timing

/// One measured interval between two adjacent GPU timestamps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GpuTimestampMarker {
    pub label: String,
    pub nanoseconds: u64,
}
impl GpuTimestampMarker {
    pub fn new(label: impl Into<String>, nanoseconds: u64) -> Self {
        Self {
            label: label.into(),
            nanoseconds,
        }
    }
}

#[derive(Default)]
struct GpuTimestampState {
    markers: Vec<GpuTimestampMarker>,
    completed: usize,
    status: String,
}

/// Optional asynchronous GPU timing for queued submissions.
///
/// Backends that cannot timestamp leave the profile untouched; callers check
/// `Device::supports_gpu_timestamps` before attaching one. Results reach the
/// profile from the backend's completion thread once the GPU retires the work,
/// so read them after the sampled frames have retired rather than right after
/// submit.
///
/// Values cover GPU execution only. Queue wait, swapchain acquire and present
/// are not included, and an unfinished marker is omitted rather than reported
/// as zero.
pub struct GpuTimestampProfile {
    state: Mutex<GpuTimestampState>,
    marker_limit: usize,
}
impl GpuTimestampProfile {
    /// Bounded so an unbounded session cannot grow the sample list for ever.
    pub fn new(marker_limit: usize) -> Self {
        Self {
            state: Mutex::new(GpuTimestampState {
                status: "idle".to_owned(),
                ..Default::default()
            }),
            marker_limit,
        }
    }

    fn lock(&self) -> MutexGuard<'_, GpuTimestampState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Why recording is or is not producing markers. `ok` means the backend is
    /// resolving queries; any other value names the gate that stopped it, which
    /// is the difference between "slow" and "not measured".
    pub fn status(&self) -> String {
        self.lock().status.clone()
    }

    pub fn note(&self, status: impl Into<String>) {
        self.lock().status = status.into();
    }

    pub fn append(&self, marker: GpuTimestampMarker) {
        let mut state = self.lock();
        state.completed += 1;
        // Retention is bounded but the count is not, so a caller can tell a
        // complete report from one that stopped early.
        if state.markers.len() >= self.marker_limit {
            return;
        }
        state.markers.push(marker);
    }

    /// Markers retained so far, in completion order. Shorter than
    /// [`Self::sample_count`] once the limit was reached, which is how
    /// truncation becomes visible.
    pub fn recorded_markers(&self) -> Vec<GpuTimestampMarker> {
        self.lock().markers.clone()
    }

    /// Total markers completed, including any not retained past the limit.
    pub fn sample_count(&self) -> usize {
        self.lock().completed
    }

    pub fn reset(&self) {
        let mut state = self.lock();
        state.markers.clear();
        state.completed = 0;
    }
}
impl Default for GpuTimestampProfile {
    fn default() -> Self {
        Self::new(600)
    }
}

/// A backend able to timestamp GPU execution for the submissions it queues.
///
/// Conformance is optional so that backends without timestamp support keep
/// their existing submission path unchanged.
pub trait GpuTimestampRecording {
    /// The profile receiving results, or `None` to record nothing. Setting it
    /// does no GPU work on its own; recording starts with the next submission.
    fn gpu_timestamp_profile(&self) -> Option<std::sync::Arc<GpuTimestampProfile>>;
    fn set_gpu_timestamp_profile(&mut self, profile: Option<std::sync::Arc<GpuTimestampProfile>>);
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeChunk {
        storage: Vec<u8>,
    }
    impl Clone for FakeChunk {
        fn clone(&self) -> Self {
            // Cloning shares nothing in the real backends either: they wrap a
            // refcounted GPU object. A fresh allocation keeps the test honest
            // about capacity accounting only.
            Self {
                storage: self.storage.clone(),
            }
        }
    }
    impl UploadRingChunk for FakeChunk {
        fn capacity(&self) -> usize {
            self.storage.len()
        }
        unsafe fn cpu_pointer(&self) -> *mut u8 {
            self.storage.as_ptr() as *mut u8
        }
    }

    #[test]
    fn alignment_rounds_up() {
        assert_eq!(align(0, 256), 0);
        assert_eq!(align(1, 256), 256);
        assert_eq!(align(256, 256), 256);
        assert_eq!(align(257, 256), 512);
        assert_eq!(align(5, 0), 5);
    }

    #[test]
    fn the_upload_allocator_reuses_and_grows_chunks() {
        // `ChunkUploadAllocator::new` raises the requested chunk size to a 64 KiB
        // GPU minimum, so the test uses realistic sizes rather than a 1024-byte
        // chunk that can never be created.
        let chunk = 64 * 1024;
        let mut allocator = ChunkUploadAllocator::new(chunk, |capacity| {
            Ok(FakeChunk {
                storage: vec![0; capacity],
            })
        });
        let first = allocator.allocate(100, 16).expect("first");
        assert_eq!(first.offset(), 0);
        let second = allocator.allocate(100, 16).expect("second");
        assert_eq!(second.offset(), 112);
        assert_eq!(allocator.chunk_count(), 1);
        // A request that cannot fit the *remainder* of the current chunk opens
        // another one. Two 100-byte allocations used 112 bytes, leaving 65 424;
        // 65 440 is the smallest aligned request above that, and still below the
        // default size so the new chunk is exactly `chunk` bytes rather than
        // doubling.
        let large = allocator.allocate(chunk - 96, 16).expect("large");
        assert_eq!(large.offset(), 0, "a new chunk starts at zero");
        assert_eq!(allocator.chunk_count(), 2);
        // The second chunk is also `chunk` bytes, so the total is two of them.
        assert_eq!(allocator.total_chunk_capacity(), chunk * 2);
        allocator.reset();
        assert_eq!(allocator.allocate(16, 16).expect("after reset").offset(), 0);
        assert!(allocator.allocate(0, 16).is_err());
    }

    #[test]
    fn a_busy_frame_slot_can_be_retried_without_waiting() {
        let ring = FrameRing::new(1);
        let (slot, _) = ring.try_begin().expect("first frame");
        ring.register_command_buffer(slot);
        ring.end();
        assert!(!ring.is_idle());
        assert!(ring.try_begin().is_none());
        ring.command_buffer_completed(slot);
        assert!(ring.is_idle());
        assert!(ring.try_begin().is_some());
        ring.end();
    }

    #[test]
    fn the_frame_ring_retires_only_completed_epochs() {
        let ring = FrameRing::new(2);
        let (slot, ready) = ring.begin();
        assert!(ready.is_empty());
        assert_eq!(ring.current_slot_index(), Some(slot));
        ring.register_command_buffer(slot);
        assert!(ring.register_retirement(7).is_empty());
        assert!(ring.end().is_empty());
        assert!(ring.command_buffer_completed(slot).is_empty());
        assert_eq!(ring.completed_frames(), 1);
        assert_eq!(ring.wait_until_idle(), vec![7]);
        assert_eq!(ring.register_retirement(8), vec![8]);
    }

    #[test]
    fn cpu_profile_only_records_detail_when_asked() {
        let mut profile = SubmissionCpuProfile::new(false);
        let start = profile.begin();
        profile.end(CpuProfilePhase::Validation, start);
        assert!(
            profile
                .begin_detail(CpuProfilePhase::BindingApply)
                .is_none()
        );
        let mut detailed = SubmissionCpuProfile::new(true);
        let detail_start = detailed.begin_detail(CpuProfilePhase::BindingApply);
        assert!(detail_start.is_some());
        detailed.end_detail(CpuProfilePhase::BindingApply, detail_start);
    }

    #[test]
    fn gpu_markers_stop_retaining_at_the_limit() {
        let profile = GpuTimestampProfile::new(2);
        for index in 0..4 {
            profile.append(GpuTimestampMarker::new("pass", index as u64));
        }
        assert_eq!(profile.sample_count(), 4);
        assert_eq!(profile.recorded_markers().len(), 2);
        profile.note("ok");
        assert_eq!(profile.status(), "ok");
        profile.reset();
        assert_eq!(profile.sample_count(), 0);
    }
}
