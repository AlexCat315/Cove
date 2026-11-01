//! Translation of Primitives/Controls/Toolbar/OverflowLayout.swift.
//!
//! Measurements are pure; publishing the chosen menu happens during placement,
//! just as a container query publishes its final allocated size after layout. The
//! main axis follows the toolbar: widths for a horizontal bar, heights for a
//! vertical one.
use crate::{LayoutSubview, ProposedViewSize};
use cove_core::{Point, Rect, Size};

use super::{ToolbarAxis, ToolbarItemPlacement};

/// The measured plan for one toolbar row.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToolbarArrangement {
    pub sizes: Vec<Size>,
    pub visible: Vec<usize>,
    /// Indices of commands moved into the overflow menu.
    pub hidden: Vec<usize>,
    pub size: Size,
}

/// Measures commands at their natural size and moves lower-priority ones into a
/// menu when the bar is too narrow.
#[derive(Clone)]
pub struct ToolbarOverflowLayout {
    pub available_extent: f32,
    pub placements: Vec<ToolbarItemPlacement>,
    pub spacing: f32,
    pub fits_content: bool,
    pub axis: ToolbarAxis,
    /// Called during placement with the commands that did not fit.
    pub on_overflow: Option<std::rc::Rc<dyn Fn(&[usize])>>,
    pub on_content_extent: Option<std::rc::Rc<dyn Fn(f32)>>,
}

impl ToolbarOverflowLayout {
    pub fn new(available_extent: f32, placements: Vec<ToolbarItemPlacement>, spacing: f32) -> Self {
        Self {
            available_extent,
            placements,
            spacing,
            fits_content: false,
            axis: ToolbarAxis::Horizontal,
            on_overflow: None,
            on_content_extent: None,
        }
    }

    fn main(&self, size: Size) -> f32 {
        if self.axis == ToolbarAxis::Vertical {
            size.height
        } else {
            size.width
        }
    }

    fn cross(&self, size: Size) -> f32 {
        if self.axis == ToolbarAxis::Vertical {
            size.width
        } else {
            size.height
        }
    }

    /// Resolve which commands fit. Pure: placement publishes the result.
    pub fn arrangement(&self, sizes: &[Size]) -> ToolbarArrangement {
        // One extra subview is the overflow button.
        if sizes.len() != self.placements.len() + 1 {
            return ToolbarArrangement {
                sizes: sizes.to_vec(),
                ..Default::default()
            };
        }
        let more = self.placements.len();
        let mut hidden: Vec<usize> = self
            .placements
            .iter()
            .enumerate()
            .filter(|(_, placement)| **placement == ToolbarItemPlacement::Overflow)
            .map(|(index, _)| index)
            .collect();
        let visible = |hidden: &[usize]| -> Vec<usize> {
            let mut shown: Vec<usize> = (0..self.placements.len())
                .filter(|index| !hidden.contains(index))
                .collect();
            if !hidden.is_empty() {
                shown.push(more);
            }
            shown
        };
        let extent = |indices: &[usize]| -> f32 {
            indices
                .iter()
                .map(|index| self.main(sizes[*index]))
                .sum::<f32>()
                + self.spacing * indices.len().saturating_sub(1) as f32
        };
        // Automatic commands are dropped lowest priority first; ties drop later
        // items first, so source order survives among equals.
        let mut candidates: Vec<usize> = self
            .placements
            .iter()
            .enumerate()
            .filter(|(_, placement)| {
                matches!(
                    placement,
                    ToolbarItemPlacement::Automatic
                        | ToolbarItemPlacement::AutomaticWithPriority(_)
                )
            })
            .map(|(index, _)| index)
            .collect();
        candidates.sort_by(|lhs, rhs| {
            let a = self.placements[*lhs].priority();
            let b = self.placements[*rhs].priority();
            if a == b { rhs.cmp(lhs) } else { a.cmp(&b) }
        });
        for candidate in candidates {
            if extent(&visible(&hidden)) > self.available_extent {
                hidden.push(candidate);
            }
        }
        hidden.sort_unstable();
        let shown = visible(&hidden);
        let length = if self.fits_content {
            extent(&shown)
        } else {
            self.available_extent.max(extent(&shown))
        };
        let thickness = shown
            .iter()
            .map(|index| self.cross(sizes[*index]))
            .fold(0.0_f32, f32::max);
        let size = if self.axis == ToolbarAxis::Vertical {
            Size {
                width: thickness,
                height: length,
            }
        } else {
            Size {
                width: length,
                height: thickness,
            }
        };
        ToolbarArrangement {
            sizes: sizes.to_vec(),
            visible: shown,
            hidden,
            size,
        }
    }

    pub fn size_that_fits(&self, _proposal: ProposedViewSize, subviews: &[LayoutSubview]) -> Size {
        let sizes: Vec<Size> = subviews
            .iter()
            .map(|subview| subview.size_that_fits(ProposedViewSize::UNSPECIFIED))
            .collect();
        self.arrangement(&sizes).size
    }

    pub fn place_subviews(
        &self,
        bounds: Rect,
        _proposal: ProposedViewSize,
        subviews: &[LayoutSubview],
    ) {
        let sizes: Vec<Size> = subviews
            .iter()
            .map(|subview| subview.size_that_fits(ProposedViewSize::UNSPECIFIED))
            .collect();
        let plan = self.arrangement(&sizes);
        let reversed = self.axis == ToolbarAxis::Horizontal
            && subviews.first().is_some_and(|subview| {
                subview.layout_direction == cove_core::scene::Direction::Rtl
            });
        let mut cursor = if self.axis == ToolbarAxis::Vertical {
            bounds.y
        } else if reversed {
            bounds.x + bounds.width
        } else {
            bounds.x
        };
        for index in 0..subviews.len() {
            let size = plan.sizes[index];
            if !plan.visible.contains(&index) {
                // A hidden command parks far off-screen rather than collapsing, so
                // its measured size stays available.
                subviews[index].place(
                    Point::new(-1_000_000.0, 0.0),
                    ProposedViewSize::UNSPECIFIED,
                    false,
                );
                continue;
            }
            if self.axis == ToolbarAxis::Vertical {
                subviews[index].place(
                    Point::new(bounds.x + (plan.size.width - size.width) / 2.0, cursor),
                    ProposedViewSize::new(Some(size.width), Some(size.height)),
                    true,
                );
                cursor += size.height + self.spacing;
            } else {
                let x = if reversed {
                    cursor - size.width
                } else {
                    cursor
                };
                subviews[index].place(
                    Point::new(x, bounds.y + (plan.size.height - size.height) / 2.0),
                    ProposedViewSize::new(Some(size.width), Some(size.height)),
                    true,
                );
                cursor += (if reversed { -1.0 } else { 1.0 }) * (size.width + self.spacing);
            }
        }
        if let Some(on_overflow) = self.on_overflow.as_ref() {
            on_overflow(&plan.hidden);
        }
        if let Some(on_content_extent) = self.on_content_extent.as_ref() {
            let total = plan
                .visible
                .iter()
                .map(|index| self.main(plan.sizes[*index]))
                .sum::<f32>()
                + self.spacing * plan.visible.len().saturating_sub(1) as f32;
            on_content_extent(total);
        }
    }
}

impl crate::Layout for ToolbarOverflowLayout {
    fn size_that_fits(
        &self,
        proposal: ProposedViewSize,
        subviews: &[crate::LayoutSubview],
    ) -> Size {
        ToolbarOverflowLayout::size_that_fits(self, proposal, subviews)
    }

    fn place_subviews(
        &self,
        bounds: Rect,
        proposal: ProposedViewSize,
        subviews: &[crate::LayoutSubview],
    ) {
        ToolbarOverflowLayout::place_subviews(self, bounds, proposal, subviews)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn toolbar_overflow_moves_low_priority_commands() {
        use crate::primitives::controls::toolbar::{ToolbarItemPlacement, ToolbarOverflowLayout};
        use cove_core::Size;
        // four commands of 44pt + 4pt spacing = 188pt; budget 100 fits two.
        let sizes = vec![
            Size {
                width: 44.0,
                height: 44.0,
            },
            Size {
                width: 44.0,
                height: 44.0,
            },
            Size {
                width: 44.0,
                height: 44.0,
            },
            Size {
                width: 44.0,
                height: 44.0,
            },
            Size {
                width: 44.0,
                height: 44.0,
            },
        ];
        let layout = ToolbarOverflowLayout::new(
            100.0,
            vec![
                ToolbarItemPlacement::AutomaticWithPriority(5),
                ToolbarItemPlacement::AutomaticWithPriority(1),
                ToolbarItemPlacement::Automatic,
                ToolbarItemPlacement::Always,
            ],
            4.0,
        );
        let plan = layout.arrangement(&sizes);
        // The Always item never moves; lowest priorities go first.
        assert!(!plan.hidden.contains(&3), "Always must stay in the bar");
        assert!(
            plan.hidden.contains(&1),
            "priority 1 drops before priority 5"
        );
    }
}
