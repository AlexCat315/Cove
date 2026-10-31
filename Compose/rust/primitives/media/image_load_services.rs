//! Translation of Primitives/Media/ImageLoadServices.swift.
//!
//! Boundary between node lifecycle, background decode and UI-thread
//! publication. Swift's `ImageLoadServices` closes over
//! `ImageAssetRegistry` and `UIWorkSchedulerHolder`; the Rust version keeps the
//! same seams so the widget layer never touches decode or scheduling details.
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use cove_core::scene::UIWorkSchedulerHolder;
use cove_imaging::{
    DecodedAnimation, DecodedImage, ImageAsset, ImageAssetRegistryHolder, ImageDecoder, PixelSize,
};

use super::image_data_download::{
    AsyncImageError, DownloadedImage, ImageDataDownload, ImageLoadingPolicy,
};

/// A decoded still image or every frame of an animation.
#[derive(Clone)]
pub enum ImagePayload {
    Still(DecodedImage),
    Animation(DecodedAnimation),
}

/// Receives low-resolution previews while a progressive image downloads.
pub type ImageProgress = Rc<dyn Fn(DecodedImage)>;

/// Frame ceiling for animated decodes, matching the reference decoder budget.
const MAXIMUM_FRAMES: usize = 512;

/// Everything the request needs to resolve a decode, independent of the view.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageLoadRequest {
    pub url: Option<String>,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub policy: ImageLoadingPolicy,
}

impl ImageLoadRequest {
    pub fn new(url: Option<impl Into<String>>, pixel_width: u32, pixel_height: u32) -> Self {
        Self {
            url: url.map(Into::into),
            pixel_width: pixel_width.clamp(1, 8192),
            pixel_height: pixel_height.clamp(1, 8192),
            policy: ImageLoadingPolicy::default(),
        }
    }

    pub fn with_policy(mut self, policy: ImageLoadingPolicy) -> Self {
        self.policy = policy;
        self
    }

    fn bounds(&self) -> PixelSize {
        PixelSize {
            width: self.pixel_width,
            height: self.pixel_height,
        }
    }

    /// Registry key for the aspect-preserving thumbnail this request decodes.
    pub fn cache_key(&self) -> Option<String> {
        ImageAssetRegistryHolder::current()?;
        self.url.as_ref().map(|url| {
            format!(
                "{url}#{}x{}#aspect-thumbnail",
                self.pixel_width, self.pixel_height
            )
        })
    }
}

/// Boundary between node lifecycle, background decode and UI-thread
/// publication.
pub struct ImageLoadServices {
    pub cached: Rc<dyn Fn(&str) -> Option<Arc<ImageAsset>>>,
    pub decode: Rc<
        dyn Fn(&ImageLoadRequest, Option<ImageProgress>) -> Result<ImagePayload, AsyncImageError>,
    >,
    pub register: Rc<dyn Fn(&str, DecodedImage) -> Result<Arc<ImageAsset>, String>>,
    pub enqueue: Rc<dyn Fn(Rc<dyn Fn()>)>,
}

impl ImageLoadServices {
    pub fn new(
        cached: Rc<dyn Fn(&str) -> Option<Arc<ImageAsset>>>,
        decode: Rc<
            dyn Fn(
                &ImageLoadRequest,
                Option<ImageProgress>,
            ) -> Result<ImagePayload, AsyncImageError>,
        >,
        register: Rc<dyn Fn(&str, DecodedImage) -> Result<Arc<ImageAsset>, String>>,
        enqueue: Rc<dyn Fn(Rc<dyn Fn()>)>,
    ) -> Self {
        Self {
            cached,
            decode,
            register,
            enqueue,
        }
    }

    /// Services backed by the installed image registry and UI work scheduler.
    pub fn current() -> Option<Rc<Self>> {
        ImageAssetRegistryHolder::current()?;
        Some(Rc::new(Self {
            cached: Rc::new(|key| {
                ImageAssetRegistryHolder::current().and_then(|registry| registry.cached(key))
            }),
            decode: Rc::new(|request, progress| decode_request(request, progress)),
            register: Rc::new(|key, decoded| {
                let registry = ImageAssetRegistryHolder::current()
                    .ok_or_else(|| AsyncImageError::HostUnavailable.to_string())?;
                registry
                    .register(key, decoded)
                    .map_err(|error| error.to_string())
            }),
            enqueue: Rc::new(|work| {
                let scheduler: Rc<dyn Fn(Rc<dyn Fn()>)> =
                    Rc::new(|next: Rc<dyn Fn()>| UIWorkSchedulerHolder::schedule(next));
                ImagePublicationQueue::shared().enqueue(work, scheduler);
            }),
        }))
    }
}

fn decode_request(
    request: &ImageLoadRequest,
    progress: Option<ImageProgress>,
) -> Result<ImagePayload, AsyncImageError> {
    let Some(url) = request.url.as_deref() else {
        return Err(AsyncImageError::UnsupportedUrl);
    };
    let bounds = request.bounds();
    let maximum_source_pixels = request.policy.maximum_source_pixels;
    let preview: Option<super::image_data_download::DownloadProgress> = progress.map(|progress| {
        Rc::new(move |partial: &[u8]| {
            if let Some(image) =
                ImageDecoder::decode_partial(partial, bounds, maximum_source_pixels)
            {
                progress(image);
            }
        }) as super::image_data_download::DownloadProgress
    });
    let download = ImageDataDownload::load(url, &request.policy, preview.as_ref())?;
    if let Some(animation) = decode_animation(&download, bounds, maximum_source_pixels)? {
        return Ok(ImagePayload::Animation(animation));
    }
    let still = ImageDecoder::decode_thumbnail(
        &download.data,
        &download.format_hint,
        bounds,
        maximum_source_pixels,
    )
    .map_err(|error| AsyncImageError::DecodeFailed(error.to_string()))?;
    Ok(ImagePayload::Still(still))
}

fn decode_animation(
    download: &DownloadedImage,
    bounds: PixelSize,
    maximum_source_pixels: u64,
) -> Result<Option<DecodedAnimation>, AsyncImageError> {
    ImageDecoder::decode_animation(
        &download.data,
        &download.format_hint,
        bounds,
        maximum_source_pixels,
        MAXIMUM_FRAMES,
    )
    .map_err(|error| AsyncImageError::DecodeFailed(error.to_string()))
}

/// Keep a folder of thumbnails from publishing all completed images in one
/// run-loop batch. Every queued publication retains its own host scheduler.
pub struct ImagePublicationQueue {
    jobs: RefCell<Vec<(Rc<dyn Fn()>, Rc<dyn Fn(Rc<dyn Fn()>)>)>>,
    head: RefCell<usize>,
    is_scheduled: RefCell<bool>,
}

thread_local! {
    static PUBLICATION_QUEUE: Rc<ImagePublicationQueue> = Rc::new(ImagePublicationQueue::new());
}

impl ImagePublicationQueue {
    fn new() -> Self {
        Self {
            jobs: RefCell::new(Vec::new()),
            head: RefCell::new(0),
            is_scheduled: RefCell::new(false),
        }
    }

    pub fn shared() -> Rc<Self> {
        PUBLICATION_QUEUE.with(Rc::clone)
    }

    pub fn enqueue(&self, work: Rc<dyn Fn()>, scheduler: Rc<dyn Fn(Rc<dyn Fn()>)>) {
        self.jobs.borrow_mut().push((work, scheduler.clone()));
        let needs_schedule = !*self.is_scheduled.borrow();
        *self.is_scheduled.borrow_mut() = true;
        if needs_schedule {
            let queue = ImagePublicationQueue::shared();
            scheduler(Rc::new(move || queue.drain()));
        }
    }

    fn drain(&self) {
        let job = {
            let head = *self.head.borrow();
            self.jobs.borrow().get(head).cloned()
        };
        let Some((work, _)) = job else { return };
        *self.head.borrow_mut() += 1;
        work();
        let next = {
            let head = *self.head.borrow();
            self.jobs.borrow().get(head).cloned()
        };
        match next {
            Some((_, scheduler)) => {
                let queue = ImagePublicationQueue::shared();
                scheduler(Rc::new(move || queue.drain()));
            }
            None => {
                self.jobs.borrow_mut().clear();
                *self.head.borrow_mut() = 0;
                *self.is_scheduled.borrow_mut() = false;
            }
        }
    }
}

struct InflightLoad {
    payload: Option<ImagePayload>,
    waiters: usize,
}

/// One download and decode per cache key, shared by every node that asks for
/// it while it is in flight. A finished load leaves the table, so later
/// requests hit the asset cache or start afresh.
pub struct ImageInflightLoads {
    loads: RefCell<HashMap<String, InflightLoad>>,
}

thread_local! {
    static INFLIGHT: Rc<ImageInflightLoads> = Rc::new(ImageInflightLoads::new());
}

impl ImageInflightLoads {
    fn new() -> Self {
        Self {
            loads: RefCell::new(HashMap::new()),
        }
    }

    pub fn shared() -> Rc<Self> {
        INFLIGHT.with(Rc::clone)
    }

    pub fn active_count(&self) -> usize {
        self.loads.borrow().len()
    }

    /// Joins the in-flight load for `key`, or starts it when none is running.
    pub fn load(
        &self,
        key: &str,
        start: impl FnOnce() -> Result<ImagePayload, AsyncImageError>,
    ) -> Result<ImagePayload, AsyncImageError> {
        if let Some(payload) = self.join(key) {
            return payload;
        }
        self.loads.borrow_mut().insert(
            key.to_string(),
            InflightLoad {
                payload: None,
                waiters: 1,
            },
        );
        let result = start();
        if let Ok(payload) = &result
            && let Some(load) = self.loads.borrow_mut().get_mut(key)
        {
            load.payload = Some(payload.clone());
        }
        self.leave(key);
        result
    }

    fn join(&self, key: &str) -> Option<Result<ImagePayload, AsyncImageError>> {
        let mut loads = self.loads.borrow_mut();
        let load = loads.get_mut(key)?;
        load.waiters += 1;
        Some(load.payload.clone().ok_or(AsyncImageError::InvalidResponse))
    }

    fn leave(&self, key: &str) {
        self.loads.borrow_mut().remove(key);
    }
}
