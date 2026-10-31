//! Sprite drawing and scene overlay for the software pet.
use super::{PetRenderState, PetSpriteSheet, PetWorld};
use crate::primitives::media::Image;
use crate::{AnyView, DynamicField, Observed, PrimitiveView, View, impl_primitive_view};
use cove_core::scene::{LayoutNode, Node};
use cove_core::{Color, DrawList, Point, Rect};
use std::rc::Rc;

/// A retained view that invalidates only when the pet render state changes.
pub struct PetSpriteView {
    world: Rc<PetWorld>,
    observed_render: Observed<PetWorld, PetRenderState>,
}
impl PetSpriteView {
    pub fn new(world: Rc<PetWorld>) -> Self {
        let observed_render = Observed::new(world.clone(), PetWorld::render);
        Self {
            world,
            observed_render,
        }
    }
}
impl View for PetSpriteView {
    fn body(&self) -> AnyView {
        AnyView::new(PetSpriteCanvas::new(self.world.clone()))
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![self.observed_render.dynamic_field("render")]
    }
}

/// Zero-size, non-interactive canvas painted in window coordinates.
#[derive(Clone)]
pub struct PetSpriteCanvas {
    world: Rc<PetWorld>,
}
impl PetSpriteCanvas {
    pub fn new(world: Rc<PetWorld>) -> Self {
        Self { world }
    }

    fn paint(list: &mut DrawList, render: PetRenderState, sheet: &PetSpriteSheet) {
        if render.presence <= 0.01 {
            return;
        }
        let Some(path) = sheet.resolved_path(render.pose) else {
            return;
        };
        let source = Image::resolve_path(path, sheet.display_size, sheet.display_size);
        source.retain(list);

        let width = sheet.display_size * render.scale_x;
        let height = sheet.display_size * render.scale_y;
        let rect = Rect::new(
            render.position.x - width * 0.5,
            render.position.y - height,
            width,
            height,
        );
        let (min_u, max_u) = if render.facing < 0.0 {
            (1.0, 0.0)
        } else {
            (0.0, 1.0)
        };
        let tint = Color {
            a: render.presence,
            ..Color::WHITE
        };
        let _ = list.add_image_quad(
            rect,
            source.texture_id(),
            tint,
            Point::new(min_u, 0.0),
            Point::new(max_u, 1.0),
        );
    }
}
impl PrimitiveView for PetSpriteCanvas {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, node: &Node) {
        let render = self.world.render();
        let sheet = self.world.sprite_sheet().clone();
        node.update_draw(render, move |list, _origin| {
            Self::paint(list, render, &sheet)
        });
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_width(Some(0.0));
        layout.set_height(Some(0.0));
        Some(layout)
    }
    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_width(Some(0.0));
        layout.set_height(Some(0.0));
    }
}
impl_primitive_view!(PetSpriteCanvas);

/// Pet layer intended to be placed above the scene's ordinary content.
pub struct PetOverlay {
    world: Rc<PetWorld>,
}
impl PetOverlay {
    pub fn new(world: Rc<PetWorld>) -> Self {
        Self { world }
    }
}
impl View for PetOverlay {
    fn body(&self) -> AnyView {
        AnyView::new(PetSpriteView::new(self.world.clone()))
    }
}
