//! The seek bar drawn across the bottom of a video, like mainstream players:
//! thin at rest, thicker with a thumb on hover, scrubbing on press and drag.

use std::rc::Rc;

use crate::{PrimitiveView, impl_primitive_view};
use cove_core::scene::{
    AccessibilityActions, AccessibilityRole, AccessibilitySemantics, AccessibilityState,
    EventResult, HoverPhase, InputHandlerRoute, InteractionRegistryHolder, InvalidationSource,
    LayoutNode, MeasureMode, Node, PointerCaptureHolder, PointerPhase, Rect,
};
use cove_core::{Color, Size};
use cove_platform::{MouseButton, SystemCursor};

const PRESSED: &str = "__media_progress_pressed";
const HOVER: &str = "__media_progress_hover";
const DRAG: &str = "__media_progress_drag";
/// Height of the hit area; the visible track is centred in it.
pub const PROGRESS_BAR_HEIGHT: f32 = 16.0;

/// `seconds` scrubbed to; `committed` is true on release.
pub type ScrubHandler = Rc<dyn Fn(f64, bool)>;
/// Seconds under the pointer, `None` when it leaves.
pub type HoverHandler = Rc<dyn Fn(Option<f64>)>;

#[derive(Clone)]
pub struct MediaProgressBar {
    pub position: f64,
    pub duration: f64,
    /// Fraction of the media already available, drawn behind the played part.
    pub buffered: Option<f64>,
    pub is_enabled: bool,
    pub played_color: Option<Color>,
    pub on_scrub: Option<ScrubHandler>,
    pub on_hover: Option<HoverHandler>,
}

impl MediaProgressBar {
    pub fn new(position: f64, duration: f64) -> Self {
        Self {
            position,
            duration,
            buffered: None,
            is_enabled: true,
            played_color: None,
            on_scrub: None,
            on_hover: None,
        }
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn buffered(mut self, fraction: Option<f64>) -> Self {
        self.buffered = fraction.filter(|value| value.is_finite());
        self
    }

    pub fn played_color(mut self, color: Color) -> Self {
        self.played_color = Some(color);
        self
    }

    pub fn on_scrub(mut self, handler: impl Fn(f64, bool) + 'static) -> Self {
        self.on_scrub = Some(Rc::new(handler));
        self
    }

    pub fn on_hover(mut self, handler: impl Fn(Option<f64>) + 'static) -> Self {
        self.on_hover = Some(Rc::new(handler));
        self
    }

    fn fraction(&self) -> f64 {
        if self.duration > 0.0 && self.position.is_finite() {
            (self.position / self.duration).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
struct PaintIdentity {
    fraction: u64,
    buffered: Option<u64>,
    enabled: bool,
    color: Option<Color>,
}

fn pointer_fraction(node: &Node, x: f32) -> Option<f64> {
    let width = node.frame().width;
    (width > 0.0 && x.is_finite())
        .then(|| ((x - node.absolute_origin().x) / width).clamp(0.0, 1.0) as f64)
}

fn repaint(node: &Node) {
    node.mark_render_dirty(InvalidationSource::style_set("media.progress"));
}

impl PrimitiveView for MediaProgressBar {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_height(Some(PROGRESS_BAR_HEIGHT));
        layout.set_measure_func(Some(Rc::new(|width, mode, _, _| Size {
            width: if mode == MeasureMode::Undefined {
                240.0
            } else {
                width
            },
            height: PROGRESS_BAR_HEIGHT,
        })));
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        let fraction = self.fraction();
        let enabled = self.is_enabled && self.duration > 0.0;
        node.set_cursor(Some(if enabled {
            SystemCursor::Pointer
        } else {
            SystemCursor::Arrow
        }));
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Slider,
            label: crate::i18n::tr("Seek"),
            value: super::format_media_time(
                cove_media::MediaTime::from_seconds(self.position.max(0.0)).ok(),
                cove_media::MediaTime::from_seconds(self.duration.max(0.0)).ok(),
            ),
            state: AccessibilityState {
                is_enabled: enabled,
                ..Default::default()
            },
            ..Default::default()
        }));
        if let Some(scrub) = self.on_scrub.clone().filter(|_| enabled) {
            let (position, duration) = (self.position, self.duration);
            let (forward, backward) = (scrub.clone(), scrub);
            node.set_accessibility_actions(AccessibilityActions {
                increment: Some(Rc::new(move || {
                    forward((position + 5.0).min(duration), true)
                })),
                decrement: Some(Rc::new(move || backward((position - 5.0).max(0.0), true))),
                ..Default::default()
            });
        }

        let identity = PaintIdentity {
            fraction: fraction.to_bits(),
            buffered: self.buffered.map(f64::to_bits),
            enabled,
            color: self.played_color,
        };
        let weak = node.downgrade();
        let buffered = self.buffered;
        let played_color = self.played_color;
        node.update_draw(identity, move |list, origin| {
            let Some(owner) = Node::upgrade(&weak) else {
                return;
            };
            let frame = owner.frame();
            if frame.width <= 0.0 {
                return;
            }
            let theme = crate::theme::theme_for_node(&owner);
            let opacity = owner.opacity();
            let pressed = owner.attachment::<bool>(PRESSED) == Some(true);
            let hover = owner.attachment::<Option<f64>>(HOVER).flatten();
            let active = enabled && (pressed || hover.is_some());
            let shown = owner
                .attachment::<Option<f64>>(DRAG)
                .flatten()
                .filter(|_| pressed)
                .unwrap_or(fraction) as f32;
            let thickness = if active { 5.0 } else { 3.0 };
            let y = origin.y + (frame.height - thickness) / 2.0;
            let radius = thickness / 2.0;
            let track = Rect::new(origin.x, y, frame.width, thickness);
            let white = |alpha: f32| Color::WHITE.multiplied_alpha(alpha * opacity);
            let _ = list.add_rounded_rect(track, radius, white(0.28));
            if let Some(buffered) = buffered {
                let width = frame.width * buffered.clamp(0.0, 1.0) as f32;
                let _ = list.add_rounded_rect(
                    Rect::new(origin.x, y, width, thickness),
                    radius,
                    white(0.38),
                );
            }
            if let Some(hover) = hover.filter(|_| enabled) {
                let width = frame.width * hover as f32;
                let _ = list.add_rounded_rect(
                    Rect::new(origin.x, y, width, thickness),
                    radius,
                    white(0.45),
                );
            }
            let color = played_color
                .unwrap_or(theme.colors.accent)
                .multiplied_alpha(opacity);
            let played = frame.width * shown;
            if played > 0.0 {
                let _ = list.add_rounded_rect(
                    Rect::new(origin.x, y, played, thickness),
                    radius.min(played / 2.0),
                    color,
                );
            }
            if active {
                let diameter = 13.0;
                let center_y = origin.y + frame.height / 2.0;
                let thumb = Rect::new(
                    origin.x + played - diameter / 2.0,
                    center_y - diameter / 2.0,
                    diameter,
                    diameter,
                );
                let _ = list.add_rounded_rect(thumb, diameter / 2.0, color);
            }
        });

        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        if !enabled {
            registry.remove(node);
            return;
        }
        let duration = self.duration;
        let hover_handler = self.on_hover.clone();
        let hover_node = node.downgrade();
        let hover_out = hover_handler.clone();
        registry.set_hover(
            node,
            Rc::new(move |phase| {
                let Some(owner) = Node::upgrade(&hover_node) else {
                    return;
                };
                if phase == HoverPhase::Leave {
                    owner.set_attachment::<Option<f64>>(HOVER, None);
                    if let Some(handler) = hover_out.as_ref() {
                        handler(None);
                    }
                    repaint(&owner);
                }
            }),
        );
        let scrub = self.on_scrub.clone();
        let press_scrub = scrub.clone();
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
                let fraction = pointer_fraction(&owner, event.x);
                match phase {
                    PointerPhase::Down => {
                        if let Some(capture) = PointerCaptureHolder::current() {
                            capture.acquire(&owner, false);
                        }
                        owner.set_attachment(PRESSED, true);
                        owner.set_attachment(DRAG, fraction);
                        if let (Some(handler), Some(fraction)) = (press_scrub.as_ref(), fraction) {
                            handler(fraction * duration, false);
                        }
                    }
                    PointerPhase::Up => {
                        if let Some(capture) = PointerCaptureHolder::current()
                            && capture.target().is_some_and(|target| target == owner)
                        {
                            capture.release();
                        }
                        let was_pressed = owner.attachment::<bool>(PRESSED) == Some(true);
                        owner.set_attachment(PRESSED, false);
                        if was_pressed
                            && let (Some(handler), Some(fraction)) =
                                (press_scrub.as_ref(), fraction)
                        {
                            handler(fraction * duration, true);
                        }
                    }
                }
                repaint(&owner);
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
                let fraction = pointer_fraction(&owner, event.x);
                owner.set_attachment(HOVER, fraction);
                if let Some(handler) = hover_handler.as_ref() {
                    handler(fraction.map(|fraction| fraction * duration));
                }
                if owner.attachment::<bool>(PRESSED) == Some(true) {
                    owner.set_attachment(DRAG, fraction);
                    if let (Some(handler), Some(fraction)) = (scrub.as_ref(), fraction) {
                        handler(fraction * duration, false);
                    }
                }
                repaint(&owner);
                EventResult::Ignored
            }),
        );
    }
}
impl_primitive_view!(MediaProgressBar);
