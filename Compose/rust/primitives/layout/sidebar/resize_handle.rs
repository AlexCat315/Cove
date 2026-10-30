//! Translation of Primitives/Layout/Sidebar/ResizeHandle.swift.
//!
//! The resizing lane belongs to layout, so its touch target never covers a
//! destination or an action in the adjacent content.
use std::cell::RefCell;
use std::rc::Rc;

use cove_core::scene::{
    AccessibilityActions, AccessibilityRole, AccessibilitySemantics, Direction, Display,     EventPhase, EventResult, FocusChainHolder, HoverPhase, InputHandlerRoute,
    InteractionRegistryHolder, LayoutNode, Node, NodeResource, PointerCaptureHolder, PointerPhase,
    PointerCaptureAttachmentKey,
};
use cove_core::Binding;
use cove_platform::{MouseButton, SystemCursor};
use crate::modifiers::interaction::CursorExt;
use crate::theme::theme_for_node;
use crate::{impl_primitive_view, PrimitiveView, View};

const START: &str = "sidebar.resize.start";
const BASIS: &str = "sidebar.resize.basis";
const HOVER: &str = "sidebar.resize.hover";

/// Logical bounds the handle resizes within.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ResizeBasis {
    low: f32,
    high: f32,
    extent: f32,
    viewport: f32,
    rtl: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ResizeStart {
    width: f32,
    x: f32,
}

/// Drag, keyboard and accessibility resize lane for a sidebar edge.
pub struct SidebarResizeHandle {
    pub value: Option<Binding<f32>>,
    pub extent: f32,
    pub range: (f32, f32),
    pub viewport: f32,
    pub resolved_width: f32,
    pub label: String,
}

impl SidebarResizeHandle {
    pub fn new(range: (f32, f32), viewport: f32, resolved_width: f32) -> Self {
        Self {
            value: None,
            extent: 8.0,
            range,
            viewport,
            resolved_width,
            label: "Resize navigation".to_string(),
        }
    }

    pub fn value(mut self, value: Binding<f32>) -> Self {
        self.value = Some(value);
        self
    }

    pub fn extent(mut self, extent: f32) -> Self {
        self.extent = extent;
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// Active only when a caller owns the width and the lane has extent.
    pub fn is_active(&self) -> bool {
        self.value.is_some() && self.extent > 0.0
    }

    fn current(&self) -> f32 {
        let proposed = self
            .value
            .as_ref()
            .map(|binding| binding.get())
            .unwrap_or(self.range.0);
        proposed.clamp(self.range.0, self.range.1)
    }

    fn write(&self, node: &Node, proposed: f32) {
        if !node.accepts_subtree_input() || !proposed.is_finite() {
            return;
        }
        let next = proposed.clamp(self.range.0, self.range.1);
        if let Some(binding) = self.value.as_ref()
            && binding.get() != next
        {
            binding.set(next);
        }
    }
}

fn is_rtl(node: &Node) -> bool {
    let mut layout = node.layout_node();
    while let Some(current) = layout {
        let direction = current.direction();
        if direction != Direction::Inherit {
            return direction == Direction::Rtl;
        }
        layout = current.parent();
    }
    false
}

fn release(node: &Node) {
    node.remove_attachment(START);
    if PointerCaptureHolder::current()
        .and_then(|capture| capture.target())
        .is_some_and(|target| target == *node)
        && let Some(capture) = PointerCaptureHolder::current()
    {
        capture.release();
    }
}

impl PrimitiveView for SidebarResizeHandle {
    fn make_node(&self) -> Node {
        let node = Node::new();
        let lifetime: Rc<dyn NodeResource> = Rc::new(SidebarResizeLifetime::new());
        node.add_resource(lifetime);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_display(if self.is_active() { Display::Flex } else { Display::None });
        layout.set_width(Some(self.extent));
        layout.set_flex_shrink(0.0);
    }

    fn update_node(&self, node: &Node) {
        let basis = ResizeBasis {
            low: self.range.0,
            high: self.range.1,
            extent: self.extent,
            viewport: self.viewport,
            rtl: is_rtl(node),
        };
        if node.attachment::<(f32, f32, f32, f32, bool)>(BASIS)
            != Some((basis.low, basis.high, basis.extent, basis.viewport, basis.rtl))
        {
            release(node);
        }
        node.set_attachment(BASIS, (basis.low, basis.high, basis.extent, basis.viewport, basis.rtl));

        let active = self.is_active();
        node.set_is_hit_testable(active);
        node.set_is_focusable(active);
        node.set_cursor(Some(if active { SystemCursor::ResizeHorizontal } else { SystemCursor::Arrow }));
        node.set_accessibility_actions(AccessibilityActions::default());
        if !active {
            release(node);
            if let Some(chain) = FocusChainHolder::current()
                && chain.focused().is_some_and(|focused| focused == *node)
            {
                chain.clear();
            }
            node.set_accessibility(None);
            if let Some(registry) = InteractionRegistryHolder::current() {
                registry.remove(node);
            }
            return;
        }

        let current = self.current();
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Slider,
            label: self.label.clone(),
            value: format!("{} pt", current.round() as i64),
            help: "Use Left/Right to resize, Shift for larger steps, Home/End for the limits".to_string(),
            ..Default::default()
        }));

        let increment_value = self.value.clone();
        let increment_range = self.range;
        let increment_node = node.downgrade();
        let decrement_value = self.value.clone();
        let decrement_range = self.range;
        let decrement_node = node.downgrade();
        let set_value = self.value.clone();
        let set_range = self.range;
        let set_node = node.downgrade();
        node.set_accessibility_actions(AccessibilityActions {
            increment: Some(Rc::new(move || {
                if let (Some(owner), Some(binding)) = (Node::upgrade(&increment_node), increment_value.as_ref()) {
                    let _ = owner;
                    let next = (binding.get() + 16.0).clamp(increment_range.0, increment_range.1);
                    binding.set(next);
                }
            })),
            decrement: Some(Rc::new(move || {
                if let (Some(owner), Some(binding)) = (Node::upgrade(&decrement_node), decrement_value.as_ref()) {
                    let _ = owner;
                    let next = (binding.get() - 16.0).clamp(decrement_range.0, decrement_range.1);
                    binding.set(next);
                }
            })),
            set_value: Some(Rc::new(move |text: &str| {
                let cleaned = text.replace("pt", "");
                let trimmed = cleaned.trim();
                let Ok(next) = trimmed.parse::<f32>() else { return };
                if !next.is_finite() {
                    return;
                }
                if let (Some(binding), Some(owner)) = (set_value.as_ref(), Node::upgrade(&set_node)) {
                    let _ = owner;
                    binding.set(next.clamp(set_range.0, set_range.1));
                }
            })),
            ..Default::default()
        });

        let clear_node = node.downgrade();
        node.set_attachment(
            PointerCaptureAttachmentKey::DID_LOSE_CAPTURE,
            Rc::new(move || {
                if let Some(owner) = Node::upgrade(&clear_node) {
                    owner.remove_attachment(START);
                    owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set("sidebar.resize"));
                }
            }) as Rc<dyn Fn()>,
        );

        let weak = node.downgrade();
        let hover_node = node.downgrade();
        node.update_draw(
            (basis.low.to_bits(), basis.high.to_bits(), basis.extent.to_bits()),
            move |list, origin| {
                let Some(owner) = Node::upgrade(&weak) else { return };
                if !owner.is_layout_visible() {
                    return;
                }
                let chain = FocusChainHolder::current();
                let focused = chain
                    .as_ref()
                    .and_then(|chain| chain.focused())
                    .is_some_and(|current| current == owner)
                    && chain.as_ref().is_some_and(|chain| chain.is_focus_visible());
                let hot = owner.attachment::<bool>(HOVER) == Some(true)
                    || owner.attachment::<ResizeStart>(START).is_some();
                // At rest the edge is the change of surface; the line appears only
                // to grab.
                if !(focused || hot) {
                    return;
                }
                let theme = theme_for_node(&owner);
                let frame = owner.frame();
                let color = if focused { theme.colors.focus_ring } else { theme.colors.accent };
                let thickness = 2.0;
                let _ = list.add_rect(
                    cove_core::Rect::new(
                        origin.x + (frame.width - thickness) / 2.0,
                        origin.y,
                        thickness,
                        frame.height,
                    ),
                    color.multiplied_alpha(owner.opacity()),
                );
            },
        );
        let _ = hover_node;

        let Some(registry) = InteractionRegistryHolder::current() else { return };
        let hover_node = node.downgrade();
        registry.set_hover(node, Rc::new(move |phase| {
            let Some(owner) = Node::upgrade(&hover_node) else { return };
            let hovered = phase == HoverPhase::Enter
                && crate::primitives::controls::adaptive::supports_control_hover(&owner);
            owner.set_attachment(HOVER, hovered);
            owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set("sidebar.resize"));
        }));

        let current_width = self.current();
        let pointer_node = node.downgrade();
        registry.set_pointer(node, InputHandlerRoute::control(), Rc::new(move |event, phase, event_phase| {
            if event_phase != EventPhase::Target || event.button != MouseButton::Left {
                return EventResult::Ignored;
            }
            let Some(owner) = Node::upgrade(&pointer_node) else { return EventResult::Ignored };
            if !owner.accepts_subtree_input() {
                return EventResult::Ignored;
            }
            match phase {
                PointerPhase::Down => {
                    if let Some(chain) = FocusChainHolder::current() {
                        chain.focus(Some(&owner), false);
                    }
                    owner.set_attachment(START, ResizeStart { width: current_width, x: event.x });
                    if let Some(capture) = PointerCaptureHolder::current() {
                        capture.acquire(&owner, true);
                    }
                }
                PointerPhase::Up => release(&owner),
            }
            owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set("sidebar.resize"));
            EventResult::Handled
        }));

        let motion_value = self.value.clone();
        let motion_range = self.range;
        let motion_node = node.downgrade();
        registry.set_motion(node, InputHandlerRoute::control(), Rc::new(move |event, phase| {
            if phase != EventPhase::Target {
                return EventResult::Ignored;
            }
            let Some(owner) = Node::upgrade(&motion_node) else { return EventResult::Ignored };
            if !owner.accepts_subtree_input() {
                return EventResult::Ignored;
            }
            if !PointerCaptureHolder::current()
                .and_then(|capture| capture.target())
                .is_some_and(|target| target == owner)
            {
                return EventResult::Ignored;
            }
            let Some(start) = owner.attachment::<ResizeStart>(START) else { return EventResult::Ignored };
            let delta = (event.x - start.x) * if is_rtl(&owner) { -1.0 } else { 1.0 };
            let next = (start.width + delta).clamp(motion_range.0, motion_range.1);
            if let Some(binding) = motion_value.as_ref()
                && binding.get() != next
            {
                binding.set(next);
            }
            EventResult::Handled
        }));

        let key_value = self.value.clone();
        let key_range = self.range;
        let key_node = node.downgrade();
        registry.set_key(node, InputHandlerRoute::control(), Rc::new(move |event, phase| {
            if phase != EventPhase::Target {
                return EventResult::Ignored;
            }
            if event.modifiers.has_ctrl() || event.modifiers.has_gui() || event.modifiers.has_alt() {
                return EventResult::Ignored;
            }
            let Some(owner) = Node::upgrade(&key_node) else { return EventResult::Ignored };
            let step = if event.modifiers.has_shift() { 32.0 } else { 16.0 };
            let current = key_value.as_ref().map(|binding| binding.get()).unwrap_or(key_range.0);
            let rtl = is_rtl(&owner);
            let next = match event.scancode {
                crate::Scancode::ARROW_LEFT => current + if rtl { step } else { -step },
                crate::Scancode::ARROW_RIGHT => current + if rtl { -step } else { step },
                crate::Scancode::HOME => key_range.0,
                crate::Scancode::END => key_range.1,
                crate::Scancode::ESCAPE => {
                    let Some(start) = owner.attachment::<ResizeStart>(START) else {
                        return EventResult::Ignored;
                    };
                    let restored = start.width.clamp(key_range.0, key_range.1);
                    if let Some(binding) = key_value.as_ref() {
                        binding.set(restored);
                    }
                    release(&owner);
                    return EventResult::Handled;
                }
                _ => return EventResult::Ignored,
            };
            let next = next.clamp(key_range.0, key_range.1);
            if let Some(binding) = key_value.as_ref()
                && binding.get() != next
            {
                binding.set(next);
            }
            EventResult::Handled
        }));
    }
}
impl_primitive_view!(SidebarResizeHandle);

/// Releases capture and clears focus when the lane leaves the tree.
struct SidebarResizeLifetime {
    capture: RefCell<Option<Rc<cove_core::scene::PointerCapture>>>,
    chain: RefCell<Option<Rc<cove_core::scene::FocusChain>>>,
}

impl SidebarResizeLifetime {
    fn new() -> Self {
        Self { capture: RefCell::new(None), chain: RefCell::new(None) }
    }
}

impl NodeResource for SidebarResizeLifetime {
    fn mount(&self, _node: &Node) {
        *self.capture.borrow_mut() = PointerCaptureHolder::current();
        *self.chain.borrow_mut() = FocusChainHolder::current();
    }

    fn unmount(&self, node: &Node) {
        if let Some(capture) = self.capture.borrow().as_ref()
            && capture.target().is_some_and(|target| target == *node)
        {
            capture.release();
        }
        if let Some(chain) = self.chain.borrow().as_ref()
            && chain.focused().is_some_and(|focused| focused == *node)
        {
            chain.clear();
        }
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}
