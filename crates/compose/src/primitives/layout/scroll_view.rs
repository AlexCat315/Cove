//! Translation of Primitives/Layout/ScrollView.swift.
//! Also translates Primitives/Layout/Scrolling/KeyboardFocus.swift via the
//! `ScrollKeyboardFocus` helper below.
//!
//! Clipped scrolling with optional offset binding and viewport observations.
//! Content must retain its natural extent on the scrolling axis. Large
//! datasets belong in a virtualized collection instead.
use std::cell::Cell;
use std::rc::Rc;

use crate::LayoutContainer;
use crate::modifiers::layout::{FrameDimension, LayoutFrameExt};
use crate::primitives::layout::horizontal_scroll_origin::HorizontalScrollOrigin;
use crate::scrolling::ScrollPhysicsEnvironment;
use crate::theme::theme_for_node;
use crate::{AnyView, PrimitiveView, View, impl_primitive_view};
use cove_composition::LayoutDebugExt;
use cove_core::scene::ScrollConsumePolicy;
use cove_core::scene::scrolling::{ScrollAnimator, ScrollPhysics};
use cove_core::scene::{
    AccessibilityActions, AccessibilityRole, AccessibilitySemantics, EventPhase, EventResult,
    FocusChainHolder, HoverPhase, InputHandlerRoute, InteractionRegistryHolder, LayoutNode, Node,
    NodeResource, Overflow, PointerCaptureHolder, PointerPhase,
};
use cove_core::{Binding, Color, Point, Rect, Size};
use cove_platform::{KeyEvent, MouseButton, MouseWheelEvent};

const DRAG_STATE: &str = "__scrollview_scrollbar_drag_state";
const CONTENT_DRAG_STATE: &str = "__scrollview_content_drag_state";
const IS_HOVERED: &str = "__scrollview_scrollbar_is_hovered";
const CHROME_OPACITY: &str = "__scrollview_scrollbar_chrome_opacity";
const TOUCH_INDICATOR: &str = "__scrollview_touch_indicator";
const CONTENT_SIZE: &str = "__scrollview_content_size";
const MOTION: &str = "__scrollview_motion";
const REPORT: &str = "__scrollview_report";
const PENDING_OFFSET: &str = "__scrollview_pending_offset";
const TIMED_REQUEST: &str = "__scrollview_timed_request";
const TIMED_REQUEST_ID: &str = "__scrollview_timed_request_id";
const ANCHOR_MARKER: &str = "__scrollview_anchor";
const ANCHOR_STATE: &str = "__scrollview_anchor_state";
const KEYBOARD_FOCUS: &str = "__scroll.keyboardFocus";
const HORIZONTAL_ORIGIN_ROLE: &str = "scroll-horizontal-origin";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScrollAxis {
    Vertical,
    Horizontal,
    Both,
}

/// How the vertical scrollbar relates to the content area.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ScrollbarGutter {
    /// Floats the bar over the content (default; canvas-style surfaces).
    #[default]
    Overlay,
    /// Reserves a fixed lane on the trailing edge so trailing controls are
    /// never covered by the bar or its grab area.
    Stable,
}

/// One declarative programmatic scroll. A new identity starts a new animation;
/// a duration of zero (or reduced motion) snaps immediately.
#[derive(Clone, Debug, PartialEq)]
pub struct ScrollRequest {
    pub id: u64,
    pub offset: Point,
    pub duration: f64,
}

impl ScrollRequest {
    pub fn new(offset: Point, duration: f64) -> Self {
        Self {
            id: next_request_id(),
            offset: Point::new(
                if offset.x.is_finite() { offset.x } else { 0.0 },
                if offset.y.is_finite() { offset.y } else { 0.0 },
            ),
            duration: if duration.is_finite() {
                duration.max(0.0)
            } else {
                0.0
            },
        }
    }
}

fn next_request_id() -> u64 {
    thread_local! { static NEXT: Cell<u64> = const { Cell::new(1) }; }
    NEXT.with(|next| {
        let value = next.get();
        next.set(value.wrapping_add(1));
        value
    })
}

/// Reported viewport state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollGeometry {
    pub offset: Point,
    pub viewport_size: Size,
    pub content_size: Size,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollViewport {
    pub offset: Point,
    pub size: Size,
}

#[derive(Clone, Copy, PartialEq)]
struct ContentSizeCache {
    size: Size,
    structure_version: u64,
}

#[derive(Clone, Copy, PartialEq)]
struct AnchorState {
    node_id: u64,
    relative: Point,
    content_size: Size,
}

#[derive(Clone, Copy, PartialEq)]
struct PaintIdentity {
    axes: ScrollAxis,
    touch_indicator: bool,
    track_bits: (u32, u32, u32, u32),
    thumb_bits: (u32, u32, u32, u32),
}

#[derive(Clone, Copy)]
enum DragAxis {
    Vertical,
    Horizontal,
}

#[derive(Clone)]
struct ScrollbarDragState {
    axis: DragAxis,
    pointer_start: f32,
    offset_start: f32,
    max_offset: f32,
    #[expect(dead_code, reason = "ported from Swift but not wired in yet")]
    track_start: f32,
    track_length: f32,
    thumb_length: f32,
}

#[derive(Clone)]
struct ContentDragState {
    start: Point,
    offset_start: Point,
    dragged: bool,
}

struct ScrollbarGeometry {
    vertical_track: Option<Rect>,
    vertical_thumb: Option<Rect>,
    horizontal_track: Option<Rect>,
    horizontal_thumb: Option<Rect>,
    max_offset_x: f32,
    max_offset_y: f32,
}

/// Clipped scrolling with optional offset binding and viewport observations.
pub struct ScrollView {
    pub axes: ScrollAxis,
    pub content: AnyView,
    pub on_viewport_change: Option<Rc<dyn Fn(ScrollViewport)>>,
    pub scroll_offset: Option<Binding<Point>>,
    pub on_geometry_change: Option<Rc<dyn Fn(ScrollGeometry)>>,
    pub accessibility_label: Option<String>,
    pub scroll_request: Option<ScrollRequest>,
    pub combines_accessibility: bool,
    pub is_scroll_enabled: bool,
    pub clips_only_overflow: bool,
    pub wheel_step: f32,
    pub consume_policy: ScrollConsumePolicy,
    pub scrollbar_gutter: ScrollbarGutter,
}

impl ScrollView {
    pub fn new(axes: ScrollAxis, content: impl View + 'static) -> Self {
        Self {
            axes,
            content: AnyView::new(content),
            on_viewport_change: None,
            scroll_offset: None,
            on_geometry_change: None,
            accessibility_label: None,
            scroll_request: None,
            combines_accessibility: false,
            is_scroll_enabled: true,
            clips_only_overflow: false,
            wheel_step: 30.0,
            consume_policy: ScrollConsumePolicy::default(),
            scrollbar_gutter: ScrollbarGutter::Overlay,
        }
    }

    pub fn vertical(content: impl View + 'static) -> Self {
        Self::new(ScrollAxis::Vertical, content)
    }

    pub fn horizontal(content: impl View + 'static) -> Self {
        Self::new(ScrollAxis::Horizontal, content)
    }

    pub fn scroll_offset(mut self, binding: Binding<Point>) -> Self {
        self.scroll_offset = Some(binding);
        self
    }

    pub fn on_viewport_change(mut self, handler: impl Fn(ScrollViewport) + 'static) -> Self {
        self.on_viewport_change = Some(Rc::new(handler));
        self
    }

    pub fn on_geometry_change(mut self, handler: impl Fn(ScrollGeometry) + 'static) -> Self {
        self.on_geometry_change = Some(Rc::new(handler));
        self
    }

    pub fn accessibility_label(mut self, label: impl Into<String>) -> Self {
        self.accessibility_label = Some(label.into());
        self
    }

    pub fn scroll_request(mut self, request: ScrollRequest) -> Self {
        self.scroll_request = Some(request);
        self
    }

    pub fn scroll_enabled(mut self, enabled: bool) -> Self {
        self.is_scroll_enabled = enabled;
        self
    }

    pub fn combine_accessibility(mut self, combines: bool) -> Self {
        self.combines_accessibility = combines;
        self
    }

    pub fn clips_only_overflow(mut self, clips: bool) -> Self {
        self.clips_only_overflow = clips;
        self
    }

    pub fn scrollbar_gutter(mut self, gutter: ScrollbarGutter) -> Self {
        self.scrollbar_gutter = gutter;
        self
    }

    pub fn wheel_step(mut self, step: f32) -> Self {
        self.wheel_step = step;
        self
    }

    pub fn consume_policy(mut self, policy: ScrollConsumePolicy) -> Self {
        self.consume_policy = policy;
        self
    }

    /// Width of the reserved lane under `Stable`: track plus inset each side.
    pub fn scrollbar_gutter_width() -> f32 {
        12.0
    }

    /// The lane `Stable` actually reserves on a hover-capable surface.
    pub fn stable_gutter_width(supports_hover: bool) -> f32 {
        if supports_hover {
            Self::scrollbar_gutter_width()
        } else {
            0.0
        }
    }
}

// MARK: - Geometry helpers

pub(crate) fn size_of(rect: Rect) -> Size {
    Size {
        width: rect.width,
        height: rect.height,
    }
}

pub(crate) fn visible_viewport_rect(node: &Node) -> Rect {
    let origin = node.absolute_origin();
    let frame = node.frame();
    let mut visible = Rect::new(origin.x, origin.y, frame.width, frame.height);
    let mut ancestor = node.parent();
    while let Some(current) = ancestor {
        if current.clips_to_bounds() {
            visible = intersect(visible, current.absolute_frame());
            if visible.width <= 0.0 || visible.height <= 0.0 {
                return Rect::new(0.0, 0.0, 0.0, 0.0);
            }
        }
        ancestor = current.parent();
    }
    Rect::new(
        visible.x - origin.x,
        visible.y - origin.y,
        visible.width,
        visible.height,
    )
}

fn intersect(lhs: Rect, rhs: Rect) -> Rect {
    let x = lhs.x.max(rhs.x);
    let y = lhs.y.max(rhs.y);
    let max_x = (lhs.x + lhs.width).min(rhs.x + rhs.width);
    let max_y = (lhs.y + lhs.height).min(rhs.y + rhs.height);
    Rect::new(x, y, (max_x - x).max(0.0), (max_y - y).max(0.0))
}

fn union(lhs: Rect, rhs: Rect) -> Rect {
    let x = lhs.x.min(rhs.x);
    let y = lhs.y.min(rhs.y);
    let max_x = (lhs.x + lhs.width).max(rhs.x + rhs.width);
    let max_y = (lhs.y + lhs.height).max(rhs.y + rhs.height);
    Rect::new(x, y, max_x - x, max_y - y)
}

fn content_bounds(node: &Node, origin: Point) -> Option<Rect> {
    if !node.is_layout_visible() {
        return None;
    }
    let frame = node.frame();
    let placed = Rect::new(
        origin.x + frame.x,
        origin.y + frame.y,
        frame.width,
        frame.height,
    );
    let is_origin = node
        .attachment::<String>("__layoutDebug.role")
        .is_some_and(|role| role == HORIZONTAL_ORIGIN_ROLE);
    let mut bounds = if is_origin { None } else { Some(placed) };
    if node.clips_to_bounds() {
        return bounds;
    }
    let child_origin = Point::new(origin.x + frame.x, origin.y + frame.y);
    for child in node.children() {
        if let Some(child_bounds) = content_bounds(&child, child_origin) {
            bounds = Some(match bounds {
                Some(current) => union(current, child_bounds),
                None => child_bounds,
            });
        }
    }
    bounds
}

fn scrollable_content_size(node: &Node) -> Size {
    let mut bounds: Option<Rect> = None;
    for child in node.children() {
        if let Some(child_bounds) = content_bounds(&child, Point::default()) {
            bounds = Some(match bounds {
                Some(current) => union(current, child_bounds),
                None => child_bounds,
            });
        }
    }
    match bounds {
        Some(bounds) => Size {
            width: (bounds.x.max(0.0) + bounds.width.max(0.0)).max(0.0),
            height: (bounds.y.max(0.0) + bounds.height.max(0.0)).max(0.0),
        },
        None => Size::default(),
    }
}

fn cached_scrollable_content_size(node: &Node) -> Size {
    if let Some(cache) = node.attachment::<ContentSizeCache>(CONTENT_SIZE)
        && cache.structure_version == node.subtree_structure_version()
    {
        return cache.size;
    }
    refresh_scrollable_content_size(node)
}

fn refresh_scrollable_content_size(node: &Node) -> Size {
    let next = scrollable_content_size(node);
    let previous = node
        .attachment::<ContentSizeCache>(CONTENT_SIZE)
        .map(|cache| cache.size);
    if previous != Some(next) {
        node.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
            "scrollContentSize",
        ));
    }
    node.set_attachment(
        CONTENT_SIZE,
        ContentSizeCache {
            size: next,
            structure_version: node.subtree_structure_version(),
        },
    );
    let viewport = size_of(visible_viewport_rect(node));
    let max_x = (next.width - viewport.width).max(0.0);
    let max_y = (next.height - viewport.height).max(0.0);
    if let Some(motion) = node.attachment::<Rc<ScrollAnimator>>(MOTION) {
        motion.update(Point::new(max_x, max_y), viewport);
    } else {
        let offset = node.content_offset();
        if offset.x > max_x || offset.y > max_y {
            node.set_content_offset(Point::new(offset.x.min(max_x), offset.y.min(max_y)));
        }
    }
    next
}

fn is_layout_suppressed(node: &Node) -> bool {
    let mut current = Some(node.clone());
    while let Some(candidate) = current {
        if candidate
            .layout_node()
            .is_some_and(|layout| layout.display() == cove_core::scene::Display::None)
        {
            return true;
        }
        current = candidate.parent();
    }
    false
}

fn resolved_physics(node: &Node) -> ScrollPhysics {
    let mut physics: ScrollPhysics = node.composition_value(&ScrollPhysicsEnvironment::key());
    if node.prefers_reduced_motion() {
        physics.momentum.is_enabled = false;
        physics.rubber_band.is_enabled = false;
    }
    physics
}

fn thumb_fraction(offset: f32, maximum_offset: f32) -> f32 {
    if maximum_offset <= 0.0 {
        return 0.0;
    }
    (offset / maximum_offset).clamp(0.0, 1.0)
}

fn thumb_shrink(offset: f32, maximum_offset: f32, viewport: f32) -> f32 {
    if viewport <= 0.0 {
        return 1.0;
    }
    let excess = (-offset).max(offset - maximum_offset);
    if excess <= 0.0 {
        return 1.0;
    }
    (1.0 - excess / viewport).max(0.4)
}

fn publish_scroll_range(motion: &ScrollAnimator, node: &Node) -> bool {
    let frame = node.frame();
    if is_layout_suppressed(node) || frame.width <= 0.0 || frame.height <= 0.0 {
        return false;
    }
    let viewport = size_of(visible_viewport_rect(node));
    let content = cached_scrollable_content_size(node);
    if content.width <= 0.0 && content.height <= 0.0 {
        return false;
    }
    motion.update(
        Point::new(
            (content.width - viewport.width).max(0.0),
            (content.height - viewport.height).max(0.0),
        ),
        viewport,
    );
    true
}

fn scrollbar_geometry(
    node: &Node,
    axes: ScrollAxis,
    thickness: f32,
    inset: f32,
) -> ScrollbarGeometry {
    let viewport = visible_viewport_rect(node);
    let content = cached_scrollable_content_size(node);
    let max_x = (content.width - viewport.width).max(0.0);
    let max_y = (content.height - viewport.height).max(0.0);
    let offset = node.content_offset();
    let mut result = ScrollbarGeometry {
        vertical_track: None,
        vertical_thumb: None,
        horizontal_track: None,
        horizontal_thumb: None,
        max_offset_x: max_x,
        max_offset_y: max_y,
    };
    if matches!(axes, ScrollAxis::Vertical | ScrollAxis::Both)
        && max_y > 0.0
        && viewport.height > 0.0
        && content.height > 0.0
    {
        let track = Rect::new(
            viewport.x + viewport.width - thickness - inset,
            viewport.y + inset,
            thickness,
            (viewport.height - 2.0 * inset).max(0.0),
        );
        let shrink = thumb_shrink(offset.y, max_y, viewport.height);
        let thumb_height = thickness
            .max((thickness * 2.0).max(track.height * (viewport.height / content.height)) * shrink);
        let fraction = thumb_fraction(offset.y, max_y);
        result.vertical_track = Some(track);
        result.vertical_thumb = Some(Rect::new(
            track.x,
            track.y + (track.height - thumb_height) * fraction,
            track.width,
            thumb_height,
        ));
    }
    if matches!(axes, ScrollAxis::Horizontal | ScrollAxis::Both)
        && max_x > 0.0
        && viewport.width > 0.0
        && content.width > 0.0
    {
        let track = Rect::new(
            viewport.x + inset,
            viewport.y + viewport.height - thickness - inset,
            (viewport.width - 2.0 * inset).max(0.0),
            thickness,
        );
        let shrink = thumb_shrink(offset.x, max_x, viewport.width);
        let thumb_width = thickness
            .max((thickness * 2.0).max(track.width * (viewport.width / content.width)) * shrink);
        let fraction = thumb_fraction(offset.x, max_x);
        result.horizontal_track = Some(track);
        result.horizontal_thumb = Some(Rect::new(
            track.x + (track.width - thumb_width) * fraction,
            track.y,
            thumb_width,
            track.height,
        ));
    }
    result
}

fn track_start(axis: DragAxis, track: Rect) -> f32 {
    match axis {
        DragAxis::Vertical => track.y,
        DragAxis::Horizontal => track.x,
    }
}

fn track_length(axis: DragAxis, track: Rect) -> f32 {
    match axis {
        DragAxis::Vertical => track.height,
        DragAxis::Horizontal => track.width,
    }
}

fn expanded_hit_track(axis: DragAxis, track: Rect, hit_slop: f32) -> Rect {
    match axis {
        DragAxis::Vertical => Rect::new(
            track.x - hit_slop,
            track.y,
            track.width + 2.0 * hit_slop,
            track.height,
        ),
        DragAxis::Horizontal => Rect::new(
            track.x,
            track.y - hit_slop,
            track.width,
            track.height + 2.0 * hit_slop,
        ),
    }
}

fn contains(rect: Rect, point: Point) -> bool {
    point.x >= rect.x
        && point.x <= rect.x + rect.width
        && point.y >= rect.y
        && point.y <= rect.y + rect.height
}

fn begin_scrollbar_drag(
    node: &Node,
    axis: DragAxis,
    local: Point,
    geometry: &ScrollbarGeometry,
    hit_slop: f32,
    motion: &ScrollAnimator,
) -> Option<ScrollbarDragState> {
    let (track, thumb, max_offset, pointer) = match axis {
        DragAxis::Vertical => (
            geometry.vertical_track?,
            geometry.vertical_thumb?,
            geometry.max_offset_y,
            local.y,
        ),
        DragAxis::Horizontal => (
            geometry.horizontal_track?,
            geometry.horizontal_thumb?,
            geometry.max_offset_x,
            local.x,
        ),
    };
    if max_offset <= 0.0 {
        return None;
    }
    if !contains(expanded_hit_track(axis, track, hit_slop), local) {
        return None;
    }
    // A grab anywhere cancels whatever the previous gesture left behind —
    // glide or return spring alike. Tracking starts empty, so this drag cannot
    // inherit the previous one's velocity either.
    motion.adopt(node.content_offset());
    if !contains(thumb, local) {
        let available = (track_length(axis, track) - track_length(axis, thumb)).max(1.0);
        let centered_start = pointer - track_length(axis, thumb) / 2.0;
        let raw = ((centered_start - track_start(axis, track)) / available) * max_offset;
        let mut next = node.content_offset();
        match axis {
            DragAxis::Vertical => next.y = raw.clamp(0.0, max_offset),
            DragAxis::Horizontal => next.x = raw.clamp(0.0, max_offset),
        }
        node.set_content_offset(next);
        motion.adopt(next);
    }
    let offset = node.content_offset();
    Some(ScrollbarDragState {
        axis,
        pointer_start: pointer,
        offset_start: match axis {
            DragAxis::Vertical => offset.y,
            DragAxis::Horizontal => offset.x,
        },
        max_offset,
        track_start: track_start(axis, track),
        track_length: track_length(axis, track),
        thumb_length: track_length(axis, thumb),
    })
}

fn set_scrollbar_chrome_visible(node: &Node, visible: bool) {
    let current = node.attachment::<f32>(CHROME_OPACITY).unwrap_or(0.0);
    let target = if visible { 1.0 } else { 0.0 };
    let weak = node.downgrade();
    node.animatable_set_with(CHROME_OPACITY, current, target, move |opacity| {
        if let Some(owner) = Node::upgrade(&weak) {
            owner.set_attachment(CHROME_OPACITY, opacity);
            owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                "scrollbarChromeOpacity",
            ));
        }
    });
}

/// Touch surfaces have no hover to reveal chrome. Like platform scroll
/// indicators, the thin thumb appears while content moves and fades once it
/// rests. Reduced motion keeps it visible instead of animating it out.
fn flash_touch_indicator(node: &Node) {
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
    // Swift wraps this fade in `withAnimation(.easeOut(duration: 0.3, delay: 0.7))`.
    // Without the context `animatable_set_with` snaps, so the reveal above and this
    // fade collapse into 0 -> 1 -> 0 in one call and the indicator is never seen.
    cove_core::scene::with_animation(
        Some(
            cove_core::scene::Animation::new(0.3, cove_core::scene::AnimationCurve::EaseOut)
                .with_delay(0.7),
        ),
        || {
            node.animatable_set_with(
                "__scrollview.chromeFade",
                1.0_f32,
                0.0_f32,
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

fn motion_animator(node: &Node, physics: ScrollPhysics) -> Rc<ScrollAnimator> {
    if let Some(existing) = node.attachment::<Rc<ScrollAnimator>>(MOTION) {
        existing.set_physics(physics);
        return existing;
    }
    let weak = node.downgrade();
    let animator = ScrollAnimator::new(
        physics,
        Point::default(),
        Size::default(),
        node.content_offset(),
        Some(Rc::new(move |offset: Point| {
            let Some(owner) = Node::upgrade(&weak) else {
                return;
            };
            if owner.content_offset() != offset
                && owner.attachment::<bool>(TOUCH_INDICATOR) == Some(true)
            {
                flash_touch_indicator(&owner);
            }
            owner.set_content_offset(offset);
            if let Some(report) = owner.attachment::<Rc<dyn Fn(&Node)>>(REPORT) {
                report(&owner);
            }
        })),
    );
    node.set_attachment(MOTION, animator.clone());
    let lifetime: Rc<dyn NodeResource> = Rc::new(ScrollMotionLifetime {
        animator: animator.clone(),
    });
    node.add_resource(lifetime);
    animator
}

struct ScrollMotionLifetime {
    animator: Rc<ScrollAnimator>,
}

impl NodeResource for ScrollMotionLifetime {
    fn mount(&self, _node: &Node) {}

    fn unmount(&self, _node: &Node) {
        self.animator.cancel();
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}

fn collect_anchors(node: &Node, anchors: &mut Vec<Node>) {
    if node.attachment::<bool>(ANCHOR_MARKER) == Some(true) {
        anchors.push(node.clone());
    }
    for child in node.children() {
        collect_anchors(&child, anchors);
    }
}

fn node_identity(node: &Node) -> u64 {
    node.id().0
}

fn update_anchor(node: &Node, motion: &ScrollAnimator, axes: ScrollAxis) {
    let mut anchors: Vec<Node> = Vec::new();
    collect_anchors(node, &mut anchors);
    if anchors.is_empty() {
        node.remove_attachment(ANCHOR_STATE);
        return;
    }
    let origin = node.absolute_origin();
    let viewport_height = visible_viewport_rect(node).height.max(1.0);
    let relative = |anchor: &Node| -> Point {
        let frame = anchor.absolute_frame();
        Point::new(frame.x - origin.x, frame.y - origin.y)
    };
    let mut visible: Vec<&Node> = Vec::new();
    for anchor in &anchors {
        let point = relative(anchor);
        if point.y + anchor.absolute_frame().height > 0.0 && point.y < viewport_height {
            visible.push(anchor);
        }
    }
    let mut fully_visible: Vec<&Node> = visible
        .iter()
        .copied()
        .filter(|anchor| relative(anchor).y >= 0.0)
        .collect();
    fully_visible.sort_by(|lhs, rhs| relative(lhs).y.total_cmp(&relative(rhs).y));
    let selected: &Node = if let Some(first) = fully_visible.first() {
        first
    } else if let Some(last) = visible
        .iter()
        .max_by(|lhs, rhs| relative(lhs).y.total_cmp(&relative(rhs).y))
    {
        last
    } else {
        &anchors[0]
    };
    let content = cached_scrollable_content_size(node);
    let mut baseline = relative(selected);
    if let Some(previous) = node.attachment::<AnchorState>(ANCHOR_STATE)
        && previous.content_size != content
        && let Some(old) = anchors
            .iter()
            .find(|anchor| node_identity(anchor) == previous.node_id)
    {
        let current = relative(old);
        let delta = Point::new(
            current.x - previous.relative.x,
            current.y - previous.relative.y,
        );
        let mut offset = node.content_offset();
        if axes != ScrollAxis::Vertical {
            offset.x += delta.x;
        }
        if axes != ScrollAxis::Horizontal {
            offset.y += delta.y;
        }
        motion.adopt(offset);
        baseline = previous.relative;
    }
    node.set_attachment(
        ANCHOR_STATE,
        AnchorState {
            node_id: node_identity(selected),
            relative: baseline,
            content_size: content,
        },
    );
}

struct ScrollViewObservers {
    scroll_offset: Option<Binding<Point>>,
    on_viewport_change: Option<Rc<dyn Fn(ScrollViewport)>>,
    on_geometry_change: Option<Rc<dyn Fn(ScrollGeometry)>>,
    accessibility_label: Option<String>,
    combines_accessibility: bool,
    is_scroll_enabled: bool,
    axes: ScrollAxis,
}

fn update_accessibility(node: &Node, view: &ScrollViewObservers) {
    if !view.is_scroll_enabled {
        node.set_accessibility(None);
        return;
    }
    let size = size_of(visible_viewport_rect(node));
    let content = cached_scrollable_content_size(node);
    let max_x = (content.width - size.width).max(0.0);
    let max_y = (content.height - size.height).max(0.0);
    if view.combines_accessibility && max_x <= 0.0 && max_y <= 0.0 {
        node.set_accessibility(None);
        return;
    }
    let offset = node.content_offset();
    let x = if max_x > 0.0 {
        ((offset.x / max_x * 100.0).round() as i64).clamp(0, 100)
    } else {
        0
    };
    let y = if max_y > 0.0 {
        ((offset.y / max_y * 100.0).round() as i64).clamp(0, 100)
    } else {
        0
    };
    let value = match view.axes {
        ScrollAxis::Vertical => format!("{y}%"),
        ScrollAxis::Horizontal => format!("{x}%"),
        ScrollAxis::Both => format!("Horizontal {x}%, vertical {y}%"),
    };
    let label = view.accessibility_label.clone().unwrap_or_else(|| {
        if view.combines_accessibility {
            String::new()
        } else {
            "Scrollable content".to_string()
        }
    });
    node.set_accessibility(Some(AccessibilitySemantics {
        role: AccessibilityRole::ScrollArea,
        label,
        value,
        help: "Use arrow keys, Page Up/Down, Home and End to scroll".to_string(),
        combines_children: view.combines_accessibility,
        ..Default::default()
    }));
}

fn page_scroll(node: &Node, axes: ScrollAxis, forward: bool) {
    let Some(motion) = node.attachment::<Rc<ScrollAnimator>>(MOTION) else {
        return;
    };
    let viewport = visible_viewport_rect(node);
    let sign = if forward { 1.0 } else { -1.0 };
    let dx = if axes == ScrollAxis::Vertical {
        0.0
    } else {
        (viewport.width - 40.0).max(40.0) * sign
    };
    let dy = if axes == ScrollAxis::Horizontal {
        0.0
    } else {
        (viewport.height - 40.0).max(40.0) * sign
    };
    motion.scroll_directly(Point::new(dx, dy));
}

fn report(node: &Node, view: &ScrollViewObservers) {
    let size = size_of(visible_viewport_rect(node));
    let offset = node.content_offset();
    if let Some(binding) = view.scroll_offset.as_ref()
        && binding.get() != offset
    {
        binding.set(offset);
    }
    if let Some(handler) = view.on_viewport_change.as_ref() {
        handler(ScrollViewport { offset, size });
    }
    if let Some(handler) = view.on_geometry_change.as_ref() {
        handler(ScrollGeometry {
            offset,
            viewport_size: size,
            content_size: cached_scrollable_content_size(node),
        });
    }
    update_accessibility(node, view);
    if let Some(motion) = node.attachment::<Rc<ScrollAnimator>>(MOTION) {
        update_anchor(node, &motion, view.axes);
    }
}

fn local_point(node: &Node, x: f32, y: f32) -> Point {
    node.convert_from_window(Point::new(x, y))
}

fn scroll_to_request(
    node: &Node,
    motion: &ScrollAnimator,
    request: &ScrollRequest,
    axes: ScrollAxis,
) {
    let maximum = motion.maximum_offset();
    let mut target = Point::new(
        request.offset.x.clamp(0.0, maximum.x),
        request.offset.y.clamp(0.0, maximum.y),
    );
    if axes == ScrollAxis::Vertical {
        target.x = 0.0;
    }
    if axes == ScrollAxis::Horizontal {
        target.y = 0.0;
    }
    if node.prefers_reduced_motion() {
        motion.adopt(target);
    } else {
        motion.scroll_to(target, request.duration);
    }
}

fn install_interaction(
    node: &Node,
    view: &ScrollViewObservers,
    wheel_step: f32,
    consume_policy: ScrollConsumePolicy,
    gutter: ScrollbarGutter,
) {
    let axes = view.axes;
    let touch_indicator = !crate::primitives::controls::adaptive::supports_control_hover(node);
    node.set_attachment(TOUCH_INDICATOR, touch_indicator);
    if let Some(layout) = node.layout_node() {
        let lane = if gutter == ScrollbarGutter::Stable && !touch_indicator {
            ScrollView::scrollbar_gutter_width()
        } else {
            0.0
        };
        layout.set_padding(
            if axes != ScrollAxis::Horizontal {
                lane
            } else {
                0.0
            },
            cove_core::scene::Edge::End,
        );
        layout.set_padding(
            if axes != ScrollAxis::Vertical {
                lane
            } else {
                0.0
            },
            cove_core::scene::Edge::Bottom,
        );
    }
    let track_thickness = if touch_indicator { 3.0 } else { 8.0 };
    let track_inset = if touch_indicator { 3.0 } else { 2.0 };
    let track_hit_slop = 6.0;

    let motion = motion_animator(node, resolved_physics(node));
    let supports_touch_drag = node
        .composition_value(&crate::adaptive::SurfaceEnvironment::key())
        .input
        .contains(cove_core::InputCapabilities::TOUCH);
    if node.attachment::<Point>(PENDING_OFFSET).is_none() {
        if node.prefers_reduced_motion() {
            motion.finish_immediately();
        }
        publish_scroll_range(&motion, node);
        motion.adopt(node.content_offset());
    }
    if let Some(request) = node.attachment::<ScrollRequest>(TIMED_REQUEST) {
        if motion.maximum_offset() == Point::default()
            && (request.offset.x != 0.0 || request.offset.y != 0.0)
        {
            // Content is not measured yet; keep the request for the first
            // layout pass instead of clamping it to zero.
            node.mark_dirty(cove_core::scene::InvalidationSource::style_set(
                "scrollView.request",
            ));
        } else {
            node.remove_attachment(TIMED_REQUEST);
            scroll_to_request(node, &motion, &request, axes);
            let observers = view.clone_for_report();
            report(node, &observers);
        }
    }

    let reveal_node = node.downgrade();
    node.set_scroll_reveal(Some(Rc::new(move |target: Rect| {
        let Some(owner) = Node::upgrade(&reveal_node) else {
            return;
        };
        let Some(motion) = owner.attachment::<Rc<ScrollAnimator>>(MOTION) else {
            return;
        };
        let previous = owner.content_offset();
        refresh_scrollable_content_size(&owner);
        publish_scroll_range(&motion, &owner);
        // Updating a smaller range can clamp the offset. The caller's
        // window-space rectangle was measured before that clamp.
        let current = owner.content_offset();
        let target = Rect::new(
            target.x + (previous.x - current.x),
            target.y + (previous.y - current.y),
            target.width,
            target.height,
        );
        let origin = owner.absolute_origin();
        let viewport = visible_viewport_rect(&owner);
        let viewport = Rect::new(
            viewport.x + origin.x,
            viewport.y + origin.y,
            viewport.width,
            viewport.height,
        );
        let mut offset = current;
        if axes != ScrollAxis::Vertical {
            offset.x += crate::primitives::layout::ScrollReveal::delta(
                target.x,
                target.x + target.width,
                viewport.x,
                viewport.x + viewport.width,
            );
        }
        if axes != ScrollAxis::Horizontal {
            offset.y += crate::primitives::layout::ScrollReveal::delta(
                target.y,
                target.y + target.height,
                viewport.y,
                viewport.y + viewport.height,
            );
        }
        motion.adopt(offset);
    })));

    if node.attachment::<f32>(CHROME_OPACITY).is_none() {
        node.set_attachment(CHROME_OPACITY, 0.0_f32);
    }
    if node.attachment::<bool>(IS_HOVERED).is_none() {
        node.set_attachment(IS_HOVERED, false);
    }

    let layout_node = node.downgrade();
    let layout_observers = view.clone_for_report();
    node.set_layout_did_update(Some(Rc::new(move |owner: &Node| {
        // Retained responsive panes suppress flex layout for the whole subtree.
        // Zero frames are not new scroll geometry: publishing them would clamp
        // the user's saved offset to zero while hidden.
        if is_layout_suppressed(owner) {
            return;
        }
        if !layout_observers.is_scroll_enabled {
            if let Some(motion) = owner.attachment::<Rc<ScrollAnimator>>(MOTION) {
                motion.cancel();
                motion.adopt(Point::default());
            }
            owner.set_content_offset(Point::default());
            ScrollKeyboardFocus::update(owner, false);
            report(owner, &layout_observers);
            return;
        }
        refresh_scrollable_content_size(owner);
        if let Some(motion) = owner.attachment::<Rc<ScrollAnimator>>(MOTION) {
            publish_scroll_range(&motion, owner);
            update_anchor(owner, &motion, layout_observers.axes);
            ScrollKeyboardFocus::update(
                owner,
                motion.maximum_offset().x > 0.5 || motion.maximum_offset().y > 0.5,
            );
        }
        if let Some(desired) = owner.attachment::<Point>(PENDING_OFFSET) {
            owner.remove_attachment(PENDING_OFFSET);
            if let Some(motion) = owner.attachment::<Rc<ScrollAnimator>>(MOTION) {
                motion.adopt(desired);
            }
        }
        if let Some(request) = owner.attachment::<ScrollRequest>(TIMED_REQUEST) {
            owner.remove_attachment(TIMED_REQUEST);
            if let Some(motion) = owner.attachment::<Rc<ScrollAnimator>>(MOTION) {
                scroll_to_request(owner, &motion, &request, layout_observers.axes);
            }
        }
        report(owner, &layout_observers);
        let _ = &layout_node;
    })));

    if !view.is_scroll_enabled {
        motion.cancel();
        motion.adopt(Point::default());
        node.set_content_offset(Point::default());
        node.set_scroll_reveal(None);
        node.set_accessibility_actions(AccessibilityActions::default());
        if let Some(registry) = InteractionRegistryHolder::current() {
            registry.remove(node);
        }
        node.update_overlay_draw(false, |_, _| {});
        return;
    }

    let Some(registry) = InteractionRegistryHolder::current() else {
        return;
    };

    let hover_node = node.downgrade();
    let hover_axes = axes;
    registry.set_hover(
        node,
        Rc::new(move |phase| {
            let Some(owner) = Node::upgrade(&hover_node) else {
                return;
            };
            match phase {
                HoverPhase::Enter => {
                    owner.set_attachment(IS_HOVERED, true);
                    if touch_indicator {
                        return;
                    }
                    set_scrollbar_chrome_visible(&owner, true);
                }
                HoverPhase::Leave => {
                    owner.set_attachment(IS_HOVERED, false);
                    let is_dragging = owner.attachment::<ScrollbarDragState>(DRAG_STATE).is_some();
                    set_scrollbar_chrome_visible(&owner, is_dragging);
                }
            }
            let _ = hover_axes;
        }),
    );

    let wheel_node = node.downgrade();
    let wheel_observers = view.clone_for_report();
    registry.set_wheel(
        node,
        InputHandlerRoute::scroll(),
        Rc::new(move |event: &MouseWheelEvent, _phase| {
            let Some(owner) = Node::upgrade(&wheel_node) else {
                return EventResult::Ignored;
            };
            if !event.input.is_continuation()
                && let (Some(mouse_x), Some(mouse_y)) = (event.mouse_x, event.mouse_y)
            {
                let local = local_point(&owner, mouse_x, mouse_y);
                if !contains(visible_viewport_rect(&owner), local) {
                    return EventResult::Ignored;
                }
            }
            let viewport = visible_viewport_rect(&owner);
            let (delta_x, delta_y) =
                event.displacement(wheel_step, viewport.width, viewport.height);
            let mut dx = if wheel_observers.axes != ScrollAxis::Vertical {
                -delta_x
            } else {
                0.0
            };
            let mut dy = if wheel_observers.axes != ScrollAxis::Horizontal {
                -delta_y
            } else {
                0.0
            };
            let previous = owner.content_offset();
            let Some(motion) = owner.attachment::<Rc<ScrollAnimator>>(MOTION) else {
                return consume_policy.result(false, false);
            };
            // Nothing measured yet — keep letting the event bubble.
            if !publish_scroll_range(&motion, &owner) {
                return consume_policy.result(false, false);
            }
            let maximum = motion.maximum_offset();
            let horizontal_overflow =
                wheel_observers.axes != ScrollAxis::Vertical && maximum.x > 0.0;
            let vertical_overflow =
                wheel_observers.axes != ScrollAxis::Horizontal && maximum.y > 0.0;
            if dx == 0.0 && dy == 0.0 {
                if event.input.phase != cove_platform::ScrollPhase::None
                    || event.input.momentum_phase != cove_platform::ScrollPhase::None
                {
                    motion.scroll_directly(Point::default());
                }
                // Zero-delta lifecycle packets cannot identify an axis. An existing
                // phased stream already retains its owner.
                return consume_policy.result(false, false);
            }
            // Overflow owns the input at either bound. An empty axis still passes
            // through, so a horizontal child does not trap a vertical parent's input.
            if !horizontal_overflow {
                dx = 0.0;
            }
            if !vertical_overflow {
                dy = 0.0;
            }
            if dx == 0.0 && dy == 0.0 {
                return consume_policy.result(false, false);
            }
            // Precise input already contains the OS's acceleration and momentum.
            // Line wheels are discrete placements. Neither may infer another fling
            // or start a spring between input packets.
            motion.adopt(previous);
            let next = motion.scroll_directly(Point::new(dx, dy));
            consume_policy.result(next != previous, true)
        }),
    );

    let key_node = node.downgrade();
    let key_axes = axes;
    registry.set_key(
        node,
        InputHandlerRoute::scroll(),
        Rc::new(move |event: &KeyEvent, phase| {
            if phase != EventPhase::Target {
                return EventResult::Ignored;
            }
            let Some(owner) = Node::upgrade(&key_node) else {
                return EventResult::Ignored;
            };
            let Some(motion) = owner.attachment::<Rc<ScrollAnimator>>(MOTION) else {
                return EventResult::Ignored;
            };
            if !publish_scroll_range(&motion, &owner) {
                return EventResult::Ignored;
            }
            let viewport = visible_viewport_rect(&owner);
            let line = 40.0_f32;
            let page = (viewport.height * 0.9).max(line);
            let mut dx = 0.0_f32;
            let mut dy = 0.0_f32;
            match event.scancode {
                crate::Scancode::ARROW_UP => dy = -line,
                crate::Scancode::ARROW_DOWN => dy = line,
                crate::Scancode::ARROW_LEFT => dx = -line,
                crate::Scancode::ARROW_RIGHT => dx = line,
                crate::Scancode::PAGE_UP => dy = -page,
                crate::Scancode::PAGE_DOWN => dy = page,
                crate::Scancode::HOME => {
                    dx = -motion.offset().x;
                    dy = -motion.offset().y;
                }
                crate::Scancode::END => {
                    let maximum = motion.maximum_offset();
                    dx = maximum.x - motion.offset().x;
                    dy = maximum.y - motion.offset().y;
                }
                _ => return EventResult::Ignored,
            }
            if key_axes == ScrollAxis::Vertical {
                dx = 0.0;
            }
            if key_axes == ScrollAxis::Horizontal {
                dy = 0.0;
            }
            if dx == 0.0 && dy == 0.0 {
                return EventResult::Ignored;
            }
            let previous = motion.offset();
            motion.stop_coasting();
            let next = motion.scroll_directly(Point::new(dx, dy));
            if next != previous {
                EventResult::Handled
            } else {
                EventResult::Ignored
            }
        }),
    );

    let pointer_node = node.downgrade();
    let pointer_axes = axes;
    registry.set_pointer(
        node,
        InputHandlerRoute::scroll_chrome(),
        Rc::new(move |event, pointer_phase, event_phase| {
            let Some(owner) = Node::upgrade(&pointer_node) else {
                return EventResult::Ignored;
            };
            match pointer_phase {
                PointerPhase::Down => {
                    if event.button != MouseButton::Left {
                        return EventResult::Ignored;
                    }
                    let local = local_point(&owner, event.x, event.y);
                    // Chrome routes are global. Its expanded hit lane must remain
                    // inside this viewport and every ancestor clip.
                    if !contains(visible_viewport_rect(&owner), local) {
                        return EventResult::Ignored;
                    }
                    // Pressing anywhere inside the container grabs content that is
                    // still coasting — the same "touch to stop" every scrollable
                    // surface has. Never consumes the event, so a press that was
                    // meant for a child still reaches it.
                    let Some(motion) = owner.attachment::<Rc<ScrollAnimator>>(MOTION) else {
                        return EventResult::Ignored;
                    };
                    motion.stop_coasting();
                    if event_phase != EventPhase::Capture && event_phase != EventPhase::Target {
                        return EventResult::Ignored;
                    }
                    let geometry =
                        scrollbar_geometry(&owner, pointer_axes, track_thickness, track_inset);
                    if !touch_indicator
                        && let Some(state) = begin_scrollbar_drag(
                            &owner,
                            DragAxis::Vertical,
                            local,
                            &geometry,
                            track_hit_slop,
                            &motion,
                        )
                    {
                        owner.set_attachment(DRAG_STATE, state);
                        if let Some(capture) = PointerCaptureHolder::current() {
                            capture.acquire(&owner, true);
                        }
                        set_scrollbar_chrome_visible(&owner, true);
                        return EventResult::Handled;
                    }
                    if !touch_indicator
                        && let Some(state) = begin_scrollbar_drag(
                            &owner,
                            DragAxis::Horizontal,
                            local,
                            &geometry,
                            track_hit_slop,
                            &motion,
                        )
                    {
                        owner.set_attachment(DRAG_STATE, state);
                        if let Some(capture) = PointerCaptureHolder::current() {
                            capture.acquire(&owner, true);
                        }
                        set_scrollbar_chrome_visible(&owner, true);
                        return EventResult::Handled;
                    }
                    if supports_touch_drag {
                        owner.set_attachment(
                            CONTENT_DRAG_STATE,
                            ContentDragState {
                                start: local,
                                offset_start: owner.content_offset(),
                                dragged: false,
                            },
                        );
                    }
                    EventResult::Ignored
                }
                PointerPhase::Up => {
                    if owner.attachment::<ScrollbarDragState>(DRAG_STATE).is_some() {
                        owner.remove_attachment(DRAG_STATE);
                        if let Some(capture) = PointerCaptureHolder::current() {
                            capture.release();
                        }
                        // A thumb release preserves its absolute placement.
                        if let Some(motion) = owner.attachment::<Rc<ScrollAnimator>>(MOTION) {
                            motion.finish_immediately();
                        }
                        let is_hovered = owner.attachment::<bool>(IS_HOVERED).unwrap_or(false);
                        set_scrollbar_chrome_visible(&owner, is_hovered);
                        return EventResult::Handled;
                    }
                    if let Some(state) = owner.attachment::<ContentDragState>(CONTENT_DRAG_STATE) {
                        owner.remove_attachment(CONTENT_DRAG_STATE);
                        if !state.dragged {
                            return EventResult::Ignored;
                        }
                        if let Some(capture) = PointerCaptureHolder::current() {
                            capture.release();
                        }
                        if let Some(motion) = owner.attachment::<Rc<ScrollAnimator>>(MOTION) {
                            motion.settle();
                        }
                        return EventResult::Handled;
                    }
                    EventResult::Ignored
                }
            }
        }),
    );

    let motion_node = node.downgrade();
    let motion_axes = axes;
    registry.set_motion(
        node,
        InputHandlerRoute::scroll_chrome(),
        Rc::new(move |event, phase| {
            let Some(owner) = Node::upgrade(&motion_node) else {
                return EventResult::Ignored;
            };
            // Let the deepest content gesture claim a touch drag first. Ancestors
            // also observed the down event; handling capture here would steal
            // nested scrolling, swipes and refreshes.
            if phase == EventPhase::Capture
                && owner.attachment::<ScrollbarDragState>(DRAG_STATE).is_none()
            {
                return EventResult::Ignored;
            }
            if let Some(state) = owner.attachment::<ContentDragState>(CONTENT_DRAG_STATE) {
                let local = local_point(&owner, event.x, event.y);
                let dx = local.x - state.start.x;
                let dy = local.y - state.start.y;
                if !state.dragged {
                    let along = match motion_axes {
                        ScrollAxis::Horizontal => dx.abs(),
                        ScrollAxis::Vertical => dy.abs(),
                        ScrollAxis::Both => dx.abs().max(dy.abs()),
                    };
                    let across = match motion_axes {
                        ScrollAxis::Horizontal => dy.abs(),
                        ScrollAxis::Vertical => dx.abs(),
                        ScrollAxis::Both => 0.0,
                    };
                    if along < 4.0 || along < across {
                        return EventResult::Ignored;
                    }
                }
                let mut next_state = ContentDragState {
                    start: state.start,
                    offset_start: state.offset_start,
                    dragged: state.dragged,
                };
                if !next_state.dragged {
                    next_state.dragged = true;
                    if let Some(capture) = PointerCaptureHolder::current() {
                        // Non-transferable, matching Swift's bare `acquire(node)`
                        // (`ScrollView.swift:585`). Marking it transferable let the
                        // scroll steal an in-flight slider or resize drag.
                        capture.acquire(&owner, false);
                    }
                }
                owner.set_attachment(CONTENT_DRAG_STATE, next_state);
                if let Some(motion) = owner.attachment::<Rc<ScrollAnimator>>(MOTION) {
                    motion.move_to(Point::new(
                        if motion_axes == ScrollAxis::Vertical {
                            state.offset_start.x
                        } else {
                            state.offset_start.x - dx
                        },
                        if motion_axes == ScrollAxis::Horizontal {
                            state.offset_start.y
                        } else {
                            state.offset_start.y - dy
                        },
                    ));
                }
                return EventResult::Handled;
            }
            let Some(state) = owner.attachment::<ScrollbarDragState>(DRAG_STATE) else {
                return EventResult::Ignored;
            };
            let Some(motion) = owner.attachment::<Rc<ScrollAnimator>>(MOTION) else {
                return EventResult::Handled;
            };
            if !publish_scroll_range(&motion, &owner) {
                return EventResult::Handled;
            }
            motion.adopt(owner.content_offset());
            let local = local_point(&owner, event.x, event.y);
            let pointer = match state.axis {
                DragAxis::Vertical => local.y,
                DragAxis::Horizontal => local.x,
            };
            let available_track = (state.track_length - state.thumb_length).max(1.0);
            // A scrollbar is an absolute placement, including release.
            let raw_offset = state.offset_start
                + ((pointer - state.pointer_start) / available_track) * state.max_offset;
            let mut next_position = motion.position();
            match state.axis {
                DragAxis::Vertical => next_position.y = raw_offset,
                DragAxis::Horizontal => next_position.x = raw_offset,
            }
            let current = motion.offset();
            motion.scroll_directly(Point::new(
                next_position.x - current.x,
                next_position.y - current.y,
            ));
            EventResult::Handled
        }),
    );

    let track_color = if touch_indicator {
        Color::CLEAR
    } else {
        theme_for_node(node).colors.surface_variant
    };
    let thumb_color = if touch_indicator {
        theme_for_node(node)
            .colors
            .on_surface_muted
            .multiplied_alpha(0.6)
    } else {
        theme_for_node(node).colors.on_surface_muted
    };
    let paint_node = node.downgrade();
    let paint_observers = view.clone_for_report();
    node.update_overlay_draw(
        PaintIdentity {
            axes,
            touch_indicator,
            track_bits: color_bits(track_color),
            thumb_bits: color_bits(thumb_color),
        },
        move |list, origin| {
            let Some(owner) = Node::upgrade(&paint_node) else {
                return;
            };
            let viewport = visible_viewport_rect(&owner);
            let viewport_size = size_of(viewport);
            let content = cached_scrollable_content_size(&owner);
            if content.width <= 0.0 && content.height <= 0.0 {
                return;
            }
            let max_x = (content.width - viewport_size.width).max(0.0);
            let max_y = (content.height - viewport_size.height).max(0.0);
            if let Some(motion) = owner.attachment::<Rc<ScrollAnimator>>(MOTION) {
                motion.update(Point::new(max_x, max_y), size_of(viewport));
            } else {
                let offset = owner.content_offset();
                if offset.x > max_x || offset.y > max_y {
                    owner.set_content_offset(Point::new(offset.x.min(max_x), offset.y.min(max_y)));
                }
            }
            let opacity = owner.attachment::<f32>(CHROME_OPACITY).unwrap_or(0.0);
            if opacity <= 0.001 {
                return;
            }
            let offset = owner.content_offset();
            let visible_track = track_color.multiplied_alpha(opacity);
            let visible_thumb = thumb_color.multiplied_alpha(opacity);
            // Sub-point overflow is layout rounding, not content to scroll to;
            // drawing a bar for it puts a scrollbar under every fitting strip.
            let overflow_tolerance = 0.5;
            if matches!(axes, ScrollAxis::Vertical | ScrollAxis::Both)
                && content.height > viewport.height + overflow_tolerance
                && viewport.height > 0.0
            {
                let track_x =
                    origin.x + viewport.x + viewport.width - track_thickness - track_inset;
                let track_y = origin.y + viewport.y + track_inset;
                let track_height = viewport.height - 2.0 * track_inset;
                let _ = list.add_rounded_rect(
                    Rect::new(track_x, track_y, track_thickness, track_height),
                    track_thickness / 2.0,
                    visible_track,
                );
                let max_off = content.height - viewport.height;
                let shrink = thumb_shrink(offset.y, max_off, viewport.height);
                let thumb_height = track_thickness.max(
                    (track_thickness * 2.0).max(track_height * (viewport.height / content.height))
                        * shrink,
                );
                let fraction = thumb_fraction(offset.y, max_off);
                let thumb_y = track_y + (track_height - thumb_height) * fraction;
                let _ = list.add_rounded_rect(
                    Rect::new(track_x, thumb_y, track_thickness, thumb_height),
                    track_thickness / 2.0,
                    visible_thumb,
                );
            }
            if matches!(axes, ScrollAxis::Horizontal | ScrollAxis::Both)
                && content.width > viewport.width + overflow_tolerance
                && viewport.width > 0.0
            {
                let track_y =
                    origin.y + viewport.y + viewport.height - track_thickness - track_inset;
                let track_x = origin.x + viewport.x + track_inset;
                let track_width = viewport.width - 2.0 * track_inset;
                let _ = list.add_rounded_rect(
                    Rect::new(track_x, track_y, track_width, track_thickness),
                    track_thickness / 2.0,
                    visible_track,
                );
                let max_off = content.width - viewport.width;
                let shrink = thumb_shrink(offset.x, max_off, viewport.width);
                let thumb_width = track_thickness.max(
                    (track_thickness * 2.0).max(track_width * (viewport.width / content.width))
                        * shrink,
                );
                let fraction = thumb_fraction(offset.x, max_off);
                let thumb_x = track_x + (track_width - thumb_width) * fraction;
                let _ = list.add_rounded_rect(
                    Rect::new(thumb_x, track_y, thumb_width, track_thickness),
                    track_thickness / 2.0,
                    visible_thumb,
                );
            }
            report(&owner, &paint_observers);
        },
    );
}

fn color_bits(color: Color) -> (u32, u32, u32, u32) {
    (
        color.r.to_bits(),
        color.g.to_bits(),
        color.b.to_bits(),
        color.a.to_bits(),
    )
}

impl View for ScrollViewObservers {
    fn body(&self) -> AnyView {
        AnyView::new(crate::EmptyView)
    }
}

impl Clone for ScrollViewObservers {
    fn clone(&self) -> Self {
        Self {
            scroll_offset: self.scroll_offset.clone(),
            on_viewport_change: self.on_viewport_change.clone(),
            on_geometry_change: self.on_geometry_change.clone(),
            accessibility_label: self.accessibility_label.clone(),
            combines_accessibility: self.combines_accessibility,
            is_scroll_enabled: self.is_scroll_enabled,
            axes: self.axes,
        }
    }
}

impl ScrollViewObservers {
    fn clone_for_report(&self) -> Self {
        self.clone()
    }
}

/// Read-only scrolling content needs a keyboard entrance. Interactive content
/// already provides one; an extra ancestor stop would steal modal/menu focus.
pub struct ScrollKeyboardFocus;

impl ScrollKeyboardFocus {
    pub fn update(node: &Node, can_scroll: bool) {
        let frame = node.frame();
        let has_viewport = can_scroll && frame.width > 0.5 && frame.height > 0.5;
        let focusable = has_viewport && !Self::has_control(node);
        node.set_is_focusable(focusable);
        if !focusable
            && FocusChainHolder::current()
                .is_some_and(|chain| chain.focused().is_some_and(|focused| focused == *node))
            && let Some(chain) = FocusChainHolder::current()
        {
            chain.clear();
        }
        let _ = KEYBOARD_FOCUS;
    }

    fn has_control(node: &Node) -> bool {
        let mut pending = node.children();
        while let Some(child) = pending.pop() {
            if !child.accepts_subtree_input() {
                continue;
            }
            if child.is_focusable() && child.is_tab_stop() {
                return true;
            }
            pending.extend(child.children());
        }
        false
    }
}

impl PrimitiveView for ScrollView {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node.set_clips_to_bounds(true);
        node.set_is_scroll_viewport(self.axes != ScrollAxis::Horizontal);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_overflow(Overflow::Hidden);
        layout.set_min_width(Some(0.0));
        layout.set_min_height(Some(0.0));
        // Gutter padding depends on the surface input and is set in `update_node`.
    }

    fn update_node(&self, node: &Node) {
        node.set_is_scroll_viewport(self.axes != ScrollAxis::Horizontal);
        let observers = ScrollViewObservers {
            scroll_offset: self.scroll_offset.clone(),
            on_viewport_change: self.on_viewport_change.clone(),
            on_geometry_change: self.on_geometry_change.clone(),
            accessibility_label: self.accessibility_label.clone(),
            combines_accessibility: self.combines_accessibility,
            is_scroll_enabled: self.is_scroll_enabled,
            axes: self.axes,
        };
        let report_observers = observers.clone_for_report();
        let report_handler: Rc<dyn Fn(&Node)> = Rc::new(move |owner: &Node| {
            report(owner, &report_observers);
        });
        node.set_attachment(REPORT, report_handler);
        update_accessibility(node, &observers);

        let increment_node = node.downgrade();
        let increment_observers = observers.clone_for_report();
        let decrement_node = node.downgrade();
        let decrement_observers = observers.clone_for_report();
        let axes = self.axes;
        node.set_accessibility_actions(AccessibilityActions {
            increment: Some(Rc::new(move || {
                if let Some(owner) = Node::upgrade(&increment_node) {
                    page_scroll(&owner, axes, true);
                    report(&owner, &increment_observers);
                }
            })),
            decrement: Some(Rc::new(move || {
                if let Some(owner) = Node::upgrade(&decrement_node) {
                    page_scroll(&owner, axes, false);
                    report(&owner, &decrement_observers);
                }
            })),
            ..Default::default()
        });

        if let Some(binding) = self.scroll_offset.as_ref() {
            let desired = binding.get();
            if desired != node.content_offset() {
                // The child tree still has its previous layout at this point. A
                // new binding offset may target newly grown content, so adopting
                // it against that old range would discard the requested position.
                node.set_attachment(PENDING_OFFSET, desired);
                node.set_content_offset(desired);
            }
        }
        if let Some(request) = self.scroll_request.as_ref()
            && node.attachment::<u64>(TIMED_REQUEST_ID) != Some(request.id)
        {
            node.set_attachment(TIMED_REQUEST_ID, request.id);
            node.set_attachment(TIMED_REQUEST, request.clone());
        }

        install_interaction(
            node,
            &observers,
            self.wheel_step,
            self.consume_policy,
            self.scrollbar_gutter,
        );
        if self.clips_only_overflow {
            let content = cached_scrollable_content_size(node);
            let viewport = visible_viewport_rect(node);
            let overflows =
                content.width - viewport.width > 0.5 || content.height - viewport.height > 0.5;
            node.set_clips_to_bounds(overflows);
        }
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        if self.axes == ScrollAxis::Vertical {
            return vec![self.content.clone()];
        }
        // Yoga right-aligns an oversized RTL child at a negative x. Scrolling
        // uses physical offsets starting at zero, so normalize that origin
        // while retaining the content's inherited logical direction.
        let container = AnyView::new(
            LayoutContainer::<HorizontalScrollOrigin, AnyView>::with_erased_content(
                HorizontalScrollOrigin,
                self.content.clone(),
            ),
        );
        let sized = AnyView::new(container.frame(
            Some(FrameDimension::Percent(100.0)),
            None,
            None,
            None,
            None,
            None,
        ));
        vec![AnyView::new(sized.layout_role(HORIZONTAL_ORIGIN_ROLE))]
    }
}
impl_primitive_view!(ScrollView);
