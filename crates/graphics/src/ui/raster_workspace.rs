use crate::rhi::*;
use std::sync::Arc;
pub(super) struct NativeUIRasterWorkspace {
    pub pixels: [usize; 2],
    pub format: TextureFormat,
    pub samples: usize,
    pub budget: usize,
    pub multisample: Option<Arc<TextureResource>>,
    pub strip: Option<Arc<TextureResource>>,
}
impl NativeUIRasterWorkspace {
    pub fn new(
        device: Arc<Device>,
        pixels: [usize; 2],
        format: TextureFormat,
        samples: usize,
        budget: usize,
        prefer_memoryless: bool,
    ) -> RhiResult<Self> {
        let (mut multisample, mut strip) = (None, None);
        if samples > 1 {
            let row = pixels[0]
                .checked_mul(if format == TextureFormat::Rgba16Float {
                    8
                } else {
                    4
                })
                .and_then(|n| n.checked_mul(samples))
                .ok_or_else(|| RhiError::invalid_argument("MSAA row size overflow"))?;
            let height = (budget / row.max(1)).max(1).min(pixels[1]);
            let attachment = |height| {
                let mut d =
                    TextureDescriptor::new(pixels[0], height, format, TextureUsage::COLOR_TARGET);
                d.sample_count = samples;
                d.transient_attachment = prefer_memoryless;
                d.label = Some("ui-raster-workspace");
                TextureResource::new(device.clone(), d).map(Arc::new)
            };
            let bounded = attachment(height)?;
            if bounded.allocation().is_some_and(|a| a.is_memoryless) && height < pixels[1] {
                multisample = Some(attachment(pixels[1])?);
            } else {
                multisample = Some(bounded);
                if height < pixels[1] {
                    let mut d = TextureDescriptor::new(
                        pixels[0],
                        height,
                        format,
                        TextureUsage::COLOR_TARGET | TextureUsage::SAMPLED,
                    );
                    d.label = Some("ui-strip-resolve");
                    strip = Some(Arc::new(TextureResource::new(device, d)?));
                }
            }
        }
        Ok(Self {
            pixels,
            format,
            samples,
            budget,
            multisample,
            strip,
        })
    }
    pub fn strip_height(&self) -> usize {
        self.multisample
            .as_ref()
            .map_or(self.pixels[1], |t| t.descriptor().height)
    }
    pub fn logical_bytes(&self) -> usize {
        self.multisample.as_ref().map_or(0, |t| {
            t.descriptor().width
                * t.descriptor().height
                * if self.format == TextureFormat::Rgba16Float {
                    8
                } else {
                    4
                }
                * self.samples
        })
    }
    pub fn estimated_bytes(&self) -> usize {
        let strip = self.strip.as_ref().map_or(0, |t| {
            t.descriptor().width
                * t.descriptor().height
                * if self.format == TextureFormat::Rgba16Float {
                    8
                } else {
                    4
                }
        });
        owned_bytes(self.memoryless(), self.logical_bytes(), strip)
    }
    pub fn allocated_bytes(&self) -> Option<usize> {
        match &self.multisample {
            None => Some(0),
            Some(t) => t
                .allocation()
                .and_then(|a| usize::try_from(a.allocated_bytes).ok()),
        }
    }
    pub fn memoryless(&self) -> bool {
        self.multisample
            .as_ref()
            .and_then(|t| t.allocation())
            .is_some_and(|a| a.is_memoryless)
    }
    pub fn target(&self, output: Texture, load_action: ColorLoadAction) -> RenderColorTarget {
        RenderColorTarget {
            texture: self.multisample.as_ref().map_or(output, |t| t.texture()),
            load_action,
            store: self.multisample.is_none(),
            resolve_texture: self
                .multisample
                .as_ref()
                .map(|_| self.strip.as_ref().map_or(output, |t| t.texture())),
        }
    }
}

/// Bytes a workspace actually owns. A tile-only multisample target reports a
/// logical size that never becomes device memory, so only its resolve strip
/// counts.
fn owned_bytes(memoryless: bool, logical: usize, strip: usize) -> usize {
    if memoryless { strip } else { logical + strip }
}

#[cfg(test)]
mod tests {
    use super::owned_bytes;

    #[test]
    fn a_memoryless_multisample_target_counts_only_its_resolve_strip() {
        assert_eq!(
            owned_bytes(true, 58_982_400, 1_048_576),
            1_048_576,
            "tile-only storage reports a logical bound, not bytes it owns"
        );
    }

    #[test]
    fn a_resident_multisample_target_counts_its_full_size() {
        assert_eq!(owned_bytes(false, 14_745_600, 0), 14_745_600);
    }
}
