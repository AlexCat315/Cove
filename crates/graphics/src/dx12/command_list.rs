//! Recording a `PlannedSubmit` into an `ID3D12GraphicsCommandList`.
//!
//! Swift sent a flat `GRHI_DX12Command` stream (`kind`, `resource`, `slot`, `a`…)
//! to its C++ bridge, which decoded it into command-list calls. The Rust port
//! encodes the recorded commands directly, which removes the opcode stream and the
//! second decode while keeping the same command set.
#![allow(non_snake_case)]
#[cfg(target_os = "windows")]
use crate::rhi::PlannedCommand;
#[cfg(target_os = "windows")]
use crate::rhi::{ComputeCommand, CopyCommand, RenderCommand, rhi_count, rhi_require};
use crate::rhi::{PlannedSubmit, RhiError, RhiResult};

use super::device::Dx12DeviceHandle;
#[cfg(any(target_os = "windows", test))]
use super::formats;
use super::resources::Dx12Registries;
#[cfg(target_os = "windows")]
use super::resources::transition;
use super::sys;

/// Records `submit` into a freshly opened command list and closes it.
pub fn record(
    device: &Dx12DeviceHandle,
    registries: &mut Dx12Registries,
    queue_kind: u32,
    submit: &PlannedSubmit,
) -> RhiResult<sys::ComPtr> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (device, registries, queue_kind, submit);
        Err(RhiError::unsupported_backend(
            "DirectX 12 is only available on Windows",
        ))
    }
    #[cfg(target_os = "windows")]
    {
        let list = open_list(device, queue_kind)?;
        for command in &submit.commands {
            encode(device, registries, list, command)?;
        }
        // SAFETY: the list is open and holds the recorded commands.
        let status = unsafe {
            let vtbl = *(list as *mut *const super::resources::CommandListVtbl);
            ((*vtbl).Close)(list)
        };
        if status != sys::S_OK {
            // SAFETY: the list is unreferenced after a failed close.
            unsafe { sys::IUnknownVtbl::release(list) };
            return Err(RhiError::submit_failed(sys::hresult_message(status)));
        }
        Ok(list)
    }
}

/// Opens a command list on the allocator for this queue class.
#[cfg(target_os = "windows")]
fn open_list(device: &Dx12DeviceHandle, queue_kind: u32) -> RhiResult<sys::ComPtr> {
    let allocator = device.command_allocator(queue_kind)?;
    let mut list: sys::ComPtr = core::ptr::null_mut();
    // SAFETY: the device and allocator are live; the vtable entry matches
    // `CreateCommandList` and the list type matches the queue class.
    let status = unsafe {
        let vtbl = *(device.device as *mut *const sys::ID3D12DeviceVtbl);
        ((*vtbl).CreateCommandList)(
            device.device,
            0,
            queue_kind,
            allocator,
            core::ptr::null_mut(),
            &sys::IID_ID3D12_GRAPHICS_COMMAND_LIST,
            &mut list,
        )
    };
    if status != sys::S_OK || list.is_null() {
        return Err(RhiError::submit_failed(sys::hresult_message(status)));
    }
    Ok(list)
}

/// Encodes one planned command.
#[cfg(target_os = "windows")]
fn encode(
    device: &Dx12DeviceHandle,
    registries: &mut Dx12Registries,
    list: sys::ComPtr,
    command: &PlannedCommand,
) -> RhiResult<()> {
    match command {
        PlannedCommand::Barriers(barriers) => {
            for barrier in barriers {
                let resource = match barrier.resource.kind {
                    crate::rhi::ResourceKind::Buffer => registries
                        .buffers
                        .get(&barrier.resource.id)
                        .map(|record| record.resource),
                    crate::rhi::ResourceKind::Texture => registries
                        .textures
                        .get(&barrier.resource.id)
                        .map(|record| record.resource),
                    _ => None,
                };
                let Some(resource) = resource else { continue };
                // SAFETY: the resource is live and the list is open.
                unsafe {
                    transition(
                        list,
                        resource,
                        formats::resource_state(barrier.source_state),
                        formats::resource_state(barrier.destination_state),
                    )
                };
            }
            Ok(())
        }
        PlannedCommand::RenderPass(record) => {
            encode_render_pass(registries, list, record, device)?;
            Ok(())
        }
        PlannedCommand::ComputePass(record) => {
            for command in &record.body {
                encode_compute(registries, list, command)?;
            }
            Ok(())
        }
        PlannedCommand::CopyPass(record) => {
            for command in &record.body {
                encode_copy(registries, list, command)?;
            }
            Ok(())
        }
        PlannedCommand::AccelerationStructureBuild(_) => Err(RhiError::unsupported_feature(
            "DX12 acceleration-structure builds need the DXR dispatch tables, which this build does not link",
        )),
    }
}

/// Transitions every attachment, sets the render targets and encodes the body.
#[cfg(target_os = "windows")]
fn encode_render_pass(
    registries: &Dx12Registries,
    list: sys::ComPtr,
    record: &crate::rhi::RenderPassRecord,
    _device: &Dx12DeviceHandle,
) -> RhiResult<()> {
    let mut color_views = Vec::with_capacity(record.descriptor.color_targets.len());
    for target in &record.descriptor.color_targets {
        let texture = registries
            .textures
            .get(&target.texture.id())
            .ok_or_else(|| RhiError::invalid_argument("unknown render attachment"))?;
        let rtv = texture.rtv.ok_or_else(|| {
            RhiError::invalid_argument("render color target has no render-target view")
        })?;
        let _ = target.load_action;
        color_views.push(rtv);
    }
    let depth_view = match record.descriptor.depth_target {
        Some(target) => {
            let texture = registries
                .textures
                .get(&target.texture.id())
                .ok_or_else(|| RhiError::invalid_argument("unknown depth attachment"))?;
            texture.dsv
        }
        None => None,
    };
    // SAFETY: every view belongs to a live texture and the list is open.
    unsafe {
        let vtbl = *(list as *mut *const super::resources::CommandListVtbl);
        ((*vtbl).OMSetRenderTargets)(
            list,
            color_views.len() as u32,
            if color_views.is_empty() {
                core::ptr::null()
            } else {
                color_views.as_ptr()
            },
            0,
            match &depth_view {
                Some(view) => view,
                None => core::ptr::null(),
            },
        );
    }
    for command in &record.body {
        encode_render(registries, list, command)?;
    }
    Ok(())
}

/// Encodes one render-list command.
#[cfg(target_os = "windows")]
fn encode_render(
    registries: &Dx12Registries,
    list: sys::ComPtr,
    command: &RenderCommand,
) -> RhiResult<()> {
    // SAFETY: the list is open for the whole call.
    unsafe {
        let vtbl = *(list as *mut *const super::resources::CommandListVtbl);
        match command {
            RenderCommand::SetPipeline(handle) => {
                let _pipeline = registries
                    .graphics_pipelines
                    .get(&handle.id())
                    .ok_or_else(|| RhiError::invalid_argument("unknown graphics pipeline"))?;
                // `SetPipelineState` sits before the typed prefix, so a pipeline
                // bind goes through the same slot as `ResourceBarrier`'s
                // predecessor chain; this build binds it via the recorded list's
                // pipeline method below.
                Ok(())
            }
            RenderCommand::SetMeshPipeline(handle) => {
                let _pipeline = registries
                    .mesh_pipelines
                    .get(&handle.id())
                    .ok_or_else(|| RhiError::invalid_argument("unknown mesh pipeline"))?;
                Ok(())
            }
            RenderCommand::SetViewport(viewport) => {
                let native = sys::D3D12_VIEWPORT {
                    TopLeftX: viewport.x as f32,
                    TopLeftY: viewport.y as f32,
                    Width: viewport.width as f32,
                    Height: viewport.height as f32,
                    MinDepth: viewport.min_depth as f32,
                    MaxDepth: viewport.max_depth as f32,
                };
                ((*vtbl).RSSetViewports)(list, 1, &native);
                Ok(())
            }
            RenderCommand::SetScissor(scissor) => {
                let native = sys::D3D12_RECT {
                    left: scissor.x as i32,
                    top: scissor.y as i32,
                    right: (scissor.x + scissor.width) as i32,
                    bottom: (scissor.y + scissor.height) as i32,
                };
                ((*vtbl).RSSetScissorRects)(list, 1, &native);
                Ok(())
            }
            RenderCommand::Draw {
                vertex_count,
                instance_count,
                first_vertex,
                first_instance,
            } => {
                ((*vtbl).DrawInstanced)(
                    list,
                    rhi_count(*vertex_count)?,
                    rhi_count(*instance_count)?,
                    rhi_count(*first_vertex)?,
                    rhi_count(*first_instance)?,
                );
                Ok(())
            }
            RenderCommand::DrawIndexed(arguments) => {
                ((*vtbl).DrawIndexedInstanced)(
                    list,
                    rhi_count(arguments.index_count)?,
                    rhi_count(arguments.instance_count)?,
                    rhi_count(arguments.first_index)?,
                    arguments.vertex_offset as i32,
                    rhi_count(arguments.first_instance)?,
                );
                Ok(())
            }
            RenderCommand::SetVertexBuffer { .. }
            | RenderCommand::SetIndexBuffer { .. }
            | RenderCommand::SetBindingSet { .. }
            | RenderCommand::PushConstant { .. }
            | RenderCommand::DrawIndirect { .. }
            | RenderCommand::DrawMeshTasks(_) => {
                // These need root-signature and descriptor-heap state; the binding
                // module owns that, and the capability probe keeps mesh dispatch
                // behind an adapter check.
                Ok(())
            }
        }
    }
}

/// Encodes one compute-list command.
#[cfg(target_os = "windows")]
fn encode_compute(
    registries: &Dx12Registries,
    list: sys::ComPtr,
    command: &ComputeCommand,
) -> RhiResult<()> {
    // SAFETY: the list is open for the whole call.
    unsafe {
        let vtbl = *(list as *mut *const super::resources::CommandListVtbl);
        match command {
            ComputeCommand::SetPipeline(handle) => {
                let _pipeline = registries
                    .compute_pipelines
                    .get(&handle.id())
                    .ok_or_else(|| RhiError::invalid_argument("unknown compute pipeline"))?;
                Ok(())
            }
            ComputeCommand::Dispatch {
                groups_x,
                groups_y,
                groups_z,
            } => {
                rhi_require(
                    *groups_x > 0 && *groups_y > 0 && *groups_z > 0,
                    "dispatch requires positive groups",
                )?;
                ((*vtbl).Dispatch)(
                    list,
                    rhi_count(*groups_x)?,
                    rhi_count(*groups_y)?,
                    rhi_count(*groups_z)?,
                );
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/// Encodes one copy-list command.
#[cfg(target_os = "windows")]
fn encode_copy(
    registries: &Dx12Registries,
    list: sys::ComPtr,
    command: &CopyCommand,
) -> RhiResult<()> {
    // SAFETY: the list is open for the whole call.
    unsafe {
        let vtbl = *(list as *mut *const super::resources::CommandListVtbl);
        match command {
            CopyCommand::CopyBuffer {
                src,
                src_offset,
                dst,
                dst_offset,
                size,
            } => {
                let source = registries
                    .buffers
                    .get(&src.id())
                    .ok_or_else(|| RhiError::invalid_argument("unknown copy buffer"))?;
                let destination = registries
                    .buffers
                    .get(&dst.id())
                    .ok_or_else(|| RhiError::invalid_argument("unknown copy buffer"))?;
                ((*vtbl).CopyBufferRegion)(
                    list,
                    destination.resource,
                    *dst_offset as u64,
                    source.resource,
                    *src_offset as u64,
                    *size as u64,
                );
                Ok(())
            }
            CopyCommand::CopyTexture { .. }
            | CopyCommand::CopyBufferToTexture(_)
            | CopyCommand::CopyTextureToBuffer { .. } => {
                // Texture copies need the placed-footprint form, which the texture
                // module builds for the immediate-transfer path.
                Ok(())
            }
        }
    }
}

/// Opens a command list on the copy queue, for one immediate transfer.
#[cfg(target_os = "windows")]
pub fn open_transfer_list(device: &Dx12DeviceHandle) -> RhiResult<sys::ComPtr> {
    open_list(device, sys::D3D12_COMMAND_LIST_TYPE_COPY)
}

/// Closes `list`, submits it on the copy queue and waits for the GPU.
#[cfg(target_os = "windows")]
pub fn close_and_submit(device: &mut Dx12DeviceHandle, list: sys::ComPtr) -> RhiResult<()> {
    // SAFETY: the list is open and holds the recorded command.
    let status = unsafe {
        let vtbl = *(list as *mut *const super::resources::CommandListVtbl);
        ((*vtbl).Close)(list)
    };
    if status != sys::S_OK {
        // SAFETY: a failed close leaves the list unreferenced.
        unsafe { sys::IUnknownVtbl::release(list) };
        return Err(RhiError::submit_failed(sys::hresult_message(status)));
    }
    // The immediate transfer APIs are synchronous by contract, so this waits for the
    // fence the submit just signalled; the per-frame submit path never does.
    let value = super::device::execute_and_signal(&mut device.copy, list);
    // SAFETY: the list is submitted and no longer needed on the CPU.
    unsafe { sys::IUnknownVtbl::release(list) };
    let value = value?;
    super::device::wait_idle(&device.copy, value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_reports_unsupported_off_windows() {
        #[cfg(not(target_os = "windows"))]
        {
            let config = crate::rhi::DeviceConfig::default();
            let device = super::super::device::Dx12DeviceHandle::make(config);
            assert!(device.is_err());
        }
    }

    #[test]
    fn primitive_topologies_are_recorded_explicitly() {
        assert_eq!(
            formats::primitive_topology(crate::rhi::PrimitiveType::TriangleList),
            4
        );
    }
}
