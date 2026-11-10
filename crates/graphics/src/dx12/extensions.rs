//! DX12 commands that require additional device or command-list interfaces.
use crate::rhi::{Buffer, BufferUsage, RhiError, RhiResult, rhi_byte_range, rhi_require};
use windows::Win32::Graphics::Direct3D12::*;
use windows::core::Interface;

use super::resources::Dx12Registries;
use super::sys::ComPtr;

pub fn native_error(error: windows::core::Error) -> RhiError {
    if error.code().0 == super::sys::DXGI_ERROR_DEVICE_REMOVED {
        RhiError::DeviceLost
    } else {
        RhiError::invalid_argument(error.to_string())
    }
}

pub struct IndirectSignatures {
    draw: ID3D12CommandSignature,
    dispatch: ID3D12CommandSignature,
}

#[derive(Clone, Copy)]
pub enum IndirectCommand {
    Draw,
    Dispatch,
}

impl IndirectCommand {
    fn descriptor(self) -> (D3D12_INDIRECT_ARGUMENT_TYPE, u32) {
        match self {
            Self::Draw => (D3D12_INDIRECT_ARGUMENT_TYPE_DRAW, 16),
            Self::Dispatch => (D3D12_INDIRECT_ARGUMENT_TYPE_DISPATCH, 12),
        }
    }
}

impl IndirectSignatures {
    pub fn new(device: &ID3D12Device) -> RhiResult<Self> {
        let create = |command: IndirectCommand| {
            let (argument_type, stride) = command.descriptor();
            let argument = D3D12_INDIRECT_ARGUMENT_DESC {
                Type: argument_type,
                ..Default::default()
            };
            let descriptor = D3D12_COMMAND_SIGNATURE_DESC {
                ByteStride: stride,
                NumArgumentDescs: 1,
                pArgumentDescs: &argument,
                NodeMask: 0,
            };
            let mut signature = None;
            // SAFETY: the descriptor and argument are valid for the call. These
            // signatures never modify the root signature.
            unsafe {
                device.CreateCommandSignature(
                    &descriptor,
                    None::<&ID3D12RootSignature>,
                    &mut signature,
                )
            }
            .map_err(native_error)?;
            signature.ok_or_else(|| RhiError::invalid_argument("missing DX12 command signature"))
        };
        Ok(Self {
            draw: create(IndirectCommand::Draw)?,
            dispatch: create(IndirectCommand::Dispatch)?,
        })
    }

    pub fn encode(
        &self,
        list: ComPtr,
        registries: &Dx12Registries,
        command: IndirectCommand,
        buffer: Buffer,
        offset: usize,
        count: u32,
    ) -> RhiResult<()> {
        let record = registries
            .buffers
            .get(&buffer.id())
            .ok_or_else(|| RhiError::invalid_argument("unknown indirect argument buffer"))?;
        rhi_require(
            record.usage.contains(BufferUsage::INDIRECT) && offset.is_multiple_of(4),
            "indirect arguments require an INDIRECT buffer and 4-byte alignment",
        )?;
        let bytes = (count as usize)
            .checked_mul(command.descriptor().1 as usize)
            .ok_or_else(|| RhiError::invalid_argument("indirect argument range overflow"))?;
        rhi_byte_range(offset, bytes, record.size as usize)?;
        if count == 0 {
            return Ok(());
        }
        let signature = match command {
            IndirectCommand::Draw => &self.draw,
            IndirectCommand::Dispatch => &self.dispatch,
        };
        // SAFETY: the pointers are live ID3D12 objects and the planner declared
        // INDIRECT_ARGUMENT access for the validated argument range.
        unsafe {
            let list = ID3D12GraphicsCommandList::from_raw_borrowed(&list).unwrap();
            let arguments = ID3D12Resource::from_raw_borrowed(&record.resource).unwrap();
            list.ExecuteIndirect(
                signature,
                count,
                arguments,
                offset as u64,
                None::<&ID3D12Resource>,
                0,
            );
        }
        Ok(())
    }
}

pub fn dispatch_mesh(list: ComPtr, groups: crate::rhi::ThreadgroupSize) -> RhiResult<()> {
    groups.validate()?;
    // DispatchMesh limits each axis to 65535 and the product to 2^22.
    rhi_require(
        [groups.x, groups.y, groups.z]
            .into_iter()
            .all(|n| n <= 65_535)
            && groups.x * groups.y * groups.z <= (1 << 22),
        "DX12 mesh dispatch exceeds the group limit",
    )?;
    // SAFETY: the list is live and a mesh PSO was bound by the caller.
    let list = unsafe { ID3D12GraphicsCommandList::from_raw_borrowed(&list).unwrap() };
    let mesh = list
        .cast::<ID3D12GraphicsCommandList6>()
        .map_err(native_error)?;
    unsafe { mesh.DispatchMesh(groups.x as u32, groups.y as u32, groups.z as u32) };
    Ok(())
}
