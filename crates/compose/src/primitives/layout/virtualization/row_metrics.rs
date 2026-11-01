//! Translation of Primitives/Layout/Virtualization/RowMetrics.swift.
//!
//! A private cache retained by the view's state. A fresh view input changes the
//! generation; scrolling the same input only queries the existing index.
use super::row_index::VariableRowIndex;

/// Cached row index that only rebuilds when its inputs change.
#[derive(Default)]
pub struct VariableRowMetrics<Id: Ord + Clone> {
    generation: Option<u64>,
    width: f32,
    scaler_bits: u32,
    spacing: f32,
    viewport_height: f64,
    rows: VariableRowIndex<Id>,
}

impl<Id: Ord + Clone> VariableRowMetrics<Id> {
    pub fn new() -> Self {
        Self {
            generation: None,
            width: -1.0,
            scaler_bits: 1.0_f32.to_bits(),
            spacing: -1.0,
            viewport_height: -1.0,
            rows: VariableRowIndex::new(),
        }
    }

    pub fn rows(&self) -> &VariableRowIndex<Id> {
        &self.rows
    }

    /// Rebuild when any authored input changed; otherwise only re-clamp when the
    /// viewport did. Scroll view owns transient rubber-band offsets, so normal
    /// scrolling preserves its motion through here untouched.
    pub fn synchronize(
        &mut self,
        ids: &[Id],
        heights: &[f32],
        generation: u64,
        width: f32,
        scaler: f32,
        spacing: f32,
        offset: f64,
        viewport: f64,
    ) -> f64 {
        let same = self.generation == Some(generation)
            && self.width == width
            && self.scaler_bits == scaler.to_bits()
            && self.spacing == spacing;
        if same {
            if self.viewport_height != viewport {
                self.viewport_height = viewport;
                return self.rows.clamp(offset, viewport);
            }
            return offset;
        }
        let anchor = self.rows.anchor_at(offset);
        self.rows.replace(ids, heights, spacing);
        self.generation = Some(generation);
        self.width = width;
        self.scaler_bits = scaler.to_bits();
        self.spacing = spacing;
        self.viewport_height = viewport;
        anchor
            .map(|anchor| self.rows.offset_for(&anchor, viewport))
            .unwrap_or_else(|| self.rows.clamp(offset, viewport))
    }
}
