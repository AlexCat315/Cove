//! Translation of `Primitives/Controls/Fields/Numeric/Vec3Field.swift` (the
//! model half: vector format/parse and clamping).
//!
//! Three caller-owned numeric values with a shared format and constraints. Drag
//! an axis chip to scrub, double-click to reset, and the row's context menu
//! copies the vector, pastes one, and resets every axis.
//!
//! Behaviour checklist:
//! - formatting is locale-independent and drops a trailing `.0` on whole numbers
//! - parsing accepts commas, semicolons and whitespace, with optional brackets
//!   or a `Name(...)` wrapper
//! - parsing rejects anything that is not exactly three finite numbers
//! - writing clamps each axis to the configured bounds and only reports real
//!   changes

/// One axis of a vector field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vec3Axis {
    X,
    Y,
    Z,
}

impl Vec3Axis {
    /// The axis label, which is also the mixed-value key.
    pub fn label(self) -> &'static str {
        match self {
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
        }
    }

    /// The lowercase form used to look the axis up in `mixed_axes`.
    pub fn key(self) -> &'static str {
        match self {
            Self::X => "x",
            Self::Y => "y",
            Self::Z => "z",
        }
    }

    pub fn index(self) -> usize {
        match self {
            Self::X => 0,
            Self::Y => 1,
            Self::Z => 2,
        }
    }
}

/// A three-component value.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn get(self, axis: Vec3Axis) -> f32 {
        match axis {
            Vec3Axis::X => self.x,
            Vec3Axis::Y => self.y,
            Vec3Axis::Z => self.z,
        }
    }

    pub fn set(&mut self, axis: Vec3Axis, value: f32) {
        match axis {
            Vec3Axis::X => self.x = value,
            Vec3Axis::Y => self.y = value,
            Vec3Axis::Z => self.z = value,
        }
    }

    pub fn components(self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }
}

/// Bounds and reset value for a vector field.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3Constraints {
    pub minimum: Option<f32>,
    pub maximum: Option<f32>,
    pub reset: Option<Vec3>,
}

impl Vec3Constraints {
    /// Clamps one value to the configured bounds.
    pub fn clamp(&self, value: f32) -> f32 {
        let mut out = value;
        if let Some(minimum) = self.minimum {
            out = out.max(minimum);
        }
        if let Some(maximum) = self.maximum {
            out = out.min(maximum);
        }
        out
    }

    /// Clamps every axis and reports which axes actually changed.
    ///
    /// Only changed axes are written back, so a caller's binding is not poked
    /// with values it already holds.
    pub fn write(&self, current: Vec3, next: Vec3) -> Vec<(Vec3Axis, f32)> {
        const AXES: [Vec3Axis; 3] = [Vec3Axis::X, Vec3Axis::Y, Vec3Axis::Z];
        AXES.into_iter()
            .filter_map(|axis| {
                let clamped = self.clamp(next.get(axis));
                (current.get(axis) != clamped).then_some((axis, clamped))
            })
            .collect()
    }
}

/// Vector formatting and parsing.
pub struct Vec3Codec;

impl Vec3Codec {
    /// Locale-independent so a copied vector pastes the same everywhere.
    ///
    /// Whole numbers print without a `.0`, which is what keeps a copied vector
    /// readable and re-parseable.
    pub fn format(vector: Vec3) -> String {
        vector
            .components()
            .into_iter()
            .map(Self::format_component)
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn format_component(value: f32) -> String {
        if value.round() == value && value.abs() < 1e9 {
            format!("{}", value as i64)
        } else {
            format!("{value}")
        }
    }

    /// Three finite numbers separated by commas, semicolons or spaces, with
    /// optional brackets or a `Name(...)` wrapper.
    pub fn parse(text: &str) -> Option<Vec3> {
        let trimmed = text.trim();
        // Strip a wrapper only when the brackets are in the right order.
        let body = match (first_bracket(trimmed), last_bracket(trimmed)) {
            (Some(open), Some(close)) if open < close => &trimmed[open + 1..close],
            _ => trimmed,
        };
        let parts: Vec<f32> = body
            .split([',', ';', ' ', '\t', '\n'])
            .filter(|part| !part.is_empty())
            .filter_map(|part| part.parse::<f32>().ok())
            .collect();
        if parts.len() != 3 || !parts.iter().all(|value| value.is_finite()) {
            return None;
        }
        Some(Vec3::new(parts[0], parts[1], parts[2]))
    }
}

fn first_bracket(text: &str) -> Option<usize> {
    text.char_indices()
        .find(|(_, character)| matches!(character, '(' | '[' | '{'))
        .map(|(index, _)| index)
}

fn last_bracket(text: &str) -> Option<usize> {
    text.char_indices()
        .rfind(|(_, character)| matches!(character, ')' | ']' | '}'))
        .map(|(index, _)| index)
}

/// Whether `axis` should show the mixed-value label.
pub fn shows_mixed(mixed_axes: &std::collections::HashSet<String>, axis: Vec3Axis) -> bool {
    mixed_axes.contains(axis.key())
}

/// The axis minimum width after text scaling, floored at 44 points.
pub fn minimum_axis_width(minimum_axis_width: f32, scale: f32) -> f32 {
    scale * minimum_axis_width.max(44.0)
}
