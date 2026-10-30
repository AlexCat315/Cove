//! Keyboard boundary for collection controls; child inputs keep first refusal.
use cove_core::scene::{
    Align, EventPhase, EventResult, FlexDirection, FocusChainHolder, InputHandlerRole,
    InputHandlerRoute, InputRoutingPriority, InteractionRegistryHolder, LayoutNode,
    Node, PointerCaptureHolder,
};
use cove_platform::KeyEvent;
use crate::{AnyView, PrimitiveView, View};
use std::rc::Rc;

pub struct SelectionKeyHost {
    is_enabled: bool,
    on_key: Rc<dyn Fn(&KeyEvent) -> bool>,
    content: AnyView,
}

impl SelectionKeyHost {
    pub fn new(
        content: impl View + 'static,
        is_enabled: bool,
        on_key: impl Fn(&KeyEvent) -> bool + 'static,
    ) -> Self {
        Self { is_enabled, on_key: Rc::new(on_key), content: AnyView::new(content) }
    }
}

impl PrimitiveView for SelectionKeyHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.set_is_focusable(true);
        node
    }

    fn update_node(&self, node: &Node) {
        node.set_is_focusable(self.is_enabled);
        node.set_allows_hit_testing(self.is_enabled);
        if !self.is_enabled {
            if let Some(captured) = PointerCaptureHolder::current_target()
                && FocusChainHolder::current().is_some_and(|chain| chain.contains(&captured, node))
            {
                PointerCaptureHolder::release_current();
            }
            if let Some(chain) = FocusChainHolder::current()
                && chain.focused().is_some_and(|focused| chain.contains(&focused, node))
            {
                chain.clear();
            }
        }

        if let Some(registry) = InteractionRegistryHolder::current() {
            let enabled = self.is_enabled;
            let on_key = self.on_key.clone();
            let route = InputHandlerRoute::new(
                InputHandlerRole::Shortcut,
                InputRoutingPriority::NORMAL,
                "selection-key-host",
            );
            registry.set_key(node, route, Rc::new(move |event, phase| {
                if !enabled || phase == EventPhase::Capture { return EventResult::Ignored; }
                if on_key(event) { EventResult::Handled } else { EventResult::Ignored }
            }));
        }
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
        layout.set_flex_grow(1.0);
        layout.set_flex_shrink(1.0);
        Some(layout)
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> { vec![self.content.clone()] }
}

impl View for SelectionKeyHost {
    fn kind(&self) -> crate::ViewKind<'_> { crate::ViewKind::Primitive(self) }
}
