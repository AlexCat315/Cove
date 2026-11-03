//! Translation of Primitives/Layout/Split/Retention.swift.
//!
//! Retained regions keep each pane at the same composition position across
//! compact and wide layouts, so switching presentation preserves editor state,
//! scrolling and focus instead of rebuilding the panes.
use std::cell::RefCell;
use std::rc::Rc;

use crate::{AnyView, PrimitiveView, View, impl_primitive_view};
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, Align, Display, FlexDirection, FocusChainHolder,
    LayoutNode, Node, PointerCaptureHolder,
};

use super::super::split_compact::SplitPane;

pub const CONTAINER_MARKER: &str = "CoveUI.split.container";
pub const PANE_MARKER: &str = "CoveUI.split.pane";

/// Focus visibility a pane wants when it becomes visible again.
#[derive(Default)]
pub struct SplitFocusTransfer {
    pub pending_visibility: RefCell<Option<bool>>,
}

impl SplitFocusTransfer {
    pub fn new() -> Rc<Self> {
        Rc::new(Self::default())
    }
}

/// One pane slot that stays mounted but hidden when its side is not shown.
pub struct SplitRetainedRegion {
    pub is_visible: bool,
    pub focus: Rc<SplitFocusTransfer>,
    pub pane: Option<SplitPane>,
    pub direction: FlexDirection,
    pub content: AnyView,
}

impl SplitRetainedRegion {
    pub fn new(
        is_visible: bool,
        focus: Rc<SplitFocusTransfer>,
        pane: Option<SplitPane>,
        direction: FlexDirection,
        content: impl View + 'static,
    ) -> Self {
        Self {
            is_visible,
            focus,
            pane,
            direction,
            content: AnyView::new(content),
        }
    }
}

fn contains(node: &Node, ancestor: &Node) -> bool {
    let mut current = Some(node.clone());
    while let Some(candidate) = current {
        if candidate == *ancestor {
            return true;
        }
        current = candidate.parent();
    }
    false
}

impl PrimitiveView for SplitRetainedRegion {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_align_items(Align::Stretch);
        layout.set_flex_direction(self.direction);
        layout.set_display(if self.is_visible {
            Display::Flex
        } else {
            Display::None
        });
    }

    fn update_node(&self, node: &Node) {
        node.set_attachment(PANE_MARKER, self.pane);
        // A pane clips its own content, so an overflowing child can never
        // answer a hit that belongs to the divider beside it.
        node.set_clips_to_bounds(true);
        node.set_is_interaction_enabled(self.is_visible);
        node.set_opacity(if self.is_visible { 1.0 } else { 0.0 });
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Group,
            is_hidden: !self.is_visible,
            ..Default::default()
        }));
        if let Some(layout) = node.layout_node() {
            layout.set_display(if self.is_visible {
                Display::Flex
            } else {
                Display::None
            });
        }
        if self.is_visible {
            return;
        }
        // A hidden pane must not keep focus or the pointer hostage.
        if let Some(chain) = FocusChainHolder::current()
            && let Some(focused) = chain.focused()
            && contains(&focused, node)
        {
            *self.focus.pending_visibility.borrow_mut() = Some(chain.is_focus_visible());
            chain.clear();
        }
        if let Some(capture) = PointerCaptureHolder::current()
            && capture
                .target()
                .is_some_and(|target| contains(&target, node))
        {
            capture.release();
        }
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(SplitRetainedRegion);

/// Hands focus back to the pane that just became visible.
pub struct SplitFocusHandoff {
    pub focus: Rc<SplitFocusTransfer>,
    pub is_compact: bool,
    pub selection: SplitPane,
}

impl SplitFocusHandoff {
    pub fn new(focus: Rc<SplitFocusTransfer>, is_compact: bool, selection: SplitPane) -> Self {
        Self {
            focus,
            is_compact,
            selection,
        }
    }

    /// The preferred Tab stop inside `root` for the pane about to be shown.
    pub fn locate_in(&self, root: &Node, preferred: bool) -> Option<Node> {
        self.locate(root, preferred)
    }

    fn locate(&self, candidate: &Node, preferred: bool) -> Option<Node> {
        if !candidate.accepts_subtree_input() {
            return None;
        }
        if preferred && self.is_compact {
            let semantics = candidate.accessibility();
            if semantics.as_ref().is_some_and(|semantics| {
                semantics.role == AccessibilityRole::Tab && semantics.state.is_selected
            }) {
                return Some(candidate.clone());
            }
        }
        if preferred
            && !self.is_compact
            && candidate
                .attachment::<Option<SplitPane>>(PANE_MARKER)
                .flatten()
                == Some(self.selection)
            && let Some(found) = self.locate(candidate, false)
        {
            return Some(found);
        }
        if !preferred && candidate.is_focusable() && candidate.is_tab_stop() {
            return Some(candidate.clone());
        }
        candidate
            .children()
            .into_iter()
            .find_map(|child| self.locate(&child, preferred))
    }
}

impl PrimitiveView for SplitFocusHandoff {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        let focus = self.focus.clone();
        let is_compact = self.is_compact;
        let selection = self.selection;
        let weak = node.downgrade();
        node.set_layout_did_update(Some(Rc::new(move |node: &Node| {
            let pending = *focus.pending_visibility.borrow();
            let Some(visible) = pending else { return };
            *focus.pending_visibility.borrow_mut() = None;
            let handoff = SplitFocusHandoff::new(focus.clone(), is_compact, selection);
            let target = handoff
                .locate(node, true)
                .or_else(|| handoff.locate(node, false));
            if let Some(target) = target {
                if let Some(chain) = FocusChainHolder::current() {
                    chain.focus(Some(&target), visible);
                }
                crate::primitives::layout::ScrollReveal::reveal(&target, false);
            }
            let _ = &weak;
        })));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        Vec::new()
    }
}
impl_primitive_view!(SplitFocusHandoff);
