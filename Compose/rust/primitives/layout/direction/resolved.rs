//! Translation of Compose/Sources/CoveUICompose/Primitives/Layout/Direction/Resolved.swift.
use std::rc::Rc;

use crate::{AnyView, PrimitiveView, View, ViewKind};
use cove_core::State;
use cove_core::scene::{Direction, LayoutNode, Node};

/// Resolves the committed layout direction during layout write-back and
/// rebuilds its content when the direction changes.
///
/// Reading the direction during composition would capture the previous
/// frame's value, so the reader installs a layout-did-update hook and walks
/// the parent chain after direction modifiers have committed.
pub struct ResolvedLayoutDirection<Content: View + 'static> {
    content: Rc<dyn Fn(Direction) -> Content>,
    direction: State<Direction>,
}
impl<Content: View + 'static> ResolvedLayoutDirection<Content> {
    pub fn new(content: impl Fn(Direction) -> Content + 'static) -> Self {
        Self {
            content: Rc::new(content),
            direction: State::new(Direction::Ltr),
        }
    }
}
impl<Content: View + 'static> View for ResolvedLayoutDirection<Content> {
    fn body(&self) -> AnyView {
        let direction = self.direction.clone();
        AnyView::new(LayoutDirectionObserver::new(
            AnyView::new((self.content)(self.direction.get())),
            Rc::new(move |value| {
                if direction.get() != value {
                    direction.set(value);
                }
            }),
        ))
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Reads the nearest explicit direction from the node ancestry after layout.
pub struct LayoutDirectionObserver {
    content: AnyView,
    on_change: Rc<dyn Fn(Direction)>,
}
impl LayoutDirectionObserver {
    pub fn new(content: AnyView, on_change: Rc<dyn Fn(Direction)>) -> Self {
        Self { content, on_change }
    }
}
impl PrimitiveView for LayoutDirectionObserver {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, node: &Node) {
        let on_change = self.on_change.clone();
        node.set_layout_did_update(Some(Rc::new(move |node: &Node| {
            let mut parent = node.parent();
            let mut direction = Direction::Ltr;
            while let Some(current) = parent {
                if let Some(layout) = current.layout_node() {
                    let value = layout.direction();
                    if value != Direction::Inherit {
                        direction = value;
                        break;
                    }
                }
                parent = current.parent();
            }
            on_change(direction);
        })));
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_align_items(cove_core::scene::Align::Stretch);
        Some(layout)
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl View for LayoutDirectionObserver {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}
