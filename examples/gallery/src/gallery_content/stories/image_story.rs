//! Layout story: image content modes and asynchronous loading.
//!
//! Swift source: `GalleryContent/Stories/Layout/ImageStory.swift`.
//!
//! The loading surface has three states and the source switch has four options.
//! Both are ordinary state machines, so they live here as data the tests can
//! assert against rather than only as view branches.

use cove_composition::Scope;
use cove_core::State;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::media::{AsyncImage, AsyncImagePhase, ImageContentMode};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::SemanticColorRef;
use cove_ui_compose::{AnyView, TupleView, View, ViewExt};

use super::feedback::StorySection;
use crate::gallery_content::catalog::GalleryOptions;

/// The default remote URL the story pre-fills.
pub const DEFAULT_REMOTE_URL: &str = "https://avatars.githubusercontent.com/u/99124972?v=4";
/// The path that intentionally does not exist, for the missing state.
pub const MISSING_PATH: &str = "/cove-gallery-missing.png";
/// The preview size, in points.
pub const PREVIEW_WIDTH: f32 = 200.0;
pub const PREVIEW_HEIGHT: f32 = 140.0;
/// Below this width the preview and its controls stack vertically.
pub const STACKING_BREAKPOINT: f32 = 380.0;

/// The sources the switch offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageSourceKind {
    Bundled,
    Remote,
    Missing,
    Empty,
}

impl ImageSourceKind {
    pub const ALL: [Self; 4] = [Self::Bundled, Self::Remote, Self::Missing, Self::Empty];

    pub fn id(self) -> &'static str {
        match self {
            Self::Bundled => "bundled",
            Self::Remote => "remote",
            Self::Missing => "missing",
            Self::Empty => "empty",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Bundled => "Bundled",
            Self::Remote => "Remote",
            Self::Missing => "Missing",
            Self::Empty => "Empty",
        }
    }

    pub fn from_id(id: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|kind| kind.id() == id)
            .unwrap_or(Self::Bundled)
    }

    /// Whether a retry is meaningful: only a concrete URL can be re-requested.
    pub fn can_retry(self) -> bool {
        !matches!(self, Self::Empty)
    }
}

/// What the placeholder surface shows for a loading phase.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageSurface {
    /// A spinner with a "Loading…" caption.
    Loading,
    /// A failure with its description as the tooltip and help text.
    Unavailable(String),
    /// No source chosen.
    Empty,
    /// The decoded image.
    Loaded,
}

/// Maps an async-image phase to the surface the story draws.
///
/// `Empty` and `Failure` share a layout but not their copy: a failure explains
/// itself through the error description, an empty slot prompts for a source.
pub fn surface_for(
    phase: &AsyncImagePhase,
    error_description: impl FnOnce() -> String,
) -> ImageSurface {
    match phase {
        AsyncImagePhase::Success(_) => ImageSurface::Loaded,
        AsyncImagePhase::Loading => ImageSurface::Loading,
        AsyncImagePhase::Failure(_) => ImageSurface::Unavailable(error_description()),
        AsyncImagePhase::Empty => ImageSurface::Empty,
    }
}

/// The primary caption for a surface.
pub fn surface_caption(surface: &ImageSurface) -> &'static str {
    match surface {
        ImageSurface::Loading => "Loading…",
        ImageSurface::Unavailable(_) => "Image unavailable",
        ImageSurface::Empty => "No image",
        ImageSurface::Loaded => "Loaded image",
    }
}

/// The secondary text: the failure description, or the empty prompt.
pub fn surface_detail(surface: &ImageSurface) -> String {
    match surface {
        ImageSurface::Unavailable(detail) => detail.clone(),
        ImageSurface::Empty => "Choose an image source".to_owned(),
        ImageSurface::Loading => String::new(),
        ImageSurface::Loaded => String::new(),
    }
}

/// `ImageStory`: three content modes plus an async-loading preview.
pub struct ImageStory {
    pub options: GalleryOptions,
    source: State<String>,
    retry: State<u32>,
    remote_url: State<String>,
}

impl ImageStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            source: State::new(ImageSourceKind::Bundled.id().to_owned()),
            retry: State::new(0),
            remote_url: State::new(DEFAULT_REMOTE_URL.to_owned()),
        }
    }

    pub fn source(&self) -> ImageSourceKind {
        ImageSourceKind::from_id(&self.source.get())
    }

    pub fn set_source(&self, source: ImageSourceKind) {
        self.source.set(source.id().to_owned());
    }

    pub fn retry_count(&self) -> u32 {
        self.retry.get()
    }

    /// Starts a new request. Only meaningful when there is a URL to request.
    pub fn retry(&self) -> bool {
        if !self.source().can_retry() {
            return false;
        }
        self.retry.set(self.retry.get().wrapping_add(1));
        true
    }

    /// The one-based request number the story shows.
    pub fn request_number(&self) -> u32 {
        self.retry.get() + 1
    }

    pub fn remote_url(&self) -> String {
        self.remote_url.get()
    }

    pub fn set_remote_url(&self, url: impl Into<String>) {
        self.remote_url.set(url.into());
    }

    /// The URL the current source resolves to, or `None` for the empty source.
    pub fn resolved_url(&self) -> Option<String> {
        match self.source() {
            ImageSourceKind::Bundled => Some("gallery-landscape.svg".to_owned()),
            ImageSourceKind::Remote => Some(self.remote_url.get().trim().to_owned()),
            ImageSourceKind::Missing => Some(MISSING_PATH.to_owned()),
            ImageSourceKind::Empty => None,
        }
    }

    /// Whether the preview and its controls stack instead of sitting side by side.
    pub fn stacks_vertically(&self, container_width: f32) -> bool {
        container_width < STACKING_BREAKPOINT
    }

    pub fn content_modes(&self) -> [ImageContentMode; 3] {
        [
            ImageContentMode::Fit,
            ImageContentMode::Fill,
            ImageContentMode::Stretch,
        ]
    }
}

impl View for ImageStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let caption = Text::new(format!("Source: {}", self.source().title()))
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::body_strong());
        let request = Text::new(format!("Request: {}", self.request_number()))
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted());

        let retry_state = self.retry.clone();
        let can_retry = self.source().can_retry();
        let retry = Button::title("Retry", move || {
            retry_state.update(|value| *value = value.wrapping_add(1))
        })
        .enabled(self.options.is_enabled && can_retry);

        let url = self.resolved_url().filter(|value| !value.is_empty());
        let preview = AsyncImage::new(
            url,
            PREVIEW_WIDTH,
            PREVIEW_HEIGHT,
            |phase: AsyncImagePhase| {
                let surface = surface_for(&phase, || "The request failed".to_owned());
                AnyView::new(
                    Text::new(surface_caption(&surface))
                        .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
                        .semantic_foreground(SemanticColorRef::on_surface_muted()),
                )
            },
        )
        .content_mode(ImageContentMode::Fill)
        .retry_id(self.retry.get() as i32);

        AnyView::new(StorySection::new(
            "Asynchronous loading",
            "Switch between a bundled SVG, a remote photo, a missing file and no source. Retry starts a new request after failure.",
            AnyView::new(TupleView::new(vec![
                AnyView::new(AnyView::new(preview).frame_height(PREVIEW_HEIGHT)),
                AnyView::new(caption),
                AnyView::new(request),
                AnyView::new(retry),
            ])),
        ))
    }

    fn tag(&self) -> &'static str {
        "ImageStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; source, retry, remote_url)
    }
}
