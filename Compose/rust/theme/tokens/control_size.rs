//! Translation of Theme/Tokens/ControlSize.swift.
use cove_core::scene::CompositionLocal;
use std::rc::Rc;
use crate::{AnyView, ProvideCompositionLocalModifier, View, ViewExt};

/// Shared control density.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ControlSize {
    Mini,
    Small,
    #[default]
    Regular,
    Large,
}
impl ControlSize {
    pub const ALL: [Self; 4] = [Self::Mini, Self::Small, Self::Regular, Self::Large];
    pub fn button_height(self) -> f32 {
        match self {
            Self::Mini => 22.0,
            Self::Small => 28.0,
            Self::Regular => 32.0,
            Self::Large => 40.0,
        }
    }
    pub fn horizontal_padding(self) -> f32 {
        match self {
            Self::Mini => 6.0,
            Self::Small => 9.0,
            Self::Regular => 12.0,
            Self::Large => 16.0,
        }
    }
    pub fn touch_height(self) -> f32 {
        match self {
            Self::Mini | Self::Small | Self::Regular => 44.0,
            Self::Large => 56.0,
        }
    }
    pub fn touch_padding(self) -> f32 {
        match self {
            Self::Mini => 12.0,
            Self::Small => 14.0,
            Self::Regular => 18.0,
            Self::Large => 24.0,
        }
    }
}

pub struct ControlSizeEnvironment;
impl ControlSizeEnvironment {
    pub fn key() -> Rc<CompositionLocal<ControlSize>> {
        thread_local! {
            static KEY: Rc<CompositionLocal<ControlSize>> = Rc::new(CompositionLocal::constant(ControlSize::Regular));
        }
        KEY.with(|key| key.clone())
    }
}

pub trait ControlSizeViewExt: View + Sized + 'static {
    fn control_size(self, size: ControlSize) -> crate::ModifiedContent {
        self.modifier(ProvideCompositionLocalModifier::new(ControlSizeEnvironment::key(), size))
    }
}
impl<T: View + Sized + 'static> ControlSizeViewExt for T {}

/// Erased helper for call sites that build their body dynamically.
pub fn control_size_environment<V: View + 'static>(view: V, size: ControlSize) -> AnyView {
    AnyView::new(view.modifier(ProvideCompositionLocalModifier::new(ControlSizeEnvironment::key(), size)))
}
