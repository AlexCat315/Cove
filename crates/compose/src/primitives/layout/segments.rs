//! Layout across the window segments a fold or hinge creates.
//!
//! Both views measure their own window frame after layout and intersect it with
//! the surface's segments, so they work full-window and when embedded. The
//! first build sees no frame and shows the single-segment content; the next
//! frame publishes the measured frame.
use std::cell::Cell;
use std::rc::Rc;

use cove_composition::Scope;
use cove_core::scene::{Align, FlexDirection, Justify, LayoutNode, Node, Wrap};
use cove_core::{Environment, Posture, Rect, State, display_features};

use crate::adaptive::SurfaceReader;
use crate::modifiers::layout::{FlexExt, FrameDimension, LayoutFrameExt};
use crate::{AnyView, BoxView, DynamicField, PrimitiveView, TupleView, View, ViewKind};

const FRAME_KEY: &str = "__cove_segmented_frame";
const TOLERANCE: f32 = 0.5;

/// How the measured container divides between two segments.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SegmentSplit {
    /// Side by side: `first` takes `leading` points, then a `gap` of points
    /// nothing is drawn into, then `second` fills the rest.
    Horizontal { leading: f32, gap: f32 },
    /// Stacked: `first` takes `top` points, then `gap`, then `second`.
    Vertical { top: f32, gap: f32 },
}

impl SegmentSplit {
    /// How `container` (in window coordinates) divides across the separating
    /// features in `environment`. `None` when at most one segment overlaps it.
    pub fn resolve(environment: &Environment, container: Rect) -> Option<Self> {
        if container.width <= 0.0 || container.height <= 0.0 {
            return None;
        }
        let segments = display_features::window_segments(
            environment.viewport.bounds(),
            &environment.display_features,
        );
        let pieces: Vec<Rect> = segments
            .iter()
            .filter_map(|segment| segment.intersection(container))
            .collect();
        let [first, second] = pieces.as_slice() else {
            return None;
        };
        if second.x >= first.max_x() {
            Some(Self::Horizontal {
                leading: first.max_x() - container.x,
                gap: second.x - first.max_x(),
            })
        } else if second.y >= first.max_y() {
            Some(Self::Vertical {
                top: first.max_y() - container.y,
                gap: second.y - first.max_y(),
            })
        } else {
            None
        }
    }
}

/// Shows `single` while one segment holds the container, and `first` / `second`
/// in the two segments a fold or hinge divides it into. Nothing is placed under
/// an occluding hinge. Physical order: left then right, top then bottom.
pub struct SegmentedLayout {
    single: AnyView,
    split: Option<(AnyView, AnyView)>,
    frame: State<Rect>,
}

impl SegmentedLayout {
    pub fn new(single: impl View + 'static) -> Self {
        Self {
            single: AnyView::new(single),
            split: None,
            frame: State::new(Rect::ZERO),
        }
    }

    pub fn split(mut self, first: impl View + 'static, second: impl View + 'static) -> Self {
        self.split = Some((AnyView::new(first), AnyView::new(second)));
        self
    }
}

impl View for SegmentedLayout {
    fn body(&self, _cx: &Scope) -> AnyView {
        let frame = self.frame.get();
        let single = self.single.clone();
        let split = self.split.clone();
        let content = AnyView::new(SurfaceReader::new(move |environment: &Environment| match (
            &split,
            SegmentSplit::resolve(environment, frame),
        ) {
            (Some((first, second)), Some(division)) => {
                divided(first.clone(), second.clone(), division)
            }
            _ => single.clone(),
        }));
        AnyView::new(FrameProbe {
            frame: self.frame.clone(),
            content,
        })
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("frame", &self.frame)]
    }
}

fn divided(first: AnyView, second: AnyView, division: SegmentSplit) -> AnyView {
    let (direction, extent, gap) = match division {
        SegmentSplit::Horizontal { leading, gap } => (FlexDirection::Row, leading, gap),
        SegmentSplit::Vertical { top, gap } => (FlexDirection::Column, top, gap),
    };
    // An empty view materialises no node, so the gap is an empty box.
    let gap_view = AnyView::new(
        BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(cove_composition::EmptyView),
        )
        .flex_with(0.0, 0.0, Some(gap.max(0.0))),
    );
    AnyView::new(
        BoxView::new(
            direction,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(first.flex_with(0.0, 0.0, Some(extent.max(0.0)))),
                gap_view,
                AnyView::new(second.flex_with(1.0, 1.0, Some(0.0))),
            ])),
        )
        .frame(
            Some(FrameDimension::Percent(100.0)),
            Some(FrameDimension::Percent(100.0)),
            None,
            None,
            None,
            None,
        ),
    )
}

/// Arranges content for the fold posture: `tabletop` content above and below a
/// horizontal fold, `book` content either side of a vertical one, `flat`
/// otherwise — including when the posture's fold does not cross this view.
pub struct PostureLayout {
    flat: AnyView,
    tabletop: Option<(AnyView, AnyView)>,
    book: Option<(AnyView, AnyView)>,
}

impl PostureLayout {
    pub fn new(flat: impl View + 'static) -> Self {
        Self {
            flat: AnyView::new(flat),
            tabletop: None,
            book: None,
        }
    }

    /// Content above the fold, then the controls below it.
    pub fn tabletop(mut self, top: impl View + 'static, bottom: impl View + 'static) -> Self {
        self.tabletop = Some((AnyView::new(top), AnyView::new(bottom)));
        self
    }

    /// The left page, then the right page.
    pub fn book(mut self, left: impl View + 'static, right: impl View + 'static) -> Self {
        self.book = Some((AnyView::new(left), AnyView::new(right)));
        self
    }
}

impl View for PostureLayout {
    fn body(&self, _cx: &Scope) -> AnyView {
        let flat = self.flat.clone();
        let tabletop = self.tabletop.clone();
        let book = self.book.clone();
        AnyView::new(crate::adaptive::PostureReader::new(
            move |posture: Posture| {
                let pair = match posture {
                    Posture::Tabletop => tabletop.clone(),
                    Posture::Book => book.clone(),
                    Posture::Flat => None,
                };
                let layout = SegmentedLayout::new(flat.clone());
                AnyView::new(match pair {
                    Some((first, second)) => layout.split(first, second),
                    None => layout,
                })
            },
        ))
    }
}

/// Publishes its own window frame after each layout pass and fills its parent.
pub(crate) struct FrameProbe {
    pub(crate) frame: State<Rect>,
    pub(crate) content: AnyView,
}

struct FrameMeasurement {
    state: std::cell::RefCell<State<Rect>>,
    last: Cell<Rect>,
}

impl FrameMeasurement {
    fn update(&self, node: &Node) {
        let frame = node.absolute_frame();
        let last = self.last.get();
        let finite = frame.x.is_finite()
            && frame.y.is_finite()
            && frame.width.is_finite()
            && frame.height.is_finite();
        if !finite
            || ((frame.x - last.x).abs() < TOLERANCE
                && (frame.y - last.y).abs() < TOLERANCE
                && (frame.width - last.width).abs() < TOLERANCE
                && (frame.height - last.height).abs() < TOLERANCE)
        {
            return;
        }
        self.last.set(frame);
        self.state.borrow().set(frame);
    }
}

impl PrimitiveView for FrameProbe {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn update_node(&self, node: &Node) {
        if let Some(measurement) = node.attachment::<Rc<FrameMeasurement>>(FRAME_KEY) {
            *measurement.state.borrow_mut() = self.frame.clone();
            return;
        }
        let measurement = Rc::new(FrameMeasurement {
            state: std::cell::RefCell::new(self.frame.clone()),
            last: Cell::new(Rect::ZERO),
        });
        node.set_attachment(FRAME_KEY, measurement.clone());
        let previous = node.layout_did_update();
        node.set_layout_did_update(Some(Rc::new(move |node| {
            if let Some(previous) = &previous {
                previous(node);
            }
            measurement.update(node);
        })));
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_width_percent(100.0);
        layout.set_height_percent(100.0);
        layout.set_flex_grow(1.0);
        layout.set_align_items(Align::Stretch);
        Some(layout)
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}

impl View for FrameProbe {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}
