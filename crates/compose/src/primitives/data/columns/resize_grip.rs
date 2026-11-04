//! Translation of `TableResizeGrip` in `Primitives/Data/Lists/Table.swift`.
//!
//! Column resize grip shared by `Table` and `DataTable`: drag, arrow keys and
//! the accessibility actions all write one constrained width through the
//! binding it is handed, so neither table needs a resize path of its own.
use std::cell::RefCell;
use std::rc::Rc;

use cove_core::Binding;
use cove_core::scene::{
    AccessibilityActions, AccessibilityRole, AccessibilitySemantics, AccessibilityState, Align,
    EventPhase, EventResult, InputHandlerRoute, InteractionRegistryHolder, LayoutNode, Node,
    PointerCaptureHolder, PointerPhase, Rect,
};
use cove_platform::{MouseButton, SystemCursor};

use crate::primitives::data::data_table::table_column::TableColumnLayout;
use crate::theme::theme_for_node;
use crate::{AnyView, PrimitiveView, View};

/// The width a grip edits, in points.
pub(crate) struct ResizeGrip {
    /// Names the column in the accessible label.
    pub title: String,
    pub width: Binding<f32>,
    pub layout: TableColumnLayout,
    pub is_enabled: bool,
}

impl ResizeGrip {
    pub(crate) fn new(
        title: impl Into<String>,
        width: Binding<f32>,
        layout: TableColumnLayout,
        is_enabled: bool,
    ) -> Self {
        Self {
            title: title.into(),
            width,
            layout,
            is_enabled,
        }
    }
}

impl PrimitiveView for ResizeGrip {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(self.is_enabled);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_width(Some(6.0));
        layout.set_align_self(Align::Stretch);
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        node.set_cursor(Some(SystemCursor::ResizeHorizontal));
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Slider,
            value: format!("{:.0}", self.width.get()),
            label: format!("Resize {} column", self.title),
            state: AccessibilityState {
                is_enabled: self.is_enabled,
                ..Default::default()
            },
            ..Default::default()
        }));

        let inc_width = self.width.clone();
        let inc_layout = self.layout;
        let dec_width = self.width.clone();
        let dec_layout = self.layout;
        let set_width = self.width.clone();
        let set_layout = self.layout;
        node.set_accessibility_actions(AccessibilityActions {
            increment: Some(Rc::new(move || {
                let next = inc_layout.constrained(inc_width.get() + 10.0);
                inc_width.set(next);
            })),
            decrement: Some(Rc::new(move || {
                let next = dec_layout.constrained(dec_width.get() - 10.0);
                dec_width.set(next);
            })),
            set_value: Some(Rc::new(move |value: &str| {
                if let Ok(parsed) = value.parse::<f32>()
                    && parsed.is_finite()
                {
                    let next = set_layout.constrained(parsed);
                    set_width.set(next);
                }
            })),
            ..Default::default()
        });

        // A hairline down the header edge: the grip is the only thing marking
        // where one column stops and the next starts.
        let theme = theme_for_node(node);
        let color = theme.colors.border.multiplied_alpha(node.opacity());
        let is_enabled = self.is_enabled;
        let height = node.frame().height;
        node.update_overlay_draw(
            (color.rgba8(), is_enabled, height.to_bits()),
            move |list, origin| {
                let _ = list.add_rect(Rect::new(origin.x + 2.5, origin.y, 1.0, height), color);
            },
        );

        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        if !self.is_enabled {
            registry.remove(node);
            return;
        }

        // The press origin and the width it started from: a motion event is
        // resolved against the press instead of accumulating deltas.
        let start = Rc::new(RefCell::new(None::<(f32, f32)>));
        {
            let down_start = start.clone();
            let down_width = self.width.clone();
            let down_node = node.downgrade();
            registry.set_pointer(
                node,
                InputHandlerRoute::control(),
                Rc::new(move |event, phase, event_phase| {
                    if event_phase == EventPhase::Capture || event.button != MouseButton::Left {
                        return EventResult::Ignored;
                    }
                    match phase {
                        PointerPhase::Down => {
                            *down_start.borrow_mut() = Some((event.x, down_width.get()));
                            if let Some(capture) = PointerCaptureHolder::current()
                                && let Some(owner) = Node::upgrade(&down_node)
                            {
                                capture.acquire(&owner, false);
                            }
                        }
                        PointerPhase::Up => {
                            *down_start.borrow_mut() = None;
                            if let Some(capture) = PointerCaptureHolder::current() {
                                capture.release();
                            }
                        }
                    }
                    EventResult::Handled
                }),
            );
        }
        {
            let motion_start = start.clone();
            let motion_width = self.width.clone();
            let motion_layout = self.layout;
            registry.set_motion(
                node,
                InputHandlerRoute::control(),
                Rc::new(move |event, _phase| {
                    let Some((started, initial)) = *motion_start.borrow() else {
                        return EventResult::Ignored;
                    };
                    let next = motion_layout.constrained(initial + event.x - started);
                    motion_width.set(next);
                    EventResult::Handled
                }),
            );
        }
        {
            let key_width = self.width.clone();
            let key_layout = self.layout;
            registry.set_key(
                node,
                InputHandlerRoute::control(),
                Rc::new(move |event, phase| {
                    if phase != EventPhase::Target {
                        return EventResult::Ignored;
                    }
                    let delta = match event.scancode {
                        crate::Scancode::ARROW_LEFT => -10.0,
                        crate::Scancode::ARROW_RIGHT => 10.0,
                        _ => return EventResult::Ignored,
                    };
                    key_width.set(key_layout.constrained(key_width.get() + delta));
                    EventResult::Handled
                }),
            );
        }
    }
}

impl View for ResizeGrip {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
    fn body(&self, _cx: &crate::Scope) -> AnyView {
        // A primitive draws itself; nothing to build.
        AnyView::new(crate::EmptyView)
    }
}
