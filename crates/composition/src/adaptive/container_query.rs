//! Translation of CoveUIComposeCore/Adaptive/ContainerQuery.swift.
//!
//! Reports the content-box size of its own subtree after every layout pass, so
//! content can adapt to the space actually available to it — a container query
//! rather than a window query. Measurement runs after composition, so the first
//! build receives zero and the next frame publishes the measured size.
use crate::Scope;
use crate::{AnyView, DynamicField, PrimitiveView, View, ViewKind};
use cove_core::Size;
use cove_core::State;
use cove_core::scene::{LayoutNode, Node};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const MEASUREMENT_KEY: &str = "__coveui_container_size_measurement";

/// Measurement state for one probe node. Lives in `Node` attachments so it
/// survives recomposition without the layout hook being reinstalled.
struct ContainerSizeMeasurement {
    state: RefCell<State<Size>>,
    last_reported: Cell<Size>,
}
impl ContainerSizeMeasurement {
    /// Frame changes below this magnitude are treated as the same size.
    const TOLERANCE: f32 = 0.5;

    fn new(state: State<Size>) -> Self {
        Self {
            state: RefCell::new(state),
            last_reported: Cell::new(Size::default()),
        }
    }

    fn update(&self, node: &Node) {
        let (padding, border) = match node.layout_node() {
            Some(layout) => (layout.resolved_padding(), layout.resolved_border()),
            None => Default::default(),
        };
        let frame = node.frame();
        let measured = Size {
            width: (frame.width - (padding.left + padding.right + border.left + border.right))
                .max(0.0),
            height: (frame.height - (padding.top + padding.bottom + border.top + border.bottom))
                .max(0.0),
        };
        if !measured.width.is_finite() || !measured.height.is_finite() {
            return;
        }
        let last = self.last_reported.get();
        if (measured.width - last.width).abs() < Self::TOLERANCE
            && (measured.height - last.height).abs() < Self::TOLERANCE
        {
            return;
        }
        self.last_reported.set(measured);
        self.state.borrow().set(measured);
    }
}

/// A view that rebuilds its content from the measured content-box size.
pub struct ContainerSizeReader<Content: View> {
    size: State<Size>,
    content: Rc<dyn Fn(Size) -> Content>,
}
impl<Content: View> ContainerSizeReader<Content> {
    pub fn new(content: impl Fn(Size) -> Content + 'static) -> Self {
        Self {
            size: State::new(Size::default()),
            content: Rc::new(content),
        }
    }
}
impl<Content: View + 'static> View for ContainerSizeReader<Content> {
    fn body(&self, _cx: &Scope) -> AnyView {
        let content = self.content.clone();
        AnyView::new(ContainerSizeProbe {
            size: self.size.clone(),
            content: Rc::new(move |size| AnyView::new(content(size))),
        })
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("size", &self.size)]
    }
}

/// Pass-through node that publishes its post-layout frame into the measurement
/// state. Reports after every layout pass; the one-time hook chain keeps
/// repeated recompositions from stacking obsolete callbacks.
struct ContainerSizeProbe {
    size: State<Size>,
    content: Rc<dyn Fn(Size) -> AnyView>,
}
impl PrimitiveView for ContainerSizeProbe {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, node: &Node) {
        if let Some(measurement) = node.attachment::<Rc<ContainerSizeMeasurement>>(MEASUREMENT_KEY)
        {
            *measurement.state.borrow_mut() = self.size.clone();
            return;
        }
        let measurement = Rc::new(ContainerSizeMeasurement::new(self.size.clone()));
        node.set_attachment(MEASUREMENT_KEY, measurement.clone());
        let previous = node.layout_did_update();
        let hook_measurement = measurement.clone();
        node.set_layout_did_update(Some(Rc::new(move |node| {
            if let Some(previous) = &previous {
                previous(node);
            }
            hook_measurement.update(node);
        })));
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_width_percent(100.0);
        layout.set_align_items(cove_core::scene::Align::Stretch);
        Some(layout)
    }
    fn update_layout(&self, _layout: &LayoutNode) {}
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![(self.content)(self.size.get())]
    }
}
impl View for ContainerSizeProbe {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}
