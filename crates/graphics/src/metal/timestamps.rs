//! Submission-owned Metal timestamp samples.

use std::ptr::NonNull;

use objc2::{Message, rc::Retained, runtime::ProtocolObject};
use objc2_foundation::NSRange;
use objc2_metal::{
    MTLAccelerationStructurePassDescriptor, MTLBlitPassDescriptor, MTLComputePassDescriptor,
    MTLCounterSampleBuffer, MTLCounterSampleBufferDescriptor, MTLCounterSamplingPoint,
    MTLCounterSet, MTLDevice, MTLRenderPassDescriptor, MTLStorageMode,
};

const STRIDE: usize = 4;

pub(super) struct SubmissionTimestamps {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    buffer: Retained<ProtocolObject<dyn MTLCounterSampleBuffer>>,
    stage_boundaries: bool,
    start: (u64, u64),
    samples: Vec<PassSampling>,
}

#[derive(Clone, Copy)]
pub(super) enum PassSampling {
    Unavailable,
    Stages(usize),
    Commands(usize),
    RenderStages(usize),
}

impl SubmissionTimestamps {
    pub(super) fn new(device: &ProtocolObject<dyn MTLDevice>, count: usize) -> Option<Self> {
        if count == 0 {
            return None;
        }
        let stage_boundaries =
            device.supportsCounterSampling(MTLCounterSamplingPoint::AtStageBoundary);
        let sets = device.counterSets()?;
        // SAFETY: Metal provides this immutable string for the lifetime of the process.
        let name = unsafe { objc2_metal::MTLCommonCounterSetTimestamp };
        let counter = sets.iter().find(|set| set.name().isEqualToString(name))?;
        let descriptor = MTLCounterSampleBufferDescriptor::new();
        descriptor.setCounterSet(Some(&counter));
        descriptor.setStorageMode(MTLStorageMode::Shared);
        // SAFETY: STRIDE reserves the maximum number of samples used by a pass.
        unsafe { descriptor.setSampleCount(count * STRIDE) };
        let buffer = device
            .newCounterSampleBufferWithDescriptor_error(&descriptor)
            .ok()?;
        Some(Self {
            device: device.retain(),
            buffer,
            stage_boundaries,
            start: clock_pair(device),
            samples: vec![PassSampling::Unavailable; count],
        })
    }

    pub(super) fn pass(&mut self, index: usize, boundary: MTLCounterSamplingPoint) -> PassSampling {
        let Some(sample) = self.samples.get_mut(index) else {
            return PassSampling::Unavailable;
        };
        let first = index * STRIDE;
        *sample = if self.stage_boundaries {
            if boundary == MTLCounterSamplingPoint::AtDrawBoundary {
                PassSampling::RenderStages(first)
            } else {
                PassSampling::Stages(first)
            }
        } else if self.device.supportsCounterSampling(boundary) {
            PassSampling::Commands(first)
        } else {
            PassSampling::Unavailable
        };
        *sample
    }

    pub(super) fn buffer(&self) -> &ProtocolObject<dyn MTLCounterSampleBuffer> {
        &self.buffer
    }

    pub(super) fn read(&self) -> Vec<Option<u64>> {
        let (cpu, gpu) = clock_pair(&self.device);
        let cpu_span = cpu.saturating_sub(self.start.0);
        let gpu_span = gpu.saturating_sub(self.start.1);
        if cpu_span == 0 || gpu_span == 0 {
            return vec![None; self.samples.len()];
        }
        // SAFETY: the command buffer completed and the range covers this buffer.
        let Some(data) = (unsafe {
            self.buffer
                .resolveCounterRange(NSRange::new(0, self.samples.len() * STRIDE))
        }) else {
            return vec![None; self.samples.len()];
        };
        // SAFETY: Metal returns immutable, resolved NSData.
        let bytes = unsafe { data.as_bytes_unchecked() };
        let tick = |index: usize| {
            let offset = index * size_of::<u64>();
            let raw = u64::from_ne_bytes(
                bytes
                    .get(offset..offset + size_of::<u64>())?
                    .try_into()
                    .ok()?,
            );
            (raw != u64::MAX && raw >= self.start.1 && raw <= gpu).then_some(raw)
        };
        self.samples
            .iter()
            .map(|sample| {
                let (start, end) = match *sample {
                    PassSampling::Unavailable => return None,
                    PassSampling::Stages(first) | PassSampling::Commands(first) => {
                        (tick(first)?, tick(first + 1)?)
                    }
                    PassSampling::RenderStages(first) => {
                        let vertex = tick(first).zip(tick(first + 1));
                        let fragment = tick(first + 2).zip(tick(first + 3));
                        match (vertex, fragment) {
                            (Some((start, _)), Some((_, end))) => (start, end),
                            (Some(pair), None) | (None, Some(pair)) => pair,
                            (None, None) => return None,
                        }
                    }
                };
                let elapsed = end.checked_sub(start)?;
                u64::try_from(u128::from(elapsed) * u128::from(cpu_span) / u128::from(gpu_span))
                    .ok()
            })
            .collect()
    }
}

fn clock_pair(device: &ProtocolObject<dyn MTLDevice>) -> (u64, u64) {
    let (mut cpu, mut gpu) = (0, 0);
    // SAFETY: both output pointers are valid for the call.
    unsafe {
        device.sampleTimestamps_gpuTimestamp(NonNull::from(&mut cpu), NonNull::from(&mut gpu))
    };
    (cpu, gpu)
}

pub(super) fn attach_render(
    descriptor: &MTLRenderPassDescriptor,
    buffer: &ProtocolObject<dyn MTLCounterSampleBuffer>,
    first: usize,
) {
    // SAFETY: Metal provides attachment 0 and the submission reserves four indices.
    unsafe {
        let attachment = descriptor
            .sampleBufferAttachments()
            .objectAtIndexedSubscript(0);
        attachment.setSampleBuffer(Some(buffer));
        attachment.setStartOfVertexSampleIndex(first);
        attachment.setEndOfVertexSampleIndex(first + 1);
        attachment.setStartOfFragmentSampleIndex(first + 2);
        attachment.setEndOfFragmentSampleIndex(first + 3);
    }
}

macro_rules! encoder_attachment {
    ($name:ident, $descriptor:ty) => {
        pub(super) fn $name(
            descriptor: &$descriptor,
            buffer: &ProtocolObject<dyn MTLCounterSampleBuffer>,
            first: usize,
        ) {
            // SAFETY: Metal provides attachment 0 and this pass reserves the two indices.
            unsafe {
                let attachment = descriptor
                    .sampleBufferAttachments()
                    .objectAtIndexedSubscript(0);
                attachment.setSampleBuffer(Some(buffer));
                attachment.setStartOfEncoderSampleIndex(first);
                attachment.setEndOfEncoderSampleIndex(first + 1);
            }
        }
    };
}

encoder_attachment!(attach_compute, MTLComputePassDescriptor);
encoder_attachment!(attach_blit, MTLBlitPassDescriptor);
encoder_attachment!(attach_acceleration, MTLAccelerationStructurePassDescriptor);
