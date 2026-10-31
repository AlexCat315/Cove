//! Translation of NativeRHI/Resources/{SwapchainResource,TextureResource}.swift:
//! shared-ownership handles whose release schedules deferred destruction.
use std::sync::Arc;

use super::{
    Device, RhiResult, SurfaceDescriptor, Swapchain, Texture, TextureAllocation, TextureDescriptor,
};

/// Owns one native window swapchain. Retain the platform window until this
/// resource has been released and its device has completed pending work.
pub struct SwapchainResource {
    device: Arc<Device>,
    swapchain: Swapchain,
    closed: bool,
}
impl SwapchainResource {
    pub fn new(device: Arc<Device>, descriptor: SurfaceDescriptor) -> RhiResult<Self> {
        let swapchain = device.make_swapchain(descriptor)?;
        Ok(Self {
            device,
            swapchain,
            closed: false,
        })
    }

    pub fn swapchain(&self) -> Swapchain {
        self.swapchain
    }

    pub fn device(&self) -> &Arc<Device> {
        &self.device
    }

    /// Waits for pending work and releases this window's native swapchain.
    /// Idempotent; call before destroying the platform window.
    pub fn close(&mut self) -> RhiResult<()> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        self.device
            .with_frame_session(|| self.device.close_swapchain(self.swapchain))
    }
}
impl Drop for SwapchainResource {
    fn drop(&mut self) {
        if !self.closed {
            self.device.destroy_swapchain(self.swapchain);
        }
    }
}

/// Shared ownership of one device texture. Borrow [`Self::texture`] for
/// recording; retain this resource until the recorded work has been submitted.
/// Release schedules destruction after the owning frame's GPU work retires.
pub struct TextureResource {
    device: Arc<Device>,
    descriptor: TextureDescriptor,
    texture: Texture,
    allocation: Option<TextureAllocation>,
}
impl TextureResource {
    pub fn new(device: Arc<Device>, descriptor: TextureDescriptor) -> RhiResult<Self> {
        let texture = device.make_texture(descriptor)?;
        let allocation = device.texture_allocation(texture);
        Ok(Self {
            device,
            descriptor,
            texture,
            allocation,
        })
    }

    pub fn texture(&self) -> Texture {
        self.texture
    }

    pub fn descriptor(&self) -> &TextureDescriptor {
        &self.descriptor
    }

    pub fn allocation(&self) -> Option<TextureAllocation> {
        self.allocation
    }

    pub fn device(&self) -> &Arc<Device> {
        &self.device
    }
}
impl Drop for TextureResource {
    fn drop(&mut self) {
        self.device.destroy_texture(self.texture);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_swapchain_resource_is_closed_once() {
        // The resource only needs the identity of the handle for this check; a
        // real device is exercised by the backend integration tests.
        let device = Arc::new(
            Device::new(
                Box::new(crate::rhi::testing::NullBackend::new()),
                crate::rhi::DeviceConfig::default(),
            )
            .expect("null device"),
        );
        let mut resource = SwapchainResource::new(
            Arc::clone(&device),
            SurfaceDescriptor::new(0x1000).with_size(64, 64),
        )
        .expect("swapchain");
        assert!(resource.close().is_ok());
        assert!(resource.close().is_ok());
    }

    #[test]
    fn a_texture_resource_reports_its_descriptor() {
        let device = Arc::new(
            Device::new(
                Box::new(crate::rhi::testing::NullBackend::new()),
                crate::rhi::DeviceConfig::default(),
            )
            .expect("null device"),
        );
        let descriptor = TextureDescriptor::new(
            8,
            8,
            crate::rhi::TextureFormat::Rgba8Unorm,
            crate::rhi::TextureUsage::SAMPLED,
        );
        let resource = TextureResource::new(Arc::clone(&device), descriptor).expect("texture");
        assert_eq!(resource.descriptor().width, 8);
        assert_eq!(resource.texture().id(), 1);
        assert!(resource.allocation().is_none());
    }
}
