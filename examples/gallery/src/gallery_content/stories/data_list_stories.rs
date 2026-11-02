//! Data display stories: lists, reordering, property grids, scrollbars and
//! timelines.
//!
//! Swift sources: `GalleryContent/Stories/Data/DataStories.swift` and
//! `GalleryContent/Stories/Layout/LayoutStories.swift`.

use cove_composition::Scope;
use cove_core::State;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::fields::slider::Slider;
use cove_ui_compose::primitives::data::lists::{
    CollectionSelection, List, ListRow, ListRowOptions, ReorderableList,
};
use cove_ui_compose::primitives::data::{
    PropertyGrid, PropertyGridRow, PropertyGridSection, Timeline, TimelineEvent, TimelineSpacing,
    TimelineZoom,
};
use cove_ui_compose::primitives::interaction::scrollbar::{Scrollbar, ScrollbarAxis};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_field::TextField;
use cove_ui_compose::theme::SemanticFontRef;
use cove_ui_compose::theme::tokens::{SemanticColorRef, SemanticStyleViewExt};
use cove_ui_compose::{AnyView, LayoutDebugExt, TupleView, View};

use super::feedback::StorySection;
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
    /// `(title, subtitle)` for every row.
    pub const ROWS: [(&'static str, &'static str); 6] = [
        ("Inbox", "24 unread"),
        ("Drafts", "3 waiting"),
        ("Sent", "Yesterday"),
        ("Archive", "1 204 messages"),
        ("Spam", "12 held"),
        ("Trash", "Empty"),
    ];

    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            selection: State::new(None),
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
                CollectionSelection::single(self.selection.binding()),
                |id: usize, is_selected: bool| {
                    let (title, subtitle) = Self::ROWS[id];
                    AnyView::new(
                        ListRow::new(title).options(
                            ListRowOptions::new()
                                .subtitle(subtitle)
                                .disclosure(is_selected),
                        ),
                    )
                },
            )
            .rows((0..Self::ROWS.len()).collect())
            .row_height(40.0)
            .on_activate(move |id| activated.set(Some(id)))
            .frame_size(320.0, 240.0)
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
}

impl ReorderableStory {
    /// The authored order.
    pub const INITIAL: [&'static str; 5] = ["Landscape", "Portrait", "Macro", "Night", "Panorama"];

    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            items: State::new(Self::INITIAL.iter().map(|s| (*s).to_owned()).collect()),
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
    }

    pub fn readout(&self) -> String {
        format!("Order: {}", self.items.get().join(" · "))
    }
}

impl View for ReorderableStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let items = self.items.clone();
        let list = AnyView::new(
            ReorderableList::new(
                self.items.get(),
                |item: &String| item.clone(),
                {
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
                },
                |item: &String, is_dragging: bool| {
                    AnyView::new(
                        ListRow::new(item.clone()).options(ListRowOptions::new().subtitle(
                            if is_dragging {
                                "Moving"
                            } else {
                                "Drag to reorder"
                            },
                        )),
                    )
                },
            )
            .row_height(36.0)
            .is_enabled(self.options.is_enabled)
            .frame_size(320.0, 220.0)
            .debug_name("reorderable-control"),
        );
        AnyView::new(StorySection::new(
            "Reordering",
            "Space lifts the focused row, arrows move it and Space drops it. Escape cancels and restores the original order.",
            AnyView::new(TupleView::new(vec![list, caption(self.readout())])),
        ))
    }

    fn tag(&self) -> &'static str {
        "ReorderableStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; items)
    }
}

/// `PropertyGridStory`: two collapsible sections of labelled values.
pub struct PropertyGridStory {
    pub options: GalleryOptions,
    name: State<TextBuffer>,
}

impl PropertyGridStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            name: State::new(TextBuffer::new("Cove Studio")),
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
        let name_field = AnyView::new(
            TextField::new("Name", self.name.binding())
                .frame_width(180.0)
                .debug_name("property-grid-name"),
        );
        let sections = vec![
            PropertyGridSection::new(
                "general",
                "General",
                vec![
                    PropertyGridRow::new("name", "Name", name_field),
                    PropertyGridRow::new(
                        "kind",
                        "Kind",
                        AnyView::new(Text::new("Workspace")),
                    ),
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
        AnyView::new(StorySection::new(
            "Labelled values",
            "Each header toggles its section. Labels keep their column; long values wrap under the label when the pane narrows.",
            AnyView::new(TupleView::new(vec![grid, caption(self.readout())])),
        ))
    }

    fn tag(&self) -> &'static str {
        "PropertyGridStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; name)
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
            .label("Scroll position")
            .frame_height(Self::VIEWPORT_EXTENT)
            .debug_name("scrollbar-control"),
        );
        AnyView::new(StorySection::new(
            "Thumb",
            "Arrows move by one line, Page Up and Page Down by a viewport, Home and End jump to the bounds.",
            AnyView::new(TupleView::new(vec![bar, caption(self.readout())])),
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
        AnyView::new(StorySection::new(
            "Events",
            "Spacing is proportional to the gap between dates. The slider scales the whole track without reordering events.",
            AnyView::new(TupleView::new(vec![
                timeline,
                zoom,
                caption(self.readout()),
            ])),
        ))
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
