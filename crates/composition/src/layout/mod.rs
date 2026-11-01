//! Translation of CoveUIComposeCore/Layout/Layout.swift.
mod direction;
mod value;
pub use direction::*;
pub use value::*;

use crate::{AnyView, PrimitiveView, View};
use cove_core::scene::{Direction, LayoutNode, Node, Point, Rect, Size};
use std::marker::PhantomData;
use std::rc::Rc;

/// A proposal expresses the space a parent offers, in logical points. A `None`
/// axis asks for intrinsic size; zero asks for the smallest fitting size.
/// Fixed child dimensions and minimums may exceed the offered space.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ProposedViewSize {
    pub width: Option<f32>,
    pub height: Option<f32>,
}
impl ProposedViewSize {
    pub fn new(width: Option<f32>, height: Option<f32>) -> Self {
        let clamp = |value: Option<f32>| {
            value
                .filter(|value| value.is_finite())
                .map(|value| value.max(0.0))
        };
        Self {
            width: clamp(width),
            height: clamp(height),
        }
    }
    pub const UNSPECIFIED: Self = Self {
        width: None,
        height: None,
    };
    pub const ZERO: Self = Self {
        width: Some(0.0),
        height: Some(0.0),
    };
}

/// A short-lived value used during measurement and placement. It exposes no
/// nodes, storage, lifecycle or mutable view state.
#[derive(Clone)]
pub struct LayoutSubview {
    node: LayoutNode,
    /// The owning container's logical direction. A child may override its own
    /// content direction without changing the container's placement.
    pub layout_direction: Direction,
}
impl LayoutSubview {
    pub fn from_layout_node(node: &LayoutNode) -> Self {
        let layout_direction = node
            .parent()
            .map(|parent| parent.custom_layout_direction())
            .unwrap_or_else(|| node.custom_layout_direction());
        Self {
            node: node.clone(),
            layout_direction,
        }
    }
    /// The value the child declared with `.layout_value(...)`.
    pub fn value<K: LayoutValueKey + 'static>(&self) -> K::Value {
        self.node
            .attachment::<K::Value>(&LayoutValueStorage::key::<K>())
            .unwrap_or_else(K::default_value)
    }
    pub fn size_that_fits(&self, proposal: ProposedViewSize) -> Size {
        self.node
            .measure_for_custom_layout(proposal.width, proposal.height)
    }
    /// Hidden placements retain their views, excluding them from paint, input
    /// and accessibility. Layout algorithms still own their geometry and gaps.
    pub fn place(&self, at: Point, proposal: ProposedViewSize, is_visible: bool) {
        self.node
            .place_for_custom_layout(at, proposal.width, proposal.height, is_visible);
    }
}

/// Implement only the geometry of a container. Ordinary child views need no
/// adaptation. Measurement must be deterministic and must not mutate UI state.
pub trait Layout {
    fn size_that_fits(&self, proposal: ProposedViewSize, subviews: &[LayoutSubview]) -> Size;
    fn place_subviews(&self, bounds: Rect, proposal: ProposedViewSize, subviews: &[LayoutSubview]);
}

/// Wraps a layout algorithm and its content, mirroring `Layout(...) { ... }`.
pub struct LayoutContainer<Algorithm: Layout, Content: View> {
    algorithm: Rc<Algorithm>,
    content: AnyView,
    _content: PhantomData<fn() -> Content>,
}
impl<Algorithm: Layout + 'static, Content: View + 'static> LayoutContainer<Algorithm, Content> {
    pub fn new(algorithm: Algorithm, content: Content) -> Self {
        Self {
            algorithm: Rc::new(algorithm),
            content: AnyView::new(content),
            _content: PhantomData,
        }
    }
    pub fn with_erased_content(algorithm: Algorithm, content: AnyView) -> Self {
        Self {
            algorithm: Rc::new(algorithm),
            content,
            _content: PhantomData,
        }
    }
}
impl<Algorithm: Layout + 'static, Content: View + 'static> PrimitiveView
    for LayoutContainer<Algorithm, Content>
{
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, _node: &Node) {}
    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }
    fn update_layout(&self, layout: &LayoutNode) {
        let measure_algorithm = self.algorithm.clone();
        let place_algorithm = self.algorithm.clone();
        layout.set_custom_layout(
            Rc::new(move |width, height, children| {
                let subviews: Vec<LayoutSubview> = children
                    .iter()
                    .map(LayoutSubview::from_layout_node)
                    .collect();
                let result = measure_algorithm
                    .size_that_fits(ProposedViewSize::new(width, height), &subviews);
                Size {
                    width: if result.width.is_finite() {
                        result.width.max(0.0)
                    } else {
                        0.0
                    },
                    height: if result.height.is_finite() {
                        result.height.max(0.0)
                    } else {
                        0.0
                    },
                }
            }),
            Rc::new(move |bounds, children| {
                let subviews: Vec<LayoutSubview> = children
                    .iter()
                    .map(LayoutSubview::from_layout_node)
                    .collect();
                place_algorithm.place_subviews(
                    bounds,
                    ProposedViewSize::new(Some(bounds.width), Some(bounds.height)),
                    &subviews,
                );
            }),
        );
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl<Algorithm: Layout + 'static, Content: View + 'static> View
    for LayoutContainer<Algorithm, Content>
{
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}
