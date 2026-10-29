//! Translation of Android/Integration/Environment.swift.

use cove_core::scene::UIWorkSchedulerHolder;
use cove_core::{set_content_scale, Font, FontWeight};
use cove_imaging::{ImageAssetRegistry, ImageAssetRegistryHolder};
use cove_text::{
    SystemFontDefaults, TextBootstrapOptions, TextEnvironment, TextEnvironmentHolder,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

/// Per-scene font/image services and a queue drained only on Android's UI thread.
pub struct SceneEnvironment {
    images: Arc<ImageAssetRegistry>,
    text: Option<TextEnvironment>,
    raster_scale: Option<f32>,
    work: Rc<RefCell<Vec<Rc<dyn Fn()>>>>,
}

impl SceneEnvironment {
    pub fn new() -> Self {
        Self {
            images: Arc::new(ImageAssetRegistry::default()),
            text: None,
            raster_scale: None,
            work: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn images(&self) -> &Arc<ImageAssetRegistry> {
        &self.images
    }

    pub fn text(&self) -> Option<&TextEnvironment> {
        self.text.as_ref()
    }

    pub fn has_work(&self) -> bool {
        !self.work.borrow().is_empty()
    }

    pub fn configure(&mut self, scale: f32) {
        let raster_scale = scale.clamp(1.0, 4.0);
        if self.raster_scale == Some(raster_scale) {
            return;
        }
        self.raster_scale = Some(raster_scale);
        self.text = Some(TextEnvironment::bootstrapped(TextBootstrapOptions {
            atlas_texture_id: 1,
            primary_font_name: SystemFontDefaults::primary_font_name(),
            default_font: Font::system(16.0, FontWeight::Regular),
            default_line_height: 22.0,
            raster_scale,
            atlas_edge: Some(2048),
            ..TextBootstrapOptions::default()
        }));
    }

    /// Makes this scene's text, image, work-scheduler and scale ambients current
    /// for the duration of `body`, restoring the previous holders on exit.
    pub fn with_current<R>(&self, scale: f32, body: impl FnOnce() -> R) -> R {
        let previous_text = TextEnvironmentHolder::current();
        let previous_images = ImageAssetRegistryHolder::current();
        let previous_scale = cove_core::content_scale();
        TextEnvironmentHolder::set_current(self.text.clone());
        ImageAssetRegistryHolder::set_current(Some(self.images.clone()));
        set_content_scale(scale);
        let queue = self.work.clone();
        UIWorkSchedulerHolder::set_enqueue(Some(Rc::new(move |work: Rc<dyn Fn()>| {
            queue.borrow_mut().push(work);
        })));
        struct Restore {
            text: Option<TextEnvironment>,
            images: Option<Arc<ImageAssetRegistry>>,
            scale: f32,
        }
        impl Drop for Restore {
            fn drop(&mut self) {
                TextEnvironmentHolder::set_current(self.text.take());
                ImageAssetRegistryHolder::set_current(self.images.take());
                set_content_scale(self.scale);
                // `UIWorkSchedulerHolder` has no getter in the runtime crate; the
                // Android host owns exactly one scene, so clearing is equivalent
                // to restoring the previous (absent) enqueue.
                UIWorkSchedulerHolder::set_enqueue(None);
            }
        }
        let _restore = Restore {
            text: previous_text,
            images: previous_images,
            scale: previous_scale,
        };
        body()
    }

    pub fn drain(&self) {
        let pending: Vec<Rc<dyn Fn()>> = std::mem::take(&mut *self.work.borrow_mut());
        for work in pending {
            work();
        }
    }

    pub fn clear_images(&self) {
        self.images.clear();
    }
}

impl Default for SceneEnvironment {
    fn default() -> Self {
        Self::new()
    }
}
