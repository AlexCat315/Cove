//! One selection surface that springs between the rows of a group.
//!
//! The host paints a single pill behind its content and moves it onto the
//! descendant marked as selected. Frames are measured relative to the host, so
//! scrolling an ancestor never makes the pill chase content; only a change of
//! the marked row travels.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use cove_core::scene::{
    ActiveAnimationContext, Align, CompositionLocal, FlexDirection, LayoutNode, LayoutUpdateHook,
    Node,
};
use cove_core::{Color, Rect, SurfaceShadow};

use crate::theme::{SemanticMotionRef, theme_for_node};
use crate::{AnyView, PrimitiveView, ViewModifier, impl_primitive_view};

const MARK: &str = "__sliding_selection.mark";
const FRAME_KEY: &str = "slidingSelection.frame";

/// What the travelling pill looks like.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlidingSelectionStyle {
    pub fill: Color,
    pub corner_radius: f32,
    pub shadow: SurfaceShadow,
}

/// True below a `SlidingSelection` host. A selectable row reads it to leave
/// its resting selection fill to the host and paint only hover and press.
pub fn sliding_selection_scope() -> Rc<CompositionLocal<bool>> {
    thread_local! {
        static KEY: Rc<CompositionLocal<bool>> = Rc::new(CompositionLocal::new(|| false));
    }
    KEY.with(Rc::clone)
}

/// Whether `node` sits below a `SlidingSelection` host.
pub fn is_in_sliding_selection(node: &Node) -> bool {
    node.composition_value(&sliding_selection_scope())
}

/// Marks the surface the nearest host covers while `0` is true.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlidingSelectionMark(pub bool);

impl ViewModifier for SlidingSelectionMark {
    fn apply_node(&self, node: &Node) {
        if node.attachment::<bool>(MARK) == Some(self.0) {
            return;
        }
        node.set_attachment(MARK, self.0);
        if !self.0 {
            return;
        }
        // Selection rarely changes layout, so the host is told directly.
        let mut cursor = node.parent();
        while let Some(candidate) = cursor {
            if let Some(state) = candidate.first_resource::<SlidingSelectionState>() {
                state.schedule(&candidate);
                return;
            }
            cursor = candidate.parent();
        }
    }
}

/// Paints one pill behind `content` and springs it to the marked descendant.
pub struct SlidingSelection {
    content: AnyView,
    style: Rc<dyn Fn(&crate::theme::Theme) -> SlidingSelectionStyle>,
}

impl SlidingSelection {
    pub fn new(
        style: impl Fn(&crate::theme::Theme) -> SlidingSelectionStyle + 'static,
        content: AnyView,
    ) -> Self {
        Self {
            content,
            style: Rc::new(style),
        }
    }
}

struct SlidingSelectionState {
    frame: Cell<Option<Rect>>,
    pending: Cell<bool>,
    hook_installed: Cell<bool>,
    draw_installed: Cell<bool>,
    style: RefCell<Option<Rc<dyn Fn(&crate::theme::Theme) -> SlidingSelectionStyle>>>,
}

impl SlidingSelectionState {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            frame: Cell::new(None),
            pending: Cell::new(false),
            hook_installed: Cell::new(false),
            draw_installed: Cell::new(false),
            style: RefCell::new(None),
        })
    }

    /// Defers the measurement to the next layout pass, when the marked row
    /// has its final frame.
    fn schedule(&self, host: &Node) {
        self.pending.set(true);
        host.mark_dirty(cove_core::scene::InvalidationSource::style_set(FRAME_KEY));
    }

    fn update(self: &Rc<Self>, host: &Node) {
        self.pending.set(false);
        let target = find_marked(host).map(|marked| {
            let origin = host.absolute_frame();
            let frame = marked.absolute_frame();
            Rect::new(
                frame.x - origin.x,
                frame.y - origin.y,
                frame.width,
                frame.height,
            )
        });
        let Some(target) = target else {
            host.cancel_animation(FRAME_KEY);
            if self.frame.replace(None).is_some() {
                host.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(FRAME_KEY));
            }
            return;
        };
        let current = self.frame.get();
        if current == Some(target) {
            return;
        }
        let weak = Rc::downgrade(self);
        let owner = host.downgrade();
        let apply = move |rect: Rect| {
            if let (Some(state), Some(host)) = (weak.upgrade(), Node::upgrade(&owner)) {
                state.frame.set(Some(rect));
                host.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(FRAME_KEY));
            }
        };
        // A first selection appears in place; a resize that keeps the row
        // where it was follows the layout instead of animating.
        let travels = current.is_some_and(|from| from.y != target.y || from.x != target.x);
        let Some(from) = current.filter(|_| travels) else {
            host.cancel_animation(FRAME_KEY);
            apply(target);
            return;
        };
        let motion = SemanticMotionRef::snappy().resolve(&theme_for_node(host));
        ActiveAnimationContext::with(Some(motion), None, None, || {
            host.animatable_set_with(FRAME_KEY, from, target, apply);
        });
    }
}

impl cove_core::scene::NodeResource for SlidingSelectionState {
    fn mount(&self, _node: &Node) {}

    fn unmount(&self, node: &Node) {
        node.cancel_animation(FRAME_KEY);
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}

fn find_marked(node: &Node) -> Option<Node> {
    for child in node.children() {
        if child.attachment::<bool>(MARK) == Some(true) && child.frame().width > 0.0 {
            return Some(child);
        }
        if let Some(found) = find_marked(&child) {
            return Some(found);
        }
    }
    None
}

impl PrimitiveView for SlidingSelection {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        let state: Rc<dyn cove_core::scene::NodeResource> = SlidingSelectionState::new();
        node.add_resource(state);
        node.set_composition_value(&sliding_selection_scope(), true);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        // Layout-neutral: the host takes the space its content would have, so
        // content that measures its container sees the same width.
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
        layout.set_flex_grow(1.0);
        layout.set_flex_shrink(1.0);
        layout.set_min_width(Some(0.0));
    }

    fn update_node(&self, node: &Node) {
        install(node, self.style.clone());
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(SlidingSelection);

/// Makes `node` itself the sliding selection host, for a primitive that already
/// owns the node its rows sit in and cannot take a wrapper without changing
/// its layout. Call it from `update_node`, before the rows compose.
pub fn install_sliding_selection(
    node: &Node,
    style: impl Fn(&crate::theme::Theme) -> SlidingSelectionStyle + 'static,
) {
    install(node, Rc::new(style));
}

fn install(node: &Node, style: Rc<dyn Fn(&crate::theme::Theme) -> SlidingSelectionStyle>) {
    if node.first_resource::<SlidingSelectionState>().is_none() {
        let state: Rc<dyn cove_core::scene::NodeResource> = SlidingSelectionState::new();
        node.add_resource(state);
        node.set_composition_value(&sliding_selection_scope(), true);
    }
    let Some(state) = node.first_resource::<SlidingSelectionState>() else {
        return;
    };
    *state.style.borrow_mut() = Some(style);
    if !state.hook_installed.replace(true) {
        let previous = node.layout_did_update();
        let weak = Rc::downgrade(&state);
        node.set_layout_did_update(Some(Rc::new(move |owner: &Node| {
            if let Some(previous) = &previous {
                previous(owner);
            }
            if let Some(state) = weak.upgrade() {
                state.update(owner);
            }
        }) as LayoutUpdateHook));
    }
    // The painter is installed once; it reads the live frame and style, so a
    // recomposition never replaces it and invalidates the host's layer.
    if !state.draw_installed.replace(true) {
        let weak = Rc::downgrade(&state);
        let owner = node.downgrade();
        node.set_draw(Some(Rc::new(move |list, origin| {
            let (Some(state), Some(host)) = (weak.upgrade(), Node::upgrade(&owner)) else {
                return;
            };
            let Some(frame) = state.frame.get() else {
                return;
            };
            let Some(style) = state.style.borrow().clone() else {
                return;
            };
            let style = style(&theme_for_node(&host));
            let rect = Rect::new(
                origin.x + frame.x,
                origin.y + frame.y,
                frame.width,
                frame.height,
            );
            let radius = style.corner_radius.min(rect.height * 0.5);
            list.add_shadow_layer(style.shadow, rect, radius, 1.0, None, Some(rect));
            let _ = list.add_rounded_rect(rect, radius, style.fill);
        })));
    }
    if state.pending.get() {
        node.mark_dirty(cove_core::scene::InvalidationSource::style_set(FRAME_KEY));
    }
}
