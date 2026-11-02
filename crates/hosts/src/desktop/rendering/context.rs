//! Translation of Desktop/Sources/CoveUI/Rendering/Context.swift.

use super::render_thread::ThreadedWindowRenderer;
use crate::desktop::application::configuration::{AppWindowRendering, ApplicationConfiguration};
use crate::desktop::window::native_renderer::{
    AppWindowRenderSettings, AppWindowRenderer, NativeAppWindowRenderer,
};
use cove_core::{GeometryAntialiasing, TextureId};
use cove_graphics::rhi::{Device, RhiResult, TextureFormat};
use cove_graphics::ui::{FontAtlasUpload, NativeDrawListRenderer};
use cove_text::FontAtlas;
use std::sync::Arc;

impl AppWindowRenderSettings {
    /// Resolves this window's settings against the application defaults. Analytic
    /// geometry antialiasing is single-sample by definition, so it wins over an
    /// authored `sampleCount`.
    pub fn resolve(config: &ApplicationConfiguration, window: AppWindowRendering) -> Self {
        let analytic = window
            .geometry_antialiasing
            .unwrap_or(config.rendering.geometry_antialiasing)
            == GeometryAntialiasing::Analytic;
        let samples = if analytic {
            1
        } else {
            window
                .sample_count
                .unwrap_or(config.rendering.sample_count)
                .max(1)
        };
        Self {
            samples,
            clear_color: window.clear_color.unwrap_or(config.rendering.clear_color),
        }
    }
}

/// Shares UI bindings, fonts and image residency across application windows.
pub struct AppRenderingContext {
    device: Arc<Device>,
    renderer: NativeDrawListRenderer,
}

impl AppRenderingContext {
    pub fn new(device: Option<Arc<Device>>, config: &ApplicationConfiguration) -> RhiResult<Self> {
        let device = match device {
            Some(device) => device,
            None => Arc::new(Device::make(config.rendering.device.clone())?),
        };
        let renderer = {
            let device = device.clone();
            device.with_frame_session(|| NativeDrawListRenderer::new(device.clone()))?
        };
        Ok(Self { device, renderer })
    }

    pub fn device(&self) -> Arc<Device> {
        self.device.clone()
    }

    /// Builds the presentation renderer for one window. The sibling shares the
    /// texture store so a glyph atlas uploaded once serves every window.
    pub fn make_window(
        &self,
        settings: AppWindowRenderSettings,
    ) -> RhiResult<Box<dyn AppWindowRenderer>> {
        let device = self.device.clone();
        let presentation = device.with_frame_session(|| {
            self.renderer.make_presentation_renderer(
                TextureFormat::Bgra8UnormSrgb,
                settings.samples as usize,
            )
        })?;
        let renderer = Box::new(NativeAppWindowRenderer::new(device, presentation, settings));
        if std::env::var_os("COVE_SYNC_RENDER").is_some_and(|value| value == "1") {
            return Ok(renderer);
        }
        Ok(Box::new(ThreadedWindowRenderer::spawn(renderer, "window")))
    }

    /// Copies the atlas's pending regions for a window renderer to upload in
    /// order with its frames.
    pub fn take_font_atlas_upload(
        &self,
        atlas: &FontAtlas,
        texture_id: TextureId,
    ) -> RhiResult<Option<FontAtlasUpload>> {
        FontAtlasUpload::take(atlas, texture_id)
    }

    pub fn upload_font_atlas(&self, atlas: &FontAtlas, texture_id: TextureId) -> RhiResult<()> {
        let device = self.device.clone();
        device.with_frame_session(|| self.renderer.upload_font_atlas(atlas, texture_id))
    }
}
