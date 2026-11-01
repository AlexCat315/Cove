//! Data story: a native web surface inside the gallery.
//!
//! Swift source: `GalleryContent/Stories/Data/WebStory.swift`.
//!
//! The composed side owns the frame, clipping and navigation controls; the
//! platform host owns the page. The status line is the only logic the story adds
//! on top of `WebViewState`, so its precedence is what the tests pin:
//! a failure reports its description, a load in flight reports the target title,
//! and an idle surface reads `Ready`.
//!
//! The Swift file is `#if os(macOS) && canImport(WebKit)` with an alert fallback
//! elsewhere. The Rust implementation has no such gate: `crates/addons/src/webview`
//! provides Apple, Windows, Linux and Android hosts, so every platform renders a
//! real page rather than a "not supported" message.

use cove_core::State;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::SemanticColorRef;
use cove_ui_compose::{AnyView, BoxView, TupleView, View};

use super::feedback::StorySection;
use crate::gallery_content::catalog::GalleryOptions;
use cove_ui_addons::webview::{WebViewCommand, WebViewPhase, WebViewRequest, WebViewState};

/// The height the story gives the web surface.
pub const WEB_SURFACE_HEIGHT: f32 = 460.0;

/// The three samples the selector offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WebSample {
    Local,
    Example,
    Repository,
}

impl WebSample {
    pub const ALL: [Self; 3] = [Self::Local, Self::Example, Self::Repository];

    pub fn title(self) -> &'static str {
        match self {
            Self::Local => "Offline sample",
            Self::Example => "example.com",
            Self::Repository => "Diffuse on GitHub",
        }
    }

    pub fn url(self) -> Option<&'static str> {
        match self {
            Self::Local => None,
            Self::Example => Some("https://example.com"),
            Self::Repository => Some("https://github.com/AlexCat315/Diffuse"),
        }
    }

    /// The request this sample loads.
    ///
    /// The local sample carries its own HTML so the story works with no network;
    /// the other two are ordinary URLs.
    pub fn request(self) -> WebViewRequest {
        match self.url() {
            Some(url) => WebViewRequest::url(url),
            None => WebViewRequest::html(LOCAL_PAGE),
        }
    }
}

/// The status line for a web state.
///
/// A failure shows its description when the host supplied one, otherwise a
/// generic failure. A load in flight names its target when the title is already
/// known, so the line does not sit on a bare "Loading…" once a page starts
/// reporting its title.
pub fn status_text(state: &WebViewState) -> String {
    if let Some(error) = &state.error_description
        && state.has_failed()
    {
        return format!("Failed: {error}");
    }
    match state.phase {
        WebViewPhase::Idle => "Ready".to_owned(),
        WebViewPhase::Loading => {
            let title = state.display_title();
            if title.is_empty() {
                "Loading…".to_owned()
            } else {
                format!("Loading {title}")
            }
        }
        WebViewPhase::Loaded => {
            let title = state.display_title();
            if title.is_empty() {
                "Loaded".to_owned()
            } else {
                title
            }
        }
        WebViewPhase::Failed => "Failed to load".to_owned(),
    }
}

/// Whether a command may be issued in this state.
pub fn command_is_enabled(command: WebViewCommand, state: &WebViewState) -> bool {
    match command {
        WebViewCommand::Back => state.can_go_back,
        WebViewCommand::Forward => state.can_go_forward,
        WebViewCommand::Reload => true,
        WebViewCommand::Stop => state.is_loading(),
    }
}

/// `WebStory`: the sample selector, navigation controls and the native surface.
pub struct WebStory {
    pub options: GalleryOptions,
    sample: State<usize>,
    state: State<WebViewState>,
    request_id: State<u64>,
}

impl WebStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            sample: State::new(0),
            state: State::new(WebViewState::default()),
            request_id: State::new(0),
        }
    }

    pub fn sample(&self) -> WebSample {
        WebSample::ALL[self.sample.get().min(WebSample::ALL.len() - 1)]
    }

    /// Switches samples and issues a fresh request.
    pub fn set_sample(&self, sample: WebSample) {
        if let Some(index) = WebSample::ALL.iter().position(|value| *value == sample) {
            self.sample.set(index);
            self.request_id.update(|value| *value += 1);
        }
    }

    pub fn request(&self) -> WebViewRequest {
        self.sample().request()
    }

    pub fn state(&self) -> WebViewState {
        self.state.get()
    }

    /// The host publishes a new snapshot; an identical one is dropped.
    pub fn publish(&self, next: WebViewState) -> bool {
        if next == self.state.get() {
            return false;
        }
        self.state.set(next);
        true
    }

    pub fn status(&self) -> String {
        status_text(&self.state.get())
    }

    pub fn can_go_back(&self) -> bool {
        self.state.get().can_go_back
    }

    pub fn can_go_forward(&self) -> bool {
        self.state.get().can_go_forward
    }

    pub fn is_loading(&self) -> bool {
        self.state.get().is_loading()
    }
}

impl View for WebStory {
    fn body(&self) -> AnyView {
        let status = Text::new(self.status())
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted());

        let note = Text::new(
            "Native views cannot be blended by the renderer: a menu or sheet opened over this rectangle is covered by the page. Rounded ancestor corners, opacity and transforms are not applied.",
        )
        .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
        .semantic_foreground(SemanticColorRef::on_surface_muted());

        AnyView::new(StorySection::new(
            "Native web content",
            "The platform host is attached above the composed surface. Back, forward, reload and stop operate the real browser; the frame follows layout and is clipped by the gallery scroll view.",
            AnyView::new(BoxView::new(
                cove_core::scene::FlexDirection::Column,
                cove_core::scene::Align::Stretch,
                cove_core::scene::Justify::FlexStart,
                cove_core::scene::Wrap::NoWrap,
                10.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(AnyView::new(status).frame_height(20.0)),
                    AnyView::new(note),
                ])),
            )),
        ))
    }

    fn tag(&self) -> &'static str {
        "WebStory"
    }
}

/// The offline document the local sample loads.
///
/// Kept byte-identical to the Swift string so the same page renders on every
/// platform; the host only needs `loadHTMLString` semantics.
pub const LOCAL_PAGE: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>CoveUI WebView</title>
  <style>
    :root { color-scheme: light dark; }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      font: 15px/1.55 -apple-system, BlinkMacSystemFont, "SF Pro Text", sans-serif;
      background: #f6f4f1;
      color: #24211d;
    }
    @media (prefers-color-scheme: dark) {
      body { background: #1d1b19; color: #f2eee7; }
      .card { background: #2a2724 !important; border-color: #3b3732 !important; }
      .muted { color: #b8b0a5 !important; }
      .accent { color: #9fd6a4 !important; }
    }
    main { max-width: 680px; margin: 0 auto; padding: 32px 28px 36px; }
    .eyebrow { font-size: 12px; letter-spacing: .08em; text-transform: uppercase; color: #8b8174; }
    h1 { font-size: 30px; line-height: 1.15; margin: 8px 0 10px; letter-spacing: -.02em; }
    p { margin: 0 0 16px; }
    .muted { color: #6f675c; }
    .card {
      display: flex; align-items: center; justify-content: space-between; gap: 16px;
      padding: 16px 18px; margin: 18px 0; background: #fffdf9;
      border: 1px solid #e4ddd2; border-radius: 14px;
    }
    .accent { color: #2f6b38; font-weight: 600; }
    button {
      font: inherit; font-weight: 600; color: #173d1d; background: #b9e4bd;
      border: 0; border-radius: 9px; padding: 8px 14px; cursor: pointer;
    }
    button:active { transform: translateY(1px); }
    a { color: inherit; }
    footer { margin-top: 24px; font-size: 12px; color: #8b8174; }
  </style>
</head>
<body>
  <main>
    <div class="eyebrow">CoveUI · WebView</div>
    <h1>Real HTML, inside a composed window.</h1>
    <p class="muted">
      This page is rendered by the platform web engine and clipped to the frame measured by CoveUI.
      Scroll the gallery and the native view follows the node.
    </p>
    <div class="card">
      <div>
        <div class="accent">Local document</div>
        <div class="muted" style="font-size:13px">No network request; loaded from a string.</div>
      </div>
      <button id="count" type="button">Tap me</button>
    </div>
    <p>
      Use the sample selector above to load a remote page. Back and Forward
      follow the browser's history between those top-level loads.
    </p>
    <footer>The web surface is a native subview; it is deliberately not part of CoveUI's DrawList.</footer>
  </main>
  <script>
    let taps = 0;
    const button = document.getElementById('count');
    button.addEventListener('click', () => {
      taps += 1;
      button.textContent = taps === 1 ? '1 tap' : `${taps} taps`;
    });
  </script>
</body>
</html>
"#;
