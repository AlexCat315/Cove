//! Translation of Primitives/Interaction/Scrollbar.swift.
//!
//! Focusable scroll thumb for a caller-owned offset. It is a companion to a
//! scroll view: bind the same offset and place it beside the viewport. The
//! component paints only the track and thumb; the caller owns layout.
use std::rc::Rc;

use crate::{PrimitiveView, impl_primitive_view};
use cove_core::scene::{
    AccessibilityActions, AccessibilityRole, AccessibilitySemantics, EventPhase, EventResult,
    FocusChainHolder, HoverPhase, InputHandlerRoute, InteractionRegistryHolder, LayoutNode, Node,
    PointerCaptureHolder, PointerPhase,
};
use cove_core::{Binding, Point, Rect};
use cove_platform::MouseButton;

const CHROME_OPACITY: &str = "__scrollbar_chrome_opacity";
const IS_HOVERED: &str = "__scrollbar_is_hovered";
const IS_DRAGGING: &str = "__scrollbar_is_dragging";
const DESIRED_VISIBLE: &str = "__scrollbar_desired_visible";
const LAST_OFFSET: &str = "__scrollbar_last_offset";
const AXIS: &str = "__scrollbar_axis";
const THICKNESS: &str = "__scrollbar_thickness";
const CONTENT: &str = "__scrollbar_content";
const VIEWPORT: &str = "__scrollbar_viewport";
const MINIMUM: &str = "__scrollbar_minimum";
const MAX_OFFSET: &str = "__scrollbar_max_offset";
const VALUE: &str = "__scrollbar_value";
const ALWAYS_VISIBLE: &str = "__scrollbar_always_visible";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScrollbarAxis {
    Vertical,
    Horizontal,
}

#[derive(Clone, Copy, PartialEq)]
struct ScrollbarPaint {
    axis: ScrollbarAxis,
    offset_bits: u32,
    content_bits: u32,
    viewport_bits: u32,
    thickness_bits: u32,
    minimum_bits: u32,
    focused: bool,
    visible_focus: bool,
}

/// Focusable scroll thumb for a caller-owned offset.
pub struct Scrollbar {
    pub axis: ScrollbarAxis,
    pub offset: Binding<f32>,
    pub content_extent: f32,
    pub viewport_extent: f32,
    pub thickness: f32,
    pub minimum_thumb_extent: f32,
    pub label: String,
    /// Keeps the bar shown at rest, like macOS "Show scroll bars: Always".
    pub always_visible: bool,
}

impl Scrollbar {
    pub fn new(
        axis: ScrollbarAxis,
        offset: Binding<f32>,
        content_extent: f32,
        viewport_extent: f32,
    ) -> Self {
        Self {
            axis,
            offset,
            content_extent: content_extent.max(0.0),
            viewport_extent: viewport_extent.max(0.0),
            thickness: 12.0,
            minimum_thumb_extent: 32.0,
            label: crate::i18n::tr("Scrollbar"),
            always_visible: false,
        }
    }

    pub fn always_visible(mut self, visible: bool) -> Self {
        self.always_visible = visible;
        self
    }

    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = if thickness.is_finite() {
            thickness.max(8.0)
        } else {
            12.0
        };
        self
    }

    pub fn minimum_thumb_extent(mut self, extent: f32) -> Self {
        self.minimum_thumb_extent = if extent.is_finite() {
            extent.max(16.0)
        } else {
            32.0
        };
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    fn max_offset(&self) -> f32 {
        (self.content_extent - self.viewport_extent).max(0.0)
    }

    fn value(&self) -> f32 {
        self.offset.get().clamp(0.0, self.max_offset())
    }

    fn thumb_extent(&self, track: f32) -> f32 {
        if self.content_extent <= self.viewport_extent || self.content_extent <= 0.0 {
            return track;
        }
        let proportional = track * self.viewport_extent / self.content_extent;
        track.min(self.minimum_thumb_extent.max(proportional))
    }
}

fn apply_desired_visibility(node: &Node) {
    if !crate::primitives::controls::adaptive::supports_control_hover(node)
        || node.attachment::<bool>(ALWAYS_VISIBLE) == Some(true)
    {
        return;
    }
    let hovered = node.attachment::<bool>(IS_HOVERED).unwrap_or(false);
    let dragging = node.attachment::<bool>(IS_DRAGGING).unwrap_or(false);
    let focused = FocusChainHolder::current()
        .and_then(|chain| chain.focused())
        .is_some_and(|focused| focused == *node);
    let desired = hovered || dragging || focused;
    let previous = node.attachment::<bool>(DESIRED_VISIBLE).unwrap_or(false);
    if desired == previous {
        return;
    }
    node.set_attachment(DESIRED_VISIBLE, desired);
    set_chrome_visible(node, desired);
}

fn set_chrome_visible(node: &Node, visible: bool) {
    let current = node.attachment::<f32>(CHROME_OPACITY).unwrap_or(0.0);
    let weak = node.downgrade();
    cove_core::scene::with_animation(
        Some(crate::theme::SemanticMotionRef::fast().resolve(&crate::theme::theme_for_node(node))),
        || {
            node.animatable_set_with(
                CHROME_OPACITY,
                current,
                if visible { 1.0 } else { 0.0 },
                move |opacity| {
                    if let Some(owner) = Node::upgrade(&weak) {
                        owner.set_attachment(CHROME_OPACITY, opacity);
                        owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                            "scrollbarChromeOpacity",
                        ));
                    }
                },
            );
        },
    );
}

/// Scrolling reveals the bar even when nothing holds it: it appears at once and
/// fades 0.3s after the content rests for 0.7s. Reduced motion keeps it visible
/// instead of animating it out.
fn flash_if_unheld(node: &Node) {
    if !crate::primitives::controls::adaptive::supports_control_hover(node)
        || node.attachment::<bool>(ALWAYS_VISIBLE) == Some(true)
    {
        return;
    }
    if node.attachment::<bool>(DESIRED_VISIBLE) == Some(true) {
        return;
    }
    let weak = node.downgrade();
    let apply = move |opacity: f32| {
        if let Some(owner) = Node::upgrade(&weak) {
            owner.set_attachment(CHROME_OPACITY, opacity);
            owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                "scrollbarChromeOpacity",
            ));
        }
    };
    let current = node.attachment::<f32>(CHROME_OPACITY).unwrap_or(0.0);
    node.animatable_set_with(CHROME_OPACITY, current, 1.0, apply);
    if node.prefers_reduced_motion() {
        return;
    }
    let weak = node.downgrade();
    cove_core::scene::with_animation(
        Some(
            cove_core::scene::Animation::new(0.3, cove_core::scene::AnimationCurve::EaseOut)
                .with_delay(0.7),
        ),
        || {
            node.animatable_set_with(CHROME_OPACITY, 1.0_f32, 0.0_f32, move |opacity| {
                if let Some(owner) = Node::upgrade(&weak) {
                    owner.set_attachment(CHROME_OPACITY, opacity);
                    owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                        "scrollbarChromeOpacity",
                    ));
                }
            });
        },
    );
}

fn scroll_by(view: &Scrollbar, by: f32) {
    let next = (view.value() + by).clamp(0.0, view.max_offset());
    view.offset.set(next);
}

fn update_from_point(view: &Scrollbar, node: &Node, point: Point) {
    if view.max_offset() <= 0.0 {
        return;
    }
    let frame = node.frame();
    let track = match view.axis {
        ScrollbarAxis::Vertical => frame.height,
        ScrollbarAxis::Horizontal => frame.width,
    };
    let thumb = view.thumb_extent(track);
    let travel = (track - thumb).max(1.0);
    let absolute = node.absolute_frame();
    let local = match view.axis {
        ScrollbarAxis::Vertical => point.y - absolute.y,
        ScrollbarAxis::Horizontal => point.x - absolute.x,
    };
    let fraction = ((local - thumb / 2.0) / travel).clamp(0.0, 1.0);
    view.offset.set(fraction * view.max_offset());
}

impl PrimitiveView for Scrollbar {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node.set_is_focusable(true);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        match self.axis {
            ScrollbarAxis::Vertical => layout.set_width(Some(self.thickness)),
            ScrollbarAxis::Horizontal => layout.set_height(Some(self.thickness)),
        }
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        // Hover-capable surfaces auto-hide the companion until hover, focus,
        // drag, or a scroll flash reveals it. Touch-only surfaces have no hover
        // to reveal chrome, so the bar stays visible there.
        let supports_hover = crate::primitives::controls::adaptive::supports_control_hover(node);
        if node.attachment::<f32>(CHROME_OPACITY).is_none() {
            node.set_attachment(
                CHROME_OPACITY,
                if supports_hover { 0.0_f32 } else { 1.0_f32 },
            );
            node.set_attachment(IS_HOVERED, false);
            node.set_attachment(IS_DRAGGING, false);
            node.set_attachment(DESIRED_VISIBLE, false);
            node.set_attachment(LAST_OFFSET, self.value());
        }
        if node.attachment::<bool>(ALWAYS_VISIBLE) != Some(self.always_visible) {
            node.set_attachment(ALWAYS_VISIBLE, self.always_visible);
            if self.always_visible {
                node.cancel_animation(CHROME_OPACITY);
                node.set_attachment(CHROME_OPACITY, 1.0_f32);
                node.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                    "scrollbarChromeOpacity",
                ));
            } else {
                // Recompute from the shown state so a resting bar fades out.
                node.set_attachment(DESIRED_VISIBLE, true);
                apply_desired_visibility(node);
            }
        }

        let value = self.value();
        let max_offset = self.max_offset();
        let percent = if max_offset > 0.0 {
            ((value / max_offset) * 100.0).round() as i64
        } else {
            0
        };
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Slider,
            label: self.label.clone(),
            value: format!("{percent}%"),
            help: crate::i18n::tr("Use arrow keys, Page Up/Down, Home and End to scroll"),
            ..Default::default()
        }));

        let increment_view = self.snapshot();
        let decrement_view = self.snapshot();
        let set_value_view = self.snapshot();
        node.set_accessibility_actions(AccessibilityActions {
            increment: Some(Rc::new(move || {
                scroll_by(&increment_view, increment_view.viewport_extent * 0.9)
            })),
            decrement: Some(Rc::new(move || {
                scroll_by(&decrement_view, -decrement_view.viewport_extent * 0.9)
            })),
            set_value: Some(Rc::new(move |text: &str| {
                if set_value_view.max_offset() <= 0.0 {
                    return;
                }
                let digits: String = text
                    .chars()
                    .filter(|character| character.is_ascii_digit())
                    .collect();
                let Ok(percent) = digits.parse::<i64>() else {
                    return;
                };
                set_value_view
                    .offset
                    .set(set_value_view.max_offset() * percent.clamp(0, 100) as f32 / 100.0);
            })),
            ..Default::default()
        });

        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        let focus_node = node.downgrade();
        node.set_attachment(
            cove_core::scene::TextInputAttachmentKey::FOCUS_CHANGE_HANDLER,
            Rc::new(move |_: bool| {
                if let Some(owner) = Node::upgrade(&focus_node) {
                    apply_desired_visibility(&owner);
                }
            }) as Rc<dyn Fn(bool)>,
        );

        let hover_node = node.downgrade();
        registry.set_hover(
            node,
            Rc::new(move |phase| {
                let Some(owner) = Node::upgrade(&hover_node) else {
                    return;
                };
                match phase {
                    HoverPhase::Enter => owner.set_attachment(IS_HOVERED, true),
                    HoverPhase::Leave => owner.set_attachment(IS_HOVERED, false),
                }
                apply_desired_visibility(&owner);
            }),
        );

        let pointer_node = node.downgrade();
        let pointer_view = self.snapshot();
        registry.set_pointer(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase, _event_phase| {
                if event.button != MouseButton::Left {
                    return EventResult::Ignored;
                }
                let Some(owner) = Node::upgrade(&pointer_node) else {
                    return EventResult::Ignored;
                };
                match phase {
                    PointerPhase::Down => {
                        if let Some(chain) = FocusChainHolder::current() {
                            chain.focus(Some(&owner), false);
                        }
                        if let Some(capture) = PointerCaptureHolder::current() {
                            capture.acquire(&owner, true);
                        }
                        owner.set_attachment(IS_DRAGGING, true);
                        apply_desired_visibility(&owner);
                        update_from_point(&pointer_view, &owner, Point::new(event.x, event.y));
                        EventResult::Handled
                    }
                    PointerPhase::Up => {
                        if PointerCaptureHolder::current()
                            .and_then(|capture| capture.target())
                            .is_some_and(|target| target == owner)
                            && let Some(capture) = PointerCaptureHolder::current()
                        {
                            capture.release();
                        }
                        owner.set_attachment(IS_DRAGGING, false);
                        apply_desired_visibility(&owner);
                        EventResult::Handled
                    }
                }
            }),
        );

        let motion_node = node.downgrade();
        let motion_view = self.snapshot();
        registry.set_motion(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, _phase| {
                // Scrub only while dragging; plain hover motion over the bar must
                // not move the thumb.
                let Some(owner) = Node::upgrade(&motion_node) else {
                    return EventResult::Ignored;
                };
                if !PointerCaptureHolder::current()
                    .and_then(|capture| capture.target())
                    .is_some_and(|target| target == owner)
                {
                    return EventResult::Ignored;
                }
                update_from_point(&motion_view, &owner, Point::new(event.x, event.y));
                EventResult::Handled
            }),
        );

        let key_view = self.snapshot();
        registry.set_key(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase| {
                if phase == EventPhase::Capture {
                    return EventResult::Ignored;
                }
                match event.scancode {
                    crate::Scancode::ARROW_UP | crate::Scancode::ARROW_LEFT => {
                        scroll_by(&key_view, -(20.0_f32).max(key_view.viewport_extent * 0.1));
                    }
                    crate::Scancode::ARROW_DOWN | crate::Scancode::ARROW_RIGHT => {
                        scroll_by(&key_view, (20.0_f32).max(key_view.viewport_extent * 0.1));
                    }
                    crate::Scancode::PAGE_UP => {
                        scroll_by(&key_view, -key_view.viewport_extent * 0.9)
                    }
                    crate::Scancode::PAGE_DOWN => {
                        scroll_by(&key_view, key_view.viewport_extent * 0.9)
                    }
                    crate::Scancode::HOME => key_view.offset.set(0.0),
                    crate::Scancode::END => key_view.offset.set(key_view.max_offset()),
                    _ => return EventResult::Ignored,
                }
                EventResult::Handled
            }),
        );

        let chain = FocusChainHolder::current();
        let focused = chain
            .as_ref()
            .and_then(|chain| chain.focused())
            .is_some_and(|focused| focused == *node);
        let visible_focus = focused && chain.as_ref().is_some_and(|chain| chain.is_focus_visible());
        node.set_attachment(
            AXIS,
            if self.axis == ScrollbarAxis::Vertical {
                0_u8
            } else {
                1_u8
            },
        );
        node.set_attachment(THICKNESS, self.thickness);
        node.set_attachment(CONTENT, self.content_extent);
        node.set_attachment(VIEWPORT, self.viewport_extent);
        node.set_attachment(MINIMUM, self.minimum_thumb_extent);
        node.set_attachment(MAX_OFFSET, max_offset);
        node.set_attachment(VALUE, value);
        let paint = ScrollbarPaint {
            axis: self.axis,
            offset_bits: value.to_bits(),
            content_bits: self.content_extent.to_bits(),
            viewport_bits: self.viewport_extent.to_bits(),
            thickness_bits: self.thickness.to_bits(),
            minimum_bits: self.minimum_thumb_extent.to_bits(),
            focused,
            visible_focus,
        };
        let weak_node = node.downgrade();
        node.update_draw(paint, move |list, origin| {
            let Some(owner) = Node::upgrade(&weak_node) else {
                return;
            };
            let theme = crate::theme::theme_for_node(&owner);
            draw_scrollbar(list, origin, &owner, &theme);
        });

        let last = node.attachment::<f32>(LAST_OFFSET).unwrap_or(value);
        if last != value {
            node.set_attachment(LAST_OFFSET, value);
            flash_if_unheld(node);
        }
        apply_desired_visibility(node);
    }
}
impl_primitive_view!(Scrollbar);

impl Scrollbar {
    fn snapshot(&self) -> Scrollbar {
        Scrollbar {
            axis: self.axis,
            offset: self.offset.clone(),
            content_extent: self.content_extent,
            viewport_extent: self.viewport_extent,
            thickness: self.thickness,
            minimum_thumb_extent: self.minimum_thumb_extent,
            label: self.label.clone(),
            always_visible: self.always_visible,
        }
    }
}

fn draw_scrollbar(
    list: &mut cove_core::DrawList,
    origin: Point,
    node: &Node,
    theme: &crate::theme::Theme,
) {
    let frame = node.frame();
    let track = match axis_of(node) {
        ScrollbarAxis::Vertical => frame.height,
        ScrollbarAxis::Horizontal => frame.width,
    };
    if track <= 0.0 {
        return;
    }
    let opacity = node.attachment::<f32>(CHROME_OPACITY).unwrap_or(1.0);
    if opacity <= 0.01 {
        return;
    }
    let thickness = node.attachment::<f32>(THICKNESS).unwrap_or(12.0);
    let thumb = thumb_extent_for(node, track);
    let travel = (track - thumb).max(0.0);
    let max_offset = node.attachment::<f32>(MAX_OFFSET).unwrap_or(0.0);
    let value = node.attachment::<f32>(VALUE).unwrap_or(0.0);
    let fraction = if max_offset > 0.0 {
        value / max_offset
    } else {
        0.0
    };
    let position = fraction * travel;
    // The macOS overlay scroller: no track at rest, a thin translucent capsule
    // thumb, and a track plus a wider thumb only while the pointer is on it.
    let engaged = node.attachment::<bool>(IS_HOVERED).unwrap_or(false)
        || node.attachment::<bool>(IS_DRAGGING).unwrap_or(false);
    let track_color = if engaged {
        theme.colors.on_surface.multiplied_alpha(0.05 * opacity)
    } else {
        cove_core::Color::CLEAR
    };
    let thumb_color = theme
        .colors
        .on_surface
        .multiplied_alpha(if engaged { 0.5 } else { 0.35 } * opacity);
    let thumb_width: f32 = if engaged { 8.0 } else { 6.0 };
    let track_width: f32 = if engaged { thickness.min(11.0) } else { 4.0 };
    let chain = FocusChainHolder::current();
    let focused = chain
        .as_ref()
        .and_then(|chain| chain.focused())
        .is_some_and(|focused| focused == *node);
    let visible_focus = focused && chain.as_ref().is_some_and(|chain| chain.is_focus_visible());
    match axis_of(node) {
        ScrollbarAxis::Vertical => {
            let _ = list.add_rounded_rect(
                Rect::new(
                    origin.x + (thickness - track_width) / 2.0,
                    origin.y,
                    track_width,
                    track,
                ),
                track_width / 2.0,
                track_color,
            );
            let x = origin.x + (thickness - thumb_width) / 2.0;
            if visible_focus {
                let _ = list.add_rounded_rect(
                    Rect::new(
                        x - 1.0,
                        origin.y + position - 1.0,
                        thumb_width + 2.0,
                        thumb + 2.0,
                    ),
                    thumb_width / 2.0 + 1.0,
                    theme.colors.focus_ring,
                );
            }
            let _ = list.add_rounded_rect(
                Rect::new(x, origin.y + position, thumb_width, thumb),
                thumb_width / 2.0,
                thumb_color,
            );
        }
        ScrollbarAxis::Horizontal => {
            let _ = list.add_rounded_rect(
                Rect::new(
                    origin.x,
                    origin.y + (thickness - track_width) / 2.0,
                    track,
                    track_width,
                ),
                track_width / 2.0,
                track_color,
            );
            let y = origin.y + (thickness - thumb_width) / 2.0;
            if visible_focus {
                let _ = list.add_rounded_rect(
                    Rect::new(
                        origin.x + position - 1.0,
                        y - 1.0,
                        thumb + 2.0,
                        thumb_width + 2.0,
                    ),
                    thumb_width / 2.0 + 1.0,
                    theme.colors.focus_ring,
                );
            }
            let _ = list.add_rounded_rect(
                Rect::new(origin.x + position, y, thumb, thumb_width),
                thumb_width / 2.0,
                thumb_color,
            );
        }
    }
}

fn axis_of(node: &Node) -> ScrollbarAxis {
    node.attachment::<u8>(AXIS)
        .map(|value| {
            if value == 0 {
                ScrollbarAxis::Vertical
            } else {
                ScrollbarAxis::Horizontal
            }
        })
        .unwrap_or(ScrollbarAxis::Vertical)
}

fn thumb_extent_for(node: &Node, track: f32) -> f32 {
    let content = node.attachment::<f32>(CONTENT).unwrap_or(0.0);
    let viewport = node.attachment::<f32>(VIEWPORT).unwrap_or(0.0);
    let minimum = node.attachment::<f32>(MINIMUM).unwrap_or(32.0);
    if content <= viewport || content <= 0.0 {
        return track;
    }
    let proportional = track * viewport / content;
    track.min(minimum.max(proportional))
}
