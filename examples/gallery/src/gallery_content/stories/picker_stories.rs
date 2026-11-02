//! Picker and small-control stories: dates, times, split buttons, key caps,
//! links, icons and menu buttons.
//!
//! Swift sources: `GalleryContent/Stories/Data/DateTableStories.swift`,
//! `GalleryContent/Stories/Controls/ControlStories.swift` and
//! `GalleryContent/Stories/Layout/NavigationStories.swift`.

use std::rc::Rc;

use cove_composition::Scope;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_core::State;
use cove_ui_compose::foundation::{Calendar, Date};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::controls::date_values::TimeOfDay;
use cove_ui_compose::primitives::controls::fields::date_picker::{
    DatePicker, DatePickerOptions, DatePickerPresentation,
};
use cove_ui_compose::primitives::controls::fields::date_range::{
    DateRange, DateRangePicker, DateRangePickerOptions,
};
use cove_ui_compose::primitives::controls::fields::time_field::{TimeField, TimeFieldOptions};
use cove_ui_compose::primitives::controls::key_cap::KeyCap;
use cove_ui_compose::primitives::controls::link::{Link, LinkUnderline};
use cove_ui_compose::primitives::controls::menu_button::MenuButtonTitle;
use cove_ui_compose::primitives::controls::split_button::SplitButton;
use cove_ui_compose::primitives::media::Icon;
use cove_ui_compose::primitives::media::ui_icon_catalog as icons;
use cove_ui_compose::primitives::overlays::menu::{MenuEntry, MenuItem};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::SemanticFontRef;
use cove_ui_compose::theme::tokens::{SemanticColorRef, SemanticIconSize, SemanticStyleViewExt};
use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView, View};

use super::feedback::StorySection;
use crate::gallery_content::catalog::GalleryOptions;

/// The story's reference day, 2026-11-02.
pub fn reference_day() -> Date {
    Date::from_epoch_day(20_759)
}

/// `YYYY-MM-DD`, or the empty-state label.
pub fn date_label(date: Option<Date>) -> String {
    match date {
        Some(date) => {
            let parts = Calendar::new().parts(date);
            format!("{:04}-{:02}-{:02}", parts.year, parts.month, parts.day)
        }
        None => "No date".to_owned(),
    }
}

/// `HH:MM`, or the empty-state label.
pub fn time_label(time: Option<TimeOfDay>) -> String {
    match time {
        Some(time) => format!("{:02}:{:02}", time.hour, time.minute),
        None => "No time".to_owned(),
    }
}

fn caption(text: String) -> AnyView {
    AnyView::new(
        Text::new(text)
            .semantic_font(SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted()),
    )
}

/// `DatePickerStory`: one day out of a month.
pub struct DatePickerStory {
    pub options: GalleryOptions,
    selection: State<Option<Date>>,
}

impl DatePickerStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            selection: State::new(Some(reference_day())),
        }
    }

    pub fn selection(&self) -> Option<Date> {
        self.selection.get()
    }

    pub fn set_selection(&self, date: Option<Date>) {
        self.selection.set(date);
    }

    pub fn readout(&self) -> String {
        date_label(self.selection.get())
    }
}

impl View for DatePickerStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let mut options = DatePickerOptions::new();
        options.presentation = DatePickerPresentation {
            placeholder: "Choose a day".to_owned(),
            is_clearable: true,
            is_enabled: self.options.is_enabled,
            width: 200.0,
        };
        options.calendar.calendar.set_first_weekday(2);
        let selection = self.selection.clone();
        let read = Rc::new(move || selection.get());
        let write = {
            let selection = self.selection.clone();
            Rc::new(move |date: Option<Date>| selection.set(date))
        };
        let field = AnyView::new(
            DatePicker::new(read, write, options)
                .frame_width(200.0)
                .debug_name("date-picker-control"),
        );
        AnyView::new(StorySection::new(
            "Single day",
            "The popover opens on the selected month. Page Up and Page Down change months; Return commits the focused day.",
            AnyView::new(TupleView::new(vec![field, caption(self.readout())])),
        ))
    }

    fn tag(&self) -> &'static str {
        "DatePickerStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; selection)
    }
}

/// `DateRangeStory`: two endpoints in one popover.
pub struct DateRangeStory {
    pub options: GalleryOptions,
    range: State<Option<DateRange>>,
}

impl DateRangeStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            range: State::new(Some(DateRange::new(
                reference_day(),
                reference_day().adding_seconds(6 * Date::SECONDS_PER_DAY),
            ))),
        }
    }

    pub fn range(&self) -> Option<DateRange> {
        self.range.get()
    }

    pub fn set_range(&self, range: Option<DateRange>) {
        self.range.set(range);
    }

    pub fn readout(&self) -> String {
        match self.range.get() {
            Some(range) => format!(
                "{} → {}",
                date_label(Some(range.lower)),
                date_label(Some(range.upper))
            ),
            None => "No range".to_owned(),
        }
    }
}

impl View for DateRangeStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let mut options = DateRangePickerOptions::new();
        options.calendar.calendar.set_first_weekday(2);
        options.presentation.is_enabled = self.options.is_enabled;
        let field = AnyView::new(
            DateRangePicker::new(self.range.binding(), options)
                .frame_width(220.0)
                .debug_name("date-range-control"),
        );
        AnyView::new(StorySection::new(
            "Range",
            "The first click sets the start, the second sets the end. Choosing an earlier end swaps the endpoints.",
            AnyView::new(TupleView::new(vec![field, caption(self.readout())])),
        ))
    }

    fn tag(&self) -> &'static str {
        "DateRangeStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; range)
    }
}

/// `TimeFieldStory`: a segmented time entry.
pub struct TimeFieldStory {
    pub options: GalleryOptions,
    value: State<Option<TimeOfDay>>,
}

impl TimeFieldStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            value: State::new(Some(TimeOfDay::new(9, 30, 0))),
        }
    }

    pub fn value(&self) -> Option<TimeOfDay> {
        self.value.get()
    }

    pub fn set_value(&self, value: Option<TimeOfDay>) {
        self.value.set(value);
    }

    pub fn readout(&self) -> String {
        time_label(self.value.get())
    }
}

impl View for TimeFieldStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let options = TimeFieldOptions {
            show_seconds: false,
            is_enabled: self.options.is_enabled,
            ..TimeFieldOptions::default()
        };
        let field = AnyView::new(
            TimeField::new(self.value.binding(), options)
                .frame_width(140.0)
                .debug_name("time-field-control"),
        );
        AnyView::new(StorySection::new(
            "Segmented entry",
            "Each segment accepts digits and clamps on commit. Up and Down step the focused segment.",
            AnyView::new(TupleView::new(vec![field, caption(self.readout())])),
        ))
    }

    fn tag(&self) -> &'static str {
        "TimeFieldStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; value)
    }
}

fn sample_entries(clicks: State<u64>) -> Vec<MenuEntry> {
    let one = {
        let clicks = clicks.clone();
        MenuItem::new(1, "Publish now", move || clicks.update(|count| *count += 1))
    };
    let two = {
        let clicks = clicks.clone();
        MenuItem::new(2, "Schedule", move || clicks.update(|count| *count += 10))
    };
    vec![
        MenuEntry::Item(one),
        MenuEntry::Item(two),
        MenuEntry::Item(MenuItem::new(3, "Unavailable", || {}).enabled(false)),
    ]
}

/// `SplitButtonStory`: a primary action plus a menu.
pub struct SplitButtonStory {
    pub options: GalleryOptions,
    clicks: State<u64>,
}

impl SplitButtonStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            clicks: State::new(0),
        }
    }

    pub fn clicks(&self) -> u64 {
        self.clicks.get()
    }

    pub fn readout(&self) -> String {
        format!("{} actions", self.clicks.get())
    }
}

impl View for SplitButtonStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let primary = {
            let clicks = self.clicks.clone();
            move || clicks.update(|count| *count += 1)
        };
        let button = AnyView::new(
            SplitButton::new("Publish", sample_entries(self.clicks.clone()), primary)
                .enabled(self.options.is_enabled)
                .debug_name("split-button-control"),
        );
        AnyView::new(StorySection::new(
            "Primary and menu",
            "Return runs the primary action. Down opens the menu; the disabled entry is skipped.",
            AnyView::new(TupleView::new(vec![button, caption(self.readout())])),
        ))
    }

    fn tag(&self) -> &'static str {
        "SplitButtonStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; clicks)
    }
}

/// `KeyCapStory`: shortcut glyphs at every length.
pub struct KeyCapStory {
    pub options: GalleryOptions,
}

impl KeyCapStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self { options }
    }
}

impl View for KeyCapStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let rows = vec![
            AnyView::new(TupleView::new(vec![
                AnyView::new(KeyCap::new(["⌘", "⇧", "P"])),
                caption("Primary + shift".to_owned()),
            ])),
            AnyView::new(TupleView::new(vec![
                AnyView::new(KeyCap::title("Esc")),
                caption("Single key".to_owned()),
            ])),
            AnyView::new(TupleView::new(vec![
                AnyView::new(KeyCap::new(["Ctrl", "Alt", "Delete"])),
                caption("Long labels keep their order".to_owned()),
            ])),
        ];
        AnyView::new(StorySection::new(
            "Shortcut glyphs",
            "Caps are one line high; the spoken form lists the keys in the same order.",
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::FlexStart,
                Justify::FlexStart,
                Wrap::NoWrap,
                10.0,
                AnyView::new(TupleView::new(rows)),
            )),
        ))
    }

    fn tag(&self) -> &'static str {
        "KeyCapStory"
    }
}

/// `LinkStory`: inline navigation with underline variants.
pub struct LinkStory {
    pub options: GalleryOptions,
    opened: State<u64>,
}

impl LinkStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            opened: State::new(0),
        }
    }

    pub fn opened(&self) -> u64 {
        self.opened.get()
    }

    pub fn readout(&self) -> String {
        format!("{} opens", self.opened.get())
    }
}

impl View for LinkStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let enabled = self.options.is_enabled;
        let make = |title: &'static str, underline: LinkUnderline| {
            let opened = self.opened.clone();
            AnyView::new(
                Link::new(Text::new(title), move || opened.update(|count| *count += 1))
                    .underline(underline)
                    .enabled(enabled),
            )
        };
        let body_copy = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::Wrap,
            6.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(Text::new("Read the")),
                make("release notes", LinkUnderline::Hover),
                AnyView::new(Text::new("before shipping.")),
            ])),
        ));
        let hover = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::Wrap,
            18.0,
            AnyView::new(TupleView::new(vec![
                make("Underlined on hover", LinkUnderline::Hover),
                make("Always underlined", LinkUnderline::Always),
            ])),
        ));
        let never = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::Wrap,
            18.0,
            AnyView::new(TupleView::new(vec![
                make("Never underlined", LinkUnderline::Never),
                make("External", LinkUnderline::Hover),
            ])),
        ));
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                24.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(StorySection::new(
                        "Inside body copy",
                        "A link sits in the sentence it belongs to and keeps the text baseline.",
                        body_copy,
                    )),
                    AnyView::new(StorySection::new(
                        "Underline on hover",
                        "The underline appears while the pointer is over the link and returns on keyboard focus.",
                        hover,
                    )),
                    AnyView::new(StorySection::new(
                        "Never underlined",
                        "Chrome-free links keep the link role and colour without a rule.",
                        AnyView::new(TupleView::new(vec![never, caption(self.readout())])),
                    )),
                ])),
            )
            .debug_name("link-story"),
        )
    }

    fn tag(&self) -> &'static str {
        "LinkStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; opened)
    }
}

/// One icon resolved through a semantic size role, with its role name below.
fn sizing_tile(size: SemanticIconSize, name: &'static str) -> AnyView {
    AnyView::new(BoxView::new(
        FlexDirection::Column,
        Align::Center,
        Justify::FlexEnd,
        Wrap::NoWrap,
        4.0,
        AnyView::new(TupleView::new(vec![
            AnyView::new(Icon::semantic(icons::action::search(), size)),
            AnyView::new(
                Text::new(name)
                    .semantic_font(SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
            ),
        ])),
    ))
}

/// One glyph tinted with a semantic role.
fn semantic_tile(
    resource: cove_imaging::BundleImageResource,
    color: SemanticColorRef,
    name: &'static str,
) -> AnyView {
    AnyView::new(BoxView::new(
        FlexDirection::Column,
        Align::Center,
        Justify::FlexEnd,
        Wrap::NoWrap,
        4.0,
        AnyView::new(TupleView::new(vec![
            AnyView::new(Icon::semantic_with_ref(
                resource,
                SemanticIconSize::medium(),
                color,
            )),
            AnyView::new(
                Text::new(name)
                    .semantic_font(SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
            ),
        ])),
    ))
}

/// `IconStory`: the bundled set at the sizes the gallery uses.
pub struct IconStory {
    pub options: GalleryOptions,
}

impl IconStory {
    /// How many icons the grid shows.
    pub const VISIBLE: usize = 24;

    pub fn new(options: GalleryOptions) -> Self {
        Self { options }
    }

    /// The icons the grid renders, in catalog order.
    pub fn entries() -> Vec<cove_ui_compose::primitives::media::UIIconEntry> {
        icons::all().into_iter().take(Self::VISIBLE).collect()
    }
}

impl View for IconStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let size = if self.options.is_enabled { 24.0 } else { 16.0 };
        let tiles = Self::entries()
            .into_iter()
            .map(|entry| {
                AnyView::new(BoxView::new(
                    FlexDirection::Column,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    4.0,
                    AnyView::new(TupleView::new(vec![
                        AnyView::new(Icon::new(entry.resource.clone(), size)),
                        AnyView::new(
                            Text::new(entry.name.clone())
                                .semantic_font(SemanticFontRef::caption())
                                .semantic_foreground(SemanticColorRef::on_surface_muted()),
                        ),
                    ])),
                ))
            })
            .collect();
        let catalog = AnyView::new(StorySection::new(
            "Catalog",
            "Every glyph is a 24 × 24 alpha mask and inherits the current foreground colour.",
            AnyView::new(BoxView::new(
                FlexDirection::Row,
                Align::FlexStart,
                Justify::FlexStart,
                Wrap::Wrap,
                20.0,
                AnyView::new(TupleView::new(tiles)),
            )),
        ));

        let sizing = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::Wrap,
            20.0,
            AnyView::new(TupleView::new(vec![
                sizing_tile(SemanticIconSize::small(), "small"),
                sizing_tile(SemanticIconSize::medium(), "medium"),
                sizing_tile(SemanticIconSize::large(), "large"),
            ])),
        ));

        let semantic = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::Wrap,
            20.0,
            AnyView::new(TupleView::new(vec![
                semantic_tile(icons::status::check_circle(), SemanticColorRef::success(), "success"),
                semantic_tile(icons::status::alert_triangle(), SemanticColorRef::warning(), "warning"),
                semantic_tile(icons::status::alert_circle(), SemanticColorRef::error(), "error"),
                semantic_tile(icons::status::info(), SemanticColorRef::info(), "info"),
            ])),
        ));

        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                24.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(StorySection::new(
                        "Accessible icon sizing",
                        "Semantic sizes follow the shared density scale instead of a fixed point value.",
                        sizing,
                    )),
                    AnyView::new(StorySection::new(
                        "Semantic SVG icons",
                        "Status glyphs carry the tone of the state they describe.",
                        semantic,
                    )),
                    catalog,
                ])),
            )
            .debug_name("icon-story"),
        )
    }

    fn tag(&self) -> &'static str {
        "IconStory"
    }
}

/// `MenuButtonStory`: a titled button that owns a menu.
pub struct MenuButtonStory {
    pub options: GalleryOptions,
    clicks: State<u64>,
}

impl MenuButtonStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            clicks: State::new(0),
        }
    }

    pub fn clicks(&self) -> u64 {
        self.clicks.get()
    }

    pub fn readout(&self) -> String {
        format!("{} menu commands", self.clicks.get())
    }
}

impl View for MenuButtonStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let entries = {
            let clicks = self.clicks.clone();
            vec![
                MenuEntry::Item(MenuItem::new(11, "Duplicate", {
                    let clicks = clicks.clone();
                    move || clicks.update(|count| *count += 1)
                })),
                MenuEntry::Item(MenuItem::new(12, "Move to…", {
                    let clicks = clicks.clone();
                    move || clicks.update(|count| *count += 2)
                })),
                MenuEntry::Separator { id: 13 },
                MenuEntry::Item(MenuItem::new(14, "Delete", move || {
                    clicks.update(|count| *count += 100)
                })),
            ]
        };
        let button = AnyView::new(
            MenuButtonTitle::new("Actions", entries)
                .enabled(self.options.is_enabled)
                .width(180.0),
        );
        AnyView::new(StorySection::new(
            "Menu button",
            "Return opens the menu, arrows move the highlight and Escape closes without a command.",
            AnyView::new(TupleView::new(vec![button, caption(self.readout())])),
        ))
    }

    fn tag(&self) -> &'static str {
        "MenuButtonStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; clicks)
    }
}

/// A plain button used by stories that need a reset affordance.
pub fn reset_button(title: &'static str, action: impl Fn() + 'static) -> AnyView {
    AnyView::new(Button::title(title, action))
}
