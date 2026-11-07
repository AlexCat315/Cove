//! Data display stories: lists, reordering, property grids, scrollbars and
//! timelines.
//!
//! Swift sources: `GalleryContent/Stories/Data/DataStories.swift` and
//! `GalleryContent/Stories/Layout/LayoutStories.swift`.

use cove_composition::Scope;
use cove_core::State;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use cove_ui_compose::primitives::controls::Toggle;
use cove_ui_compose::primitives::controls::fields::numeric::NumberField;
use cove_ui_compose::primitives::controls::fields::slider::Slider;
use cove_ui_compose::primitives::data::lists::RowState;
use cove_ui_compose::primitives::data::lists::{
    List, ListRow, ListRowAccessory, ReorderableList, Selection,
};
use cove_ui_compose::primitives::data::{
    PropertyGrid, PropertyGridRow, PropertyGridSection, Timeline, TimelineEvent, TimelineGroup,
    TimelineSpacing, TimelineZoom,
};
use cove_ui_compose::primitives::feedback::StatusTone;
use cove_ui_compose::primitives::interaction::{Scrollbar, ScrollbarAxis};
use cove_ui_compose::primitives::layout::{ScrollAxis, ScrollView, ScrollbarGutter};
use cove_ui_compose::primitives::media::ui_icon_catalog;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_field::TextField;
use cove_ui_compose::primitives::text::text_field::text_field_options::TextFieldBehavior;
use cove_ui_compose::theme::SemanticFontRef;
use cove_ui_compose::theme::tokens::{SemanticColorRef, SemanticStyleViewExt};
use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView, View};

use super::feedback::StorySection;
use super::showcase::stack;
use crate::gallery_content::catalog::GalleryOptions;

fn caption(text: String) -> AnyView {
    AnyView::new(
        Text::new(text)
            .semantic_font(SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted()),
    )
}

/// `ListStory`: single-selection rows with subtitles.
pub struct ListStory {
    pub options: GalleryOptions,
    selection: State<Option<usize>>,
    activated: State<Option<usize>>,
}

impl ListStory {
    /// `(title, trailing value)` for every row.
    pub const ROWS: [(&'static str, &'static str); 6] = [
        ("Inbox", "24"),
        ("Drafts", "3"),
        ("Sent", "Yesterday"),
        ("Archive", "1 204"),
        ("Spam", "12"),
        ("Trash", "Empty"),
    ];

    fn icon(index: usize) -> cove_imaging::BundleImageResource {
        match index {
            0 => ui_icon_catalog::file::inbox(),
            1 => ui_icon_catalog::action::pencil(),
            2 => ui_icon_catalog::shape::send(),
            3 => ui_icon_catalog::file::archive(),
            4 => ui_icon_catalog::status::shield(),
            _ => ui_icon_catalog::action::trash(),
        }
    }

    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            selection: State::new(Some(2)),
            activated: State::new(None),
        }
    }

    pub fn selection(&self) -> Option<usize> {
        self.selection.get()
    }

    pub fn activated(&self) -> Option<usize> {
        self.activated.get()
    }

    pub fn readout(&self) -> String {
        match (self.selection.get(), self.activated.get()) {
            (Some(selected), Some(activated)) if selected == activated => {
                format!("Activated {}", Self::ROWS[activated].0)
            }
            (Some(selected), _) => format!("Selected {}", Self::ROWS[selected].0),
            _ => "Nothing selected".to_owned(),
        }
    }
}

impl View for ListStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let activated = self.activated.clone();
        let list = AnyView::new(
            List::new(
                (0..Self::ROWS.len()).collect(),
                |id: &usize, _state: RowState| {
                    // The row inks itself from the band it sits on: icons,
                    // values and the chevron follow the selection and focus.
                    let (title, detail) = Self::ROWS[*id];
                    let row = ListRow::new(title)
                        .icon(Self::icon(*id))
                        .accessory(ListRowAccessory::Disclosure);
                    let row = match *id {
                        0 => row.badge("24"),
                        5 => row.destructive().value(detail),
                        _ => row.value(detail),
                    };
                    AnyView::new(row)
                },
            )
            .selection(Selection::single(self.selection.binding()))
            .row_height(36.0)
            .on_activate(move |id: &usize| activated.set(Some(*id)))
            .frame_height(6.0 * 36.0 + 8.0)
            .debug_name("list-control"),
        );
        AnyView::new(StorySection::new(
            "Selectable rows",
            "Up and Down move the highlight, Return activates. Clicking selects without activating.",
            AnyView::new(TupleView::new(vec![list, caption(self.readout())])),
        ))
    }

    fn tag(&self) -> &'static str {
        "ListStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; selection, activated)
    }
}

/// `ReorderableStory`: a session-driven reorderable list.
pub struct ReorderableStory {
    pub options: GalleryOptions,
    items: State<Vec<String>>,
    cards: State<Vec<String>>,
}

impl ReorderableStory {
    /// The authored order.
    pub const INITIAL: [&'static str; 5] = ["Landscape", "Portrait", "Macro", "Night", "Panorama"];
    /// The authored order of the horizontal strip.
    pub const CARDS: [&'static str; 6] = ["One", "Two", "Three", "Four", "Five", "Six"];

    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            items: State::new(Self::INITIAL.iter().map(|s| (*s).to_owned()).collect()),
            cards: State::new(Self::CARDS.iter().map(|s| (*s).to_owned()).collect()),
        }
    }

    pub fn items(&self) -> Vec<String> {
        self.items.get()
    }

    /// Moves the row at `from` to `to`, matching the drag callback.
    pub fn move_row(&self, from: usize, to: usize) {
        let mut items = self.items.get();
        if from >= items.len() || to > items.len() {
            return;
        }
        let item = items.remove(from);
        items.insert(to.min(items.len()), item);
        self.items.set(items);
    }

    pub fn reset(&self) {
        self.items
            .set(Self::INITIAL.iter().map(|s| (*s).to_owned()).collect());
        self.cards
            .set(Self::CARDS.iter().map(|s| (*s).to_owned()).collect());
    }

    pub fn readout(&self) -> String {
        format!("Order: {}", self.items.get().join(" · "))
    }

    pub fn card_readout(&self) -> String {
        format!("Card order: {}", self.cards.get().join(" · "))
    }
}

impl View for ReorderableStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let items = self.items.clone();
        let list = AnyView::new(
            ReorderableList::new(
                self.items.get(),
                |item: &String| item.clone(),
                |item: &String, _state: RowState| {
                    AnyView::new(
                        AnyView::new(BoxView::new(
                            FlexDirection::Row,
                            Align::Center,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            8.0,
                            AnyView::new(TupleView::new(vec![AnyView::new(
                                Text::new(item.clone()).flex(1.0).frame_min_width(0.0),
                            )])),
                        ))
                        .padding_edges(12.0, 0.0),
                    )
                },
            )
            .on_move({
                let items = items.clone();
                move |from: usize, to: usize| {
                    let mut current = items.get();
                    if from >= current.len() || to > current.len() {
                        return;
                    }
                    let item = current.remove(from);
                    current.insert(to.min(current.len()), item);
                    items.set(current);
                }
            })
            .row_height(44.0)
            .is_enabled(self.options.is_enabled)
            .frame_height(220.0)
            .debug_name("reorderable-control"),
        );
        let cards = self.cards.clone();
        let strip = AnyView::new(
            ReorderableList::new(
                self.cards.get(),
                |item: &String| item.clone(),
                |item: &String, _state: RowState| {
                    AnyView::new(
                        AnyView::new(BoxView::new(
                            FlexDirection::Column,
                            Align::Center,
                            Justify::Center,
                            Wrap::NoWrap,
                            0.0,
                            AnyView::new(TupleView::new(vec![AnyView::new(Text::new(
                                item.clone(),
                            ))])),
                        ))
                        .padding_edges(0.0, 10.0),
                    )
                },
            )
            .on_move({
                let cards = cards.clone();
                move |from: usize, to: usize| {
                    let mut current = cards.get();
                    if from >= current.len() || to > current.len() {
                        return;
                    }
                    let item = current.remove(from);
                    current.insert(to.min(current.len()), item);
                    cards.set(current);
                }
            })
            .row_height(112.0)
            .row_spacing(8.0)
            .axis(ScrollAxis::Horizontal)
            .is_enabled(self.options.is_enabled)
            .frame_height(96.0)
            .debug_name("reorderable-horizontal"),
        );
        AnyView::new(TupleView::new(vec![
            AnyView::new(StorySection::new(
                "Reordering",
                "Space lifts the focused row, arrows move it and Space drops it. Escape cancels and restores the original order.",
                AnyView::new(TupleView::new(vec![list, caption(self.readout())])),
            )),
            AnyView::new(StorySection::new(
                "Reordering along the scroll axis",
                "The same handle and settle spring drive a horizontal list; the cards reflow as you drag them left or right.",
                AnyView::new(TupleView::new(vec![strip, caption(self.card_readout())])),
            )),
        ]))
    }

    fn tag(&self) -> &'static str {
        "ReorderableStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; items, cards)
    }
}

/// `PropertyGridStory`: collapsible labelled values, typed inspector controls
/// and the empty state.
pub struct PropertyGridStory {
    pub options: GalleryOptions,
    name: State<TextBuffer>,
    visible: State<bool>,
    exposure: State<f32>,
}

impl PropertyGridStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            name: State::new(TextBuffer::new("Cove Studio")),
            visible: State::new(true),
            exposure: State::new(1.2),
        }
    }

    pub fn name(&self) -> String {
        self.name.get().string_value()
    }

    pub fn readout(&self) -> String {
        format!("Name: {}", self.name.get().string_value())
    }
}

impl View for PropertyGridStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let enabled = self.options.is_enabled;
        let name_field = AnyView::new(
            TextField::new("Name", self.name.binding())
                .behavior(TextFieldBehavior {
                    disabled: !enabled,
                    ..Default::default()
                })
                .frame_width(180.0)
                .debug_name("property-grid-name"),
        );
        let sections = vec![
            PropertyGridSection::new(
                "general",
                "General",
                vec![
                    PropertyGridRow::new("name", "Name", name_field),
                    PropertyGridRow::new("kind", "Kind", AnyView::new(Text::new("Workspace"))),
                ],
            )
            .collapsible(true)
            .row_count(true),
            PropertyGridSection::new(
                "geometry",
                "Geometry",
                vec![
                    PropertyGridRow::new("width", "Width", AnyView::new(Text::new("1440 pt"))),
                    PropertyGridRow::new("height", "Height", AnyView::new(Text::new("900 pt"))),
                ],
            )
            .collapsible(true)
            .starts_collapsed(true),
        ];
        let grid = AnyView::new(
            PropertyGrid::new(sections)
                .label_width(110.0)
                .frame_width(360.0)
                .debug_name("property-grid-control"),
        );
        let labelled = AnyView::new(StorySection::new(
            "Labelled values",
            "Each header toggles its section. Labels keep their column; long values wrap under the label when the pane narrows.",
            AnyView::new(TupleView::new(vec![grid, caption(self.readout())])),
        ));

        let inspector_sections = vec![
            PropertyGridSection::new(
                "identity",
                "Identity",
                vec![
                    PropertyGridRow::new(
                        "name",
                        "Name",
                        AnyView::new(
                            TextField::new("Name", self.name.binding())
                                .behavior(TextFieldBehavior {
                                    disabled: !enabled,
                                    ..Default::default()
                                })
                                .debug_name("property-grid-inspector-name"),
                        ),
                    ),
                    PropertyGridRow::new(
                        "visible",
                        "Visible",
                        AnyView::new(Toggle::new(self.visible.binding()).enabled(enabled)),
                    ),
                ],
            )
            .collapsible(true),
            PropertyGridSection::new(
                "camera",
                "Camera",
                vec![PropertyGridRow::new(
                    "exposure",
                    "Exposure",
                    AnyView::new(
                        NumberField::new(self.exposure.binding())
                            .enabled(enabled)
                            .range(Some(0.0), Some(8.0))
                            .step(0.1),
                    ),
                )],
            )
            .collapsible(true),
        ];
        let inspector = AnyView::new(
            PropertyGrid::new(inspector_sections)
                .min_value_width(200.0)
                .frame_height(270.0)
                .debug_name("property-grid-inspector"),
        );
        let empty = AnyView::new(
            PropertyGrid::new(Vec::new())
                .empty_text("Select an object to inspect its properties")
                .frame_height(70.0)
                .debug_name("property-grid-empty"),
        );
        let inspector_section = AnyView::new(StorySection::new(
            "Inspector",
            "Typed controls occupy value slots; section and row sizing belongs to the grid.",
            AnyView::new(TupleView::new(vec![inspector, empty])),
        ));
        stack(vec![labelled, inspector_section])
    }

    fn tag(&self) -> &'static str {
        "PropertyGridStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; name, visible, exposure)
    }
}

/// `ScrollbarStory`: a standalone thumb over caller-owned content.
pub struct ScrollbarStory {
    pub options: GalleryOptions,
    offset: State<f32>,
}

impl ScrollbarStory {
    /// The scrollable extent the story reports.
    pub const CONTENT_EXTENT: f32 = 600.0;
    /// The viewport extent the story reports.
    pub const VIEWPORT_EXTENT: f32 = 200.0;
    /// Lines in the scrolled document.
    pub const LINE_COUNT: usize = 15;

    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            offset: State::new(0.0),
        }
    }

    pub fn offset(&self) -> f32 {
        self.offset.get()
    }

    pub fn max_offset(&self) -> f32 {
        (Self::CONTENT_EXTENT - Self::VIEWPORT_EXTENT).max(0.0)
    }

    pub fn set_offset(&self, offset: f32) {
        self.offset.set(offset.clamp(0.0, self.max_offset()));
    }

    pub fn readout(&self) -> String {
        format!("{:.0} / {:.0} pt", self.offset.get(), self.max_offset())
    }
}

impl View for ScrollbarStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let bar = AnyView::new(
            Scrollbar::new(
                ScrollbarAxis::Vertical,
                self.offset.binding(),
                Self::CONTENT_EXTENT,
                Self::VIEWPORT_EXTENT,
            )
            .thickness(12.0)
            .always_visible(true)
            .label("Scroll position")
            .frame_height(Self::VIEWPORT_EXTENT)
            .debug_name("scrollbar-control"),
        );
        // The document the thumb drives: the scroll view shares the story's
        // offset and hides its own bar, so the standalone thumb is the only one.
        let lines = (1..=Self::LINE_COUNT)
            .map(|line| {
                AnyView::new(
                    Text::new(format!("Line {line}"))
                        .padding_edges(12.0, 0.0)
                        .frame_height(Self::CONTENT_EXTENT / Self::LINE_COUNT as f32),
                )
            })
            .collect::<Vec<_>>();
        let offset = self.offset.clone();
        let write = self.offset.clone();
        let max_offset = self.max_offset();
        let document = AnyView::new(
            ScrollView::vertical(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(lines)),
            ))
            .scroll_offset(cove_core::Binding::new(
                move || cove_core::Point::new(0.0, offset.get()),
                move |point: cove_core::Point| write.set(point.y.clamp(0.0, max_offset)),
            ))
            .scrollbar_gutter(ScrollbarGutter::Hidden)
            .flex_with(1.0, 1.0, Some(0.0))
            .frame_height(Self::VIEWPORT_EXTENT)
            .debug_name("scrollbar-document"),
        );
        let viewport = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            4.0,
            AnyView::new(TupleView::new(vec![document, bar])),
        ));
        AnyView::new(StorySection::new(
            "Thumb",
            "Arrows move by one line, Page Up and Page Down by a viewport, Home and End jump to the bounds.",
            AnyView::new(TupleView::new(vec![viewport, caption(self.readout())])),
        ))
    }

    fn tag(&self) -> &'static str {
        "ScrollbarStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; offset)
    }
}

/// `TimelineStory`: dated events with a zoom factor.
pub struct TimelineStory {
    pub options: GalleryOptions,
    zoom: State<f32>,
}

impl TimelineStory {
    /// The first event's date, in seconds since the epoch.
    pub const START: f64 = 1_762_000_000.0;

    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            zoom: State::new(1.0),
        }
    }

    pub fn zoom(&self) -> f32 {
        self.zoom.get()
    }

    pub fn set_zoom(&self, zoom: f32) {
        self.zoom.set(zoom.clamp(0.5, 3.0));
    }

    pub fn readout(&self) -> String {
        format!("{:.1}× zoom", self.zoom.get())
    }
}

impl View for TimelineStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let binding = self.zoom.binding();
        let timeline = AnyView::new(
            Timeline::new(vec![
                TimelineEvent::new("Opened project")
                    .id("open")
                    .date(Self::START)
                    .detail("Cove Studio"),
                TimelineEvent::new("Added component")
                    .id("add")
                    .date(Self::START + 3_600.0)
                    .detail("Timeline"),
                TimelineEvent::new("Saved")
                    .id("save")
                    .date(Self::START + 7_200.0)
                    .detail("3 files"),
            ])
            .configure(|timeline| {
                timeline.spacing = TimelineSpacing::proportional(40.0);
                timeline.zoom = Some(TimelineZoom::new(binding).range(0.5, 3.0));
                timeline.shows_trailing_connector = true;
            })
            .frame_width(420.0)
            .debug_name("timeline-control"),
        );
        let zoom = AnyView::new(
            Slider::new(self.zoom_binding())
                .range((0.5, 3.0))
                .step(0.1)
                .frame_width(180.0)
                .debug_name("timeline-zoom"),
        );
        let grouped = AnyView::new(
            Timeline::groups(vec![
                TimelineGroup::new(
                    "Today",
                    vec![
                        TimelineEvent::new("Review requested")
                            .id("review")
                            .date(Self::START + 10_800.0)
                            .tone(StatusTone::Warning),
                        TimelineEvent::new("Checks passed")
                            .id("checks")
                            .date(Self::START + 14_400.0)
                            .tone(StatusTone::Success),
                    ],
                )
                .id("today")
                .is_collapsible(true),
                TimelineGroup::new(
                    "Yesterday",
                    vec![
                        TimelineEvent::new("Build failed")
                            .id("failed")
                            .date(Self::START + 1_800.0)
                            .tone(StatusTone::Danger),
                    ],
                )
                .id("yesterday")
                .is_collapsible(true),
            ])
            .frame_width(420.0)
            .debug_name("timeline-grouped"),
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
                        "Event sequence",
                        "Spacing is proportional to the gap between dates. The slider scales the whole track without reordering events.",
                        AnyView::new(TupleView::new(vec![
                            timeline,
                            zoom,
                            caption(self.readout()),
                        ])),
                    )),
                    AnyView::new(StorySection::new(
                        "Groups, collapse and zoom",
                        "Groups keep their own headings and outcomes; collapsing a group hides its events without touching the others.",
                        grouped,
                    )),
                ])),
            )
            .debug_name("timeline-story"),
        )
    }

    fn tag(&self) -> &'static str {
        "TimelineStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; zoom)
    }
}

impl TimelineStory {
    fn zoom_binding(&self) -> cove_core::Binding<f64> {
        let state = self.zoom.clone();
        cove_core::Binding::new(
            {
                let state = state.clone();
                move || state.get() as f64
            },
            move |value| state.set(value as f32),
        )
    }
}
