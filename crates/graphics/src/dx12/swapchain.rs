//! Swapchain creation, acquire and present for the DX12 backend.
//!
//! Flip-model swapchains present by index, so an "acquired image" is just the
//! current back buffer. Swift's C++ bridge tracked this; the Rust port keeps the
//! same state per window.
#![allow(non_snake_case)]
use crate::rhi::{RhiError, RhiResult, SurfaceDescriptor, Swapchain, SwapchainImage, Texture};

use super::device::{Dx12DeviceHandle, Dx12Swapchain};
#[cfg(any(target_os = "windows", test))]
use super::formats;
use super::resources::Dx12Registries;
use super::sys;
#[cfg(test)]
use crate::rhi::TextureFormat;

/// Creates the swapchain, its back buffers and their render-target views.
pub fn create(
    device: &mut Dx12DeviceHandle,
    registries: &std::sync::Mutex<Dx12Registries>,
    handle: Swapchain,
    descriptor: &SurfaceDescriptor,
    generation: u64,
) -> RhiResult<()> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (device, registries, handle, descriptor, generation);
        Err(RhiError::unsupported_backend(
            "DirectX 12 is only available on Windows",
        ))
    }
    #[cfg(target_os = "windows")]
    {
        let width = descriptor.width;
        let height = descriptor.height;
        let format = formats::swapchain_format(descriptor.color_format);
        let buffer_count = 2u32;
        let description = sys::DXGI_SWAP_CHAIN_DESC1 {
            Width: width as u32,
            Height: height as u32,
            Format: format,
            Stereo: 0,
            SampleDesc: sys::DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: sys::DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: buffer_count,
            Scaling: sys::DXGI_SCALING_STRETCH,
            SwapEffect: sys::DXGI_SWAP_EFFECT_FLIP_DISCARD,
            AlphaMode: 0,
            Flags: if descriptor.vsync_enabled {
                0
            } else {
                sys::DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING
            },
        };
        let mut swapchain1: sys::ComPtr = core::ptr::null_mut();
        // SAFETY: the factory and the HWND are live; the vtable entry matches
        // `CreateSwapChainForHwnd`, which takes the queue, the window and the
        // description.
        let status = unsafe {
            let vtbl = *(device.factory as *mut *const FactoryVtbl);
            ((*vtbl).CreateSwapChainForHwnd)(
                device.factory,
                device.graphics.queue,
                descriptor.native_handle as sys::HWND,
                &description,
                core::ptr::null(),
                core::ptr::null_mut(),
                &mut swapchain1,
            )
        };
        if status != sys::S_OK || swapchain1.is_null() {
            return Err(RhiError::make_swapchain_failed(sys::hresult_message(
                status,
            )));
        }
        let mut swapchain: sys::ComPtr = core::ptr::null_mut();
        // SAFETY: the factory returned a live IDXGISwapChain1. Query the
        // interface that owns GetCurrentBackBufferIndex before using it.
        let status = unsafe {
            let vtbl = *(swapchain1 as *mut *const sys::IUnknownVtbl);
            let status =
                ((*vtbl).QueryInterface)(swapchain1, &sys::IID_IDXGI_SWAP_CHAIN3, &mut swapchain);
            sys::IUnknownVtbl::release(swapchain1);
            status
        };
        if status != sys::S_OK || swapchain.is_null() {
            return Err(RhiError::make_swapchain_failed(sys::hresult_message(
                status,
            )));
        }

        // Wrap each back buffer as a render target. The frontend renders to the
        // handle returned by `acquire`, so the buffer must be registered as a
        // texture *and* given a real RTV; a null descriptor would make every
        // render pass fail to bind its attachment.
        let mut guard = registries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut buffers = Vec::with_capacity(buffer_count as usize);
        let mut texture_ids = Vec::with_capacity(buffer_count as usize);
        let mut render_targets = Vec::with_capacity(buffer_count as usize);
        let outcome: RhiResult<()> = (|| {
            for index in 0..buffer_count {
                let mut resource: sys::ComPtr = core::ptr::null_mut();
                // SAFETY: the swapchain is live and the index is inside its buffer count.
                let status = unsafe {
                    let vtbl = *(swapchain as *mut *const SwapChainVtbl);
                    ((*vtbl).GetBuffer)(swapchain, index, &sys::IID_ID3D12_RESOURCE, &mut resource)
                };
                if status != sys::S_OK || resource.is_null() {
                    return Err(RhiError::make_swapchain_failed(sys::hresult_message(
                        status,
                    )));
                }
                let id = device.next_internal_id();
                let rtv_index = device.take_rtv_index()?;
                let rtv = super::texture::rtv_heap_cpu(device, rtv_index)?;
                // SAFETY: the device and back buffer are live; a null description asks
                // D3D12 for a default full-resource view, which is what a swapchain
                // back buffer needs.
                unsafe {
                    let vtbl = *(device.device as *mut *const sys::ID3D12DeviceVtbl);
                    ((*vtbl).CreateRenderTargetView)(
                        device.device,
                        resource,
                        core::ptr::null(),
                        rtv,
                    );
                }
                guard.textures.insert(
                    id,
                    super::resources::Dx12TextureRecord {
                        resource,
                        format: descriptor.color_format,
                        width,
                        height,
                        depth: 1,
                        layers: 1,
                        mip_levels: 1,
                        sample_count: 1,
                        dimension: crate::rhi::TextureDimension::Texture2D,
                        usage: crate::rhi::TextureUsage::COLOR_TARGET,
                        rtv: Some(rtv),
                        dsv: None,
                    },
                );
                buffers.push(resource);
                texture_ids.push(id);
                render_targets.push(rtv);
            }
            Ok(())
        })();
        if let Err(error) = outcome {
            // The registry must not keep records whose COM pointers are about to
            // be released by the failure path below.
            for id in &texture_ids {
                guard.textures.remove(id);
            }
            // SAFETY: the buffers wrapped so far are released before the
            // swapchain, which owns the rest.
            unsafe {
                for buffer in &buffers {
                    sys::IUnknownVtbl::release(*buffer);
                }
                sys::IUnknownVtbl::release(swapchain);
            }
            return Err(error);
        }
        drop(guard);
        device.swapchains.insert(
            handle.id(),
            Dx12Swapchain {
                swapchain,
                width,
                height,
                format: descriptor.color_format,
                vsync: descriptor.vsync_enabled,
                generation,
                buffers,
                texture_ids,
                render_targets,
                current_index: 0,
                acquired: None,
            },
        );
        Ok(())
    }
}

/// Releases one window's swapchain and its buffers, returning the texture
/// handles the caller must drop from the resource registry. Those records hold
/// the same COM pointers, so removing them (without releasing again) prevents a
/// double free when the registries are torn down.
pub fn destroy(device: &mut Dx12DeviceHandle, handle: Swapchain) -> Vec<u32> {
    let Some(chain) = device.swapchains.remove(&handle.id()) else {
        return Vec::new();
    };
    let texture_ids = chain.texture_ids.clone();
    // SAFETY: the frame ring waited for every presentation, and each pointer is
    // released exactly once.
    unsafe {
        for buffer in chain.buffers {
            sys::IUnknownVtbl::release(buffer);
        }
        sys::IUnknownVtbl::release(chain.swapchain);
    }
    texture_ids
}

/// Returns the current back buffer as a swapchain image.
pub fn acquire(device: &mut Dx12DeviceHandle, handle: Swapchain) -> RhiResult<SwapchainImage> {
    let Some(chain) = device.swapchains.get_mut(&handle.id()) else {
        return Err(RhiError::SwapchainAcquireFailed(
            "configure a swapchain and return its acquired image first".into(),
        ));
    };
    if chain.acquired.is_some() {
        return Err(RhiError::SwapchainAcquireFailed(
            "present the acquired back buffer before acquiring another".into(),
        ));
    }
    let index = current_index(chain.swapchain).unwrap_or(chain.current_index);
    let texture = *chain
        .texture_ids
        .get(index as usize)
        .ok_or_else(|| RhiError::SwapchainAcquireFailed("back-buffer index out of range".into()))?;
    chain.current_index = index;
    chain.acquired = Some(index);
    Ok(SwapchainImage::new(
        handle,
        chain.generation,
        Texture::new(texture),
        chain.width,
        chain.height,
    ))
}

/// Queues one present for the acquired back buffer.
pub fn present(device: &mut Dx12DeviceHandle, image: SwapchainImage) -> RhiResult<()> {
    let Some(chain) = device.swapchains.get_mut(&image.swapchain.id()) else {
        return Err(RhiError::present_failed(
            "image was not acquired from this swapchain",
        ));
    };
    if chain.acquired.is_none() {
        return Err(RhiError::present_failed(
            "image was not acquired from this swapchain",
        ));
    }
    chain.acquired = None;
    present_swapchain(chain)?;
    Ok(())
}

/// `IDXGISwapChain3::GetCurrentBackBufferIndex`.
fn current_index(swapchain: sys::ComPtr) -> Option<u32> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = swapchain;
        None
    }
    #[cfg(target_os = "windows")]
    {
        // SAFETY: the swapchain is live and the call has no preconditions.
        let index = unsafe {
            let vtbl = *(swapchain as *mut *const SwapChainVtbl);
            ((*vtbl).GetCurrentBackBufferIndex)(swapchain)
        };
        Some(index)
    }
}

/// Queues one `Present`.
fn present_swapchain(chain: &Dx12Swapchain) -> RhiResult<()> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = chain;
        Err(RhiError::unsupported_backend(
            "DirectX 12 is only available on Windows",
        ))
    }
    #[cfg(target_os = "windows")]
    {
        // A VSync present uses interval 1; an unsynchronized one uses 0 with the
        // tearing flag the swapchain was created with.
        let (interval, flags) = if chain.vsync {
            (1u32, 0u32)
        } else {
            (0u32, sys::DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING)
        };
        // SAFETY: the swapchain is live and holds the presented buffers.
        let status = unsafe {
            let vtbl = *(chain.swapchain as *mut *const SwapChainVtbl);
            ((*vtbl).Present)(chain.swapchain, interval, flags)
        };
        if status != sys::S_OK && status != sys::DXGI_STATUS_OCCLUDED {
            return Err(RhiError::present_failed(sys::hresult_message(status)));
        }
        Ok(())
    }
}

/// `IDXGISwapChain3`'s vtable up to `GetCurrentBackBufferIndex`, in header
/// order: `IDXGIObject`, `IDXGIDeviceSubObject`, then each swap-chain revision.
#[repr(C)]
struct SwapChainVtbl {
    base: sys::IUnknownVtbl,
    // IDXGIObject
    SetPrivateData: usize,
    SetPrivateDataInterface: usize,
    GetPrivateData: usize,
    GetParent: usize,
    // IDXGIDeviceSubObject
    GetDevice: usize,
    // IDXGISwapChain
    Present: unsafe extern "system" fn(sys::ComPtr, u32, u32) -> sys::HRESULT,
    GetBuffer: unsafe extern "system" fn(
        sys::ComPtr,
        u32,
        *const sys::GUID,
        *mut sys::ComPtr,
    ) -> sys::HRESULT,
    SetFullscreenState: usize,
    GetFullscreenState: usize,
    GetDesc: usize,
    ResizeBuffers: usize,
    ResizeTarget: usize,
    GetContainingOutput: usize,
    GetFrameStatistics: usize,
    GetLastPresentCount: usize,
    // IDXGISwapChain1
    GetDesc1: usize,
    GetFullscreenDesc: usize,
    GetHwnd: usize,
    GetCoreWindow: usize,
    Present1: usize,
    IsTemporaryMonoSupported: usize,
    GetRestrictToOutput: usize,
    SetBackgroundColor: usize,
    GetBackgroundColor: usize,
    SetRotation: usize,
    GetRotation: usize,
    // IDXGISwapChain2
    SetSourceSize: usize,
    GetSourceSize: usize,
    SetMaximumFrameLatency: usize,
    GetMaximumFrameLatency: usize,
    GetFrameLatencyWaitableObject: usize,
    SetMatrixTransform: usize,
    GetMatrixTransform: usize,
    // IDXGISwapChain3
    GetCurrentBackBufferIndex: unsafe extern "system" fn(sys::ComPtr) -> u32,
}

/// `IDXGIFactory`'s vtable prefix up to `CreateSwapChainForHwnd`.
#[repr(C)]
struct FactoryVtbl {
    base: sys::IUnknownVtbl,
    SetPrivateData: usize,
    SetPrivateDataInterface: usize,
    GetPrivateData: usize,
    GetParent: usize,
    EnumAdapters: usize,
    MakeWindowAssociation: usize,
    GetWindowAssociation: usize,
    CreateSwapChain: usize,
    CreateSoftwareAdapter: usize,
    EnumAdapters1: usize,
    IsCurrent: usize,
    IsWindowedStereoEnabled: usize,
    CreateSwapChainForHwnd: unsafe extern "system" fn(
        sys::ComPtr,
        sys::ComPtr,
        sys::HWND,
        *const sys::DXGI_SWAP_CHAIN_DESC1,
        *const sys::DXGI_SWAP_CHAIN_FULLSCREEN_DESC,
        *mut sys::ComPtr,
        *mut sys::ComPtr,
    ) -> sys::HRESULT,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dx12_swapchain_creation_reports_unsupported_off_windows() {
        #[cfg(not(target_os = "windows"))]
        {
            let mut device = Dx12DeviceHandle::make(crate::rhi::DeviceConfig::default());
            assert!(device.is_err());
            let _ = &mut device;
        }
    }

    #[test]
    fn a_texture_format_has_a_swapchain_form() {
        assert_eq!(
            formats::swapchain_format(TextureFormat::Bgra8UnormSrgb),
            sys::DXGI_FORMAT_B8G8R8A8_UNORM
        );
    }
}
