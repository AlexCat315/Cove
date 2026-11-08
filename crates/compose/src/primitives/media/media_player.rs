//! Playback components driven by any `cove_media::PlaybackControl`.
//!
//! The control's change callback runs on media threads; it posts at most one
//! pending snapshot refresh to the UI thread, so bodies recompose at the
//! backend's report rate and never block on the player.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use cove_core::{Binding, Signal};
use cove_media::{
    ChangeListener, MediaTime, PlaybackControl, PlaybackState, PlayerSnapshot, SeekRequest,
};

use super::audio_meter::AudioLevelMeter;
use super::ui_icon_catalog::{editor, media};
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::AppearanceExt;
use crate::primitives::controls::button::{Button, ButtonIconSource, ButtonRole};
use crate::primitives::controls::fields::Slider;
use crate::primitives::text::Text;
use crate::theme::styles::buttons::{ButtonStyleViewExt, ghost};
use crate::theme::tokens::semantic_modifiers::SemanticStyleViewExt;
use crate::theme::{ControlSize, ControlSizeViewExt, SemanticColorRef, SemanticFontRef};
use crate::{
    AnyView, BoxView, Column, HorizontalAlignment, Row, Scope, TupleView, VerticalAlignment, View,
};
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};

/// `m:ss`, or `h:mm:ss` when `scale` (usually the duration) reaches an hour.
pub fn format_media_time(time: Option<MediaTime>, scale: Option<MediaTime>) -> String {
    let Some(time) = time else {
        return "--:--".to_owned();
    };
    let total = time.0 / 1_000_000_000;
    let (hours, minutes, seconds) = (total / 3600, (total / 60) % 60, total % 60);
    if hours > 0 || scale.is_some_and(|scale| scale.0 >= 3_600_000_000_000) {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// Keeps a snapshot signal current with a control for as long as the scope lives.
#[derive(Default)]
struct PlaybackLink {
    bound: RefCell<Option<(usize, ChangeListener)>>,
}

impl PlaybackLink {
    fn bind(&self, control: &Arc<dyn PlaybackControl>, snapshot: Signal<PlayerSnapshot>) {
        let identity = Arc::as_ptr(control) as *const () as usize;
        if self
            .bound
            .borrow()
            .as_ref()
            .is_some_and(|(bound, _)| *bound == identity)
        {
            return;
        }
        snapshot.set_if_changed(control.snapshot());
        let sender = snapshot.sender();
        let pending = Arc::new(AtomicBool::new(false));
        let source = control.clone();
        let listener = control.on_change(Arc::new(move || {
            if pending.swap(true, Ordering::AcqRel) {
                return;
            }
            let pending = pending.clone();
            let source = source.clone();
            sender.update(move |current| {
                pending.store(false, Ordering::Release);
                *current = source.snapshot();
            });
        }));
        *self.bound.borrow_mut() = Some((identity, listener));
    }
}

/// The live snapshot of `control`, recomposing the calling body when it changes.
#[track_caller]
pub fn use_playback(cx: &Scope, control: &Arc<dyn PlaybackControl>) -> PlayerSnapshot {
    let snapshot = cx.state(|| control.snapshot());
    let link = cx.remember(|| Rc::new(PlaybackLink::default()));
    link.bind(control, snapshot);
    snapshot.get()
}

/// Transient scrubbing and volume state owned by a transport's scope.
#[derive(Clone, Copy)]
struct TransportState {
    /// Seconds under the thumb while dragging or until a released seek lands.
    scrub: Signal<Option<(f64, Option<Instant>)>>,
    /// Volume chosen in this UI; the snapshot lags behind a drag.
    volume: Signal<Option<f64>>,
    /// Volume to restore when unmuting.
    restore: Signal<f64>,
}

/// Play/pause, scrubber, elapsed and remaining time, mute and volume.
#[derive(Clone)]
pub struct MediaTransport {
    pub control: Arc<dyn PlaybackControl>,
    pub shows_volume: bool,
    pub is_enabled: bool,
}

impl MediaTransport {
    pub fn new(control: Arc<dyn PlaybackControl>) -> Self {
        Self {
            control,
            shows_volume: true,
            is_enabled: true,
        }
    }

    pub fn shows_volume(mut self, shows: bool) -> Self {
        self.shows_volume = shows;
        self
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }
}

/// Play when paused or ended (from the start), pause when playing.
pub fn toggle_playback(control: &dyn PlaybackControl, snapshot: &PlayerSnapshot) {
    let _ = match snapshot.state {
        PlaybackState::Playing => control.pause(),
        PlaybackState::Ended => control
            .seek(SeekRequest::exact(MediaTime(0)))
            .and_then(|()| control.play()),
        _ => control.play(),
    };
}

impl View for MediaTransport {
    fn body(&self, cx: &Scope) -> AnyView {
        let snapshot = use_playback(cx, &self.control);
        let state = TransportState {
            scrub: cx.state(|| None),
            volume: cx.state(|| None),
            restore: cx.state(|| 1.0),
        };
        let ready = self.is_enabled
            && matches!(
                snapshot.state,
                PlaybackState::Paused | PlaybackState::Playing | PlaybackState::Ended
            );
        let playing = snapshot.state == PlaybackState::Playing;

        let toggle_control = self.control.clone();
        let toggle_snapshot = snapshot.clone();
        let label = if playing { "Pause" } else { "Play" };
        let play = Button::icon_with_options(
            ButtonIconSource::resource(if playing {
                editor::pause()
            } else {
                editor::play()
            }),
            Some(14.0),
            None,
            ButtonRole::Normal,
            ready,
            snapshot.state == PlaybackState::Opening,
            false,
            Some(label.to_owned()),
            move || toggle_playback(toggle_control.as_ref(), &toggle_snapshot),
        )
        .button_style(ghost())
        .control_size(ControlSize::Small)
        .accessibility_label(label);

        let duration = snapshot.duration;
        let seekable = snapshot.seekable_range.is_some() && duration.is_some_and(|d| d.0 > 0);
        let position = snapshot.position.unwrap_or_default().seconds();
        let shown = match state.scrub.get() {
            Some((seconds, None)) => seconds,
            Some((seconds, Some(released)))
                if snapshot.seeking || released.elapsed() < Duration::from_millis(300) =>
            {
                seconds
            }
            _ => position,
        };
        let total = duration.map_or(1.0, MediaTime::seconds).max(f64::EPSILON);
        let scrub_control = self.control.clone();
        let scrub = state.scrub;
        let binding = Binding::new(
            move || shown.min(total),
            move |seconds: f64| {
                scrub.set(Some((seconds, None)));
                if let Ok(time) = MediaTime::from_seconds(seconds) {
                    let _ = scrub_control.seek(SeekRequest::keyframe(time));
                }
            },
        );
        let release_control = self.control.clone();
        let scrubber = Slider::new(binding)
            .range((0.0, total))
            .enabled(ready && seekable)
            .on_editing_changed(move |editing| {
                if editing {
                    return;
                }
                let Some((seconds, _)) = scrub.get_untracked() else {
                    return;
                };
                scrub.set(Some((seconds, Some(Instant::now()))));
                if let Ok(time) = MediaTime::from_seconds(seconds) {
                    let _ = release_control.seek(SeekRequest::exact(time));
                }
            });

        let shown_time = MediaTime::from_seconds(shown).ok();
        let elapsed = time_label(format_media_time(shown_time, duration));
        let remaining = time_label(format_media_time(duration, duration));

        // The thumb overhangs the track ends; the padding keeps it off the labels.
        let mut items = vec![
            AnyView::new(play),
            elapsed,
            AnyView::new(
                Row::new(
                    VerticalAlignment::Center,
                    0.0,
                    AnyView::new(scrubber.flex(1.0)),
                )
                .padding_edges(12.0, 0.0)
                .flex(1.0),
            ),
            remaining,
        ];
        if self.shows_volume {
            items.push(self.volume_controls(&snapshot, state));
        }
        AnyView::new(Row::new(
            VerticalAlignment::Center,
            8.0,
            AnyView::new(TupleView::new(items)),
        ))
    }
}

impl MediaTransport {
    fn volume_controls(&self, snapshot: &PlayerSnapshot, state: TransportState) -> AnyView {
        let volume = state.volume.get().unwrap_or(snapshot.volume);
        let muted = volume <= 0.0;
        let mute_control = self.control.clone();
        let local = state.volume;
        let restore = state.restore;
        let label = if muted { "Unmute" } else { "Mute" };
        let mute = Button::icon_with_options(
            ButtonIconSource::resource(if muted {
                media::volume_x()
            } else {
                media::volume()
            }),
            Some(14.0),
            None,
            ButtonRole::Normal,
            self.is_enabled,
            false,
            false,
            Some(label.to_owned()),
            move || {
                let next = if muted {
                    restore.get_untracked().max(0.05)
                } else {
                    restore.set(volume);
                    0.0
                };
                local.set(Some(next));
                let _ = mute_control.set_volume(next);
            },
        )
        .button_style(ghost())
        .control_size(ControlSize::Small)
        .accessibility_label(label);

        let volume_control = self.control.clone();
        let slider = Slider::new(Binding::new(
            move || volume,
            move |value: f64| {
                let value = value.clamp(0.0, 1.0);
                local.set(Some(value));
                let _ = volume_control.set_volume(value);
            },
        ))
        .range((0.0, 1.0))
        .enabled(self.is_enabled)
        .accessibility_label("Volume")
        .frame_width(72.0);
        AnyView::new(Row::new(
            VerticalAlignment::Center,
            2.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(mute),
                AnyView::new(slider),
            ])),
        ))
    }
}

fn time_label(text: String) -> AnyView {
    AnyView::new(
        Text::new(text)
            .semantic_font(SemanticFontRef::mono())
            .semantic_foreground(SemanticColorRef::on_surface_muted())
            .frame_min_width(40.0)
            .shrink(0.0),
    )
}

/// A column whose children span its width.
fn stretched(spacing: f32, items: Vec<AnyView>) -> BoxView {
    BoxView::new(
        FlexDirection::Column,
        Align::Stretch,
        Justify::FlexStart,
        Wrap::NoWrap,
        spacing,
        AnyView::new(TupleView::new(items)),
    )
}

/// A compact audio player: title, live level meter and transport.
#[derive(Clone)]
pub struct AudioPlayer {
    pub control: Arc<dyn PlaybackControl>,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub shows_meter: bool,
}

impl AudioPlayer {
    pub fn new(control: Arc<dyn PlaybackControl>) -> Self {
        Self {
            control,
            title: None,
            subtitle: None,
            shows_meter: true,
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    pub fn shows_meter(mut self, shows: bool) -> Self {
        self.shows_meter = shows;
        self
    }
}

impl View for AudioPlayer {
    fn body(&self, cx: &Scope) -> AnyView {
        let snapshot = use_playback(cx, &self.control);
        let mut heading: Vec<AnyView> = Vec::new();
        if let Some(title) = &self.title {
            heading.push(AnyView::new(
                Text::new(title.clone())
                    .line_limit(Some(1))
                    .semantic_font(SemanticFontRef::body_strong()),
            ));
        }
        if let Some(subtitle) = &self.subtitle {
            heading.push(AnyView::new(
                Text::new(subtitle.clone())
                    .line_limit(Some(1))
                    .semantic_font(SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
            ));
        }
        let mut header: Vec<AnyView> = Vec::new();
        if !heading.is_empty() {
            header.push(AnyView::new(
                Column::new(
                    HorizontalAlignment::Leading,
                    2.0,
                    AnyView::new(TupleView::new(heading)),
                )
                .flex(1.0),
            ));
        }
        if self.shows_meter
            && let Some(meter) = self.control.audio_meter()
        {
            header.push(AnyView::new(
                AudioLevelMeter::new(meter)
                    .active(snapshot.state == PlaybackState::Playing)
                    .frame_width(96.0),
            ));
        }
        let mut rows: Vec<AnyView> = Vec::new();
        if !header.is_empty() {
            rows.push(AnyView::new(Row::new(
                VerticalAlignment::Center,
                12.0,
                AnyView::new(TupleView::new(header)),
            )));
        }
        rows.push(AnyView::new(MediaTransport::new(self.control.clone())));
        AnyView::new(
            stretched(8.0, rows)
                .padding(12.0)
                .semantic_background(SemanticColorRef::surface_raised())
                .semantic_border(SemanticColorRef::border(), 1.0)
                .corner_radius(10.0),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_switch_to_hours_with_the_scale() {
        let time = MediaTime(83_000_000_000);
        assert_eq!(format_media_time(Some(time), None), "1:23");
        assert_eq!(
            format_media_time(Some(time), Some(MediaTime(4_000_000_000_000))),
            "0:01:23"
        );
        assert_eq!(format_media_time(None, None), "--:--");
    }
}
