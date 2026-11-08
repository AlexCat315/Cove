//! Video and audio playback from local files and network URLs.
//!
//! Sources come from the native open panel, a drop onto the player, or an
//! `http(s)://` URL. Danmaku can be typed under the video or loaded from a
//! Bilibili XML file; one saved next to a video with the same name loads
//! automatically. Playback needs the `media-ffmpeg` feature.

use cove_composition::Scope;
#[cfg(not(feature = "media-ffmpeg"))]
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
#[cfg(not(feature = "media-ffmpeg"))]
use cove_ui_compose::primitives::text::Text;
#[cfg(not(feature = "media-ffmpeg"))]
use cove_ui_compose::theme::SemanticFontRef;
#[cfg(not(feature = "media-ffmpeg"))]
use cove_ui_compose::theme::tokens::SemanticColorRef;
use cove_ui_compose::{AnyView, View};

use crate::gallery_content::catalog::GalleryOptions;

/// Gallery page for `VideoPlayer`, `AudioPlayer`, danmaku and `Waveform`.
pub struct MediaPlayerStory {
    pub options: GalleryOptions,
}

impl MediaPlayerStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self { options }
    }
}

impl View for MediaPlayerStory {
    #[cfg(feature = "media-ffmpeg")]
    fn body(&self, cx: &Scope) -> AnyView {
        backend::story(cx, &self.options)
    }

    #[cfg(not(feature = "media-ffmpeg"))]
    fn body(&self, _cx: &Scope) -> AnyView {
        let _ = &self.options;
        AnyView::new(
            Text::new(
                "Playback needs FFmpeg: cargo run -p cove-examples --bin gallery --features media-ffmpeg",
            )
            .semantic_font(SemanticFontRef::body())
            .semantic_foreground(SemanticColorRef::on_surface_muted()),
        )
    }
}

#[cfg(feature = "media-ffmpeg")]
mod backend {
    use std::path::Path;
    use std::rc::Rc;
    use std::sync::Arc;

    use cove_composition::Scope;
    use cove_core::scene::{Align, FilePicker, FilePickerFilter, FlexDirection, Justify, Wrap};
    use cove_core::{Binding, Color, Signal};
    use cove_media::{
        AudioFormat, AudioOutput, AudioPeak, ContextOptions, DecoderOptions, MediaContext,
        MediaDecoder, MediaSource, MediaTime, PlaybackControl, Player, PlayerOptions,
        SeekRequest,
    };
    use cove_ui_compose::modifiers::interaction::AccessibilityExt;
    use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt};
    use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
    use cove_ui_compose::primitives::controls::Button;
    use cove_ui_compose::primitives::interaction::{DroppedFile, FileDropZone};
    use cove_ui_compose::primitives::media::{
        AudioPlayer, DanmakuFeed, VideoPlayer, Waveform, send_danmaku, use_playback,
    };
    use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
    use cove_ui_compose::primitives::text::{Text, TextField, TextFieldEvents};
    use cove_ui_compose::theme::SemanticFontRef;
    use cove_ui_compose::theme::tokens::SemanticColorRef;
    use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView};

    use super::super::feedback::StorySection;
    use crate::gallery_content::catalog::GalleryOptions;

    const VIDEO_EXTENSIONS: [&str; 9] = [
        "mp4", "m4v", "mov", "mkv", "webm", "avi", "flv", "ts", "wmv",
    ];
    const AUDIO_EXTENSIONS: [&str; 9] = [
        "mp3", "m4a", "aac", "flac", "wav", "ogg", "opus", "wma", "aiff",
    ];
    const DANMAKU_COLORS: [(u8, u8, u8); 6] = [
        (255, 255, 255),
        (254, 3, 2),
        (255, 170, 2),
        (255, 237, 79),
        (0, 205, 0),
        (2, 145, 255),
    ];

    struct Media {
        _context: MediaContext,
        video: Player,
        audio: Player,
    }

    impl Media {
        fn new() -> Option<Rc<Self>> {
            let context = MediaContext::new(ContextOptions::default()).ok()?;
            let player = |audio| {
                context.create_player(PlayerOptions {
                    autoplay: true,
                    audio,
                })
            };
            Some(Rc::new(Self {
                video: player(AudioOutput::DefaultDevice).ok()?,
                audio: player(AudioOutput::DefaultDevice).ok()?,
                _context: context,
            }))
        }
    }

    /// A display name for a path or URL.
    pub(super) fn source_name(location: &str) -> String {
        let trimmed = location.split(['?', '#']).next().unwrap_or(location);
        trimmed
            .rsplit(['/', '\\'])
            .find(|part| !part.is_empty())
            .unwrap_or(location)
            .to_owned()
    }

    pub(super) fn source(location: &str) -> Option<MediaSource> {
        let location = location.trim();
        if location.is_empty() {
            return None;
        }
        if location.contains("://") {
            MediaSource::uri(location).ok()
        } else {
            Some(MediaSource::file(location))
        }
    }

    fn open(player: &Player, location: &str) {
        if let Some(source) = source(location)
            && let Ok(request) = player.open(source)
        {
            let _ = player.detach(request);
        }
    }

    fn pick(extensions: &[&str], done: impl FnOnce(String) + 'static) {
        FilePicker::request(
            &[FilePickerFilter::new(
                "Media",
                extensions.iter().map(|extension| format!(".{extension}")),
            )],
            false,
            Box::new(move |paths| {
                if let Some(path) = paths.into_iter().next() {
                    done(path);
                }
            }),
        );
    }

    /// Loads Bilibili XML danmaku from `path` into `feed`.
    fn load_danmaku(feed: &DanmakuFeed, path: &Path) -> usize {
        let Ok(xml) = std::fs::read_to_string(path) else {
            return 0;
        };
        let comments = DanmakuFeed::parse_bilibili_xml(&xml);
        let count = comments.len();
        feed.clear();
        feed.extend(comments);
        count
    }

    fn caption(text: impl Into<String>) -> AnyView {
        AnyView::new(
            Text::new(text.into())
                .semantic_font(SemanticFontRef::caption())
                .semantic_foreground(SemanticColorRef::on_surface_muted()),
        )
    }

    fn stack(direction: FlexDirection, align: Align, spacing: f32, items: Vec<AnyView>) -> AnyView {
        AnyView::new(BoxView::new(
            direction,
            align,
            Justify::FlexStart,
            Wrap::NoWrap,
            spacing,
            AnyView::new(TupleView::new(items)),
        ))
    }

    fn text_binding(signal: Signal<TextBuffer>) -> Binding<TextBuffer> {
        Binding::new(move || signal.get(), move |value| signal.set(value))
    }

    /// File button, URL field and open button for one player.
    fn source_bar(
        extensions: &'static [&'static str],
        url: Signal<TextBuffer>,
        enabled: bool,
        open: Rc<dyn Fn(String)>,
        debug: &'static str,
    ) -> AnyView {
        let from_file = open.clone();
        let from_url = open.clone();
        let submit = open;
        let browse = Button::title("Open file…", move || {
            let open = from_file.clone();
            pick(extensions, move |path| open(path));
        })
        .enabled(enabled)
        .debug_name(&format!("{debug}-open-file"));
        let field = TextField::new("https://example.com/media.mp4", text_binding(url))
            .events(TextFieldEvents {
                on_submit: Some(Rc::new(move || submit(url.get_untracked().string_value()))),
                ..TextFieldEvents::default()
            })
            .debug_name(&format!("{debug}-url"));
        let go = Button::title("Open URL", move || from_url(url.get_untracked().string_value()))
            .enabled(enabled)
            .debug_name(&format!("{debug}-open-url"));
        stack(
            FlexDirection::Row,
            Align::Center,
            8.0,
            vec![
                AnyView::new(browse),
                AnyView::new(field.flex(1.0)),
                AnyView::new(go),
            ],
        )
    }

    pub fn story(cx: &Scope, options: &GalleryOptions) -> AnyView {
        let Some(media) = cx.remember(Media::new) else {
            return caption("No media backend is available.");
        };
        let enabled = options.is_enabled;
        let video_url = cx.state(TextBuffer::empty);
        let audio_url = cx.state(TextBuffer::empty);
        let comment = cx.state(TextBuffer::empty);
        let color = cx.state(|| 0usize);
        let video_title = cx.state(|| None::<String>);
        let audio_title = cx.state(|| None::<(String, String)>);
        let peaks = cx.state(|| Arc::<[AudioPeak]>::from(Vec::new()));
        let notice = cx.state(|| None::<String>);
        let feed = cx.remember(DanmakuFeed::new);

        let video_control: Arc<dyn PlaybackControl> = Arc::new(media.video.clone());
        let audio_control: Arc<dyn PlaybackControl> = Arc::new(media.audio.clone());
        let video_snapshot = use_playback(cx, &video_control);
        let audio_snapshot = use_playback(cx, &audio_control);

        let open_video: Rc<dyn Fn(String)> = {
            let media = media.clone();
            let feed = feed.clone();
            Rc::new(move |location: String| {
                let location = location.trim().to_owned();
                if location.is_empty() {
                    return;
                }
                open(&media.video, &location);
                video_title.set(Some(source_name(&location)));
                feed.clear();
                let loaded = if location.contains("://") {
                    0
                } else {
                    load_danmaku(&feed, &Path::new(&location).with_extension("xml"))
                };
                notice.set((loaded > 0).then(|| format!("Loaded {loaded} danmaku")));
            })
        };
        let open_audio: Rc<dyn Fn(String)> = {
            let media = media.clone();
            Rc::new(move |location: String| {
                let location = location.trim().to_owned();
                let Some(decode_source) = source(&location) else {
                    return;
                };
                open(&media.audio, &location);
                let name = source_name(&location);
                audio_title.set(Some((name.clone(), String::new())));
                peaks.set(Arc::from(Vec::new()));
                // Waveform and tags are read on a worker; the result is posted back.
                let peaks_sender = peaks.sender();
                let title_sender = audio_title.sender();
                std::thread::spawn(move || {
                    let Ok(mut decoder) = MediaDecoder::open(
                        decode_source,
                        DecoderOptions {
                            audio_format: AudioFormat::new(8_000, 1).ok(),
                            ..DecoderOptions::audio_only()
                        },
                    ) else {
                        return;
                    };
                    let info = decoder.info().clone();
                    let title = info.title().map_or_else(|| name.clone(), str::to_owned);
                    let artist = info.artist().unwrap_or_default().to_owned();
                    title_sender.set(Some((title, artist)));
                    if let Ok(Some(buffer)) = decoder.read_audio(None) {
                        peaks_sender.set(Arc::from(buffer.peaks(600)));
                    }
                });
            })
        };

        // Video.
        let drop_video = open_video.clone();
        let mut player = VideoPlayer::new(video_control.clone()).danmaku(feed.clone());
        if let Some(title) = video_title.get() {
            player = player.title(title);
        }
        let player = FileDropZone::new(
            move |files: Vec<DroppedFile>| {
                if let Some(file) = files.into_iter().next() {
                    drop_video(file.path);
                }
            },
            player.debug_name("media-video-player"),
        )
        .accepted_extensions(VIDEO_EXTENSIONS.iter().map(|extension| extension.to_string()))
        .maximum_files(1);

        let send_feed = feed.clone();
        let send_control = video_control.clone();
        let send = Rc::new(move || {
            let (r, g, b) = DANMAKU_COLORS[color.get_untracked() % DANMAKU_COLORS.len()];
            if send_danmaku(
                &send_feed,
                send_control.as_ref(),
                &comment.get_untracked().string_value(),
                Color::from_rgba8(r, g, b, 255),
            )
            .is_some()
            {
                comment.set(TextBuffer::empty());
            }
        });
        let submit = send.clone();
        let (r, g, b) = DANMAKU_COLORS[color.get() % DANMAKU_COLORS.len()];
        let swatch = Button::new(
            Text::new("●").color(Color::from_rgba8(r, g, b, 255)),
            move || color.update(|index| *index += 1),
        )
        .accessibility_label("Danmaku colour")
        .debug_name("media-danmaku-colour");
        let load_feed = feed.clone();
        let load = Button::title("Load XML…", move || {
            let feed = load_feed.clone();
            FilePicker::request(
                &[FilePickerFilter::new("Danmaku", [".xml".to_owned()])],
                false,
                Box::new(move |paths| {
                    if let Some(path) = paths.first() {
                        let count = load_danmaku(&feed, Path::new(path));
                        notice.set(Some(format!("Loaded {count} danmaku")));
                    }
                }),
            );
        })
        .enabled(enabled);
        let danmaku_bar = stack(
            FlexDirection::Row,
            Align::Center,
            8.0,
            vec![
                AnyView::new(swatch),
                AnyView::new(
                    TextField::new("Send a danmaku at the current time", text_binding(comment))
                        .events(TextFieldEvents {
                            on_submit: Some(Rc::new(move || submit())),
                            ..TextFieldEvents::default()
                        })
                        .debug_name("media-danmaku-input")
                        .flex(1.0),
                ),
                AnyView::new(
                    Button::title("Send", move || send())
                        .enabled(enabled && video_snapshot.duration.is_some())
                        .debug_name("media-danmaku-send"),
                ),
                AnyView::new(load),
            ],
        );
        let mut video_items = vec![
            source_bar(&VIDEO_EXTENSIONS, video_url, enabled, open_video, "media-video"),
            AnyView::new(player),
            danmaku_bar,
        ];
        let status = video_snapshot
            .last_error
            .as_ref()
            .map(|error| error.message.clone())
            .or_else(|| notice.get())
            .or_else(|| {
                video_title
                    .get()
                    .is_none()
                    .then(|| "Open a file, drop one on the player, or enter a URL.".to_owned())
            });
        if let Some(status) = status {
            video_items.push(caption(status));
        }
        let video = StorySection::new(
            "VideoPlayer",
            "",
            AnyView::new(
                stack(FlexDirection::Column, Align::Stretch, 10.0, video_items).frame_width(720.0),
            ),
        );

        // Audio.
        let (title, subtitle) = audio_title
            .get()
            .unwrap_or_else(|| ("No audio".to_owned(), String::new()));
        let duration = audio_snapshot.duration.map_or(0.0, MediaTime::seconds);
        let progress = if duration > 0.0 {
            audio_snapshot.position.unwrap_or_default().seconds() / duration
        } else {
            0.0
        };
        let seek_control = audio_control.clone();
        let waveform = Waveform::new(peaks.get())
            .progress(progress)
            .height(56.0)
            .on_seek(move |fraction| {
                if let Ok(time) = MediaTime::from_seconds(fraction * duration) {
                    let _ = seek_control.seek(SeekRequest::exact(time));
                }
            })
            .debug_name("media-waveform");
        let drop_audio = open_audio.clone();
        let audio_card = FileDropZone::new(
            move |files: Vec<DroppedFile>| {
                if let Some(file) = files.into_iter().next() {
                    drop_audio(file.path);
                }
            },
            AudioPlayer::new(audio_control)
                .title(title)
                .subtitle(subtitle)
                .debug_name("media-audio-player"),
        )
        .accepted_extensions(AUDIO_EXTENSIONS.iter().map(|extension| extension.to_string()))
        .maximum_files(1);
        let mut audio_items = vec![
            source_bar(&AUDIO_EXTENSIONS, audio_url, enabled, open_audio, "media-audio"),
            AnyView::new(audio_card),
            AnyView::new(waveform),
        ];
        if let Some(error) = audio_snapshot.last_error.as_ref() {
            audio_items.push(caption(error.message.clone()));
        }
        let audio = StorySection::new(
            "AudioPlayer",
            "",
            AnyView::new(
                stack(FlexDirection::Column, Align::Stretch, 10.0, audio_items).frame_width(720.0),
            ),
        );
        stack(
            FlexDirection::Column,
            Align::Stretch,
            16.0,
            vec![AnyView::new(video), AnyView::new(audio)],
        )
    }
}

#[cfg(all(test, feature = "media-ffmpeg"))]
mod tests {
    use super::backend::{source, source_name};

    #[test]
    fn names_and_sources_follow_the_location() {
        assert_eq!(source_name("/movies/Clip One.mp4"), "Clip One.mp4");
        assert_eq!(
            source_name("https://cdn.example.com/a/b.mp4?token=1"),
            "b.mp4"
        );
        assert!(source("   ").is_none());
        assert!(matches!(
            source("https://example.com/v.mp4"),
            Some(cove_media::MediaSource::Uri(_))
        ));
        assert!(matches!(
            source("/tmp/v.mp4"),
            Some(cove_media::MediaSource::File(_))
        ));
    }
}
