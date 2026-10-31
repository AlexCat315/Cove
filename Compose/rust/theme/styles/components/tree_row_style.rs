//! Translation of Theme/Styles/Components/TreeRowStyle.swift.
use std::any::TypeId;
use std::rc::Rc;

use cove_core::scene::CompositionLocal;

use crate::theme::Theme;
use crate::{AnyView, ModifiedContent, View, ViewExt};

use super::default_tree_row_style::DefaultTreeRowStyle;

pub struct TreeRowStyleConfiguration {
    pub content: AnyView,
    pub depth: usize,
    pub indentation: f32,
    pub disclosure_width: f32,
    pub has_children: bool,
    pub is_expanded: bool,
    pub is_search_hit: bool,
    pub is_selected: bool,
    pub is_hovered: bool,
    pub is_enabled: bool,
    pub theme: Theme,
}

/// Interaction snapshot used by built-in tree row styles to key implicit
/// transitions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TreeRowInteractionKey {
    pub is_search_hit: bool,
    pub is_selected: bool,
    pub is_hovered: bool,
    pub is_enabled: bool,
}

impl TreeRowStyleConfiguration {
    pub fn interaction_key(&self) -> TreeRowInteractionKey {
        TreeRowInteractionKey {
            is_search_hit: self.is_search_hit,
            is_selected: self.is_selected,
            is_hovered: self.is_hovered,
            is_enabled: self.is_enabled,
        }
    }
}

pub trait TreeRowStyle {
    fn make_body(&self, configuration: TreeRowStyleConfiguration) -> AnyView;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TreeRowStyleScope {
    /// Leave the disclosure, indentation and trailing controls outside the style.
    #[default]
    Content,
    /// Style the whole row while preserving the slots' own input handling.
    Row,
}

/// Type-erased tree row style, compared by concrete type.
pub struct AnyTreeRowStyle {
    make_body: Rc<dyn Fn(TreeRowStyleConfiguration) -> AnyView>,
    style_id: TypeId,
    is_default: bool,
    pub scope: TreeRowStyleScope,
}
impl AnyTreeRowStyle {
    pub fn new<S: TreeRowStyle + 'static>(style: S, scope: TreeRowStyleScope) -> Self {
        let is_default = TypeId::of::<S>() == TypeId::of::<DefaultTreeRowStyle>();
        Self {
            make_body: Rc::new(move |configuration| style.make_body(configuration)),
            style_id: TypeId::of::<S>(),
            is_default,
            scope,
        }
    }
    pub fn make_body(&self, configuration: TreeRowStyleConfiguration) -> AnyView {
        (self.make_body)(configuration)
    }
    /// A custom style watches its own hover; the default one is recomposed by
    /// the host only for selection and search changes.
    pub fn requires_hover_recompose(&self) -> bool {
        !self.is_default
    }
}
impl Clone for AnyTreeRowStyle {
    fn clone(&self) -> Self {
        Self {
            make_body: self.make_body.clone(),
            style_id: self.style_id,
            is_default: self.is_default,
            scope: self.scope,
        }
    }
}
impl PartialEq for AnyTreeRowStyle {
    fn eq(&self, other: &Self) -> bool {
        self.style_id == other.style_id && self.scope == other.scope
    }
}
impl Eq for AnyTreeRowStyle {}

pub struct TreeRowStyleEnvironment;
impl TreeRowStyleEnvironment {
    pub fn key() -> Rc<CompositionLocal<AnyTreeRowStyle>> {
        thread_local! {
            static KEY: Rc<CompositionLocal<AnyTreeRowStyle>> = Rc::new(CompositionLocal::new(|| {
                AnyTreeRowStyle::new(DefaultTreeRowStyle, TreeRowStyleScope::Content)
            }));
        }
        KEY.with(|key| key.clone())
    }
}

pub trait TreeRowStyleViewExt: View + Sized + 'static {
    fn tree_row_style<S: TreeRowStyle + 'static>(self, style: S) -> ModifiedContent {
        self.tree_row_style_scoped(style, TreeRowStyleScope::Content)
    }
    fn tree_row_style_scoped<S: TreeRowStyle + 'static>(
        self,
        style: S,
        scope: TreeRowStyleScope,
    ) -> ModifiedContent {
        self.composition_local(
            TreeRowStyleEnvironment::key(),
            AnyTreeRowStyle::new(style, scope),
        )
    }
}
impl<T: View + Sized + 'static> TreeRowStyleViewExt for T {}
