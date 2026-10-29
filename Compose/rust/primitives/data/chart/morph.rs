//! Translation of `Primitives/Data/Chart/Morph.swift`.
//!
//! Only sampled values animate. Authored data and session state stay separate.
use std::cell::RefCell;
use std::rc::{Rc, Weak};

use cove_core::scene::{Animation, Interpolatable, Node};

use super::cache::ChartResolutionCache;
use super::model::{ChartDatum, ChartSlice};
use super::plot::ChartContent;

#[derive(Clone, Debug)]
pub struct ChartSamples {
    pub series: Vec<Vec<ChartDatum>>,
    pub slices: Vec<f32>,
}
impl ChartSamples {
    pub fn new(content: &ChartContent) -> Self {
        match content {
            ChartContent::Cartesian { series, .. } => {
                Self { series: series.iter().map(|item| item.data.clone()).collect(), slices: Vec::new() }
            }
            ChartContent::Pie(slices) => {
                Self { series: Vec::new(), slices: slices.iter().map(|slice| slice.value).collect() }
            }
        }
    }

    pub fn applying(&self, content: &ChartContent) -> ChartContent {
        match content {
            ChartContent::Cartesian { series, guides, x } => {
                let mut items = series.clone();
                for (index, item) in items.iter_mut().enumerate() {
                    if let Some(samples) = self.series.get(index) {
                        item.data = samples.clone();
                    }
                }
                ChartContent::Cartesian { series: items, guides: guides.clone(), x: x.clone() }
            }
            ChartContent::Pie(slices) => {
                let mut items = slices.clone();
                for (index, item) in items.iter_mut().enumerate() {
                    if let Some(value) = self.slices.get(index) {
                        item.value = *value;
                    }
                }
                ChartContent::Pie(items)
            }
        }
    }

    fn mix(a: f32, b: f32, t: f32) -> f32 {
        if !a.is_finite() || !b.is_finite() {
            return b;
        }
        a + (b - a) * t.clamp(0.0, 1.0)
    }

    fn same(a: f32, b: f32) -> bool {
        a == b || (a.is_nan() && b.is_nan())
    }
}

impl PartialEq for ChartSamples {
    fn eq(&self, other: &Self) -> bool {
        self.slices.len() == other.slices.len()
            && self.slices.iter().zip(&other.slices).all(|(a, b)| Self::same(*a, *b))
            && self.series.len() == other.series.len()
            && self.series.iter().zip(&other.series).all(|(a, b)| {
                a.len() == b.len()
                    && a.iter().zip(b).all(|(x, y)| Self::same(x.x, y.x) && Self::same(x.y, y.y))
            })
    }
}

impl Interpolatable for ChartSamples {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Self {
        let mut result = to.clone();
        for s in 0..to.series.len() {
            for i in 0..to.series[s].len() {
                let target = to.series[s][i];
                let source = from
                    .series
                    .get(s)
                    .and_then(|track| track.get(i))
                    .copied()
                    .unwrap_or(ChartDatum::new(target.x, 0.0));
                result.series[s][i] = ChartDatum::new(
                    Self::mix(source.x, target.x, progress),
                    Self::mix(source.y, target.y, progress),
                );
            }
        }
        for i in 0..to.slices.len() {
            result.slices[i] = Self::mix(
                from.slices.get(i).copied().unwrap_or(0.0),
                to.slices[i],
                progress,
            );
        }
        result
    }
}

/// Drives the `chart.samples` animatable property. Reduced motion snaps.
pub struct ChartTransition {
    target: Option<ChartSamples>,
}
impl Default for ChartTransition {
    fn default() -> Self {
        Self { target: None }
    }
}
impl ChartTransition {
    pub fn update(
        &mut self,
        content: &ChartContent,
        animation: Option<Animation>,
        node: &Node,
        cache: Rc<RefCell<ChartResolutionCache>>,
    ) {
        let next = ChartSamples::new(content);
        if self.target.as_ref() == Some(&next) || node.prefers_reduced_motion() {
            return;
        }
        let previous = cache.borrow().samples.clone().unwrap_or_else(|| next.clone());
        self.target = Some(next.clone());
        let weak_node = node.downgrade();
        let apply = move |value: ChartSamples| {
            cache.borrow_mut().samples = Some(value);
            cache.borrow_mut().invalidate();
            if let Some(node) = Node::upgrade(&weak_node) {
                node.mark_render_dirty(cove_core::scene::InvalidationSource::Unknown);
            }
        };
        // The surrounding `withAnimation` transaction supplies the animation
        // when this chart has none of its own; the node's helper handles both.
        node.animatable_set_with("chart.samples", previous, next, apply);
        let _ = animation;
    }
}

/// A pie slice sample is kept as a value only; titles and colors stay authored.
pub fn slice_values(slices: &[ChartSlice]) -> Vec<f32> {
    slices.iter().map(|slice| slice.value).collect()
}
