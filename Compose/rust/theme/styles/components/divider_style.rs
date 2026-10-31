//! Translation of Theme/Styles/Components/DividerStyle.swift.
use std::any::TypeId;
use std::rc::Rc;

use cove_core::scene::CompositionLocal;

use crate::theme::Theme;
use crate::{AnyView, ModifiedContent, View, ViewExt};

use super::default_divider_style::DefaultDividerStyle;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DividerOrientation {
    Horizontal,
    Vertical,
}

#[derive(Clone)]
pub struct DividerStyleConfiguration {
    pub orientation: DividerOrientation,
    pub thickness: f32,
    pub theme: Theme,
}

pub trait DividerStyle {
    fn make_body(&self, configuration: DividerStyleConfiguration) -> AnyView;
}

/// Type-erased divider style. `CompositionLocal` requires equality, so the
/// erased value compares by concrete style type (Swift's `AnyDividerStyle` has
/// no authored payload to compare).
pub struct AnyDividerStyle {
    make_body: Rc<dyn Fn(DividerStyleConfiguration) -> AnyView>,
    style_id: TypeId,
}
impl AnyDividerStyle {
    pub fn new<S: DividerStyle + 'static>(style: S) -> Self {
        Self {
            make_body: Rc::new(move |configuration| style.make_body(configuration)),
            style_id: TypeId::of::<S>(),
        }
    }
    pub fn make_body(&self, configuration: DividerStyleConfiguration) -> AnyView {
        (self.make_body)(configuration)
    }
}
impl Clone for AnyDividerStyle {
    fn clone(&self) -> Self {
        Self {
            make_body: self.make_body.clone(),
            style_id: self.style_id,
        }
    }
}
impl PartialEq for AnyDividerStyle {
    fn eq(&self, other: &Self) -> bool {
        self.style_id == other.style_id
    }
}
impl Eq for AnyDividerStyle {}

pub struct DividerStyleEnvironment;
impl DividerStyleEnvironment {
    pub fn key() -> Rc<CompositionLocal<AnyDividerStyle>> {
        thread_local! {
            static KEY: Rc<CompositionLocal<AnyDividerStyle>> =
                Rc::new(CompositionLocal::new(|| AnyDividerStyle::new(DefaultDividerStyle)));
        }
        KEY.with(|key| key.clone())
    }
}

pub trait DividerStyleViewExt: View + Sized + 'static {
    fn divider_style<S: DividerStyle + 'static>(self, style: S) -> ModifiedContent {
        self.composition_local(DividerStyleEnvironment::key(), AnyDividerStyle::new(style))
    }
}
impl<T: View + Sized + 'static> DividerStyleViewExt for T {}

pub fn default_divider_style() -> DefaultDividerStyle {
    DefaultDividerStyle
}
