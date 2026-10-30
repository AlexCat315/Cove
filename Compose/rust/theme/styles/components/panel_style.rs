//! Translation of Theme/Styles/Components/PanelStyle.swift.
use std::any::TypeId;
use std::rc::Rc;

use cove_core::scene::CompositionLocal;

use crate::theme::Theme;
use crate::{AnyView, ModifiedContent, View, ViewExt};

use super::default_panel_style::DefaultPanelStyle;

pub struct PanelStyleConfiguration {
    pub title: String,
    pub accessory: AnyView,
    pub content: AnyView,
    pub is_active: bool,
    pub theme: Theme,
}

pub trait PanelStyle {
    fn make_body(&self, configuration: PanelStyleConfiguration) -> AnyView;
}

/// Type-erased panel style. `CompositionLocal` requires equality, so the erased
/// value compares by concrete style type.
pub struct AnyPanelStyle {
    make_body: Rc<dyn Fn(PanelStyleConfiguration) -> AnyView>,
    style_id: TypeId,
}
impl AnyPanelStyle {
    pub fn new<S: PanelStyle + 'static>(style: S) -> Self {
        Self {
            make_body: Rc::new(move |configuration| style.make_body(configuration)),
            style_id: TypeId::of::<S>(),
        }
    }
    pub fn make_body(&self, configuration: PanelStyleConfiguration) -> AnyView {
        (self.make_body)(configuration)
    }
}
impl Clone for AnyPanelStyle {
    fn clone(&self) -> Self {
        Self { make_body: self.make_body.clone(), style_id: self.style_id }
    }
}
impl PartialEq for AnyPanelStyle {
    fn eq(&self, other: &Self) -> bool {
        self.style_id == other.style_id
    }
}
impl Eq for AnyPanelStyle {}

pub struct PanelStyleEnvironment;
impl PanelStyleEnvironment {
    pub fn key() -> Rc<CompositionLocal<AnyPanelStyle>> {
        thread_local! {
            static KEY: Rc<CompositionLocal<AnyPanelStyle>> =
                Rc::new(CompositionLocal::new(|| AnyPanelStyle::new(DefaultPanelStyle)));
        }
        KEY.with(|key| key.clone())
    }
}

pub trait PanelStyleViewExt: View + Sized + 'static {
    fn panel_style<S: PanelStyle + 'static>(self, style: S) -> ModifiedContent {
        self.composition_local(PanelStyleEnvironment::key(), AnyPanelStyle::new(style))
    }
}
impl<T: View + Sized + 'static> PanelStyleViewExt for T {}
