//! Translation of Primitives/Overlays/Modal/DragHandle.swift.
//!
//! Full-width drag area with a centered visual grabber. The hit target is 44 pt
//! tall even when the visible bar is only 4 pt.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::modifiers::layout::LayoutFrameExt;
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::theme::SemanticColorRef;
use crate::{AnyView, BoxView, PrimitiveView, View, ViewExt, impl_primitive_view};
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, Align, EventResult, FlexDirection, InputHandlerRole,
    InputHandlerRoute, InputRoutingPriority, InteractionRegistryHolder, Justify, LayoutNode, Node,
    PointerCaptureHolder, PointerPhase, Wrap,
};
use cove_platform::MouseButton;

/// Drag state for one sheet handle.
#[derive(Clone, Copy, Debug, Default)]
struct DragState {
    start_y: f32,
    last_y: f32,
    last_time: f64,
    translation: f32,
    velocity: f32,
    active: bool,
}

/// Sheet grabber that reports drag translation and release velocity.
pub struct SheetDragHandle {
    pub on_translation: Rc<dyn Fn(f32)>,
    pub on_end: Rc<dyn Fn(f32, f32)>,
}

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64())
        .unwrap_or(0.0)
}

impl SheetDragHandle {
    pub fn new(
        on_translation: impl Fn(f32) + 'static,
        on_end: impl Fn(f32, f32) + 'static,
    ) -> Self {
        Self {
            on_translation: Rc::new(on_translation),
            on_end: Rc::new(on_end),
        }
    }
}

impl PrimitiveView for SheetDragHandle {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node.set_attachment(
            "bottom-sheet.drag",
            Rc::new(RefCell::new(DragState::default())),
        );
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_height(Some(44.0));
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Center);
        layout.set_justify_content(Justify::Center);
        Some(layout)
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_height(Some(44.0));
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Center);
        layout.set_justify_content(Justify::Center);
    }

    fn update_node(&self, node: &Node) {
        node.set_is_hit_testable(true);
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Button,
            label: "Resize sheet".to_string(),
            ..Default::default()
        }));
        let state = node
            .attachment::<Rc<RefCell<DragState>>>("bottom-sheet.drag")
            .unwrap_or_else(|| {
                let created = Rc::new(RefCell::new(DragState::default()));
                node.set_attachment("bottom-sheet.drag", created.clone());
                created
            });
        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        let route = InputHandlerRoute::new(
            InputHandlerRole::Drag,
            InputRoutingPriority::CHROME,
            "bottom-sheet.handle",
        );
        let pointer_state = state.clone();
        let on_translation = self.on_translation.clone();
        let on_end = self.on_end.clone();
        let pointer_node = node.downgrade();
        registry.set_pointer(
            node,
            route.clone(),
            Rc::new(move |event, phase, _event_phase| {
                if event.button != MouseButton::Left {
                    return EventResult::Ignored;
                }
                let Some(owner) = Node::upgrade(&pointer_node) else {
                    return EventResult::Ignored;
                };
                let mut drag = pointer_state.borrow_mut();
                match phase {
                    PointerPhase::Down => {
                        drag.start_y = event.y;
                        drag.last_y = event.y;
                        drag.last_time = now();
                        drag.translation = 0.0;
                        drag.velocity = 0.0;
                        drag.active = true;
                        drop(drag);
                        on_translation(0.0);
                        if let Some(capture) = PointerCaptureHolder::current() {
                            capture.acquire(&owner, true);
                        }
                    }
                    PointerPhase::Up => {
                        drag.active = false;
                        let (translation, velocity) = (drag.translation, drag.velocity);
                        drop(drag);
                        if let Some(capture) = PointerCaptureHolder::current() {
                            capture.release();
                        }
                        on_end(translation, velocity);
                    }
                }
                EventResult::Handled
            }),
        );
        let motion_state = state;
        let motion_translation = self.on_translation.clone();
        registry.set_motion(
            node,
            route,
            Rc::new(move |event, _phase| {
                let mut drag = motion_state.borrow_mut();
                if !drag.active {
                    return EventResult::Ignored;
                }
                let time = now();
                let delta = time - drag.last_time;
                if delta > 0.001 {
                    drag.velocity = (event.y - drag.last_y) / delta as f32;
                }
                drag.last_y = event.y;
                drag.last_time = time;
                drag.translation = event.y - drag.start_y;
                let translation = drag.translation;
                drop(drag);
                motion_translation(translation);
                EventResult::Handled
            }),
        );
        let _ = Cell::new(0.0_f32);
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::Center,
                Wrap::NoWrap,
                0.0,
                AnyView::new(
                    BoxView::new(
                        FlexDirection::Row,
                        Align::Center,
                        Justify::Center,
                        Wrap::NoWrap,
                        0.0,
                        AnyView::new(crate::EmptyView),
                    )
                    .frame_size(36.0, 4.0)
                    .semantic_background(SemanticColorRef::on_surface_muted())
                    .corner_radius(2.0),
                ),
            )
            .frame_size(f32::INFINITY, 44.0),
        )]
    }
}
impl_primitive_view!(SheetDragHandle);
