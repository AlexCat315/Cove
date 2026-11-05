//! Keyboard boundary for collection controls; child inputs keep first refusal.
use crate::{AnyView, PrimitiveView, View};
use cove_core::scene::{
    Align, EventPhase, EventResult, FlexDirection, FocusChainHolder, InputHandlerRole,
    InputHandlerRoute, InputRoutingPriority, InteractionRegistryHolder, LayoutNode, Node,
    PointerCaptureHolder,
};
use cove_platform::KeyEvent;
use std::rc::Rc;

/// Set while a node inside this host holds the keyboard focus, so the host can
/// reclaim it when the focused row leaves the input graph.
const OWNED_FOCUS: &str = "__selection_key_host_owned_focus";

pub(crate) struct SelectionKeyHost {
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
        Self {
            is_enabled,
            on_key: Rc::new(on_key),
            content: AnyView::new(content),
        }
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
                && chain
                    .focused()
                    .is_some_and(|focused| chain.contains(&focused, node))
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
            registry.set_key(
                node,
                route,
                Rc::new(move |event, phase| {
                    if !enabled || phase == EventPhase::Capture {
                        return EventResult::Ignored;
                    }
                    if on_key(event) {
                        EventResult::Handled
                    } else {
                        EventResult::Ignored
                    }
                }),
            );
        }

        // A row that collapses, filters away or is removed takes its focus with
        // it, and the collection stops answering the keyboard. Record that this
        // host held the focus while the row is still mounted, then take it back
        // for the subtree once the pass that dropped the row has run.
        if let Some(chain) = FocusChainHolder::current() {
            let owned = chain.focused().is_some_and(|focused| {
                std::ptr::eq(&focused, node) || chain.contains(&focused, node)
            });
            node.set_attachment(OWNED_FOCUS, owned);
            if owned {
                let weak = node.downgrade();
                node.set_layout_did_update(Some(Rc::new(move |owner: &Node| {
                    let Some(host) = Node::upgrade(&weak) else {
                        return;
                    };
                    let Some(chain) = FocusChainHolder::current() else {
                        return;
                    };
                    if chain.focused().is_some()
                        || host.attachment::<bool>(OWNED_FOCUS) != Some(true)
                    {
                        return;
                    }
                    // Recovering a lost row keeps focus where it was: an
                    // automatic collapse must not paint a focus ring the person
                    // never asked for.
                    chain.focus(Some(owner), chain.is_focus_visible());
                })
                    as cove_core::scene::LayoutUpdateHook));
            }
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

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}

impl View for SelectionKeyHost {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
}
