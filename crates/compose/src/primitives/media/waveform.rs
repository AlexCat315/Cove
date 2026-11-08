//! Static waveform with playback progress and pointer seeking.
//!
//! Peaks usually come from `MediaDecoder::read_audio` followed by
//! `AudioBuffer::peaks`; the view resamples them to one bar per column.

use std::rc::Rc;
use std::sync::Arc;

use crate::theme::SemanticColorRef;
use crate::{PrimitiveView, impl_primitive_view};
use cove_core::scene::{
    AccessibilityActions, AccessibilityRole, AccessibilitySemantics, AccessibilityState,
    EventResult, InputHandlerRoute, InteractionRegistryHolder, LayoutNode, MeasureMode, Node,
    PointerCaptureHolder, PointerPhase, Rect,
};
use cove_media::AudioPeak;
use cove_platform::{MouseButton, SystemCursor};

const DRAGGING: &str = "__waveform_dragging";

/// Bars of an audio envelope, filled up to `progress`.
#[derive(Clone)]
pub struct Waveform {
    pub peaks: Arc<[AudioPeak]>,
    /// Played fraction in `0..=1`.
    pub progress: f64,
    pub bar_width: f32,
    pub bar_gap: f32,
    pub height: f32,
    /// Scale the loudest column to the full height.
    pub normalized: bool,
    pub on_seek: Option<Rc<dyn Fn(f64)>>,
}

impl Waveform {
    pub fn new(peaks: impl Into<Arc<[AudioPeak]>>) -> Self {
        Self {
            peaks: peaks.into(),
            progress: 0.0,
            bar_width: 2.0,
            bar_gap: 1.0,
            height: 48.0,
            normalized: true,
            on_seek: None,
        }
    }

    pub fn progress(mut self, progress: f64) -> Self {
        self.progress = if progress.is_finite() {
            progress.clamp(0.0, 1.0)
        } else {
            0.0
        };
        self
    }

    pub fn bars(mut self, width: f32, gap: f32) -> Self {
        if width.is_finite() && width > 0.0 {
            self.bar_width = width;
        }
        if gap.is_finite() && gap >= 0.0 {
            self.bar_gap = gap;
        }
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        if height.is_finite() && height > 0.0 {
            self.height = height;
        }
        self
    }

    pub fn normalized(mut self, normalized: bool) -> Self {
        self.normalized = normalized;
        self
    }

    /// Called with the fraction under the pointer on press and while dragging.
    pub fn on_seek(mut self, action: impl Fn(f64) + 'static) -> Self {
        self.on_seek = Some(Rc::new(action));
        self
    }
}

/// The largest envelope among the peaks that fall into each of `bars` columns.
pub fn resample_peaks(peaks: &[AudioPeak], bars: usize) -> Vec<AudioPeak> {
    if peaks.is_empty() || bars == 0 {
        return Vec::new();
    }
    (0..bars)
        .map(|bar| {
            let start = bar * peaks.len() / bars;
            let end = ((bar + 1) * peaks.len() / bars)
                .max(start + 1)
                .min(peaks.len());
            peaks[start..end]
                .iter()
                .fold(AudioPeak::default(), |acc, peak| AudioPeak {
                    min: acc.min.min(peak.min),
                    max: acc.max.max(peak.max),
                    rms: acc.rms.max(peak.rms),
                })
        })
        .collect()
}

#[derive(Clone, PartialEq)]
struct PaintIdentity {
    peaks: usize,
    count: usize,
    progress: u64,
    bar: (u32, u32),
    normalized: bool,
}

fn seek_fraction(node: &Node, x: f32) -> Option<f64> {
    let width = node.frame().width;
    if width <= 0.0 || !x.is_finite() {
        return None;
    }
    Some(((x - node.absolute_origin().x) / width).clamp(0.0, 1.0) as f64)
}

impl PrimitiveView for Waveform {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_node(&self, node: &Node) {
        let seekable = self.on_seek.is_some();
        node.set_is_hit_testable(seekable);
        node.set_cursor(seekable.then_some(SystemCursor::Pointer));
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Slider,
            label: crate::i18n::tr("Waveform"),
            value: format!("{}%", (self.progress * 100.0).round() as i64),
            state: AccessibilityState {
                is_enabled: seekable,
                ..Default::default()
            },
            ..Default::default()
        }));
        if let Some(seek) = self.on_seek.clone() {
            let progress = self.progress;
            let (increment, decrement) = (seek.clone(), seek);
            node.set_accessibility_actions(AccessibilityActions {
                increment: Some(Rc::new(move || increment((progress + 0.05).min(1.0)))),
                decrement: Some(Rc::new(move || decrement((progress - 0.05).max(0.0)))),
                ..Default::default()
            });
        }
        let height = self.height;
        if let Some(layout) = node.layout_node() {
            layout.set_measure_func(Some(Rc::new(move |width, width_mode, _, _| {
                cove_core::Size {
                    width: if width_mode == MeasureMode::Undefined {
                        240.0
                    } else {
                        width
                    },
                    height,
                }
            })));
        }

        let peaks = self.peaks.clone();
        let progress = self.progress as f32;
        let (bar_width, gap) = (self.bar_width, self.bar_gap);
        let gain = if self.normalized {
            let loudest = self
                .peaks
                .iter()
                .map(|peak| peak.max.abs().max(peak.min.abs()))
                .fold(0.0f32, f32::max);
            if loudest > 1e-4 { 1.0 / loudest } else { 1.0 }
        } else {
            1.0
        };
        let weak = node.downgrade();
        node.update_draw(
            PaintIdentity {
                peaks: Arc::as_ptr(&self.peaks) as *const AudioPeak as usize,
                count: self.peaks.len(),
                progress: self.progress.to_bits(),
                bar: (bar_width.to_bits(), gap.to_bits()),
                normalized: self.normalized,
            },
            move |list, origin| {
                let Some(owner) = Node::upgrade(&weak) else {
                    return;
                };
                let frame = owner.frame();
                if frame.width <= 0.0 || frame.height <= 0.0 {
                    return;
                }
                let theme = crate::theme::theme_for_node(&owner);
                let opacity = owner.opacity();
                let played = SemanticColorRef::accent()
                    .resolve(&theme)
                    .multiplied_alpha(opacity);
                let pending = SemanticColorRef::on_surface_muted()
                    .resolve(&theme)
                    .multiplied_alpha(opacity * 0.5);
                let step = bar_width + gap;
                let count = ((frame.width + gap) / step).floor().max(1.0) as usize;
                let columns = resample_peaks(&peaks, count);
                let middle = origin.y + frame.height / 2.0;
                let half = frame.height / 2.0 * gain;
                let split = origin.x + frame.width * progress;
                for (index, peak) in columns.iter().enumerate() {
                    let x = origin.x + index as f32 * step;
                    let top = middle - peak.max.max(0.0) * half;
                    let bottom = middle - peak.min.min(0.0) * half;
                    let extent = (bottom - top).max(1.0);
                    let color = if x + bar_width / 2.0 <= split {
                        played
                    } else {
                        pending
                    };
                    let bar = Rect::new(x, top, bar_width, extent);
                    let _ = list.add_rounded_rect(bar, (bar_width / 2.0).min(extent / 2.0), color);
                }
            },
        );

        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        let Some(seek) = self.on_seek.clone() else {
            registry.remove(node);
            return;
        };
        let press_seek = seek.clone();
        let press_node = node.downgrade();
        registry.set_pointer(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase, _| {
                if event.button != MouseButton::Left {
                    return EventResult::Ignored;
                }
                let Some(owner) = Node::upgrade(&press_node) else {
                    return EventResult::Ignored;
                };
                match phase {
                    PointerPhase::Down => {
                        if let Some(capture) = PointerCaptureHolder::current() {
                            capture.acquire(&owner, false);
                        }
                        owner.set_attachment(DRAGGING, true);
                        if let Some(fraction) = seek_fraction(&owner, event.x) {
                            press_seek(fraction);
                        }
                    }
                    PointerPhase::Up => {
                        owner.set_attachment(DRAGGING, false);
                        if let Some(capture) = PointerCaptureHolder::current()
                            && capture.target().is_some_and(|target| target == owner)
                        {
                            capture.release();
                        }
                    }
                }
                EventResult::Handled
            }),
        );
        let motion_node = node.downgrade();
        registry.set_motion(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, _| {
                let Some(owner) = Node::upgrade(&motion_node) else {
                    return EventResult::Ignored;
                };
                if owner.attachment::<bool>(DRAGGING) != Some(true) {
                    return EventResult::Ignored;
                }
                if let Some(fraction) = seek_fraction(&owner, event.x) {
                    seek(fraction);
                }
                EventResult::Handled
            }),
        );
    }
}
impl_primitive_view!(Waveform);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resampling_keeps_the_loudest_envelope() {
        let peaks: Vec<AudioPeak> = (0..10)
            .map(|i| AudioPeak {
                min: -(i as f32) / 10.0,
                max: i as f32 / 10.0,
                rms: 0.0,
            })
            .collect();
        let bars = resample_peaks(&peaks, 2);
        assert_eq!(bars.len(), 2);
        assert_eq!(bars[0].max, 0.4);
        assert_eq!(bars[1].min, -0.9);
        assert_eq!(resample_peaks(&peaks, 20).len(), 20);
    }
}
