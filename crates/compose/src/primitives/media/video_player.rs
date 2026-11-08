//! A video player with its controls inside the picture, as in mainstream players:
//! the seek bar and control row fade in over a gradient at the bottom and hide
//! while playing; a click toggles playback, a double click toggles full screen.
//!
//! Full screen presents the same surface in a window-filling portal and asks the
//! host for a full-screen window. Picture-in-picture opens an always-on-top
//! window through [`WindowPresentation`]; hosts without one (mobile, tests) get
//! an in-window floating player instead.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cove_composition::{AnyView, PrimitiveView, TupleView, View, ViewExt};
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, Align, Edge, EventResult, FlexDirection,
    FocusChainHolder, HoverPhase, InputHandlerRoute, InteractionRegistryHolder, Justify,
    LayoutCalc, LayoutNode, LayoutStyleValue, Node, NodeResource, PointerPhase, PositionType, Wrap,
};
use cove_core::{Color, LinearGradient, Point, Signal, Size};
use cove_media::{
    MediaTime, PlaybackControl, PlaybackState, PlayerSnapshot, SeekRequest, TrackKind,
};
use cove_platform::MouseButton;

use super::danmaku::{DanmakuComment, DanmakuFeed, DanmakuLayer, DanmakuSettings};
use super::frame_ticker::FrameTicker;
use super::image_geometry::ImageContentMode;
use super::media_player::{format_media_time, use_playback};
use super::progress_bar::MediaProgressBar;
use super::ui_icon_catalog::{action, asset, editor, media};
use super::video::VideoView;
use crate::core::{FloatingWindowId, FloatingWindowRequest, WindowPresentation};
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{FrameDimension, LayoutFrameExt, LayoutValuesExt, PaddingExt};
use crate::modifiers::style::AppearanceExt;
use crate::primitives::controls::button::{Button, ButtonIconSource, ButtonRole};
use crate::primitives::controls::fields::Slider;
use crate::primitives::controls::menu_button::MenuButton;
use crate::primitives::feedback::Spinner;
use crate::primitives::layout::ZStack;
use crate::primitives::overlays::anchored::placement::PopoverPlacement;
use crate::primitives::overlays::menu::{MenuEntry, MenuItem, MenuSubmenu};
use crate::primitives::overlays::portal::{
    PortalPlacementMode, PortalResource, PortalStoreEnvironment, PortalStoreHolder,
    portal_window_bounds,
};
use crate::primitives::text::Text;
use crate::theme::styles::buttons::{ButtonStyleViewExt, ghost};
use crate::theme::tokens::semantic_modifiers::SemanticStyleViewExt;
use crate::theme::{ControlSize, ControlSizeViewExt, SemanticFontRef};
use crate::{BoxView, Row, Scope, Spacer, VerticalAlignment, impl_primitive_view};

/// Where the player's picture is shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlayerPresentation {
    #[default]
    Inline,
    Fullscreen,
    PictureInPicture,
}

/// Viewer choices the settings menu edits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerSettings {
    pub content_mode: ImageContentMode,
    pub danmaku: DanmakuSettings,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        Self {
            content_mode: ImageContentMode::Fit,
            danmaku: DanmakuSettings::default(),
        }
    }
}

pub const PLAYBACK_RATES: [f64; 7] = [0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0];
const SEEK_STEP: f64 = 5.0;
const VOLUME_STEP: f64 = 0.1;

#[derive(Clone, Debug, PartialEq)]
struct PlayerUi {
    controls: bool,
    presentation: PlayerPresentation,
    settings: PlayerSettings,
    hover: Option<f64>,
    /// Seconds under the scrubber, and when it was released.
    scrub: Option<(f64, Option<Instant>)>,
    volume: Option<f64>,
    restore_volume: f64,
}

impl Default for PlayerUi {
    fn default() -> Self {
        Self {
            controls: true,
            presentation: PlayerPresentation::Inline,
            settings: PlayerSettings::default(),
            hover: None,
            scrub: None,
            volume: None,
            restore_volume: 1.0,
        }
    }
}

/// Imperative state that must not recompose the player.
struct PlayerRuntime {
    activity: Cell<Instant>,
    /// Last pointer position over the picture; only real movement counts as activity.
    pointer: Cell<Option<(f32, f32)>>,
    over_controls: Cell<bool>,
    floating: Cell<Option<FloatingWindowId>>,
    /// The host put the window in full screen for this player.
    window_fullscreen: Cell<bool>,
    /// The host reported the full-screen window, so a later `false` means the
    /// viewer left full screen through the system.
    fullscreen_confirmed: Cell<bool>,
    polls: Cell<u32>,
}

impl Default for PlayerRuntime {
    fn default() -> Self {
        Self {
            activity: Cell::new(Instant::now()),
            pointer: Cell::new(None),
            over_controls: Cell::new(false),
            floating: Cell::new(None),
            window_fullscreen: Cell::new(false),
            fullscreen_confirmed: Cell::new(false),
            polls: Cell::new(0),
        }
    }
}

/// Commands shared by the gesture layer, buttons, keys and the PiP window.
#[derive(Clone)]
pub struct PlayerActions {
    control: Arc<dyn PlaybackControl>,
    ui: Signal<PlayerUi>,
    runtime: Rc<PlayerRuntime>,
    title: Option<String>,
}

impl PlayerActions {
    fn snapshot(&self) -> PlayerSnapshot {
        self.control.snapshot()
    }

    pub fn toggle_playback(&self) {
        super::media_player::toggle_playback(self.control.as_ref(), &self.snapshot());
        self.activity();
    }

    pub fn seek_by(&self, delta: f64) {
        let snapshot = self.snapshot();
        let Some(duration) = snapshot.duration else {
            return;
        };
        let target = (snapshot.position.unwrap_or_default().seconds() + delta)
            .clamp(0.0, duration.seconds());
        if let Ok(time) = MediaTime::from_seconds(target) {
            let _ = self.control.seek(SeekRequest::exact(time));
        }
        self.activity();
    }

    pub fn set_volume(&self, volume: f64) {
        let volume = volume.clamp(0.0, 1.0);
        self.ui.update(|ui| ui.volume = Some(volume));
        let _ = self.control.set_volume(volume);
    }

    fn volume(&self) -> f64 {
        self.ui
            .with_untracked(|ui| ui.volume)
            .unwrap_or_else(|| self.snapshot().volume)
    }

    pub fn toggle_mute(&self) {
        let volume = self.volume();
        if volume > 0.0 {
            self.ui.update(|ui| ui.restore_volume = volume);
            self.set_volume(0.0);
        } else {
            let restore = self.ui.with_untracked(|ui| ui.restore_volume).max(0.05);
            self.set_volume(restore);
        }
    }

    pub fn set_rate(&self, rate: f64) {
        let _ = self.control.set_rate(rate);
    }

    pub fn update_settings(&self, change: impl FnOnce(&mut PlayerSettings)) {
        self.ui.update(|ui| change(&mut ui.settings));
    }

    pub fn toggle_danmaku(&self) {
        self.update_settings(|settings| settings.danmaku.enabled = !settings.danmaku.enabled);
    }

    fn scrub(&self, seconds: f64, committed: bool) {
        self.ui.update(|ui| {
            ui.scrub = Some((seconds, committed.then(Instant::now)));
        });
        if let Ok(time) = MediaTime::from_seconds(seconds.max(0.0)) {
            let request = if committed {
                SeekRequest::exact(time)
            } else {
                SeekRequest::keyframe(time)
            };
            let _ = self.control.seek(request);
        }
        self.activity();
    }

    fn hover(&self, seconds: Option<f64>) {
        self.ui.update_if(|ui| {
            let changed = ui.hover != seconds;
            ui.hover = seconds;
            changed
        });
    }

    /// Shows the controls and restarts the hide countdown.
    pub fn activity(&self) {
        self.runtime.activity.set(Instant::now());
        self.ui.update_if(|ui| {
            let hidden = !ui.controls;
            ui.controls = true;
            hidden
        });
    }

    fn hide_controls(&self) {
        self.ui.update_if(|ui| {
            let shown = ui.controls;
            ui.controls = false;
            ui.hover = None;
            shown
        });
    }

    pub fn presentation(&self) -> PlayerPresentation {
        self.ui.with_untracked(|ui| ui.presentation)
    }

    pub fn toggle_fullscreen(&self) {
        self.set_presentation(if self.presentation() == PlayerPresentation::Fullscreen {
            PlayerPresentation::Inline
        } else {
            PlayerPresentation::Fullscreen
        });
    }

    pub fn toggle_picture_in_picture(&self) {
        self.set_presentation(
            if self.presentation() == PlayerPresentation::PictureInPicture {
                PlayerPresentation::Inline
            } else {
                PlayerPresentation::PictureInPicture
            },
        );
    }

    pub fn set_presentation(&self, next: PlayerPresentation) {
        let current = self.presentation();
        if current == next {
            return;
        }
        match current {
            PlayerPresentation::Fullscreen if self.runtime.window_fullscreen.replace(false) => {
                WindowPresentation::set_fullscreen(false);
            }
            PlayerPresentation::PictureInPicture => {
                if let Some(id) = self.runtime.floating.take() {
                    WindowPresentation::close_floating(id);
                }
            }
            _ => {}
        }
        self.runtime.fullscreen_confirmed.set(false);
        self.runtime.polls.set(0);
        match next {
            PlayerPresentation::Fullscreen => {
                self.runtime
                    .window_fullscreen
                    .set(WindowPresentation::set_fullscreen(true));
            }
            PlayerPresentation::PictureInPicture => {
                let actions = self.clone();
                let floating = WindowPresentation::open_floating(FloatingWindowRequest {
                    title: self
                        .title
                        .clone()
                        .unwrap_or_else(|| "Picture in Picture".into()),
                    width: 480.0,
                    height: 270.0,
                    content: AnyView::new(PictureInPicture {
                        actions,
                        floating: true,
                    }),
                });
                self.runtime.floating.set(floating);
            }
            PlayerPresentation::Inline => {}
        }
        self.ui.update(|ui| {
            ui.presentation = next;
            ui.controls = true;
        });
        self.runtime.activity.set(Instant::now());
    }
}

/// A video with in-picture controls, danmaku, settings, full screen and PiP.
#[derive(Clone)]
pub struct VideoPlayer {
    pub control: Arc<dyn PlaybackControl>,
    pub title: Option<String>,
    pub danmaku: Option<DanmakuFeed>,
    pub settings: PlayerSettings,
    /// Seconds of stillness before the controls hide while playing.
    pub autohide: f64,
    pub corner_radius: f32,
    pub allows_fullscreen: bool,
    pub allows_picture_in_picture: bool,
}

impl VideoPlayer {
    pub fn new(control: Arc<dyn PlaybackControl>) -> Self {
        Self {
            control,
            title: None,
            danmaku: None,
            settings: PlayerSettings::default(),
            autohide: 2.5,
            corner_radius: 8.0,
            allows_fullscreen: true,
            allows_picture_in_picture: true,
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn danmaku(mut self, feed: DanmakuFeed) -> Self {
        self.danmaku = Some(feed);
        self
    }

    /// Initial settings; the viewer changes them from the settings menu.
    pub fn settings(mut self, settings: PlayerSettings) -> Self {
        self.settings = settings;
        self
    }

    pub fn autohide(mut self, seconds: f64) -> Self {
        if seconds.is_finite() && seconds > 0.0 {
            self.autohide = seconds;
        }
        self
    }

    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = radius.max(0.0);
        self
    }

    pub fn allows_fullscreen(mut self, allows: bool) -> Self {
        self.allows_fullscreen = allows;
        self
    }

    pub fn allows_picture_in_picture(mut self, allows: bool) -> Self {
        self.allows_picture_in_picture = allows;
        self
    }
}

impl View for VideoPlayer {
    fn body(&self, cx: &Scope) -> AnyView {
        let snapshot = use_playback(cx, &self.control);
        let initial = self.settings;
        let ui_signal = cx.state(move || PlayerUi {
            settings: initial,
            ..PlayerUi::default()
        });
        let runtime = cx.remember(|| Rc::new(PlayerRuntime::default()));
        let actions = PlayerActions {
            control: self.control.clone(),
            ui: ui_signal,
            runtime,
            title: self.title.clone(),
        };
        let ui = ui_signal.get();
        let surface = || {
            Surface {
                player: self.clone(),
                actions: actions.clone(),
                snapshot: snapshot.clone(),
                ui: ui.clone(),
            }
            .build()
        };
        let inline = match ui.presentation {
            PlayerPresentation::Inline => surface(),
            PlayerPresentation::Fullscreen => placeholder(&actions, "Playing in full screen"),
            PlayerPresentation::PictureInPicture => {
                placeholder(&actions, "Playing in picture-in-picture")
            }
        };
        let overlay = match ui.presentation {
            PlayerPresentation::Fullscreen => Some(Presented::Fullscreen(surface())),
            PlayerPresentation::PictureInPicture if actions.runtime.floating.get().is_none() => {
                Some(Presented::Floating(AnyView::new(PictureInPicture {
                    actions: actions.clone(),
                    floating: false,
                })))
            }
            _ => None,
        };
        AnyView::new(PresentationHost {
            content: inline,
            overlay,
            actions,
            radius: self.corner_radius,
        })
    }
}

/// Everything layered over the picture, rebuilt from one snapshot.
struct Surface {
    player: VideoPlayer,
    actions: PlayerActions,
    snapshot: PlayerSnapshot,
    ui: PlayerUi,
}

fn fill(view: impl View + 'static) -> AnyView {
    AnyView::new(view.frame(
        Some(FrameDimension::Percent(100.0)),
        Some(FrameDimension::Percent(100.0)),
        None,
        None,
        None,
        None,
    ))
}

fn overlay_button(
    icon: cove_imaging::BundleImageResource,
    label: &str,
    size: f32,
    action: impl Fn() + 'static,
) -> AnyView {
    AnyView::new(
        Button::icon_with_options(
            ButtonIconSource::resource(icon),
            Some(size),
            Some(Color::WHITE),
            ButtonRole::Normal,
            true,
            false,
            false,
            Some(label.to_owned()),
            action,
        )
        .button_style(ghost())
        .control_size(ControlSize::Regular)
        .accessibility_label(label),
    )
}

fn white_text(text: impl Into<String>) -> AnyView {
    AnyView::new(
        Text::new(text.into())
            .color(Color::WHITE)
            .line_limit(Some(1))
            .semantic_font(SemanticFontRef::caption()),
    )
}

impl Surface {
    fn playing(&self) -> bool {
        self.snapshot.state == PlaybackState::Playing
    }

    fn build(self) -> AnyView {
        let mut layers = vec![fill(
            VideoView::new(self.player.control.video_output())
                .content_mode(self.ui.settings.content_mode)
                .background(Some(Color::BLACK)),
        )];
        if let Some(feed) = self.player.danmaku.clone()
            && self.ui.settings.danmaku.enabled
        {
            layers.push(fill(
                DanmakuLayer::new(feed, self.position())
                    .settings(self.ui.settings.danmaku)
                    .playing(self.playing(), self.snapshot.rate),
            ));
        }
        layers.push(fill(GestureLayer {
            actions: self.actions.clone(),
            playing: self.playing(),
            controls: self.ui.controls,
            autohide: self.player.autohide,
        }));
        if let Some(center) = self.center() {
            layers.push(center);
        }
        // The bar stays mounted and only fades: removing it would change the hit
        // target under a resting pointer, which reads as movement and shows it again.
        let visible = self.ui.controls || !self.playing();
        layers.push(fill(ChromeVisibility {
            visible,
            content: fill(self.chrome()),
        }));
        AnyView::new(
            ZStack::new(TupleView::new(layers))
                .background_color(Color::BLACK)
                .clipped()
                .accessibility_label(self.player.title.clone().unwrap_or_else(|| "Video".into())),
        )
    }

    fn position(&self) -> f64 {
        match self.ui.scrub {
            Some((seconds, None)) => seconds,
            Some((seconds, Some(released)))
                if self.snapshot.seeking || released.elapsed() < Duration::from_millis(400) =>
            {
                seconds
            }
            _ => self.snapshot.position.unwrap_or_default().seconds(),
        }
    }

    fn center(&self) -> Option<AnyView> {
        let loading = self.snapshot.state == PlaybackState::Opening || self.snapshot.seeking;
        if loading {
            return Some(AnyView::new(Spinner::new().size(36.0)));
        }
        if matches!(
            self.snapshot.state,
            PlaybackState::Paused | PlaybackState::Ended
        ) {
            let actions = self.actions.clone();
            return Some(AnyView::new(
                Button::icon_with_options(
                    ButtonIconSource::resource(media::play_fill()),
                    Some(30.0),
                    Some(Color::WHITE),
                    ButtonRole::Normal,
                    true,
                    false,
                    false,
                    None,
                    move || actions.toggle_playback(),
                )
                .button_style(ghost())
                .frame_size(64.0, 64.0)
                .background_color(Color::BLACK.multiplied_alpha(0.45))
                .corner_radius(32.0)
                .accessibility_label("Play"),
            ));
        }
        None
    }

    fn chrome(&self) -> AnyView {
        let mut column: Vec<AnyView> = Vec::new();
        if let Some(title) = &self.player.title {
            column.push(AnyView::new(
                Row::new(VerticalAlignment::Center, 0.0, white_text(title.clone()))
                    .padding_edges(14.0, 10.0)
                    .background_gradient(LinearGradient {
                        start: Color::BLACK.multiplied_alpha(0.55),
                        end: Color::BLACK.multiplied_alpha(0.0),
                        angle: 180.0,
                    }),
            ));
        }
        column.push(AnyView::new(Spacer::new(0.0)));
        column.push(AnyView::new(ControlsRegion {
            runtime: self.actions.runtime.clone(),
            content: self.bottom(),
        }));
        AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(TupleView::new(column)),
        ))
    }

    fn bottom(&self) -> AnyView {
        let duration = self.snapshot.duration.map_or(0.0, MediaTime::seconds);
        let position = self.position();
        let seekable = self.snapshot.seekable_range.is_some() && duration > 0.0;
        let scrub = self.actions.clone();
        let hover = self.actions.clone();
        let bar = MediaProgressBar::new(position, duration)
            .enabled(seekable)
            .on_scrub(move |seconds, committed| scrub.scrub(seconds, committed))
            .on_hover(move |seconds| hover.hover(seconds));
        let mut bar_layers = vec![AnyView::new(bar)];
        if let Some(seconds) = self.ui.hover.filter(|_| duration > 0.0) {
            let fraction = (seconds / duration).clamp(0.0, 1.0) as f32;
            bar_layers.push(AnyView::new(
                white_text(format_media_time(
                    MediaTime::from_seconds(seconds).ok(),
                    self.snapshot.duration,
                ))
                .padding_edges(6.0, 2.0)
                .background_color(Color::BLACK.multiplied_alpha(0.75))
                .corner_radius(4.0)
                .position_value(LayoutStyleValue::Points(-26.0), Edge::Top)
                .position_value(
                    LayoutStyleValue::Calc(LayoutCalc::new(fraction * 100.0, -22.0)),
                    Edge::Left,
                )
                .modifier(AbsolutePosition),
            ));
        }
        let bar_block = AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(TupleView::new(bar_layers)),
        ));
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(vec![bar_block, self.controls_row()])),
            )
            .padding_edges(12.0, 6.0)
            .background_gradient(LinearGradient {
                start: Color::BLACK.multiplied_alpha(0.0),
                end: Color::BLACK.multiplied_alpha(0.7),
                angle: 180.0,
            }),
        )
    }

    fn controls_row(&self) -> AnyView {
        let actions = &self.actions;
        let playing = self.playing();
        let play = {
            let actions = actions.clone();
            overlay_button(
                if playing {
                    editor::pause()
                } else {
                    editor::play()
                },
                if playing { "Pause" } else { "Play" },
                18.0,
                move || actions.toggle_playback(),
            )
        };
        let volume = self.ui.volume.unwrap_or(self.snapshot.volume);
        let mute = {
            let actions = actions.clone();
            overlay_button(
                if volume <= 0.0 {
                    media::volume_x()
                } else {
                    media::volume()
                },
                if volume <= 0.0 { "Unmute" } else { "Mute" },
                18.0,
                move || actions.toggle_mute(),
            )
        };
        let volume_slider = {
            let actions = actions.clone();
            let slider = Slider::new(cove_core::Binding::new(
                move || volume,
                move |value: f64| actions.set_volume(value),
            ))
            .range((0.0, 1.0))
            .accessibility_label("Volume")
            .frame_width(72.0);
            // The thumb overhangs the track; the padded row keeps it off the time label.
            AnyView::new(
                Row::new(VerticalAlignment::Center, 0.0, AnyView::new(slider))
                    .padding_edges(10.0, 0.0),
            )
        };
        let time = white_text(format!(
            "{} / {}",
            format_media_time(
                MediaTime::from_seconds(self.position().max(0.0)).ok(),
                self.snapshot.duration
            ),
            format_media_time(self.snapshot.duration, self.snapshot.duration)
        ));
        let mut items = vec![
            play,
            mute,
            volume_slider,
            AnyView::new(time.padding_edges(8.0, 0.0)),
            AnyView::new(Spacer::new(0.0)),
        ];
        if self.player.danmaku.is_some() {
            let enabled = self.ui.settings.danmaku.enabled;
            let actions = actions.clone();
            items.push(overlay_button(
                if enabled {
                    media::danmaku()
                } else {
                    media::danmaku_off()
                },
                if enabled {
                    "Hide danmaku"
                } else {
                    "Show danmaku"
                },
                18.0,
                move || actions.toggle_danmaku(),
            ));
        }
        items.push(self.settings_menu());
        if self.player.allows_picture_in_picture {
            let actions = actions.clone();
            items.push(overlay_button(
                media::picture_in_picture(),
                "Picture in picture",
                18.0,
                move || actions.toggle_picture_in_picture(),
            ));
        }
        if self.player.allows_fullscreen {
            let fullscreen = self.ui.presentation == PlayerPresentation::Fullscreen;
            let actions = actions.clone();
            items.push(overlay_button(
                asset(if fullscreen { "minimize" } else { "maximize" }),
                if fullscreen {
                    "Exit full screen"
                } else {
                    "Full screen"
                },
                18.0,
                move || actions.toggle_fullscreen(),
            ));
        }
        AnyView::new(Row::new(
            VerticalAlignment::Center,
            2.0,
            AnyView::new(TupleView::new(items)),
        ))
    }

    fn settings_menu(&self) -> AnyView {
        let actions = &self.actions;
        let settings = self.ui.settings;
        let mut entries = Vec::new();
        let rate = self.snapshot.rate;
        entries.push(MenuEntry::Submenu(MenuSubmenu::new(
            100,
            format!("Speed · {}", rate_label(rate)),
            PLAYBACK_RATES
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    let actions = actions.clone();
                    let value = *value;
                    MenuEntry::Item(
                        MenuItem::new(101 + index as u64, rate_label(value), move || {
                            actions.set_rate(value)
                        })
                        .selected((rate - value).abs() < 0.01),
                    )
                })
                .collect(),
        )));
        let modes = [
            (ImageContentMode::Fit, "Fit"),
            (ImageContentMode::Fill, "Fill"),
            (ImageContentMode::Stretch, "Stretch"),
        ];
        entries.push(MenuEntry::Submenu(MenuSubmenu::new(
            200,
            "Picture",
            modes
                .iter()
                .enumerate()
                .map(|(index, (mode, title))| {
                    let actions = actions.clone();
                    let mode = *mode;
                    MenuEntry::Item(
                        MenuItem::new(201 + index as u64, *title, move || {
                            actions.update_settings(|settings| settings.content_mode = mode)
                        })
                        .selected(settings.content_mode == mode),
                    )
                })
                .collect(),
        )));
        if self.player.danmaku.is_some() {
            entries.push(MenuEntry::Submenu(danmaku_menu(actions, settings.danmaku)));
        }
        let audio: Vec<_> = self
            .snapshot
            .tracks
            .iter()
            .filter(|track| track.kind == TrackKind::Audio)
            .cloned()
            .collect();
        if audio.len() > 1 {
            let selected = self.snapshot.selected_tracks.clone();
            entries.push(MenuEntry::Submenu(MenuSubmenu::new(
                400,
                "Audio track",
                audio
                    .iter()
                    .enumerate()
                    .map(|(index, track)| {
                        let control = actions.control.clone();
                        let chosen = track.id.clone();
                        let others = selected.clone();
                        MenuEntry::Item(
                            MenuItem::new(
                                401 + index as u64,
                                track
                                    .description
                                    .clone()
                                    .unwrap_or_else(|| track.id.clone()),
                                move || {
                                    let mut tracks: Vec<String> = others
                                        .iter()
                                        .filter(|id| !id.starts_with("audio:"))
                                        .cloned()
                                        .collect();
                                    tracks.push(chosen.clone());
                                    let _ = control.select_tracks(tracks);
                                },
                            )
                            .selected(selected.contains(&track.id)),
                        )
                    })
                    .collect(),
            )));
        }
        AnyView::new(
            MenuButton::new(
                entries,
                AnyView::new(super::icon::Icon::with_color(
                    action::settings(),
                    18.0,
                    Color::WHITE,
                )),
            )
            .ghost()
            .placement(PopoverPlacement::TopTrailing)
            .width(220.0)
            .accessibility_label("Settings")
            .tooltip("Settings"),
        )
    }
}

fn rate_label(rate: f64) -> String {
    if (rate - 1.0).abs() < 0.01 {
        "Normal".into()
    } else {
        format!("{}×", (rate * 100.0).round() / 100.0)
    }
}

fn danmaku_menu(actions: &PlayerActions, settings: DanmakuSettings) -> MenuSubmenu {
    let option =
        |id: u64, title: &str, selected: bool, change: Rc<dyn Fn(&mut DanmakuSettings)>| {
            let actions = actions.clone();
            MenuEntry::Item(
                MenuItem::new(id, title, move || {
                    let change = change.clone();
                    actions.update_settings(move |player| change(&mut player.danmaku));
                })
                .selected(selected),
            )
        };
    let mut entries = vec![MenuEntry::label(300, "Opacity")];
    for (index, opacity) in [1.0f32, 0.75, 0.5, 0.25].into_iter().enumerate() {
        entries.push(option(
            301 + index as u64,
            &format!("{}%", (opacity * 100.0) as u32),
            (settings.opacity - opacity).abs() < 0.01,
            Rc::new(move |danmaku| danmaku.opacity = opacity),
        ));
    }
    entries.push(MenuEntry::separator(310));
    entries.push(MenuEntry::label(311, "Speed"));
    for (index, (seconds, title)) in [(12.0f32, "Slow"), (8.0, "Normal"), (5.0, "Fast")]
        .into_iter()
        .enumerate()
    {
        entries.push(option(
            312 + index as u64,
            title,
            (settings.scroll_seconds - seconds).abs() < 0.01,
            Rc::new(move |danmaku| danmaku.scroll_seconds = seconds),
        ));
    }
    entries.push(MenuEntry::separator(320));
    entries.push(MenuEntry::label(321, "Size"));
    for (index, (scale, title)) in [(0.8f32, "Small"), (1.0, "Normal"), (1.3, "Large")]
        .into_iter()
        .enumerate()
    {
        entries.push(option(
            322 + index as u64,
            title,
            (settings.font_scale - scale).abs() < 0.01,
            Rc::new(move |danmaku| danmaku.font_scale = scale),
        ));
    }
    entries.push(MenuEntry::separator(330));
    entries.push(MenuEntry::label(331, "Area"));
    for (index, (area, title)) in [
        (0.25f32, "Quarter"),
        (0.5, "Half"),
        (0.75, "Three quarters"),
        (1.0, "Full"),
    ]
    .into_iter()
    .enumerate()
    {
        entries.push(option(
            332 + index as u64,
            title,
            (settings.area - area).abs() < 0.01,
            Rc::new(move |danmaku| danmaku.area = area),
        ));
    }
    MenuSubmenu::new(299, "Danmaku", entries)
}

/// What the player shows while its picture is elsewhere.
fn placeholder(actions: &PlayerActions, message: &str) -> AnyView {
    let back = actions.clone();
    AnyView::new(
        ZStack::new(TupleView::new(vec![AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Center,
            Justify::Center,
            Wrap::NoWrap,
            10.0,
            AnyView::new(TupleView::new(vec![
                white_text(message.to_owned()),
                AnyView::new(
                    Button::title("Return", move || {
                        back.set_presentation(PlayerPresentation::Inline)
                    })
                    .control_size(ControlSize::Small),
                ),
            ])),
        ))]))
        .background_color(Color::from_rgba8(24, 24, 27, 255)),
    )
}

/// Marks a modifier chain's node as absolutely positioned.
struct AbsolutePosition;

impl cove_composition::ViewModifier for AbsolutePosition {
    fn apply_layout(&self, layout: &LayoutNode) {
        layout.set_position_type(PositionType::Absolute);
    }
}

enum Presented {
    Fullscreen(AnyView),
    Floating(AnyView),
}

/// Lays out the inline content (16:9 until the first frame) and presents the
/// full-screen or in-window floating surface through a portal.
struct PresentationHost {
    content: AnyView,
    overlay: Option<Presented>,
    actions: PlayerActions,
    radius: f32,
}

impl PrimitiveView for PresentationHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.set_clips_to_bounds(true);
        let portal: Rc<dyn NodeResource> = PortalResource::new();
        node.add_resource(portal);
        let ticker: Rc<dyn NodeResource> = FrameTicker::new();
        node.add_resource(ticker);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_aspect_ratio(Some(16.0 / 9.0));
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        node.set_corner_radius(self.radius);
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Group,
            label: crate::i18n::tr("Video player"),
            ..Default::default()
        }));
        if let Some(ticker) = node.first_resource::<FrameTicker>() {
            let actions = self.actions.clone();
            let watching = matches!(
                actions.presentation(),
                PlayerPresentation::Fullscreen | PlayerPresentation::PictureInPicture
            );
            ticker.configure(watching, Rc::new(move |_| watch_presentation(&actions)));
        }
        let Some(resource) = node.first_resource::<PortalResource>() else {
            return;
        };
        let store = Some(
            node.composition_value(&PortalStoreEnvironment::key())
                .unwrap_or_else(PortalStoreHolder::current),
        );
        match &self.overlay {
            Some(Presented::Fullscreen(content)) => resource.present(
                store,
                Point::default(),
                None,
                AnyView::new(FullscreenKeys {
                    actions: self.actions.clone(),
                    content: fill(content.clone()),
                }),
                true,
                PortalPlacementMode::Fixed,
                None,
                None,
            ),
            Some(Presented::Floating(content)) => {
                let bounds = portal_window_bounds(node);
                let size = Size {
                    width: 320.0,
                    height: 180.0,
                };
                resource.present(
                    store,
                    Point::new(
                        bounds.x + bounds.width - size.width - 16.0,
                        bounds.y + bounds.height - size.height - 16.0,
                    ),
                    Some(size.width),
                    AnyView::new(content.clone().frame_size(size.width, size.height)),
                    false,
                    PortalPlacementMode::Fixed,
                    None,
                    None,
                )
            }
            None => NodeResource::unmount(&*resource, node),
        }
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![fill(self.content.clone())]
    }
}
impl_primitive_view!(PresentationHost);

/// Returns to inline when the PiP window or the system full screen was closed.
fn watch_presentation(actions: &PlayerActions) {
    let runtime = &actions.runtime;
    let polls = runtime.polls.get().wrapping_add(1);
    runtime.polls.set(polls);
    if !polls.is_multiple_of(15) {
        return;
    }
    match actions.presentation() {
        PlayerPresentation::PictureInPicture => {
            if let Some(id) = runtime.floating.get()
                && !WindowPresentation::is_floating_open(id)
            {
                runtime.floating.set(None);
                actions.set_presentation(PlayerPresentation::Inline);
            }
        }
        PlayerPresentation::Fullscreen if runtime.window_fullscreen.get() => {
            let fullscreen = WindowPresentation::is_fullscreen();
            if fullscreen {
                runtime.fullscreen_confirmed.set(true);
            } else if runtime.fullscreen_confirmed.get() {
                runtime.window_fullscreen.set(false);
                actions.set_presentation(PlayerPresentation::Inline);
            }
        }
        _ => {}
    }
}

/// The full-screen surface plus Escape to leave it.
struct FullscreenKeys {
    actions: PlayerActions,
    content: AnyView,
}

impl PrimitiveView for FullscreenKeys {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_node(&self, node: &Node) {
        node.set_background_color(Some(Color::BLACK));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let _ = &self.actions;
        vec![self.content.clone()]
    }
}
impl_primitive_view!(FullscreenKeys);

/// Shows or hides the control layer without unmounting it.
struct ChromeVisibility {
    visible: bool,
    content: AnyView,
}

impl PrimitiveView for ChromeVisibility {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_node(&self, node: &Node) {
        node.set_opacity(if self.visible { 1.0 } else { 0.0 });
        node.set_allows_hit_testing(self.visible);
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(ChromeVisibility);

/// Tracks the pointer over the control bar so it does not hide under it.
struct ControlsRegion {
    runtime: Rc<PlayerRuntime>,
    content: AnyView,
}

impl PrimitiveView for ControlsRegion {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_node(&self, node: &Node) {
        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        let runtime = self.runtime.clone();
        registry.set_hover(
            node,
            Rc::new(move |phase| {
                let entered = phase == HoverPhase::Enter;
                runtime.over_controls.set(entered);
                if entered {
                    runtime.activity.set(Instant::now());
                }
            }),
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(ControlsRegion);

/// Clicks, double clicks, pointer activity, keyboard shortcuts and auto-hide.
struct GestureLayer {
    actions: PlayerActions,
    playing: bool,
    controls: bool,
    autohide: f64,
}

impl PrimitiveView for GestureLayer {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node.set_is_focusable(true);
        let ticker: Rc<dyn NodeResource> = FrameTicker::new();
        node.add_resource(ticker);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_node(&self, node: &Node) {
        if let Some(ticker) = node.first_resource::<FrameTicker>() {
            let actions = self.actions.clone();
            let autohide = Duration::from_secs_f64(self.autohide);
            ticker.configure(
                self.playing && self.controls,
                Rc::new(move |_| {
                    let runtime = &actions.runtime;
                    if !runtime.over_controls.get() && runtime.activity.get().elapsed() >= autohide
                    {
                        actions.hide_controls();
                    }
                }),
            );
        }
        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        let actions = self.actions.clone();
        let focus_node = node.downgrade();
        registry.set_pointer(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase, _| {
                if event.button != MouseButton::Left || phase != PointerPhase::Down {
                    return EventResult::Ignored;
                }
                if let (Some(chain), Some(owner)) =
                    (FocusChainHolder::current(), Node::upgrade(&focus_node))
                {
                    chain.focus(Some(&owner), false);
                }
                if event.clicks >= 2 {
                    // The first click of the pair already toggled playback; undo it.
                    actions.toggle_playback();
                    actions.toggle_fullscreen();
                } else {
                    actions.toggle_playback();
                }
                EventResult::Handled
            }),
        );
        let motion = self.actions.clone();
        registry.set_motion(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, _| {
                let runtime = &motion.runtime;
                let moved = runtime.pointer.get().is_none_or(|(x, y)| {
                    (x - event.x).abs() >= 1.0 || (y - event.y).abs() >= 1.0
                });
                runtime.pointer.set(Some((event.x, event.y)));
                runtime.over_controls.set(false);
                if moved {
                    motion.activity();
                }
                EventResult::Ignored
            }),
        );
        let keys = self.actions.clone();
        registry.set_key(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, _| handle_key(&keys, event.scancode)),
        );
    }
}
impl_primitive_view!(GestureLayer);

/// Space/K play, ←/→ seek 5 s, ↑/↓ volume, F full screen, M mute, D danmaku,
/// P picture-in-picture, Escape back to inline.
fn handle_key(actions: &PlayerActions, scancode: u32) -> EventResult {
    use crate::Scancode;
    match scancode {
        Scancode::SPACE | Scancode::K => actions.toggle_playback(),
        Scancode::ARROW_LEFT => actions.seek_by(-SEEK_STEP),
        Scancode::ARROW_RIGHT => actions.seek_by(SEEK_STEP),
        Scancode::ARROW_UP => {
            actions.set_volume(actions.volume() + VOLUME_STEP);
            actions.activity();
        }
        Scancode::ARROW_DOWN => {
            actions.set_volume(actions.volume() - VOLUME_STEP);
            actions.activity();
        }
        Scancode::F => actions.toggle_fullscreen(),
        Scancode::M => actions.toggle_mute(),
        Scancode::D => actions.toggle_danmaku(),
        Scancode::P => actions.toggle_picture_in_picture(),
        Scancode::ESCAPE if actions.presentation() != PlayerPresentation::Inline => {
            actions.set_presentation(PlayerPresentation::Inline)
        }
        _ => return EventResult::Ignored,
    }
    EventResult::Handled
}

/// The compact picture shown in a PiP window or the in-window floating player.
struct PictureInPicture {
    actions: PlayerActions,
    floating: bool,
}

impl View for PictureInPicture {
    fn body(&self, cx: &Scope) -> AnyView {
        let snapshot = use_playback(cx, &self.actions.control);
        let playing = snapshot.state == PlaybackState::Playing;
        let toggle = self.actions.clone();
        let back = self.actions.clone();
        let controls = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::SpaceBetween,
            Wrap::NoWrap,
            4.0,
            AnyView::new(TupleView::new(vec![
                overlay_button(
                    asset("picture-in-picture"),
                    "Back to player",
                    16.0,
                    move || back.set_presentation(PlayerPresentation::Inline),
                ),
                overlay_button(
                    if playing {
                        editor::pause()
                    } else {
                        editor::play()
                    },
                    if playing { "Pause" } else { "Play" },
                    16.0,
                    move || toggle.toggle_playback(),
                ),
            ])),
        ));
        let chrome =
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexEnd,
                Wrap::NoWrap,
                0.0,
                AnyView::new(controls.padding_edges(6.0, 4.0).background_gradient(
                    LinearGradient {
                        start: Color::BLACK.multiplied_alpha(0.0),
                        end: Color::BLACK.multiplied_alpha(0.65),
                        angle: 180.0,
                    },
                )),
            ));
        let picture = ZStack::new(TupleView::new(vec![
            fill(
                VideoView::new(self.actions.control.video_output()).background(Some(Color::BLACK)),
            ),
            fill(chrome),
        ]))
        .background_color(Color::BLACK)
        .clipped();
        if self.floating {
            fill(picture)
        } else {
            AnyView::new(picture.corner_radius(10.0).shadow(
                Color::BLACK.multiplied_alpha(0.35),
                0.0,
                6.0,
                18.0,
            ))
        }
    }
}

/// Sends a comment at the current playback position.
pub fn send_danmaku(
    feed: &DanmakuFeed,
    control: &dyn PlaybackControl,
    text: &str,
    color: Color,
) -> Option<DanmakuComment> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let time = control.snapshot().position.unwrap_or_default();
    let comment = DanmakuComment::new(time, text).color(color).own(true);
    let id = feed.push(comment.clone());
    Some(DanmakuComment { id, ..comment })
}

#[cfg(test)]
mod tests {
    use super::rate_label;

    #[test]
    fn rates_read_like_player_menus() {
        assert_eq!(rate_label(1.0), "Normal");
        assert_eq!(rate_label(1.5), "1.5×");
        assert_eq!(rate_label(0.75), "0.75×");
    }
}
