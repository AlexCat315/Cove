//! Premultiplied DXGI presentation over the Win32 system backdrop.
use crate::rhi::{RhiError, RhiResult, SurfaceDescriptor, TextureFormat};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::{
    Direct3D12::ID3D12CommandQueue,
    DirectComposition::*,
    Dwm::*,
    Dxgi::{self, Common::*},
};
use windows::Win32::UI::Controls::MARGINS;
use windows::core::Interface;

use super::{device::Dx12DeviceHandle, formats, sys};

pub struct CompositionTarget {
    device: IDCompositionDevice,
    target: IDCompositionTarget,
}

impl Drop for CompositionTarget {
    fn drop(&mut self) {
        // SAFETY: the target belongs to this device. Unlink the visual before a
        // replacement swapchain binds another root to the same HWND.
        unsafe {
            let _ = self.target.SetRoot(None::<&IDCompositionVisual>);
            let _ = self.device.Commit();
        }
    }
}

pub fn create(
    device: &Dx12DeviceHandle,
    surface: &SurfaceDescriptor,
) -> RhiResult<(sys::ComPtr, CompositionTarget)> {
    let hwnd = HWND(surface.native_handle as *mut _);
    let descriptor = Dxgi::DXGI_SWAP_CHAIN_DESC1 {
        Width: surface.width as u32,
        Height: surface.height as u32,
        Format: DXGI_FORMAT(formats::swapchain_format(surface.color_format) as i32),
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: Dxgi::DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 2,
        Scaling: Dxgi::DXGI_SCALING_STRETCH,
        SwapEffect: Dxgi::DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
        AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
        ..Default::default()
    };
    let make = || -> windows::core::Result<_> {
        // SAFETY: the factory, direct queue and HWND remain live during setup.
        unsafe {
            let factory = Dxgi::IDXGIFactory2::from_raw_borrowed(&device.factory).unwrap();
            let queue = ID3D12CommandQueue::from_raw_borrowed(&device.graphics.queue).unwrap();
            let chain = factory.CreateSwapChainForComposition(
                queue,
                &descriptor,
                None::<&Dxgi::IDXGIOutput>,
            )?;
            if surface.color_format == TextureFormat::Rgba16Float {
                chain
                    .cast::<Dxgi::IDXGISwapChain3>()?
                    .SetColorSpace1(DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709)?;
            }
            let compositor: IDCompositionDevice =
                DCompositionCreateDevice(None::<&Dxgi::IDXGIDevice>)?;
            // Keep the swap chain below native child windows such as XAML Islands.
            let target = compositor.CreateTargetForHwnd(hwnd, false)?;
            let visual = compositor.CreateVisual()?;
            visual.SetContent(&chain)?;
            target.SetRoot(&visual)?;
            compositor.Commit()?;

            // The extended frame exposes DWM behind the premultiplied visual.
            let margins = MARGINS {
                cxLeftWidth: -1,
                cxRightWidth: -1,
                cyTopHeight: -1,
                cyBottomHeight: -1,
            };
            DwmExtendFrameIntoClientArea(hwnd, &margins)?;
            // Desktop Acrylic is available on newer Windows 11 builds. Older
            // versions still composite the transparent pixels over the frame.
            let acrylic = DWMSBT_TRANSIENTWINDOW;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_SYSTEMBACKDROP_TYPE,
                (&acrylic as *const DWM_SYSTEMBACKDROP_TYPE).cast(),
                core::mem::size_of_val(&acrylic) as u32,
            );
            Ok((
                chain.into_raw(),
                CompositionTarget {
                    device: compositor,
                    target,
                },
            ))
        }
    };
    make().map_err(|error| RhiError::make_swapchain_failed(format!("DirectComposition: {error}")))
}
