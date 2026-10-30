//! Translation of Primitives/Navigation/NavigationStack.swift.
//!
//! Session-owned page stack. The caller owns `path`; pushing appends a route,
//! popping removes the last one, and a non-empty initial path restores a deep
//! link on the first frame.
use std::cell::{Cell, RefCell};
use std::hash::Hash;
use std::rc::Rc;

use cove_core::scene::{
    Align, Direction, EventResult, FlexDirection, FocusChainHolder, InputHandlerRole,
    InputHandlerRoute, InputRoutingPriority, InteractionRegistryHolder, LayoutNode, Node,
    PointerCaptureHolder, PointerPhase,
};
use cove_core::{Binding, Point, Rect};
use cove_platform::MouseButton;
use crate::adaptive::{ContainerSizeReader, SurfaceEnvironment};
use crate::animation::content::AnimatedVisibility;
use crate::animation::transition::Transition;
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt};
use crate::modifiers::style::AppearanceExt;
use crate::primitives::layout::ResolvedLayoutDirection;
use crate::theme::{Theme, ThemeReader};
use crate::{impl_primitive_view, AnyView, PrimitiveView, View};

const PATH_COUNT: &str = "navigation.path.count";
const PAGE_IS_TOP: &str = "navigation.page.isTop";
const PAGE_TITLE_FOCUSED: &str = "navigation.page.titleFocused";
const EDGE_TRACK: &str = "navigation.edge.track";
const EDGE_WIDTH: f32 = 24.0;

/// Edge-swipe tracker for one leading-edge touch.
struct EdgeTrack {
    start_x: f32,
    start_y: f32,
    sample_x: f32,
    sample_time: f64,
    velocity: f32,
    dragging: Cell<bool>,
}

impl EdgeTrack {
    fn new(local: Point) -> Self {
        Self {
            start_x: local.x,
            start_y: local.y,
            sample_x: local.x,
            sample_time: now(),
            velocity: 0.0,
            dragging: Cell::new(false),
        }
    }

    /// Inward speed over windows of at least 50 ms, so events delivered in one
    /// burst do not read as a flick, and a pause before release does.
    fn record(&mut self, x: f32, sign: f32) {
        let time = now();
        let elapsed = time - self.sample_time;
        if elapsed > 0.15 {
            self.velocity = 0.0;
            self.sample_x = x;
            self.sample_time = time;
        } else if elapsed >= 0.05 {
            self.velocity = (x - self.sample_x) * sign / elapsed as f32;
            self.sample_x = x;
            self.sample_time = time;
        }
    }
}

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64())
        .unwrap_or(0.0)
}

/// Session-owned page stack.
pub struct NavigationStack<Route: Hash + Clone + PartialEq + Send + Sync + 'static> {
    pub path: Binding<Vec<Route>>,
    /// Horizontal travel of the page transition in logical points.
    pub transition_distance: f32,
    root: Rc<dyn Fn() -> AnyView>,
    destination: Rc<dyn Fn(Route) -> AnyView>,
    last_count: crate::State<usize>,
    last_routes: crate::State<Vec<Route>>,
    is_animating: crate::State<bool>,
    swipe: crate::State<f32>,
    is_swiping: crate::State<bool>,
}

pub struct NavigationPages {
    pub width: f32,
    pub height: f32,
    pub sign: f32,
    pub forward: bool,
}

impl<Route: Hash + Clone + PartialEq + Send + Sync + 'static> NavigationStack<Route> {
    pub fn new(
        path: Binding<Vec<Route>>,
        root: impl Fn() -> AnyView + 'static,
        destination: impl Fn(Route) -> AnyView + 'static,
    ) -> Self {
        let initial = path.get();
        Self {
            path,
            transition_distance: 32.0,
            root: Rc::new(root),
            destination: Rc::new(destination),
            last_count: crate::State::new(initial.len()),
            last_routes: crate::State::new(initial),
            is_animating: crate::State::new(false),
            swipe: crate::State::new(0.0_f32),
            is_swiping: crate::State::new(false),
        }
    }

    pub fn transition_distance(mut self, distance: f32) -> Self {
        self.transition_distance = if distance.is_finite() { distance.max(0.0) } else { 32.0 };
        self
    }

    fn page(
        &self,
        route: Option<Route>,
        index: usize,
        depth: usize,
        sign: f32,
        forward: bool,
        theme: &Theme,
        width: f32,
        height: f32,
    ) -> AnyView {
        let is_top = depth == 0;
        let revealed = self.is_swiping.get() && depth == 1;
        let transition = if revealed {
            Transition::identity()
        } else {
            Transition::navigation(self.transition_distance, forward)
        };
        let swipe = self.swipe.get();
        let shift = if is_top {
            swipe
        } else if revealed {
            (swipe - width) * 0.3
        } else {
            0.0
        };
        let animating = self.is_animating.clone();
        let content = self.page_content(route.clone(), theme, width, height);
        let is_animating_flag = animating.get();
        let page = AnimatedVisibility::new(
            is_top || revealed,
            transition,
            None,
            index > 0,
            Some(Rc::new(move |visible| {
                if visible && is_animating_flag {
                    animating.set(false);
                }
            })),
            AnyView::new(NavigationPageFocusHost { is_top, content }),
        )
        .retaining_content(true);
        AnyView::new(crate::animation::geometry::offset(page, shift * sign, 0.0))
    }

    fn page_content(&self, route: Option<Route>, theme: &Theme, width: f32, height: f32) -> AnyView {
        let content = match route {
            Some(route) => (self.destination)(route),
            None => (self.root)(),
        };
        AnyView::new(
            content
                .frame(
                    Some(crate::modifiers::layout::FrameDimension::Points(width.max(1.0))),
                    Some(crate::modifiers::layout::FrameDimension::Points(height.max(1.0))),
                    None,
                    None,
                    None,
                    None,
                )
                .background_color(theme.colors.background),
        )
    }

    fn stack_pages(&self, sign: f32, width: f32, height: f32, theme: &Theme) -> AnyView {
        let routes = self.path.get();
        let forward = routes.len() >= self.last_count.get();
        let mut pages: Vec<AnyView> = Vec::new();
        pages.push(AnyView::new(NavigationPathWatcher {
            count: routes.len(),
            on_change: Rc::new({
                let state = self.clone_state();
                move |count, animates| state.observe(count, animates)
            }),
        }));
        pages.push(self.page(None, 0, routes.len(), sign, forward, theme, width, height));
        for (offset, route) in routes.iter().enumerate() {
            pages.push(self.page(
                Some(route.clone()),
                offset + 1,
                routes.len() - offset - 1,
                sign,
                forward,
                theme,
                width,
                height,
            ));
        }
        AnyView::new(
            NavigationEdgeSwipe {
                is_enabled: !routes.is_empty(),
                sign,
                width,
                content: AnyView::new(crate::primitives::layout::ZStack::new(
                    crate::Alignment::TopLeading,
                    AnyView::new(crate::TupleView::new(pages)),
                )),
                on_change: None,
                on_end: None,
            }
            .with_callbacks(
                Rc::new({
                    let state = self.clone_state();
                    move |distance| {
                        if !state.is_swiping.get() {
                            state.is_swiping.set(true);
                        }
                        state.swipe.set(state.width.min(distance));
                    }
                }),
                Rc::new({
                    let state = self.clone_state();
                    let theme = theme.clone();
                    move |commit| state.end_swipe(commit, &theme)
                }),
            ),
        )
    }

    fn clone_state(&self) -> NavigationState<Route> {
        NavigationState {
            path: self.path.clone(),
            last_count: self.last_count.clone(),
            last_routes: self.last_routes.clone(),
            is_animating: self.is_animating.clone(),
            swipe: self.swipe.clone(),
            is_swiping: self.is_swiping.clone(),
            width: 0.0,
        }
    }
}

struct NavigationState<Route: Hash + Clone + PartialEq + Send + Sync + 'static> {
    path: Binding<Vec<Route>>,
    last_count: crate::State<usize>,
    last_routes: crate::State<Vec<Route>>,
    is_animating: crate::State<bool>,
    swipe: crate::State<f32>,
    is_swiping: crate::State<bool>,
    width: f32,
}

impl<Route: Hash + Clone + PartialEq + Send + Sync + 'static> NavigationState<Route> {
    fn observe(&self, count: usize, animates: bool) {
        if count == self.last_count.get() {
            return;
        }
        if self.is_animating.get() {
            if self.path.get() != self.last_routes.get() {
                self.path.set(self.last_routes.get());
            }
            return;
        }
        self.last_routes.set(self.path.get());
        self.last_count.set(count);
        self.is_animating.set(animates);
    }

    fn end_swipe(&self, commit: bool, _theme: &Theme) {
        if commit && !self.path.get().is_empty() {
            // The revealed page is already in place, so the pop has no
            // transition to wait for.
            let mut next = self.path.get();
            next.pop();
            self.last_routes.set(next.clone());
            self.last_count.set(next.len());
            self.swipe.set(0.0);
            self.is_swiping.set(false);
            self.path.set(next);
        } else {
            self.swipe.set(0.0);
            self.is_swiping.set(false);
        }
    }
}

/// Size and direction context resolved by the navigation host.
pub struct NavigationStackHost<Route: Hash + Clone + PartialEq + Send + Sync + 'static> {
    pub stack: Rc<NavigationStack<Route>>,
    pub width: f32,
    pub height: f32,
    pub direction: Direction,
}

impl<Route: Hash + Clone + PartialEq + Send + Sync + 'static> View for NavigationStackHost<Route> {
    fn body(&self) -> AnyView {
        let width = self.width;
        let height = self.height;
        let sign = if self.direction == Direction::Rtl { -1.0 } else { 1.0 };
        let stack = self.stack.clone();
        let inner = AnyView::new(ThemeReader::new(move |theme: &Theme| {
            stack.stack_pages(sign, width, height, theme)
        }));
        AnyView::new(inner.frame_size(width, height))
    }
}

impl<Route: Hash + Clone + PartialEq + Send + Sync + 'static> View for NavigationStack<Route> {
    fn body(&self) -> AnyView {
        let stack = Rc::new(self.snapshot());
        let width_cell = Rc::new(Cell::new(0.0_f32));
        let height_cell = Rc::new(Cell::new(0.0_f32));
        let width = width_cell.clone();
        let height = height_cell.clone();
        let stack_for_size = stack.clone();
        AnyView::new(ContainerSizeReader::new(move |size: cove_core::Size| {
            width.set(size.width);
            height.set(size.height);
            ResolvedLayoutDirection::new({
                let stack = stack_for_size.clone();
                move |direction: Direction| NavigationStackHost {
                    stack: stack.clone(),
                    width: size.width,
                    height: size.height,
                    direction,
                }
            })
        }))
    }
}

impl<Route: Hash + Clone + PartialEq + Send + Sync + 'static> NavigationStack<Route> {
    fn snapshot(&self) -> NavigationStack<Route> {
        NavigationStack {
            path: self.path.clone(),
            transition_distance: self.transition_distance,
            root: self.root.clone(),
            destination: self.destination.clone(),
            last_count: self.last_count.clone(),
            last_routes: self.last_routes.clone(),
            is_animating: self.is_animating.clone(),
            swipe: self.swipe.clone(),
            is_swiping: self.is_swiping.clone(),
        }
    }
}

/// Tracks a touch that starts at the leading edge. Down is only observed, so
/// taps still reach the page; the drag is claimed once it moves inward more than
/// it moves vertically.
struct NavigationEdgeSwipe {
    is_enabled: bool,
    sign: f32,
    width: f32,
    content: AnyView,
    on_change: Option<Rc<dyn Fn(f32)>>,
    on_end: Option<Rc<dyn Fn(bool)>>,
}

impl NavigationEdgeSwipe {
    fn with_callbacks(
        mut self,
        on_change: Rc<dyn Fn(f32)>,
        on_end: Rc<dyn Fn(bool)>,
    ) -> Self {
        self.on_change = Some(on_change);
        self.on_end = Some(on_end);
        self
    }
}

impl PrimitiveView for NavigationEdgeSwipe {
    // Hit-testable so the edge also catches touches over page areas that are not.
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_align_items(Align::Stretch);
        Some(layout)
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
    }

    fn update_node(&self, node: &Node) {
        let Some(registry) = InteractionRegistryHolder::current() else { return };
        let route = InputHandlerRoute::new(
            InputHandlerRole::ScrollChrome, InputRoutingPriority::CAPTURE, "navigation.edge",
        );
        let is_enabled = self.is_enabled;
        let sign = self.sign;
        let width = self.width;
        let on_change = self.on_change.clone();
        let on_end = self.on_end.clone();
        if !is_enabled
            && let Some(track) = node.attachment::<Rc<RefCell<EdgeTrack>>>(EDGE_TRACK)
        {
            let dragging = track.borrow().dragging.get();
            node.remove_attachment(EDGE_TRACK);
            if dragging {
                if PointerCaptureHolder::current()
                    .and_then(|capture| capture.target())
                    .is_some_and(|target| target == *node)
                    && let Some(capture) = PointerCaptureHolder::current()
                {
                    capture.release();
                }
                if let Some(on_end) = on_end.as_ref() {
                    on_end(false);
                }
            }
        }
        let pointer_node = node.downgrade();
        registry.set_pointer(node, route.clone(), Rc::new(move |event, phase, event_phase| {
            if event.button != MouseButton::Left {
                return EventResult::Ignored;
            }
            let Some(owner) = Node::upgrade(&pointer_node) else { return EventResult::Ignored };
            match phase {
                PointerPhase::Down => {
                    if event_phase != cove_core::scene::EventPhase::Capture {
                        return EventResult::Ignored;
                    }
                    owner.remove_attachment(EDGE_TRACK);
                    if !is_enabled {
                        return EventResult::Ignored;
                    }
                    let supports_touch = owner
                        .composition_value(&SurfaceEnvironment::key())
                        .input
                        .contains(cove_core::InputCapabilities::TOUCH);
                    if !supports_touch {
                        return EventResult::Ignored;
                    }
                    let local = owner.convert_from_window(Point::new(event.x, event.y));
                    let inset = if sign < 0.0 { owner.frame().width - local.x } else { local.x };
                    if inset < 0.0 || inset > EDGE_WIDTH {
                        return EventResult::Ignored;
                    }
                    owner.set_attachment(EDGE_TRACK, Rc::new(RefCell::new(EdgeTrack::new(local))));
                    EventResult::Ignored
                }
                PointerPhase::Up => {
                    let Some(track) = owner.attachment::<Rc<RefCell<EdgeTrack>>>(EDGE_TRACK) else {
                        return EventResult::Ignored;
                    };
                    owner.remove_attachment(EDGE_TRACK);
                    if !track.borrow().dragging.get() {
                        return EventResult::Ignored;
                    }
                    if PointerCaptureHolder::current()
                        .and_then(|capture| capture.target())
                        .is_some_and(|target| target == owner)
                        && let Some(capture) = PointerCaptureHolder::current()
                    {
                        capture.release();
                    }
                    let local = owner.convert_from_window(Point::new(event.x, event.y));
                    let start_x = track.borrow().start_x;
                    let travel = (local.x - start_x) * sign;
                    track.borrow_mut().record(local.x, sign);
                    let velocity = track.borrow().velocity;
                    let commit = travel > width / 3.0 || (travel > 0.0 && velocity > 600.0);
                    if let Some(on_end) = on_end.as_ref() {
                        on_end(commit);
                    }
                    EventResult::Handled
                }
            }
        }));
        let motion_node = node.downgrade();
        registry.set_motion(node, route, Rc::new(move |event, _phase| {
            let Some(owner) = Node::upgrade(&motion_node) else { return EventResult::Ignored };
            let Some(track) = owner.attachment::<Rc<RefCell<EdgeTrack>>>(EDGE_TRACK) else {
                return EventResult::Ignored;
            };
            let local = owner.convert_from_window(Point::new(event.x, event.y));
            let (start_x, start_y) = {
                let track = track.borrow();
                (track.start_x, track.start_y)
            };
            let dx = (local.x - start_x) * sign;
            let dy = local.y - start_y;
            if !track.borrow().dragging.get() {
                if dy.abs() > 10.0 && dy.abs() > dx {
                    owner.remove_attachment(EDGE_TRACK);
                    return EventResult::Ignored;
                }
                if !(dx >= 8.0 && dx > dy.abs() * 1.5) {
                    return EventResult::Ignored;
                }
                track.borrow().dragging.set(true);
                if let Some(capture) = PointerCaptureHolder::current() {
                    capture.acquire(&owner, true);
                }
            }
            track.borrow_mut().record(local.x, sign);
            if let Some(on_change) = on_change.as_ref() {
                on_change(dx.max(0.0));
            }
            EventResult::Handled
        }));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(NavigationEdgeSwipe);

/// Reports path-count changes, with whether a page transition will run.
struct NavigationPathWatcher {
    count: usize,
    on_change: Rc<dyn Fn(usize, bool)>,
}

impl PrimitiveView for NavigationPathWatcher {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_width(Some(0.0));
        layout.set_height(Some(0.0));
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        let previous = node.attachment::<usize>(PATH_COUNT);
        node.set_attachment(PATH_COUNT, self.count);
        if let Some(previous) = previous
            && previous != self.count
        {
            (self.on_change)(self.count, !node.prefers_reduced_motion());
        }
    }
}
impl_primitive_view!(NavigationPathWatcher);

/// Marks a page's title as the focus target once a new page has laid out.
struct NavigationPageFocusHost {
    is_top: bool,
    content: AnyView,
}

impl PrimitiveView for NavigationPageFocusHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_align_items(Align::Stretch);
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        let was_top = node.attachment::<bool>(PAGE_IS_TOP).unwrap_or(false);
        node.set_attachment(PAGE_IS_TOP, self.is_top);
        if !self.is_top {
            node.set_attachment(PAGE_TITLE_FOCUSED, false);
            return;
        }
        if was_top {
            return;
        }
        let weak = node.downgrade();
        node.set_layout_did_update(Some(Rc::new(move |owner: &Node| {
            if owner.attachment::<bool>(PAGE_IS_TOP) != Some(true) {
                return;
            }
            if owner.attachment::<bool>(PAGE_TITLE_FOCUSED) == Some(true) {
                return;
            }
            let Some(title) = find_title(owner) else { return };
            let Some(chain) = FocusChainHolder::current() else { return };
            owner.set_attachment(PAGE_TITLE_FOCUSED, true);
            chain.focus(Some(&title), false);
            let _ = &weak;
        }) as cove_core::scene::LayoutUpdateHook));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(NavigationPageFocusHost);

fn find_title(node: &Node) -> Option<Node> {
    if let Some(semantics) = node.accessibility()
        && (semantics.identifier == "navigation.title"
            || semantics.role == cove_core::scene::AccessibilityRole::Heading)
    {
        return Some(node.clone());
    }
    for child in node.children() {
        if let Some(found) = find_title(&child) {
            return Some(found);
        }
    }
    None
}

/// Marks a page title as the focus target for a navigation push/pop. The stack
/// focuses it without a visible keyboard ring once the new page has laid out;
/// ordinary keyboard traversal continues from there.
pub fn navigation_title(title: impl Into<String>, content: impl View + 'static) -> AnyView {
    let title = title.into();
    AnyView::new(
        content.accessibility(move |semantics| {
            semantics.role = cove_core::scene::AccessibilityRole::Heading;
            semantics.label = title.clone();
            semantics.identifier = "navigation.title".to_string();
        }),
    )
}
