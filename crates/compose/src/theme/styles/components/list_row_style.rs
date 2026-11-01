//! Translation of Theme/Styles/Components/ListRowStyle.swift.
use std::any::TypeId;
use std::rc::Rc;

use cove_core::InputCapabilities;
use cove_core::scene::CompositionLocal;

use crate::theme::Theme;
use crate::{AnyView, ModifiedContent, View, ViewExt};

use super::default_list_row_style::DefaultListRowStyle;

pub struct ListRowStyleConfiguration {
    /// User-supplied row content.
    pub content: AnyView,
    pub is_selected: bool,
    pub is_hovered: bool,
    pub is_enabled: bool,
    /// The row box is fixed at this height, so a style lays its content out
    /// *inside* it instead of letting a taller child push the painted row past
    /// its own slot.
    pub row_height: f32,
    pub theme: Theme,
    pub input: InputCapabilities,
}

/// Interaction snapshot used by built-in list row styles to key implicit
/// transitions without requiring external animation scopes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ListRowInteractionKey {
    pub is_selected: bool,
    pub is_hovered: bool,
    pub is_enabled: bool,
}

impl ListRowStyleConfiguration {
    pub fn interaction_key(&self) -> ListRowInteractionKey {
        ListRowInteractionKey {
            is_selected: self.is_selected,
            is_hovered: self.is_hovered,
            is_enabled: self.is_enabled,
        }
    }
}

pub trait ListRowStyle {
    fn make_body(&self, configuration: ListRowStyleConfiguration) -> AnyView;
}

/// Type-erased list row style, compared by concrete type.
pub struct AnyListRowStyle {
    make_body: Rc<dyn Fn(ListRowStyleConfiguration) -> AnyView>,
    style_id: TypeId,
}
impl AnyListRowStyle {
    pub fn new<S: ListRowStyle + 'static>(style: S) -> Self {
        Self {
            make_body: Rc::new(move |configuration| style.make_body(configuration)),
            style_id: TypeId::of::<S>(),
        }
    }
    pub fn make_body(&self, configuration: ListRowStyleConfiguration) -> AnyView {
        (self.make_body)(configuration)
    }
}
impl Clone for AnyListRowStyle {
    fn clone(&self) -> Self {
        Self {
            make_body: self.make_body.clone(),
            style_id: self.style_id,
        }
    }
}
impl PartialEq for AnyListRowStyle {
    fn eq(&self, other: &Self) -> bool {
        self.style_id == other.style_id
    }
}
impl Eq for AnyListRowStyle {}

pub struct ListRowStyleEnvironment;
impl ListRowStyleEnvironment {
    pub fn key() -> Rc<CompositionLocal<AnyListRowStyle>> {
        thread_local! {
            static KEY: Rc<CompositionLocal<AnyListRowStyle>> =
                Rc::new(CompositionLocal::new(|| AnyListRowStyle::new(DefaultListRowStyle)));
        }
        KEY.with(|key| key.clone())
    }
}

pub trait ListRowStyleViewExt: View + Sized + 'static {
    fn list_row_style<S: ListRowStyle + 'static>(self, style: S) -> ModifiedContent {
        self.composition_local(ListRowStyleEnvironment::key(), AnyListRowStyle::new(style))
    }
}
impl<T: View + Sized + 'static> ListRowStyleViewExt for T {}
