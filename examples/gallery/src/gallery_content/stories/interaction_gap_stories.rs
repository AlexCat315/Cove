//! Stories for the interaction and responsive-layout primitives the migration
//! audit listed as having no gallery coverage
//! (`Docs/audit-swift-migration-dead-code-and-gallery-coverage.md` §5.4).
//!
//! Each page drives the primitive's real public surface rather than describing
//! it:
//!
//! - `HorizontalSwipe` is a live host: drag across the card to turn the page.
//! - `EdgeAutoScroll` exposes the velocity curve as a table, because the
//!   driver itself needs a drag session inside a scroll view.
//! - `ScrollReveal` reproduces `delta` against a real `ScrollView` offset.
//! - `ActionGroupLayout` is instantiated at three widths so the stacking
//!   threshold is visible.

use cove_composition::Scope;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_core::{Point, Rect, State};
use cove_ui_compose::modifiers::layout::{LayoutFrameExt, PaddingExt};
use cove_ui_compose::modifiers::style::AppearanceExt;
use cove_ui_compose::primitives::interaction::edge_auto_scroll::MAXIMUM_SPEED;
use cove_ui_compose::primitives::interaction::{
    HorizontalSwipe, HorizontalSwipeDirection, edge_auto_scroll_velocity,
};
use cove_ui_compose::primitives::layout::{ActionGroupLayout, ScrollReveal};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::{SemanticColorRef, SemanticStyleViewExt};
use cove_ui_compose::{
    AnyView, BoxView, HorizontalAlignment, LayoutContainer, ScrollView, TupleView, View,
};

use super::feedback::StorySection;
use crate::gallery_content::catalog::{CowSize, GalleryOptions};

fn column(items: Vec<AnyView>, spacing: f32) -> AnyView {
    AnyView::new(BoxView::new(
        FlexDirection::Column,
        Align::Stretch,
        Justify::FlexStart,
        Wrap::NoWrap,
        spacing,
        AnyView::new(TupleView::new(items)),
    ))
}

fn row(items: Vec<AnyView>, spacing: f32) -> AnyView {
    AnyView::new(BoxView::new(
        FlexDirection::Row,
        Align::Center,
        Justify::FlexStart,
        Wrap::Wrap,
        spacing,
        AnyView::new(TupleView::new(items)),
    ))
}

fn button(title: &str, is_enabled: bool, action: impl Fn() + 'static) -> AnyView {
    AnyView::new(
        cove_ui_compose::primitives::controls::Button::title(title, action).enabled(is_enabled),
    )
}

/// The pages a swipe can turn to.
const SWIPE_PAGES: [&str; 3] = [
    "Overview · drag left to advance",
    "Details · drag left again",
    "Summary · the last page wraps",
];

/// `HorizontalSwipe`: a horizontal drag turns one page.
pub struct HorizontalSwipeStory {
    options: GalleryOptions,
    page: State<usize>,
    last: State<String>,
    turns: State<u32>,
}

impl HorizontalSwipeStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            page: State::new(0),
            last: State::new(String::new()),
            turns: State::new(0),
        }
    }

    pub fn page(&self) -> usize {
        self.page.get()
    }

    pub fn turns(&self) -> u32 {
        self.turns.get()
    }

    /// The distance a drag must cover before it counts as a page turn:
    /// `max(32pt, 15% of the host width)`.
    pub fn threshold(host_width: f32) -> f32 {
        32.0_f32.max(host_width * 0.15)
    }
}

impl View for HorizontalSwipeStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let page = self.page.clone();
        let last = self.last.clone();
        let turns = self.turns.clone();

        let card = AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::Center,
                Wrap::NoWrap,
                6.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(Text::new(SWIPE_PAGES[self.page.get() % SWIPE_PAGES.len()])),
                    AnyView::new(Text::new(format!(
                        "page {} of {}",
                        self.page.get() + 1,
                        SWIPE_PAGES.len()
                    ))),
                ])),
            )
            .padding(24.0)
            .frame_height(140.0)
            .corner_radius(12.0)
            .semantic_background(SemanticColorRef::surface_variant()),
        );

        let host = AnyView::new(
            HorizontalSwipe::new(
                move |direction: HorizontalSwipeDirection| {
                    let step = match direction {
                        HorizontalSwipeDirection::Left => 1,
                        HorizontalSwipeDirection::Right => SWIPE_PAGES.len() - 1,
                    };
                    page.set((page.get() + step) % SWIPE_PAGES.len());
                    last.set(match direction {
                        HorizontalSwipeDirection::Left => "Left".to_string(),
                        HorizontalSwipeDirection::Right => "Right".to_string(),
                    });
                    turns.set(turns.get() + 1);
                },
                card,
            )
            .enabled(self.options.is_enabled),
        );

        let reset = {
            let page = self.page.clone();
            let last = self.last.clone();
            let turns = self.turns.clone();
            move || {
                page.set(0);
                last.set(String::new());
                turns.set(0);
            }
        };

        let readout = Text::new(if self.last.get().is_empty() {
            "no swipe observed yet".to_string()
        } else {
            format!(
                "last swipe: {} · turns: {}",
                self.last.get(),
                self.turns.get()
            )
        });

        AnyView::new(column(
            vec![
                AnyView::new(StorySection::new(
                    "Swipe host",
                    "Press and drag horizontally across the card. A drag under the threshold is ignored; a vertical drag is rejected and left to the enclosing scroll view.",
                    host,
                )),
                AnyView::new(StorySection::new(
                    "Gesture model",
                    format!(
                        "The session observes the capture phase, so a nested button still receives its tap. Threshold: {}pt at a 320pt host width.",
                        Self::threshold(320.0)
                    ),
                    column(
                        vec![
                            AnyView::new(readout),
                            button("Reset", self.options.is_enabled, reset),
                        ],
                        10.0,
                    ),
                )),
            ],
            20.0,
        ))
    }
}

/// The viewport the velocity table is measured against.
const EDGE_VIEWPORT: Rect = Rect::new(0.0, 0.0, 320.0, 400.0);

/// `EdgeAutoScroll`: scroll-while-dragging speed near a viewport edge.
pub struct EdgeAutoScrollStory {
    options: GalleryOptions,
}

impl EdgeAutoScrollStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self { options }
    }

    /// Pointer positions the table samples, top to bottom.
    pub const SAMPLES: [f32; 11] = [
        0.0, 20.0, 40.0, 48.0, 100.0, 200.0, 300.0, 352.0, 360.0, 380.0, 400.0,
    ];

    /// Signed speed at `pointer_y` in the demo viewport.
    pub fn speed(pointer_y: f32) -> f32 {
        edge_auto_scroll_velocity(pointer_y, EDGE_VIEWPORT)
    }

    /// The edge zone: `min(48, height / 4)`.
    pub fn zone() -> f32 {
        48.0_f32.min(EDGE_VIEWPORT.height / 4.0)
    }

    /// Full-scale bar length for the current density.
    pub fn bar_scale(size: CowSize) -> f32 {
        match size {
            CowSize::Mini => 100.0,
            CowSize::Small => 130.0,
            CowSize::Regular => 160.0,
            CowSize::Large => 200.0,
        }
    }
}

impl View for EdgeAutoScrollStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let scale = Self::bar_scale(self.options.size);
        let rows = Self::SAMPLES
            .iter()
            .map(|pointer_y| {
                let speed = Self::speed(*pointer_y);
                let magnitude = (speed.abs() / MAXIMUM_SPEED).clamp(0.0, 1.0);
                let bar = AnyView::new(
                    Text::new("")
                        .frame_width((magnitude * scale).max(1.0))
                        .frame_height(6.0)
                        .semantic_background(if speed < 0.0 {
                            SemanticColorRef::accent_muted()
                        } else if speed > 0.0 {
                            SemanticColorRef::accent()
                        } else {
                            SemanticColorRef::surface_variant()
                        }),
                );
                AnyView::new(row(
                    vec![
                        AnyView::new(Text::new(format!("{pointer_y:>5.0}pt"))),
                        bar,
                        AnyView::new(Text::new(format!("{speed:>8.1} pt/s"))),
                    ],
                    8.0,
                ))
            })
            .collect::<Vec<_>>();

        let summary = format!(
            "Zone: {:.0}pt at each edge. Speed rises quadratically with depth and reaches ±{:.0}pt/s at the edge itself. A pointer resting in the zone keeps scrolling.",
            Self::zone(),
            MAXIMUM_SPEED
        );

        AnyView::new(column(
            vec![
                AnyView::new(StorySection::new(
                    "Velocity curve",
                    format!(
                        "Pointer height in a {:.0}pt viewport against the signed scroll speed it produces.",
                        EDGE_VIEWPORT.height
                    ),
                    column(rows, 4.0),
                )),
                AnyView::new(StorySection::new(
                    "Where it is used",
                    summary,
                    column(
                        vec![AnyView::new(Text::new(
                            "The driver is a node resource: it ticks between `begin` and `end` and moves the enclosing scroll view. `ReorderableList` is the in-tree consumer — open that page and drag a row to the top or bottom edge.",
                        ))],
                        8.0,
                    ),
                )),
            ],
            20.0,
        ))
    }
}

/// The viewport height the reveal demo assumes.
const REVEAL_VIEWPORT: f32 = 200.0;
/// Fixed row height in the reveal demo.
const REVEAL_ROW: f32 = 28.0;
/// Rows in the reveal demo.
const REVEAL_ROWS: usize = 20;

/// `ScrollReveal`: the offset a scroll container needs to bring a row into view.
pub struct ScrollRevealStory {
    options: GalleryOptions,
    offset: State<Point>,
    last_delta: State<f32>,
}

impl ScrollRevealStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            offset: State::new(Point::new(0.0, 0.0)),
            last_delta: State::new(0.0),
        }
    }

    /// Content height of the demo list.
    pub fn content_height() -> f32 {
        REVEAL_ROWS as f32 * REVEAL_ROW
    }

    /// The delta `ScrollReveal::delta` computes for one row.
    pub fn delta_for(row: usize) -> f32 {
        let start = row as f32 * REVEAL_ROW;
        ScrollReveal::delta(start, start + REVEAL_ROW, 0.0, REVEAL_VIEWPORT)
    }

    pub fn offset(&self) -> f32 {
        self.offset.get().y
    }
}

impl View for ScrollRevealStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let rows = (0..REVEAL_ROWS)
            .map(|index| AnyView::new(Text::new(format!("Row {index}")).frame_height(REVEAL_ROW)))
            .collect::<Vec<_>>();

        let scroll = AnyView::new(
            ScrollView::vertical(column(rows, 0.0))
                .scroll_offset(self.offset.binding())
                .frame_height(REVEAL_VIEWPORT),
        );

        let mut buttons = Vec::new();
        for row in [0_usize, 6, 12, 19] {
            let offset = self.offset.clone();
            let last_delta = self.last_delta.clone();
            buttons.push(button(
                &format!("Reveal {row}"),
                self.options.is_enabled,
                move || {
                    let delta = Self::delta_for(row);
                    let limit = (Self::content_height() - REVEAL_VIEWPORT).max(0.0);
                    let y = (offset.get().y + delta).clamp(0.0, limit);
                    offset.set(Point::new(0.0, y));
                    last_delta.set(delta);
                },
            ));
        }
        let reset = {
            let offset = self.offset.clone();
            let last_delta = self.last_delta.clone();
            move || {
                offset.set(Point::new(0.0, 0.0));
                last_delta.set(0.0);
            }
        };
        buttons.push(button("Reset", self.options.is_enabled, reset));

        let readout = Text::new(format!(
            "offset {:.0}pt · last delta {:+.0}pt · rows 0–{} are taller than the {:.0}pt viewport",
            self.offset.get().y,
            self.last_delta.get(),
            REVEAL_ROWS - 1,
            REVEAL_VIEWPORT
        ));

        AnyView::new(column(
            vec![
                AnyView::new(StorySection::new(
                    "Reveal targets",
                    "Each button applies `delta(row_start, row_end, 0, viewport_height)` to the scroll offset, which is exactly what `reveal` does to a scroll ancestor.",
                    column(vec![row(buttons, 8.0), AnyView::new(readout)], 10.0),
                )),
                AnyView::new(StorySection::new(
                    "Scroll container",
                    "A row already inside the viewport yields a zero delta; a row below yields its overhang; a row taller than the viewport aligns to the top.",
                    scroll,
                )),
            ],
            20.0,
        ))
    }
}

/// `ActionGroupLayout`: actions that stack when their natural widths overflow.
pub struct ActionGroupLayoutStory {
    options: GalleryOptions,
    stacks: State<bool>,
    alignment: State<HorizontalAlignment>,
    spacing: State<f32>,
    pressed: State<String>,
}

impl ActionGroupLayoutStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            stacks: State::new(false),
            alignment: State::new(HorizontalAlignment::Trailing),
            spacing: State::new(8.0),
            pressed: State::new(String::new()),
        }
    }

    /// Widths the same group is measured at, widest first.
    pub const WIDTHS: [f32; 3] = [420.0, 300.0, 220.0];

    pub fn stacks(&self) -> bool {
        self.stacks.get()
    }
}

impl View for ActionGroupLayoutStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let pressed = self.pressed.clone();
        let actions = AnyView::new(TupleView::new(
            ["Cancel", "Save Draft", "Publish"]
                .iter()
                .map(|title| {
                    let pressed = pressed.clone();
                    let label = title.to_string();
                    AnyView::new(
                        cove_ui_compose::primitives::controls::Button::title(*title, move || {
                            pressed.set(label.clone())
                        })
                        .enabled(self.options.is_enabled),
                    )
                })
                .collect::<Vec<_>>(),
        ));

        let algorithm = ActionGroupLayout {
            stacks: self.stacks.get(),
            alignment: self.alignment.get(),
            spacing: self.spacing.get(),
        };

        let widths = Self::WIDTHS
            .iter()
            .map(|width| {
                let group = AnyView::new(
                    LayoutContainer::<ActionGroupLayout, AnyView>::with_erased_content(
                        ActionGroupLayout {
                            stacks: algorithm.stacks,
                            alignment: algorithm.alignment,
                            spacing: algorithm.spacing,
                        },
                        actions.clone(),
                    )
                    .frame_width(*width),
                );
                AnyView::new(column(
                    vec![AnyView::new(Text::new(format!("{width:.0}pt wide"))), group],
                    6.0,
                ))
            })
            .collect::<Vec<_>>();

        let stacks = self.stacks.clone();
        let alignment = self.alignment.clone();
        let spacing = self.spacing.clone();
        let toggle_stacks = {
            let stacks = stacks.clone();
            move || stacks.set(!stacks.get())
        };
        let cycle_alignment = {
            let alignment = alignment.clone();
            move || {
                alignment.set(match alignment.get() {
                    HorizontalAlignment::Leading => HorizontalAlignment::Center,
                    HorizontalAlignment::Center => HorizontalAlignment::Trailing,
                    HorizontalAlignment::Trailing => HorizontalAlignment::Leading,
                })
            }
        };
        let wider = {
            let spacing = spacing.clone();
            move || spacing.set((spacing.get() + 4.0).min(24.0))
        };
        let narrower = {
            let spacing = spacing.clone();
            move || spacing.set((spacing.get() - 4.0).max(0.0))
        };

        let controls = row(
            vec![
                button(
                    if self.stacks.get() {
                        "Always stack: on"
                    } else {
                        "Always stack: off"
                    },
                    self.options.is_enabled,
                    toggle_stacks,
                ),
                button(
                    &format!("Align: {:?}", self.alignment.get()),
                    self.options.is_enabled,
                    cycle_alignment,
                ),
                button("Spacing +4", self.options.is_enabled, wider),
                button("Spacing −4", self.options.is_enabled, narrower),
            ],
            8.0,
        );

        AnyView::new(column(
            vec![
                AnyView::new(StorySection::new(
                    "Same group, three widths",
                    "Without `stacks` the group stays on one line while the natural widths fit and becomes a column as soon as they overflow.",
                    column(widths, 14.0),
                )),
                AnyView::new(StorySection::new(
                    "Layout parameters",
                    format!(
                        "stacks={} · alignment={:?} · spacing={:.0} · last press: {}",
                        self.stacks.get(),
                        self.alignment.get(),
                        self.spacing.get(),
                        if self.pressed.get().is_empty() {
                            "none".to_string()
                        } else {
                            self.pressed.get()
                        }
                    ),
                    controls,
                )),
            ],
            20.0,
        ))
    }
}
