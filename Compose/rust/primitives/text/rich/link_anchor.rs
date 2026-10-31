//! Translation of Primitives/Text/Rich/LinkAnchor.swift.
//!
//! The keyboard and accessibility face of one link in a paragraph. It sits over
//! the link's first line fragment, takes Tab focus, announces itself as a link
//! with its full text and destination, and opens on Return. Pointer input passes
//! through to the paragraph, which hit-tests every fragment.
use std::rc::Rc;

use crate::{PrimitiveView, View, impl_primitive_view};
use cove_core::scene::{
    AccessibilityActions, AccessibilityRole, AccessibilitySemantics, EventPhase, EventResult,
    InputHandlerRoute, InteractionRegistryHolder, LayoutNode, Node, TextInputAttachmentKey,
};

/// Surface-side slot remembering which link currently holds focus, so the
/// paragraph can paint its ring.
pub const FOCUSED_LINK: &str = "__richtext.focusedLink";

/// The focusable, announced face of one link.
pub struct RichTextLinkAnchor {
    pub destination: String,
    /// Full visible text of the link, used as the accessible label.
    pub label: String,
    pub index: usize,
    pub on_open_link: Option<Rc<dyn Fn(&str)>>,
    /// The paragraph surface node, notified so it can repaint the ring.
    pub surface: Option<Node>,
}

impl PrimitiveView for RichTextLinkAnchor {
    fn make_node(&self) -> Node {
        let node = Node::new();
        // Pointer input passes through: the paragraph hit-tests every fragment.
        node.set_is_hit_testable(false);
        node.set_is_focusable(true);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_width_percent(100.0);
        layout.set_height_percent(100.0);
    }

    fn update_node(&self, node: &Node) {
        let open = self.on_open_link.as_ref().map(|handler| {
            let handler = handler.clone();
            let destination = self.destination.clone();
            Rc::new(move || handler(&destination)) as Rc<dyn Fn()>
        });
        node.set_is_focusable(open.is_some());
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Link,
            label: self.label.clone(),
            help: self.destination.clone(),
            state: cove_core::scene::AccessibilityState {
                is_enabled: open.is_some(),
                ..Default::default()
            },
            ..Default::default()
        }));
        let activate = open.clone();
        node.set_accessibility_actions(AccessibilityActions {
            activate,
            ..Default::default()
        });

        let surface = self.surface.clone();
        let index = self.index;
        node.set_attachment(
            TextInputAttachmentKey::FOCUS_CHANGE_HANDLER,
            Rc::new(move |focused: bool| {
                let Some(surface) = surface.as_ref() else {
                    return;
                };
                let current = surface.attachment::<usize>(FOCUSED_LINK);
                if focused {
                    surface.set_attachment(FOCUSED_LINK, index);
                } else if current == Some(index) {
                    surface.remove_attachment(FOCUSED_LINK);
                }
                surface.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                    "richText.focus",
                ));
            }) as cove_core::scene::TextInputFocusChangeHandler,
        );

        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        let Some(open) = open else {
            registry.remove(node);
            return;
        };
        registry.set_key(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase| {
                if phase == EventPhase::Capture || event.is_repeat {
                    return EventResult::Ignored;
                }
                match event.scancode {
                    crate::Scancode::RETURN | crate::Scancode::KEYPAD_ENTER => {
                        open();
                        EventResult::Handled
                    }
                    _ => EventResult::Ignored,
                }
            }),
        );
    }
}
impl_primitive_view!(RichTextLinkAnchor);
