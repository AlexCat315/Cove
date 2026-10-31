//! Panel sizing contracts and the linear resolver over the layout tree.
//!
//! Swift normalises authored values at construction *and* at decoding, so an
//! invalid document can never reach the layout engine. Rust does the same by
//! routing both paths through the same constructors.
use super::ids::{WorkspaceSlotID, WorkspaceSplitAxis, WorkspaceTabGroupID};
use super::model::{WorkspaceDocument, WorkspaceLayoutNode};
use serde::{Deserialize, Deserializer, Serialize};

/// Logical content points, excluding workspace chrome.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceAxisSizing {
    pub minimum: f32,
    pub preferred: Option<f32>,
    pub maximum: Option<f32>,
}

impl Default for WorkspaceAxisSizing {
    fn default() -> Self {
        Self::new(0.0, None, None)
    }
}

impl WorkspaceAxisSizing {
    pub fn new(minimum: f32, preferred: Option<f32>, maximum: Option<f32>) -> Self {
        let minimum = if minimum.is_finite() {
            minimum.max(0.0)
        } else {
            0.0
        };
        let maximum = maximum.and_then(|value| value.is_finite().then(|| value.max(minimum)));
        let preferred = preferred.and_then(|value| {
            value.is_finite().then(|| match maximum {
                Some(ceiling) => value.max(minimum).min(ceiling),
                None => value.max(minimum),
            })
        });
        Self {
            minimum,
            preferred,
            maximum,
        }
    }

    fn combine_stacked(first: Self, second: Self, gap: f32) -> Self {
        Self::new(
            first.minimum + second.minimum + gap,
            optional_sum(first.preferred, second.preferred, gap),
            optional_sum(first.maximum, second.maximum, gap),
        )
    }

    fn combine_sibling(first: Self, second: Self) -> Self {
        Self::new(
            first.minimum.max(second.minimum),
            [first.preferred, second.preferred]
                .into_iter()
                .flatten()
                .reduce(f32::max),
            [first.maximum, second.maximum]
                .into_iter()
                .flatten()
                .reduce(f32::min),
        )
    }
}

fn optional_sum(first: Option<f32>, second: Option<f32>, gap: f32) -> Option<f32> {
    Some(first? + second? + gap)
}

/// Width and height constraints belong to the panel and follow it when it is
/// moved, grouped or floated.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WorkspacePanelSizing {
    pub width: WorkspaceAxisSizing,
    pub height: WorkspaceAxisSizing,
}

impl WorkspacePanelSizing {
    pub fn new(width: WorkspaceAxisSizing, height: WorkspaceAxisSizing) -> Self {
        Self { width, height }
    }
}

/// Measured tab bar height and gutter. Values live on the container, never in
/// the document.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorkspaceLayoutMetrics {
    pub tab_height: f32,
    pub divider: f32,
    pub shows_collapsed_rails: bool,
}

impl Default for WorkspaceLayoutMetrics {
    fn default() -> Self {
        Self {
            tab_height: 34.0,
            divider: 1.0,
            shows_collapsed_rails: true,
        }
    }
}

impl WorkspaceLayoutMetrics {
    pub fn new(theme: &super::theme::WorkspaceTheme, scaler: f32) -> Self {
        Self {
            tab_height: (scaler * theme.tab_bar_height).max(26.0),
            divider: theme.gutter.max(1.0),
            shows_collapsed_rails: theme.shows_collapsed_rails,
        }
    }
}

/// Linear in the layout tree. Every panel in a tab group participates so
/// changing the active tab never jumps a divider or loses a hidden panel's
/// size contract.
pub struct WorkspaceSizingResolver<'a> {
    pub document: &'a WorkspaceDocument,
    pub metrics: WorkspaceLayoutMetrics,
}

impl<'a> WorkspaceSizingResolver<'a> {
    pub fn new(document: &'a WorkspaceDocument) -> Self {
        Self {
            document,
            metrics: WorkspaceLayoutMetrics::default(),
        }
    }

    pub fn with_metrics(document: &'a WorkspaceDocument, metrics: WorkspaceLayoutMetrics) -> Self {
        Self { document, metrics }
    }

    pub fn group(
        &self,
        group_id: &WorkspaceTabGroupID,
        axis: WorkspaceSplitAxis,
    ) -> WorkspaceAxisSizing {
        let Some(group) = self.document.groups.get(group_id) else {
            return WorkspaceAxisSizing::default();
        };
        let sizes: Vec<&WorkspaceAxisSizing> = group
            .panels
            .iter()
            .filter_map(|id| self.document.panels.get(id))
            .map(|panel| match axis {
                WorkspaceSplitAxis::Horizontal => &panel.sizing.width,
                WorkspaceSplitAxis::Vertical => &panel.sizing.height,
            })
            .collect();
        let chrome = if axis == WorkspaceSplitAxis::Vertical {
            self.metrics.tab_height
        } else {
            0.0
        };
        let minimum = sizes
            .iter()
            .map(|size| size.minimum)
            .fold(0.0_f32, f32::max)
            + chrome;
        let preferred = sizes
            .iter()
            .filter_map(|size| size.preferred)
            .reduce(f32::max)
            .map(|value| value + chrome);
        let maximum = sizes
            .iter()
            .filter_map(|size| size.maximum)
            .reduce(f32::min)
            .map(|value| value + chrome);
        WorkspaceAxisSizing::new(minimum, preferred, maximum)
    }

    pub fn slot(&self, slot_id: &WorkspaceSlotID, axis: WorkspaceSplitAxis) -> WorkspaceAxisSizing {
        self.document
            .slot(slot_id)
            .layout
            .as_ref()
            .and_then(|layout| self.visible_layout(layout, axis))
            .unwrap_or_default()
    }

    fn visible_layout(
        &self,
        node: &WorkspaceLayoutNode,
        axis: WorkspaceSplitAxis,
    ) -> Option<WorkspaceAxisSizing> {
        match node {
            WorkspaceLayoutNode::Group(id) => {
                let group = self.document.groups.get(id)?;
                (!group.is_collapsed && !group.panels.is_empty()).then(|| self.group(id, axis))
            }
            WorkspaceLayoutNode::Split {
                axis: split_axis,
                first,
                second,
                ..
            } => {
                match (
                    self.visible_layout(first, axis),
                    self.visible_layout(second, axis),
                ) {
                    (Some(first), Some(second)) => {
                        Some(self.combine(first, second, *split_axis == axis, self.metrics.divider))
                    }
                    (Some(value), None) | (None, Some(value)) => Some(value),
                    (None, None) => None,
                }
            }
        }
    }

    pub fn layout(
        &self,
        node: &WorkspaceLayoutNode,
        axis: WorkspaceSplitAxis,
    ) -> WorkspaceAxisSizing {
        match node {
            WorkspaceLayoutNode::Group(id) => self.group(id, axis),
            WorkspaceLayoutNode::Split {
                axis: split_axis,
                first,
                second,
                ..
            } => self.combine(
                self.layout(first, axis),
                self.layout(second, axis),
                *split_axis == axis,
                self.metrics.divider,
            ),
        }
    }

    pub fn combine(
        &self,
        first: WorkspaceAxisSizing,
        second: WorkspaceAxisSizing,
        stacked: bool,
        gap: f32,
    ) -> WorkspaceAxisSizing {
        if stacked {
            WorkspaceAxisSizing::combine_stacked(first, second, gap)
        } else {
            WorkspaceAxisSizing::combine_sibling(first, second)
        }
    }

    /// Clamp a proposed floating frame to the group's own size contract.
    pub fn floating_frame(
        &self,
        proposed: WorkspaceRect,
        group_id: &WorkspaceTabGroupID,
        title_height: f32,
    ) -> WorkspaceRect {
        let width = self.group(group_id, WorkspaceSplitAxis::Horizontal);
        let height = self.group(group_id, WorkspaceSplitAxis::Vertical);
        let mut result = proposed;
        result.width = width
            .minimum
            .max(
                width
                    .maximum
                    .unwrap_or(f32::INFINITY)
                    .min(if proposed.width.is_finite() {
                        proposed.width
                    } else {
                        width.preferred.unwrap_or(520.0)
                    }),
            );
        let height_proposal = if proposed.height.is_finite() {
            proposed.height
        } else {
            height.preferred.unwrap_or(390.0) + title_height
        };
        result.height = (height.minimum + title_height).max(
            (height.maximum.unwrap_or(f32::INFINITY - title_height) + title_height)
                .min(height_proposal),
        );
        result
    }
}

/// Split constraints, clamped into the range the two sides actually allow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplitConstraints {
    pub fraction: (f32, f32),
    pub minimum_first: f32,
    pub minimum_second: f32,
    pub maximum_first: Option<f32>,
    pub maximum_second: Option<f32>,
}

impl Default for SplitConstraints {
    fn default() -> Self {
        Self {
            fraction: (0.0, 1.0),
            minimum_first: 0.0,
            minimum_second: 0.0,
            maximum_first: None,
            maximum_second: None,
        }
    }
}

impl SplitConstraints {
    pub fn range(&self, available: f32) -> (f32, f32) {
        let (low, high) = self.fraction;
        if available <= 0.0 {
            return (low, high);
        }
        let mut lower = low;
        let mut upper = high;
        if self.minimum_first > 0.0 {
            lower = lower.max(self.minimum_first / available);
        }
        if let Some(maximum) = self.maximum_first {
            upper = upper.min(maximum / available);
        }
        if self.minimum_second > 0.0 {
            upper = upper.min(1.0 - self.minimum_second / available);
        }
        if let Some(maximum) = self.maximum_second {
            lower = lower.max(1.0 - maximum / available);
        }
        if !lower.is_finite() {
            lower = low;
        }
        if !upper.is_finite() {
            upper = high;
        }
        (lower.min(upper), upper.max(lower))
    }
}

/// A resolved split: the fraction to use and the range it may travel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorkspaceResolvedSplit {
    pub fraction: f32,
    pub range: (f32, f32),
    pub available: f32,
}

impl WorkspaceResolvedSplit {
    pub fn new(
        fraction: Option<f32>,
        fallback: f32,
        available: f32,
        first: WorkspaceAxisSizing,
        second: WorkspaceAxisSizing,
    ) -> Self {
        let available = available.max(0.0);
        let constraints = SplitConstraints {
            fraction: (0.0, 1.0),
            minimum_first: first.minimum,
            minimum_second: second.minimum,
            maximum_first: first.maximum,
            maximum_second: second.maximum,
        };
        let range = constraints.range(available);
        let initial = match fraction {
            Some(value) => value,
            None if available > 0.0 && first.preferred.is_some() => {
                first.preferred.unwrap_or(0.0) / available
            }
            None if available > 0.0 && second.preferred.is_some() => {
                1.0 - second.preferred.unwrap_or(0.0) / available
            }
            None => fallback,
        };
        let initial = if initial.is_finite() {
            initial
        } else {
            fallback
        };
        Self {
            fraction: range.0.max(range.1.min(initial)),
            range,
            available,
        }
    }
}

/// Frame of a floating window. Minimum sizes keep an empty window interactive.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Default for WorkspaceRect {
    fn default() -> Self {
        Self::new(120.0, 80.0, 520.0, 420.0)
    }
}

impl WorkspaceRect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width: width.max(160.0),
            height: height.max(120.0),
        }
    }
}

/// Split fractions. `None` preserves the panels' preferred initial size; a user
/// resize writes an explicit fraction that survives window changes and
/// document round trips.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceSplitFractions {
    pub leading: Option<f32>,
    pub center_trailing: Option<f32>,
    pub top_bottom: Option<f32>,
    pub top: Option<f32>,
}

impl WorkspaceSplitFractions {
    pub fn new(
        leading: Option<f32>,
        center_trailing: Option<f32>,
        top_bottom: Option<f32>,
        top: Option<f32>,
    ) -> Self {
        Self {
            leading: leading.map(Self::clamp),
            center_trailing: center_trailing.map(Self::clamp),
            top_bottom: top_bottom.map(Self::clamp),
            top: top.map(Self::clamp),
        }
    }
    pub fn clamp(value: f32) -> f32 {
        if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            0.5
        }
    }
}

/// Deserialising a normalized sizing value must not bypass the constructor,
/// so the derived `Deserialize` is replaced by one that re-normalizes.
pub(crate) fn deserialize_axis_sizing<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<WorkspaceAxisSizing, D::Error> {
    #[derive(Deserialize)]
    struct Raw {
        #[serde(default)]
        minimum: f32,
        #[serde(default)]
        preferred: Option<f32>,
        #[serde(default)]
        maximum: Option<f32>,
    }
    let raw = Raw::deserialize(deserializer)?;
    Ok(WorkspaceAxisSizing::new(
        raw.minimum,
        raw.preferred,
        raw.maximum,
    ))
}
