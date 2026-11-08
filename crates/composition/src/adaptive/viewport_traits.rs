//! Translation of CoveUIComposeCore/Adaptive/ViewportTraits.swift.
use super::SurfaceEnvironment;
use crate::{AnyView, PrimitiveView, View, ViewKind};
use cove_core::scene::{LayoutNode, Node};
use cove_core::{
    DesignLanguage, DeviceClass, Environment, HeightClass, Orientation, Platform, Posture,
    WidthClass,
};
use std::rc::Rc;

macro_rules! viewport_reader {
    ($reader:ident, $value:ty, $select:expr, $doc:literal) => {
        #[doc = $doc]
        pub struct $reader<Content: View> {
            content: Rc<dyn Fn($value) -> Content>,
        }
        impl<Content: View> $reader<Content> {
            pub fn new(content: impl Fn($value) -> Content + 'static) -> Self {
                Self {
                    content: Rc::new(content),
                }
            }
        }
        impl<Content: View + 'static> PrimitiveView for $reader<Content> {
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
                let environment: Environment = node.composition_value(&SurfaceEnvironment::key());
                let select: fn(&Environment) -> $value = $select;
                vec![AnyView::new((self.content)(select(&environment)))]
            }
        }
        impl<Content: View + 'static> View for $reader<Content> {
            fn kind(&self) -> ViewKind<'_> {
                ViewKind::Primitive(self)
            }
            fn tag(&self) -> &'static str {
                std::any::type_name::<Self>()
            }
        }
    };
}

viewport_reader!(
    HeightClassReader,
    HeightClass,
    |environment: &Environment| environment.viewport.height_class(),
    "Reads only the surface height class into application content."
);
viewport_reader!(
    WidthClassReader,
    WidthClass,
    |environment: &Environment| environment.viewport.width_class(),
    "Reads only the surface width class into application content."
);
viewport_reader!(
    OrientationReader,
    Orientation,
    |environment: &Environment| environment.viewport.orientation(),
    "Reads whether the surface is portrait or landscape. Square counts as landscape."
);
viewport_reader!(
    PostureReader,
    Posture,
    |environment: &Environment| environment.posture,
    "Reads only the fold posture into application content."
);
viewport_reader!(
    DeviceClassReader,
    DeviceClass,
    |environment: &Environment| environment.device_class,
    "Reads only the hardware class into application content."
);
viewport_reader!(
    PlatformReader,
    Platform,
    |environment: &Environment| environment.platform,
    "Reads only the platform whose conventions apply."
);
viewport_reader!(
    DesignLanguageReader,
    DesignLanguage,
    |environment: &Environment| environment.design_language,
    "Reads only the design language components resolve their styles from."
);
