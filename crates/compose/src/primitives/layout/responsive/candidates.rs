//! Select the first measured child layout that fits the proposed size.
use crate::{Layout, LayoutSubview, ProposedViewSize};
use cove_core::Size;
use std::rc::Rc;

pub struct LayoutThatFits {
    candidates: Vec<Rc<dyn Layout>>,
}

impl LayoutThatFits {
    pub fn new(candidates: Vec<Rc<dyn Layout>>) -> Self {
        assert!(
            !candidates.is_empty(),
            "LayoutThatFits requires at least one candidate"
        );
        Self { candidates }
    }

    fn selection(&self, proposal: ProposedViewSize, subviews: &[LayoutSubview]) -> (usize, Size) {
        let mut last_size = Size::default();
        for (index, candidate) in self.candidates.iter().enumerate() {
            let size = candidate.size_that_fits(proposal, subviews);
            last_size = size;
            if fits(size.width, proposal.width) && fits(size.height, proposal.height) {
                return (index, size);
            }
        }
        (self.candidates.len() - 1, last_size)
    }
}

impl Layout for LayoutThatFits {
    fn size_that_fits(&self, proposal: ProposedViewSize, subviews: &[LayoutSubview]) -> Size {
        self.selection(proposal, subviews).1
    }

    fn place_subviews(
        &self,
        bounds: cove_core::Rect,
        _proposal: ProposedViewSize,
        subviews: &[LayoutSubview],
    ) {
        let actual = ProposedViewSize::new(Some(bounds.width), Some(bounds.height));
        let (index, _) = self.selection(actual, subviews);
        self.candidates[index].place_subviews(bounds, actual, subviews);
    }
}

fn fits(extent: f32, available: Option<f32>) -> bool {
    extent.is_finite() && extent >= 0.0 && available.is_none_or(|available| extent <= available)
}
