//! Stateful action buttons and their style-driven presentation.
use crate::Scope;
use crate::primitives::media::{Image, ImageContentMode, ImageRenderingMode, ImageSource};
use crate::primitives::text::Text;
pub use crate::theme::styles::buttons::ButtonRole;
use crate::theme::styles::buttons::{
    AnyButtonStyle, ButtonStyleConfiguration, ButtonStyleEnvironment,
};
use crate::theme::{ControlSizeEnvironment, IconMetrics, IconMetricsReader, theme_for_node};
use crate::{AnyView, BoxView, DynamicField, PrimitiveView, TupleView, View, impl_primitive_view};
use cove_core::scene::{
    AccessibilityActions, AccessibilityRole, AccessibilitySemantics, Align, CompositionLocal,
    EventPhase, EventResult, FlexDirection, FocusChainHolder, HoverPhase, InputHandlerRoute,
    InteractionRegistryHolder, Justify, LayoutNode, Node, PointerCaptureAttachmentKey,
    PointerCaptureHolder, PointerPhase, TextInputAttachmentKey, TextInputFocusChangeHandler,
    WindowRevealContext, Wrap,
};
use cove_core::{Color, State, TextureId};
use cove_imaging::BundleImageResource;
use cove_platform::{MouseButton, SystemCursor};
use std::rc::Rc;

const PRESSED: &str = "__button_pressed";
const HOVERED: &str = "__button_hovered";
const ACTIVE_PRESS: &str = "__button_active_press";
const HOST_MARKER: &str = "__button_host";
const REQUIRES_RECOMPOSE: &str = "__button_requires_interaction_recompose";

/// Optional action-control semantics supplied by wrappers such as `Link`.
pub struct ActionControlAccessibility;
impl ActionControlAccessibility {
    pub fn role() -> Rc<CompositionLocal<AccessibilityRole>> {
        thread_local! { static KEY: Rc<CompositionLocal<AccessibilityRole>> = Rc::new(CompositionLocal::constant(AccessibilityRole::Button)); }
        KEY.with(Rc::clone)
    }
    pub fn label() -> Rc<CompositionLocal<Option<String>>> {
        thread_local! { static KEY: Rc<CompositionLocal<Option<String>>> = Rc::new(CompositionLocal::constant(None)); }
        KEY.with(Rc::clone)
    }
    pub fn value() -> Rc<CompositionLocal<String>> {
        thread_local! { static KEY: Rc<CompositionLocal<String>> = Rc::new(CompositionLocal::constant(String::new())); }
        KEY.with(Rc::clone)
    }
    pub fn expanded() -> Rc<CompositionLocal<Option<bool>>> {
        thread_local! { static KEY: Rc<CompositionLocal<Option<bool>>> = Rc::new(CompositionLocal::constant(None)); }
        KEY.with(Rc::clone)
    }
}

/// Button label plus action. Interaction state is retained through State so
/// custom styles can rebuild from the same configuration as built-in styles.
pub struct Button {
    pub role: ButtonRole,
    pub is_enabled: bool,
    pub is_loading: bool,
    pub is_selected: bool,
    is_icon_only: bool,
    /// Rows that own their height opt out of the touch extent floor, which
    /// would otherwise inflate a list row past its scaled metric.
    ignores_touch_extent: bool,
    icon_size: Option<f32>,
    pub tooltip: Option<String>,
    /// The visible title, used as the accessible name when no explicit
    /// accessibility label is supplied by a wrapper such as `Link`.
    accessibility_text: Option<String>,
    label: AnyView,
    action: Rc<dyn Fn()>,
    pressed: State<bool>,
    hovered: State<bool>,
    focused: State<bool>,
}
impl Button {
    pub fn new<Label: View + 'static>(label: Label, action: impl Fn() + 'static) -> Self {
        Self::with_options(label, action, ButtonRole::Normal, true, false, false, None)
    }

    pub fn with_options<Label: View + 'static>(
        label: Label,
        action: impl Fn() + 'static,
        role: ButtonRole,
        is_enabled: bool,
        is_loading: bool,
        is_selected: bool,
        tooltip: Option<String>,
    ) -> Self {
        Self {
            role,
            is_enabled,
            is_loading,
            is_selected,
            is_icon_only: false,
            ignores_touch_extent: false,
            icon_size: None,
            tooltip,
            accessibility_text: None,
            label: AnyView::new(label),
            action: Rc::new(action),
            pressed: State::new(false),
            hovered: State::new(false),
            focused: State::new(false),
        }
    }

    /// Keep the authored height even on a touch surface.
    pub fn ignoring_touch_extent(mut self) -> Self {
        self.ignores_touch_extent = true;
        self
    }

    pub fn title(title: impl Into<String>, action: impl Fn() + 'static) -> Self {
        let title = title.into();
        let mut button = Self::new(Text::new(title.clone()).truncated(), action);
        button.accessibility_text = Some(title);
        button
    }

    pub fn localized_title(
        key: impl Into<crate::i18n::LocalizedStringKey>,
        action: impl Fn() + 'static,
    ) -> Self {
        Self::new(Text::localized(key), action)
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.is_enabled = enabled;
        self
    }
    pub fn loading(mut self, loading: bool) -> Self {
        self.is_loading = loading;
        self
    }
    pub fn selected(mut self, selected: bool) -> Self {
        self.is_selected = selected;
        self
    }
    pub fn role(mut self, role: ButtonRole) -> Self {
        self.role = role;
        self
    }
    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }
    pub fn with_tooltip(mut self, tooltip: Option<String>) -> Self {
        self.tooltip = tooltip;
        self
    }
}
impl View for Button {
    fn body(&self, _cx: &Scope) -> AnyView {
        let label = if self.is_loading {
            let label = self.label.clone();
            if self.is_icon_only {
                let icon_size = self.icon_size;
                AnyView::new(IconMetricsReader::new(move |node| {
                    let metrics = IconMetrics::new(node);
                    crate::primitives::feedback::Spinner::new()
                        .size(metrics.size(icon_size.unwrap_or(metrics.theme().icons.medium)))
                }))
            } else {
                let spinner = AnyView::new(IconMetricsReader::new(|node| {
                    let metrics = IconMetrics::new(node);
                    crate::primitives::feedback::Spinner::new()
                        .size(metrics.size(metrics.theme().icons.medium))
                }));
                AnyView::new(BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::Center,
                    Wrap::NoWrap,
                    6.0,
                    AnyView::new(TupleView::new(vec![spinner, label])),
                ))
            }
        } else {
            self.label.clone()
        };
        AnyView::new(ButtonHost {
            role: self.role,
            is_enabled: self.is_enabled && !self.is_loading,
            is_selected: self.is_selected,
            is_icon_only: self.is_icon_only,
            ignores_touch_extent: self.ignores_touch_extent,
            accessibility_text: self.accessibility_text.clone(),
            label,
            action: self.action.clone(),
            tooltip: self.tooltip.clone(),
            pressed: self.pressed.clone(),
            hovered: self.hovered.clone(),
            focused: self.focused.clone(),
        })
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("isPressed", &self.pressed),
            DynamicField::state("isHovered", &self.hovered),
            DynamicField::state("showsFocusRing", &self.focused),
        ]
    }
}

#[derive(Clone)]
pub enum ButtonIconSource {
    Texture(TextureId),
    File(String),
    Resource(BundleImageResource),
}
impl ButtonIconSource {
    pub fn texture(texture_id: TextureId) -> Self {
        Self::Texture(texture_id)
    }
    pub fn file(path: impl Into<String>) -> Self {
        Self::File(path.into())
    }
    pub fn resource(resource: BundleImageResource) -> Self {
        Self::Resource(resource)
    }
}

#[derive(Clone)]
pub struct ButtonIcon {
    pub source: ButtonIconSource,
    pub size: Option<f32>,
    pub tint: Option<Color>,
}
impl ButtonIcon {
    pub fn new(source: ButtonIconSource, size: Option<f32>, tint: Option<Color>) -> Self {
        Self {
            source,
            size: size.map(|value| {
                if value.is_finite() {
                    value.max(1.0)
                } else {
                    16.0
                }
            }),
            tint,
        }
    }
}
impl View for ButtonIcon {
    fn body(&self, _cx: &Scope) -> AnyView {
        let source = self.source.clone();
        let size = self.size;
        let tint = self.tint;
        AnyView::new(IconMetricsReader::new(move |node| {
            let metrics = IconMetrics::new(node);
            let size = metrics.size(size.unwrap_or(metrics.theme().icons.medium));
            let image = match &source {
                ButtonIconSource::Texture(texture_id) => Image::new(
                    ImageSource::Texture {
                        texture_id: *texture_id,
                        size: None,
                    },
                    cove_core::Size {
                        width: size,
                        height: size,
                    },
                )
                .content_mode(ImageContentMode::Fit)
                .rendering_mode(ImageRenderingMode::AlphaMask),
                ButtonIconSource::File(path) => Image::from_file(
                    path,
                    cove_core::Size {
                        width: size,
                        height: size,
                    },
                )
                .content_mode(ImageContentMode::Fit)
                .rendering_mode(ImageRenderingMode::AlphaMask),
                ButtonIconSource::Resource(resource) => {
                    let resolved = metrics.theme().icons.resolve(resource);
                    Image::from_resource(
                        &resolved,
                        cove_core::Size {
                            width: size,
                            height: size,
                        },
                    )
                    .content_mode(ImageContentMode::Fit)
                    .rendering_mode(ImageRenderingMode::AlphaMask)
                }
            };
            if let Some(tint) = tint {
                image.foreground(tint)
            } else {
                image
            }
        }))
    }
}

struct ButtonHost {
    role: ButtonRole,
    is_enabled: bool,
    is_selected: bool,
    is_icon_only: bool,
    ignores_touch_extent: bool,
    accessibility_text: Option<String>,
    label: AnyView,
    action: Rc<dyn Fn()>,
    tooltip: Option<String>,
    pressed: State<bool>,
    hovered: State<bool>,
    focused: State<bool>,
}
impl ButtonHost {
    fn write_interaction(
        node: &Node,
        key: &str,
        state: &State<bool>,
        value: bool,
        recompose: bool,
    ) {
        if recompose {
            if state.get() != value {
                state.set(value);
            }
        } else if node.attachment::<bool>(key) != Some(value) {
            node.set_attachment(key, value);
            node.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(key));
        }
        crate::theme::styles::buttons::update_builtin_button_chrome_descendants(node);
    }

    fn activate(node: &Node, action: &dyn Fn()) {
        WindowRevealContext::record_activation(node);
        action();
    }
}
impl PrimitiveView for ButtonHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node.set_is_focusable(self.is_enabled);
        node
    }

    fn update_node(&self, node: &Node) {
        crate::primitives::controls::adaptive::apply_touch_control_extent(
            node,
            self.is_enabled && !self.ignores_touch_extent,
        );
        let style: AnyButtonStyle = node.composition_value(&ButtonStyleEnvironment::key());
        let recompose = style.requires_interaction_recompose;
        node.set_attachment(HOST_MARKER, true);
        node.set_attachment(REQUIRES_RECOMPOSE, recompose);
        node.set_is_focusable(self.is_enabled);
        node.set_cursor(Some(if self.is_enabled {
            SystemCursor::Pointer
        } else {
            SystemCursor::NotAllowed
        }));

        let role = node.composition_value(&ActionControlAccessibility::role());
        let label = node.composition_value(&ActionControlAccessibility::label());
        let mut semantics = AccessibilitySemantics::new(role);
        semantics.state.is_enabled = self.is_enabled;
        semantics.state.is_selected = self.is_selected;
        semantics.state.is_expanded =
            node.composition_value(&ActionControlAccessibility::expanded());
        semantics.value = node.composition_value(&ActionControlAccessibility::value());
        semantics.help = self.tooltip.clone().unwrap_or_default();
        semantics.combines_children = true;
        if let Some(label) = label {
            semantics.label = label;
        } else if let Some(text) = &self.accessibility_text {
            // A titled button is announced by its title, matching a native
            // button whose accessible name is its visible text.
            semantics.label = text.clone();
        } else if self.is_icon_only {
            semantics.label = self.tooltip.clone().unwrap_or_default();
        }
        node.set_accessibility(Some(semantics));
        let mut actions = AccessibilityActions::default();
        if self.is_enabled {
            let weak_node = node.downgrade();
            let action = self.action.clone();
            actions.activate = Some(Rc::new(move || {
                if let Some(node) = Node::upgrade(&weak_node) {
                    Self::activate(&node, action.as_ref());
                }
            }));
        }
        node.set_accessibility_actions(actions);

        let focus_state = self.focused.clone();
        let focus_node = node.downgrade();
        let focus_change: TextInputFocusChangeHandler = Rc::new(move |focused| {
            let visible = focused
                && FocusChainHolder::current().is_some_and(|chain| chain.is_focus_visible());
            if recompose && focus_state.get() != visible {
                focus_state.set(visible);
            }
            if let Some(node) = Node::upgrade(&focus_node) {
                crate::theme::styles::buttons::update_builtin_button_chrome_descendants(&node);
            }
        });
        node.set_attachment(TextInputAttachmentKey::FOCUS_CHANGE_HANDLER, focus_change);

        if !self.is_enabled {
            node.remove_attachment(ACTIVE_PRESS);
            if let Some(capture) = PointerCaptureHolder::current()
                && capture.target().as_ref() == Some(node)
            {
                capture.release();
            }
            if let Some(registry) = InteractionRegistryHolder::current() {
                registry.remove(node);
            }
            Self::write_interaction(node, PRESSED, &self.pressed, false, recompose);
            Self::write_interaction(node, HOVERED, &self.hovered, false, recompose);
            return;
        }

        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        let supports_hover = crate::primitives::controls::adaptive::supports_control_hover(node);
        let hovered = self.hovered.clone();
        let hover_node = node.clone();
        registry.set_hover(
            node,
            Rc::new(move |phase| {
                let value = phase == HoverPhase::Enter && supports_hover;
                Self::write_interaction(&hover_node, HOVERED, &hovered, value, recompose);
            }),
        );

        let weak_node = node.downgrade();
        let lost_pressed = self.pressed.clone();
        let lost_node = node.downgrade();
        node.set_attachment(
            PointerCaptureAttachmentKey::DID_LOSE_CAPTURE,
            Rc::new(move || {
                if let Some(node) = Node::upgrade(&lost_node) {
                    node.remove_attachment(ACTIVE_PRESS);
                    Self::write_interaction(&node, PRESSED, &lost_pressed, false, recompose);
                }
            }) as Rc<dyn Fn()>,
        );

        let press_node = weak_node.clone();
        let press_state = self.pressed.clone();
        let press_action = self.action.clone();
        registry.set_pointer(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase, event_phase| {
                if event_phase == EventPhase::Capture || event.button != MouseButton::Left {
                    return EventResult::Ignored;
                }
                let Some(node) = Node::upgrade(&press_node) else {
                    return EventResult::Ignored;
                };
                match phase {
                    PointerPhase::Down => {
                        node.set_attachment(ACTIVE_PRESS, true);
                        if let Some(capture) = PointerCaptureHolder::current() {
                            capture.acquire(&node, true);
                        }
                        Self::write_interaction(&node, PRESSED, &press_state, true, recompose);
                        EventResult::Handled
                    }
                    PointerPhase::Up => {
                        let active = node.attachment::<bool>(ACTIVE_PRESS) == Some(true);
                        node.remove_attachment(ACTIVE_PRESS);
                        let inside = node
                            .absolute_frame()
                            .contains(cove_core::Point::new(event.x, event.y));
                        if let Some(capture) = PointerCaptureHolder::current()
                            && capture.target().as_ref() == Some(&node)
                        {
                            capture.release();
                        }
                        if !active {
                            return EventResult::Ignored;
                        }
                        Self::write_interaction(&node, PRESSED, &press_state, false, recompose);
                        if inside {
                            Self::activate(&node, press_action.as_ref());
                        }
                        EventResult::Handled
                    }
                }
            }),
        );

        let motion_node = node.downgrade();
        let motion_pressed = self.pressed.clone();
        let motion_hovered = self.hovered.clone();
        registry.set_motion(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase| {
                if phase == EventPhase::Capture {
                    return EventResult::Ignored;
                }
                let Some(node) = Node::upgrade(&motion_node) else {
                    return EventResult::Ignored;
                };
                if node.attachment::<bool>(ACTIVE_PRESS) != Some(true) {
                    return EventResult::Ignored;
                }
                let inside = node
                    .absolute_frame()
                    .contains(cove_core::Point::new(event.x, event.y));
                Self::write_interaction(
                    &node,
                    HOVERED,
                    &motion_hovered,
                    inside && supports_hover,
                    recompose,
                );
                Self::write_interaction(&node, PRESSED, &motion_pressed, inside, recompose);
                EventResult::Handled
            }),
        );

        // Swift activates on key down; a repeat does not activate again.
        let key_node = node.downgrade();
        let key_action = self.action.clone();
        registry.set_key(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase| {
                if phase != EventPhase::Target
                    || event.is_repeat
                    || !matches!(
                        event.scancode,
                        crate::Scancode::RETURN
                            | crate::Scancode::SPACE
                            | crate::Scancode::KEYPAD_ENTER
                    )
                {
                    return EventResult::Ignored;
                }
                let Some(node) = Node::upgrade(&key_node) else {
                    return EventResult::Ignored;
                };
                Self::activate(&node, key_action.as_ref());
                EventResult::Handled
            }),
        );
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(cove_core::scene::FlexDirection::Row);
        layout.set_align_items(cove_core::scene::Align::Center);
        layout.set_justify_content(cove_core::scene::Justify::Center);
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let style: AnyButtonStyle = node.composition_value(&ButtonStyleEnvironment::key());
        let mut configuration = ButtonStyleConfiguration::new(
            self.label.clone(),
            self.role,
            self.is_enabled
                && if style.requires_interaction_recompose {
                    self.pressed.get()
                } else {
                    node.attachment(PRESSED).unwrap_or(false)
                },
            self.is_enabled
                && if style.requires_interaction_recompose {
                    self.hovered.get()
                } else {
                    node.attachment(HOVERED).unwrap_or(false)
                },
            self.is_enabled && self.focused.get(),
            self.is_enabled,
            theme_for_node(node),
        );
        configuration.is_selected = self.is_selected;
        configuration.control_size = node.composition_value(&ControlSizeEnvironment::key());
        configuration.is_icon_only = self.is_icon_only;
        configuration.input = node
            .composition_value(&crate::adaptive::SurfaceEnvironment::key())
            .input;
        vec![style.make_body(configuration)]
    }
}
impl_primitive_view!(ButtonHost);

/// Icon-only action control convenience constructor.
impl Button {
    pub fn icon_default(source: ButtonIconSource, action: impl Fn() + 'static) -> Self {
        Self::icon_with_options(
            source,
            None,
            None,
            ButtonRole::Normal,
            true,
            false,
            false,
            None,
            action,
        )
    }

    pub fn icon(source: ButtonIconSource, size: f32, action: impl Fn() + 'static) -> Self {
        Self::icon_with_options(
            source,
            Some(size),
            None,
            ButtonRole::Normal,
            true,
            false,
            false,
            None,
            action,
        )
    }

    pub fn icon_with_options(
        source: ButtonIconSource,
        size: Option<f32>,
        tint: Option<Color>,
        role: ButtonRole,
        is_enabled: bool,
        is_loading: bool,
        is_selected: bool,
        tooltip: Option<String>,
        action: impl Fn() + 'static,
    ) -> Self {
        let mut button = Self::with_options(
            ButtonIcon::new(source, size, tint),
            action,
            role,
            is_enabled,
            is_loading,
            is_selected,
            tooltip,
        );
        button.is_icon_only = true;
        button.icon_size = size;
        button
    }
}
