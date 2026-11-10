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
    let state = create_graphics_native(device, registries, descriptor)?;
    // A pipeline carries the layout it was built with: the encoder needs it to
    // bind the matching root signature and root parameters at draw time.
    registries.graphics_pipelines.insert(
        handle.id(),
        super::resources::Dx12GraphicsPipelineRecord {
            pipeline: state,
            layout_id: descriptor.layout.id(),
            primitive: descriptor.primitive,
            vertex_layout: descriptor.vertex_layout.clone(),
        },
    );
    Ok(())
}

/// Creates a pipeline stream with mesh and optional amplification shaders.
pub fn create_mesh(
    device: &Dx12DeviceHandle,
    registries: &mut Dx12Registries,
    handle: MeshPipeline,
    descriptor: &MeshPipelineDescriptor,
) -> RhiResult<()> {
    let state = create_mesh_native(device, registries, descriptor)?;
    registries.mesh_pipelines.insert(
        handle.id(),
        super::resources::Dx12MeshPipelineRecord {
            pipeline: state,
            layout_id: descriptor.layout.id(),
            colors: descriptor
                .color_attachments
                .iter()
                .map(|attachment| attachment.format)
                .collect(),
            depth: descriptor.depth_format,
        },
    );
    Ok(())
}

/// Creates one compute pipeline state and records it.
pub fn create_compute(
    device: &Dx12DeviceHandle,
    registries: &mut Dx12Registries,
    handle: ComputePipeline,
    descriptor: &ComputePipelineDescriptor,
) -> RhiResult<()> {
    let state = create_compute_native(device, registries, descriptor)?;
    registries.compute_pipelines.insert(
        handle.id(),
        super::resources::Dx12ComputePipelineRecord {
            pipeline: state,
            layout_id: descriptor.layout.id(),
        },
    );
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
            .ok_or_else(|| RhiError::invalid_argument("pipeline references an unknown layout"))?;

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

        let mut pipeline = sys::D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            pRootSignature: root_signature,
            ..Default::default()
        };
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
        use windows::Win32::Graphics::Direct3D12::*;
        use windows::core::Interface;

        rhi_require(
            device.capabilities.mesh_shading.mesh,
            "DX12 adapter does not support mesh shaders",
        )?;
        rhi_require(
            descriptor.color_attachments.len() <= 8,
            "at most eight color attachments are supported",
        )?;

        let bytecode =
            |shader: Option<crate::rhi::ShaderModule>| -> RhiResult<sys::D3D12_SHADER_BYTECODE> {
                let Some(shader) = shader else {
                    return Ok(sys::D3D12_SHADER_BYTECODE::default());
                };
                let record = registries.shaders.get(&shader.id()).ok_or_else(|| {
                    RhiError::invalid_argument("mesh pipeline references an unknown shader")
                })?;
                Ok(sys::D3D12_SHADER_BYTECODE {
                    pShaderBytecode: record.bytecode.as_ptr().cast(),
                    BytecodeLength: record.bytecode.len(),
                })
            };
        let root_signature = registries
            .root_signatures
            .get(&descriptor.layout.id())
            .copied()
            .ok_or_else(|| RhiError::invalid_argument("pipeline references an unknown layout"))?;

        #[repr(C)]
        struct Subobject<T> {
            // The native stream aligns each subobject to the pointer size.
            alignment: [usize; 0],
            kind: D3D12_PIPELINE_STATE_SUBOBJECT_TYPE,
            value: T,
        }
        impl<T> Subobject<T> {
            fn new(kind: D3D12_PIPELINE_STATE_SUBOBJECT_TYPE, value: T) -> Self {
                Self {
                    alignment: [],
                    kind,
                    value,
                }
            }
        }
        #[repr(C)]
        struct Targets {
            formats: [u32; 8],
            count: u32,
        }
        #[repr(C)]
        struct MeshStream {
            root: Subobject<sys::ComPtr>,
            task: Subobject<sys::D3D12_SHADER_BYTECODE>,
            mesh: Subobject<sys::D3D12_SHADER_BYTECODE>,
            fragment: Subobject<sys::D3D12_SHADER_BYTECODE>,
            blend: Subobject<sys::D3D12_BLEND_DESC>,
            mask: Subobject<u32>,
            raster: Subobject<sys::D3D12_RASTERIZER_DESC>,
            depth: Subobject<sys::D3D12_DEPTH_STENCIL_DESC>,
            topology: Subobject<u32>,
            targets: Subobject<Targets>,
            depth_format: Subobject<u32>,
            samples: Subobject<[u32; 2]>,
        }

        let mut targets = Targets {
            formats: [0; 8],
            count: descriptor.color_attachments.len() as u32,
        };
        let mut blend = sys::D3D12_BLEND_DESC::default();
        blend.IndependentBlendEnable = 1;
        for (index, attachment) in descriptor.color_attachments.iter().enumerate() {
            targets.formats[index] = formats::dxgi_format(attachment.format);
            let target = &mut blend.RenderTarget[index];
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
            target.RenderTargetWriteMask = 0xF;
        }
        let raster = sys::D3D12_RASTERIZER_DESC {
            FillMode: formats::fill_mode(descriptor.rasterization.fill_mode),
            CullMode: formats::cull_mode(descriptor.rasterization.cull_mode),
            FrontCounterClockwise: formats::front_counter_clockwise(
                descriptor.rasterization.front_winding,
            ),
            DepthClipEnable: 1,
            ..Default::default()
        };
        let mut depth = sys::D3D12_DEPTH_STENCIL_DESC::default();
        if let Some(state) = descriptor.depth_stencil {
            depth.DepthEnable = u32::from(descriptor.depth_format.is_some());
            depth.DepthWriteMask = u32::from(state.depth_write_enabled);
            depth.DepthFunc = formats::comparison_function(state.depth_compare);
        }
        let stream = MeshStream {
            root: Subobject::new(
                D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_ROOT_SIGNATURE,
                root_signature,
            ),
            task: Subobject::new(
                D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_AS,
                bytecode(descriptor.task)?,
            ),
            mesh: Subobject::new(
                D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_MS,
                bytecode(Some(descriptor.mesh))?,
            ),
            fragment: Subobject::new(
                D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_PS,
                bytecode(descriptor.fragment)?,
            ),
            blend: Subobject::new(D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_BLEND, blend),
            mask: Subobject::new(D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_SAMPLE_MASK, u32::MAX),
            raster: Subobject::new(D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_RASTERIZER, raster),
            depth: Subobject::new(D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL, depth),
            topology: Subobject::new(
                D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_PRIMITIVE_TOPOLOGY,
                formats::primitive_topology_type(crate::rhi::PrimitiveType::TriangleList),
            ),
            targets: Subobject::new(
                D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_RENDER_TARGET_FORMATS,
                targets,
            ),
            depth_format: Subobject::new(
                D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL_FORMAT,
                descriptor
                    .depth_format
                    .map(formats::dxgi_format)
                    .unwrap_or(sys::DXGI_FORMAT_UNKNOWN),
            ),
            samples: Subobject::new(D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_SAMPLE_DESC, [1, 0]),
        };
        let description = D3D12_PIPELINE_STATE_STREAM_DESC {
            SizeInBytes: core::mem::size_of_val(&stream),
            pPipelineStateSubobjectStream: (&stream as *const MeshStream).cast_mut().cast(),
        };
        // SAFETY: the pointer came from CreateDevice and the stream keeps its
        // subobjects and borrowed shader bytecode live for pipeline creation.
        let native = unsafe { ID3D12Device::from_raw_borrowed(&device.device).unwrap() };
        let native = native
            .cast::<ID3D12Device2>()
            .map_err(super::extensions::native_error)?;
        let pipeline = unsafe { native.CreatePipelineState::<ID3D12PipelineState>(&description) }
            .map_err(super::extensions::native_error)?;
        Ok(pipeline.into_raw())
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
            .ok_or_else(|| RhiError::invalid_argument("pipeline references an unknown layout"))?;
        let mut pipeline = sys::D3D12_COMPUTE_PIPELINE_STATE_DESC {
            pRootSignature: root_signature,
            ..Default::default()
        };
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
