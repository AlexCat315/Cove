//! Translation of Primitives/Controls/Form/LabelPlacement.swift.

/// Label placement shared by forms and property grids. Adaptive placement uses
/// the field's own allocated width, rather than the host window's width class.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FieldLabelPlacement {
    #[default]
    Adaptive,
    Leading,
    Stacked,
}

impl FieldLabelPlacement {
    pub fn is_leading(self, width: f32, label_width: f32, min_value_width: f32, spacing: f32) -> bool {
        match self {
            Self::Adaptive => width >= label_width + spacing + min_value_width,
            Self::Leading => true,
            Self::Stacked => false,
        }
    }
}
