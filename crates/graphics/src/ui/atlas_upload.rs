use super::draw_list_renderer::NativeDrawListRenderer;
use crate::rhi::*;
use cove_core::{TextureId, color_glyph_atlas_id};
use cove_text::{FontAtlas, UploadPayload};

/// The dirty regions of a font atlas, copied out so the GPU upload can run on
/// the render thread in order with the frames that use them.
pub struct FontAtlasUpload {
    texture_id: TextureId,
    size: [usize; 2],
    alpha: Option<UploadPayload>,
    color: Option<UploadPayload>,
}

impl FontAtlasUpload {
    /// Copies the atlas's pending regions and marks it clean. `None` when
    /// nothing changed.
    pub fn take(atlas: &FontAtlas, texture_id: TextureId) -> RhiResult<Option<Self>> {
        if texture_id == 0 || texture_id >= 0x8000_0000 {
            return Err(RhiError::invalid_argument("invalid font atlas texture ID"));
        }
        let alpha = atlas.dirty_upload_payload();
        let color = atlas.color_dirty_upload_payload();
        atlas.mark_clean();
        if alpha.is_none() && color.is_none() {
            return Ok(None);
        }
        Ok(Some(Self {
            texture_id,
            size: [atlas.width(), atlas.height()],
            alpha,
            color,
        }))
    }
}

impl NativeDrawListRenderer {
    pub fn upload_font_atlas(&self, atlas: &FontAtlas, texture_id: TextureId) -> RhiResult<()> {
        match FontAtlasUpload::take(atlas, texture_id)? {
            Some(upload) => self.apply_font_atlas_upload(upload),
            None => Ok(()),
        }
    }

    /// Writes copied atlas regions into the shared texture store.
    pub fn apply_font_atlas_upload(&self, upload: FontAtlasUpload) -> RhiResult<()> {
        if let Some(payload) = upload.alpha {
            let r = payload.region;
            self.register_texture(
                upload.texture_id,
                payload.pixels,
                upload.size,
                TextureUploadRegion::with_origin(
                    r.width as i64,
                    r.height as i64,
                    r.x as i64,
                    r.y as i64,
                ),
                TextureFormat::R8Unorm,
            )?;
        }
        if let Some(payload) = upload.color {
            let r = payload.region;
            self.register_texture(
                color_glyph_atlas_id(upload.texture_id),
                payload.pixels,
                [payload.texture_width, payload.texture_height],
                TextureUploadRegion::with_origin(
                    r.width as i64,
                    r.height as i64,
                    r.x as i64,
                    r.y as i64,
                ),
                TextureFormat::Rgba8Unorm,
            )?;
        }
        Ok(())
    }
}
