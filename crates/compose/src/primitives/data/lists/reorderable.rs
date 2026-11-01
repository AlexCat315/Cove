//! Translation of Primitives/Data/Lists/Reorder/{Reorderable,Handle,Preview}.swift
//! (the view half; the drag arithmetic lives in [`reorder_model`]).
//!
//! Vertical list whose rows can be reordered with a drag handle or the keyboard.
//! The caller owns the order: `on_move` receives the source index and the final
//! index after the row is removed.
//!
//! The handle is a 44pt-wide target and is the only drag surface, so row content
//! keeps its own buttons and taps. While dragging, a 2pt accent line marks the
//! insertion boundary; Escape cancels without calling `on_move`.
//!
//! Behaviour checklist:
//! - the drag position is pointer travel plus automatic scroll, so the drop
//!   boundary stays in content coordinates while the list scrolls
//! - the drop indicator is hidden when the drop would be a no-op
//! - keyboard Up/Down move the focused handle; assistive clients use
//!   increment/decrement
//! - the dragged and last keyboard-moved rows always stay mounted so pointer
//!   capture and focus survive auto-scrolling
//! - the dragged row stays as a dimmed placeholder while the lifted copy follows

use std::cell::RefCell;
use std::rc::Rc;

use super::reorder_model::{
    ReorderDragSession, ReorderPreviewOffset, ReorderableListModel, drop_indicator_y,
    reorder_row_height,
};
use crate::modifiers::interaction::{AccessibilityExt, HitTestingExt};
use crate::modifiers::layout::{AbsolutePositionModifier, FlexExt, LayoutFrameExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::interaction::edge_auto_scroll::EdgeAutoScroll;
use crate::primitives::layout::virtualization::fixed_stack::visible_range;
use crate::primitives::layout::{ScrollAxis, ScrollView, ScrollViewport, ScrollbarGutter};
use crate::primitives::media::{Icon, ui_icon_catalog};
use crate::theme::theme_for_node;
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View};
use cove_composition::Scancode;
use cove_composition::ViewExt;
use cove_core::State;
use cove_core::scene::{
    AccessibilityActions, AccessibilityRole, AccessibilitySemantics, Align, EventPhase,
    EventResult, FlexDirection, HoverPhase, InputHandlerRoute, InteractionRegistryHolder, Justify,
    LayoutNode, Node, PointerCaptureAttachmentKey, PointerCaptureHolder, PointerPhase, Wrap,
};
use cove_platform::{KeyEvent, MouseButton, SystemCursor};

const HANDLE_ACTIVE: &str = "__reorder.active";
const HANDLE_START_Y: &str = "__reorder.startY";
const HANDLE_HOVERED: &str = "__reorder.hovered";
const HANDLE_LAST_Y: &str = "__reorder.lastY";

/// Everything a handle needs to drive one drag. Cloning keeps the same stores,
/// so a clone is the same list rather than a copy — this is what lets the
/// handle closures act without borrowing the view for the node's lifetime.
pub struct ReorderSession<Id> {
    session: Rc<ReorderDragSession>,
    dragging: State<Option<Id>>,
    boundary: State<Option<usize>>,
    retained: State<Option<Id>>,
    window: State<std::ops::Range<usize>>,
    width: State<f32>,
    ids: Rc<RefCell<Vec<Id>>>,
    on_move: Rc<dyn Fn(usize, usize)>,
    is_enabled: bool,
    row_height: f32,
    row_spacing: f32,
}

impl<Id: Eq + std::hash::Hash + Clone + Send + Sync + 'static> Clone for ReorderSession<Id> {
    fn clone(&self) -> Self {
        Self {
            session: self.session.clone(),
            dragging: self.dragging.clone(),
            boundary: self.boundary.clone(),
            retained: self.retained.clone(),
            window: self.window.clone(),
            width: self.width.clone(),
            ids: self.ids.clone(),
            on_move: self.on_move.clone(),
            is_enabled: self.is_enabled,
            row_height: self.row_height,
            row_spacing: self.row_spacing,
        }
    }
}

impl<Id: Eq + std::hash::Hash + Clone + Send + Sync + 'static> ReorderSession<Id> {
    /// The count of authored rows, as the last projection saw them.
    pub fn count(&self) -> usize {
        self.ids.borrow().len()
    }

    pub fn is_dragging(&self) -> bool {
        self.dragging.get().is_some()
    }

    pub fn dragging_id(&self) -> Option<Id> {
        self.dragging.get()
    }

    fn index_of(&self, id: &Id) -> Option<usize> {
        self.ids.borrow().iter().position(|other| other == id)
    }

    pub fn is_row_dragging(&self, id: &Id) -> bool {
        self.dragging.get().as_ref() == Some(id)
    }

    pub fn begin_drag(&self, index: usize) {
        if !self.is_enabled {
            return;
        }
        let stride = self.row_height + self.row_spacing;
        self.session.begin(index as f32 * stride, self.row_height);
        self.dragging.set(self.ids.borrow().get(index).cloned());
        self.boundary.set(Some(index));
    }

    pub fn update_drag(&self, travel: f32, scrolled: f32) {
        self.session.move_by(travel);
        let Some(dragging) = self.dragging.get() else {
            return;
        };
        let Some(source) = self.index_of(&dragging) else {
            return;
        };
        let boundary = ReorderableListModel::drop_boundary(
            source,
            travel,
            scrolled,
            self.row_height + self.row_spacing,
            self.count(),
        );
        self.boundary.set(Some(boundary));
    }

    pub fn finish_drag(&self) {
        let dragging = self.dragging.get();
        let boundary = self.boundary.get();
        self.dragging.set(None);
        self.boundary.set(None);
        self.session.end();
        let (Some(dragging), Some(boundary)) = (dragging, boundary) else {
            return;
        };
        let Some(source) = self.index_of(&dragging) else {
            return;
        };
        if let Some(destination) = ReorderableListModel::destination(source, boundary) {
            (self.on_move)(source, destination);
        }
    }

    pub fn cancel_drag(&self) {
        self.dragging.set(None);
        self.boundary.set(None);
        self.session.end();
    }

    /// Keyboard move: records the moved row so it stays mounted after the
    /// caller's reorder lands.
    pub fn move_row(&self, index: usize, delta: i64) {
        if !self.is_enabled {
            return;
        }
        let Some(destination) = ReorderableListModel::stepped(index, delta, self.count()) else {
            return;
        };
        self.retained.set(self.ids.borrow().get(index).cloned());
        (self.on_move)(index, destination);
    }

    pub fn visible_boundary(&self) -> Option<usize> {
        let dragging = self.dragging.get()?;
        let boundary = self.boundary.get()?;
        let source = self.index_of(&dragging)?;
        ReorderableListModel::visible_boundary(source, boundary)
    }

    /// Indices to mount: the visible window plus the dragged and last
    /// keyboard-moved rows.
    pub fn mounted_indices(&self) -> Vec<usize> {
        let mut retained = Vec::new();
        if let Some(id) = self.dragging.get()
            && let Some(index) = self.index_of(&id)
        {
            retained.push(index);
        }
        if let Some(id) = self.retained.get()
            && let Some(index) = self.index_of(&id)
            && !retained.contains(&index)
        {
            retained.push(index);
        }
        ReorderableListModel::mounted_indices(self.window.get(), self.count(), &retained)
    }

    pub fn preview_top(&self) -> f32 {
        self.session.preview_top()
    }
}

/// Vertical, keyboard-accessible reorderable list.
pub struct ReorderableList<Item, Id, RowContent>
where
    Item: Clone + 'static,
    Id: Eq + std::hash::Hash + Clone + Send + Sync + 'static,
    RowContent: Fn(&Item, bool) -> AnyView + 'static,
{
    pub data: Vec<Item>,
    /// Authored logical row height, before text scaling and the touch floor.
    pub row_height: f32,
    /// Authored logical row spacing, before text scaling.
    pub row_spacing: f32,
    pub is_enabled: bool,
    /// Called with `(from, to)`, where `to` is the final index after removal.
    pub on_move: Rc<dyn Fn(usize, usize)>,
    pub row_label: Rc<dyn Fn(&Item) -> String>,
    pub row_content: RowContent,
    id_of: Rc<dyn Fn(&Item) -> Id>,
    session: Rc<ReorderSession<Id>>,
    preview_offset: ReorderPreviewOffset,
}

impl<Item, Id, RowContent> ReorderableList<Item, Id, RowContent>
where
    Item: Clone + 'static,
    Id: Eq + std::hash::Hash + Clone + Send + Sync + 'static,
    RowContent: Fn(&Item, bool) -> AnyView + 'static,
{
    pub fn new(
        data: Vec<Item>,
        id_of: impl Fn(&Item) -> Id + 'static,
        on_move: impl Fn(usize, usize) + 'static,
        row_content: RowContent,
    ) -> Self {
        let id_of = Rc::new(id_of);
        let ids: Vec<Id> = data.iter().map(|item| id_of(item)).collect();
        let session = Rc::new(ReorderSession {
            session: Rc::new(ReorderDragSession::new()),
            dragging: State::new(None),
            boundary: State::new(None),
            retained: State::new(None),
            window: State::new(0..24),
            width: State::new(0.0),
            ids: Rc::new(RefCell::new(ids)),
            on_move: Rc::new(on_move),
            is_enabled: true,
            row_height: 36.0,
            row_spacing: 0.0,
        });
        Self {
            data,
            row_height: 36.0,
            row_spacing: 0.0,
            is_enabled: true,
            on_move: session.on_move.clone(),
            row_label: Rc::new(|_| String::new()),
            row_content,
            id_of,
            session,
            preview_offset: ReorderPreviewOffset::new(),
        }
    }

    pub fn row_height(mut self, row_height: f32) -> Self {
        self.row_height = row_height.max(1.0);
        self
    }

    pub fn row_spacing(mut self, row_spacing: f32) -> Self {
        self.row_spacing = row_spacing.max(0.0);
        self
    }

    pub fn is_enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn row_label(mut self, label: impl Fn(&Item) -> String + 'static) -> Self {
        self.row_label = Rc::new(label);
        self
    }

    /// The resolved row height: authored height after text scaling, raised to
    /// the minimum control extent on a touch surface.
    pub fn resolved_row_height(
        &self,
        scaled: f32,
        is_touch: bool,
        minimum_control_extent: f32,
    ) -> f32 {
        reorder_row_height(scaled, is_touch, minimum_control_extent)
    }

    /// Keyboard stepping from `index`, clamped rather than wrapped.
    pub fn step(&self, index: usize, delta: i64) -> Option<usize> {
        if !self.is_enabled {
            return None;
        }
        ReorderableListModel::stepped(index, delta, self.data.len())
    }

    pub fn is_dragging(&self) -> bool {
        self.session.is_dragging()
    }

    pub fn handle_label(&self, index: usize) -> String {
        let title = self.data.get(index).map(|item| (self.row_label)(item));
        ReorderableListModel::handle_label(title.as_deref(), index, self.data.len())
    }

    /// Publishes this build's projection into the shared session so the handle
    /// closures resolve indices against the same order the rows were built from.
    fn publish(&self) {
        let ids: Vec<Id> = self.data.iter().map(|item| (self.id_of)(item)).collect();
        *self.session.ids.borrow_mut() = ids;
    }
}

impl<Item, Id, RowContent> View for ReorderableList<Item, Id, RowContent>
where
    Item: Clone + 'static,
    Id: Eq + std::hash::Hash + Clone + Send + Sync + 'static,
    RowContent: Fn(&Item, bool) -> AnyView + 'static,
{
    fn body(&self) -> AnyView {
        self.publish();
        let row_height = self.row_height;
        let row_spacing = self.row_spacing;
        let row_stride = row_height + row_spacing;
        let count = self.data.len();
        let session = self.session.clone();
        let dragging = session.dragging_id();

        let mounted = mounted_window(&session, count);
        let rows: Vec<AnyView> = mounted
            .into_iter()
            .filter_map(|index| {
                let item = self.data.get(index)?.clone();
                let id = (self.id_of)(&item);
                let is_dragging = dragging.as_ref() == Some(&id);
                let label = self.handle_label(index);
                let content = (self.row_content)(&item, is_dragging);
                Some(AnyView::new(ReorderRowSlot {
                    index,
                    row_stride,
                    row_height,
                    is_dragging,
                    handle: ReorderHandle {
                        index,
                        count,
                        label,
                        is_enabled: self.is_enabled,
                        is_dragging,
                        row_height,
                        session: session.clone(),
                    },
                    content,
                }))
            })
            .collect();

        let content_height = (count as f32 * row_stride - row_spacing).max(0.0);
        let width = session.width.get();

        let column = AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(rows)),
            ))
            .frame(
                Some(crate::modifiers::layout::FrameDimension::Points(
                    width.max(1.0),
                )),
                Some(crate::modifiers::layout::FrameDimension::Points(
                    content_height.max(1.0),
                )),
                None,
                None,
                None,
                None,
            ),
        );

        let mut layers = vec![column];
        if let Some(boundary) = session.visible_boundary() {
            let y = drop_indicator_y(boundary, count, row_stride, content_height);
            layers.push(AnyView::new(
                AnyView::new(
                    AnyView::new(crate::EmptyView)
                        .frame_height(2.0)
                        .semantic_background(crate::theme::SemanticColorRef::accent())
                        .corner_radius(1.0)
                        .modifier(cove_composition::LayoutDebugModifier::debug_name(
                            "reorder-drop-indicator",
                        )),
                )
                .with_modifier(AbsolutePositionModifier::new(
                    Some(8.0),
                    Some(y),
                    Some(8.0),
                    None,
                )),
            ));
        }

        let scroll_session = session.clone();
        let scroll = ScrollView::new(
            ScrollAxis::Vertical,
            AnyView::new(crate::primitives::layout::ZStack::new(
                crate::Alignment::TopLeading,
                AnyView::new(TupleView::new(layers)),
            )),
        )
        .scrollbar_gutter(ScrollbarGutter::Stable)
        .on_viewport_change(move |viewport: ScrollViewport| {
            let drag = &scroll_session.session;
            drag.set_scroll_y(viewport.offset.y);
            drag.set_viewport_height(viewport.size.height);
            if (viewport.size.width - scroll_session.width.get()).abs() > 0.5 {
                scroll_session.width.set(viewport.size.width);
            }
            let visible = visible_range(
                scroll_session.count(),
                (scroll_session.row_height + scroll_session.row_spacing).max(1.0),
                viewport.offset.y,
                viewport.size.height,
                4,
            );
            if scroll_session.window.get() != visible {
                scroll_session.window.set(visible);
            }
        });

        let mut surface = vec![AnyView::new(AnyView::new(scroll).flex_with(
            1.0,
            1.0,
            Some(0.0),
        ))];
        if let Some(dragging) = dragging
            && let Some(item) = self.data.iter().find(|item| (self.id_of)(item) == dragging)
        {
            let preview = (self.row_content)(item, true);
            surface.push(AnyView::new(
                AnyView::new(ReorderPreviewHost {
                    offset: self.preview_offset.clone(),
                    session: session.clone(),
                    row_height,
                    width,
                    content: preview,
                })
                .with_modifier(AbsolutePositionModifier::new(
                    Some(0.0),
                    Some(0.0),
                    Some(0.0),
                    Some(0.0),
                ))
                .allows_hit_testing(false),
            ));
        }

        AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(surface)),
            ))
            .flex_with(1.0, 1.0, Some(0.0)),
        )
    }
}

fn mounted_window<Id: Eq + std::hash::Hash + Clone + Send + Sync + 'static>(
    session: &Rc<ReorderSession<Id>>,
    count: usize,
) -> Vec<usize> {
    let mut retained = Vec::new();
    if let Some(id) = session.dragging.get()
        && let Some(index) = session.index_of(&id)
    {
        retained.push(index);
    }
    if let Some(id) = session.retained.get()
        && let Some(index) = session.index_of(&id)
        && !retained.contains(&index)
    {
        retained.push(index);
    }
    ReorderableListModel::mounted_indices(session.window.get(), count, &retained)
}

/// One row: content plus its drag handle, absolutely positioned so offscreen
/// rows take no layout space.
struct ReorderRowSlot<Id: Eq + std::hash::Hash + Clone + Send + Sync + 'static> {
    index: usize,
    row_stride: f32,
    row_height: f32,
    is_dragging: bool,
    handle: ReorderHandle<Id>,
    content: AnyView,
}

impl<Id: Eq + std::hash::Hash + Clone + Send + Sync + 'static> PrimitiveView
    for ReorderRowSlot<Id>
{
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        use cove_core::scene::{Edge, PositionType};
        layout.set_position_type(PositionType::Absolute);
        layout.set_position(0.0, Edge::Left);
        layout.set_position(self.index as f32 * self.row_stride, Edge::Top);
        layout.set_position(0.0, Edge::Right);
        layout.set_height(Some(self.row_height));
    }

    fn update_node(&self, node: &Node) {
        node.set_corner_radius(6.0);
        node.set_opacity(if self.is_dragging { 0.35 } else { 1.0 });
        node.set_is_hit_testable(false);
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                4.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(
                        AnyView::new(self.content.clone())
                            .flex_with(1.0, 1.0, Some(0.0))
                            .clipped(),
                    ),
                    AnyView::new(self.handle.clone()),
                ])),
            ))
            .frame_height(self.row_height)
            .clipped(),
        )]
    }
}

impl<Id: Eq + std::hash::Hash + Clone + Send + Sync + 'static> View for ReorderRowSlot<Id> {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Drag/focus handle behind one reorderable row. Interaction state lives on the
/// node so a parent recompose mid-drag keeps the pointer transaction.
#[derive(Clone)]
struct ReorderHandle<Id: Eq + std::hash::Hash + Clone + Send + Sync + 'static> {
    index: usize,
    count: usize,
    label: String,
    is_enabled: bool,
    is_dragging: bool,
    row_height: f32,
    session: Rc<ReorderSession<Id>>,
}

impl<Id: Eq + std::hash::Hash + Clone + Send + Sync + 'static> PrimitiveView for ReorderHandle<Id> {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_width(Some(44.0));
        layout.set_height(Some(self.row_height));
        layout.set_flex_shrink(0.0);
        layout.set_align_items(Align::Center);
        layout.set_justify_content(Justify::Center);
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        let session = self.session.clone();
        let index = self.index;
        let count = self.count;
        let is_enabled = self.is_enabled;

        node.set_corner_radius(6.0);
        node.set_is_focusable(self.is_enabled);
        node.set_cursor(if self.is_enabled {
            Some(SystemCursor::Pointer)
        } else {
            None
        });
        node.set_attachment(
            cove_composition::LayoutDebugAttachmentKey::DEBUG_NAME,
            format!("reorder-handle-{index}"),
        );
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Slider,
            label: self.label.clone(),
            value: format!("Position {} of {count}", index + 1),
            help: "Drag the handle, or use Up and Down to move this row.".to_string(),
            state: cove_core::scene::AccessibilityState {
                is_enabled: self.is_enabled,
                is_selected: self.is_dragging,
                ..Default::default()
            },
            ..Default::default()
        }));

        if self.is_enabled {
            let up = session.clone();
            let down = session.clone();
            node.set_accessibility_actions(AccessibilityActions {
                increment: Some(Rc::new(move || up.move_row(index, 1))),
                decrement: Some(Rc::new(move || down.move_row(index, -1))),
                ..Default::default()
            });
        }

        let paint_node = node.clone();
        let paint: Rc<dyn Fn()> = Rc::new(move || {
            let active = paint_node.attachment::<bool>(HANDLE_ACTIVE) == Some(true);
            let hovered = paint_node.attachment::<bool>(HANDLE_HOVERED) == Some(true);
            let theme = theme_for_node(&paint_node);
            let color = if active {
                Some(theme.colors.state_layer_pressed)
            } else if hovered {
                Some(theme.colors.state_layer_hover)
            } else {
                Some(cove_core::Color::CLEAR)
            };
            paint_node.set_background_color(color);
        });
        paint();

        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };

        let hover_node = node.downgrade();
        let hover_paint = paint.clone();
        registry.set_hover(
            node,
            Rc::new(move |phase: HoverPhase| {
                if let Some(owner) = Node::upgrade(&hover_node) {
                    owner.set_attachment(HANDLE_HOVERED, phase == HoverPhase::Enter && is_enabled);
                    hover_paint();
                }
            }),
        );

        let auto_scroll = EdgeAutoScroll::new();
        node.add_resource(auto_scroll.clone());

        let pointer_session = session.clone();
        let pointer_node = node.clone();
        let pointer_paint = paint.clone();
        let pointer_scroll = auto_scroll.clone();
        registry.set_pointer(
            node,
            InputHandlerRoute::control(),
            Rc::new(
                move |event: &cove_platform::MouseButtonEvent,
                      phase: PointerPhase,
                      delivery: EventPhase| {
                    if delivery != EventPhase::Target
                        || !is_enabled
                        || event.button != MouseButton::Left
                    {
                        return EventResult::Ignored;
                    }
                    match phase {
                        PointerPhase::Down => {
                            pointer_node.set_attachment(HANDLE_ACTIVE, true);
                            pointer_node.set_attachment(HANDLE_START_Y, event.y);
                            pointer_node.set_attachment(HANDLE_LAST_Y, event.y);
                            let report_node = pointer_node.downgrade();
                            let report_session = pointer_session.clone();
                            let report_scroll = pointer_scroll.clone();
                            (pointer_scroll).begin(
                                event.y,
                                Rc::new(move || {
                                    if let Some(owner) = Node::upgrade(&report_node)
                                        && let Some(start) = owner.attachment::<f32>(HANDLE_START_Y)
                                        && let Some(last) = owner.attachment::<f32>(HANDLE_LAST_Y)
                                    {
                                        report_session.update_drag(
                                            last - start,
                                            report_scroll.scrolled_distance(),
                                        );
                                    }
                                }),
                            );
                            if let Some(capture) = PointerCaptureHolder::current() {
                                capture.acquire(&pointer_node, false);
                            }
                            pointer_paint();
                            pointer_session.begin_drag(index);
                            EventResult::Handled
                        }
                        PointerPhase::Up => {
                            if pointer_node.attachment::<bool>(HANDLE_ACTIVE) != Some(true) {
                                return EventResult::Ignored;
                            }
                            pointer_node.set_attachment(HANDLE_ACTIVE, false);
                            pointer_node.remove_attachment(HANDLE_START_Y);
                            pointer_scroll.end();
                            PointerCaptureHolder::release_current();
                            pointer_paint();
                            pointer_session.finish_drag();
                            EventResult::Handled
                        }
                    }
                },
            ),
        );

        let motion_session = session.clone();
        let motion_node = node.clone();
        let motion_scroll = auto_scroll.clone();
        registry.set_motion(
            node,
            InputHandlerRoute::control(),
            Rc::new(
                move |event: &cove_platform::MouseMotionEvent, _phase: EventPhase| {
                    if motion_node.attachment::<bool>(HANDLE_ACTIVE) != Some(true) {
                        return EventResult::Ignored;
                    }
                    motion_node.set_attachment(HANDLE_LAST_Y, event.y);
                    motion_scroll.update(event.y);
                    if let Some(start) = motion_node.attachment::<f32>(HANDLE_START_Y) {
                        motion_session
                            .update_drag(event.y - start, motion_scroll.scrolled_distance());
                    }
                    EventResult::Handled
                },
            ),
        );

        let key_session = session.clone();
        let key_node = node.downgrade();
        let key_scroll = auto_scroll.clone();
        registry.set_key(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event: &KeyEvent, phase: EventPhase| {
                if phase == EventPhase::Capture || !is_enabled {
                    return EventResult::Ignored;
                }
                match event.scancode {
                    Scancode::ARROW_UP => {
                        key_session.move_row(index, -1);
                        EventResult::Handled
                    }
                    Scancode::ARROW_DOWN => {
                        key_session.move_row(index, 1);
                        EventResult::Handled
                    }
                    Scancode::ESCAPE => {
                        let Some(owner) = Node::upgrade(&key_node) else {
                            return EventResult::Ignored;
                        };
                        if owner.attachment::<bool>(HANDLE_ACTIVE) != Some(true) {
                            return EventResult::Ignored;
                        }
                        owner.set_attachment(HANDLE_ACTIVE, false);
                        owner.remove_attachment(HANDLE_START_Y);
                        key_scroll.end();
                        PointerCaptureHolder::release_current();
                        key_session.cancel_drag();
                        EventResult::Handled
                    }
                    _ => EventResult::Ignored,
                }
            }),
        );

        let clear_node = node.downgrade();
        node.set_attachment(
            PointerCaptureAttachmentKey::DID_LOSE_CAPTURE,
            Rc::new(move || {
                if let Some(owner) = Node::upgrade(&clear_node) {
                    owner.remove_attachment(HANDLE_START_Y);
                    owner.set_attachment(HANDLE_ACTIVE, false);
                }
            }) as Rc<dyn Fn()>,
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![AnyView::new(
            AnyView::new(Icon::with_semantic(
                ui_icon_catalog::action::grip(),
                16.0,
                crate::theme::SemanticColorRef::on_surface_muted(),
            ))
            .accessibility_hidden(true),
        )]
    }
}

impl<Id: Eq + std::hash::Hash + Clone + Send + Sync + 'static> View for ReorderHandle<Id> {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Covers the list viewport and paints the lifted row at the session's
/// position through its content offset, so following the pointer neither
/// relayouts nor recomposes. It never takes input.
struct ReorderPreviewHost<Id: Eq + std::hash::Hash + Clone + Send + Sync + 'static> {
    offset: ReorderPreviewOffset,
    session: Rc<ReorderSession<Id>>,
    row_height: f32,
    width: f32,
    content: AnyView,
}

impl<Id: Eq + std::hash::Hash + Clone + Send + Sync + 'static> PrimitiveView
    for ReorderPreviewHost<Id>
{
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        self.offset.bind(std::rc::Rc::new(node.clone()));
        self.offset.apply(self.session.preview_top());
        node.set_clips_to_bounds(true);
        node.set_is_hit_testable(false);
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let height = self.row_height;
        vec![AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                4.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(
                        AnyView::new(self.content.clone())
                            .flex_with(1.0, 1.0, Some(0.0))
                            .clipped(),
                    ),
                    AnyView::new(
                        AnyView::new(Icon::with_semantic(
                            ui_icon_catalog::action::grip(),
                            16.0,
                            crate::theme::SemanticColorRef::on_surface_muted(),
                        ))
                        .frame_size(44.0, height),
                    ),
                ])),
            ))
            .frame(
                Some(crate::modifiers::layout::FrameDimension::Points(
                    self.width.max(1.0),
                )),
                Some(crate::modifiers::layout::FrameDimension::Points(height)),
                None,
                None,
                None,
                None,
            )
            .semantic_background(crate::theme::SemanticColorRef::surface())
            .corner_radius(6.0)
            .border_color(cove_core::Color::from_rgba8(0, 0, 0, 26), 1.0)
            .shadow(cove_core::Color::from_rgba8(0, 0, 0, 71), 0.0, 4.0, 12.0)
            .modifier(cove_composition::LayoutDebugModifier::debug_name(
                "reorder-preview",
            )),
        )]
    }
}

impl<Id: Eq + std::hash::Hash + Clone + Send + Sync + 'static> View for ReorderPreviewHost<Id> {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}
