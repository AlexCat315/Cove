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
use cove_ui_compose::primitives::layout::carousel::{
    Carousel, CarouselAutoplay, CarouselIndicator, CarouselPage,
};
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
        let long = AnyView::new(
            Panel::new(
                "Selection inspector with a title long enough to need truncation",
                AnyView::new(card("Selection", "Nothing selected", 120.0)),
            )
            .active(false)
            .frame_size(320.0, 220.0)
            .debug_name("panel-long-title"),
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
                        "Panel slots",
                        "The header keeps its title and accessory on one line. An active panel draws the accent border.",
                        AnyView::new(TupleView::new(vec![panel, caption(self.readout())])),
                    )),
                    AnyView::new(StorySection::new(
                        "Long titles",
                        "A long title truncates before the accessory, which keeps its full size.",
                        long,
                    )),
                ])),
            )
            .debug_name("panel-story"),
        )
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
        let proportional = AnyView::new(
            SplitView::new(
                SplitAxis::Horizontal,
                0.35,
                AnyView::new(card("Sidebar", "35%", 140.0)),
                AnyView::new(card("Content", "65%", 140.0)),
            )
            .spacing(0.0)
            .divider_thickness(0.0)
            .frame_size(420.0, 140.0)
            .debug_name("split-proportional"),
        );
        let vertical = AnyView::new(
            SplitView::new(
                SplitAxis::Vertical,
                0.5,
                AnyView::new(card("Top", "50%", 90.0)),
                AnyView::new(card("Bottom", "50%", 90.0)),
            )
            .spacing(0.0)
            .divider_thickness(0.0)
            .frame_size(320.0, 180.0)
            .debug_name("split-vertical"),
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
                        "Proportional split",
                        "A fixed fraction divides the pane with no divider and no drag target.",
                        AnyView::new(TupleView::new(vec![proportional, vertical])),
                    )),
                    AnyView::new(StorySection::new(
                        "Resizable and compact split",
                        "Drag the divider or focus it and use the arrow keys. The fraction stays inside its authored bounds.",
                        AnyView::new(TupleView::new(vec![split, caption(self.readout())])),
                    )),
                ])),
            )
            .debug_name("split-story"),
        )
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
    hits: State<u32>,
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
            hits: State::new(0),
        }
    }

    pub fn hits(&self) -> u32 {
        self.hits.get()
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
        let hits = self.hits.clone();
        let hit_target = AnyView::new(
            ZStack::new(
                Alignment::Center,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(
                        AnyView::new(card("Backdrop", "240 × 120", 120.0)).frame_width(240.0),
                    ),
                    AnyView::new(
                        cove_ui_compose::primitives::controls::Button::title("Tap target", move || {
                            hits.update(|count| *count += 1)
                        })
                        .enabled(self.options.is_enabled),
                    ),
                ])),
            )
            .frame_size(240.0, 120.0)
            .debug_name("zstack-hit-testing"),
        );
        let hit_readout = format!("{} taps reached the front layer", self.hits.get());

        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                24.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(StorySection::new(
                        "Layered content",
                        "Children are drawn in order and aligned inside the stack's bounds. The stack measures to its largest child.",
                        AnyView::new(TupleView::new(vec![
                            stack,
                            button,
                            caption(self.readout()),
                        ])),
                    )),
                    AnyView::new(StorySection::new(
                        "Hit testing",
                        "Only the topmost child under the pointer takes the press; background layers stay inert.",
                        AnyView::new(TupleView::new(vec![
                            hit_target,
                            caption(hit_readout),
                        ])),
                    )),
                ])),
            )
            .debug_name("zstack-story"),
        )
    }

    fn tag(&self) -> &'static str {
        "ZStackStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; alignment, hits)
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
        let single = AnyView::new(
            StatusBar::new(
                AnyView::new(TupleView::new(vec![])),
                AnyView::new(StatusBarText::new("Ln 1").detail("Col 1").monospaced(true)),
            )
            .shows_divider(true)
            .enabled(self.options.is_enabled)
            .frame_width(420.0)
            .debug_name("status-bar-single-slot"),
        );
        let long = AnyView::new(
            StatusBar::new(
                AnyView::new(TupleView::new(vec![AnyView::new(StatusBarText::new(
                    "feature/navigation-drawer-accessibility-followups",
                ))])),
                AnyView::new(StatusBarText::new("Ln 402").detail("Col 118")),
            )
            .shows_divider(true)
            .enabled(self.options.is_enabled)
            .frame_width(260.0)
            .debug_name("status-bar-long-branch"),
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
                        "Window footer",
                        "One height at every width. When the slots do not fit, the leading text truncates before the trailing counts.",
                        bar,
                    )),
                    AnyView::new(StorySection::new(
                        "Single slot",
                        "A bar with nothing on the leading edge still keeps the trailing counts aligned.",
                        single,
                    )),
                    AnyView::new(StorySection::new(
                        "Long branch names",
                        "The leading slot ellipsizes before the trailing counters give up their space.",
                        long,
                    )),
                ])),
            )
            .debug_name("status-bar-story"),
        )
    }

    fn tag(&self) -> &'static str {
        "StatusBarStory"
    }
}

/// `CarouselStory`: paged content with keyboard paging.
pub struct CarouselStory {
    pub options: GalleryOptions,
    page: State<String>,
    autoplay_page: State<String>,
    many_page: State<String>,
}

impl CarouselStory {
    /// The page ids, in order.
    pub const PAGES: [&'static str; 3] = ["overview", "activity", "settings"];

    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            page: State::new(Self::PAGES[0].to_owned()),
            autoplay_page: State::new(Self::PAGES[0].to_owned()),
            many_page: State::new("page-1".to_owned()),
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
        let paged = AnyView::new(StorySection::new(
            "Paged content",
            "Left and Right page the carousel; Home and End jump to the ends. Wrapping is off, so the ends stop.",
            AnyView::new(TupleView::new(vec![carousel, caption(self.readout())])),
        ));

        let autoplay = AnyView::new(
            Carousel::new(self.pages())
                .selection(self.autoplay_page.binding())
                .autoplay(CarouselAutoplay::new(4.0))
                .indicator(CarouselIndicator::Dots)
                .height(120.0)
                .enabled(self.options.is_enabled)
                .frame_width(360.0)
                .debug_name("carousel-autoplay"),
        );

        let many: Vec<CarouselPage<String>> = (1..=12)
            .map(|index| {
                let id = format!("page-{index}");
                CarouselPage::new(
                    id.clone(),
                    AnyView::new(card(&id.clone(), "Beyond eight pages the indicator counts", 100.0)),
                )
                .caption(id)
            })
            .collect();
        let many = AnyView::new(
            Carousel::new(many)
                .selection(self.many_page.binding())
                .allows_wrap(true)
                .height(120.0)
                .enabled(self.options.is_enabled)
                .frame_width(360.0)
                .debug_name("carousel-many"),
        );

        let bare = AnyView::new(
            Carousel::new(self.pages())
                .shows_arrows(false)
                .allows_wrap(false)
                .indicator(CarouselIndicator::Dots)
                .height(110.0)
                .enabled(self.options.is_enabled)
                .frame_width(360.0)
                .debug_name("carousel-no-chrome"),
        );

        let empty = AnyView::new(
            Carousel::new(Vec::<CarouselPage<String>>::new())
                .height(72.0)
                .enabled(self.options.is_enabled)
                .frame_width(360.0)
                .debug_name("carousel-empty"),
        );

        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                24.0,
                AnyView::new(TupleView::new(vec![
                    paged,
                    AnyView::new(StorySection::new(
                        "Autoplay",
                        "Pages advance every four seconds; hovering or focusing pauses, a manual page change restarts the interval.",
                        autoplay,
                    )),
                    AnyView::new(StorySection::new(
                        "Many pages",
                        "Beyond eight pages dots stop being countable, so the indicator shows a count.",
                        many,
                    )),
                    AnyView::new(StorySection::new(
                        "No wrap, no chrome",
                        "Onboarding flows end where the last page ends: wrapping off, arrows removed.",
                        bare,
                    )),
                    AnyView::new(StorySection::new(
                        "Empty collection",
                        "An empty carousel has no page action and no keyboard stop.",
                        empty,
                    )),
                ])),
            )
            .debug_name("carousel-story"),
        )
    }

    fn tag(&self) -> &'static str {
        "CarouselStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; page)
    }
}
