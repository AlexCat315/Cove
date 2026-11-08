//! Live per-channel level bars fed by a `cove_media::AudioMeter`.
//!
//! While active, a node-owned animation clock repaints every display frame and
//! the draw reads the meter directly; levels rise instantly and fall at a fixed
//! rate so short transients stay visible.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

use crate::primitives::media::LoadingAnimationClock;
use crate::theme::SemanticColorRef;
use crate::{PrimitiveView, impl_primitive_view};
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, LayoutNode, MeasureMode, Node, NodeResource, Rect,
};
use cove_media::AudioMeter;

/// Decibels below full scale shown at the left edge of a bar.
const FLOOR_DB: f32 = -60.0;
/// Fall rate of the displayed level, in dB per second.
const FALL_DB_PER_SECOND: f32 = 30.0;

/// Horizontal level bars, one per channel.
#[derive(Clone)]
pub struct AudioLevelMeter {
    pub meter: AudioMeter,
    /// Repaint every frame. Inactive meters fall to silence and stop the clock.
    pub is_active: bool,
    pub bar_thickness: f32,
}

impl AudioLevelMeter {
    pub fn new(meter: AudioMeter) -> Self {
        Self {
            meter,
            is_active: true,
            bar_thickness: 4.0,
        }
    }

    pub fn active(mut self, is_active: bool) -> Self {
        self.is_active = is_active;
        self
    }

    pub fn bar_thickness(mut self, thickness: f32) -> Self {
        if thickness.is_finite() && thickness > 0.0 {
            self.bar_thickness = thickness.min(24.0);
        }
        self
    }
}

/// `0..=1` position of a linear amplitude on the meter's dB scale.
pub fn meter_fraction(amplitude: f32) -> f32 {
    if !amplitude.is_finite() || amplitude <= 0.0 {
        return 0.0;
    }
    ((20.0 * amplitude.log10() - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0)
}

/// Displayed levels with peak-hold ballistics.
#[derive(Default)]
struct Ballistics {
    shown: RefCell<Vec<f32>>,
    last: Cell<Option<Instant>>,
}

impl NodeResource for Ballistics {
    fn mount(&self, _node: &Node) {}
    fn unmount(&self, _node: &Node) {
        self.shown.borrow_mut().clear();
        self.last.set(None);
    }
    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}

impl Ballistics {
    fn advance(&self, targets: &[f32]) -> Vec<f32> {
        let now = Instant::now();
        let elapsed = self
            .last
            .replace(Some(now))
            .map_or(0.0, |last| now.duration_since(last).as_secs_f32());
        let fall = FALL_DB_PER_SECOND / -FLOOR_DB * elapsed;
        let mut shown = self.shown.borrow_mut();
        shown.resize(targets.len(), 0.0);
        for (value, &target) in shown.iter_mut().zip(targets) {
            *value = target.max(*value - fall).clamp(0.0, 1.0);
        }
        shown.clone()
    }
}

#[derive(Clone, PartialEq)]
struct PaintIdentity {
    active: bool,
    thickness: u32,
}

impl PrimitiveView for AudioLevelMeter {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        let clock: Rc<dyn NodeResource> = LoadingAnimationClock::new();
        node.add_resource(clock);
        let ballistics: Rc<dyn NodeResource> = Rc::new(Ballistics::default());
        node.add_resource(ballistics);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_node(&self, node: &Node) {
        let level = meter_fraction(self.meter.levels().max_peak());
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Progress,
            label: crate::i18n::tr("Audio level"),
            value: format!("{}%", (level * 100.0).round() as i64),
            ..Default::default()
        }));
        let thickness = self.bar_thickness;
        let channels = self.meter.levels().peak.len().clamp(1, 8) as f32;
        if let Some(layout) = node.layout_node() {
            let height = channels * thickness + (channels - 1.0) * 2.0;
            layout.set_measure_func(Some(Rc::new(move |width, width_mode, _, _| {
                cove_core::Size {
                    width: if width_mode == MeasureMode::Undefined {
                        96.0
                    } else {
                        width
                    },
                    height,
                }
            })));
        }
        if let Some(clock) = node.first_resource::<LoadingAnimationClock>() {
            clock.configure(1.0, self.is_active && !node.prefers_reduced_motion());
        }
        let meter = self.meter.clone();
        let weak = node.downgrade();
        let active = self.is_active;
        node.update_draw(
            PaintIdentity {
                active,
                thickness: thickness.to_bits(),
            },
            move |list, origin| {
                let Some(owner) = Node::upgrade(&weak) else {
                    return;
                };
                let frame = owner.frame();
                if frame.width <= 0.0 {
                    return;
                }
                let levels = meter.levels();
                let targets: Vec<f32> = if active {
                    levels
                        .peak
                        .iter()
                        .map(|&peak| meter_fraction(peak))
                        .collect()
                } else {
                    vec![0.0; levels.peak.len().max(1)]
                };
                // Inactive meters have no clock to animate a fall, so they drop at once.
                let shown = match owner.first_resource::<Ballistics>() {
                    Some(ballistics) if active => ballistics.advance(&targets),
                    Some(ballistics) => {
                        ballistics.shown.borrow_mut().clear();
                        ballistics.last.set(None);
                        targets.clone()
                    }
                    None => targets.clone(),
                };
                let theme = crate::theme::theme_for_node(&owner);
                let opacity = owner.opacity();
                let track = theme.colors.surface_pressed.multiplied_alpha(opacity);
                let low = SemanticColorRef::success()
                    .resolve(&theme)
                    .multiplied_alpha(opacity);
                let mid = SemanticColorRef::warning()
                    .resolve(&theme)
                    .multiplied_alpha(opacity);
                let high = SemanticColorRef::error()
                    .resolve(&theme)
                    .multiplied_alpha(opacity);
                let radius = thickness / 2.0;
                for (row, &value) in shown.iter().take(8).enumerate() {
                    let y = origin.y + row as f32 * (thickness + 2.0);
                    let bar = Rect::new(origin.x, y, frame.width, thickness);
                    let _ = list.add_rounded_rect(bar, radius, track);
                    let filled = frame.width * value;
                    if filled <= 0.5 {
                        continue;
                    }
                    let color = if value > meter_fraction(0.7) {
                        high
                    } else if value > meter_fraction(0.25) {
                        mid
                    } else {
                        low
                    };
                    let fill = Rect::new(origin.x, y, filled, thickness);
                    let _ = list.add_rounded_rect(fill, radius.min(filled / 2.0), color);
                }
            },
        );
    }
}
impl_primitive_view!(AudioLevelMeter);

#[cfg(test)]
mod tests {
    use super::meter_fraction;

    #[test]
    fn decibel_scale_spans_sixty_db() {
        assert_eq!(meter_fraction(0.0), 0.0);
        assert_eq!(meter_fraction(1.0), 1.0);
        assert!((meter_fraction(0.001) - 0.0).abs() < 1e-6);
        assert!((meter_fraction(0.0316) - 0.5).abs() < 0.01);
    }
}
