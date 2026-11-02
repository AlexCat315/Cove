//! Controls story: wheel selection.
//!
//! Swift source: `GalleryContent/Stories/Controls/WheelPickerStory.swift`.
//!
//! Three wheels (hour, minute, half-day) plus a volume wheel with a disabled top
//! row. The option tables and the committed-value label are the contract.

use cove_composition::Scope;
use cove_core::State;
use cove_ui_compose::modifiers::layout::FlexExt;
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::controls::Checkbox;
use cove_ui_compose::primitives::controls::fields::{
    WheelPicker, WheelPickerModel, WheelPickerOption,
};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::SemanticColorRef;
use cove_ui_compose::{AnyView, BoxView, TupleView, View};

use super::feedback::StorySection;
use crate::gallery_content::catalog::GalleryOptions;

/// The hour wheel lists 1…12, where 0 reads as 12.
pub fn hour_options() -> Vec<WheelPickerOption<i32>> {
    (0..12)
        .map(|hour| {
            WheelPickerOption::new(hour, format!("{:02}", if hour == 0 { 12 } else { hour }))
        })
        .collect()
}

/// The minute wheel steps in fives.
pub fn minute_options() -> Vec<WheelPickerOption<i32>> {
    (0..60)
        .step_by(5)
        .map(|minute| WheelPickerOption::new(minute, format!("{minute:02}")))
        .collect()
}

/// The half-day wheel.
pub fn period_options() -> Vec<WheelPickerOption<i32>> {
    vec![
        WheelPickerOption::new(0, "AM"),
        WheelPickerOption::new(1, "PM"),
    ]
}

/// The volume wheel, 0…10, with the top row disabled.
pub fn volume_options() -> Vec<WheelPickerOption<i32>> {
    (0..=10)
        .map(|level| WheelPickerOption::new(level, format!("{level}")).enabled(level != 10))
        .collect()
}

/// The time label the story's badge shows.
pub fn time_label(hour: i32, minute: i32, period: i32) -> String {
    let hour = if hour == 0 { 12 } else { hour };
    format!(
        "{hour:02}:{minute:02} {}",
        if period == 0 { "AM" } else { "PM" }
    )
}

/// `WheelPickerStory`: three time wheels and a volume wheel.
pub struct WheelPickerStory {
    pub options: GalleryOptions,
    hour: State<i32>,
    minute: State<i32>,
    period: State<i32>,
    volume: State<i32>,
    is_enabled: State<bool>,
}

impl WheelPickerStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            hour: State::new(9),
            minute: State::new(30),
            period: State::new(0),
            volume: State::new(6),
            is_enabled: State::new(true),
        }
    }

    pub fn hour(&self) -> i32 {
        self.hour.get()
    }

    pub fn minute(&self) -> i32 {
        self.minute.get()
    }

    pub fn period(&self) -> i32 {
        self.period.get()
    }

    pub fn volume(&self) -> i32 {
        self.volume.get()
    }

    pub fn is_enabled(&self) -> bool {
        self.is_enabled.get()
    }

    /// Restores the authored defaults, which is what the Reset button does.
    pub fn reset(&self) {
        self.hour.set(9);
        self.minute.set(30);
        self.period.set(0);
        self.volume.set(6);
    }

    /// Sets the wheels from a 24-hour clock reading.
    ///
    /// Minutes snap down to the nearest five, because the wheel only offers
    /// those; the hour folds into 1…12 and the half-day follows the original.
    pub fn set_time_24h(&self, hour: i32, minute: i32) {
        let hour12 = hour % 12;
        self.hour.set(if hour12 == 0 { 12 } else { hour12 });
        self.minute.set((minute / 5) * 5);
        self.period.set(if hour >= 12 { 1 } else { 0 });
    }

    pub fn time_label(&self) -> String {
        time_label(self.hour.get(), self.minute.get(), self.period.get())
    }

    pub fn hour_wheel(&self) -> WheelPickerModel<i32> {
        WheelPickerModel::new(hour_options(), Some(self.hour.get()))
    }

    pub fn minute_wheel(&self) -> WheelPickerModel<i32> {
        WheelPickerModel::new(minute_options(), Some(self.minute.get()))
    }

    pub fn period_wheel(&self) -> WheelPickerModel<i32> {
        WheelPickerModel::new(period_options(), Some(self.period.get()))
    }

    pub fn volume_wheel(&self) -> WheelPickerModel<i32> {
        WheelPickerModel::new(volume_options(), Some(self.volume.get()))
    }

    fn wheel_column(
        &self,
        title: &str,
        selection: cove_core::Binding<i32>,
        options: Vec<WheelPickerOption<i32>>,
    ) -> AnyView {
        let enabled = self.options.is_enabled && self.is_enabled.get();
        AnyView::new(
            AnyView::new(
                WheelPicker::new(selection, options)
                    .enabled(enabled)
                    .label(title.to_owned()),
            )
            .flex(1.0),
        )
    }
}

impl View for WheelPickerStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let enabled = self.options.is_enabled;

        let badge = Text::new(self.time_label())
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::info());

        let header = AnyView::new(BoxView::new(
            cove_core::scene::FlexDirection::Row,
            cove_core::scene::Align::Center,
            cove_core::scene::Justify::FlexStart,
            cove_core::scene::Wrap::NoWrap,
            8.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(
                    Text::new("Start time")
                        .semantic_font(cove_ui_compose::theme::SemanticFontRef::body_strong()),
                ),
                AnyView::new(AnyView::new(Text::new("")).flex(1.0)),
                AnyView::new(badge),
            ])),
        ));

        let toggle = AnyView::new(BoxView::new(
            cove_core::scene::FlexDirection::Row,
            cove_core::scene::Align::Center,
            cove_core::scene::Justify::FlexStart,
            cove_core::scene::Wrap::NoWrap,
            8.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(Checkbox::from_boolean(self.is_enabled.binding()).enabled(enabled)),
                AnyView::new(
                    Text::new("Enabled")
                        .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
                        .semantic_foreground(SemanticColorRef::on_surface_muted()),
                ),
            ])),
        ));

        let wheels = AnyView::new(BoxView::new(
            cove_core::scene::FlexDirection::Row,
            cove_core::scene::Align::Center,
            cove_core::scene::Justify::FlexStart,
            cove_core::scene::Wrap::NoWrap,
            8.0,
            AnyView::new(TupleView::new(vec![
                self.wheel_column("Hour", self.hour.binding(), hour_options()),
                self.wheel_column("Minute", self.minute.binding(), minute_options()),
                self.wheel_column("Period", self.period.binding(), period_options()),
                self.wheel_column("Volume", self.volume.binding(), volume_options()),
            ])),
        ));

        AnyView::new(StorySection::new(
            "Wheel selection",
            "Scroll, drag, click a row or use arrow keys / Home / End. The center band marks the committed value; disabled rows are skipped.",
            AnyView::new(TupleView::new(vec![toggle, header, wheels])),
        ))
    }

    fn tag(&self) -> &'static str {
        "WheelPickerStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; hour, minute, period, volume, is_enabled)
    }
}
