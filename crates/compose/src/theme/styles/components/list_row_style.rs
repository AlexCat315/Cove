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
    /// The list holds keyboard focus: its selection is the accent band with
    /// light ink. Otherwise the selection is a neutral band and keeps the
    /// row's own ink, as a macOS list does when another view is focused.
    pub is_emphasized: bool,
    pub is_hovered: bool,
    pub is_enabled: bool,
    /// The row box is fixed at this height, so a style lays its content out
    /// *inside* it instead of letting a taller child push the painted row past
    /// its own slot.
    pub row_height: f32,
    /// Another row follows in the same group, so a hairline belongs under this
    /// one. The last row of a list or section ends the group instead.
    pub shows_separator: bool,
    /// Keeps the hairline inside these edges. `None` inherits the style's own
    /// inset; `Some(0.0)` draws it edge to edge.
    pub separator_inset: Option<f32>,
    pub theme: Theme,
    pub input: InputCapabilities,
}

/// What a row's content should know about the band it sits on. The default
/// row style publishes it, so `ListRow` and custom content ink themselves
/// instead of every caller re-deriving colours from the selection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ListRowContext {
    pub is_selected: bool,
    pub is_emphasized: bool,
    pub is_enabled: bool,
}

impl ListRowContext {
    pub fn key() -> Rc<CompositionLocal<ListRowContext>> {
        thread_local! {
            static KEY: Rc<CompositionLocal<ListRowContext>> =
                Rc::new(CompositionLocal::new(|| ListRowContext {
                    is_enabled: true,
                    ..ListRowContext::default()
                }));
        }
        KEY.with(|key| key.clone())
    }

    /// The row sits on the accent band, so text and glyphs take `on_accent`.
    pub fn is_on_accent(&self) -> bool {
        self.is_selected && self.is_emphasized
    }
}

/// Builds row content from the band it sits on, for custom rows that tint
/// glyphs or swap content when selected.
pub struct ListRowContextReader {
    content: Rc<dyn Fn(ListRowContext) -> AnyView>,
}

impl ListRowContextReader {
    pub fn new<V: View + 'static>(content: impl Fn(ListRowContext) -> V + 'static) -> Self {
        Self {
            content: Rc::new(move |context| AnyView::new(content(context))),
        }
    }
}

impl crate::PrimitiveView for ListRowContextReader {
    fn make_node(&self) -> cove_core::scene::Node {
        let node = cove_core::scene::Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }
    fn update_node(&self, _node: &cove_core::scene::Node) {}
    fn children(&self, node: &cove_core::scene::Node) -> Vec<AnyView> {
        vec![(self.content)(
            node.composition_value(&ListRowContext::key()),
        )]
    }
}
crate::impl_primitive_view!(ListRowContextReader);

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
