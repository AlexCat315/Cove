//! Translation of Primitives/Overlays/Tooltip.swift.
//!
//! An arbitrary target and a non-interactive description, presented outside
//! clipping through the portal. Window-wide rules live in `TooltipArbitration`:
//! one tip at a time, the innermost target wins, and a tip that follows another
//! within the warm window skips the rest delay so the pointer can travel along a
//! toolbar.
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use crate::impl_primitive_view;
use crate::modifiers::layout::LayoutFrameExt;
use crate::primitives::overlays::portal::{PortalPlacementMode, PortalResource, PortalStoreHolder};
use cove_composition::{AnyView, PrimitiveView, View};
use cove_core::scene::{
    Align, AnimatorScheduler, EventResult, FocusChainHolder, HoverPhase, InputHandlerRoute,
    InteractionRegistryHolder, LayoutNode, Node, NodeResource, PointerPhase,
};
use cove_core::{Point, Rect, Size};

/// How long the pointer must rest before a tip appears.
pub const DEFAULT_DELAY: f64 = 0.6;
/// A tip that appeared this soon before the next target shows at once.
pub const WARM_WINDOW: f64 = 0.4;
/// Node attachment published by `Text` when it had to clip.
pub const TEXT_TRUNCATED: &str = "__text_truncated";

/// Authored tooltip options.
#[derive(Clone, Debug, PartialEq)]
pub struct TooltipOptions {
    pub delay: f64,
    pub max_width: f32,
    pub gap: f32,
    pub is_enabled: bool,
    /// Labels that are fully visible repeat what the person already reads. Set by
    /// `Text::truncated` so the description appears only when the node actually
    /// clipped its text.
    pub only_when_truncated: bool,
}

impl Default for TooltipOptions {
    fn default() -> Self {
        Self {
            delay: DEFAULT_DELAY,
            max_width: 320.0,
            gap: 6.0,
            is_enabled: true,
            only_when_truncated: false,
        }
    }
}

impl TooltipOptions {
    pub fn validate(&mut self) {
        self.delay = if self.delay.is_finite() {
            self.delay.max(0.0)
        } else {
            DEFAULT_DELAY
        };
        self.max_width = if self.max_width.is_finite() {
            self.max_width.max(80.0)
        } else {
            320.0
        };
        self.gap = if self.gap.is_finite() {
            self.gap.max(0.0)
        } else {
            6.0
        };
    }

    pub fn validated(mut self) -> Self {
        self.validate();
        self
    }
}

/// Where an explicitly anchored tip points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TooltipAnchor {
    Point(Point),
    Range(Rect),
}

impl TooltipAnchor {
    pub fn rect(self) -> Rect {
        match self {
            Self::Point(point) => Rect::new(point.x, point.y, 0.0, 0.0),
            Self::Range(rect) => rect,
        }
    }
}

/// Window-wide tooltip rules.
///
/// Owned by the window's input context, like the portal store: two windows must
/// not take tips from each other, and a fresh context starts with none on screen.
/// The holder is empty until a window registers one, so a bare tree simply has no
/// arbitration and every tip times out on its own.
#[derive(Default)]
pub struct TooltipArbitration {
    sessions: RefCell<Vec<Weak<TooltipSession>>>,
    last_shown: Cell<f64>,
    is_notifying: Cell<bool>,
}

thread_local! {
    static CURRENT_ARBITRATION: RefCell<Option<Rc<TooltipArbitration>>> = const { RefCell::new(None) };
}

impl TooltipArbitration {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            last_shown: Cell::new(f64::NEG_INFINITY),
            ..Default::default()
        })
    }

    /// The active window's rules; `None` until a window registers one.
    pub fn current() -> Option<Rc<Self>> {
        CURRENT_ARBITRATION.with(|slot| slot.borrow().clone())
    }

    /// Register a window's rules for as long as its input context is current.
    pub fn activate(arbitration: Option<Rc<Self>>) -> Box<dyn FnOnce()> {
        let previous = CURRENT_ARBITRATION.with(|slot| slot.replace(arbitration));
        Box::new(move || {
            CURRENT_ARBITRATION.with(|slot| *slot.borrow_mut() = previous);
        })
    }

    pub fn register(&self, session: &Rc<TooltipSession>) {
        self.prune();
        let mut sessions = self.sessions.borrow_mut();
        if !sessions
            .iter()
            .any(|entry| entry.ptr_eq(&Rc::downgrade(session)))
        {
            sessions.push(Rc::downgrade(session));
        }
    }

    pub fn unregister(&self, session: &Rc<TooltipSession>) {
        let target = Rc::downgrade(session);
        self.sessions
            .borrow_mut()
            .retain(|entry| entry.strong_count() > 0 && !entry.ptr_eq(&target));
    }

    /// A tip went up within the warm window, so the next one needs no rest.
    pub fn is_warm(&self) -> bool {
        self.prune();
        now() - self.last_shown.get() <= WARM_WINDOW
    }

    pub fn note_shown(&self) {
        self.last_shown.set(now());
    }

    /// An enabled target inside this session's element is waiting for its own tip,
    /// so this one must stay quiet: over nested targets the inner wins.
    pub fn has_inner_active(&self, session: &TooltipSession) -> bool {
        self.prune();
        let Some(node) = session.anchor_node() else {
            return false;
        };
        self.live_sessions().iter().any(|other| {
            !std::ptr::eq(other.as_ref(), session)
                && other.is_active()
                && other
                    .anchor_node()
                    .is_some_and(|other_node| is_descendant(&other_node, &node))
        })
    }

    /// A new tip takes the screen from any other that is still up.
    pub fn hide_others(&self, session: &TooltipSession) {
        for other in self.live_sessions() {
            if !std::ptr::eq(other.as_ref(), session) && other.is_presented() {
                other.hide();
            }
        }
    }

    /// After a tip goes, a target that is still hovered may now own the screen.
    pub fn reconsider(&self, session: &TooltipSession) {
        if self.is_notifying.get() {
            return;
        }
        self.is_notifying.set(true);
        for other in self.live_sessions() {
            if !std::ptr::eq(other.as_ref(), session) {
                other.reconsider();
            }
        }
        self.is_notifying.set(false);
    }

    pub fn reset(&self) {
        self.sessions.borrow_mut().clear();
        self.last_shown.set(f64::NEG_INFINITY);
        self.is_notifying.set(false);
    }

    fn prune(&self) {
        self.sessions
            .borrow_mut()
            .retain(|entry| entry.strong_count() > 0);
    }

    fn live_sessions(&self) -> Vec<Rc<TooltipSession>> {
        self.prune();
        self.sessions
            .borrow()
            .iter()
            .filter_map(Weak::upgrade)
            .collect()
    }
}

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |elapsed| elapsed.as_secs_f64())
}

fn is_descendant(node: &Node, ancestor: &Node) -> bool {
    let mut parent = node.parent();
    while let Some(candidate) = parent {
        if candidate == *ancestor {
            return true;
        }
        parent = candidate.parent();
    }
    false
}

/// The rest/dismiss/hover state machine behind one target.
pub struct TooltipSession {
    node: RefCell<Option<Node>>,
    content: RefCell<Option<AnyView>>,
    options: RefCell<TooltipOptions>,
    hovered: Cell<bool>,
    focused: Cell<bool>,
    elapsed: Cell<f64>,
    /// When the pointer arrived: a target that waited behind a nested one still
    /// measures its rest from here, so it opens as soon as it wins.
    hover_since: Cell<f64>,
    presented: Cell<bool>,
    dismissed_until_leave: Cell<bool>,
    /// The window that mounted this tip. Unmount must leave *that* window's list
    /// even if the ambient has already moved on to another one.
    owner: RefCell<Option<Rc<TooltipArbitration>>>,
    /// Re-evaluated before the tip appears, not just when the pointer enters.
    can_show: RefCell<Rc<dyn Fn() -> bool>>,
    is_finished: Cell<bool>,
    self_weak: RefCell<Weak<TooltipSession>>,
}

impl Default for TooltipSession {
    fn default() -> Self {
        Self {
            node: RefCell::new(None),
            content: RefCell::new(None),
            options: RefCell::new(TooltipOptions::default()),
            hovered: Cell::new(false),
            focused: Cell::new(false),
            elapsed: Cell::new(0.0),
            hover_since: Cell::new(0.0),
            presented: Cell::new(false),
            dismissed_until_leave: Cell::new(false),
            owner: RefCell::new(None),
            can_show: RefCell::new(Rc::new(|| true)),
            is_finished: Cell::new(true),
            self_weak: RefCell::new(Weak::new()),
        }
    }
}

impl NodeResource for TooltipSession {
    fn mount(&self, node: &Node) {
        *self.node.borrow_mut() = Some(node.clone());
        *self.owner.borrow_mut() = TooltipArbitration::current();
        if let Some(arbitration) = self.arbitration() {
            let weak = self.me_weak();
            if let Some(session) = weak.upgrade() {
                arbitration.register(&session);
            }
        }
    }

    fn unmount(&self, _node: &Node) {
        self.hide();
        if let Some(arbitration) = self.arbitration()
            && let Some(session) = self.me_weak().upgrade()
        {
            arbitration.unregister(&session);
        }
        *self.owner.borrow_mut() = None;
        *self.node.borrow_mut() = None;
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}

impl TooltipSession {
    pub fn new() -> Rc<Self> {
        let session = Rc::new(Self::default());
        *session.self_weak.borrow_mut() = Rc::downgrade(&session);
        session
    }

    fn arbitration(&self) -> Option<Rc<TooltipArbitration>> {
        self.owner
            .borrow()
            .clone()
            .or_else(TooltipArbitration::current)
    }

    pub fn anchor_node(&self) -> Option<Node> {
        self.node.borrow().clone()
    }

    pub fn is_presented(&self) -> bool {
        self.presented.get()
    }

    pub fn is_finished(&self) -> bool {
        self.is_finished.get()
    }

    /// Hovered (or keyboard-focused) with something worth showing.
    pub fn is_active(&self) -> bool {
        let options = self.options.borrow();
        options.is_enabled
            && (self.hovered.get() || self.focused.get())
            && (!options.only_when_truncated || self.shows_truncated_text())
            && (self.can_show.borrow())()
    }

    pub fn configure(
        &self,
        content: AnyView,
        options: TooltipOptions,
        focused: bool,
        can_show: Rc<dyn Fn() -> bool>,
    ) {
        *self.content.borrow_mut() = Some(content);
        *self.options.borrow_mut() = options;
        self.focused.set(focused);
        *self.can_show.borrow_mut() = can_show;
        self.refresh();
    }

    pub fn set_hovered(&self, hovered: bool) {
        if hovered && !self.hovered.get() {
            self.hover_since.set(now());
        }
        self.hovered.set(hovered);
        self.refresh();
    }

    pub fn set_focused(&self, focused: bool) {
        self.focused.set(focused);
        self.refresh();
    }

    fn refresh(&self) {
        let options = self.options.borrow().clone();
        let shows_text = !options.only_when_truncated || self.shows_truncated_text();
        if !(options.is_enabled && (self.hovered.get() || self.focused.get()) && shows_text)
            || !(self.can_show.borrow())()
        {
            self.dismissed_until_leave.set(false);
            self.hide();
            return;
        }
        if self.dismissed_until_leave.get() {
            return;
        }
        // Over elements inside one another only the inner tip shows.
        if self
            .arbitration()
            .is_some_and(|arbitration| arbitration.has_inner_active(self))
        {
            self.hide();
            return;
        }
        if self.presented.get() {
            self.show();
            return;
        }
        if !self.is_finished.get() {
            return;
        }
        self.elapsed.set(if self.hovered.get() {
            (now() - self.hover_since.get()).max(0.0)
        } else {
            0.0
        });
        // A tip that follows another within the warm window skips the rest, so a
        // run along a toolbar does not wait again. Keyboard focus does *not* skip
        // it: focus lands on a trigger the instant a menu closes, and a chip
        // covering the bar the person just used reads as an accident.
        let warm = self
            .arbitration()
            .is_some_and(|arbitration| arbitration.is_warm());
        if options.delay == 0.0 || warm || self.elapsed.get() >= options.delay {
            self.show();
        } else {
            self.is_finished.set(false);
            AnimatorScheduler::current().register(Rc::new(ClockRef {
                session: self.me_weak(),
            }));
        }
    }

    /// A tip that went may let a still-hovered target show without moving the
    /// pointer onto it again.
    pub fn reconsider(&self) {
        if self.presented.get() {
            return;
        }
        self.refresh();
    }

    /// A truncated label marks its node while drawing. Text that fits keeps the
    /// tooltip hidden, so hovering ordinary labels stays quiet.
    fn shows_truncated_text(&self) -> bool {
        self.node
            .borrow()
            .as_ref()
            .is_some_and(|node| node.attachment::<bool>(TEXT_TRUNCATED) == Some(true))
    }

    /// Advance the rest timer. Returns `true` while it still needs ticks.
    pub fn tick(&self, delta_time: f64) -> bool {
        if self.is_finished.get() {
            return false;
        }
        self.elapsed.set(self.elapsed.get() + delta_time.max(0.0));
        if self.elapsed.get() >= self.options.borrow().delay {
            self.is_finished.set(true);
            if (self.can_show.borrow())() {
                self.show();
            } else {
                self.hide();
            }
            return false;
        }
        true
    }

    fn show(&self) {
        let Some(node) = self.node.borrow().clone() else {
            return;
        };
        if !node.accepts_subtree_input() {
            return;
        }
        let Some(content) = self.content.borrow().clone() else {
            return;
        };
        if !(self.can_show.borrow())() {
            return;
        }
        let Some(arbitration) = self.arbitration() else {
            return;
        };
        if arbitration.has_inner_active(self) {
            return;
        }
        self.presented.set(true);
        // One tip at a time: anything already on screen gives way to this one.
        arbitration.hide_others(self);
        arbitration.note_shown();
        let anchor = node.absolute_frame();
        let gap = self.options.borrow().gap;
        let window = window_bounds(&node);
        let options = self.options.borrow().clone();
        let sized_content = AnyView::new(ContentBox {
            content: AnyView::new(content.frame(
                None,
                None,
                None,
                None,
                Some(options.max_width),
                None,
            )),
        });
        let position_for_size =
            Rc::new(move |size: Size| Self::resting_spot(size, anchor, gap, window));
        if let Some(resource) = portal_resource(&node) {
            resource.present(
                Some(PortalStoreHolder::current()),
                Self::resting_spot(Size::default(), anchor, gap, window),
                None,
                sized_content,
                false,
                PortalPlacementMode::Fixed,
                Some(position_for_size),
                None,
            );
            let anchor_node = node.clone();
            resource.set_dismissal(
                Rc::new(move || anchor_node.absolute_frame()),
                Rc::new({
                    let weak = self.me_weak();
                    move || {
                        if let Some(session) = weak.upgrade() {
                            session.dismiss();
                        }
                    }
                }),
            );
        }
    }

    /// Centered above the anchor, like the reference engine. The chip's own size
    /// decides where it rests, and it only drops below when the top of the window
    /// would clip.
    fn resting_spot(size: Size, anchor: Rect, gap: f32, window: Rect) -> Point {
        let above = anchor.y - gap - size.height;
        let below = anchor.y + anchor.height + gap;
        let y = if above >= window.y {
            above
        } else if below + size.height <= window.y + window.height {
            below
        } else {
            above
        };
        Point::new(anchor.x + anchor.width / 2.0 - size.width / 2.0, y)
    }

    pub fn update_position(&self) {
        let Some(node) = self.node.borrow().clone() else {
            return;
        };
        if !node.accepts_subtree_input() {
            self.dismiss();
            return;
        }
        if !self.presented.get() {
            return;
        }
        let anchor = node.absolute_frame();
        let gap = self.options.borrow().gap;
        let spot = Self::resting_spot(Size::default(), anchor, gap, window_bounds(&node));
        if let Some(resource) = portal_resource(&node) {
            resource.update_position(spot);
        }
    }

    /// Returns whether a tip was on screen.
    pub fn dismiss(&self) -> bool {
        let was_presented = self.presented.get();
        self.dismissed_until_leave.set(true);
        self.hide();
        was_presented
    }

    pub fn hide(&self) {
        let was_presented = self.presented.get();
        self.is_finished.set(true);
        self.presented.set(false);
        if let Some(node) = self.node.borrow().clone()
            && let Some(resource) = portal_resource(&node)
        {
            resource.unmount(&node);
        }
        // A target the tip was covering (a nested one that just closed) may now
        // own the screen without the pointer moving onto it again.
        if was_presented && let Some(arbitration) = self.arbitration() {
            arbitration.reconsider(self);
        }
    }

    fn me_weak(&self) -> Weak<TooltipSession> {
        self.self_weak.borrow().clone()
    }
}

fn portal_resource(node: &Node) -> Option<Rc<PortalResource>> {
    node.attachment::<Rc<PortalResource>>(PORTAL_ATTACHMENT)
}

/// The portal resource key the tooltip host publishes.
pub const PORTAL_ATTACHMENT: &str = "tooltip.portal";

fn window_bounds(node: &Node) -> Rect {
    // The window is the root of the node tree.
    let mut root = node.clone();
    while let Some(parent) = root.parent() {
        root = parent;
    }
    root.absolute_frame()
}

/// Renders portal content with hit-testing off so a tip never blocks its target.
struct ContentBox {
    content: AnyView,
}

impl PrimitiveView for ContentBox {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
crate::impl_primitive_view!(ContentBox);

/// A scheduled session tick, so the rest timer only needs the scheduler.
struct ClockRef {
    session: Weak<TooltipSession>,
}

impl cove_core::scene::AnyAnimationController for ClockRef {
    fn is_finished(&self) -> bool {
        self.session
            .upgrade()
            .is_none_or(|session| session.is_finished())
    }

    fn tick(&self, delta_time: f64) {
        if let Some(session) = self.session.upgrade() {
            session.tick(delta_time);
        }
    }

    fn finish_immediately(&self) {
        if let Some(session) = self.session.upgrade() {
            session.hide();
        }
    }

    fn cancel(&self) {
        if let Some(session) = self.session.upgrade() {
            session.hide();
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// The active window's tooltip rules; `None` until a window registers one.
pub struct TooltipArbitrationHolder;

impl TooltipArbitrationHolder {
    pub fn current() -> Option<Rc<TooltipArbitration>> {
        TooltipArbitration::current()
    }
}

/// Registers a window's tooltip rules for as long as its input context is
/// current, mirroring `PortalStoreAmbient`.
pub struct TooltipArbitrationAmbient {
    pub arbitration: Rc<TooltipArbitration>,
}

impl TooltipArbitrationAmbient {
    pub fn new() -> Self {
        Self {
            arbitration: TooltipArbitration::new(),
        }
    }

    pub fn with(arbitration: Rc<TooltipArbitration>) -> Self {
        Self { arbitration }
    }
}

impl Default for TooltipArbitrationAmbient {
    fn default() -> Self {
        Self::new()
    }
}

impl cove_core::scene::ScopedAmbient for TooltipArbitrationAmbient {
    fn activate(&self) -> Box<dyn FnOnce()> {
        TooltipArbitration::activate(Some(self.arbitration.clone()))
    }
}

/// An arbitrary target and non-interactive description, presented outside
/// clipping through the portal.
pub struct Tooltip {
    pub options: TooltipOptions,
    /// Explicitly anchored tips are already debounced by their producer (for
    /// example LSP hover), so presentation follows `is_presented` directly.
    pub anchor: Option<TooltipAnchor>,
    pub is_presented: bool,
    /// Whether the target had to clip its text, so a redundant description stays
    /// quiet.
    pub shows_truncated_text: Rc<dyn Fn() -> bool>,
    target: Option<AnyView>,
    content: AnyView,
}

impl Tooltip {
    pub fn new(target: impl View + 'static, content: impl View + 'static) -> Self {
        Self {
            options: TooltipOptions::default(),
            anchor: None,
            is_presented: true,
            shows_truncated_text: Rc::new(|| true),
            target: Some(AnyView::new(target)),
            content: AnyView::new(content),
        }
    }

    pub fn anchored(anchor: TooltipAnchor, content: impl View + 'static) -> Self {
        Self {
            options: TooltipOptions::default(),
            anchor: Some(anchor),
            is_presented: true,
            shows_truncated_text: Rc::new(|| true),
            target: None,
            content: AnyView::new(content),
        }
    }

    pub fn options(mut self, options: TooltipOptions) -> Self {
        self.options = options.validated();
        self
    }

    pub fn configure(mut self, configure: impl FnOnce(&mut TooltipOptions)) -> Self {
        configure(&mut self.options);
        self.options.validate();
        self
    }

    pub fn presented(mut self, is_presented: bool) -> Self {
        self.is_presented = is_presented;
        self
    }

    pub fn only_when_truncated(mut self, only_when_truncated: bool) -> Self {
        self.options.only_when_truncated = only_when_truncated;
        self
    }

    pub fn shows_truncated_text(mut self, resolve: impl Fn() -> bool + 'static) -> Self {
        self.shows_truncated_text = Rc::new(resolve);
        self
    }
}

impl View for Tooltip {
    fn body(&self) -> AnyView {
        if let Some(anchor) = self.anchor {
            return AnyView::new(CoordinateTooltipHost {
                anchor,
                content: self.content.clone(),
                options: self.options.clone(),
                is_presented: self.is_presented,
            });
        }
        let focus = FocusChainHolder::current();
        let focused = focus.as_ref().and_then(|chain| chain.focused());
        let focus_visible = focus.as_ref().is_some_and(|chain| chain.is_focus_visible());
        AnyView::new(TooltipHost {
            target: self
                .target
                .clone()
                .unwrap_or_else(|| AnyView::new(crate::EmptyView)),
            content: self.content.clone(),
            options: self.options.clone(),
            focused,
            focus_visible,
            shows_truncated_text: self.shows_truncated_text.clone(),
        })
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Explicitly anchored tip: presentation follows `is_presented` directly.
struct CoordinateTooltipHost {
    anchor: TooltipAnchor,
    content: AnyView,
    options: TooltipOptions,
    is_presented: bool,
}

impl PrimitiveView for CoordinateTooltipHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.add_resource(PortalResource::new());
        node.set_attachment("tooltip.coordinate", true);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        let available = node.accepts_subtree_input();
        node.set_attachment(COORDINATE_AVAILABLE, available);
        let host = CoordinateTooltipHost {
            anchor: self.anchor,
            content: self.content.clone(),
            options: self.options.clone(),
            is_presented: self.is_presented,
        };
        node.set_layout_did_update(Some(Rc::new(move |node: &Node| {
            let next = node.accepts_subtree_input();
            if node.attachment::<bool>(COORDINATE_AVAILABLE) == Some(next) {
                return;
            }
            node.set_attachment(COORDINATE_AVAILABLE, next);
            host.present(node);
        })));
        self.present(node);
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        Vec::new()
    }
}
impl_primitive_view!(CoordinateTooltipHost);

const COORDINATE_AVAILABLE: &str = "CoveUI.tooltip.coordinate.available";

impl CoordinateTooltipHost {
    fn present(&self, node: &Node) {
        let Some(resource) = node.first_resource::<PortalResource>() else {
            return;
        };
        if !node.accepts_subtree_input() || !self.is_presented || !self.options.is_enabled {
            resource.unmount(node);
            return;
        }
        let anchor = self.anchor.rect();
        let options = self.options.clone();
        let content = AnyView::new(ContentBox {
            content: AnyView::new(SizeLimited {
                content: self.content.clone(),
                max_width: options.max_width,
            }),
        });
        resource.present(
            Some(PortalStoreHolder::current()),
            Point::new(anchor.x, anchor.y + anchor.height + options.gap),
            None,
            content,
            false,
            PortalPlacementMode::Fixed,
            Some(Rc::new(move |_size: Size| {
                Point::new(anchor.x, anchor.y + anchor.height + options.gap)
            })),
            None,
        );
        let target = anchor;
        resource.set_dismissal(Rc::new(move || target), Rc::new(|| {}));
    }
}

/// Pointer/focus host: owns the portal resource and the arbitration session.
struct TooltipHost {
    target: AnyView,
    content: AnyView,
    options: TooltipOptions,
    focused: Option<Node>,
    focus_visible: bool,
    shows_truncated_text: Rc<dyn Fn() -> bool>,
}

impl PrimitiveView for TooltipHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node.add_resource(PortalResource::new());
        node.set_attachment(
            PORTAL_ATTACHMENT,
            node.first_resource::<PortalResource>()
                .expect("portal resource"),
        );
        node.set_attachment("tooltip.host", true);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_align_items(Align::Stretch);
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        let session = node.first_resource::<TooltipSession>().unwrap_or_else(|| {
            let session = TooltipSession::new();
            node.add_resource(session.clone());
            session
        });
        let in_subtree = self.focused.as_ref().is_some_and(|focused| {
            FocusChainHolder::current().is_some_and(|chain| chain.contains(focused, node))
        });
        let truncated = self.shows_truncated_text.clone();
        session.configure(
            self.content.clone(),
            self.options.clone(),
            self.focus_visible && in_subtree,
            Rc::new(move || truncated()),
        );
        if !supports_control_hover(node) {
            session.set_hovered(false);
        }
        if let Some(registry) = InteractionRegistryHolder::current() {
            let hover_session = Rc::downgrade(&session);
            let hover_owner = node.downgrade();
            registry.set_hover(
                node,
                Rc::new(move |phase: HoverPhase| {
                    let Some(session) = hover_session.upgrade() else {
                        return;
                    };
                    let allowed = Node::upgrade(&hover_owner)
                        .is_some_and(|owner| supports_control_hover(&owner));
                    session.set_hovered(phase == HoverPhase::Enter && allowed);
                }),
            );
            let pointer_session = Rc::downgrade(&session);
            registry.set_pointer(
                node,
                InputHandlerRoute::control(),
                Rc::new(move |_, phase, _| {
                    if phase == PointerPhase::Down
                        && let Some(session) = pointer_session.upgrade()
                    {
                        session.dismiss();
                    }
                    EventResult::Ignored
                }),
            );
            let key_session = Rc::downgrade(&session);
            registry.set_key(
                node,
                InputHandlerRoute::control(),
                Rc::new(move |event, _| {
                    if event.scancode != crate::Scancode::ESCAPE {
                        return EventResult::Ignored;
                    }
                    key_session
                        .upgrade()
                        .is_some_and(|session| session.dismiss())
                        .then_some(EventResult::Handled)
                        .unwrap_or(EventResult::Ignored)
                }),
            );
        }
        let layout_session = Rc::downgrade(&session);
        node.set_layout_did_update(Some(Rc::new(move |_node: &Node| {
            if let Some(session) = layout_session.upgrade() {
                session.update_position();
            }
        })));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.target.clone()]
    }
}
impl_primitive_view!(TooltipHost);

fn supports_control_hover(node: &Node) -> bool {
    crate::primitives::controls::adaptive::supports_control_hover(node)
}

/// Bounds a chip's width while keeping its measured height.
struct SizeLimited {
    content: AnyView,
    max_width: f32,
}

impl PrimitiveView for SizeLimited {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_max_width(Some(self.max_width));
        Some(layout)
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_max_width(Some(self.max_width));
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(SizeLimited);
