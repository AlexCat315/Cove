//! Translation of Theme/Styles/Buttons/ButtonStyle.swift.
use std::any::TypeId;
use std::rc::Rc;
use std::sync::Arc;

use cove_core::InputCapabilities;
use cove_core::scene::CompositionLocal;

use crate::theme::{ControlSize, Theme};
use crate::{AnyView, ModifiedContent, View, ViewExt};

use super::destructive_button_style::DestructiveButtonStyle;
use super::ghost_button_style::GhostButtonStyle;
use super::plain_button_style::PlainButtonStyle;
use super::primary_button_style::PrimaryButtonStyle;
use super::secondary_button_style::SecondaryButtonStyle;
use super::toggle_button_style::ToggleButtonStyle;

/// Semantic role of a button. Styles consult this to decide whether to swap in
/// destructive coloring, etc. The role is orthogonal to the concrete style:
/// any `ButtonStyle` may opt to render `Destructive` differently from `Normal`,
/// but is not required to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ButtonRole {
    Normal,
    Destructive,
    Cancel,
}

/// Snapshot of state passed to `ButtonStyle::make_body` on every recompose.
///
/// The label is type-erased (`AnyView`) to avoid leaking the caller's label
/// generic into every style's body. `theme` is captured at body-construction
/// time so style implementations stay pure (no node lookup required).
#[derive(Clone)]
pub struct ButtonStyleConfiguration {
    pub label: AnyView,
    pub role: ButtonRole,
    pub is_pressed: bool,
    pub is_hovered: bool,
    /// Focus worth drawing: true only while the button holds focus *and* the
    /// last focus change came from the keyboard. Pointer focus shares the same
    /// keyboard routing but never leaves a ring behind, so a style can paint
    /// `is_focused` unconditionally without special-casing the input device.
    pub is_focused: bool,
    pub is_enabled: bool,
    /// On/off state for toggle-shaped buttons (tool chips, segmented choices,
    /// tabs). A first-class style input: styles decide the selected look once,
    /// instead of every call site hand-mixing accent tokens around the label.
    pub is_selected: bool,
    pub theme: Theme,
    pub control_size: ControlSize,
    pub is_icon_only: bool,
    pub input: InputCapabilities,
}
impl ButtonStyleConfiguration {
    pub fn new(
        label: AnyView,
        role: ButtonRole,
        is_pressed: bool,
        is_hovered: bool,
        is_focused: bool,
        is_enabled: bool,
        theme: Theme,
    ) -> Self {
        Self {
            label,
            role,
            is_pressed,
            is_hovered,
            is_focused,
            is_enabled,
            is_selected: false,
            theme,
            control_size: ControlSize::Regular,
            is_icon_only: false,
            input: InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
        }
    }
    pub fn with_selected(mut self, value: bool) -> Self {
        self.is_selected = value;
        self
    }
    pub fn with_control_size(mut self, value: ControlSize) -> Self {
        self.control_size = value;
        self
    }
    pub fn with_icon_only(mut self, value: bool) -> Self {
        self.is_icon_only = value;
        self
    }
    pub fn with_input(mut self, value: InputCapabilities) -> Self {
        self.input = value;
        self
    }

    /// The interaction-state subset used by built-in styles to key implicit
    /// transitions. Custom styles may use this same key, or define their own.
    pub fn interaction_key(&self) -> ButtonInteractionKey {
        ButtonInteractionKey {
            is_pressed: self.is_pressed,
            is_hovered: self.is_hovered,
            is_focused: self.is_focused,
            is_enabled: self.is_enabled,
            is_selected: self.is_selected,
        }
    }
}

/// Equatable snapshot of the interaction-driven flags of a
/// `ButtonStyleConfiguration`. Built-in styles use this as the keying value for
/// their implicit transitions so hover / press / focus / enabled changes
/// animate without the call site wrapping state mutation in `withAnimation`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ButtonInteractionKey {
    pub is_pressed: bool,
    pub is_hovered: bool,
    pub is_focused: bool,
    pub is_enabled: bool,
    pub is_selected: bool,
}

/// Style protocol. Implementors describe a button's complete visual tree given
/// its configuration; the `Button` primitive itself only owns hit-testing and
/// state.
pub trait ButtonStyle {
    fn make_body(&self, configuration: ButtonStyleConfiguration) -> AnyView;

    /// Identity of the authored style value, mirroring Swift's `Hashable`
    /// conformance. Stateless styles keep the default (type-only identity);
    /// parameterised styles fold their stored values in.
    fn style_identity(&self) -> u64 {
        0
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct ButtonStyleIdentity {
    style_id: TypeId,
    value: Option<u64>,
}

/// Type-erased `ButtonStyle` ferried through the composition tree via
/// `ButtonStyleEnvironment`. Stores the body factory as a closure.
pub struct AnyButtonStyle {
    make_body: Arc<dyn Fn(ButtonStyleConfiguration) -> AnyView>,
    pub style_id: TypeId,
    pub requires_interaction_recompose: bool,
    identity: ButtonStyleIdentity,
}
impl AnyButtonStyle {
    pub fn new<S: ButtonStyle + 'static>(style: S) -> Self {
        let style_id = TypeId::of::<S>();
        let identity = ButtonStyleIdentity {
            style_id,
            value: Some(style.style_identity()),
        };
        let requires_interaction_recompose = !is_node_driven_interaction_style::<S>();
        let style = Rc::new(style);
        Self {
            make_body: Arc::new(move |configuration| style.make_body(configuration)),
            style_id,
            requires_interaction_recompose,
            identity,
        }
    }

    pub fn make_body(&self, configuration: ButtonStyleConfiguration) -> AnyView {
        (self.make_body)(configuration)
    }
}
impl Clone for AnyButtonStyle {
    fn clone(&self) -> Self {
        Self {
            make_body: self.make_body.clone(),
            style_id: self.style_id,
            requires_interaction_recompose: self.requires_interaction_recompose,
            identity: self.identity.clone(),
        }
    }
}
impl PartialEq for AnyButtonStyle {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
    }
}
impl Eq for AnyButtonStyle {}

/// Styles whose chrome repaints itself in place. Every other style must be
/// rebuilt on interaction changes so it can observe `is_pressed` / `is_hovered`.
fn is_node_driven_interaction_style<S: ButtonStyle + 'static>() -> bool {
    let id = TypeId::of::<S>();
    id == TypeId::of::<PlainButtonStyle>()
        || id == TypeId::of::<PrimaryButtonStyle>()
        || id == TypeId::of::<SecondaryButtonStyle>()
        || id == TypeId::of::<GhostButtonStyle>()
        || id == TypeId::of::<DestructiveButtonStyle>()
        || id == TypeId::of::<ToggleButtonStyle>()
}

/// CompositionLocal that carries the active `AnyButtonStyle`. The default is
/// `PrimaryButtonStyle`, so a bare `Button(...)` always renders.
pub struct ButtonStyleEnvironment;
impl ButtonStyleEnvironment {
    pub fn key() -> Rc<CompositionLocal<AnyButtonStyle>> {
        thread_local! {
            static KEY: Rc<CompositionLocal<AnyButtonStyle>> =
                Rc::new(CompositionLocal::new(|| AnyButtonStyle::new(PrimaryButtonStyle)));
        }
        KEY.with(|key| key.clone())
    }
}

pub trait ButtonStyleViewExt: View + Sized + 'static {
    /// Override the `ButtonStyle` used by every `Button` in this subtree.
    /// Nesting is supported; the nearest provider wins.
    fn button_style<S: ButtonStyle + 'static>(self, style: S) -> ModifiedContent {
        self.button_style_any(AnyButtonStyle::new(style))
    }

    /// Overload for an already-erased style, mirroring
    /// `buttonStyle(_ style: AnyButtonStyle)`.
    fn button_style_any(self, style: AnyButtonStyle) -> ModifiedContent {
        self.composition_local(ButtonStyleEnvironment::key(), style)
    }
}
impl<T: View + Sized + 'static> ButtonStyleViewExt for T {}

// MARK: - Convenience constructors
//
// Swift's `ButtonStyle.primary` and friends are static member shorthands; Rust
// has no protocol extension statics, so the same spellings become free
// functions returning the concrete style.

pub fn primary() -> PrimaryButtonStyle {
    PrimaryButtonStyle
}
pub fn secondary() -> SecondaryButtonStyle {
    SecondaryButtonStyle
}
pub fn ghost() -> GhostButtonStyle {
    GhostButtonStyle
}
pub fn destructive() -> DestructiveButtonStyle {
    DestructiveButtonStyle
}
pub fn toggle() -> ToggleButtonStyle {
    ToggleButtonStyle::new(28.0, 26.0)
}
pub fn plain() -> PlainButtonStyle {
    PlainButtonStyle
}
