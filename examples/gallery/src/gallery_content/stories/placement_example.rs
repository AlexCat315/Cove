//! Overlay story: the popover placement playground.
//!
//! Swift source: `GalleryContent/Stories/Overlays/Popconfirm/PlacementExample.swift`.
//!
//! Twelve named placements plus a compact fallback that mirrors the trigger to
//! the side the placement asks for. The mirror math is ordinary arithmetic with
//! three branches that are easy to transpose, so it lives here as a function.

use std::rc::Rc;

use cove_composition::Scope;
use cove_core::State;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_ui_compose::adaptive::ContainerSizeReader;
use cove_ui_compose::modifiers::layout::{
    AbsolutePositionModifier, FrameDimension, LayoutFrameExt, PaddingExt,
};
use cove_ui_compose::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use cove_ui_compose::primitives::controls::ButtonRole;
use cove_ui_compose::primitives::controls::fields::select::{Select, SelectOption};
use cove_ui_compose::primitives::overlays::{PopoverPlacement, Popconfirm};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::{SemanticColorRef, TextScaler, TextScalerReader};
use cove_ui_compose::theme::{Theme, ThemeReader};
use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView, View, ViewExt};

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
///
/// The owning page receives each confirmed placement through `on_action`, so
/// one "Last action" readout covers the whole Popconfirm page.
pub struct PlacementExample {
    pub options: GalleryOptions,
    selected: State<usize>,
    on_action: Rc<dyn Fn(String)>,
}

impl PlacementExample {
    pub fn new(options: GalleryOptions, on_action: impl Fn(String) + 'static) -> Self {
        Self {
            options,
            selected: State::new(1),
            on_action: Rc::new(on_action),
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
}

/// One preview trigger: a real `Popconfirm` bubble on the requested side.
fn preview(
    variant: PlacementVariant,
    is_enabled: bool,
    on_action: Rc<dyn Fn(String)>,
) -> AnyView {
    AnyView::new(Popconfirm::new(
        variant.label,
        None,
        format!("{} placement", variant.label),
        "The bubble stays on the requested side of its trigger.",
        "OK",
        "Cancel",
        ButtonRole::Normal,
        variant.placement,
        is_enabled,
        move || on_action(variant.label.to_owned()),
    ))
}

fn side_pair(left: usize, right: usize, is_enabled: bool, on_action: Rc<dyn Fn(String)>) -> AnyView {
    AnyView::new(BoxView::new(
        FlexDirection::Row,
        Align::Center,
        Justify::SpaceBetween,
        Wrap::NoWrap,
        8.0,
        AnyView::new(TupleView::new(vec![
            preview(VARIANTS[left], is_enabled, on_action.clone()),
            preview(VARIANTS[right], is_enabled, on_action),
        ])),
    ))
}

fn centred_row(indices: &[usize], is_enabled: bool, on_action: Rc<dyn Fn(String)>) -> AnyView {
    AnyView::new(BoxView::new(
        FlexDirection::Row,
        Align::Center,
        Justify::Center,
        Wrap::NoWrap,
        8.0,
        AnyView::new(TupleView::new(
            indices
                .iter()
                .map(|&index| preview(VARIANTS[index], is_enabled, on_action.clone()))
                .collect(),
        )),
    ))
}

/// The full grid: top and bottom rows centred, side pairs pushed to the edges,
/// and the middle row flanking an "Anchor" caption.
fn grid(is_enabled: bool, on_action: Rc<dyn Fn(String)>) -> AnyView {
    let anchor_caption = AnyView::new(
        Text::new("Anchor")
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted()),
    );
    AnyView::new(BoxView::new(
        FlexDirection::Column,
        Align::Stretch,
        Justify::FlexStart,
        Wrap::NoWrap,
        8.0,
        AnyView::new(TupleView::new(vec![
            centred_row(&[0, 1, 2], is_enabled, on_action.clone()),
            side_pair(3, 6, is_enabled, on_action.clone()),
            AnyView::new(BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::SpaceBetween,
                Wrap::NoWrap,
                8.0,
                AnyView::new(TupleView::new(vec![
                    preview(VARIANTS[4], is_enabled, on_action.clone()),
                    anchor_caption,
                    preview(VARIANTS[7], is_enabled, on_action.clone()),
                ])),
            )),
            side_pair(5, 8, is_enabled, on_action.clone()),
            centred_row(&[9, 10, 11], is_enabled, on_action),
        ])),
    ))
}

/// The compact fallback: a selector plus one mirrored trigger.
fn compact(
    selected: State<usize>,
    is_enabled: bool,
    on_action: Rc<dyn Fn(String)>,
    width: f32,
    scaler: &TextScaler,
) -> AnyView {
    let index = selected.get().min(VARIANTS.len() - 1);
    let trigger_width = compact_trigger_width(width, scaler.factor());
    let anchor = compact_anchor(VARIANTS[index].placement, width, trigger_width);
    let options: Vec<SelectOption<usize>> = VARIANTS
        .iter()
        .enumerate()
        .map(|(index, variant)| SelectOption::new(index, variant.label, true))
        .collect();
    let mirrored = AnyView::new(
        BoxView::new(
            FlexDirection::Row,
            Align::Center,
            anchor.justify,
            Wrap::NoWrap,
            0.0,
            preview(VARIANTS[index], is_enabled, on_action),
        )
        .frame(
            Some(FrameDimension::Points(trigger_width)),
            None,
            None,
            None,
            None,
            None,
        )
        .modifier(AbsolutePositionModifier::new(
            Some(anchor.x),
            Some(anchor.y),
            None,
            None,
        )),
    );
    AnyView::new(BoxView::new(
        FlexDirection::Column,
        Align::Stretch,
        Justify::FlexStart,
        Wrap::NoWrap,
        12.0,
        AnyView::new(TupleView::new(vec![
            AnyView::new(Select::new(selected.binding(), options).enabled(is_enabled)),
            AnyView::new(
                BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    mirrored,
                )
                .frame(
                    None,
                    Some(FrameDimension::Points(180.0)),
                    None,
                    None,
                    None,
                    None,
                ),
            ),
        ])),
    ))
}

impl View for PlacementExample {
    fn body(&self, _cx: &Scope) -> AnyView {
        let is_enabled = self.options.is_enabled;
        let selected = self.selected.clone();
        let on_action = self.on_action.clone();
        AnyView::new(ThemeReader::new(move |theme: &Theme| {
            let selected = selected.clone();
            let on_action = on_action.clone();
            let region = AnyView::new(TextScalerReader::new(move |scaler| {
                let selected = selected.clone();
                let on_action = on_action.clone();
                let height = 240.0_f32.max(scaler.scale(32.0) * 5.0 + 32.0);
                AnyView::new(
                    AnyView::new(ContainerSizeReader::new(move |size| {
                        if shows_full_grid(size.width) {
                            grid(is_enabled, on_action.clone())
                        } else {
                            compact(
                                selected.clone(),
                                is_enabled,
                                on_action.clone(),
                                size.width,
                                &scaler,
                            )
                        }
                    }))
                    .frame(
                        None,
                        Some(FrameDimension::Points(height)),
                        None,
                        None,
                        None,
                        None,
                    )
                    .debug_name("popover-placement-preview"),
                )
            }));
            AnyView::new(BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::Center,
                Wrap::NoWrap,
                0.0,
                AnyView::new(
                    BoxView::new(
                        FlexDirection::Column,
                        Align::Stretch,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        10.0,
                        AnyView::new(TupleView::new(vec![
                            AnyView::new(Text::new("Placement playground").semantic_font(
                                cove_ui_compose::theme::SemanticFontRef::body_strong(),
                            )),
                            AnyView::new(
                                Text::new(
                                    "Named sides stay on that side. The card and pointer are shared with Popover.",
                                )
                                .semantic_font(
                                    cove_ui_compose::theme::SemanticFontRef::caption(),
                                )
                                .semantic_foreground(SemanticColorRef::on_surface_muted()),
                            ),
                            region,
                        ])),
                    )
                    .padding(14.0)
                    .background_color(theme.colors.surface)
                    .corner_radius(theme.radius.md)
                    .border_color(theme.colors.border, 1.0)
                    .frame(
                        Some(FrameDimension::Percent(100.0)),
                        None,
                        None,
                        None,
                        Some(400.0),
                        None,
                    )
                    .debug_name("placement-playground"),
                ),
            ))
        }))
    }

    fn tag(&self) -> &'static str {
        "PopoverPlacementExample"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; selected)
    }
}
