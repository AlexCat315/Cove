//! Translation of `Primitives/Data/Chart/Axis.swift`.
//!
//! Where the plot sits inside a chart and the tick labels around it. The chart
//! has to know label widths before it can size the plot, so this resolves from
//! the data-derived scale: domain -> ticks -> label widths -> plot.
//!
//! `TextEnvironment` is not available in Rust yet; measurements fall back to
//! the caption glyph approximation the Swift headless path uses, and labels are
//! carried for a future text host.
use cove_core::scene::{Point, Rect, Size};

use crate::theme::{TextStyleToken, Theme};

use super::axes::ChartValueAxes;
use super::coordinates::ChartGeometry;
use super::model::{Axis as AxisConfig, ChartOrientation, ChartRange, ChartStyle, ChartXDomain};
use super::scale::ChartScale;
use super::viewport::ChartWindow;

#[derive(Clone)]
pub struct ChartText {
    pub token: TextStyleToken,
}
impl ChartText {
    pub fn new(theme: &Theme) -> Self {
        Self {
            token: theme.typography.caption.clone(),
        }
    }

    pub fn size(&self, text: &str) -> Size {
        if text.is_empty() {
            return Size {
                width: 0.0,
                height: 0.0,
            };
        }
        Size {
            width: text.chars().count() as f32 * self.token.font.size() * 0.55,
            height: self.token.line_height,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ChartTick {
    /// Screen coordinate along the axis this tick belongs to.
    pub position: f32,
    pub label: String,
}

#[derive(Clone)]
pub struct ChartFrame {
    /// Chart box after `contentInset`, before axis gutters.
    pub bounds: Rect,
    pub plot: Rect,
    pub value_ticks: Vec<ChartTick>,
    pub secondary_ticks: Vec<ChartTick>,
    pub category_ticks: Vec<ChartTick>,
    pub value_title: String,
    pub secondary_title: String,
    pub category_title: String,
    pub orientation: ChartOrientation,
}
impl ChartFrame {
    /// Space between a tick label and the plot edge.
    pub const LABEL_GAP: f32 = 6.0;

    #[allow(clippy::too_many_arguments)]
    pub fn cartesian(
        axes: &ChartValueAxes,
        x_domain: &ChartXDomain,
        x_scale: Option<&ChartScale>,
        style: &ChartStyle,
        axis: &AxisConfig,
        size: Size,
        origin: Point,
        text: &ChartText,
        window: Option<&ChartWindow>,
    ) -> Self {
        let bounds = Self::inset(size, origin, style);
        let orientation = style.orientation;
        let value_labels = Self::labels(
            &axes.primary_scale,
            axis.y.is_labeled,
            axis.y.format.as_deref(),
            axes.primary.is_percent,
        );
        let has_secondary = axes.uses_secondary() && axis.secondary.is_labeled;
        let secondary_labels = if has_secondary {
            let scale = axes.scale(super::model::ChartValueAxis::Right);
            Self::labels(
                scale,
                true,
                axis.secondary.format.as_deref(),
                axes.secondary
                    .as_ref()
                    .map(|stack| stack.is_percent)
                    .unwrap_or(false),
            )
        } else {
            Vec::new()
        };
        let category_texts = if axis.x.is_labeled {
            Self::category_texts(x_domain, axes.sample_count, x_scale, axis)
        } else {
            Vec::new()
        };
        let has_categories = !category_texts.is_empty();

        let line_height = text.size("0").height;
        let value_width = value_labels
            .iter()
            .map(|label| text.size(label).width)
            .reduce(f32::max)
            .unwrap_or(0.0);
        let secondary_width = secondary_labels
            .iter()
            .map(|label| text.size(label).width)
            .reduce(f32::max)
            .unwrap_or(0.0);
        let category_width = category_texts
            .iter()
            .map(|label| text.size(label).width)
            .reduce(f32::max)
            .unwrap_or(0.0);
        let value_title_height = if axis.y.title.is_empty() {
            0.0
        } else {
            text.size(&axis.y.title).height
        };
        let secondary_title_height = if has_secondary && !axis.secondary.title.is_empty() {
            text.size(&axis.secondary.title).height
        } else {
            0.0
        };
        let category_title_height = if has_categories && !axis.x.title.is_empty() {
            text.size(&axis.x.title).height
        } else {
            0.0
        };

        let mut left = 0.0_f32;
        let mut right = 0.0_f32;
        let mut top = 0.0_f32;
        let mut bottom = 0.0_f32;
        match orientation {
            ChartOrientation::Vertical => {
                left = if axis.y.is_labeled {
                    value_width + Self::LABEL_GAP
                } else {
                    0.0
                };
                right = if has_secondary {
                    secondary_width + Self::LABEL_GAP
                } else {
                    0.0
                };
                let pad = if axis.y.is_labeled || has_secondary {
                    line_height / 2.0
                } else {
                    0.0
                };
                let title = value_title_height.max(secondary_title_height);
                top = if title > 0.0 { title + pad + 2.0 } else { pad };
                bottom = (if has_categories {
                    line_height + 4.0
                } else {
                    0.0
                }) + if category_title_height > 0.0 {
                    category_title_height + 2.0
                } else {
                    0.0
                };
            }
            ChartOrientation::Horizontal => {
                left = if has_categories {
                    category_width + Self::LABEL_GAP
                } else {
                    0.0
                };
                bottom = (if axis.y.is_labeled {
                    line_height + 4.0
                } else {
                    0.0
                }) + if value_title_height > 0.0 {
                    value_title_height + 2.0
                } else {
                    0.0
                } + if axis.y.is_labeled {
                    line_height / 2.0
                } else {
                    0.0
                };
                let title = category_title_height.max(secondary_title_height);
                top = (if has_secondary {
                    line_height / 2.0 + line_height + Self::LABEL_GAP
                } else {
                    0.0
                }) + if title > 0.0 { title + 2.0 } else { 0.0 };
            }
        }
        let left_inset = left.min(bounds.width * 0.4);
        let right_inset = right.min(bounds.width * 0.3);
        let top_inset = top.min(bounds.height * 0.35);
        let bottom_inset = bottom.min(bounds.height * 0.4);
        let plot = Rect::new(
            bounds.x + left_inset,
            bounds.y + top_inset,
            (bounds.width - left_inset - right_inset).max(0.0),
            (bounds.height - top_inset - bottom_inset).max(0.0),
        );

        let value_ticks = axes
            .primary_scale
            .ticks
            .iter()
            .zip(value_labels)
            .map(|(value, label)| ChartTick {
                position: orientation.along(&axes.primary_scale, *value, plot),
                label,
            })
            .collect();
        let secondary_ticks = axes
            .secondary_scale
            .as_ref()
            .map(|scale| {
                scale
                    .ticks
                    .iter()
                    .zip(secondary_labels)
                    .map(|(value, label)| ChartTick {
                        position: orientation.along(scale, *value, plot),
                        label,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let category_ticks = if has_categories {
            Self::category_ticks(
                &category_texts,
                x_domain,
                x_scale,
                axes.sample_count,
                axes.has_bars,
                plot,
                orientation,
                text,
                window,
            )
        } else {
            Vec::new()
        };
        Self {
            bounds,
            plot,
            value_ticks,
            secondary_ticks,
            category_ticks,
            value_title: axis.y.title.clone(),
            secondary_title: if has_secondary {
                axis.secondary.title.clone()
            } else {
                String::new()
            },
            category_title: if has_categories {
                axis.x.title.clone()
            } else {
                String::new()
            },
            orientation,
        }
    }

    fn category_texts(
        domain: &ChartXDomain,
        count: usize,
        scale: Option<&ChartScale>,
        axis: &AxisConfig,
    ) -> Vec<String> {
        match domain {
            ChartXDomain::Categorical(labels) => labels.iter().take(count).cloned().collect(),
            ChartXDomain::Numeric => {
                let Some(scale) = scale else {
                    return Vec::new();
                };
                scale
                    .ticks
                    .iter()
                    .map(|value| {
                        axis.x
                            .format
                            .as_ref()
                            .map(|format| format(*value))
                            .unwrap_or_else(|| scale.label(*value))
                    })
                    .collect()
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn category_ticks(
        texts: &[String],
        x_domain: &ChartXDomain,
        x_scale: Option<&ChartScale>,
        count: usize,
        has_bars: bool,
        plot: Rect,
        orientation: ChartOrientation,
        text: &ChartText,
        window: Option<&ChartWindow>,
    ) -> Vec<ChartTick> {
        if texts.is_empty() {
            return Vec::new();
        }
        if x_domain.is_numeric()
            && let Some(scale) = x_scale
        {
            return texts
                .iter()
                .zip(&scale.ticks)
                .map(|(label, value)| ChartTick {
                    position: orientation.across(scale, *value, plot),
                    label: label.clone(),
                })
                .collect();
        }
        let span = window
            .map(|window| window.range.span())
            .unwrap_or(count.max(1) as f32);
        let step = Self::label_step(
            texts,
            orientation.breadth(plot) / span.max(1.0),
            orientation,
            text,
        );
        let origin = orientation.across_origin(plot);
        let end = origin + orientation.breadth(plot);
        let mut ticks = Vec::new();
        let mut index = 0;
        while index < texts.len() {
            let position = window
                .map(|window| window.position(index as f32, plot, orientation))
                .unwrap_or_else(|| {
                    ChartGeometry::across_index(index, count, has_bars, plot, orientation)
                });
            if position >= origin && position <= end {
                ticks.push(ChartTick {
                    position,
                    label: texts[index].clone(),
                });
            }
            index += step.max(1);
        }
        ticks
    }

    /// A percent axis reads as shares even though plotted values are normalised.
    fn labels(
        scale: &ChartScale,
        labeled: bool,
        format: Option<&(dyn Fn(f32) -> String + Send + Sync)>,
        percent: bool,
    ) -> Vec<String> {
        if !labeled {
            return scale.ticks.iter().map(|_| String::new()).collect();
        }
        scale
            .ticks
            .iter()
            .map(|value| {
                if let Some(format) = format {
                    return format(*value);
                }
                let text = scale.label(*value);
                if percent { format!("{text}%") } else { text }
            })
            .collect()
    }

    /// Pie/donut has no axes: the plot fills the inset box.
    pub fn pie(style: &ChartStyle, size: Size, origin: Point) -> Self {
        let bounds = Self::inset(size, origin, style);
        Self {
            bounds,
            plot: bounds,
            value_ticks: Vec::new(),
            secondary_ticks: Vec::new(),
            category_ticks: Vec::new(),
            value_title: String::new(),
            secondary_title: String::new(),
            category_title: String::new(),
            orientation: style.orientation,
        }
    }

    fn label_step(
        texts: &[String],
        slot: f32,
        orientation: ChartOrientation,
        text: &ChartText,
    ) -> usize {
        if slot <= 0.0 {
            return texts.len().max(1);
        }
        let needed = if orientation == ChartOrientation::Vertical {
            texts
                .iter()
                .map(|label| text.size(label).width)
                .reduce(f32::max)
                .unwrap_or(0.0)
                + 8.0
        } else {
            text.size("0").height + 2.0
        };
        (needed / slot).ceil().max(1.0) as usize
    }

    fn inset(size: Size, origin: Point, style: &ChartStyle) -> Rect {
        let inset = style.content_inset.min(size.width.min(size.height) * 0.35);
        Rect::new(
            origin.x + inset,
            origin.y + inset,
            (size.width - inset * 2.0).max(0.0),
            (size.height - inset * 2.0).max(0.0),
        )
    }

    /// Ratio window helper retained for overlays.
    pub fn visible_range(&self) -> ChartRange {
        ChartRange::new(0.0, 1.0)
    }
}
