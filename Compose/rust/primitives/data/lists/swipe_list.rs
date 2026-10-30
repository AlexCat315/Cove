//! Translation of Primitives/Data/Lists/SwipeList.swift.
//!
//! Vertical list with trailing swipe actions. Selection and keyboard navigation
//! follow `List`; a swipe toward the logical leading edge reveals up to two
//! actions. Every row also carries an actions menu, so the same commands are
//! reachable with a pointer, a keyboard or a screen reader.
//!
//! The list mounts all rows (like `ReorderableList`); use it for settings,
//! inbox rows and playlists rather than unbounded collections.
//!
//! Behaviour checklist:
//! - at most two actions appear on the swipe surface; the menu lists every one
//! - a horizontal drag past 8pt activates the swipe and captures the pointer
//! - a mostly vertical drag abandons the swipe so the list can scroll
//! - the reveal snaps open past 45% of its extent, otherwise closed
//! - Escape closes an open row; reduced motion snaps without animating
//! - a click inside the row selects it; Escape and selection live on the row

use std::rc::Rc;

use cove_composition::Scancode;
use cove_core::scene::{
    AccessibilityActions, AccessibilityRole, AccessibilitySemantics, Align, EventPhase,
    EventResult, FlexDirection, HoverPhase, InputHandlerRoute, InteractionRegistryHolder,
    Justify, LayoutNode, Node, PointerCaptureAttachmentKey, PointerCaptureHolder, PointerPhase,
    Wrap,
};
use cove_core::{Binding, Environment, InputCapabilities, State};
use cove_platform::{KeyEvent, MouseButton, SystemCursor};
use crate::adaptive::SurfaceEnvironment;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::button::{Button, ButtonIcon, ButtonIconSource};
use crate::primitives::media::ui_icon_catalog;
use crate::primitives::feedback::StatusTone;
use crate::primitives::overlays::menu::{MenuEntry, MenuItem, MenuItemRole};
use crate::primitives::overlays::ContextMenu;
use crate::primitives::text::Text;
use crate::theme::styles::buttons::{AnyButtonStyle, ButtonStyleViewExt};
use crate::theme::SemanticFontRef;
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View};

/// One trailing action on a `SwipeList` row.
#[derive(Clone)]
pub struct SwipeAction {
    pub id: u64,
    pub title: String,
    pub tone: StatusTone,
    pub is_enabled: bool,
    pub action: Rc<dyn Fn()>,
}

impl SwipeAction {
    pub fn new(id: impl Into<u64>, title: impl Into<String>, action: impl Fn() + 'static) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            tone: StatusTone::Neutral,
            is_enabled: true,
            action: Rc::new(action),
        }
    }

    pub fn tone(mut self, tone: StatusTone) -> Self {
        self.tone = tone;
        self
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }
}

const SWIPE_START: &str = "__swipe.start";
const SWIPE_BASE: &str = "__swipe.base";
const SWIPE_ACTIVE: &str = "__swipe.active";
const SWIPE_REJECTED: &str = "__swipe.rejected";
const SWIPE_HOVERED: &str = "__swipe.hovered";

/// Authored swipe surface values, shared with the model so scaling agrees.
pub const SWIPE_ACTION_WIDTH: f32 = 74.0;
pub const SWIPE_MAX_ACTIONS: usize = 2;
pub const SWIPE_TRAILING_SLOT: f32 = 44.0;
/// Horizontal travel past which a press becomes a swipe.
pub const SWIPE_ACTIVATION_DISTANCE: f32 = 8.0;
/// Vertical travel past which a press is abandoned to the list's scrolling.
pub const SWIPE_REJECTION_DISTANCE: f32 = 4.0;
/// Fraction of the extent past which the reveal snaps open.
pub const SWIPE_SNAP_THRESHOLD: f32 = 0.45;

/// The action extent for `count` actions within `row_width`.
pub fn swipe_action_extent(row_width: f32, count: usize) -> f32 {
    let count = count.max(1) as f32;
    44.0_f32.max((SWIPE_ACTION_WIDTH).min(row_width / count))
}

/// The snapped reveal for a live `reveal` value.
pub fn swipe_snap_target(reveal: f32, max_reveal: f32) -> f32 {
    if reveal < -max_reveal * SWIPE_SNAP_THRESHOLD { -max_reveal } else { 0.0 }
}

/// Vertical list with trailing swipe actions.
pub struct SwipeList<Item, Id, RowContent>
where
    Item: Clone + 'static,
    Id: Ord + Clone + Send + Sync + 'static,
    RowContent: Fn(&Item, bool) -> AnyView + 'static,
{
    pub data: Vec<Item>,
    pub selection: Binding<Option<Id>>,
    pub row_height: f32,
    pub row_spacing: f32,
    pub is_enabled: bool,
    pub row_label: Rc<dyn Fn(&Item) -> String>,
    pub actions: Rc<dyn Fn(&Item) -> Vec<SwipeAction>>,
    pub on_activate: Option<Rc<dyn Fn(&Item)>>,
    pub row_content: RowContent,
    id_of: Rc<dyn Fn(&Item) -> Id>,
    cursor: State<Option<Id>>,
}

impl<Item, Id, RowContent> SwipeList<Item, Id, RowContent>
where
    Item: Clone + 'static,
    Id: Ord + Clone + Send + Sync + 'static,
    RowContent: Fn(&Item, bool) -> AnyView + 'static,
{
    pub fn new(
        data: Vec<Item>,
        id_of: impl Fn(&Item) -> Id + 'static,
        actions: impl Fn(&Item) -> Vec<SwipeAction> + 'static,
        row_content: RowContent,
    ) -> Self {
        Self {
            data,
            selection: Binding::constant(None),
            row_height: 36.0,
            row_spacing: 0.0,
            is_enabled: true,
            row_label: Rc::new(|_| String::new()),
            actions: Rc::new(actions),
            on_activate: None,
            row_content,
            id_of: Rc::new(id_of),
            cursor: State::new(None),
        }
    }

    pub fn selection(mut self, selection: Binding<Option<Id>>) -> Self {
        self.selection = selection;
        self
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

    pub fn on_activate(mut self, handler: impl Fn(&Item) + 'static) -> Self {
        self.on_activate = Some(Rc::new(handler));
        self
    }

    pub fn select(&self, item: &Item) {
        let id = (self.id_of)(item);
        self.cursor.set(Some(id.clone()));
        self.selection.set(Some(id));
    }

    pub fn activate(&self, item: &Item) {
        self.select(item);
        if let Some(handler) = &self.on_activate {
            handler(item);
        }
    }

    /// Down/Up/Home/End move the selection; Return activates it.
    pub fn handle_key(&self, event: &KeyEvent) -> bool {
        if !self.is_enabled || self.data.is_empty() {
            return false;
        }
        let position_of = |id: &Id| self.data.iter().position(|item| &(self.id_of)(item) == id);
        let current = self
            .cursor
            .get()
            .as_ref()
            .and_then(position_of)
            .or_else(|| self.selection.get().as_ref().and_then(position_of));

        if event.scancode == Scancode::RETURN || event.scancode == Scancode::KEYPAD_ENTER {
            let Some(current) = current else { return false };
            let item = self.data[current].clone();
            self.activate(&item);
            return true;
        }

        let next = match event.scancode {
            Scancode::ARROW_DOWN => current.map(|value| value + 1).unwrap_or(0),
            Scancode::ARROW_UP => current
                .and_then(|value| value.checked_sub(1))
                .unwrap_or(self.data.len() - 1),
            Scancode::HOME => 0,
            Scancode::END => self.data.len() - 1,
            _ => return false,
        };
        let next = next.min(self.data.len() - 1);
        let item = self.data[next].clone();
        self.select(&item);
        true
    }
}

impl<Item, Id, RowContent> View for SwipeList<Item, Id, RowContent>
where
    Item: Clone + 'static,
    Id: Ord + Clone + Send + Sync + 'static,
    RowContent: Fn(&Item, bool) -> AnyView + 'static,
{
    fn body(&self) -> AnyView {
        let total_height: f32 = {
            let count = self.data.len() as f32;
            (count * (self.row_height + self.row_spacing) - self.row_spacing).max(0.0)
        };
        let rows: Vec<AnyView> = self
            .data
            .iter()
            .map(|item| {
                let id = (self.id_of)(item);
                let selected = self.selection.get().as_ref() == Some(&id);
                let label = (self.row_label)(item);
                let actions = if self.is_enabled { (self.actions)(item) } else { Vec::new() };
                let content = (self.row_content)(item, selected);
                let item = item.clone();
                let cursor = self.cursor.clone();
                let selection = self.selection.clone();
                let on_activate = self.on_activate.clone();
                let id_of = self.id_of.clone();
                let select: Rc<dyn Fn()> = Rc::new(move || {
                    let id = id_of(&item);
                    cursor.set(Some(id.clone()));
                    selection.set(Some(id));
                    if let Some(handler) = &on_activate {
                        handler(&item);
                    }
                });
                AnyView::new(SwipeRow {
                    label,
                    is_selected: selected,
                    is_enabled: self.is_enabled,
                    row_height: self.row_height,
                    actions,
                    on_select: select,
                    content,
                })
            })
            .collect();

        AnyView::new(crate::adaptive::SurfaceReader::new({
            let row_spacing = self.row_spacing;
            let on_key = {
                let data = self.data.clone();
                let id_of = self.id_of.clone();
                let is_enabled = self.is_enabled;
                let cursor = self.cursor.clone();
                let selection = self.selection.clone();
                let on_activate = self.on_activate.clone();
                move |event: &KeyEvent| -> bool {
                    if !is_enabled || data.is_empty() {
                        return false;
                    }
                    let position_of = |id: &Id| data.iter().position(|item| &id_of(item) == id);
                    let current = cursor
                        .get()
                        .as_ref()
                        .and_then(position_of)
                        .or_else(|| selection.get().as_ref().and_then(position_of));
                    if event.scancode == Scancode::RETURN
                        || event.scancode == Scancode::KEYPAD_ENTER
                    {
                        let Some(position) = current else { return false };
                        let item = data[position].clone();
                        let id = id_of(&item);
                        cursor.set(Some(id.clone()));
                        selection.set(Some(id));
                        if let Some(handler) = &on_activate {
                            handler(&item);
                        }
                        return true;
                    }
                    let next = match event.scancode {
                        Scancode::ARROW_DOWN => current.map(|value| value + 1).unwrap_or(0),
                        Scancode::ARROW_UP => current
                            .and_then(|value| value.checked_sub(1))
                            .unwrap_or(data.len() - 1),
                        Scancode::HOME => 0,
                        Scancode::END => data.len() - 1,
                        _ => return false,
                    };
                    let next = next.min(data.len() - 1);
                    let id = id_of(&data[next]);
                    cursor.set(Some(id.clone()));
                    selection.set(Some(id));
                    true
                }
            };
            move |environment: &Environment| {
                let touch = environment.input.contains(InputCapabilities::TOUCH);
                let floor = if touch { environment.minimum_control_extent() } else { 0.0 };
                let _ = floor;
                let _ = &on_key;
                let column = AnyView::new(BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    row_spacing,
                    AnyView::new(TupleView::new(rows.clone())),
                ))
                .frame(None, None, None, Some(total_height.max(1.0)), None, None);

                AnyView::new(crate::primitives::layout::ScrollView::new(
                    crate::primitives::layout::ScrollAxis::Vertical,
                    column,
                ))
            }
        }))
    }
}

/// Row content plus the swipe reveal. Reveal state lives in the view so
/// updating it rebuilds the offset and the action row.
struct SwipeRow {
    label: String,
    is_selected: bool,
    is_enabled: bool,
    row_height: f32,
    actions: Vec<SwipeAction>,
    on_select: Rc<dyn Fn()>,
    content: AnyView,
}

impl SwipeRow {
    fn visible_actions(&self) -> &[SwipeAction] {
        &self.actions[..self.actions.len().min(SWIPE_MAX_ACTIONS)]
    }
}

impl View for SwipeRow {
    fn body(&self) -> AnyView {
        let actions = self.visible_actions().to_vec();
        let max_reveal = swipe_action_extent(1000.0, actions.len()) * actions.len() as f32;
        let row_height = self.row_height;
        let is_enabled = self.is_enabled;
        let reveal = State::new(0.0_f32);

        let actions_menu: Rc<dyn Fn() -> Vec<MenuEntry>> = {
            let actions = self.actions.clone();
            let reveal = reveal.clone();
            Rc::new(move || {
                actions
                    .iter()
                    .map(|action| {
                        let reset = reveal.clone();
                        let run = action.action.clone();
                        let action_tone = action.tone;
                        let enabled = is_enabled && action.is_enabled;
                        MenuEntry::Item(
                            MenuItem::new(action.id, action.title.clone(), move || {
                                reset.set(0.0);
                                run();
                            })
                            .enabled(enabled)
                            .role(if action_tone == StatusTone::Danger {
                                MenuItemRole::Destructive
                            } else {
                                MenuItemRole::Normal
                            }),
                        )
                    })
                    .collect()
            })
        };

        let action_buttons: Vec<AnyView> = actions
            .iter()
            .map(|action| {
                let title = action.title.clone();
                let enabled = is_enabled && action.is_enabled;
                let reset = reveal.clone();
                let run = action.action.clone();
                let style = if action.tone == StatusTone::Danger {
                    AnyButtonStyle::new(crate::theme::styles::buttons::destructive())
                } else {
                    AnyButtonStyle::new(crate::theme::styles::buttons::secondary())
                };
                AnyView::new(
                    AnyView::new(
                        Button::new(
                            AnyView::new(
                                Text::new(title.clone())
                                    .truncated()
                                    .semantic_font(SemanticFontRef::caption()),
                            ),
                            move || {
                                reset.set(0.0);
                                run();
                            },
                        )
                        .enabled(enabled),
                    )
                    .button_style_any(style)
                    .frame_height(row_height),
                )
            })
            .collect();

        let action_row = if action_buttons.is_empty() {
            None
        } else {
            Some(AnyView::new(
                BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexEnd,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(TupleView::new(action_buttons)),
                )
                .frame_height(row_height),
            ))
        };

        let trailing_slot = if actions.is_empty() {
            None
        } else {
            let open = {
                let reveal = reveal.clone();
                Rc::new(move || {
                    if reveal.get() >= -0.5 {
                        reveal.set(-max_reveal);
                    }
                })
            };
            Some(AnyView::new(
                AnyView::new(
                    Button::new(
                        ButtonIcon::new(
                            ButtonIconSource::resource(ui_icon_catalog::action::more_horizontal()),
                            Some(16.0),
                            None,
                        ),
                        move || open(),
                    )
                    .enabled(is_enabled)
                    .tooltip(format!("Actions for {}", self.label)),
                )
                .button_style(crate::theme::styles::buttons::ghost())
                .frame_size(SWIPE_TRAILING_SLOT, row_height),
            ))
        };

        let host = AnyView::new(SwipeRowHost {
            label: self.label.clone(),
            is_selected: self.is_selected,
            is_enabled,
            row_height,
            max_reveal,
            reveal: reveal.clone(),
            on_select: self.on_select.clone(),
            content: self.content.clone(),
            action_row,
            trailing_slot,
        });

        let with_actions = if self.actions.is_empty() {
            host
        } else {
            AnyView::new(
                AnyView::new(ContextMenu::new(move || actions_menu(), host))
                    .frame_height(row_height),
            )
        };

        AnyView::new(AnyView::new(with_actions).frame_height(row_height).clipped())
    }
}

/// The interactive host: pointer swipe, hover band, selection and semantics.
struct SwipeRowHost {
    label: String,
    is_selected: bool,
    is_enabled: bool,
    row_height: f32,
    max_reveal: f32,
    reveal: State<f32>,
    on_select: Rc<dyn Fn()>,
    content: AnyView,
    action_row: Option<AnyView>,
    trailing_slot: Option<AnyView>,
}

impl PrimitiveView for SwipeRowHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_height(Some(self.row_height.max(1.0)));
    }

    fn update_node(&self, node: &Node) {
        let reveal = self.reveal.clone();
        node.set_corner_radius(6.0);
        node.set_cursor(Some(if self.is_enabled {
            SystemCursor::Pointer
        } else {
            SystemCursor::NotAllowed
        }));
        node.set_attachment(
            cove_composition::LayoutDebugAttachmentKey::DEBUG_NAME,
            format!("swipe-row-{}", self.label),
        );
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::ListItem,
            label: self.label.clone(),
            state: cove_core::scene::AccessibilityState {
                is_selected: self.is_selected,
                is_enabled: self.is_enabled,
                ..Default::default()
            },
            combines_children: true,
            ..Default::default()
        }));
        let select = self.on_select.clone();
        node.set_accessibility_actions(AccessibilityActions {
            activate: if self.is_enabled {
                Some(Rc::new(move || select()))
            } else {
                None
            },
            ..Default::default()
        });

        node.set_content_offset(cove_core::Point::new(-reveal.get(), 0.0));

        let Some(registry) = InteractionRegistryHolder::current() else { return };

        let is_enabled = self.is_enabled;
        let is_selected = self.is_selected;
        let max_reveal = self.max_reveal;
        let paint_node = node.clone();
        let paint: Rc<dyn Fn()> = Rc::new(move || {
            let active = paint_node.attachment::<bool>(SWIPE_ACTIVE) == Some(true);
            let hovered = paint_node.attachment::<bool>(SWIPE_HOVERED) == Some(true);
            let color = if active {
                None
            } else if is_selected {
                Some(crate::theme::theme_for_node(&paint_node).colors.state_layer_selected)
            } else if hovered && is_enabled {
                Some(crate::theme::theme_for_node(&paint_node).colors.state_layer_hover)
            } else {
                Some(cove_core::Color::CLEAR)
            };
            paint_node.set_background_color(color);
        });
        paint();

        let hover_node = node.downgrade();
        let hover_paint = paint.clone();
        registry.set_hover(node, Rc::new(move |phase: HoverPhase| {
            if let Some(owner) = Node::upgrade(&hover_node) {
                owner.set_attachment(SWIPE_HOVERED, phase == HoverPhase::Enter);
                hover_paint();
            }
        }));

        let pointer_node = node.clone();
        let pointer_reveal = reveal.clone();
        let pointer_paint = paint.clone();
        let pointer_select = self.on_select.clone();
        let environment = node.composition_value(&SurfaceEnvironment::key());
        let supports_swipe = environment.input.contains(InputCapabilities::TOUCH);
        let _ = supports_swipe;
        registry.set_pointer(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event: &cove_platform::MouseButtonEvent, phase: PointerPhase, delivery: EventPhase| {
                if delivery != EventPhase::Target || !is_enabled || event.button != MouseButton::Left {
                    return EventResult::Ignored;
                }
                match phase {
                    PointerPhase::Down => {
                        pointer_node.set_attachment(SWIPE_START, cove_core::Point::new(event.x, event.y));
                        pointer_node.set_attachment(SWIPE_BASE, pointer_reveal.get());
                        pointer_node.set_attachment(SWIPE_ACTIVE, false);
                        pointer_node.set_attachment(SWIPE_REJECTED, false);
                        EventResult::Ignored
                    }
                    PointerPhase::Up => {
                        let started = pointer_node.attachment::<cove_core::Point>(SWIPE_START).is_some();
                        let active = pointer_node.attachment::<bool>(SWIPE_ACTIVE) == Some(true);
                        let rejected = pointer_node.attachment::<bool>(SWIPE_REJECTED) == Some(true);
                        pointer_node.remove_attachment(SWIPE_START);
                        pointer_node.remove_attachment(SWIPE_BASE);
                        pointer_node.set_attachment(SWIPE_ACTIVE, false);
                        pointer_node.set_attachment(SWIPE_REJECTED, false);
                        PointerCaptureHolder::release_current();

                        if active {
                            pointer_reveal.set(swipe_snap_target(pointer_reveal.get(), max_reveal));
                            pointer_paint();
                            return EventResult::Handled;
                        }
                        if started && !rejected && pointer_reveal.get() < -0.5 {
                            pointer_reveal.set(swipe_snap_target(pointer_reveal.get(), max_reveal));
                            pointer_paint();
                            return EventResult::Handled;
                        }
                        if started
                            && !rejected
                            && pointer_node
                                .absolute_frame()
                                .contains(cove_core::Point::new(event.x, event.y))
                        {
                            (pointer_select)();
                            pointer_paint();
                            return EventResult::Handled;
                        }
                        EventResult::Ignored
                    }
                }
            }),
        );

        let motion_node = node.clone();
        let motion_reveal = reveal.clone();
        let motion_paint = paint.clone();
        let touch = node
            .composition_value(&SurfaceEnvironment::key())
            .input
            .contains(InputCapabilities::TOUCH);
        registry.set_motion(node, InputHandlerRoute::control(), Rc::new(move |event: &cove_platform::MouseMotionEvent, _phase: EventPhase| {
            if !is_enabled || !touch {
                return EventResult::Ignored;
            }
            let Some(start) = motion_node.attachment::<cove_core::Point>(SWIPE_START) else {
                return EventResult::Ignored;
            };
            let Some(base) = motion_node.attachment::<f32>(SWIPE_BASE) else {
                return EventResult::Ignored;
            };
            if motion_node.attachment::<bool>(SWIPE_REJECTED) == Some(true) {
                return EventResult::Ignored;
            }
            let dx = event.x - start.x;
            let dy = event.y - start.y;
            if motion_node.attachment::<bool>(SWIPE_ACTIVE) != Some(true) {
                if dx.abs() < SWIPE_ACTIVATION_DISTANCE || dx.abs() <= dy.abs() {
                    if dy.abs() >= SWIPE_REJECTION_DISTANCE && dy.abs() > dx.abs() {
                        motion_node.set_attachment(SWIPE_REJECTED, true);
                    }
                    return EventResult::Ignored;
                }
                motion_node.set_attachment(SWIPE_ACTIVE, true);
                if let Some(capture) = PointerCaptureHolder::current() {
                    capture.acquire(&motion_node, false);
                }
                motion_paint();
            }
            let next = (base + dx).min(0.0).max(-max_reveal);
            if (next - motion_reveal.get()).abs() > f32::EPSILON {
                motion_reveal.set(next);
                motion_node.set_content_offset(cove_core::Point::new(-next, 0.0));
            }
            EventResult::Handled
        }));

        let key_reveal = reveal.clone();
        registry.set_key(node, InputHandlerRoute::control(), Rc::new(move |event: &KeyEvent, phase: EventPhase| {
            if phase == EventPhase::Capture || event.scancode != Scancode::ESCAPE {
                return EventResult::Ignored;
            }
            if key_reveal.get() >= -0.5 {
                return EventResult::Ignored;
            }
            key_reveal.set(0.0);
            EventResult::Handled
        }));

        let clear_node = node.downgrade();
        node.set_attachment(
            PointerCaptureAttachmentKey::DID_LOSE_CAPTURE,
            Rc::new(move || {
                if let Some(owner) = Node::upgrade(&clear_node) {
                    owner.remove_attachment(SWIPE_START);
                    owner.remove_attachment(SWIPE_BASE);
                    owner.set_attachment(SWIPE_ACTIVE, false);
                    owner.set_attachment(SWIPE_REJECTED, true);
                }
            }) as Rc<dyn Fn()>,
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let mut row = vec![AnyView::new(
            AnyView::new(self.content.clone())
                .flex_with(1.0, 1.0, Some(0.0))
                .clipped(),
        )];
        if let Some(slot) = &self.trailing_slot {
            row.push(slot.clone());
        }
        let content = AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                4.0,
                AnyView::new(TupleView::new(row)),
            ))
            .frame_height(self.row_height),
        );

        match &self.action_row {
            Some(actions) => vec![AnyView::new(crate::primitives::layout::ZStack::new(
                crate::Alignment::Trailing,
                AnyView::new(TupleView::new(vec![actions.clone(), content])),
            ))],
            None => vec![content],
        }
    }
}

impl View for SwipeRowHost {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}
