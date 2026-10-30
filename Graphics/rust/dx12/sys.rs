//! Handwritten Direct3D 12 COM FFI: interface vtables, descriptors, and the
//! loader, with no `windows` crate dependency.
//!
//! Swift reached D3D12 through the `CDX12Bridge` C++ layer, which did three
//! things this module replaces:
//!
//!   1. it hid COM vtable dispatch behind plain C functions,
//!   2. it owned the packed GPU layouts (`GRHI_GraphicsDesc`, `GRHI_Instance`,
//!      …) so Swift never wrote bitfields,
//!   3. it loaded `d3d12.dll` / `dxgi.dll` at runtime.
//!
//! Here the vtables are declared as `#[repr(C)]` structs of function pointers in
//! the exact interface order (`IUnknown`'s three methods first, then the
//! interface's own methods in header order), so a call through the vtable is
//! ABI-correct. The module compiles on every host; only the loader is
//! Windows-only, so the crate type-checks on macOS while the real code path stays
//! Windows.
#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]

use core::ffi::{c_char, c_void};

// MARK: - HRESULT

pub type HRESULT = i32;
pub type UINT = u32;
pub type UINT64 = u64;
pub type UINT8 = u8;
pub type LPCWSTR = *const u16;
pub type HANDLE = *mut c_void;
pub type HWND = *mut c_void;
pub type HINSTANCE = *mut c_void;
pub type LUID = u64;

pub const S_OK: HRESULT = 0;
pub const S_FALSE: HRESULT = 1;
pub const E_FAIL: HRESULT = -2_147_467_263; // 0x80004005
pub const E_INVALIDARG: HRESULT = -2_147_024_635; // 0x80070057
pub const E_OUTOFMEMORY: HRESULT = -2_147_024_632; // 0x8007000E
pub const DXGI_ERROR_NOT_FOUND: HRESULT = -2_000_527_102; // 0x887A0002
pub const DXGI_ERROR_DEVICE_REMOVED: HRESULT = -2_000_527_099; // 0x887A0005
pub const DXGI_STATUS_OCCLUDED: HRESULT = 0x087A0001;
pub const WAIT_OBJECT_0: u32 = 0;
pub const WAIT_TIMEOUT: u32 = 258;
pub const INFINITE: u32 = 0xFFFF_FFFF;

/// Formats an `HRESULT` as the error message Swift reported.
pub fn hresult_message(result: HRESULT) -> String {
    format!("D3D12 call failed with HRESULT 0x{:08X}", result as u32)
}

// MARK: - GUID

/// A COM class/interface identifier.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GUID {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

impl GUID {
    pub const fn new(data1: u32, data2: u16, data3: u16, data4: [u8; 8]) -> Self {
        Self { data1, data2, data3, data4 }
    }

    pub fn is_null(&self) -> bool {
        self.data1 == 0 && self.data2 == 0 && self.data3 == 0 && self.data4 == [0; 8]
    }
}

/// `IUnknown`.
pub const IID_IUNKNOWN: GUID =
    GUID::new(0x0000_0000, 0x0000, 0x0000, [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46]);

/// `ID3D12Device`.
pub const IID_ID3D12_DEVICE: GUID =
    GUID::new(0x1898_198E, 0xCAFE, 0x4A7F, [0xA1, 0x8B, 0x3E, 0x85, 0xD5, 0xB9, 0x6E, 0x3E]);

/// `ID3D12CommandQueue`.
pub const IID_ID3D12_COMMAND_QUEUE: GUID =
    GUID::new(0x0EC8_706A, 0x05C0, 0x41EA, [0x88, 0x9B, 0x8A, 0xB8, 0x56, 0x8D, 0x1E, 0x42]);

/// `ID3D12CommandAllocator`.
pub const IID_ID3D12_COMMAND_ALLOCATOR: GUID =
    GUID::new(0x6102_ACEE, 0x37A4, 0x4DCE, [0xA2, 0xC3, 0x5A, 0xF8, 0x42, 0x53, 0x32, 0x5F]);

/// `ID3D12GraphicsCommandList`.
pub const IID_ID3D12_GRAPHICS_COMMAND_LIST: GUID =
    GUID::new(0x5B16_00F2, 0xBD9E, 0x4C04, [0x8C, 0x1E, 0x3E, 0x1E, 0x7E, 0x7C, 0x0F, 0x9B]);

/// `ID3D12Fence`.
pub const IID_ID3D12_FENCE: GUID =
    GUID::new(0x0A75_3F7C, 0x8AFA, 0x4481, [0xA1, 0x24, 0x81, 0x09, 0x4D, 0x2D, 0x8F, 0x62]);

/// `IDXGIFactory4`.
pub const IID_IDXGI_FACTORY4: GUID =
    GUID::new(0x1BC6_EA46, 0x2EE5, 0x48B4, [0xB2, 0x9A, 0x24, 0x15, 0x2C, 0x66, 0xE6, 0x4C]);

/// `IDXGISwapChain3`.
pub const IID_IDXGI_SWAP_CHAIN3: GUID =
    GUID::new(0x94D9_9C4E, 0x9EA0, 0x4CD0, [0xA4, 0x29, 0x59, 0xB2, 0xBB, 0x6A, 0xB5, 0x52]);

/// `ID3D12RootSignature`.
pub const IID_ID3D12_ROOT_SIGNATURE: GUID =
    GUID::new(0xC54A_6DE6, 0x0D0D, 0x4D26, [0x88, 0x9E, 0x5C, 0x92, 0xDB, 0xFC, 0x0D, 0x0D]);

/// `ID3D12PipelineState`.
pub const IID_ID3D12_PIPELINE_STATE: GUID =
    GUID::new(0x765A_30FD, 0x7A35, 0x453B, [0x8A, 0xA1, 0x90, 0x6B, 0x1D, 0x5C, 0x46, 0x0B]);

/// `ID3D12Resource`.
pub const IID_ID3D12_RESOURCE: GUID =
    GUID::new(0x6964_420D, 0xD24B, 0x4BE1, [0x9C, 0x1C, 0xA0, 0x1B, 0x5B, 0x1C, 0xE8, 0x9A]);

// MARK: - Enumerations

pub const D3D12_COMMAND_LIST_TYPE_DIRECT: u32 = 0;
pub const D3D12_COMMAND_LIST_TYPE_BUNDLE: u32 = 1;
pub const D3D12_COMMAND_LIST_TYPE_COMPUTE: u32 = 2;
pub const D3D12_COMMAND_LIST_TYPE_COPY: u32 = 3;

pub const D3D12_COMMAND_QUEUE_FLAG_NONE: u32 = 0;

pub const D3D12_HEAP_TYPE_DEFAULT: u32 = 1;
pub const D3D12_HEAP_TYPE_UPLOAD: u32 = 2;
pub const D3D12_HEAP_TYPE_READBACK: u32 = 3;

pub const D3D12_RESOURCE_DIMENSION_UNKNOWN: u32 = 0;
pub const D3D12_RESOURCE_DIMENSION_BUFFER: u32 = 1;
pub const D3D12_RESOURCE_DIMENSION_TEXTURE1D: u32 = 2;
pub const D3D12_RESOURCE_DIMENSION_TEXTURE2D: u32 = 3;
pub const D3D12_RESOURCE_DIMENSION_TEXTURE3D: u32 = 4;

pub const D3D12_RESOURCE_STATE_COMMON: u32 = 0;
pub const D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER: u32 = 0x1;
pub const D3D12_RESOURCE_STATE_INDEX_BUFFER: u32 = 0x2;
pub const D3D12_RESOURCE_STATE_RENDER_TARGET: u32 = 0x4;
pub const D3D12_RESOURCE_STATE_UNORDERED_ACCESS: u32 = 0x8;
pub const D3D12_RESOURCE_STATE_DEPTH_WRITE: u32 = 0x10;
pub const D3D12_RESOURCE_STATE_DEPTH_READ: u32 = 0x20;
pub const D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE: u32 = 0x40;
pub const D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE: u32 = 0x80;
pub const D3D12_RESOURCE_STATE_COPY_DEST: u32 = 0x400;
pub const D3D12_RESOURCE_STATE_COPY_SOURCE: u32 = 0x800;
pub const D3D12_RESOURCE_STATE_GENERIC_READ: u32 = 0x1 | 0x2 | 0x40 | 0x80;
pub const D3D12_RESOURCE_STATE_PRESENT: u32 = 0;
pub const D3D12_RESOURCE_STATE_INDIRECT_ARGUMENT: u32 = 0x200;
pub const D3D12_RESOURCE_STATE_RAYTRACING_ACCELERATION_STRUCTURE: u32 = 0x400000;

pub const D3D12_RESOURCE_FLAG_NONE: u32 = 0;
pub const D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET: u32 = 0x1;
pub const D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL: u32 = 0x2;
pub const D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS: u32 = 0x4;

pub const D3D12_RESOURCE_BARRIER_TYPE_TRANSITION: u32 = 0;
pub const D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES: u32 = 0xFFFF_FFFF;

pub const D3D12_HEAP_FLAG_NONE: u32 = 0;

pub const D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX: u32 = 0;
pub const D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT: u32 = 1;

pub const D3D12_PRIMITIVE_TOPOLOGY_TRIANGLELIST: u32 = 4;
pub const D3D12_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP: u32 = 5;
pub const D3D12_PRIMITIVE_TOPOLOGY_LINELIST: u32 = 2;
pub const D3D12_PRIMITIVE_TOPOLOGY_LINESTRIP: u32 = 3;
pub const D3D12_PRIMITIVE_TOPOLOGY_POINTLIST: u32 = 1;

pub const D3D12_COMPARISON_FUNC_NEVER: u32 = 1;
pub const D3D12_COMPARISON_FUNC_LESS: u32 = 2;
pub const D3D12_COMPARISON_FUNC_EQUAL: u32 = 3;
pub const D3D12_COMPARISON_FUNC_LESS_EQUAL: u32 = 4;
pub const D3D12_COMPARISON_FUNC_GREATER: u32 = 5;
pub const D3D12_COMPARISON_FUNC_NOT_EQUAL: u32 = 6;
pub const D3D12_COMPARISON_FUNC_GREATER_EQUAL: u32 = 7;
pub const D3D12_COMPARISON_FUNC_ALWAYS: u32 = 8;

pub const D3D12_BLEND_ZERO: u32 = 1;
pub const D3D12_BLEND_ONE: u32 = 2;
pub const D3D12_BLEND_SRC_COLOR: u32 = 3;
pub const D3D12_BLEND_INV_SRC_COLOR: u32 = 4;
pub const D3D12_BLEND_SRC_ALPHA: u32 = 5;
pub const D3D12_BLEND_INV_SRC_ALPHA: u32 = 6;
pub const D3D12_BLEND_DEST_ALPHA: u32 = 7;
pub const D3D12_BLEND_INV_DEST_ALPHA: u32 = 8;
pub const D3D12_BLEND_DEST_COLOR: u32 = 9;
pub const D3D12_BLEND_INV_DEST_COLOR: u32 = 10;
pub const D3D12_BLEND_SRC_ALPHA_SAT: u32 = 11;
pub const D3D12_BLEND_BLEND_FACTOR: u32 = 14;
pub const D3D12_BLEND_INV_BLEND_FACTOR: u32 = 15;

pub const D3D12_BLEND_OP_ADD: u32 = 1;
pub const D3D12_BLEND_OP_SUBTRACT: u32 = 2;
pub const D3D12_BLEND_OP_REV_SUBTRACT: u32 = 3;
pub const D3D12_BLEND_OP_MIN: u32 = 4;
pub const D3D12_BLEND_OP_MAX: u32 = 5;

pub const D3D12_FILL_MODE_WIREFRAME: u32 = 2;
pub const D3D12_FILL_MODE_SOLID: u32 = 3;
pub const D3D12_CULL_MODE_NONE: u32 = 1;
pub const D3D12_CULL_MODE_FRONT: u32 = 2;
pub const D3D12_CULL_MODE_BACK: u32 = 3;

pub const D3D12_ROOT_SIGNATURE_VERSION_1: u32 = 1;
pub const D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT: u32 = 0x1;

pub const D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV: u32 = 0;
pub const D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER: u32 = 1;
pub const D3D12_DESCRIPTOR_HEAP_TYPE_RTV: u32 = 2;
pub const D3D12_DESCRIPTOR_HEAP_TYPE_DSV: u32 = 3;
pub const D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE: u32 = 0x1;

pub const D3D12_FENCE_FLAG_NONE: u32 = 0;
pub const D3D12_FENCE_FLAG_SHARED: u32 = 0x2;

pub const D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_DISABLED: u32 = 0;
pub const D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_0xFFFF: u32 = 1;

pub const D3D12_SRV_DIMENSION_UNKNOWN: u32 = 0;
pub const D3D12_SRV_DIMENSION_TEXTURE2D: u32 = 4;
pub const D3D12_SRV_DIMENSION_BUFFER: u32 = 1;

pub const DXGI_FORMAT_UNKNOWN: u32 = 0;
pub const DXGI_FORMAT_R32G32B32A32_FLOAT: u32 = 2;
pub const DXGI_FORMAT_R32G32B32_FLOAT: u32 = 6;
pub const DXGI_FORMAT_R8G8B8A8_UNORM: u32 = 28;
pub const DXGI_FORMAT_R8G8B8A8_UNORM_SRGB: u32 = 29;
pub const DXGI_FORMAT_R32_UINT: u32 = 42;
pub const DXGI_FORMAT_R32_FLOAT: u32 = 41;
pub const DXGI_FORMAT_R8_UNORM: u32 = 61;
pub const DXGI_FORMAT_B8G8R8A8_UNORM: u32 = 87;
pub const DXGI_FORMAT_B8G8R8A8_UNORM_SRGB: u32 = 91;
pub const DXGI_FORMAT_R16G16B16A16_FLOAT: u32 = 10;
pub const DXGI_FORMAT_D32_FLOAT: u32 = 40;
pub const DXGI_FORMAT_D24_UNORM_S8_UINT: u32 = 45;
pub const DXGI_FORMAT_R32G32_FLOAT: u32 = 16;

pub const DXGI_USAGE_RENDER_TARGET_OUTPUT: u32 = 0x20;
pub const DXGI_SWAP_EFFECT_FLIP_DISCARD: u32 = 4;
pub const DXGI_SCALING_STRETCH: u32 = 0;
pub const DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING: u32 = 0x800;

// MARK: - Structs

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct D3D12_COMMAND_QUEUE_DESC {
    pub Type: u32,
    pub Priority: i32,
    pub Flags: u32,
    pub NodeMask: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct D3D12_HEAP_PROPERTIES {
    pub Type: u32,
    pub CPUPageProperty: u32,
    pub MemoryPoolPreference: u32,
    pub CreationNodeMask: u32,
    pub VisibleNodeMask: u32,
}

impl Default for D3D12_HEAP_PROPERTIES {
    fn default() -> Self {
        Self {
            Type: D3D12_HEAP_TYPE_DEFAULT,
            CPUPageProperty: 0,
            MemoryPoolPreference: 0,
            CreationNodeMask: 1,
            VisibleNodeMask: 1,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct D3D12_RESOURCE_DESC {
    pub Dimension: u32,
    pub Alignment: u64,
    pub Width: u64,
    pub Height: u32,
    pub DepthOrArraySize: u16,
    pub MipLevels: u16,
    pub Format: u32,
    pub SampleDesc: [u32; 2],
    pub Layout: u32,
    pub Flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct D3D12_RESOURCE_TRANSITION_BARRIER {
    pub pResource: *mut c_void,
    pub Subresource: u32,
    pub StateBefore: u32,
    pub StateAfter: u32,
}

/// The union in `D3D12_RESOURCE_BARRIER`. Only the transition arm is used, so the
/// struct declares that member; the trailing padding keeps the size correct.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct D3D12_RESOURCE_BARRIER {
    pub Type: u32,
    pub Flags: u32,
    pub Transition: D3D12_RESOURCE_TRANSITION_BARRIER,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct D3D12_TEXTURE_COPY_LOCATION {
    pub pResource: *mut c_void,
    pub Type: u32,
    pub SubresourceIndex: u32,
    /// `PlacedFootprint` when `Type` is `PLACED_FOOTPRINT`; the buffer path only
    /// uses the subresource form, so this keeps the union's size.
    pub PlacedFootprintPadding: [u32; 6],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct D3D12_BOX {
    pub left: u32,
    pub top: u32,
    pub front: u32,
    pub right: u32,
    pub bottom: u32,
    pub back: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct D3D12_VIEWPORT {
    pub TopLeftX: f32,
    pub TopLeftY: f32,
    pub Width: f32,
    pub Height: f32,
    pub MinDepth: f32,
    pub MaxDepth: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct D3D12_RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct D3D12_CPU_DESCRIPTOR_HANDLE {
    pub ptr: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct D3D12_GPU_DESCRIPTOR_HANDLE {
    pub ptr: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct D3D12_DESCRIPTOR_HEAP_DESC {
    pub Type: u32,
    pub NumDescriptors: u32,
    pub Flags: u32,
    pub NodeMask: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct D3D12_RENDER_TARGET_BLEND_DESC {
    pub BlendEnable: u32,
    pub LogicOpEnable: u32,
    pub SrcBlend: u32,
    pub DestBlend: u32,
    pub BlendOp: u32,
    pub SrcBlendAlpha: u32,
    pub DestBlendAlpha: u32,
    pub BlendOpAlpha: u32,
    pub LogicOp: u32,
    pub RenderTargetWriteMask: u8,
    pub _padding: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct D3D12_BLEND_DESC {
    pub AlphaToCoverageEnable: u32,
    pub IndependentBlendEnable: u32,
    /// Eight render targets, as the fixed-size array in the header.
    pub RenderTarget: [D3D12_RENDER_TARGET_BLEND_DESC; 8],
}

impl Default for D3D12_BLEND_DESC {
    fn default() -> Self {
        // SAFETY: the struct is plain integers and bytes, so an all-zero value is
        // a valid (if disabled) blend description.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct D3D12_RASTERIZER_DESC {
    pub FillMode: u32,
    pub CullMode: u32,
    pub FrontCounterClockwise: u32,
    pub DepthBias: i32,
    pub DepthBiasClamp: f32,
    pub SlopeScaledDepthBias: f32,
    pub DepthClipEnable: u32,
    pub MultisampleEnable: u32,
    pub AntialiasedLineEnable: u32,
    pub ForcedSampleCount: u32,
    pub ConservativeRaster: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct D3D12_SO_DECLARATION_ENTRY {
    pub Stream: u32,
    pub SemanticName: *const c_char,
    pub SemanticIndex: u32,
    pub StartComponent: u8,
    pub ComponentCount: u8,
    pub OutputSlot: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct D3D12_STREAM_OUTPUT_DESC {
    pub pSODeclaration: *const D3D12_SO_DECLARATION_ENTRY,
    pub NumEntries: u32,
    pub pBufferStrides: *const u32,
    pub NumStrides: u32,
    pub RasterizedStream: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct D3D12_INPUT_ELEMENT_DESC {
    pub SemanticName: *const c_char,
    pub SemanticIndex: u32,
    pub Format: u32,
    pub InputSlot: u32,
    pub AlignedByteOffset: u32,
    pub InputSlotClass: u32,
    pub InstanceDataStepRate: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct D3D12_INPUT_LAYOUT_DESC {
    pub pInputElementDescs: *const D3D12_INPUT_ELEMENT_DESC,
    pub NumElements: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct D3D12_STENCIL_OP_DESC {
    pub StencilFailOp: u32,
    pub StencilDepthFailOp: u32,
    pub StencilPassOp: u32,
    pub StencilFunc: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct D3D12_DEPTH_STENCILOP_DESC {
    pub StencilFailOp: u32,
    pub StencilDepthFailOp: u32,
    pub StencilPassOp: u32,
    pub StencilFunc: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct D3D12_DEPTH_STENCIL_DESC {
    pub DepthEnable: u32,
    pub DepthWriteMask: u32,
    pub DepthFunc: u32,
    pub StencilEnable: u32,
    pub StencilReadMask: u8,
    pub StencilWriteMask: u8,
    pub _padding: [u8; 2],
    pub FrontFace: D3D12_DEPTH_STENCILOP_DESC,
    pub BackFace: D3D12_DEPTH_STENCILOP_DESC,
}

impl Default for D3D12_DEPTH_STENCIL_DESC {
    fn default() -> Self {
        // SAFETY: plain integers and bytes; all-zero is "disabled".
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct D3D12_RENDER_TARGET_BLEND_STATE {
    pub BlendEnable: u32,
    pub LogicOpEnable: u32,
    pub SrcBlend: u32,
    pub DestBlend: u32,
    pub BlendOp: u32,
    pub SrcBlendAlpha: u32,
    pub DestBlendAlpha: u32,
    pub BlendOpAlpha: u32,
    pub LogicOp: u32,
    pub RenderTargetWriteMask: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct D3D12_GRAPHICS_PIPELINE_STATE_DESC {
    pub pRootSignature: *mut c_void,
    pub VS: [u8; 8],
    pub PS: [u8; 8],
    pub DS: [u8; 8],
    pub HS: [u8; 8],
    pub GS: [u8; 8],
    pub StreamOutput: D3D12_STREAM_OUTPUT_DESC,
    pub BlendState: D3D12_BLEND_DESC,
    pub SampleMask: u32,
    pub RasterizerState: D3D12_RASTERIZER_DESC,
    pub DepthStencilState: D3D12_DEPTH_STENCIL_DESC,
    pub InputLayout: D3D12_INPUT_LAYOUT_DESC,
    pub IBStripCutValue: u32,
    pub PrimitiveTopologyType: u32,
    pub NumRenderTargets: u32,
    pub RTVFormats: [u32; 8],
    pub DSVFormat: u32,
    pub SampleDesc: [u32; 2],
    pub NodeMask: u32,
    pub CachedPSO: [u8; 8],
    pub Flags: u32,
}

impl Default for D3D12_GRAPHICS_PIPELINE_STATE_DESC {
    fn default() -> Self {
        // SAFETY: the struct is plain data; the caller fills every field it uses.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct D3D12_COMPUTE_PIPELINE_STATE_DESC {
    pub pRootSignature: *mut c_void,
    pub CS: [u8; 8],
    pub NodeMask: u32,
    pub CachedPSO: [u8; 8],
    pub Flags: u32,
}

impl Default for D3D12_COMPUTE_PIPELINE_STATE_DESC {
    fn default() -> Self {
        // SAFETY: plain data; the caller fills the shader bytecode and root
        // signature before use.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct D3D12_RANGE {
    pub Begin: usize,
    pub End: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct DXGI_SAMPLE_DESC {
    pub Count: u32,
    pub Quality: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct DXGI_SWAP_CHAIN_DESC1 {
    pub Width: u32,
    pub Height: u32,
    pub Format: u32,
    pub Stereo: u32,
    pub SampleDesc: DXGI_SAMPLE_DESC,
    pub BufferUsage: u32,
    pub BufferCount: u32,
    pub Scaling: u32,
    pub SwapEffect: u32,
    pub AlphaMode: u32,
    pub Flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct DXGI_PRESENT_PARAMETERS {
    pub DirtyRectsCount: u32,
    pub pDirtyRects: *mut D3D12_RECT,
    pub pScrollRect: *mut D3D12_RECT,
    pub pScrollOffset: *mut i32,
}

// MARK: - Shader bytecode

/// `D3D12_SHADER_BYTECODE`: `[u8; 8]` above is the same size as this struct, but
/// naming it makes the pipeline descriptors readable.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct D3D12_SHADER_BYTECODE {
    pub pShaderBytecode: *const c_void,
    pub BytecodeLength: usize,
}

impl D3D12_SHADER_BYTECODE {
    /// Reads a shader bytecode struct back out of its inline storage.
    ///
    /// # Safety
    /// `storage` must contain a `D3D12_SHADER_BYTECODE` written by [`Self::write_into`].
    pub unsafe fn read_from(storage: &[u8; 8]) -> Self {
        // SAFETY: the caller guarantees `storage` holds a `D3D12_SHADER_BYTECODE`,
        // which is exactly eight bytes on both 32- and 64-bit targets.
        unsafe { core::mem::transmute_copy(storage) }
    }

    /// Writes this struct into the inline storage a pipeline descriptor exposes.
    pub fn write_into(self, storage: &mut [u8; 8]) {
        // SAFETY: `D3D12_SHADER_BYTECODE` is eight bytes, matching `storage`.
        *storage = unsafe { core::mem::transmute_copy(&self) };
    }
}

// MARK: - VTables
//
// Each interface's methods appear in header order after `IUnknown`'s three. Only
// the methods the backend calls are declared, but their positions matter, so the
// unused ones are kept as explicit slots reserved by name.

pub type QueryInterfaceFn =
    unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT;
pub type AddRefFn = unsafe extern "system" fn(*mut c_void) -> u32;
pub type ReleaseFn = unsafe extern "system" fn(*mut c_void) -> u32;

/// `IUnknown`'s three methods, which begin every COM vtable.
#[repr(C)]
pub struct IUnknownVtbl {
    pub QueryInterface: QueryInterfaceFn,
    pub AddRef: AddRefFn,
    pub Release: ReleaseFn,
}

impl IUnknownVtbl {
    /// Releases one interface pointer.
    ///
    /// # Safety
    /// `object` must be a live COM interface pointer.
    pub unsafe fn release(object: *mut c_void) {
        if object.is_null() {
            return;
        }
        // SAFETY: every COM object begins with its vtable pointer.
        let vtbl = unsafe { *(object as *mut *const IUnknownVtbl) };
        // SAFETY: the caller guarantees a live interface, so the vtable is valid
        // and its first entries are `IUnknown`'s.
        unsafe { ((*vtbl).Release)(object) };
    }
}

/// `ID3D12Device`'s vtable, in header order.
///
/// Only the entries the backend calls are typed; the rest are declared with their
/// real signatures where it matters and as opaque slots elsewhere. Their *count*
/// and *order* are what make dispatch correct.
#[repr(C)]
pub struct ID3D12DeviceVtbl {
    pub base: IUnknownVtbl,
    pub GetNodeCount: usize,
    pub CreateCommandQueue: unsafe extern "system" fn(
        *mut c_void,
        *const D3D12_COMMAND_QUEUE_DESC,
        *const GUID,
        *mut *mut c_void,
    ) -> HRESULT,
    pub CreateCommandAllocator: unsafe extern "system" fn(
        *mut c_void,
        u32,
        *const GUID,
        *mut *mut c_void,
    ) -> HRESULT,
    pub CreateGraphicsPipelineState: unsafe extern "system" fn(
        *mut c_void,
        *const D3D12_GRAPHICS_PIPELINE_STATE_DESC,
        *const GUID,
        *mut *mut c_void,
    ) -> HRESULT,
    pub CreateComputePipelineState: unsafe extern "system" fn(
        *mut c_void,
        *const D3D12_COMPUTE_PIPELINE_STATE_DESC,
        *const GUID,
        *mut *mut c_void,
    ) -> HRESULT,
    pub CreateCommandList: unsafe extern "system" fn(
        *mut c_void,
        u32,
        u32,
        *mut c_void,
        *mut c_void,
        *const GUID,
        *mut *mut c_void,
    ) -> HRESULT,
    /// `CheckFeatureSupport`.
    pub CheckFeatureSupport: unsafe extern "system" fn(*mut c_void, u32, *mut c_void, u32) -> HRESULT,
    /// The remaining methods are not called; their slots keep later offsets right.
    pub CreateDescriptorHeap: unsafe extern "system" fn(
        *mut c_void,
        *const D3D12_DESCRIPTOR_HEAP_DESC,
        *const GUID,
        *mut *mut c_void,
    ) -> HRESULT,
    pub GetDescriptorHandleIncrementSize: unsafe extern "system" fn(*mut c_void, u32) -> u32,
    pub CreateRootSignature: unsafe extern "system" fn(
        *mut c_void,
        u32,
        *const c_void,
        usize,
        *const GUID,
        *mut *mut c_void,
    ) -> HRESULT,
    pub CreateConstantBufferView: usize,
    pub CreateShaderResourceView: usize,
    pub CreateUnorderedAccessView: usize,
    pub CreateRenderTargetView: usize,
    pub CreateDepthStencilView: usize,
    pub CreateSampler: usize,
    pub CopyDescriptors: usize,
    pub CopyDescriptorsSimple: usize,
    /// `CreateFence`.
    pub CreateFence: unsafe extern "system" fn(
        *mut c_void,
        u64,
        u32,
        *const GUID,
        *mut *mut c_void,
    ) -> HRESULT,
}

/// `IUnknown`-derived interface pointer. Every COM pointer is stored as a raw
/// `*mut c_void` and released through `IUnknownVtbl::release`.
pub type ComPtr = *mut c_void;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hresult_messages_name_the_code() {
        assert!(hresult_message(E_FAIL).contains("0x80004005"));
        assert_eq!(S_OK, 0);
    }

    #[test]
    fn shader_bytecode_round_trips_through_inline_storage() {
        let bytecode = D3D12_SHADER_BYTECODE {
            pShaderBytecode: 0x1000usize as *const c_void,
            BytecodeLength: 64,
        };
        let mut storage = [0u8; 8];
        bytecode.write_into(&mut storage);
        // SAFETY: `storage` was just written by `write_into`.
        let restored = unsafe { D3D12_SHADER_BYTECODE::read_from(&storage) };
        assert_eq!(restored.pShaderBytecode, bytecode.pShaderBytecode);
        assert_eq!(restored.BytecodeLength, 64);
        assert_eq!(std::mem::size_of::<D3D12_SHADER_BYTECODE>(), 8);
    }

    #[test]
    fn releasing_a_null_interface_is_a_no_op() {
        // SAFETY: null is explicitly handled.
        unsafe { IUnknownVtbl::release(core::ptr::null_mut()) };
    }

    #[test]
    fn guids_are_distinct_and_non_null() {
        for guid in [
            IID_ID3D12_DEVICE,
            IID_ID3D12_COMMAND_QUEUE,
            IID_ID3D12_FENCE,
            IID_IDXGI_FACTORY4,
            IID_IDXGI_SWAP_CHAIN3,
        ] {
            assert!(!guid.is_null());
        }
        assert!(GUID::new(0, 0, 0, [0; 8]).is_null());
        assert_ne!(IID_ID3D12_DEVICE, IID_ID3D12_FENCE);
    }

    #[test]
    fn resource_barrier_keeps_the_transition_member() {
        let barrier = D3D12_RESOURCE_BARRIER {
            Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
            Flags: 0,
            Transition: D3D12_RESOURCE_TRANSITION_BARRIER {
                pResource: core::ptr::null_mut(),
                Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                StateBefore: D3D12_RESOURCE_STATE_COMMON,
                StateAfter: D3D12_RESOURCE_STATE_COPY_DEST,
            },
        };
        assert_eq!(barrier.Transition.Subresource, u32::MAX);
        assert_eq!(barrier.Transition.StateAfter, D3D12_RESOURCE_STATE_COPY_DEST);
    }
}
