//! Translation of Theme/Styles/Components/TextFieldStyle.swift.
use std::any::TypeId;
use std::rc::Rc;

use cove_core::scene::CompositionLocal;

use crate::theme::Theme;
use crate::{AnyView, ModifiedContent, View, ViewExt};

use super::default_text_field_style::DefaultTextFieldStyle;

/// Snapshot passed to a `TextFieldStyle` on every recompose. The `content` slot
/// is the type-erased visual produced by the field itself — styles wrap it with
/// chrome (background, border, focus ring) instead of owning the input pipeline.
pub struct TextFieldStyleConfiguration {
    pub content: AnyView,
    pub placeholder: String,
    pub is_focused: bool,
    pub is_editing: bool,
    pub is_error: bool,
    pub is_enabled: bool,
    pub theme: Theme,
    pub is_hovered: bool,
}

/// Interaction snapshot used by built-in text field styles to key implicit
/// transitions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextFieldInteractionKey {
    pub is_focused: bool,
    pub is_editing: bool,
    pub is_error: bool,
    pub is_enabled: bool,
}

impl TextFieldStyleConfiguration {
    pub fn interaction_key(&self) -> TextFieldInteractionKey {
        TextFieldInteractionKey {
            is_focused: self.is_focused,
            is_editing: self.is_editing,
            is_error: self.is_error,
            is_enabled: self.is_enabled,
        }
    }
}

pub trait TextFieldStyle {
    fn make_body(&self, configuration: TextFieldStyleConfiguration) -> AnyView;
}

/// Type-erased style ferried through the composition tree, compared by concrete
/// type.
pub struct AnyTextFieldStyle {
    make_body: Rc<dyn Fn(TextFieldStyleConfiguration) -> AnyView>,
    style_id: TypeId,
}
impl AnyTextFieldStyle {
    pub fn new<S: TextFieldStyle + 'static>(style: S) -> Self {
        Self {
            make_body: Rc::new(move |configuration| style.make_body(configuration)),
            style_id: TypeId::of::<S>(),
        }
    }
    pub fn make_body(&self, configuration: TextFieldStyleConfiguration) -> AnyView {
        (self.make_body)(configuration)
    }
}
impl Clone for AnyTextFieldStyle {
    fn clone(&self) -> Self {
        Self { make_body: self.make_body.clone(), style_id: self.style_id }
    }
}
impl PartialEq for AnyTextFieldStyle {
    fn eq(&self, other: &Self) -> bool {
        self.style_id == other.style_id
    }
}
impl Eq for AnyTextFieldStyle {}

pub struct TextFieldStyleEnvironment;
impl TextFieldStyleEnvironment {
    pub fn key() -> Rc<CompositionLocal<AnyTextFieldStyle>> {
        thread_local! {
            static KEY: Rc<CompositionLocal<AnyTextFieldStyle>> =
                Rc::new(CompositionLocal::new(|| AnyTextFieldStyle::new(DefaultTextFieldStyle)));
        }
        KEY.with(|key| key.clone())
    }
}

pub trait TextFieldStyleViewExt: View + Sized + 'static {
    fn text_field_style<S: TextFieldStyle + 'static>(self, style: S) -> ModifiedContent {
        self.composition_local(TextFieldStyleEnvironment::key(), AnyTextFieldStyle::new(style))
    }
}
impl<T: View + Sized + 'static> TextFieldStyleViewExt for T {}
