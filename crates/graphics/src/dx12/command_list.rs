//! Recording a `PlannedSubmit` into an `ID3D12GraphicsCommandList`.
//!
//! Swift sent a flat `GRHI_DX12Command` stream (`kind`, `resource`, `slot`, `a`…)
//! to its C++ bridge, which decoded it into command-list calls. The Rust port
//! encodes the recorded commands directly, which removes the opcode stream and the
//! second decode while keeping the same command set.
#![allow(non_snake_case)]
#[cfg(target_os = "windows")]
use crate::rhi::{
    ColorLoadAction, ComputeCommand, CopyCommand, DepthLoadAction, PlannedCommand, RenderCommand,
    rhi_byte_range, rhi_count, rhi_require, rhi_texture_subresource_extent,
    rhi_texture_upload_bytes,
};
use crate::rhi::{PlannedSubmit, RhiError, RhiResult};

use super::device::Dx12DeviceHandle;
#[cfg(any(target_os = "windows", test))]
use super::formats;
use super::resources::Dx12Registries;
#[cfg(target_os = "windows")]
use super::resources::{
    CommandListVtbl, D3D12_INDEX_BUFFER_VIEW, D3D12_VERTEX_BUFFER_VIEW, transition,
};
use super::sys;

/// Records `submit` into a freshly opened command list and closes it.
///
/// The returned pointers are upload-heap staging buffers whose contents the
/// recorded copies still reference; the caller releases them once the GPU has
/// retired this submission.
pub fn record(
    device: &mut Dx12DeviceHandle,
    registries: &mut Dx12Registries,
    queue_kind: u32,
    submit: &PlannedSubmit,
) -> RhiResult<(sys::ComPtr, Vec<sys::ComPtr>)> {
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
        let mut staged: Vec<sys::ComPtr> = Vec::new();
        for command in &submit.commands {
            if let Err(error) = encode(device, registries, list, command, &mut staged) {
                // SAFETY: recording failed before the list was submitted, so the
                // staging buffers were never read by the GPU.
                unsafe {
                    sys::IUnknownVtbl::release(list);
                    for resource in staged {
                        sys::IUnknownVtbl::release(resource);
                    }
                }
                return Err(error);
            }
        }
        // SAFETY: the list is open and holds the recorded commands.
        let status = unsafe {
            let vtbl = *(list as *mut *const CommandListVtbl);
            ((*vtbl).Close)(list)
        };
        if status != sys::S_OK {
            // SAFETY: a failed close leaves the list unreferenced.
            unsafe {
                sys::IUnknownVtbl::release(list);
                for resource in staged {
                    sys::IUnknownVtbl::release(resource);
                }
            }
            return Err(RhiError::submit_failed(sys::hresult_message(status)));
        }
        Ok((list, staged))
    }
}

/// Opens a command list on the allocator for this queue class.
#[cfg(target_os = "windows")]
fn open_list(device: &mut Dx12DeviceHandle, queue_kind: u32) -> RhiResult<sys::ComPtr> {
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
    staged: &mut Vec<sys::ComPtr>,
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
        PlannedCommand::RenderPass(record) => encode_render_pass(device, registries, list, record),
        PlannedCommand::ComputePass(record) => {
            encode_compute_pass(device, registries, list, record)
        }
        PlannedCommand::CopyPass(record) => {
            for command in &record.body {
                encode_copy(device, registries, list, command, staged)?;
            }
            Ok(())
        }
        PlannedCommand::AccelerationStructureBuild(_) => Err(RhiError::unsupported_feature(
            "DX12 acceleration-structure builds need the DXR dispatch tables, which this build does not link",
        )),
    }
}

/// State that must persist between commands of one pass.
#[cfg(target_os = "windows")]
struct BindState {
    /// Layout of the pipeline currently bound; binding sets and push constants
    /// resolve their root-parameter indices through it.
    layout_id: Option<u32>,
    /// Graphics pipeline whose vertex-input strides vertex-buffer views need.
    graphics_pipeline: Option<u32>,
    /// `SetDescriptorHeaps` may only be called once per command list.
    heaps_set: bool,
    compute: bool,
}

#[cfg(target_os = "windows")]
impl BindState {
    fn new(compute: bool) -> Self {
        Self {
            layout_id: None,
            graphics_pipeline: None,
            heaps_set: false,
            compute,
        }
    }
}

/// Transitions every attachment, sets the render targets, applies clears and
/// encodes the body.
#[cfg(target_os = "windows")]
fn encode_render_pass(
    device: &Dx12DeviceHandle,
    registries: &Dx12Registries,
    list: sys::ComPtr,
    record: &crate::rhi::RenderPassRecord,
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
        let vtbl = *(list as *mut *const CommandListVtbl);
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
        for (target, view) in record.descriptor.color_targets.iter().zip(&color_views) {
            if let ColorLoadAction::Clear(color) = target.load_action {
                ((*vtbl).ClearRenderTargetView)(list, *view, color.as_ptr(), 0, core::ptr::null());
            }
        }
        if let Some((DepthLoadAction::Clear(depth), view)) = record
            .descriptor
            .depth_target
            .map(|target| target.load_action)
            .zip(depth_view)
        {
            ((*vtbl).ClearDepthStencilView)(
                list,
                view,
                sys::D3D12_CLEAR_FLAG_DEPTH,
                depth as f32,
                0,
                0,
                core::ptr::null(),
            );
        }
    }
    let mut state = BindState::new(false);
    for command in &record.body {
        encode_render(device, registries, list, &mut state, command)?;
    }
    Ok(())
}

/// Encodes one render-list command.
#[cfg(target_os = "windows")]
fn encode_render(
    device: &Dx12DeviceHandle,
    registries: &Dx12Registries,
    list: sys::ComPtr,
    state: &mut BindState,
    command: &RenderCommand,
) -> RhiResult<()> {
    match command {
        RenderCommand::SetPipeline(handle) => {
            bind_graphics_pipeline(registries, list, state, handle.id())
        }
        RenderCommand::SetMeshPipeline(handle) => {
            bind_mesh_pipeline(registries, list, state, handle.id())
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
            // SAFETY: the list is open and the viewport outlives the call.
            unsafe {
                let vtbl = *(list as *mut *const CommandListVtbl);
                ((*vtbl).RSSetViewports)(list, 1, &native);
            }
            Ok(())
        }
        RenderCommand::SetScissor(scissor) => {
            let native = sys::D3D12_RECT {
                left: scissor.x as i32,
                top: scissor.y as i32,
                right: (scissor.x + scissor.width) as i32,
                bottom: (scissor.y + scissor.height) as i32,
            };
            // SAFETY: the list is open and the rectangle outlives the call.
            unsafe {
                let vtbl = *(list as *mut *const CommandListVtbl);
                ((*vtbl).RSSetScissorRects)(list, 1, &native);
            }
            Ok(())
        }
        RenderCommand::Draw {
            vertex_count,
            instance_count,
            first_vertex,
            first_instance,
        } => {
            // SAFETY: the list is open for the whole call.
            unsafe {
                let vtbl = *(list as *mut *const CommandListVtbl);
                ((*vtbl).DrawInstanced)(
                    list,
                    rhi_count(*vertex_count)?,
                    rhi_count(*instance_count)?,
                    rhi_count(*first_vertex)?,
                    rhi_count(*first_instance)?,
                );
            }
            Ok(())
        }
        RenderCommand::DrawIndexed(arguments) => {
            // SAFETY: the list is open for the whole call.
            unsafe {
                let vtbl = *(list as *mut *const CommandListVtbl);
                ((*vtbl).DrawIndexedInstanced)(
                    list,
                    rhi_count(arguments.index_count)?,
                    rhi_count(arguments.instance_count)?,
                    rhi_count(arguments.first_index)?,
                    arguments.vertex_offset as i32,
                    rhi_count(arguments.first_instance)?,
                );
            }
            Ok(())
        }
        RenderCommand::SetBindingSet {
            slot,
            set,
            dynamic_offsets,
        } => bind_set(
            device,
            registries,
            list,
            state,
            *slot,
            set.id(),
            dynamic_offsets,
        ),
        RenderCommand::PushConstant { slot, data, .. } => {
            push_constants(registries, list, state, *slot, data)
        }
        RenderCommand::SetVertexBuffer {
            slot,
            buffer,
            offset,
        } => bind_vertex_buffer(registries, list, state, *slot, buffer.id(), *offset),
        RenderCommand::SetIndexBuffer {
            buffer,
            offset,
            index_type,
        } => bind_index_buffer(registries, list, buffer.id(), *offset, *index_type),
        RenderCommand::DrawIndirect { .. } | RenderCommand::DrawMeshTasks(_) => {
            Err(RhiError::unsupported_feature(
                "DX12 indirect draws and mesh dispatches need command signatures, which this build does not create",
            ))
        }
    }
}

/// Encodes one compute-list command.
#[cfg(target_os = "windows")]
fn encode_compute_pass(
    device: &Dx12DeviceHandle,
    registries: &Dx12Registries,
    list: sys::ComPtr,
    record: &crate::rhi::ComputePassRecord,
) -> RhiResult<()> {
    let mut state = BindState::new(true);
    for command in &record.body {
        encode_compute(device, registries, list, &mut state, command)?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn encode_compute(
    device: &Dx12DeviceHandle,
    registries: &Dx12Registries,
    list: sys::ComPtr,
    state: &mut BindState,
    command: &ComputeCommand,
) -> RhiResult<()> {
    match command {
        ComputeCommand::SetPipeline(handle) => {
            bind_compute_pipeline(registries, list, state, handle.id())
        }
        ComputeCommand::SetBindingSet {
            slot,
            set,
            dynamic_offsets,
        } => bind_set(
            device,
            registries,
            list,
            state,
            *slot,
            set.id(),
            dynamic_offsets,
        ),
        ComputeCommand::PushConstant { slot, data, .. } => {
            push_constants(registries, list, state, *slot, data)
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
            // SAFETY: the list is open for the whole call.
            unsafe {
                let vtbl = *(list as *mut *const CommandListVtbl);
                ((*vtbl).Dispatch)(
                    list,
                    rhi_count(*groups_x)?,
                    rhi_count(*groups_y)?,
                    rhi_count(*groups_z)?,
                );
            }
            Ok(())
        }
        ComputeCommand::DispatchIndirect { .. } => Err(RhiError::unsupported_feature(
            "DX12 indirect dispatch needs a command signature, which this build does not create",
        )),
    }
}

/// Encodes one copy-list command.
#[cfg(target_os = "windows")]
fn encode_copy(
    device: &Dx12DeviceHandle,
    registries: &Dx12Registries,
    list: sys::ComPtr,
    command: &CopyCommand,
    staged: &mut Vec<sys::ComPtr>,
) -> RhiResult<()> {
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
            // SAFETY: both buffers and the list are live.
            unsafe {
                let vtbl = *(list as *mut *const CommandListVtbl);
                ((*vtbl).CopyBufferRegion)(
                    list,
                    destination.resource,
                    *dst_offset as u64,
                    source.resource,
                    *src_offset as u64,
                    *size as u64,
                );
            }
            Ok(())
        }
        CopyCommand::CopyBufferToTexture(upload) => {
            encode_buffer_to_texture(device, registries, list, upload, staged)
        }
        CopyCommand::CopyTexture { .. } | CopyCommand::CopyTextureToBuffer { .. } => {
            // Texture-to-texture and readback copies still go through the
            // immediate-transfer path; planar copies are not recorded here.
            Ok(())
        }
    }
}

/// Validates and records one buffer-to-texture upload.
///
/// D3D12 requires the buffer side of a placed footprint to use a row pitch that
/// is a multiple of 256 bytes, while the RHI accepts tightly packed rows. Rows
/// are re-laid out into a short-lived upload-heap buffer; the caller releases it
/// once the submission's fence has retired.
#[cfg(target_os = "windows")]
fn encode_buffer_to_texture(
    device: &Dx12DeviceHandle,
    registries: &Dx12Registries,
    list: sys::ComPtr,
    upload: &crate::rhi::TextureBufferUpload,
    staged: &mut Vec<sys::ComPtr>,
) -> RhiResult<()> {
    let texture = registries
        .textures
        .get(&upload.texture.id())
        .ok_or_else(|| RhiError::invalid_argument("unknown upload texture"))?;
    let buffer = registries
        .buffers
        .get(&upload.buffer.id())
        .ok_or_else(|| RhiError::invalid_argument("unknown upload buffer"))?;
    let (extent_width, extent_height) = rhi_texture_subresource_extent(
        upload.subresource,
        texture.width,
        texture.height,
        texture.mip_levels,
        texture.layers,
    )?;
    let capacity = buffer.size as usize;
    let available = capacity.saturating_sub(upload.offset.min(capacity));
    let byte_count = rhi_texture_upload_bytes(
        upload.region,
        upload.bytes_per_row,
        texture.format,
        extent_width,
        extent_height,
        available,
    )?;
    rhi_byte_range(upload.offset, byte_count, capacity)?;
    rhi_require(
        !buffer.mapped.is_null(),
        "recorded upload needs a CPU-visible source buffer",
    )?;

    let width = upload.region.width as u32;
    let height = upload.region.height as u32;
    let pitch = upload
        .bytes_per_row
        .next_multiple_of(sys::D3D12_TEXTURE_DATA_PITCH_ALIGNMENT);
    let staging_size = pitch
        .checked_mul(height as usize)
        .ok_or_else(|| RhiError::invalid_argument("texture upload is too large"))?;
    let staging = super::buffer::create_in_heap_for_transfer(
        device.device,
        staging_size,
        sys::D3D12_HEAP_TYPE_UPLOAD,
    )?;
    // Register the resource before anything else can fail: `record` releases
    // everything in `staged` if encoding aborts.
    staged.push(staging.resource);
    rhi_require(
        !staging.mapped.is_null(),
        "upload staging heap is not mapped",
    )?;

    for row in 0..height as usize {
        let start = upload.offset + row * upload.bytes_per_row;
        let end = (start + upload.bytes_per_row).min(upload.offset + byte_count);
        if end <= start {
            break;
        }
        // SAFETY: `start..end` lies inside the validated source buffer range and
        // `row * pitch + (end - start)` lies inside the staging allocation.
        unsafe {
            core::ptr::copy_nonoverlapping(
                buffer.mapped.add(start),
                staging.mapped.add(row * pitch),
                end - start,
            );
        }
    }

    let destination = sys::D3D12_TEXTURE_COPY_LOCATION::subresource(
        texture.resource,
        (upload.subresource.mip_level + upload.subresource.layer * texture.mip_levels) as u32,
    );
    let source = sys::D3D12_TEXTURE_COPY_LOCATION::placed(
        staging.resource,
        sys::D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
            Offset: 0,
            Footprint: sys::D3D12_SUBRESOURCE_FOOTPRINT {
                Format: formats::dxgi_format(texture.format),
                Width: width,
                Height: height,
                Depth: 1,
                RowPitch: pitch as u32,
            },
        },
    );
    let source_box = sys::D3D12_BOX {
        left: 0,
        top: 0,
        front: 0,
        right: width,
        bottom: height,
        back: 1,
    };
    let x = upload.region.origin[0].max(0) as u32;
    let y = upload.region.origin[1].max(0) as u32;
    // SAFETY: both resources are live; the frontend's barrier pass put the
    // texture into COPY_DEST, and the footprint describes the staging buffer.
    unsafe {
        let vtbl = *(list as *mut *const CommandListVtbl);
        ((*vtbl).CopyTextureRegion)(list, &destination, x, y, 0, &source, &source_box);
    }
    Ok(())
}

/// The root signature a pipeline was compiled against.
#[cfg(target_os = "windows")]
fn root_signature(registries: &Dx12Registries, layout_id: u32) -> RhiResult<sys::ComPtr> {
    registries
        .root_signatures
        .get(&layout_id)
        .copied()
        .ok_or_else(|| RhiError::invalid_argument("pipeline references an unknown layout"))
}

#[cfg(target_os = "windows")]
fn bind_graphics_pipeline(
    registries: &Dx12Registries,
    list: sys::ComPtr,
    state: &mut BindState,
    pipeline_id: u32,
) -> RhiResult<()> {
    let record = registries
        .graphics_pipelines
        .get(&pipeline_id)
        .ok_or_else(|| RhiError::invalid_argument("unknown graphics pipeline"))?;
    let signature = root_signature(registries, record.layout_id)?;
    // SAFETY: the pipeline state and root signature are live COM objects and the
    // list is open on a direct queue, which supports the graphics root signature.
    unsafe {
        let vtbl = *(list as *mut *const CommandListVtbl);
        ((*vtbl).SetPipelineState)(list, record.pipeline);
        ((*vtbl).SetGraphicsRootSignature)(list, signature);
        ((*vtbl).IASetPrimitiveTopology)(list, formats::primitive_topology(record.primitive));
    }
    state.layout_id = Some(record.layout_id);
    state.graphics_pipeline = Some(pipeline_id);
    Ok(())
}

#[cfg(target_os = "windows")]
fn bind_mesh_pipeline(
    registries: &Dx12Registries,
    list: sys::ComPtr,
    state: &mut BindState,
    pipeline_id: u32,
) -> RhiResult<()> {
    let record = registries
        .mesh_pipelines
        .get(&pipeline_id)
        .ok_or_else(|| RhiError::invalid_argument("unknown mesh pipeline"))?;
    let signature = root_signature(registries, record.layout_id)?;
    // SAFETY: as above; mesh pipelines have no input assembler state to set.
    unsafe {
        let vtbl = *(list as *mut *const CommandListVtbl);
        ((*vtbl).SetPipelineState)(list, record.pipeline);
        ((*vtbl).SetGraphicsRootSignature)(list, signature);
    }
    state.layout_id = Some(record.layout_id);
    state.graphics_pipeline = None;
    Ok(())
}

#[cfg(target_os = "windows")]
fn bind_compute_pipeline(
    registries: &Dx12Registries,
    list: sys::ComPtr,
    state: &mut BindState,
    pipeline_id: u32,
) -> RhiResult<()> {
    let record = registries
        .compute_pipelines
        .get(&pipeline_id)
        .ok_or_else(|| RhiError::invalid_argument("unknown compute pipeline"))?;
    let signature = root_signature(registries, record.layout_id)?;
    // SAFETY: the pipeline state and root signature are live COM objects and the
    // list is open on a compute queue.
    unsafe {
        let vtbl = *(list as *mut *const CommandListVtbl);
        ((*vtbl).SetPipelineState)(list, record.pipeline);
        ((*vtbl).SetComputeRootSignature)(list, signature);
    }
    state.layout_id = Some(record.layout_id);
    state.graphics_pipeline = None;
    Ok(())
}

/// Sets the two shader-visible descriptor heaps once. D3D12 invalidates
/// descriptor-table bindings whenever `SetDescriptorHeaps` is called again, so
/// this happens before the first table is bound and never per set.
#[cfg(target_os = "windows")]
fn set_descriptor_heaps(device: &Dx12DeviceHandle, list: sys::ComPtr, state: &mut BindState) {
    if state.heaps_set {
        return;
    }
    let heaps = [device.resource_heap, device.sampler_heap];
    // SAFETY: both heaps were created shader-visible by the device and the list
    // is open; at most one heap of each type is supplied.
    unsafe {
        let vtbl = *(list as *mut *const CommandListVtbl);
        ((*vtbl).SetDescriptorHeaps)(list, heaps.len() as u32, heaps.as_ptr());
    }
    state.heaps_set = true;
}

/// Binds one set: its two descriptor tables and any buffer root descriptors.
#[cfg(target_os = "windows")]
#[allow(clippy::too_many_arguments)]
fn bind_set(
    device: &Dx12DeviceHandle,
    registries: &Dx12Registries,
    list: sys::ComPtr,
    state: &mut BindState,
    slot: u32,
    set_id: u32,
    dynamic_offsets: &[u32],
) -> RhiResult<()> {
    let layout_id = state.layout_id.ok_or_else(|| {
        RhiError::layout_mismatch("binding set does not match the active pipeline layout")
    })?;
    let layout = registries.pipeline_layouts.get(&layout_id).ok_or_else(|| {
        RhiError::layout_mismatch("binding set does not match the active pipeline layout")
    })?;
    let set = registries.binding_sets.get(&set_id).ok_or_else(|| {
        RhiError::layout_mismatch("binding set does not match the active pipeline layout")
    })?;
    rhi_require(
        (slot as usize) < layout.set_layouts.len()
            && layout.set_layouts[slot as usize].id() == set.layout_id,
        "binding set does not match the active pipeline layout",
    )?;
    let declarations = registries
        .binding_layouts
        .get(&set.layout_id)
        .ok_or_else(|| RhiError::invalid_argument("unknown binding layout"))?
        .entries
        .clone();
    let dynamic_count = declarations
        .iter()
        .filter(|declaration| {
            declaration.buffer.has_dynamic_offset
                && matches!(
                    declaration.binding_type,
                    crate::rhi::BindingType::UniformBuffer | crate::rhi::BindingType::StorageBuffer
                )
        })
        .count();
    rhi_require(
        dynamic_offsets.len() == dynamic_count,
        "dynamic offset count does not match the binding layout",
    )?;
    set_descriptor_heaps(device, list, state);

    let resource_root = layout
        .resource_table_root_index
        .get(slot as usize)
        .copied()
        .flatten();
    let sampler_root = layout
        .sampler_table_root_index
        .get(slot as usize)
        .copied()
        .flatten();
    let resource_handle = match resource_root {
        Some(_) if set.resource_heap_count > 0 => Some(super::texture::resource_heap_gpu(
            device,
            set.resource_heap_base,
        )?),
        _ => None,
    };
    let sampler_handle = match sampler_root {
        Some(_) if set.sampler_heap_count > 0 => Some(super::texture::sampler_heap_gpu(
            device,
            set.sampler_heap_base,
        )?),
        _ => None,
    };
    // SAFETY: the list is open, the heaps are bound, and every handle comes from
    // a range reserved for this set at registration time.
    unsafe {
        let vtbl = *(list as *mut *const CommandListVtbl);
        if let (Some(root), Some(handle)) = (resource_root, resource_handle) {
            if state.compute {
                ((*vtbl).SetComputeRootDescriptorTable)(list, root, handle);
            } else {
                ((*vtbl).SetGraphicsRootDescriptorTable)(list, root, handle);
            }
        }
        if let (Some(root), Some(handle)) = (sampler_root, sampler_handle) {
            if state.compute {
                ((*vtbl).SetComputeRootDescriptorTable)(list, root, handle);
            } else {
                ((*vtbl).SetGraphicsRootDescriptorTable)(list, root, handle);
            }
        }
    }

    // Buffers are bound as root descriptors (GPU virtual addresses), one per
    // declared slot. Dynamic offsets are consumed in declaration order, the same
    // order the frontend supplies them in.
    let mut next_dynamic = 0usize;
    for declaration in &declarations {
        if !matches!(
            declaration.binding_type,
            crate::rhi::BindingType::UniformBuffer | crate::rhi::BindingType::StorageBuffer
        ) {
            continue;
        }
        let entry = set
            .entries
            .iter()
            .find(|candidate| candidate.slot == declaration.slot)
            .ok_or_else(|| RhiError::layout_mismatch("binding has no declaration"))?;
        let (buffer, offset) = match entry.resource {
            crate::rhi::BindingResource::UniformBuffer { buffer, offset, .. }
            | crate::rhi::BindingResource::StorageBuffer { buffer, offset } => (buffer, offset),
            _ => return Err(RhiError::layout_mismatch("buffer slot has no buffer")),
        };
        let record = registries
            .buffers
            .get(&buffer.id())
            .ok_or_else(|| RhiError::invalid_argument("binding references an unknown buffer"))?;
        let extra = if declaration.buffer.has_dynamic_offset {
            let value = dynamic_offsets[next_dynamic] as u64;
            next_dynamic += 1;
            value
        } else {
            0
        };
        let address = record
            .gpu_address
            .saturating_add(offset as u64)
            .saturating_add(extra);
        let root = layout
            .buffer_root_index
            .get(&(slot, declaration.slot))
            .copied()
            .ok_or_else(|| RhiError::layout_mismatch("buffer binding has no root parameter"))?;
        // SAFETY: the address belongs to a live buffer and the root parameter was
        // created for this exact (set, slot) pair.
        unsafe {
            let vtbl = *(list as *mut *const CommandListVtbl);
            match (
                state.compute,
                declaration.binding_type,
                declaration.buffer.read_only,
            ) {
                (true, crate::rhi::BindingType::UniformBuffer, _) => {
                    ((*vtbl).SetComputeRootConstantBufferView)(list, root, address)
                }
                (false, crate::rhi::BindingType::UniformBuffer, _) => {
                    ((*vtbl).SetGraphicsRootConstantBufferView)(list, root, address)
                }
                (true, _, true) => ((*vtbl).SetComputeRootShaderResourceView)(list, root, address),
                (false, _, true) => {
                    ((*vtbl).SetGraphicsRootShaderResourceView)(list, root, address)
                }
                (true, _, false) => {
                    ((*vtbl).SetComputeRootUnorderedAccessView)(list, root, address)
                }
                (false, _, false) => {
                    ((*vtbl).SetGraphicsRootUnorderedAccessView)(list, root, address)
                }
            }
        }
    }
    Ok(())
}

/// Copies a push-constant range into its root-constants parameter.
#[cfg(target_os = "windows")]
fn push_constants(
    registries: &Dx12Registries,
    list: sys::ComPtr,
    state: &mut BindState,
    slot: u32,
    data: &[u8],
) -> RhiResult<()> {
    let layout_id = state.layout_id.ok_or_else(|| {
        RhiError::layout_mismatch("push constant does not match the active pipeline layout")
    })?;
    let layout = registries.pipeline_layouts.get(&layout_id).ok_or_else(|| {
        RhiError::layout_mismatch("push constant does not match the active pipeline layout")
    })?;
    let root = layout
        .push_constant_root_index
        .get(&slot)
        .copied()
        .ok_or_else(|| RhiError::layout_mismatch("push constant has no root parameter"))?;
    let range = layout
        .push_constants
        .iter()
        .find(|range| range.slot == slot)
        .ok_or_else(|| RhiError::layout_mismatch("push constant has no declared range"))?;
    rhi_require(
        range.byte_count > 0 && range.byte_count % 4 == 0,
        "push-constant byte count must be a positive multiple of four",
    )?;
    rhi_require(
        data.len() == range.byte_count as usize,
        "push-constant data does not match the declared range",
    )?;
    let values = (range.byte_count / 4) as u32;
    // SAFETY: the list is open, `data` outlives the call and its length equals
    // the number of 32-bit values the root parameter declares.
    unsafe {
        let vtbl = *(list as *mut *const CommandListVtbl);
        if state.compute {
            ((*vtbl).SetComputeRoot32BitConstants)(list, root, values, data.as_ptr(), 0);
        } else {
            ((*vtbl).SetGraphicsRoot32BitConstants)(list, root, values, data.as_ptr(), 0);
        }
    }
    Ok(())
}

/// Binds one vertex buffer view at its declared slot.
#[cfg(target_os = "windows")]
fn bind_vertex_buffer(
    registries: &Dx12Registries,
    list: sys::ComPtr,
    state: &BindState,
    slot: u32,
    buffer_id: u32,
    offset: usize,
) -> RhiResult<()> {
    let record = registries
        .buffers
        .get(&buffer_id)
        .ok_or_else(|| RhiError::invalid_argument("unknown vertex buffer"))?;
    rhi_require(
        offset < record.size as usize,
        "vertex-buffer offset is outside the buffer",
    )?;
    let stride = state
        .graphics_pipeline
        .and_then(|id| registries.graphics_pipelines.get(&id))
        .and_then(|pipeline| pipeline.vertex_layout.as_ref())
        .and_then(|layout| layout.buffer_layouts.get(slot as usize))
        .map_or(0, |layout| layout.stride as u32);
    let view = D3D12_VERTEX_BUFFER_VIEW {
        BufferLocation: record.gpu_address.saturating_add(offset as u64),
        SizeInBytes: (record.size as usize - offset).min(u32::MAX as usize) as u32,
        StrideInBytes: stride,
    };
    // SAFETY: the list is open and the view points into a live buffer.
    unsafe {
        let vtbl = *(list as *mut *const CommandListVtbl);
        ((*vtbl).IASetVertexBuffers)(list, slot, 1, &view);
    }
    Ok(())
}

/// Binds one index buffer view.
#[cfg(target_os = "windows")]
fn bind_index_buffer(
    registries: &Dx12Registries,
    list: sys::ComPtr,
    buffer_id: u32,
    offset: usize,
    index_type: crate::rhi::IndexType,
) -> RhiResult<()> {
    let record = registries
        .buffers
        .get(&buffer_id)
        .ok_or_else(|| RhiError::invalid_argument("unknown index buffer"))?;
    rhi_require(
        offset < record.size as usize,
        "index-buffer offset is outside the buffer",
    )?;
    let view = D3D12_INDEX_BUFFER_VIEW {
        BufferLocation: record.gpu_address.saturating_add(offset as u64),
        SizeInBytes: (record.size as usize - offset).min(u32::MAX as usize) as u32,
        Format: match index_type {
            crate::rhi::IndexType::Uint16 => sys::DXGI_FORMAT_R16_UINT,
            crate::rhi::IndexType::Uint32 => sys::DXGI_FORMAT_R32_UINT,
        },
    };
    // SAFETY: the list is open and the view points into a live buffer.
    unsafe {
        let vtbl = *(list as *mut *const CommandListVtbl);
        ((*vtbl).IASetIndexBuffer)(list, &view);
    }
    Ok(())
}

/// Opens a command list on the copy queue, for one immediate transfer.
#[cfg(target_os = "windows")]
pub fn open_transfer_list(device: &mut Dx12DeviceHandle) -> RhiResult<sys::ComPtr> {
    open_list(device, sys::D3D12_COMMAND_LIST_TYPE_COPY)
}

/// Closes `list`, submits it on the copy queue and waits for the GPU.
#[cfg(target_os = "windows")]
pub fn close_and_submit(device: &mut Dx12DeviceHandle, list: sys::ComPtr) -> RhiResult<()> {
    // SAFETY: the list is open and holds the recorded command.
    let status = unsafe {
        let vtbl = *(list as *mut *const CommandListVtbl);
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
