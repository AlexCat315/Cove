//! Translation of Primitives/Data/Lists/List.swift.
//!
//! Vertical, virtualized list with single or multiple selection. Pass an
//! `Option<Id>` binding for single selection or a set binding for multiple
//! selection. With multiple selection a plain click selects one row, Shift-click
//! extends from the anchor, Cmd/Ctrl-click toggles, Shift+Arrow extends, Space
//! toggles the cursor row and Cmd/Ctrl+A selects everything. Visual chrome is
//! delegated to the active list row style.
use crate::Scope;
use std::rc::Rc;

use crate::adaptive::SurfaceReader;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt};
use crate::primitives::controls::adaptive::{apply_touch_control_extent, supports_control_hover};
use crate::primitives::layout::virtualization::FixedVirtualStack;
use crate::primitives::overlays::selection_key_host::SelectionKeyHost;
use crate::theme::styles::components::selection_appearance::{SelectionAppearance, SelectionRole};
use crate::theme::styles::components::{ListRowStyleConfiguration, ListRowStyleEnvironment};
use crate::theme::{TextScalerReader, theme_for_node};
use crate::{AnyView, PrimitiveView, View, ViewExt, impl_primitive_view};
use cove_core::scene::{
    AccessibilityActions, AccessibilityRole, AccessibilitySemantics, AccessibilityState,
    EventPhase, EventResult, HoverPhase, InputHandlerRoute, InteractionRegistryHolder, LayoutNode,
    Node, PointerCaptureHolder, PointerPhase,
};
use cove_core::{Color, Environment, InputCapabilities};
use cove_platform::{KeyEvent, MouseButton, SystemCursor};

use super::refresh::RefreshOptions;
use super::selection::{CollectionId, ListCore, RowState, Selection};

const ROW_PRESSED: &str = "__list_row_pressed";
const ROW_HOVERED: &str = "__list_row_hovered";
const ROW_ID: &str = "__list_row_id";

/// Row host: owns tap handling and resolves the active row style on every
/// recompose.
pub(crate) struct ListRowHost {
    pub id: u64,
    pub is_selected: bool,
    pub row_height: f32,
    pub shows_separator: bool,
    pub on_activate: Rc<dyn Fn()>,
    pub on_tap: Option<Rc<dyn Fn(cove_platform::KeyModifiers)>>,
    pub content: AnyView,
}

impl PrimitiveView for ListRowHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        // A row is a fixed virtualized slot; taller content must not bleed into
        // the neighbouring row.
        node.set_clips_to_bounds(true);
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
            shows_separator: self.shows_separator,
            theme,
            input,
        })]
    }
}
impl_primitive_view!(ListRowHost);

/// Vertical, virtualized list with single or multiple selection.
pub struct List<Id: CollectionId> {
    pub ids: Rc<[Id]>,
    pub selection: Selection<Id>,
    /// Authored logical height, before text scaling and the touch density floor.
    pub row_height: Option<f32>,
    pub row_spacing: f32,
    pub on_activate: Option<Rc<dyn Fn(&Id)>>,
    pub refresh: Option<RefreshOptions>,
    pub row_content: Rc<dyn Fn(&Id, RowState) -> AnyView>,
    anchor: cove_core::State<Option<Id>>,
    cursor: cove_core::State<Option<Id>>,
}

impl<Id: CollectionId> List<Id> {
    pub fn new(ids: Vec<Id>, row_content: impl Fn(&Id, RowState) -> AnyView + 'static) -> Self {
        Self {
            ids: ids.into(),
            selection: Selection::none(),
            row_height: None,
            row_spacing: 0.0,
            on_activate: None,
            refresh: None,
            row_content: Rc::new(row_content),
            anchor: cove_core::State::new(None),
            cursor: cove_core::State::new(None),
        }
    }

    pub fn selection(mut self, selection: Selection<Id>) -> Self {
        self.selection = selection;
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

    pub fn on_activate(mut self, handler: impl Fn(&Id) + 'static) -> Self {
        self.on_activate = Some(Rc::new(handler));
        self
    }

    pub fn refresh(mut self, refresh: RefreshOptions) -> Self {
        self.refresh = Some(refresh);
        self
    }

    fn core(&self) -> ListCore<Id> {
        ListCore {
            ids: self.ids.clone(),
            selection: self.selection.clone(),
            anchor: self.anchor.clone(),
            cursor: self.cursor.clone(),
            on_activate: self.on_activate.clone(),
        }
    }

    /// Keyboard handling for the cursor row.
    pub fn handle_key(&self, event: &KeyEvent) -> bool {
        self.core().handle_key(event)
    }

    fn rows_view(&self) -> AnyView {
        let list = self.snapshot();
        AnyView::new(SurfaceReader::new(move |surface: &Environment| {
            let list = list.snapshot();
            let touch = surface.input.contains(InputCapabilities::TOUCH);
            let minimum = if touch {
                surface.minimum_control_extent()
            } else {
                0.0
            };
            TextScalerReader::new(move |scaler| {
                let authored = list.row_height.unwrap_or(if touch { 56.0 } else { 30.0 });
                let height = scaler.scale(authored).max(minimum);
                let core = list.core();
                let target = core.target();
                let rows = list.row_content.clone();
                let last = list.ids.last().cloned();
                // The id after each row: a hairline never runs into the
                // selection band below it, as in a macOS inset list.
                let next: Rc<std::collections::HashMap<Id, Id>> = Rc::new(
                    list.ids
                        .windows(2)
                        .map(|pair| (pair[0].clone(), pair[1].clone()))
                        .collect(),
                );
                let mut stack =
                    FixedVirtualStack::new(list.ids.to_vec(), height, move |id: &Id| {
                        let selected = core.selection.contains(id);
                        let activate = core.clone();
                        let activate_id = id.clone();
                        let on_tap = core.selection.allows_multiple().then(|| {
                            let tap = core.clone();
                            let tap_id = id.clone();
                            Rc::new(move |modifiers: cove_platform::KeyModifiers| {
                                tap.tap(tap_id.clone(), modifiers)
                            })
                                as Rc<dyn Fn(cove_platform::KeyModifiers)>
                        });
                        AnyView::new(ListRowHost {
                            id: 0,
                            is_selected: selected,
                            row_height: height,
                            shows_separator: last.as_ref() != Some(id)
                                && !next
                                    .get(id)
                                    .is_some_and(|after| core.selection.contains(after)),
                            on_activate: Rc::new(move || activate.activate(activate_id.clone())),
                            on_tap,
                            content: rows(id, RowState::selected(selected)),
                        })
                    })
                    .spacing(scaler.scale(list.row_spacing));
                if let Some(index) = target {
                    stack = stack.scroll_to_index(index);
                }
                AnyView::new(stack)
                    .flex(1.0)
                    .frame(None, None, Some(0.0), Some(0.0), None, None)
            })
        }))
    }
}

impl<Id: CollectionId> View for List<Id> {
    fn body(&self, _cx: &Scope) -> AnyView {
        let rows = self.rows_view();
        let core = self.core();
        let host = AnyView::new(SelectionKeyHost::new(rows, true, move |event| {
            core.handle_key(event)
        }));
        // An inset list: one rounded box in the surface fill, its rows inset
        // so the selection band clears the box edge.
        AnyView::new(
            host.flex(1.0)
                .frame(None, None, Some(0.0), Some(0.0), None, None)
                .modifier(InsetListBox),
        )
    }

    /// Swift keeps `anchor` and `cursor` in `@State`; declaring them carries the
    /// values over when the parent rebuilds the list.
    fn dynamic_properties(&self) -> Vec<crate::DynamicField> {
        vec![
            crate::DynamicField::state("anchor", &self.anchor),
            crate::DynamicField::state("cursor", &self.cursor),
        ]
    }
}

impl<Id: CollectionId> List<Id> {
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

/// The macOS inset list box: surface fill, group radius and a small side
/// inset so the rounded selection band never touches the box edge.
pub(crate) struct InsetListBox;

impl cove_composition::ViewModifier for InsetListBox {
    fn apply_node(&self, node: &Node) {
        let theme = theme_for_node(node);
        node.set_background_color(Some(theme.colors.surface));
        node.set_corner_radius(theme.radius.lg);
    }

    fn apply_layout(&self, layout: &cove_core::scene::LayoutNode) {
        // Horizontal only: an authored list height stays the rows' height.
        layout.set_padding(4.0, cove_core::scene::Edge::Left);
        layout.set_padding(4.0, cove_core::scene::Edge::Right);
    }
}
