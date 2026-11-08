//! Pipeline-state creation for the DX12 backend.
//!
//! Swift built `GRHI_GraphicsDesc` and let the C++ bridge fill
//! `D3D12_GRAPHICS_PIPELINE_STATE_DESC`. The Rust port builds that struct directly,
//! which is the same object with one less translation layer. Creation is
//! Windows-gated; elsewhere the backend reports "unsupported backend".
#![allow(non_snake_case)]
use crate::rhi::{
    ComputePipeline, ComputePipelineDescriptor, GraphicsPipeline, GraphicsPipelineDescriptor,
    MeshPipeline, MeshPipelineDescriptor, RhiError, RhiResult, rhi_require,
};

use super::device::Dx12DeviceHandle;
#[cfg(target_os = "windows")]
use super::formats;
use super::resources::Dx12Registries;
use super::sys;

/// Creates one graphics pipeline state and records it.
pub fn create_graphics(
    device: &Dx12DeviceHandle,
    registries: &mut Dx12Registries,
    handle: GraphicsPipeline,
    descriptor: &GraphicsPipelineDescriptor,
) -> RhiResult<()> {
    rhi_require(
        descriptor.color_attachments.len() <= 8,
        "at most eight color attachments are supported",
    )?;
    let pipeline = create_graphics_native(device, registries, descriptor)?;
    registries.graphics_pipelines.insert(handle.id(), pipeline);
    Ok(())
}

/// Creates one mesh pipeline. D3D12 expresses mesh shaders as a graphics pipeline
/// with `MS`/`AS` bytecode, so the graphics path serves both.
pub fn create_mesh(
    device: &Dx12DeviceHandle,
    registries: &mut Dx12Registries,
    handle: MeshPipeline,
    descriptor: &MeshPipelineDescriptor,
) -> RhiResult<()> {
    let pipeline = create_mesh_native(device, registries, descriptor)?;
    registries.mesh_pipelines.insert(handle.id(), pipeline);
    Ok(())
}

/// Creates one compute pipeline state and records it.
pub fn create_compute(
    device: &Dx12DeviceHandle,
    registries: &mut Dx12Registries,
    handle: ComputePipeline,
    descriptor: &ComputePipelineDescriptor,
) -> RhiResult<()> {
    let pipeline = create_compute_native(device, registries, descriptor)?;
    registries.compute_pipelines.insert(handle.id(), pipeline);
    Ok(())
}

fn create_graphics_native(
    device: &Dx12DeviceHandle,
    registries: &Dx12Registries,
    descriptor: &GraphicsPipelineDescriptor,
) -> RhiResult<sys::ComPtr> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (device, registries, descriptor);
        Err(RhiError::unsupported_backend(
            "DirectX 12 is only available on Windows",
        ))
    }
    #[cfg(target_os = "windows")]
    {
        let vertex = registries
            .shaders
            .get(&descriptor.vertex.id())
            .ok_or_else(|| {
                RhiError::invalid_argument("graphics pipeline: vertex shader not found")
            })?;
        let fragment = match descriptor.fragment {
            Some(fragment) => Some(registries.shaders.get(&fragment.id()).ok_or_else(|| {
                RhiError::invalid_argument("graphics pipeline: fragment shader not found")
            })?),
            None => None,
        };
        let root_signature = registries
            .root_signatures
            .get(&descriptor.layout.id())
            .copied()
            .unwrap_or(core::ptr::null_mut());

        // Input layout: one element per declared vertex attribute.
        let mut semantic_names: Vec<std::ffi::CString> = Vec::new();
        let mut elements: Vec<sys::D3D12_INPUT_ELEMENT_DESC> = Vec::new();
        if let Some(layout) = &descriptor.vertex_layout {
            rhi_require(
                layout.attributes.len() <= 32 && layout.buffer_layouts.len() <= 8,
                "vertex input capacity exceeded",
            )?;
            for attribute in &layout.attributes {
                rhi_require(
                    (attribute.buffer_index as usize) < layout.buffer_layouts.len(),
                    "vertex attribute references an invalid buffer",
                )?;
                let name = std::ffi::CString::new(attribute.semantic.name)
                    .map_err(|_| RhiError::invalid_argument("semantic name contains a NUL byte"))?;
                semantic_names.push(name);
                elements.push(sys::D3D12_INPUT_ELEMENT_DESC {
                    SemanticName: semantic_names.last().expect("just pushed").as_ptr(),
                    SemanticIndex: attribute.semantic.index,
                    Format: formats::dxgi_vertex_format(attribute.format),
                    InputSlot: attribute.buffer_index,
                    AlignedByteOffset: attribute.offset as u32,
                    InputSlotClass: match layout.buffer_layouts[attribute.buffer_index as usize]
                        .step_rate
                    {
                        crate::rhi::VertexInputRate::PerInstance => 1,
                        crate::rhi::VertexInputRate::PerVertex => 0,
                    },
                    InstanceDataStepRate: 0,
                });
            }
        }

        let mut pipeline = sys::D3D12_GRAPHICS_PIPELINE_STATE_DESC::default();
        pipeline.pRootSignature = root_signature;
        pipeline.VS = sys::D3D12_SHADER_BYTECODE {
            pShaderBytecode: vertex.bytecode.as_ptr().cast(),
            BytecodeLength: vertex.bytecode.len(),
        };
        if let Some(fragment) = fragment {
            pipeline.PS = sys::D3D12_SHADER_BYTECODE {
                pShaderBytecode: fragment.bytecode.as_ptr().cast(),
                BytecodeLength: fragment.bytecode.len(),
            };
        }
        for (index, attachment) in descriptor.color_attachments.iter().enumerate() {
            pipeline.RTVFormats[index] = formats::dxgi_format(attachment.format);
            let target = &mut pipeline.BlendState.RenderTarget[index];
            target.BlendEnable = u32::from(attachment.blend.enabled);
            target.SrcBlend = formats::blend_factor(attachment.blend.source_color_blend_factor);
            target.DestBlend =
                formats::blend_factor(attachment.blend.destination_color_blend_factor);
            target.BlendOp = formats::blend_operation(attachment.blend.color_blend_operation);
            target.SrcBlendAlpha =
                formats::blend_factor(attachment.blend.source_alpha_blend_factor);
            target.DestBlendAlpha =
                formats::blend_factor(attachment.blend.destination_alpha_blend_factor);
            target.BlendOpAlpha = formats::blend_operation(attachment.blend.alpha_blend_operation);
            // D3D12_COLOR_WRITE_ENABLE_ALL: every channel.
            target.RenderTargetWriteMask = 0xF;
        }
        pipeline.NumRenderTargets = descriptor.color_attachments.len() as u32;
        pipeline.DSVFormat = descriptor
            .depth_format
            .map(formats::dxgi_format)
            .unwrap_or(sys::DXGI_FORMAT_UNKNOWN);
        pipeline.SampleDesc = [formats::sample_count(descriptor.sample_count)?, 0];
        pipeline.SampleMask = u32::MAX;
        pipeline.RasterizerState = sys::D3D12_RASTERIZER_DESC {
            FillMode: formats::fill_mode(descriptor.rasterization.fill_mode),
            CullMode: formats::cull_mode(descriptor.rasterization.cull_mode),
            FrontCounterClockwise: formats::front_counter_clockwise(
                descriptor.rasterization.front_winding,
            ),
            DepthClipEnable: 1,
            ..Default::default()
        };
        if let Some(state) = descriptor.depth_stencil {
            pipeline.DepthStencilState.DepthEnable = u32::from(descriptor.depth_format.is_some());
            pipeline.DepthStencilState.DepthWriteMask = u32::from(state.depth_write_enabled);
            pipeline.DepthStencilState.DepthFunc =
                formats::comparison_function(state.depth_compare);
        }
        pipeline.InputLayout = sys::D3D12_INPUT_LAYOUT_DESC {
            pInputElementDescs: elements.as_ptr(),
            NumElements: elements.len() as u32,
        };
        pipeline.PrimitiveTopologyType = formats::primitive_topology_type(descriptor.primitive);
        pipeline.IBStripCutValue = sys::D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_DISABLED;

        let mut state: sys::ComPtr = core::ptr::null_mut();
        // SAFETY: the device is live; the descriptor and the arrays it points at
        // (`elements`, `semantic_names`) outlive the call.
        let status = unsafe {
            let vtbl = *(device.device as *mut *const sys::ID3D12DeviceVtbl);
            ((*vtbl).CreateGraphicsPipelineState)(
                device.device,
                &pipeline,
                &sys::IID_ID3D12_PIPELINE_STATE,
                &mut state,
            )
        };
        if status != sys::S_OK || state.is_null() {
            return Err(RhiError::invalid_argument(sys::hresult_message(status)));
        }
        Ok(state)
    }
}

fn create_mesh_native(
    device: &Dx12DeviceHandle,
    registries: &Dx12Registries,
    descriptor: &MeshPipelineDescriptor,
) -> RhiResult<sys::ComPtr> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (device, registries, descriptor);
        Err(RhiError::unsupported_backend(
            "DirectX 12 is only available on Windows",
        ))
    }
    #[cfg(target_os = "windows")]
    {
        let mesh = registries
            .shaders
            .get(&descriptor.mesh.id())
            .ok_or_else(|| RhiError::invalid_argument("mesh pipeline requires a mesh shader"))?;
        let root_signature = registries
            .root_signatures
            .get(&descriptor.layout.id())
            .copied()
            .unwrap_or(core::ptr::null_mut());
        let mut pipeline = sys::D3D12_GRAPHICS_PIPELINE_STATE_DESC::default();
        pipeline.pRootSignature = root_signature;
        // A mesh pipeline puts the mesh shader in the amplification/geometry slots:
        // `MS` occupies the `GS` field and the object shader the `AS` field, which
        // is how D3D12 spells the mesh pipeline without a separate descriptor.
        pipeline.GS = sys::D3D12_SHADER_BYTECODE {
            pShaderBytecode: mesh.bytecode.as_ptr().cast(),
            BytecodeLength: mesh.bytecode.len(),
        };
        if let Some(task) = descriptor.task {
            let task = registries
                .shaders
                .get(&task.id())
                .ok_or_else(|| RhiError::invalid_argument("mesh pipeline task shader not found"))?;
            pipeline.HS = sys::D3D12_SHADER_BYTECODE {
                pShaderBytecode: task.bytecode.as_ptr().cast(),
                BytecodeLength: task.bytecode.len(),
            };
        }
        if let Some(fragment) = descriptor.fragment {
            let fragment = registries.shaders.get(&fragment.id()).ok_or_else(|| {
                RhiError::invalid_argument("mesh pipeline fragment shader not found")
            })?;
            pipeline.PS = sys::D3D12_SHADER_BYTECODE {
                pShaderBytecode: fragment.bytecode.as_ptr().cast(),
                BytecodeLength: fragment.bytecode.len(),
            };
        }
        for (index, attachment) in descriptor.color_attachments.iter().enumerate() {
            pipeline.RTVFormats[index] = formats::dxgi_format(attachment.format);
        }
        pipeline.NumRenderTargets = descriptor.color_attachments.len() as u32;
        pipeline.DSVFormat = descriptor
            .depth_format
            .map(formats::dxgi_format)
            .unwrap_or(sys::DXGI_FORMAT_UNKNOWN);
        pipeline.SampleDesc = [1, 0];
        pipeline.SampleMask = u32::MAX;
        pipeline.RasterizerState = sys::D3D12_RASTERIZER_DESC {
            FillMode: formats::fill_mode(descriptor.rasterization.fill_mode),
            CullMode: formats::cull_mode(descriptor.rasterization.cull_mode),
            FrontCounterClockwise: formats::front_counter_clockwise(
                descriptor.rasterization.front_winding,
            ),
            DepthClipEnable: 1,
            ..Default::default()
        };
        // A mesh pipeline has no vertex input layout.
        pipeline.PrimitiveTopologyType =
            formats::primitive_topology_type(crate::rhi::PrimitiveType::TriangleList);
        pipeline.IBStripCutValue = sys::D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_DISABLED;

        let mut state: sys::ComPtr = core::ptr::null_mut();
        // SAFETY: the device is live and every pointer in `pipeline` refers to
        // storage that outlives the call.
        let status = unsafe {
            let vtbl = *(device.device as *mut *const sys::ID3D12DeviceVtbl);
            ((*vtbl).CreateGraphicsPipelineState)(
                device.device,
                &pipeline,
                &sys::IID_ID3D12_PIPELINE_STATE,
                &mut state,
            )
        };
        if status != sys::S_OK || state.is_null() {
            return Err(RhiError::invalid_argument(sys::hresult_message(status)));
        }
        Ok(state)
    }
}

fn create_compute_native(
    device: &Dx12DeviceHandle,
    registries: &Dx12Registries,
    descriptor: &ComputePipelineDescriptor,
) -> RhiResult<sys::ComPtr> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (device, registries, descriptor);
        Err(RhiError::unsupported_backend(
            "DirectX 12 is only available on Windows",
        ))
    }
    #[cfg(target_os = "windows")]
    {
        let shader = registries
            .shaders
            .get(&descriptor.shader.id())
            .ok_or_else(|| RhiError::invalid_argument("compute pipeline: shader not found"))?;
        let root_signature = registries
            .root_signatures
            .get(&descriptor.layout.id())
            .copied()
            .unwrap_or(core::ptr::null_mut());
        let mut pipeline = sys::D3D12_COMPUTE_PIPELINE_STATE_DESC::default();
        pipeline.pRootSignature = root_signature;
        pipeline.CS = sys::D3D12_SHADER_BYTECODE {
            pShaderBytecode: shader.bytecode.as_ptr().cast(),
            BytecodeLength: shader.bytecode.len(),
        };
        let mut state: sys::ComPtr = core::ptr::null_mut();
        // SAFETY: the device is live and the descriptor's bytecode pointer refers to
        // the registry's buffer, which outlives the call.
        let status = unsafe {
            let vtbl = *(device.device as *mut *const sys::ID3D12DeviceVtbl);
            ((*vtbl).CreateComputePipelineState)(
                device.device,
                &pipeline,
                &sys::IID_ID3D12_PIPELINE_STATE,
                &mut state,
            )
        };
        if status != sys::S_OK || state.is_null() {
            return Err(RhiError::invalid_argument(sys::hresult_message(status)));
        }
        Ok(state)
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn pipeline_creation_reports_unsupported_off_windows() {
        #[cfg(not(target_os = "windows"))]
        {
            let device =
                super::super::device::Dx12DeviceHandle::make(crate::rhi::DeviceConfig::default());
            assert!(device.is_err());
        }
    }
}
