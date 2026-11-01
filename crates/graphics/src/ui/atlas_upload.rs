use super::draw_list_renderer::NativeDrawListRenderer;
use crate::rhi::*;
use cove_core::{TextureId, color_glyph_atlas_id};
use cove_text::FontAtlas;
impl NativeDrawListRenderer {
    pub fn upload_font_atlas(&self, atlas: &FontAtlas, texture_id: TextureId) -> RhiResult<()> {
        if texture_id == 0 || texture_id >= 0x8000_0000 {
            return Err(RhiError::invalid_argument("invalid font atlas texture ID"));
        }
        if let Some(payload) = atlas.dirty_upload_payload() {
            let r = payload.region;
            self.register_texture(
                texture_id,
                payload.pixels,
                [atlas.width(), atlas.height()],
                TextureUploadRegion::with_origin(
                    r.width as i64,
                    r.height as i64,
                    r.x as i64,
                    r.y as i64,
                ),
                TextureFormat::R8Unorm,
            )?;
        }
        if let Some(payload) = atlas.color_dirty_upload_payload() {
            let r = payload.region;
            self.register_texture(
                color_glyph_atlas_id(texture_id),
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
        atlas.mark_clean();
        Ok(())
    }
}
