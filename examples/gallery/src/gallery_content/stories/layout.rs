//! Layout stories: masonry flow, measured grids and virtualized grids.
//!
//! Swift sources: `GalleryContent/Stories/Layout/MasonryStory.swift`
//! (`MasonryStory`, `MasonrySample`) and the grid sections of
//! `GalleryContent/Stories/Layout/FoundationExamples.swift`.

use cove_composition::Scope;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_core::{State, TextScale};
use cove_ui_compose::modifiers::layout::{LayoutFrameExt, PaddingExt};
use cove_ui_compose::modifiers::style::AppearanceExt;
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::layout::grid::{Grid, GridColumn, GridRowHeights, LazyGrid};
use cove_ui_compose::primitives::layout::masonry::masonry::Masonry;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::styles::buttons::button_style::ButtonStyleViewExt;
use cove_ui_compose::theme::styles::buttons::{GhostButtonStyle, SecondaryButtonStyle};
use cove_ui_compose::theme::tokens::{SemanticColorRef, SemanticStyleViewExt};
use cove_ui_compose::{Alignment, AnyView, BoxView, DynamicField, LayoutDebugExt, TupleView, View};

use crate::gallery_content::catalog::GalleryOptions;
use crate::gallery_content::stories::StorySection;

/// The eight authored card heights, cycled as cards are added.
pub const MASONRY_HEIGHTS: [f32; 8] = [112.0, 164.0, 128.0, 188.0, 96.0, 144.0, 116.0, 176.0];

/// The card height at `index`, cycling through the authored list.
pub fn masonry_height(index: usize) -> f32 {
    MASONRY_HEIGHTS[index % MASONRY_HEIGHTS.len()]
}

/// `8 + extraCards cards · 1–2 columns`
pub fn masonry_count_line(extra_cards: i32) -> String {
    format!("{} cards · 1–2 columns", 8 + extra_cards)
}

/// `MasonryStory`.
pub struct MasonryStory {
    pub options: GalleryOptions,
    pub extra_cards: State<i32>,
}

impl MasonryStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            extra_cards: State::new(0),
        }
    }

    pub fn add_card(&self) {
        self.extra_cards.update(|cards| *cards += 1);
    }

    pub fn reset(&self) {
        self.extra_cards.set(0);
    }

    pub fn count_line(&self) -> String {
        masonry_count_line(self.extra_cards.get())
    }

    pub fn card_count(&self) -> usize {
        (8 + self.extra_cards.get()).max(0) as usize
    }
}

impl View for MasonryStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let enabled = self.options.is_enabled;
        let add = {
            let extra = self.extra_cards.clone();
            move || extra.update(|cards| *cards += 1)
        };
        let reset = {
            let extra = self.extra_cards.clone();
            move || extra.set(0)
        };

        let controls = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::NoWrap,
            8.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(
                    Button::title("Add card", add)
                        .enabled(enabled)
                        .button_style(SecondaryButtonStyle),
                ),
                AnyView::new(
                    Button::title("Reset", reset)
                        .enabled(enabled)
                        .button_style(GhostButtonStyle),
                ),
                AnyView::new(
                    Text::new(self.count_line())
                        .semantic_foreground(SemanticColorRef::on_surface_muted()),
                ),
            ])),
        ));

        let cards = AnyView::new(Masonry::new(
            220.0,
            2,
            14.0,
            14.0,
            Alignment::TopLeading,
            AnyView::new(MasonryCards {
                count: self.card_count(),
            }),
        ));

        let caption = AnyView::new(
            Text::new("Minimum column width 220pt · maximum 2 columns · 14pt gaps")
                .semantic_foreground(SemanticColorRef::on_surface_muted()),
        );

        AnyView::new(
            StorySection::new(
                "Waterfall columns",
                "Cards flow into the shortest column, so mixed heights pack without row gaps. The column count follows the available width; resize the window or add cards to see the reflow.",
                AnyView::new(BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    12.0,
                    AnyView::new(TupleView::new(vec![controls, cards, caption])),
                )),
            )
            .debug_name("masonry-demo"),
        )
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("extraCards", &self.extra_cards)]
    }
}

/// The generated card list, so `Masonry` receives one structural child list.
struct MasonryCards {
    count: usize,
}

impl View for MasonryCards {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(cove_ui_compose::TupleView::new(
            (0..self.count)
                .map(|index| AnyView::new(MasonryCard { index }))
                .collect(),
        ))
    }
}

/// One masonry card.
struct MasonryCard {
    index: usize,
}

impl View for MasonryCard {
    fn body(&self, _cx: &Scope) -> AnyView {
        let height = masonry_height(self.index);
        // Even cards use `surface`, odd use `surfaceVariant`.
        let fill = if self.index.is_multiple_of(2) {
            SemanticColorRef::surface()
        } else {
            SemanticColorRef::surface_variant()
        };
        let form = if self.index.is_multiple_of(2) {
            "Long form"
        } else {
            "Short form"
        };
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                9.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(
                        Text::new(format!("Masonry card {}", self.index + 1))
                            .semantic_foreground(SemanticColorRef::on_surface()),
                    ),
                    AnyView::new(
                        Text::new(format!(
                            "Mixed-height content packs into the shortest column. This card is {}pt tall.",
                            height as i32
                        ))
                        .semantic_foreground(SemanticColorRef::on_surface_variant()),
                    ),
                    AnyView::new(
                        Text::new(form).semantic_foreground(SemanticColorRef::on_surface_muted()),
                    ),
                ])),
            )
            .padding(14.0)
            .frame_height(height)
            .semantic_background(fill)
            .corner_radius(12.0)
            .semantic_border(SemanticColorRef::border(), 1.0)
            .debug_name(&format!("masonry-card-{}", self.index)),
        )
    }
}

/// `MasonrySample`: the compact two-card variant the adaptive matrix reuses.
pub struct MasonrySample;

impl View for MasonrySample {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(
            Masonry::new(
                220.0,
                2,
                10.0,
                10.0,
                Alignment::TopLeading,
                AnyView::new(cove_ui_compose::TupleView::new(vec![
                    AnyView::new(MasonrySampleCard {
                        index: 1,
                        height: 70.0,
                    }),
                    AnyView::new(MasonrySampleCard {
                        index: 2,
                        height: 100.0,
                    }),
                ])),
            )
            .debug_name("masonry-sample"),
        )
    }
}

struct MasonrySampleCard {
    index: usize,
    height: f32,
}

impl View for MasonrySampleCard {
    fn body(&self, _cx: &Scope) -> AnyView {
        let fill = if self.index.is_multiple_of(2) {
            SemanticColorRef::surface()
        } else {
            SemanticColorRef::surface_variant()
        };
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                6.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(Text::new(format!("Card {}", self.index))),
                    AnyView::new(
                        Text::new("Mixed height")
                            .semantic_foreground(SemanticColorRef::on_surface_muted()),
                    ),
                ])),
            )
            .padding(10.0)
            .frame_height(self.height)
            .semantic_background(fill)
            .corner_radius(10.0)
            .semantic_border(SemanticColorRef::border(), 1.0)
            .debug_name(&format!("masonry-sample-card-{}", self.index)),
        )
    }
}

/// The grid sections of `LayoutFoundationExamples`.
pub struct LayoutGridStory {
    pub options: GalleryOptions,
}

impl LayoutGridStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self { options }
    }
}

impl View for LayoutGridStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let enabled = self.options.is_enabled;
        // Fixed 100pt, then two flexible tracks with a 120pt minimum.
        let measured = AnyView::new(
            Grid::new(
                vec![
                    GridColumn::Fixed(100.0),
                    GridColumn::Flexible {
                        minimum: 120.0,
                        maximum: f32::INFINITY,
                    },
                    GridColumn::Flexible {
                        minimum: 120.0,
                        maximum: f32::INFINITY,
                    },
                ],
                12.0,
                12.0,
                Alignment::TopLeading,
                None,
                enabled,
                AnyView::new(cove_ui_compose::TupleView::new(vec![
                    AnyView::new(GridCell::new("Fixed", "100 points")),
                    AnyView::new(GridCell::new("Flexible", "Natural row height")),
                    AnyView::new(GridCell::new(
                        "Flexible",
                        "Text wraps as the window narrows, including at 200%.",
                    )),
                    AnyView::new(GridCell::new("Row 2", "No fixed row slot")),
                    AnyView::new(GridCell::new(
                        "Content",
                        "Measured by the same engine that draws the text.",
                    )),
                    AnyView::new(GridCell::new("More", "One view per cell")),
                ])),
            )
            .debug_name("layout-measured-grid"),
        );

        // One adaptive track with a 150pt minimum: 1..n columns.
        let adaptive = AnyView::new(
            Grid::new(
                vec![GridColumn::Adaptive {
                    minimum: 150.0,
                    maximum: f32::INFINITY,
                }],
                12.0,
                12.0,
                Alignment::TopLeading,
                None,
                enabled,
                AnyView::new(cove_ui_compose::TupleView::new(
                    (1..=6)
                        .map(|index| {
                            AnyView::new(GridCell::new(
                                format!("Adaptive {index}"),
                                "Resize the window",
                            ))
                        })
                        .collect(),
                )),
            )
            .debug_name("layout-adaptive-grid"),
        );

        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                24.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(StorySection::new(
                        "Measured grid",
                        "Fixed, flexible and adaptive tracks share the public Layout measurement and placement contract. Row heights follow their tallest content.",
                        AnyView::new(BoxView::new(
                            FlexDirection::Column,
                            Align::Stretch,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            12.0,
                            AnyView::new(TupleView::new(vec![measured, adaptive])),
                        )),
                    )),
                ])),
            )
            .debug_name("layout-grid-story"),
        )
    }
}

struct GridCell {
    title: String,
    detail: String,
}

impl GridCell {
    fn new(title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            detail: detail.into(),
        }
    }
}

impl View for GridCell {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                4.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(Text::new(self.title.clone())),
                    AnyView::new(
                        Text::new(self.detail.clone())
                            .semantic_foreground(SemanticColorRef::on_surface_muted()),
                    ),
                ])),
            )
            .padding(12.0)
            .semantic_background(SemanticColorRef::surface_variant())
            .corner_radius(6.0),
        )
    }
}

/// The two virtualized grids from `LayoutFoundationExamples`.
pub struct LazyGridStory;

impl View for LazyGridStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        // 100,000 cells in 4 fixed-height columns.
        let dense = AnyView::new(
            LazyGrid::new(
                (0..100_000u64).collect::<Vec<_>>(),
                std::rc::Rc::new(|index: &u64| format!("dense-{index}")),
                4,
                GridRowHeights::fixed(48.0),
                8.0,
                8.0,
                2,
                None,
                None,
                None,
                std::rc::Rc::new(|_: &u64| AnyView::new(LazyCell::new("Cell"))),
            )
            .frame_height(240.0)
            .debug_name("layout-lazy-grid"),
        );

        // 200 columns x 200 rows, every seventh item spanning two columns.
        let wide = AnyView::new(
            LazyGrid::new(
                (0..40_000u64).collect::<Vec<_>>(),
                std::rc::Rc::new(|index: &u64| format!("wide-{index}")),
                200,
                GridRowHeights::fixed(40.0),
                6.0,
                6.0,
                2,
                None,
                Some(90.0),
                Some(std::rc::Rc::new(|index: &u64| {
                    if index.is_multiple_of(7) { 2 } else { 1 }
                })),
                std::rc::Rc::new(|index: &u64| AnyView::new(LazyCell::new(format!("#{index}")))),
            )
            .frame_height(220.0)
            .debug_name("layout-wide-lazy-grid"),
        );

        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                24.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(StorySection::new(
                        "Lazy grid · 100,000 cells",
                        "Only viewport rows plus overscan are constructed. Authored row heights automatically follow the text scale.",
                        dense,
                    )),
                    AnyView::new(StorySection::new(
                        "Wide lazy grid · 200 × 200",
                        "Fixed-width columns scroll on both axes and only the visible rows and columns are built. Every seventh item spans two columns.",
                        wide,
                    )),
                ])),
            )
            .debug_name("layout-lazy-grid-story"),
        )
    }
}

struct LazyCell {
    label: String,
}

impl LazyCell {
    fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
        }
    }
}

impl View for LazyCell {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(
            Text::new(self.label.clone())
                .padding(8.0)
                .semantic_background(SemanticColorRef::surface_variant())
                .corner_radius(6.0),
        )
    }
}

/// The default text scale, exposed so the row-height callbacks stay honest.
pub fn default_text_scale() -> TextScale {
    TextScale::default()
}

#[cfg(test)]
mod checks {
    use super::*;

    #[test]
    fn heights_cycle_through_the_authored_list() {
        assert_eq!(masonry_height(0), 112.0);
        assert_eq!(masonry_height(3), 188.0);
        assert_eq!(masonry_height(7), 176.0);
        // Index 8 wraps to the first entry.
        assert_eq!(masonry_height(8), 112.0);
        assert_eq!(masonry_height(15), 176.0);
    }

    #[test]
    fn count_line_tracks_added_cards() {
        assert_eq!(masonry_count_line(0), "8 cards · 1–2 columns");
        assert_eq!(masonry_count_line(3), "11 cards · 1–2 columns");
        assert_eq!(masonry_count_line(-8), "0 cards · 1–2 columns");
    }

    #[test]
    fn story_state_adds_and_resets() {
        let story = MasonryStory::new(GalleryOptions::default());
        assert_eq!(story.card_count(), 8);
        story.add_card();
        story.add_card();
        assert_eq!(story.card_count(), 10);
        assert_eq!(story.count_line(), "10 cards · 1–2 columns");
        story.reset();
        assert_eq!(story.card_count(), 8);
        assert_eq!(story.extra_cards.get(), 0);
    }

    #[test]
    fn story_state_never_reports_a_negative_count() {
        let story = MasonryStory::new(GalleryOptions::default());
        story.extra_cards.set(-20);
        assert_eq!(story.card_count(), 0, "a negative count clamps at zero");
    }
}
