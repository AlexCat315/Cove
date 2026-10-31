//! Translation of `Primitives/Overlays/ActionSheet.swift` and
//! `Primitives/Overlays/ActionSheet/Content.swift`.
//!
//! An action drawer attached to a caller-owned trigger. The sheet is one bottom
//! drawer at every width: compact surfaces get a full-width panel with a drag
//! handle and no close control; wider surfaces get a centered, bounded drawer
//! with a top close control. The caller owns the visible labels and supplies the
//! localized accessibility name for that close control, so the component never
//! writes interface copy such as "Cancel" or "Close" itself.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{EdgeInsets, FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::button::{
    ActionControlAccessibility, Button, ButtonIcon, ButtonIconSource, ButtonRole,
};
use crate::primitives::layout::{ScrollAxis, ScrollView, ScrollbarGutter};
use crate::primitives::media::ui_icon_catalog;
use crate::primitives::overlays::modal::{
    Modal, ModalBackdrop, ModalOptions, ModalPlacement, SheetDragHandle,
};
use crate::primitives::overlays::roving_focus::{
    RovingFocusHost, RovingItem, SelectionNavigation, roving_identity,
};
use crate::primitives::text::{Divider, DividerAxis, Text};
use crate::theme::styles::buttons::{
    ButtonStyle, ButtonStyleConfiguration, ButtonStyleViewExt, GhostButtonStyle,
    SecondaryButtonStyle,
};
use crate::theme::{
    ControlSize, ControlSizeViewExt, SemanticColorRef, SemanticFontRef, theme_for_node,
};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View, ViewExt};
use cove_core::scene::{AccessibilityRole, Align, FlexDirection, Justify, Node, Wrap};
use cove_core::{Binding, Color, Size};
use cove_imaging::BundleImageResource;

/// A stable, caller-owned command. Dismissal is supplied separately.
#[derive(Clone)]
pub struct ActionSheetAction {
    pub id: String,
    pub title: String,
    pub icon: Option<BundleImageResource>,
    pub role: ButtonRole,
    pub is_enabled: bool,
    action: Rc<dyn Fn()>,
}

impl ActionSheetAction {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        action: impl Fn() + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            icon: None,
            role: ButtonRole::Normal,
            is_enabled: true,
            action: Rc::new(action),
        }
    }

    pub fn icon(mut self, icon: BundleImageResource) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn role(mut self, role: ButtonRole) -> Self {
        self.role = role;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.is_enabled = enabled;
        self
    }

    /// Runs the action.
    ///
    /// Public because a caller may also reach an action from a keyboard shortcut
    /// or a command palette rather than the sheet's own row.
    pub fn perform(&self) {
        (self.action)();
    }
}

/// Title, explanation and the actions themselves.
#[derive(Clone)]
pub struct ActionSheetQuestion {
    pub title: String,
    pub message: String,
    pub actions: Vec<ActionSheetAction>,
    pub close_label: String,
}

impl ActionSheetQuestion {
    pub fn new(
        title: impl Into<String>,
        message: impl Into<String>,
        actions: Vec<ActionSheetAction>,
        close_label: impl Into<String>,
    ) -> Self {
        let mut seen = std::collections::HashSet::new();
        for action in &actions {
            assert!(
                seen.insert(action.id.clone()),
                "ActionSheet action IDs must be unique"
            );
        }
        Self {
            title: title.into(),
            message: message.into(),
            actions,
            close_label: close_label.into(),
        }
    }

    fn enabled_ids(&self) -> Vec<String> {
        self.actions
            .iter()
            .filter(|action| action.is_enabled)
            .map(|action| action.id.clone())
            .collect()
    }
}

/// Action drawer attached to a caller-owned trigger.
pub struct ActionSheet {
    pub is_presented: Binding<bool>,
    pub is_enabled: bool,
    pub question: ActionSheetQuestion,
    label: AnyView,
    state: Rc<ActionSheetState>,
}

struct ActionSheetState {
    drag_offset: Cell<f32>,
    highlighted: RefCell<Option<String>>,
}

struct ActionSheetTrigger {
    is_presented: Binding<bool>,
    is_enabled: bool,
    label: AnyView,
    drag_state: Rc<ActionSheetState>,
}

impl PrimitiveView for ActionSheetTrigger {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, _node: &Node) {}
    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let is_presented = self.is_presented.clone();
        let drag_state = self.drag_state.clone();
        let label = self.label.clone();
        let enabled = self.is_enabled;
        let expanded = Cell::new(false);
        let _ = &expanded;
        vec![AnyView::new(
            Button::with_options(
                label,
                move || {
                    drag_state.drag_offset.set(0.0);
                    is_presented.set(!is_presented.get());
                },
                ButtonRole::Normal,
                enabled,
                false,
                false,
                None,
            )
            .button_style(SecondaryButtonStyle),
        )]
    }
}

impl View for ActionSheetTrigger {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

impl ActionSheet {
    pub fn new(
        is_presented: Binding<bool>,
        question: ActionSheetQuestion,
        label: impl View + 'static,
    ) -> Self {
        Self {
            is_presented,
            is_enabled: true,
            question,
            label: AnyView::new(label),
            state: Rc::new(ActionSheetState {
                drag_offset: Cell::new(0.0),
                highlighted: RefCell::new(None),
            }),
        }
    }

    pub fn is_enabled(mut self, enabled: bool) -> Self {
        self.is_enabled = enabled;
        self
    }

    fn is_active(&self) -> bool {
        self.is_enabled && !self.question.actions.is_empty()
    }
}

impl View for ActionSheet {
    fn body(&self) -> AnyView {
        let presentable = self.is_active();
        // A sheet with nothing to present cannot stay open.
        if !presentable && self.is_presented.get() {
            self.is_presented.set(false);
        }
        let trigger = AnyView::new(ActionSheetTrigger {
            is_presented: self.is_presented.clone(),
            is_enabled: presentable,
            label: self.label.clone(),
            drag_state: self.state.clone(),
        });
        let drawer = if presentable {
            AnyView::new(ActionSheetDrawer {
                is_presented: self.is_presented.clone(),
                question: self.question.clone(),
                state: self.state.clone(),
            })
        } else {
            AnyView::new(crate::EmptyView)
        };
        AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::FlexStart,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(vec![trigger, drawer])),
            ))
            .z_index(if self.is_presented.get() {
                10_000.0
            } else {
                0.0
            }),
        )
    }
}

/// Measures the drawer, then hands the sized payload to `Modal`.
struct ActionSheetDrawer {
    is_presented: Binding<bool>,
    question: ActionSheetQuestion,
    state: Rc<ActionSheetState>,
}

impl PrimitiveView for ActionSheetDrawer {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, _node: &Node) {}
    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }
    fn children(&self, node: &Node) -> Vec<AnyView> {
        let theme = theme_for_node(node);
        let surface: cove_core::Environment =
            node.composition_value(&crate::adaptive::SurfaceEnvironment::key());
        let window_height = surface.viewport.height;
        let is_compact = surface.viewport.width_class() == cove_core::SizeClass::Compact;
        let safe_bottom = surface.safe_area.bottom.max(0.0);
        let inset = if is_compact { 0.0 } else { 12.0 };
        let width = if is_compact {
            surface.viewport.width.max(64.0)
        } else {
            560.0_f32.min(220.0_f32.max(surface.viewport.width - inset * 2.0))
        };
        // The drawer content is a handle (compact only), a fixed header and a
        // scrollable action list. Reserve the chrome height so a long list never
        // pushes the header or close control off a short window.
        let top_clearance = if is_compact { 68.0 } else { 48.0 };
        let chrome = (if is_compact { 44.0 } else { 0.0 }) + 140.0;
        let action_viewport = 88.0_f32.max(window_height - top_clearance - safe_bottom - chrome);

        let mut modal_options = ModalOptions::default();
        modal_options.geometry.placement = ModalPlacement::Bottom;
        modal_options.geometry.inset = inset;
        modal_options.geometry.width = width;
        modal_options.geometry.height = f32::MAX;
        modal_options.geometry.corner_radius = 18.0;
        modal_options.fits_content_height = true;
        modal_options.backdrop = ModalBackdrop::Dimmed;
        modal_options.surface = SemanticColorRef::surface_floating();
        modal_options.translation_y = self.state.drag_offset.get();
        let _ = &theme;

        let content = AnyView::new(ActionSheetContent {
            question: self.question.clone(),
            is_presented: self.is_presented.clone(),
            maximum_height: action_viewport,
            shows_handle: is_compact,
            state: self.state.clone(),
        })
        .padding_insets(EdgeInsets::new(0.0, 0.0, safe_bottom, 0.0));

        vec![AnyView::new(
            Modal::new(self.is_presented.clone(), content).options(modal_options),
        )]
    }
}

impl View for ActionSheetDrawer {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

struct ActionSheetContent {
    question: ActionSheetQuestion,
    is_presented: Binding<bool>,
    maximum_height: f32,
    shows_handle: bool,
    state: Rc<ActionSheetState>,
}

impl PrimitiveView for ActionSheetContent {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, _node: &Node) {}
    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let enabled = self.question.enabled_ids();
        let highlighted = self.state.highlighted.borrow().clone();
        let current = highlighted
            .filter(|id| enabled.contains(id))
            .or_else(|| enabled.first().cloned());

        let mut column: Vec<AnyView> = Vec::new();
        if self.shows_handle {
            let drag_state = self.state.clone();
            let presented = self.is_presented.clone();
            column.push(AnyView::new(
                SheetDragHandle::new(
                    {
                        let drag_state = drag_state.clone();
                        move |translation: f32| drag_state.drag_offset.set(translation.max(0.0))
                    },
                    move |distance: f32, velocity: f32| {
                        drag_state.drag_offset.set(0.0);
                        if distance > 80.0 || (distance > 0.0 && velocity > 900.0) {
                            presented.set(false);
                        }
                    },
                )
                .accessibility_hidden(true),
            ));
        }

        let mut header: Vec<AnyView> = vec![AnyView::new(
            Text::new(self.question.title.clone()).semantic_font(SemanticFontRef::body_strong()),
        )];
        if !self.question.message.is_empty() {
            header.push(AnyView::new(
                Text::new(self.question.message.clone())
                    .semantic_font(SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
            ));
        }
        // Wider surfaces carry the close control in the header's trailing slot
        // instead of the compact drag handle, so the header reserves 52pt for it.
        let mut header_row: Vec<AnyView> = vec![AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                6.0,
                AnyView::new(TupleView::new(header)),
            ))
            .flex(1.0)
            .shrink(1.0),
        )];
        if !self.shows_handle {
            header_row.push(self.close_button());
        }
        column.push(AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                8.0,
                AnyView::new(TupleView::new(header_row)),
            ))
            .padding_insets(EdgeInsets::new(8.0, 12.0, 10.0, 12.0))
            .frame(None, None, None, Some(44.0), None, None),
        ));
        column.push(AnyView::new(Divider::new(
            None,
            1.0,
            DividerAxis::Horizontal,
        )));

        let rows = self.rows(&enabled, current.as_deref());
        let roving = RovingFocusHost::new(
            enabled.clone(),
            {
                let current = current.clone();
                move || current.clone()
            },
            {
                let state = self.state.clone();
                move |id: String| *state.highlighted.borrow_mut() = Some(id)
            },
            |id: &String| roving_identity(id),
            rows,
        )
        .command(|event: &cove_platform::KeyEvent| match event.scancode {
            crate::Scancode::ARROW_UP
            | crate::Scancode::ARROW_DOWN
            | crate::Scancode::HOME
            | crate::Scancode::END => SelectionNavigation::command_for(event.scancode),
            _ => None,
        });
        let scroller = ScrollView::new(ScrollAxis::Vertical, AnyView::new(roving))
            .scrollbar_gutter(ScrollbarGutter::Overlay);
        column.push(AnyView::new(AnyView::new(scroller).frame(
            None,
            None,
            None,
            None,
            None,
            Some(self.maximum_height),
        )));

        let mut body = AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(column)),
            ))
            .frame_min_width(0.0),
        );

        let label = self.question.title.clone();
        let help = self.question.message.clone();
        vec![AnyView::new(body.accessibility(move |semantics| {
            semantics.role = AccessibilityRole::Dialog;
            semantics.label = label.clone();
            semantics.help = help.clone();
        }))]
    }
}

impl ActionSheetContent {
    fn rows(&self, enabled: &[String], current: Option<&str>) -> AnyView {
        let mut items: Vec<AnyView> = Vec::new();
        for (index, action) in self.question.actions.iter().enumerate() {
            let is_tab_stop = current == Some(action.id.as_str());
            let identity: Rc<dyn Fn(&String) -> u64> = Rc::new(|id: &String| roving_identity(id));
            let row = self.row(action);
            items.push(AnyView::new(AnyView::new(row.modifier(RovingItem::new(
                action.id.clone(),
                is_tab_stop,
                identity,
            )))));
            if index < self.question.actions.len() - 1 {
                items.push(AnyView::new(Divider::new(
                    None,
                    1.0,
                    DividerAxis::Horizontal,
                )));
            }
        }
        AnyView::new(
            AnyView::new(
                AnyView::new(BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(TupleView::new(items)),
                ))
                .padding_edges(0.0, 4.0),
            )
            .accessibility_hidden(enabled.is_empty()),
        )
    }

    fn row(&self, action: &ActionSheetAction) -> AnyView {
        let color = if !action.is_enabled {
            SemanticColorRef::on_surface_disabled()
        } else if action.role == ButtonRole::Destructive {
            SemanticColorRef::error()
        } else {
            SemanticColorRef::on_surface()
        };
        let mut label: Vec<AnyView> = Vec::new();
        if let Some(icon) = action.icon.clone() {
            label.push(AnyView::new(
                ButtonIcon::new(ButtonIconSource::resource(icon), Some(14.0), None)
                    .accessibility_hidden(true),
            ));
        }
        label.push(AnyView::new(
            Text::new(action.title.clone())
                .semantic_foreground(color)
                .frame_min_width(0.0)
                .flex(1.0)
                .shrink(1.0)
                .flex_basis(Some(0.0)),
        ));
        let content = AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                10.0,
                AnyView::new(TupleView::new(label)),
            ))
            .frame_min_width(0.0),
        );

        let presented = self.is_presented.clone();
        let role = action.role;
        let is_enabled = action.is_enabled;
        let title = action.title.clone();
        let activated = action.clone();
        AnyView::new(
            Button::with_options(
                content,
                move || {
                    if !presented.get() || !activated.is_enabled {
                        return;
                    }
                    presented.set(false);
                    activated.perform();
                },
                role,
                is_enabled,
                false,
                false,
                None,
            )
            .button_style(ActionSheetRowStyle)
            .accessibility_label(title),
        )
    }

    fn close_button(&self) -> AnyView {
        let presented = self.is_presented.clone();
        let close_label = self.question.close_label.clone();
        AnyView::new(
            Button::icon(
                ButtonIconSource::resource(ui_icon_catalog::action::x()),
                14.0,
                move || presented.set(false),
            )
            .tooltip(close_label.clone())
            .button_style(GhostButtonStyle)
            .accessibility_label(close_label.clone())
            .control_size(ControlSize::Small)
            .frame(
                Some(crate::modifiers::layout::FrameDimension::Points(44.0)),
                Some(crate::modifiers::layout::FrameDimension::Points(44.0)),
                None,
                None,
                None,
                None,
            ),
        )
    }
}

impl View for ActionSheetContent {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Row chrome: press, hover and focus fills over a full-width band.
struct ActionSheetRowStyle;

impl ButtonStyle for ActionSheetRowStyle {
    fn make_body(&self, configuration: ButtonStyleConfiguration) -> AnyView {
        let theme = configuration.theme.clone();
        let fill = if configuration.is_pressed {
            theme.colors.state_layer_pressed
        } else if configuration.is_hovered || configuration.is_focused {
            theme.colors.state_layer_hover
        } else {
            Color::CLEAR
        };
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                configuration.label.clone(),
            )
            .padding_edges(12.0, 10.0)
            .frame(
                Some(crate::modifiers::layout::FrameDimension::Percent(100.0)),
                None,
                Some(0.0),
                Some(44.0),
                None,
                None,
            )
            .background_color(fill)
            .corner_radius(theme.radius.sm),
        )
    }
}

/// Node-local expanded state exposed to assistive technology on the trigger.
pub fn action_sheet_expanded_key() -> Rc<cove_core::scene::CompositionLocal<Option<bool>>> {
    ActionControlAccessibility::expanded()
}

/// Keeps the catalog reachable for callers that need the same close glyph.
pub fn action_sheet_close_icon() -> BundleImageResource {
    ui_icon_catalog::action::x()
}

/// Sized payload helper for callers that construct the drawer directly.
pub fn action_sheet_drawer_size(viewport: Size) -> (f32, f32) {
    let compact = viewport.width < 600.0;
    (
        if compact { viewport.width } else { 560.0 },
        viewport.height,
    )
}
