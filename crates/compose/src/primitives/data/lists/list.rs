//! Translation of Primitives/Data/Lists/List.swift.
//!
//! Vertical, virtualized list with single or multiple selection. Pass an
//! `Option<Id>` binding for single selection or a set binding for multiple
//! selection. With multiple selection a plain click selects one row, Shift-click
//! extends from the anchor, Cmd/Ctrl-click toggles, Shift+Arrow extends, Space
//! toggles the cursor row and Cmd/Ctrl+A selects everything. Visual chrome is
//! delegated to the active list row style.
use std::rc::Rc;

use crate::modifiers::layout::{FlexExt, LayoutFrameExt};
use crate::primitives::controls::adaptive::{apply_touch_control_extent, supports_control_hover};
use crate::theme::styles::components::selection_appearance::{SelectionAppearance, SelectionRole};
use crate::theme::styles::components::{ListRowStyleConfiguration, ListRowStyleEnvironment};
use crate::theme::theme_for_node;
use crate::{AnyView, BoxView, PrimitiveView, View, impl_primitive_view};
use cove_core::Color;
use cove_core::scene::{
    AccessibilityActions, AccessibilityRole, AccessibilitySemantics, AccessibilityState, Align,
    EventPhase, EventResult, FlexDirection, HoverPhase, InputHandlerRoute,
    InteractionRegistryHolder, Justify, LayoutNode, Node, PointerCaptureHolder, PointerPhase, Wrap,
};
use cove_platform::{KeyEvent, MouseButton, SystemCursor};

use super::refresh::RefreshOptions;
use super::selection::CollectionSelection;

const ROW_PRESSED: &str = "__list_row_pressed";
const ROW_HOVERED: &str = "__list_row_hovered";
const ROW_ID: &str = "__list_row_id";

/// Row host: owns tap handling and resolves the active row style on every
/// recompose.
pub struct ListRowHost {
    pub id: u64,
    pub is_selected: bool,
    pub row_height: f32,
    pub on_activate: Rc<dyn Fn()>,
    pub on_tap: Option<Rc<dyn Fn(cove_platform::KeyModifiers)>>,
    pub content: AnyView,
}

impl PrimitiveView for ListRowHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_height(Some(self.row_height));
        Some(layout)
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_height(Some(self.row_height));
    }

    fn update_node(&self, node: &Node) {
        apply_touch_control_extent(node, true);
        node.set_attachment(ROW_ID, self.id);
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::ListItem,
            state: AccessibilityState {
                is_selected: self.is_selected,
                ..Default::default()
            },
            combines_children: true,
            ..Default::default()
        }));
        let activate = self.on_activate.clone();
        let activate_node = node.downgrade();
        node.set_accessibility_actions(AccessibilityActions {
            activate: Some(Rc::new(move || {
                if let Some(owner) = Node::upgrade(&activate_node) {
                    activate();
                    owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                        "listRowActivate",
                    ));
                }
            })),
            ..Default::default()
        });

        let clear_node = node.downgrade();
        node.set_attachment(
            cove_core::scene::PointerCaptureAttachmentKey::DID_LOSE_CAPTURE,
            Rc::new(move || {
                if let Some(owner) = Node::upgrade(&clear_node) {
                    owner.set_attachment(ROW_PRESSED, false);
                    owner.set_background_color(Some(Color::CLEAR));
                }
            }) as Rc<dyn Fn()>,
        );

        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        node.set_cursor(Some(SystemCursor::Pointer));
        // Hover paints on the host so entering a row never recomposes it. The
        // radius comes from the shared selection appearance, so the hover band
        // is not the one square thing left on the row.
        let touch = node
            .composition_value(&crate::adaptive::SurfaceEnvironment::key())
            .input
            .contains(cove_core::InputCapabilities::TOUCH);
        let radius = if touch {
            0.0
        } else {
            SelectionAppearance::resolve(SelectionRole::Navigation, &theme_for_node(node))
                .corner_radius
        };
        node.set_corner_radius(radius);
        let is_selected = self.is_selected;
        // Shared by the initial paint and the hover handler, so it must be
        // callable more than once: capture the weak handle by clone each call.
        let hover_fn: Rc<dyn Fn(bool)> = Rc::new({
            let hover_node = node.downgrade();
            move |hovered: bool| {
                let Some(owner) = Node::upgrade(&hover_node) else {
                    return;
                };
                let hovered = hovered && supports_control_hover(&owner);
                owner.set_attachment(ROW_HOVERED, hovered);
                let fill = if hovered && !is_selected {
                    theme_for_node(&owner).colors.state_layer_hover
                } else {
                    Color::CLEAR
                };
                owner.set_background_color(Some(fill));
            }
        });
        hover_fn(node.attachment::<bool>(ROW_HOVERED) == Some(true));
        let hover_fn_for_hover = hover_fn.clone();
        registry.set_hover(
            node,
            Rc::new(move |phase| {
                hover_fn_for_hover(phase == HoverPhase::Enter);
            }),
        );

        let captured = self.on_activate.clone();
        let on_tap = self.on_tap.clone();
        let hover_fn_for_pointer = hover_fn.clone();
        let pointer_node = node.downgrade();
        registry.set_pointer(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase, event_phase| {
                // Embedded controls get first refusal before row selection.
                if event_phase == EventPhase::Capture || event.button != MouseButton::Left {
                    return EventResult::Ignored;
                }
                let Some(owner) = Node::upgrade(&pointer_node) else {
                    return EventResult::Ignored;
                };
                match phase {
                    PointerPhase::Down => {
                        owner.set_attachment(ROW_PRESSED, true);
                        let touch = owner
                            .composition_value(&crate::adaptive::SurfaceEnvironment::key())
                            .input
                            .contains(cove_core::InputCapabilities::TOUCH);
                        if touch {
                            owner.set_background_color(Some(
                                theme_for_node(&owner).colors.state_layer_pressed,
                            ));
                        }
                        if let Some(capture) = PointerCaptureHolder::current() {
                            capture.acquire(&owner, true);
                        }
                        EventResult::Handled
                    }
                    PointerPhase::Up => {
                        let was_pressed = owner.attachment::<bool>(ROW_PRESSED).unwrap_or(false);
                        owner.set_attachment(ROW_PRESSED, false);
                        hover_fn_for_pointer(false);
                        if PointerCaptureHolder::current()
                            .and_then(|capture| capture.target())
                            .is_some_and(|target| target == owner)
                            && let Some(capture) = PointerCaptureHolder::current()
                        {
                            capture.release();
                        }
                        if !was_pressed {
                            return EventResult::Ignored;
                        }
                        let frame = owner.absolute_frame();
                        if event.x >= frame.x
                            && event.x <= frame.x + frame.width
                            && event.y >= frame.y
                            && event.y <= frame.y + frame.height
                        {
                            match on_tap.as_ref() {
                                Some(on_tap) => on_tap(event.modifiers),
                                None => captured(),
                            }
                        }
                        EventResult::Handled
                    }
                }
            }),
        );
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let style = node.composition_value(&ListRowStyleEnvironment::key());
        let theme = theme_for_node(node);
        let input = node
            .composition_value(&crate::adaptive::SurfaceEnvironment::key())
            .input;
        vec![style.make_body(ListRowStyleConfiguration {
            content: self.content.clone(),
            is_selected: self.is_selected,
            is_hovered: node.attachment::<bool>(ROW_HOVERED).unwrap_or(false),
            is_enabled: true,
            row_height: self.row_height,
            theme,
            input,
        })]
    }
}
impl_primitive_view!(ListRowHost);

/// Vertical, virtualized list with single or multiple selection.
pub struct List<Id: Ord + Clone + Send + Sync + 'static> {
    pub ids: Vec<Id>,
    pub selection: CollectionSelection<Id>,
    /// Authored logical height, before text scaling and the touch density floor.
    pub row_height: Option<f32>,
    pub row_spacing: f32,
    pub on_activate: Option<Rc<dyn Fn(Id)>>,
    pub refresh: Option<RefreshOptions>,
    pub row_content: Rc<dyn Fn(Id, bool) -> AnyView>,
    anchor: cove_core::State<Option<Id>>,
    cursor: cove_core::State<Option<Id>>,
}

impl<Id: Ord + Clone + Send + Sync + 'static> List<Id> {
    pub fn new(
        selection: CollectionSelection<Id>,
        row_content: impl Fn(Id, bool) -> AnyView + 'static,
    ) -> Self {
        Self {
            ids: Vec::new(),
            selection,
            row_height: None,
            row_spacing: 0.0,
            on_activate: None,
            refresh: None,
            row_content: Rc::new(row_content),
            anchor: cove_core::State::new(None),
            cursor: cove_core::State::new(None),
        }
    }

    pub fn rows(mut self, ids: Vec<Id>) -> Self {
        self.ids = ids;
        self
    }

    pub fn row_height(mut self, row_height: f32) -> Self {
        self.row_height = Some(if row_height.is_finite() {
            row_height.max(1.0)
        } else {
            30.0
        });
        self
    }

    pub fn row_spacing(mut self, row_spacing: f32) -> Self {
        self.row_spacing = row_spacing.max(0.0);
        self
    }

    pub fn on_activate(mut self, handler: impl Fn(Id) + 'static) -> Self {
        self.on_activate = Some(Rc::new(handler));
        self
    }

    pub fn refresh(mut self, refresh: RefreshOptions) -> Self {
        self.refresh = Some(refresh);
        self
    }

    fn index_of(&self, target: &Id) -> Option<usize> {
        self.ids.iter().position(|id| id == target)
    }

    fn ids_between(&self, first: &Id, second: &Id) -> Option<Vec<Id>> {
        let start = self.index_of(first)?;
        let end = self.index_of(second)?;
        let (low, high) = (start.min(end), start.max(end));
        Some(self.ids[low..=high].to_vec())
    }

    #[expect(dead_code, reason = "ported from Swift but not wired in yet")]
    fn tap(&self, id: Id, modifiers: cove_platform::KeyModifiers) {
        if modifiers.has_shift()
            && let Some(anchor) = self.anchor.get()
            && let Some(range) = self.ids_between(&anchor, &id)
        {
            self.selection.replace(range);
        } else if modifiers.has_gui() || modifiers.has_ctrl() {
            self.selection.toggle(id.clone());
            self.anchor.set(Some(id.clone()));
        } else {
            self.selection.replace([id.clone()]);
            self.anchor.set(Some(id.clone()));
        }
        self.cursor.set(Some(id));
    }

    fn activate(&self, id: Id) {
        self.selection.replace([id.clone()]);
        self.anchor.set(Some(id.clone()));
        self.cursor.set(Some(id.clone()));
        if let Some(handler) = self.on_activate.as_ref() {
            handler(id);
        }
    }

    /// Keyboard handling for the cursor row.
    pub fn handle_key(&self, event: &KeyEvent) -> bool {
        if self.ids.is_empty() {
            return false;
        }
        let multiple = self.selection.allows_multiple();
        let current = (if multiple { self.cursor.get() } else { None })
            .and_then(|id| self.index_of(&id))
            .or_else(|| self.selection.first().and_then(|id| self.index_of(&id)));
        if multiple
            && (event.modifiers.has_gui() || event.modifiers.has_ctrl())
            && event.scancode == crate::Scancode::A
        {
            self.selection.replace(self.ids.clone());
            self.anchor.set(self.ids.first().cloned());
            self.cursor.set(self.ids.last().cloned());
            return true;
        }
        if multiple
            && event.scancode == crate::Scancode::SPACE
            && let Some(current) = current
        {
            let value = self.ids[current].clone();
            self.selection.toggle(value.clone());
            self.anchor.set(Some(value.clone()));
            self.cursor.set(Some(value));
            return true;
        }
        let next = match event.scancode {
            crate::Scancode::ARROW_DOWN => {
                (current.map_or(0, |index| index + 1)).min(self.ids.len() - 1)
            }
            crate::Scancode::ARROW_UP => {
                current.map_or(self.ids.len() - 1, |index| index.saturating_sub(1))
            }
            crate::Scancode::HOME => 0,
            crate::Scancode::END => self.ids.len() - 1,
            crate::Scancode::RETURN | crate::Scancode::KEYPAD_ENTER => {
                let Some(current) = current else { return false };
                let value = self.ids[current].clone();
                if multiple {
                    self.activate(value);
                } else if let Some(handler) = self.on_activate.as_ref() {
                    handler(value);
                }
                return true;
            }
            _ => return false,
        };
        let value = self.ids[next].clone();
        if multiple
            && event.modifiers.has_shift()
            && let Some(anchor) = self.anchor.get()
            && let Some(range) = self.ids_between(&anchor, &value)
        {
            self.selection.replace(range);
        } else {
            self.selection.replace([value.clone()]);
            self.anchor.set(Some(value.clone()));
        }
        self.cursor.set(Some(value));
        true
    }

    fn rows_view(&self) -> AnyView {
        let touch = false;
        let effective_height = self.row_height.unwrap_or(if touch { 56.0 } else { 30.0 });
        let mut rows: Vec<AnyView> = Vec::new();
        for id in &self.ids {
            let selected = self.selection.contains(id);
            let tap_id = id.clone();
            let activate_id = id.clone();
            let selection = self.selection.clone();
            let anchor = self.anchor.clone();
            let cursor = self.cursor.clone();
            let on_activate = self.on_activate.clone();
            let multiple = self.selection.allows_multiple();
            let ids = self.ids.clone();
            let content = (self.row_content)(id.clone(), selected);
            let activate_selection = selection.clone();
            let activate_anchor = anchor.clone();
            let activate_cursor = cursor.clone();
            rows.push(AnyView::new(ListRowHost {
                id: 0,
                is_selected: selected,
                row_height: effective_height,
                on_activate: Rc::new(move || {
                    activate_selection.replace([activate_id.clone()]);
                    activate_anchor.set(Some(activate_id.clone()));
                    activate_cursor.set(Some(activate_id.clone()));
                    if let Some(handler) = on_activate.as_ref() {
                        handler(activate_id.clone());
                    }
                }),
                on_tap: multiple.then(|| {
                    let selection = selection.clone();
                    let anchor = anchor.clone();
                    let cursor = cursor.clone();
                    let ids = ids.clone();
                    Rc::new(move |modifiers: cove_platform::KeyModifiers| {
                        let index_of = |target: &Id| ids.iter().position(|id| id == target);
                        if modifiers.has_shift()
                            && let Some(from) = anchor.get()
                            && let (Some(start), Some(end)) = (index_of(&from), index_of(&tap_id))
                        {
                            let (low, high) = (start.min(end), start.max(end));
                            selection.replace(ids[low..=high].to_vec());
                        } else if modifiers.has_gui() || modifiers.has_ctrl() {
                            selection.toggle(tap_id.clone());
                            anchor.set(Some(tap_id.clone()));
                        } else {
                            selection.replace([tap_id.clone()]);
                            anchor.set(Some(tap_id.clone()));
                        }
                        cursor.set(Some(tap_id.clone()));
                    }) as Rc<dyn Fn(cove_platform::KeyModifiers)>
                }),
                content,
            }));
        }
        let scroll = crate::primitives::layout::ScrollView::vertical(AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            self.row_spacing,
            AnyView::new(crate::TupleView::new(rows)),
        )));
        AnyView::new(
            scroll
                .flex(1.0)
                .frame(None, None, Some(0.0), Some(0.0), None, None),
        )
    }
}

impl<Id: Ord + Clone + Send + Sync + 'static> View for List<Id> {
    fn body(&self) -> AnyView {
        let rows = self.rows_view();
        let host = AnyView::new(ListKeyHost {
            list: self.snapshot(),
            content: rows,
        });
        AnyView::new(
            host.flex(1.0)
                .frame(None, None, Some(0.0), Some(0.0), None, None),
        )
    }
}

impl<Id: Ord + Clone + Send + Sync + 'static> List<Id> {
    fn snapshot(&self) -> List<Id> {
        List {
            ids: self.ids.clone(),
            selection: self.selection.clone(),
            row_height: self.row_height,
            row_spacing: self.row_spacing,
            on_activate: self.on_activate.clone(),
            refresh: self.refresh.clone(),
            row_content: self.row_content.clone(),
            anchor: self.anchor.clone(),
            cursor: self.cursor.clone(),
        }
    }
}

/// Owns the keyboard routing for the list.
struct ListKeyHost<Id: Ord + Clone + Send + Sync + 'static> {
    list: List<Id>,
    content: AnyView,
}

impl<Id: Ord + Clone + Send + Sync + 'static> PrimitiveView for ListKeyHost<Id> {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        let list = self.list.snapshot();
        registry.set_key(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase| {
                if phase == EventPhase::Capture {
                    return EventResult::Ignored;
                }
                if list.handle_key(event) {
                    EventResult::Handled
                } else {
                    EventResult::Ignored
                }
            }),
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl<Id: Ord + Clone + Send + Sync + 'static> View for ListKeyHost<Id> {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}
