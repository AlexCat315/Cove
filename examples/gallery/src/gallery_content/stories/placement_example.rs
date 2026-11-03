//! Overlay story: the popover placement playground.
//!
//! Swift source: `GalleryContent/Stories/Overlays/Popconfirm/PlacementExample.swift`.
//!
//! Twelve named placements plus a compact fallback that mirrors the trigger to
//! the side the placement asks for. The mirror math is ordinary arithmetic with
//! three branches that are easy to transpose, so it lives here as a function.

use cove_composition::Scope;
use cove_core::State;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::overlays::{Popover, PopoverPlacement, PopoverTriggerDescription};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::SemanticColorRef;
use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView, View};

use super::feedback::StorySection;
use crate::gallery_content::catalog::GalleryOptions;

/// The width at or above which the story shows the full grid instead of the
/// compact selector.
pub const GRID_MINIMUM_WIDTH: f32 = 340.0;
/// The compact preview's trigger width floor.
pub const COMPACT_TRIGGER_FLOOR: f32 = 80.0;
/// The compact preview's unscaled trigger width.
pub const COMPACT_TRIGGER_BASIS: f32 = 70.0;

/// One placement variant, with its short label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlacementVariant {
    pub label: &'static str,
    pub placement: PopoverPlacement,
}

/// The twelve variants in the authored order.
///
/// The order matters: the grid lays them out by index, and the compact mirror
/// classifies them by index ranges.
pub const VARIANTS: [PlacementVariant; 12] = [
    PlacementVariant {
        label: "TL",
        placement: PopoverPlacement::TopLeading,
    },
    PlacementVariant {
        label: "Top",
        placement: PopoverPlacement::Top,
    },
    PlacementVariant {
        label: "TR",
        placement: PopoverPlacement::TopTrailing,
    },
    PlacementVariant {
        label: "LT",
        placement: PopoverPlacement::LeadingTop,
    },
    PlacementVariant {
        label: "Left",
        placement: PopoverPlacement::Leading,
    },
    PlacementVariant {
        label: "LB",
        placement: PopoverPlacement::LeadingBottom,
    },
    PlacementVariant {
        label: "RT",
        placement: PopoverPlacement::TrailingTop,
    },
    PlacementVariant {
        label: "Right",
        placement: PopoverPlacement::Trailing,
    },
    PlacementVariant {
        label: "RB",
        placement: PopoverPlacement::TrailingBottom,
    },
    PlacementVariant {
        label: "BL",
        placement: PopoverPlacement::BottomLeading,
    },
    PlacementVariant {
        label: "Bottom",
        placement: PopoverPlacement::Bottom,
    },
    PlacementVariant {
        label: "BR",
        placement: PopoverPlacement::BottomTrailing,
    },
];

/// Where the compact preview puts its trigger.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompactAnchor {
    pub x: f32,
    pub y: f32,
    pub justify: Justify,
}

/// The compact mirror for `placement`.
///
/// A leading placement parks the trigger at the right edge so the bubble has room
/// on the left, a trailing placement parks it at the left edge, and a centred
/// placement splits the difference. Top placements sit low, side placements
/// vertically centre, bottom placements sit at the top.
pub fn compact_anchor(
    placement: PopoverPlacement,
    width: f32,
    trigger_width: f32,
) -> CompactAnchor {
    use PopoverPlacement::*;
    let (justify, x) = match placement {
        LeadingTop | Leading | LeadingBottom => {
            (Justify::FlexEnd, (width - trigger_width).max(0.0))
        }
        TrailingTop | Trailing | TrailingBottom => (Justify::FlexStart, 0.0),
        _ => (Justify::Center, ((width - trigger_width) / 2.0).max(0.0)),
    };
    let y = match placement {
        TopLeading | Top | TopTrailing => 130.0,
        LeadingTop | Leading | LeadingBottom | TrailingTop | Trailing | TrailingBottom => 55.0,
        _ => 0.0,
    };
    CompactAnchor { x, y, justify }
}

/// Whether the full grid fits at `width`.
pub fn shows_full_grid(width: f32) -> bool {
    width >= GRID_MINIMUM_WIDTH
}

/// The compact trigger width for a viewport width and text scale.
///
/// Capped at the viewport so a narrow window still shows a usable trigger.
pub fn compact_trigger_width(width: f32, scale: f32) -> f32 {
    width
        .min(COMPACT_TRIGGER_BASIS.max(scale.max(0.0) * COMPACT_TRIGGER_BASIS))
        .max(COMPACT_TRIGGER_FLOOR.min(width))
}

/// `PopoverPlacementExample`: every named side, with a compact fallback.
pub struct PlacementExample {
    pub options: GalleryOptions,
    selected: State<usize>,
    last_action: State<String>,
    /// One presentation flag per variant, so each trigger owns its popover.
    is_presented: State<Vec<bool>>,
}

impl PlacementExample {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            selected: State::new(1),
            last_action: State::new(String::new()),
            is_presented: State::new(vec![false; VARIANTS.len()]),
        }
    }

    pub fn selected(&self) -> usize {
        self.selected.get()
    }

    pub fn set_selected(&self, index: usize) {
        self.selected.set(index.min(VARIANTS.len() - 1));
    }

    /// The variant currently mirrored in the compact layout.
    pub fn selected_variant(&self) -> PlacementVariant {
        VARIANTS[self.selected.get().min(VARIANTS.len() - 1)]
    }

    pub fn last_action(&self) -> String {
        self.last_action.get()
    }

    pub fn record(&self, label: impl Into<String>) {
        self.last_action.set(label.into());
    }

    /// Every placement, in the order the grid shows them.
    pub fn placements(&self) -> Vec<PopoverPlacement> {
        VARIANTS.iter().map(|variant| variant.placement).collect()
    }
}

impl PlacementExample {
    /// The presentation binding of the variant at `index`.
    fn presentation(&self, index: usize) -> cove_core::Binding<bool> {
        let read = self.is_presented.clone();
        let write = self.is_presented.clone();
        cove_core::Binding::new(
            move || read.get().get(index).copied().unwrap_or(false),
            move |value: bool| {
                write.update(|states| {
                    if let Some(slot) = states.get_mut(index) {
                        *slot = value;
                    }
                });
            },
        )
    }
}

impl View for PlacementExample {
    fn body(&self, _cx: &Scope) -> AnyView {
        let enabled = self.options.is_enabled;
        let title = Text::new("Placement playground")
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::body_strong());
        let detail = Text::new(
            "Named sides stay on that side. The card and pointer are shared with Popover.",
        )
        .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
        .semantic_foreground(SemanticColorRef::on_surface_muted());

        let record = self.last_action.clone();
        let grid = VARIANTS
            .iter()
            .enumerate()
            .map(|(index, variant)| {
                let record = record.clone();
                let content = AnyView::new(BoxView::new(
                    FlexDirection::Column,
                    Align::FlexStart,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    4.0,
                    AnyView::new(TupleView::new(vec![
                        AnyView::new(Text::new(format!("{:?}", variant.placement))),
                        AnyView::new(
                            Text::new("The bubble keeps this side of the trigger.")
                                .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
                                .semantic_foreground(SemanticColorRef::on_surface_muted()),
                        ),
                        AnyView::new(Button::title("Move to Projects", move || {
                            record.set("Move to Projects".to_owned())
                        })),
                    ])),
                ));
                AnyView::new(
                    Popover::new(Text::new(variant.label), content)
                        .is_presented(self.presentation(index))
                        .placement(variant.placement)
                        .width(240.0)
                        .trigger_description(PopoverTriggerDescription {
                            label: format!("{:?} placement", variant.placement),
                            value: String::new(),
                            help: None,
                        })
                        .enabled(enabled)
                        .debug_name("placement-popover"),
                )
            })
            .collect();

        let last = Text::new(format!("Last action: {}", self.last_action()))
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted());

        AnyView::new(StorySection::new(
            "Placement playground",
            "Named sides stay on that side. The card and pointer are shared with Popover.",
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                10.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(title),
                    AnyView::new(detail),
                    AnyView::new(BoxView::new(
                        FlexDirection::Row,
                        Align::Center,
                        Justify::FlexStart,
                        Wrap::Wrap,
                        8.0,
                        AnyView::new(TupleView::new(grid)),
                    )),
                    AnyView::new(last),
                ])),
            )),
        ))
    }

    fn tag(&self) -> &'static str {
        "PopoverPlacementExample"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; selected, last_action, is_presented)
    }
}
