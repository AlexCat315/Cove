//! Translation of Primitives/Controls/Fields/Numeric/Vec3Field.swift (the view
//! half).
//!
//! Three caller-owned numeric values with a shared format and constraints. Each
//! axis is a [`NumberField`] led by its coloured axis chip: drag a chip to scrub
//! that axis, double-click it to reset the axis. The row's context menu copies
//! the vector, pastes one and resets every axis.
//!
//! Vector formatting, parsing, clamping and the mixed-value rule live in
//! [`Vec3Codec`], [`Vec3Constraints`], [`shows_mixed`] and
//! [`minimum_axis_width`].
use std::collections::HashSet;
use std::rc::Rc;

use cove_core::scene::{Align, FlexDirection, Justify, Node, Wrap};
use cove_core::{Binding, Color};

use crate::modifiers::layout::LayoutFrameExt;
use crate::primitives::controls::fields::numeric::number_field::{NumberField, NumberFieldPrefix};
use crate::primitives::controls::fields::vec3_model::{
    Vec3, Vec3Axis, Vec3Codec, Vec3Constraints, minimum_axis_width, shows_mixed,
};
use crate::primitives::text::text_field::TextFieldSize;
use crate::theme::SemanticColorRef;
use crate::{impl_primitive_view, AnyView, BoxView, PrimitiveView, TupleView, View};

/// Axis chip width used when the row has room for three side by side.
pub const AXIS_BASIS: f32 = 96.0;

/// RGB tints for the X, Y and Z chips.
pub fn axis_tint(axis: Vec3Axis) -> SemanticColorRef {
    let color = match axis {
        Vec3Axis::X => Color::from_rgba8(211, 67, 67, 255),
        Vec3Axis::Y => Color::from_rgba8(95, 173, 86, 255),
        Vec3Axis::Z => Color::from_rgba8(78, 128, 220, 255),
    };
    SemanticColorRef::new(move |_| color)
}

/// Three numeric fields sharing one format and one set of constraints.
pub struct Vec3Field {
    pub x: Binding<f32>,
    pub y: Binding<f32>,
    pub z: Binding<f32>,
    pub decimals: usize,
    pub size: TextFieldSize,
    pub is_enabled: bool,
    pub constraints: Vec3Constraints,
    pub mixed_axes: HashSet<String>,
    pub mixed_value_label: String,
    pub minimum_axis_width: f32,
    pub on_copy: Option<Rc<dyn Fn(String)>>,
    pub on_paste: Option<Rc<dyn Fn() -> Option<Vec3>>>,
}

impl Vec3Field {
    pub fn new(x: Binding<f32>, y: Binding<f32>, z: Binding<f32>) -> Self {
        Self {
            x,
            y,
            z,
            decimals: 2,
            size: TextFieldSize::Small,
            is_enabled: true,
            constraints: Vec3Constraints::default(),
            mixed_axes: HashSet::new(),
            mixed_value_label: "Mixed value".to_string(),
            minimum_axis_width: 88.0,
            on_copy: None,
            on_paste: None,
        }
    }

    pub fn decimals(mut self, decimals: usize) -> Self {
        self.decimals = decimals.min(6);
        self
    }

    pub fn size(mut self, size: TextFieldSize) -> Self {
        self.size = size;
        self
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn constraints(mut self, constraints: Vec3Constraints) -> Self {
        self.constraints = constraints;
        self
    }

    /// Axes whose values disagree, by lowercase axis name.
    pub fn mixed_axes(mut self, mixed_axes: HashSet<String>) -> Self {
        self.mixed_axes = mixed_axes;
        self
    }

    pub fn mixed_value_label(mut self, label: impl Into<String>) -> Self {
        self.mixed_value_label = label.into();
        self
    }

    /// Dense pointer workbenches can use narrower axes.
    pub fn minimum_axis_width(mut self, minimum_axis_width: f32) -> Self {
        self.minimum_axis_width = minimum_axis_width;
        self
    }

    /// Supplies the clipboard writer used by the row's copy action.
    pub fn on_copy(mut self, handler: impl Fn(String) + 'static) -> Self {
        self.on_copy = Some(Rc::new(handler));
        self
    }

    /// Supplies the clipboard reader used by the row's paste action.
    pub fn on_paste(mut self, handler: impl Fn() -> Option<Vec3> + 'static) -> Self {
        self.on_paste = Some(Rc::new(handler));
        self
    }

    fn snapshot(&self) -> Vec3Field {
        Vec3Field {
            x: self.x.clone(),
            y: self.y.clone(),
            z: self.z.clone(),
            decimals: self.decimals,
            size: self.size,
            is_enabled: self.is_enabled,
            constraints: self.constraints,
            mixed_axes: self.mixed_axes.clone(),
            mixed_value_label: self.mixed_value_label.clone(),
            minimum_axis_width: self.minimum_axis_width,
            on_copy: self.on_copy.clone(),
            on_paste: self.on_paste.clone(),
        }
    }

    /// Reads the three bindings as one vector.
    pub fn value(&self) -> Vec3 {
        Vec3::new(self.x.get(), self.y.get(), self.z.get())
    }

    /// Writes every axis that actually changes, after clamping.
    pub fn write(&self, vector: Vec3) {
        let current = self.value();
        for (axis, value) in self.constraints.write(current, vector) {
            match axis {
                Vec3Axis::X => self.x.set(value),
                Vec3Axis::Y => self.y.set(value),
                Vec3Axis::Z => self.z.set(value),
            }
        }
    }

    /// Copies the row's vector in the locale-independent format.
    pub fn copy(&self) {
        if let Some(handler) = self.on_copy.as_ref() {
            handler(Vec3Codec::format(self.value()));
        }
    }

    /// Pastes a vector parsed from the clipboard, when the host supplies one.
    pub fn paste(&self) {
        if !self.is_enabled {
            return;
        }
        let Some(read) = self.on_paste.as_ref() else { return };
        if let Some(vector) = read() {
            self.write(vector);
        }
    }

    /// Resets every axis to the authored reset value.
    pub fn reset(&self) {
        if !self.is_enabled {
            return;
        }
        if let Some(reset) = self.constraints.reset {
            self.write(reset);
        }
    }

    fn axis(&self, axis: Vec3Axis, value: Binding<f32>) -> AnyView {
        let mixed = shows_mixed(&self.mixed_axes, axis);
        let reset = self.constraints.reset.map(|reset| reset.get(axis));
        let prefix = NumberFieldPrefix::new(axis.label()).tint(axis_tint(axis));
        let mut field = NumberField::new(value)
            .decimals(self.decimals)
            .size(self.size)
            .enabled(self.is_enabled)
            .range(self.constraints.minimum, self.constraints.maximum)
            .prefix(prefix);
        if mixed {
            field = field.mixed_value_label(self.mixed_value_label.clone());
        }
        if let Some(reset) = reset {
            field = field.reset_value(reset);
        }
        AnyView::new(field)
    }

    /// The minimum width one axis needs at a given text scale.
    pub fn resolved_minimum_width(&self, scale: f32) -> f32 {
        minimum_axis_width(self.minimum_axis_width, scale)
    }
}

impl View for Vec3Field {
    fn body(&self) -> AnyView {
        let axes = vec![
            self.axis(Vec3Axis::X, self.x.clone()),
            self.axis(Vec3Axis::Y, self.y.clone()),
            self.axis(Vec3Axis::Z, self.z.clone()),
        ];
        AnyView::new(Vec3Row {
            minimum_width: self.resolved_minimum_width(1.0),
            axes: AnyView::new(TupleView::new(
                axes.into_iter()
                    .map(|axis| AnyView::new(AnyView::new(axis).frame_width(AXIS_BASIS)))
                    .collect(),
            )),
        })
    }
}

impl Clone for Vec3Field {
    fn clone(&self) -> Self {
        self.snapshot()
    }
}

/// The three axes: a wrapping row that keeps every value visible, spilling to a
/// second line when the available width cannot hold three.
struct Vec3Row {
    minimum_width: f32,
    axes: AnyView,
}

impl PrimitiveView for Vec3Row {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Row, Align::Center, Justify::FlexStart, Wrap::Wrap, 8.0,
                self.axes.clone(),
            )
            .frame_min_width(self.minimum_width),
        )]
    }
}
impl_primitive_view!(Vec3Row);
