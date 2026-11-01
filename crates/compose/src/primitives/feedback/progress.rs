//! Translation of Primitives/Feedback/Progress.swift.
use crate::impl_primitive_view;
use crate::primitives::feedback::{ProgressValue, StatusTone};
use crate::primitives::media::LoadingAnimationClock;
use crate::{PrimitiveView, View};
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, AccessibilityState, LayoutNode, MeasureMode, Node,
    NodeResource, Rect,
};
use std::rc::Rc;

/// Determinate or indeterminate linear progress.
pub struct ProgressView {
    /// Fraction completed in `0...1`. `None` renders the indeterminate sweep.
    pub value: Option<f64>,
    pub tone: StatusTone,
    /// Explicit track thickness. `None` follows the shared control size.
    pub height: Option<f32>,
    pub is_enabled: bool,
}

#[derive(Clone, PartialEq)]
struct PaintIdentity {
    value: Option<f64>,
    tone: StatusTone,
    height: f32,
    is_enabled: bool,
    indeterminate: bool,
}

impl ProgressView {
    pub fn new(value: Option<f64>) -> Self {
        Self {
            value,
            tone: StatusTone::Info,
            height: None,
            is_enabled: true,
        }
    }

    pub fn tone(mut self, tone: StatusTone) -> Self {
        self.tone = tone;
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    /// `value` forced into `0...1`; `None` and non-finite values read as `0`
    /// because drawing a 250%-wide fill is never the honest answer.
    pub fn clamped(&self) -> f64 {
        match self.value {
            Some(value) if value.is_finite() => value.clamp(0.0, 1.0),
            _ => 0.0,
        }
    }

    fn is_indeterminate(&self) -> bool {
        self.value.is_none_or(|value| !value.is_finite())
    }

    fn resolved_height(&self, node: &Node) -> f32 {
        if let Some(height) = self.height
            && height.is_finite()
        {
            return height.clamp(2.0, 64.0);
        }
        match node.composition_value(&crate::theme::ControlSizeEnvironment::key()) {
            crate::theme::ControlSize::Mini => 3.0,
            crate::theme::ControlSize::Small => 4.0,
            crate::theme::ControlSize::Regular => 6.0,
            crate::theme::ControlSize::Large => 8.0,
        }
    }
}

impl PrimitiveView for ProgressView {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        let clock: Rc<dyn NodeResource> = LoadingAnimationClock::new();
        node.add_resource(clock);
        let value: Rc<dyn NodeResource> = ProgressValue::new();
        node.add_resource(value);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_node(&self, node: &Node) {
        node.set_is_focusable(false);
        let resolved = self.clamped();
        let indeterminate = self.is_indeterminate();
        let percent = (resolved * 100.0).round() as i64;
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Progress,
            label: "Progress".to_string(),
            value: if indeterminate {
                "In progress".to_string()
            } else {
                format!("{percent}%")
            },
            state: AccessibilityState {
                is_enabled: self.is_enabled,
                ..Default::default()
            },
            ..Default::default()
        }));

        let height = self.resolved_height(node);
        if let Some(layout) = node.layout_node() {
            layout.set_height(Some(height));
            // A fixed extent is enough: the track is never sized by content and
            // a measured node would reject overlapping children.
            layout.set_measure_func(Some(Rc::new(
                move |width, width_mode, _height, _height_mode| {
                    let width = if width_mode == MeasureMode::Undefined {
                        160.0
                    } else {
                        width
                    };
                    cove_core::Size { width, height }
                },
            )));
        }

        if let Some(clock) = node.first_resource::<LoadingAnimationClock>() {
            clock.configure(1.4, indeterminate && !node.prefers_reduced_motion());
        }
        if let Some(presentation) = node.first_resource::<ProgressValue>() {
            presentation.update(node, resolved, !indeterminate);
        }

        let identity = PaintIdentity {
            value: Some(resolved),
            tone: self.tone,
            height,
            is_enabled: self.is_enabled,
            indeterminate,
        };
        let weak_node = node.downgrade();
        let clock = node.first_resource::<LoadingAnimationClock>();
        let presentation = node.first_resource::<ProgressValue>();
        let tone = self.tone;
        let is_enabled = self.is_enabled;
        node.update_draw(identity, move |list, origin| {
            let Some(owner) = Node::upgrade(&weak_node) else {
                return;
            };
            let frame = owner.frame();
            let rect = Rect::new(origin.x, origin.y, frame.width, frame.height);
            if rect.width <= 0.0 || rect.height <= 0.0 {
                return;
            }
            let theme = crate::theme::theme_for_node(&owner);
            let base = tone.color().resolve(&theme);
            let alpha = owner.opacity() * if is_enabled { 1.0 } else { 0.5 };
            let radius = rect.height / 2.0;
            let _ =
                list.add_rounded_rect(rect, radius, theme.colors.border.multiplied_alpha(alpha));
            if rect.width <= rect.height {
                return;
            }
            if indeterminate {
                let Some(clock) = clock.as_ref() else { return };
                let phase = clock.phase() as f32;
                // Ping-pong with a smoothstep so the sweep eases at both ends
                // instead of snapping direction.
                let triangle = if phase < 0.5 {
                    phase * 2.0
                } else {
                    (1.0 - phase) * 2.0
                };
                let eased = triangle * triangle * (3.0 - 2.0 * triangle);
                let segment_width = rect.height.max(rect.width * 0.4);
                let x = rect.x + (rect.width - segment_width) * eased;
                let segment = Rect::new(x, rect.y, segment_width, rect.height);
                let _ = list.add_rounded_rect(segment, radius, base.multiplied_alpha(alpha));
            } else {
                let Some(presentation) = presentation.as_ref() else {
                    return;
                };
                let filled = rect.width * presentation.value() as f32;
                if filled <= 0.0 {
                    return;
                }
                let fill = Rect::new(rect.x, rect.y, filled, rect.height);
                let _ = list.add_rounded_rect(
                    fill,
                    radius.min(filled / 2.0),
                    base.multiplied_alpha(alpha),
                );
            }
        });
    }
}
impl_primitive_view!(ProgressView);
