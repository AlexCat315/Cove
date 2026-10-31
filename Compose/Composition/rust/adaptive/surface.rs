//! Translation of CoveUIComposeCore/Adaptive/Surface.swift.
use crate::{AnyView, PrimitiveView, View, ViewKind};
use cove_core::scene::{LayoutNode, Node};
use cove_core::{CompositionLocal, Environment};
use std::rc::Rc;

pub struct SurfaceEnvironment;
impl SurfaceEnvironment {
    /// `CompositionLocal` carries a non-`Sync` default closure; the scene is
    /// single-threaded, so the declaration is kept per-thread like other
    /// per-window ambient keys.
    pub fn key() -> Rc<CompositionLocal<Environment>> {
        thread_local! {
            static KEY: Rc<CompositionLocal<Environment>> = Rc::new(CompositionLocal::constant(Environment::default()));
        }
        KEY.with(|key| key.clone())
    }
}

/// Reads per-surface capabilities without coupling content to a native host.
pub struct SurfaceReader<Content: View> {
    content: Rc<dyn Fn(&Environment) -> Content>,
}
impl<Content: View> SurfaceReader<Content> {
    pub fn new(content: impl Fn(&Environment) -> Content + 'static) -> Self {
        Self {
            content: Rc::new(content),
        }
    }
}
impl<Content: View + 'static> PrimitiveView for SurfaceReader<Content> {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, _node: &Node) {}
    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }
    fn children(&self, node: &Node) -> Vec<AnyView> {
        let environment = node.composition_value(&SurfaceEnvironment::key());
        vec![AnyView::new((self.content)(&environment))]
    }
}
impl<Content: View + 'static> View for SurfaceReader<Content> {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

pub trait SurfaceEnvironmentExt: crate::View + Sized + 'static {
    /// Usually provided by the application host; override for previews or an
    /// embedded surface.
    fn surface_environment(self, environment: Environment) -> crate::ModifiedContent;
}
impl<T: crate::View + Sized + 'static> SurfaceEnvironmentExt for T {
    fn surface_environment(self, environment: Environment) -> crate::ModifiedContent {
        use crate::ViewExt;
        self.composition_local(SurfaceEnvironment::key(), environment)
    }
}
