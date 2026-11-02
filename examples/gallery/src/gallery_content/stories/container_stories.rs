//! Container stories: panels, split views, scrolling, stacking, status bars and
//! carousels.
//!
//! Swift sources: `GalleryContent/Stories/Layout/LayoutStories.swift` and
//! `GalleryContent/Stories/Layout/NavigationStories.swift`.

use cove_composition::Scope;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_core::{Binding, State};
use cove_ui_compose::modifiers::layout::{LayoutFrameExt, PaddingExt};
use cove_ui_compose::modifiers::style::AppearanceExt;
use cove_ui_compose::primitives::layout::carousel::{Carousel, CarouselPage};
use cove_ui_compose::primitives::layout::split::{SplitAxis, SplitConstraints, SplitView};
use cove_ui_compose::primitives::layout::status_bar::{StatusBar, StatusBarText};
use cove_ui_compose::primitives::layout::{
    BoundedScrollView, Panel, ScrollView, ZStack,
};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::SemanticFontRef;
use cove_ui_compose::theme::tokens::{SemanticColorRef, SemanticStyleViewExt};
use cove_ui_compose::{
    Alignment, AnyView, BoxView, LayoutDebugExt, TupleView, View,
};

use super::feedback::StorySection;
use crate::gallery_content::catalog::GalleryOptions;

fn caption(text: String) -> AnyView {
    AnyView::new(
        Text::new(text)
            .semantic_font(SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted()),
    )
}

fn card(title: &str, body: &str, height: f32) -> AnyView {
    AnyView::new(
        AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            6.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(Text::new(title.to_owned())),
                AnyView::new(
                    Text::new(body.to_owned())
                        .semantic_font(SemanticFontRef::caption())
                        .semantic_foreground(SemanticColorRef::on_surface_muted()),
                ),
            ])),
        ))
        .padding(12.0)
        .corner_radius(8.0)
        .semantic_border(SemanticColorRef::border(), 1.0)
        .frame_height(height),
    )
}

/// `PanelStory`: a titled surface with an accessory.
pub struct PanelStory {
    pub options: GalleryOptions,
    pinned: State<bool>,
}

impl PanelStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            pinned: State::new(true),
        }
    }

    pub fn pinned(&self) -> bool {
        self.pinned.get()
    }

    pub fn set_pinned(&self, pinned: bool) {
        self.pinned.set(pinned);
    }

    pub fn readout(&self) -> String {
        if self.pinned.get() {
            "Pinned open".to_owned()
        } else {
            "Floating".to_owned()
        }
    }
}

impl View for PanelStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let accessory = {
            let pinned = self.pinned.clone();
            AnyView::new(
                cove_ui_compose::primitives::controls::Button::title("Toggle", move || {
                    pinned.update(|value| *value = !*value)
                })
                .enabled(self.options.is_enabled),
            )
        };
        let panel = AnyView::new(
            Panel::new(
                "Inspector",
                AnyView::new(card("Selection", "Nothing selected", 120.0)),
            )
            .accessory(accessory)
            .active(self.pinned.get())
            .frame_size(320.0, 220.0)
            .debug_name("panel-control"),
        );
        AnyView::new(StorySection::new(
            "Titled surface",
            "The header keeps its title and accessory on one line. An active panel draws the accent border.",
            AnyView::new(TupleView::new(vec![panel, caption(self.readout())])),
        ))
    }

    fn tag(&self) -> &'static str {
        "PanelStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; pinned)
    }
}

/// `SplitViewStory`: two panes with a draggable divider.
pub struct SplitViewStory {
    pub options: GalleryOptions,
    fraction: State<f32>,
}

impl SplitViewStory {
    /// The divider's minimum and maximum fractions.
    pub const RANGE: (f32, f32) = (0.2, 0.8);

    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            fraction: State::new(0.5),
        }
    }

    pub fn fraction(&self) -> f32 {
        self.fraction.get()
    }

    /// Writes a clamped split fraction.
    pub fn set_fraction(&self, fraction: f32) {
        self.fraction.set(fraction.clamp(Self::RANGE.0, Self::RANGE.1));
    }

    pub fn readout(&self) -> String {
        format!("{:.0} / {:.0}", self.fraction.get() * 100.0, (1.0 - self.fraction.get()) * 100.0)
    }
}

impl View for SplitViewStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let mut constraints = SplitConstraints::default();
        constraints.fraction_min = Self::RANGE.0;
        constraints.fraction_max = Self::RANGE.1;
        let split = AnyView::new(
            SplitView::resizable(
                SplitAxis::Horizontal,
                self.fraction.binding(),
                AnyView::new(card("Navigator", "Drag the divider", 160.0)),
                AnyView::new(card("Canvas", "The other pane", 160.0)),
            )
            .spacing(0.0)
            .divider_thickness(6.0)
            .constraints(constraints)
            .frame_size(420.0, 160.0)
            .debug_name("split-control"),
        );
        AnyView::new(StorySection::new(
            "Resizable split",
            "Drag the divider or focus it and use the arrow keys. The fraction stays inside its authored bounds.",
            AnyView::new(TupleView::new(vec![split, caption(self.readout())])),
        ))
    }

    fn tag(&self) -> &'static str {
        "SplitViewStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; fraction)
    }
}

/// `ScrollViewStory`: an unbounded scroller with its own bar.
pub struct ScrollViewStory {
    pub options: GalleryOptions,
}

impl ScrollViewStory {
    /// How many rows the content holds.
    pub const ROWS: usize = 20;

    pub fn new(options: GalleryOptions) -> Self {
        Self { options }
    }

    /// `20 rows of 32 points`, the content height the story reports.
    pub fn content_height() -> f32 {
        Self::ROWS as f32 * 32.0
    }
}

impl View for ScrollViewStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let rows = (0..Self::ROWS)
            .map(|index| AnyView::new(Text::new(format!("Row {index}"))))
            .collect();
        let scroller = AnyView::new(
            ScrollView::new(
                cove_ui_compose::primitives::layout::ScrollAxis::Vertical,
                AnyView::new(BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    8.0,
                    AnyView::new(TupleView::new(rows)),
                )),
            )
            .scroll_enabled(self.options.is_enabled)
            .accessibility_label("Long content")
            .frame_size(320.0, 200.0)
            .debug_name("scroll-view-control"),
        );
        AnyView::new(StorySection::new(
            "Free scrolling",
            "The bar appears while scrolling and fades afterwards. Page Up and Page Down scroll one viewport.",
            scroller,
        ))
    }

    fn tag(&self) -> &'static str {
        "ScrollViewStory"
    }
}

/// `BoundedScrollStory`: content capped at a maximum height.
pub struct BoundedScrollStory {
    pub options: GalleryOptions,
}

impl BoundedScrollStory {
    /// The maximum height the view honours.
    pub const MAX_HEIGHT: f32 = 160.0;

    pub fn new(options: GalleryOptions) -> Self {
        Self { options }
    }
}

impl View for BoundedScrollStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let content = (0..12)
            .map(|index| AnyView::new(Text::new(format!("Section {index}"))))
            .collect();
        let scroller = AnyView::new(
            BoundedScrollView::new(AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                8.0,
                AnyView::new(TupleView::new(content)),
            )))
            .content_height(384.0)
            .max_height(Self::MAX_HEIGHT)
            .frame_width(320.0)
            .debug_name("bounded-scroll-control"),
        );
        AnyView::new(StorySection::new(
            "Bounded height",
            "Short content measures to its natural height; long content stops at the maximum and scrolls.",
            scroller,
        ))
    }

    fn tag(&self) -> &'static str {
        "BoundedScrollStory"
    }
}

/// `ZStackStory`: overlapping layers with an alignment.
pub struct ZStackStory {
    pub options: GalleryOptions,
    alignment: State<usize>,
}

impl ZStackStory {
    /// The alignments the story cycles through, with their labels.
    pub const ALIGNMENTS: [(&'static str, Alignment); 3] = [
        ("Top leading", Alignment::TopLeading),
        ("Center", Alignment::Center),
        ("Bottom trailing", Alignment::BottomTrailing),
    ];

    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            alignment: State::new(1),
        }
    }

    pub fn alignment_index(&self) -> usize {
        self.alignment.get()
    }

    pub fn cycle(&self) {
        self.alignment
            .update(|index| *index = (*index + 1) % Self::ALIGNMENTS.len());
    }

    pub fn readout(&self) -> String {
        Self::ALIGNMENTS[self.alignment.get()].0.to_owned()
    }
}

impl View for ZStackStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let (_, alignment) = Self::ALIGNMENTS[self.alignment.get() % Self::ALIGNMENTS.len()];
        let stack = AnyView::new(
            ZStack::new(
                alignment,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(AnyView::new(card("Back layer", "240 × 120", 120.0)).frame_width(240.0)),
                    AnyView::new(AnyView::new(card("Front layer", "120 × 60", 60.0)).frame_width(120.0)),
                ])),
            )
            .frame_size(240.0, 120.0)
            .debug_name("zstack-control"),
        );
        let button = {
            let alignments = self.alignment.clone();
            AnyView::new(cove_ui_compose::primitives::controls::Button::title(
                "Next alignment",
                move || {
                    alignments
                        .update(|index| *index = (*index + 1) % ZStackStory::ALIGNMENTS.len())
                },
            ))
        };
        AnyView::new(StorySection::new(
            "Layers",
            "Children are drawn in order and aligned inside the stack's bounds. The stack measures to its largest child.",
            AnyView::new(TupleView::new(vec![
                stack,
                button,
                caption(self.readout()),
            ])),
        ))
    }

    fn tag(&self) -> &'static str {
        "ZStackStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; alignment)
    }
}

/// `StatusBarStory`: one bar with leading and trailing slots.
pub struct StatusBarStory {
    pub options: GalleryOptions,
}

impl StatusBarStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self { options }
    }
}

impl View for StatusBarStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let leading = AnyView::new(TupleView::new(vec![
            AnyView::new(StatusBarText::new("main")),
            AnyView::new(
                StatusBarText::new("UTF-8")
                    .monospaced(true)
                    .detail("LF"),
            ),
        ]));
        let trailing = AnyView::new(
            StatusBarText::new("Ln 12")
                .detail("Col 4")
                .monospaced(true),
        );
        let bar = AnyView::new(
            StatusBar::new(leading, trailing)
                .shows_divider(true)
                .enabled(self.options.is_enabled)
                .frame_width(420.0)
                .debug_name("status-bar-control"),
        );
        AnyView::new(StorySection::new(
            "Chrome",
            "One height at every width. When the slots do not fit, the leading text truncates before the trailing counts.",
            bar,
        ))
    }

    fn tag(&self) -> &'static str {
        "StatusBarStory"
    }
}

/// `CarouselStory`: paged content with keyboard paging.
pub struct CarouselStory {
    pub options: GalleryOptions,
    page: State<String>,
}

impl CarouselStory {
    /// The page ids, in order.
    pub const PAGES: [&'static str; 3] = ["overview", "activity", "settings"];

    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            page: State::new(Self::PAGES[0].to_owned()),
        }
    }

    pub fn page(&self) -> String {
        self.page.get()
    }

    pub fn select(&self, id: &str) -> bool {
        match Self::PAGES.iter().position(|page| *page == id) {
            Some(index) => {
                self.page.set(Self::PAGES[index].to_owned());
                true
            }
            None => false,
        }
    }

    pub fn readout(&self) -> String {
        format!("Page: {}", self.page.get())
    }

    fn pages(&self) -> Vec<CarouselPage<String>> {
        Self::PAGES
            .iter()
            .map(|id| {
                CarouselPage::new(
                    (*id).to_owned(),
                    AnyView::new(card(id, "One page per caption", 120.0)),
                )
                .caption(*id)
            })
            .collect()
    }

    fn binding(&self) -> Binding<String> {
        self.page.binding()
    }
}

impl View for CarouselStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let carousel = AnyView::new(
            Carousel::new(self.pages())
                .selection(self.binding())
                .shows_arrows(true)
                .allows_wrap(false)
                .height(140.0)
                .enabled(self.options.is_enabled)
                .frame_width(360.0)
                .debug_name("carousel-control"),
        );
        AnyView::new(StorySection::new(
            "Paged content",
            "Left and Right page the carousel; Home and End jump to the ends. Wrapping is off, so the ends stop.",
            AnyView::new(TupleView::new(vec![carousel, caption(self.readout())])),
        ))
    }

    fn tag(&self) -> &'static str {
        "CarouselStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; page)
    }
}
