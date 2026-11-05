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
use crate::Scope;
use std::collections::HashSet;
use std::rc::Rc;

use cove_core::{Binding, Color};

use crate::modifiers::layout::LayoutFrameExt;
use crate::primitives::controls::fields::numeric::number_field::{NumberField, NumberFieldPrefix};
use crate::primitives::controls::fields::numeric::options::NumberFieldOptions;
use crate::primitives::controls::fields::vec3_model::{
    Vec3, Vec3Axis, Vec3Codec, Vec3Constraints, minimum_axis_width, shows_mixed,
};
use crate::primitives::overlays::context_menu::ContextMenu;
use crate::primitives::overlays::menu::{MenuEntry, MenuItem};
use crate::primitives::text::text_field::TextFieldSize;
use crate::theme::SemanticColorRef;
use crate::{AnyView, TupleView, View};

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
    /// Formatting, bounds, step and enabled state shared by all three axes, the
    /// same type a single [`NumberField`] takes.
    pub options: NumberFieldOptions,
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
            options: NumberFieldOptions {
                size: TextFieldSize::Small,
                ..Default::default()
            },
            constraints: Vec3Constraints::default(),
            mixed_axes: HashSet::new(),
            mixed_value_label: crate::i18n::tr("Mixed value"),
            minimum_axis_width: 88.0,
            on_copy: None,
            on_paste: None,
        }
    }

    /// The presentation shared by all three axes.
    pub fn options(mut self, options: NumberFieldOptions) -> Self {
        self.options = options;
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
            options: self.options.clone(),
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
        } else {
            cove_core::scene::ClipboardHolder::write(&Vec3Codec::format(self.value()));
        }
    }

    /// Pastes through an authored reader, or asynchronously through the platform clipboard.
    pub fn paste(&self) {
        if !self.options.is_enabled {
            return;
        }
        if let Some(read) = self.on_paste.as_ref() {
            if let Some(vector) = read() {
                self.write(vector);
            }
        } else {
            let field = self.snapshot();
            cove_core::scene::ClipboardHolder::request_read(Box::new(move |text| {
                if let Some(vector) = text.and_then(|text| Vec3Codec::parse(&text)) {
                    field.write(vector);
                }
            }));
        }
    }

    /// Resets every axis to the authored reset value.
    pub fn reset(&self) {
        if !self.options.is_enabled {
            return;
        }
        if let Some(reset) = self.constraints.reset {
            self.write(reset);
        }
    }

    /// The row's context menu. An authored synchronous reader can preflight Paste;
    /// the platform bridge validates asynchronous clipboard replies before writing.
    fn menu(&self) -> Vec<MenuEntry> {
        let copy = self.snapshot();
        let mut entries = vec![MenuEntry::Item(
            MenuItem::new(1, crate::i18n::tr("Copy Vector"), move || copy.copy()).enabled(true),
        )];
        let paste = self.snapshot();
        let can_paste =
            self.options.is_enabled && self.on_paste.as_ref().is_none_or(|read| read().is_some());
        entries.push(MenuEntry::Item(
            MenuItem::new(2, crate::i18n::tr("Paste Vector"), move || paste.paste())
                .enabled(can_paste),
        ));
        if self.constraints.reset.is_some() {
            let reset = self.snapshot();
            entries.push(MenuEntry::Item(
                MenuItem::new(3, crate::i18n::tr("Reset Vector"), move || reset.reset())
                    .enabled(self.options.is_enabled),
            ));
        }
        entries
    }

    fn axis(&self, axis: Vec3Axis, value: Binding<f32>) -> AnyView {
        let mixed = shows_mixed(&self.mixed_axes, axis);
        let reset = self.constraints.reset.map(|reset| reset.get(axis));
        let prefix = NumberFieldPrefix::new(axis.label()).tint(axis_tint(axis));
        let mut field = NumberField::new(value)
            .options(self.options.clone())
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
    fn body(&self, _cx: &Scope) -> AnyView {
        let axes = [
            self.axis(Vec3Axis::X, self.x.clone()),
            self.axis(Vec3Axis::Y, self.y.clone()),
            self.axis(Vec3Axis::Z, self.z.clone()),
        ];
        let minimum = self.minimum_axis_width;
        let row = crate::theme::TextScalerReader::new(move |scaler| {
            crate::LayoutContainer::new(
                crate::primitives::controls::fields::numeric::axis_layout::AxisLayout::new(
                    4.0,
                    minimum_axis_width(minimum, scaler.scale(1.0)),
                    scaler.scale(AXIS_BASIS),
                ),
                TupleView::new(
                    axes.iter()
                        .cloned()
                        .map(|axis| {
                            AnyView::new(axis.frame(
                                Some(crate::modifiers::layout::FrameDimension::Percent(100.0)),
                                None,
                                None,
                                None,
                                None,
                                None,
                            ))
                        })
                        .collect(),
                ),
            )
        });
        let field = self.snapshot();
        AnyView::new(ContextMenu::new(move || field.menu(), row).width(220.0))
    }
}

impl Clone for Vec3Field {
    fn clone(&self) -> Self {
        self.snapshot()
    }
}
