//! Translation of Primitives/Media/AsyncImage.swift.
//!
//! File or HTTP image with explicit loading/failure content. Decode happens off
//! the UI loop; registration and publication use the captured host scheduler.
//! The node owns cancellation, so an old request cannot replace a newer source.
use crate::Scope;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use crate::impl_primitive_view;
use crate::primitives::media::image_data_download::AsyncImageError;
use crate::primitives::media::image_geometry::ImageContentMode;
use crate::primitives::media::image_load_services::{
    ImageDecodeExecution, ImageInflightLoads, ImageLoadRequest, ImageLoadServices, ImagePayload,
    ImageProgress,
};
use crate::primitives::media::{Image, ImageSource};
use crate::{AnyView, PrimitiveView, View};
use cove_core::scene::{
    AnimatorScheduler, AnyAnimationController, InvalidationSource, LayoutNode, Node, NodeResource,
};
use cove_imaging::ImageAsset;

pub use crate::primitives::media::image_data_download::ImageLoadingPolicy;

#[derive(Clone)]
pub enum AsyncImagePhase {
    Empty,
    Loading,
    Success(Image),
    Failure(AsyncImageError),
}

/// Registered frames of one animated image. Identity equality: a new decode is
/// a new animation even when its pixels match.
pub struct AsyncImageAnimation {
    pub frames: Vec<std::sync::Arc<ImageAsset>>,
    pub durations: Vec<f64>,
    pub loop_count: i32,
}

impl AsyncImageAnimation {
    pub fn new(
        frames: Vec<std::sync::Arc<ImageAsset>>,
        durations: Vec<f64>,
        loop_count: i32,
    ) -> Self {
        Self {
            frames,
            durations,
            loop_count,
        }
    }

    pub fn total_duration(&self) -> f64 {
        self.durations.iter().sum()
    }

    /// Frame shown `elapsed` seconds into playback, holding the last frame once
    /// a finite loop count is exhausted.
    pub fn frame_at(&self, elapsed: f64) -> (usize, bool) {
        let cycle = self.total_duration();
        if self.frames.len() <= 1 || cycle <= 0.0 {
            return (0, true);
        }
        if self.loop_count > 0 && elapsed >= cycle * self.loop_count as f64 {
            return (self.frames.len() - 1, true);
        }
        let mut remaining = elapsed.rem_euclid(cycle);
        for (index, duration) in self.durations.iter().enumerate() {
            if remaining < *duration {
                return (index, false);
            }
            remaining -= duration;
        }
        (self.frames.len() - 1, false)
    }
}

#[derive(Clone)]
enum AsyncImageStatus {
    Empty,
    Loading,
    Success(std::sync::Arc<ImageAsset>),
    Animated(Rc<AsyncImageAnimation>),
    Failure(AsyncImageError),
}

impl AsyncImageStatus {
    fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }
}

/// Remembers decoded animations by cache key so remounting a row plays again
/// without a download. Small and least-recently used.
pub struct AsyncImageAnimationCache {
    entries: RefCell<HashMap<String, (Rc<AsyncImageAnimation>, u64)>>,
    clock: RefCell<u64>,
    capacity: usize,
}

thread_local! {
    static ANIMATION_CACHE: Rc<AsyncImageAnimationCache> =
        Rc::new(AsyncImageAnimationCache::new(64));
}

impl AsyncImageAnimationCache {
    fn new(capacity: usize) -> Self {
        Self {
            entries: RefCell::new(HashMap::new()),
            clock: RefCell::new(0),
            capacity,
        }
    }

    pub fn shared() -> Rc<Self> {
        ANIMATION_CACHE.with(Rc::clone)
    }

    pub fn value(&self, key: &str) -> Option<Rc<AsyncImageAnimation>> {
        let mut entries = self.entries.borrow_mut();
        let (animation, use_clock) = entries.get(key)?.clone();
        *self.clock.borrow_mut() += 1;
        let now = *self.clock.borrow();
        entries.insert(key.to_string(), (animation.clone(), now));
        let _ = use_clock;
        Some(animation)
    }

    pub fn store(&self, animation: Rc<AsyncImageAnimation>, key: &str) {
        *self.clock.borrow_mut() += 1;
        let now = *self.clock.borrow();
        let mut entries = self.entries.borrow_mut();
        entries.insert(key.to_string(), (animation, now));
        if entries.len() > self.capacity
            && let Some(oldest) = entries
                .iter()
                .min_by_key(|(_, (_, used))| *used)
                .map(|(key, _)| key.clone())
        {
            entries.remove(&oldest);
        }
    }
}

/// Observable request/phase state shared by the body and the node resource.
pub struct AsyncImageSession {
    request: RefCell<Option<ImageLoadRequest>>,
    phase: RefCell<AsyncImageStatus>,
    frame: RefCell<usize>,
    is_preview: RefCell<bool>,
    /// Readers subscribe; every publication (including one that arrives from
    /// a background decode) re-runs them.
    changed: cove_core::Signal<()>,
}

impl Drop for AsyncImageSession {
    fn drop(&mut self) {
        self.changed.dispose();
    }
}

impl AsyncImageSession {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            request: RefCell::new(None),
            phase: RefCell::new(AsyncImageStatus::Empty),
            frame: RefCell::new(0),
            is_preview: RefCell::new(false),
            changed: cove_core::Signal::detached(()),
        })
    }

    /// Current animation frame; playback writes it, the body reads it.
    pub fn frame_index(&self) -> usize {
        self.changed.track();
        *self.frame.borrow()
    }

    pub fn set_frame_index(&self, value: usize, node: Option<&Node>) {
        if *self.frame.borrow() == value {
            return;
        }
        *self.frame.borrow_mut() = value;
        if let Some(node) = node {
            node.mark_render_dirty(InvalidationSource::style_set("asyncImage.frame"));
        }
        self.changed.notify();
    }

    /// The animation published for the current request.
    pub fn animation(&self) -> Option<Rc<AsyncImageAnimation>> {
        self.changed.track();
        match &*self.phase.borrow() {
            AsyncImageStatus::Animated(animation) => Some(animation.clone()),
            _ => None,
        }
    }

    pub fn is_previewing(&self, request: &ImageLoadRequest) -> bool {
        self.changed.track();
        self.request.borrow().as_ref() == Some(request) && *self.is_preview.borrow()
    }

    /// A progressive preview: shown like a success, replaced by the final
    /// result.
    pub fn publish_preview(&self, asset: std::sync::Arc<ImageAsset>, request: &ImageLoadRequest) {
        *self.request.borrow_mut() = Some(request.clone());
        *self.phase.borrow_mut() = AsyncImageStatus::Success(asset);
        *self.is_preview.borrow_mut() = true;
        self.changed.notify();
    }

    fn status(&self, request: &ImageLoadRequest) -> AsyncImageStatus {
        self.changed.track();
        if self.request.borrow().as_ref() == Some(request) {
            self.phase.borrow().clone()
        } else if request.url.is_none() {
            AsyncImageStatus::Empty
        } else {
            AsyncImageStatus::Loading
        }
    }

    fn publish(&self, phase: AsyncImageStatus, request: &ImageLoadRequest, node: Option<&Node>) {
        let unchanged = self.request.borrow().as_ref() == Some(request)
            && status_eq(&self.phase.borrow(), &phase);
        if unchanged {
            return;
        }
        if matches!(phase, AsyncImageStatus::Animated(_)) {
            *self.frame.borrow_mut() = 0;
        }
        *self.request.borrow_mut() = Some(request.clone());
        *self.phase.borrow_mut() = phase;
        *self.is_preview.borrow_mut() = false;
        if let Some(node) = node {
            node.mark_render_dirty(InvalidationSource::style_set("asyncImage.phase"));
        }
        self.changed.notify();
    }
}

fn status_eq(lhs: &AsyncImageStatus, rhs: &AsyncImageStatus) -> bool {
    match (lhs, rhs) {
        (AsyncImageStatus::Empty, AsyncImageStatus::Empty) => true,
        (AsyncImageStatus::Loading, AsyncImageStatus::Loading) => true,
        (AsyncImageStatus::Success(left), AsyncImageStatus::Success(right)) => {
            std::sync::Arc::ptr_eq(left, right)
        }
        (AsyncImageStatus::Animated(left), AsyncImageStatus::Animated(right)) => {
            Rc::ptr_eq(left, right)
        }
        (AsyncImageStatus::Failure(left), AsyncImageStatus::Failure(right)) => left == right,
        _ => false,
    }
}

/// File or HTTP image with explicit loading/failure content.
pub struct AsyncImage {
    pub url: Option<String>,
    pub width: f32,
    pub height: f32,
    pub content_mode: ImageContentMode,
    pub retry_id: i32,
    pub policy: ImageLoadingPolicy,
    content: Rc<dyn Fn(AsyncImagePhase) -> AnyView>,
    session: Rc<AsyncImageSession>,
}

impl AsyncImage {
    pub fn new(
        url: Option<impl Into<String>>,
        width: f32,
        height: f32,
        content: impl Fn(AsyncImagePhase) -> AnyView + 'static,
    ) -> Self {
        Self {
            url: url.map(Into::into),
            width: if width.is_finite() {
                width.clamp(1.0, 4096.0)
            } else {
                32.0
            },
            height: if height.is_finite() {
                height.clamp(1.0, 4096.0)
            } else {
                32.0
            },
            content_mode: ImageContentMode::Fit,
            retry_id: 0,
            policy: ImageLoadingPolicy::default(),
            content: Rc::new(content),
            session: AsyncImageSession::new(),
        }
    }

    pub fn content_mode(mut self, content_mode: ImageContentMode) -> Self {
        self.content_mode = content_mode;
        self
    }

    pub fn retry_id(mut self, retry_id: i32) -> Self {
        self.retry_id = retry_id;
        self
    }

    pub fn policy(mut self, mut policy: ImageLoadingPolicy) -> Self {
        policy.validate();
        self.policy = policy;
        self
    }

    pub fn session(&self) -> Rc<AsyncImageSession> {
        self.session.clone()
    }

    fn request(&self) -> ImageLoadRequest {
        ImageLoadRequest {
            url: self.url.clone(),
            pixel_width: pixel_extent(self.width, self.content_mode == ImageContentMode::Fill),
            pixel_height: pixel_extent(self.height, self.content_mode == ImageContentMode::Fill),
            policy: self.policy.clone(),
            retry_id: self.retry_id,
        }
    }
}

fn pixel_extent(points: f32, is_fill: bool) -> u32 {
    let density = crate::primitives::media::image::ContentScaleHolder::current();
    let scale = if density.is_finite() {
        density.clamp(1.0, 8.0)
    } else {
        1.0
    };
    // Fill uses a larger aspect-preserving thumbnail before the central crop.
    let oversample = if is_fill { 2.0 } else { 1.0 };
    let value = (points * scale * oversample).round();
    (value.clamp(1.0, 8192.0)) as u32
}

impl View for AsyncImage {
    fn body(&self, cx: &Scope) -> AnyView {
        // The session outlives parent rebuilds; a rebuilt value's fresh session
        // would never receive the in-flight result.
        let session = cx.remember(|| self.session.clone());
        let request = self.request();
        let phase = match session.status(&request) {
            AsyncImageStatus::Empty => AsyncImagePhase::Empty,
            AsyncImageStatus::Loading => AsyncImagePhase::Loading,
            AsyncImageStatus::Success(asset) => AsyncImagePhase::Success(image_for_asset(
                asset,
                self.width,
                self.height,
                self.content_mode,
            )),
            AsyncImageStatus::Animated(animation) => {
                let index = animation
                    .frames
                    .len()
                    .saturating_sub(1)
                    .min(session.frame_index());
                AsyncImagePhase::Success(image_for_asset(
                    animation.frames[index].clone(),
                    self.width,
                    self.height,
                    self.content_mode,
                ))
            }
            AsyncImageStatus::Failure(error) => AsyncImagePhase::Failure(error),
        };
        let content = (self.content)(phase);
        AnyView::new(AsyncImageHost {
            session: session.clone(),
            request,
            content,
        })
    }
}

fn image_for_asset(
    asset: std::sync::Arc<ImageAsset>,
    width: f32,
    height: f32,
    content_mode: ImageContentMode,
) -> Image {
    Image::new(ImageSource::Asset(asset), cove_core::Size { width, height })
        .content_mode(content_mode)
}

struct AsyncImageHost {
    session: Rc<AsyncImageSession>,
    request: ImageLoadRequest,
    content: AnyView,
}

impl PrimitiveView for AsyncImageHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        let resource: Rc<dyn NodeResource> = Rc::new(AsyncImageResource::new());
        node.add_resource(resource);
        let playback: Rc<dyn NodeResource> = Rc::new(AsyncImagePlayback::new());
        node.add_resource(playback);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_width(Some(self.request_width()));
        layout.set_height(Some(self.request_height()));
        layout.set_align_items(cove_core::scene::Align::Center);
        layout.set_justify_content(cove_core::scene::Justify::Center);
        Some(layout)
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_width(Some(self.request_width()));
        layout.set_height(Some(self.request_height()));
    }

    fn update_node(&self, node: &Node) {
        let width = self.request_width();
        let height = self.request_height();
        if let Some(resource) = node.first_resource::<AsyncImageResource>() {
            resource.configure(&self.session, &self.request);
        }
        if let Some(playback) = node.first_resource::<AsyncImagePlayback>() {
            playback.configure(&self.session, node.prefers_reduced_motion());
        }
        let _ = (width, height);
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}

impl_primitive_view!(AsyncImageHost);

impl AsyncImageHost {
    fn request_width(&self) -> f32 {
        (self.request.pixel_width as f32).max(1.0)
    }

    fn request_height(&self) -> f32 {
        (self.request.pixel_height as f32).max(1.0)
    }
}

struct AsyncImageResource {
    node: RefCell<Option<Node>>,
    request: RefCell<Option<ImageLoadRequest>>,
    generation: RefCell<u64>,
}

impl AsyncImageResource {
    fn new() -> Self {
        Self {
            node: RefCell::new(None),
            request: RefCell::new(None),
            generation: RefCell::new(0),
        }
    }

    fn cancel(&self) {
        *self.generation.borrow_mut() += 1;
        *self.request.borrow_mut() = None;
    }

    fn configure(&self, session: &Rc<AsyncImageSession>, request: &ImageLoadRequest) {
        if self.request.borrow().as_ref() == Some(request) {
            return;
        }
        self.cancel();
        *self.request.borrow_mut() = Some(request.clone());
        let node = self.node.borrow().clone();
        let Some(key) = request.cache_key() else {
            session.publish(AsyncImageStatus::Empty, request, node.as_ref());
            return;
        };
        let Some(services) = ImageLoadServices::current() else {
            session.publish(
                AsyncImageStatus::Failure(AsyncImageError::HostUnavailable),
                request,
                node.as_ref(),
            );
            return;
        };
        if let Some(asset) = (services.cached)(&key) {
            session.publish(AsyncImageStatus::Success(asset), request, node.as_ref());
            return;
        }
        if let Some(animation) = AsyncImageAnimationCache::shared().value(&key) {
            session.publish(
                AsyncImageStatus::Animated(animation),
                request,
                node.as_ref(),
            );
            return;
        }
        session.publish(AsyncImageStatus::Loading, request, node.as_ref());

        // Previews of a progressive image show until the final decode lands.
        // The decode may run on a worker, so previews come back through the
        // UI thread's inbox.
        let (preview_sender, preview_receiver) = {
            let session = session.clone();
            let request = request.clone();
            let node = node.clone();
            cove_core::work::ui_channel(move |image: cove_imaging::DecodedImage| {
                let loading =
                    session.status(&request).is_loading() || session.is_previewing(&request);
                if !loading {
                    return;
                }
                if let Ok(asset) = ImageAsset::new(image) {
                    session.publish_preview(asset, &request);
                    if let Some(node) = node.as_ref() {
                        node.mark_render_dirty(InvalidationSource::style_set("asyncImage.phase"));
                    }
                }
            })
        };
        let progress: ImageProgress = Arc::new(move |image| preview_sender.send(image));

        let finish = {
            let session = session.clone();
            let request = request.clone();
            let services = services.clone();
            let key = key.clone();
            move |result: Result<ImagePayload, AsyncImageError>| {
                drop(preview_receiver);
                let Some(node) = node else { return };
                publish_result(&session, &request, &services, &key, &node, result);
            }
        };
        match services.execution {
            ImageDecodeExecution::Inline => {
                let result = ImageInflightLoads::shared().load(&key, || {
                    (services.decode)(request, Some(progress.clone()))
                });
                finish(result);
            }
            ImageDecodeExecution::Background => {
                let decode = services.decode.clone();
                let job_request = request.clone();
                ImageInflightLoads::shared().load_in_background(
                    &key,
                    move || decode(&job_request, Some(progress)),
                    finish,
                );
            }
        }
    }
}

/// Registers a finished decode and publishes it to the session.
fn publish_result(
    session: &Rc<AsyncImageSession>,
    request: &ImageLoadRequest,
    services: &ImageLoadServices,
    key: &str,
    node: &Node,
    result: Result<ImagePayload, AsyncImageError>,
) {
    match result {
        Ok(ImagePayload::Still(decoded)) => match ImageAsset::new(decoded) {
            Ok(asset) => {
                if let Ok(registered) = (services.register)(key, clone_asset_image(&asset)) {
                    session.publish(AsyncImageStatus::Success(registered), request, Some(node));
                } else {
                    session.publish(AsyncImageStatus::Success(asset), request, Some(node));
                }
            }
            Err(error) => session.publish(
                AsyncImageStatus::Failure(AsyncImageError::DecodeFailed(error.to_string())),
                request,
                Some(node),
            ),
        },
        Ok(ImagePayload::Animation(decoded)) => {
            let mut frames = Vec::with_capacity(decoded.frames.len());
            for (index, frame) in decoded.frames.into_iter().enumerate() {
                let frame_key = format!("{key}#frame-{index}");
                match (services.register)(&frame_key, frame) {
                    Ok(asset) => frames.push(asset),
                    Err(error) => {
                        session.publish(
                            AsyncImageStatus::Failure(AsyncImageError::DecodeFailed(error)),
                            request,
                            Some(node),
                        );
                        return;
                    }
                }
            }
            let animation = Rc::new(AsyncImageAnimation::new(
                frames,
                decoded.durations,
                decoded.loop_count as i32,
            ));
            AsyncImageAnimationCache::shared().store(animation.clone(), key);
            session.publish(AsyncImageStatus::Animated(animation), request, Some(node));
        }
        Err(error) => session.publish(AsyncImageStatus::Failure(error), request, Some(node)),
    }
}

fn clone_asset_image(asset: &std::sync::Arc<ImageAsset>) -> cove_imaging::DecodedImage {
    cove_core::Bitmap::new(
        asset.image.width(),
        asset.image.height(),
        asset.image.pixels().to_vec(),
    )
    .unwrap_or_else(|_| {
        cove_core::Bitmap::new(1, 1, vec![0, 0, 0, 0]).expect("1x1 bitmap is always valid")
    })
}

impl NodeResource for AsyncImageResource {
    fn mount(&self, node: &Node) {
        *self.node.borrow_mut() = Some(node.clone());
    }

    fn unmount(&self, _node: &Node) {
        self.cancel();
        *self.node.borrow_mut() = None;
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}

/// Plays an animated image on the shared animation scheduler. Reduced motion
/// shows the first frame; a finite loop count ends on the last frame.
pub struct AsyncImagePlayback {
    state: Rc<PlaybackState>,
    scheduler: RefCell<Option<AnimatorScheduler>>,
    controller: RefCell<Option<Rc<dyn AnyAnimationController>>>,
}

struct PlaybackState {
    session: RefCell<Option<Rc<AsyncImageSession>>>,
    animation: RefCell<Option<Rc<AsyncImageAnimation>>>,
    elapsed: RefCell<f64>,
    is_finished: RefCell<bool>,
    node: RefCell<Option<Node>>,
}

impl PlaybackState {
    fn cancel(&self) {
        *self.is_finished.borrow_mut() = true;
        *self.animation.borrow_mut() = None;
    }
}

impl AnyAnimationController for PlaybackState {
    fn is_finished(&self) -> bool {
        *self.is_finished.borrow()
    }

    fn tick(&self, delta_time: f64) {
        if *self.is_finished.borrow() {
            return;
        }
        let Some(animation) = self.animation.borrow().clone() else {
            return;
        };
        if !delta_time.is_finite() || delta_time < 0.0 {
            return;
        }
        *self.elapsed.borrow_mut() += delta_time;
        let (index, finished) = animation.frame_at(*self.elapsed.borrow());
        if let Some(session) = self.session.borrow().as_ref() {
            session.set_frame_index(index, self.node.borrow().as_ref());
        }
        if finished {
            *self.is_finished.borrow_mut() = true;
        }
    }

    fn finish_immediately(&self) {
        self.cancel();
    }

    fn cancel(&self) {
        *self.is_finished.borrow_mut() = true;
        *self.animation.borrow_mut() = None;
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl AsyncImagePlayback {
    fn new() -> Self {
        Self {
            state: Rc::new(PlaybackState {
                session: RefCell::new(None),
                animation: RefCell::new(None),
                elapsed: RefCell::new(0.0),
                is_finished: RefCell::new(true),
                node: RefCell::new(None),
            }),
            scheduler: RefCell::new(None),
            controller: RefCell::new(None),
        }
    }

    fn configure(&self, session: &Rc<AsyncImageSession>, reduced_motion: bool) {
        let next = if reduced_motion {
            None
        } else {
            session.animation()
        };
        let same = match (&*self.state.animation.borrow(), &next) {
            (Some(current), Some(next)) => Rc::ptr_eq(current, next),
            (None, None) => true,
            _ => false,
        };
        if same
            && self
                .state
                .session
                .borrow()
                .as_ref()
                .is_some_and(|current| Rc::ptr_eq(current, session))
        {
            return;
        }
        self.cancel();
        *self.state.session.borrow_mut() = Some(session.clone());
        *self.state.animation.borrow_mut() = next.clone();
        let Some(_animation) = next else { return };
        *self.state.elapsed.borrow_mut() = 0.0;
        *self.state.is_finished.borrow_mut() = false;
        let controller: Rc<dyn AnyAnimationController> = self.state.clone();
        *self.controller.borrow_mut() = Some(controller.clone());
        if let Some(scheduler) = self.scheduler.borrow().as_ref() {
            scheduler.register(controller);
        }
    }

    fn cancel(&self) {
        if let (Some(scheduler), Some(controller)) = (
            self.scheduler.borrow().as_ref(),
            self.controller.borrow().as_ref(),
        ) {
            scheduler.unregister(controller);
        }
        self.controller.borrow_mut().take();
        self.state.cancel();
    }
}

impl NodeResource for AsyncImagePlayback {
    fn mount(&self, node: &Node) {
        *self.scheduler.borrow_mut() = Some(AnimatorScheduler::current());
        *self.state.node.borrow_mut() = Some(node.clone());
    }

    fn unmount(&self, _node: &Node) {
        self.cancel();
        *self.scheduler.borrow_mut() = None;
        *self.state.node.borrow_mut() = None;
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}
