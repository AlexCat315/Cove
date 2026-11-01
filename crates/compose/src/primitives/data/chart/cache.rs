//! Translation of `Primitives/Data/Chart/Cache.swift`.
//!
//! Reuses one resolved chart across the passes that need it. Two slots cover
//! the two spaces a chart lives in: drawing/hit-testing in absolute
//! coordinates and the overlay in the node's own space.
use cove_core::scene::{Point, Size};

use crate::theme::tokens::TextStyleToken;

use super::morph::ChartSamples;
use super::plot::ChartResolution;

struct Slot {
    size: Size,
    origin: Point,
    text: TextStyleToken,
    resolution: ChartResolution,
}

pub struct ChartResolutionCache {
    slots: Vec<Slot>,
    pub samples: Option<ChartSamples>,
    /// How many resolutions have been built. A repeat resolve leaves it alone.
    pub builds: usize,
}
impl Default for ChartResolutionCache {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            samples: None,
            builds: 0,
        }
    }
}
impl ChartResolutionCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Forgets both slots after the data or text metrics change.
    pub fn invalidate(&mut self) {
        self.slots.clear();
    }

    pub fn resolution(
        &mut self,
        size: Size,
        origin: Point,
        text: &TextStyleToken,
        build: impl FnOnce() -> ChartResolution,
    ) -> ChartResolution {
        if let Some(hit) = self
            .slots
            .iter()
            .find(|slot| slot.size == size && slot.origin == origin && &slot.text == text)
        {
            return hit.resolution.clone();
        }
        let value = build();
        self.builds += 1;
        self.slots.push(Slot {
            size,
            origin,
            text: text.clone(),
            resolution: value.clone(),
        });
        // A resize drag walks through sizes one at a time; two slots keep both
        // spaces warm without growing.
        if self.slots.len() > 2 {
            self.slots.remove(0);
        }
        value
    }
}
