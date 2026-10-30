//! Main-thread owner for rendering a view tree into an immutable draw snapshot.
use cove_core::scene::{
    color_glyph_atlas_id, ComponentSourceLocation, DrawListAtlasDirty, DrawListSnapshot,
    LayerAwareNodeRenderer, NodeTree, Recomposer, WindowRevealContext,
};
use cove_core::{Color, DrawList, Font, FontWeight, GeometryAntialiasing};
use cove_imaging::{ImageAssetRegistry, ImageAssetRegistryHolder};
use cove_text::{SystemFontDefaults, TextBootstrapOptions, TextCacheLimits, TextEnvironment, TextEnvironmentHolder};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use crate::{EmptyView, View, ViewGraph};

pub struct OffscreenView {
    tree: Rc<RefCell<NodeTree>>,
    recomposer: Rc<Recomposer>,
    graph: Rc<ViewGraph>,
    layer_renderer: LayerAwareNodeRenderer,
    draw_list: DrawList,
    atlas_texture_id: u32,
    image_assets: Arc<ImageAssetRegistry>,
    text_env: Option<TextEnvironment>,
    last_scale: f32,
    did_install_root: bool,
    cached_snapshot: Option<DrawListSnapshot>,
    recorded_scale: f32,
}

impl OffscreenView {
    pub fn new(geometry_antialiasing: GeometryAntialiasing) -> Self {
        let tree = Rc::new(RefCell::new(NodeTree::new()));
        let recomposer = Rc::new(Recomposer::new());
        let graph = Rc::new(ViewGraph::new(tree.clone(), recomposer.clone()));
        let mut draw_list = DrawList::default();
        draw_list.geometry_antialiasing = geometry_antialiasing;
        Self {
            tree,
            recomposer,
            graph,
            layer_renderer: LayerAwareNodeRenderer::new(),
            draw_list,
            atlas_texture_id: 1,
            image_assets: Arc::new(ImageAssetRegistry::default()),
            text_env: None,
            last_scale: 0.0,
            did_install_root: false,
            cached_snapshot: None,
            recorded_scale: 0.0,
        }
    }

    pub fn set_root_view<V: View + 'static>(&mut self, view: V) {
        if self.did_install_root { return; }
        self.ensure_text_environment(1.0);
        with_text_environment(self.text_env.clone(), self.image_assets.clone(), self.last_scale, || {
            self.graph.install(view, ComponentSourceLocation::default());
        });
        self.did_install_root = true;
    }

    pub fn render(&mut self, width: u32, height: u32, content_scale: f32) -> Option<DrawListSnapshot> {
        if width == 0 || height == 0 || !content_scale.is_finite() || content_scale <= 0.0 { return None; }
        if !self.did_install_root { self.set_root_view(EmptyView); }
        self.ensure_text_environment(content_scale);
        let text_env = self.text_env.clone();
        let image_assets = self.image_assets.clone();
        let installed_scale = self.last_scale;

        let mut recorded = false;
        with_text_environment(text_env, image_assets, installed_scale, || {
            self.recomposer.commit_all();
            let root = self.tree.borrow().root();
            WindowRevealContext::clear_activation(root.as_ref());
            self.graph.compute_layout_if_needed(width as f32, height as f32);
            let logical_size = [width as f32, height as f32];
            let atlas_dirty = self.text_env.as_ref().is_some_and(|environment| environment.atlas.is_dirty());
            let has_render_updates = self.tree.borrow().has_render_updates();
            if self.cached_snapshot.as_ref().is_some_and(|snapshot| snapshot.logical_size == logical_size)
                && self.recorded_scale == content_scale
                && !has_render_updates
                && !atlas_dirty
            {
                return;
            }
            self.draw_list.reset();
            self.draw_list.pixel_scale = content_scale;
            self.layer_renderer.render(self.graph.render_tree(), &mut self.draw_list);
            self.tree.borrow_mut().flush();
            self.recorded_scale = content_scale;
            recorded = true;
        });
        if !recorded { return self.cached_snapshot.clone(); }

        let mut atlas_updates = Vec::new();
        if let Some(environment) = &self.text_env {
            if let Some(payload) = environment.atlas.dirty_upload_payload() {
                atlas_updates.push(DrawListAtlasDirty::new(
                    payload.pixels,
                    payload.region.x as u32,
                    payload.region.y as u32,
                    payload.region.width as u32,
                    payload.region.height as u32,
                    payload.texture_width as u32,
                    payload.texture_height as u32,
                    self.atlas_texture_id,
                    cove_core::GlyphAtlasFormat::Alpha,
                ));
            }
            if let Some(payload) = environment.atlas.color_dirty_upload_payload() {
                atlas_updates.push(DrawListAtlasDirty::new(
                    payload.pixels,
                    payload.region.x as u32,
                    payload.region.y as u32,
                    payload.region.width as u32,
                    payload.region.height as u32,
                    payload.texture_width as u32,
                    payload.texture_height as u32,
                    color_glyph_atlas_id(self.atlas_texture_id),
                    cove_core::GlyphAtlasFormat::Color,
                ));
            }
            environment.atlas.mark_clean();
        }

        let snapshot = DrawListSnapshot {
            vertices: self.draw_list.vertices().to_vec(),
            indices: self.draw_list.indices().to_vec(),
            batches: self.draw_list.batches().to_vec(),
            logical_size: [width as f32, height as f32],
            atlas_updates,
            resources: self.draw_list.resources.clone(),
        };
        let mut reusable = snapshot.clone();
        reusable.atlas_updates.clear();
        self.cached_snapshot = Some(reusable);
        Some(snapshot)
    }

    pub fn trim_memory(&mut self) {
        if let Some(environment) = &self.text_env { environment.trim_text_caches(); }
        self.image_assets.clear();
        self.graph.render_tree().trim_caches();
        self.cached_snapshot = None;
        self.draw_list.reset();
    }

    pub fn graph(&self) -> &Rc<ViewGraph> { &self.graph }

    fn ensure_text_environment(&mut self, scale: f32) {
        let scale = scale.max(1.0);
        if (scale - self.last_scale).abs() <= 0.01 { return; }
        self.last_scale = scale;
        let atlas_edge = ((512.0 * scale).ceil().max(512.0) as usize).min(16_384);
        self.text_env = Some(TextEnvironment::bootstrapped(TextBootstrapOptions {
            atlas_texture_id: self.atlas_texture_id,
            primary_font_name: SystemFontDefaults::primary_font_name(),
            default_font: Font::system(16.0, FontWeight::Regular),
            default_line_height: 20.0,
            default_color: Color::WHITE,
            raster_scale: scale,
            atlas_edge: Some(atlas_edge),
            cache_limits: TextCacheLimits::default(),
        }));
        if let Some(root) = self.tree.borrow().root() {
            crate::modifiers::style::invalidate_text_style(&root);
        }
    }

}

fn with_text_environment<R>(
    text_env: Option<TextEnvironment>,
    image_assets: Arc<ImageAssetRegistry>,
    scale: f32,
    body: impl FnOnce() -> R,
) -> R {
    let previous_scale = cove_core::content_scale();
    struct RestoreScale(f32);
    impl Drop for RestoreScale { fn drop(&mut self) { cove_core::set_content_scale(self.0); } }
    let _restore = RestoreScale(previous_scale);
    cove_core::set_content_scale(if scale > 0.0 { scale } else { 1.0 });
    TextEnvironmentHolder::with_current(text_env, || ImageAssetRegistryHolder::with_current(image_assets, body))
}

impl Default for OffscreenView {
    fn default() -> Self { Self::new(GeometryAntialiasing::Analytic) }
}
