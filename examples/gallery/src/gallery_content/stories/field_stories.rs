//! Field stories: sliders, text input, numeric, select and value-bearing fields.
//!
//! Swift sources: `GalleryContent/Stories/Controls/ControlStories.swift` and
//! `GalleryContent/Stories/Data/DataStories.swift`. Each story owns live state
//! and renders the real widget, so the gallery page is the control itself rather
//! than a description of it.

use cove_composition::Scope;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_core::{Binding, Color, State};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::fields::asset_ref_field::AssetRefField;
use cove_ui_compose::primitives::controls::fields::asset_ref_model::AssetRef;
use cove_ui_compose::primitives::controls::fields::color_field::ColorField;
use cove_ui_compose::primitives::controls::fields::json_field::JsonField;
use cove_ui_compose::primitives::controls::fields::numeric::NumberField;
use cove_ui_compose::primitives::controls::fields::range_slider::{RangeSlider, RangeValue};
use cove_ui_compose::primitives::controls::fields::search_field::SearchField;
use cove_ui_compose::primitives::controls::fields::select::{Select, SelectOption};
use cove_ui_compose::primitives::controls::fields::slider::Slider;
use cove_ui_compose::primitives::controls::fields::vec3_field::Vec3Field;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_field::TextField;
use cove_ui_compose::primitives::text::text_field::text_field_options::{
    TextFieldBehavior, TextFieldDecoration,
};
use cove_ui_compose::theme::SemanticFontRef;
use cove_ui_compose::theme::tokens::{SemanticColorRef, SemanticStyleViewExt};
use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView, View};

use super::feedback::StorySection;
use crate::gallery_content::catalog::GalleryOptions;

/// A label, its control and a muted readout, stacked top to bottom.
fn field_row(label: &str, control: AnyView, readout: String) -> AnyView {
    AnyView::new(BoxView::new(
        FlexDirection::Column,
        Align::FlexStart,
        Justify::FlexStart,
        Wrap::NoWrap,
        6.0,
        AnyView::new(TupleView::new(vec![
            AnyView::new(Text::new(label).semantic_foreground(SemanticColorRef::on_surface())),
            control,
            AnyView::new(
                Text::new(readout)
                    .semantic_font(SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
            ),
        ])),
    ))
}

/// A row of fields that wraps when the pane narrows.
fn field_grid(fields: Vec<AnyView>) -> AnyView {
    AnyView::new(BoxView::new(
        FlexDirection::Row,
        Align::FlexStart,
        Justify::FlexStart,
        Wrap::Wrap,
        20.0,
        AnyView::new(TupleView::new(fields)),
    ))
}

/// `SliderStory`: one continuous value with a live readout.
pub struct SliderStory {
    pub options: GalleryOptions,
    value: State<f64>,
    stepped: State<f64>,
    disabled_value: State<f64>,
}

impl SliderStory {
    /// Slider bounds.
    pub const RANGE: (f64, f64) = (0.0, 1.0);
    /// Slider step.
    pub const STEP: f64 = 0.01;

    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            value: State::new(0.35),
            stepped: State::new(2.0),
            disabled_value: State::new(0.6),
        }
    }

    pub fn value(&self) -> f64 {
        self.value.get()
    }

    /// Writes a value clamped to the slider's bounds.
    pub fn set_value(&self, value: f64) {
        self.value
            .set(value.clamp(Self::RANGE.0, Self::RANGE.1));
    }

    pub fn readout(&self) -> String {
        format!("{:.0}%", self.value.get() * 100.0)
    }

    fn binding(&self) -> Binding<f64> {
        self.value.binding()
    }
}

impl View for SliderStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let slider = AnyView::new(
            Slider::new(self.binding())
                .range(Self::RANGE)
                .step(Self::STEP)
                .enabled(self.options.is_enabled)
                .frame_width(240.0)
                .debug_name("slider-control"),
        );
        let stepped = AnyView::new(
            Slider::new(self.stepped.binding())
                .range((0.0, 4.0))
                .step(0.5)
                .enabled(self.options.is_enabled)
                .frame_width(240.0)
                .debug_name("slider-stepped"),
        );
        let disabled = AnyView::new(
            Slider::new(self.disabled_value.binding())
                .range(Self::RANGE)
                .step(Self::STEP)
                .enabled(false)
                .frame_width(240.0)
                .debug_name("slider-disabled"),
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
                        "Continuous value",
                        "Drag the thumb or use the arrow keys. Home and End jump to the bounds; one step is 1%.",
                        field_row("Opacity", slider, self.readout()),
                    )),
                    AnyView::new(StorySection::new(
                        "Stepped",
                        "A step quantizes every write, so pointer and keyboard land on the same values.",
                        field_row(
                            "Priority",
                            stepped,
                            format!("{:.1} of 4.0", self.stepped.get()),
                        ),
                    )),
                    AnyView::new(StorySection::new(
                        "Disabled",
                        "The value stays readable while the control rejects input and leaves the Tab order.",
                        field_row(
                            "Volume",
                            disabled,
                            format!("{:.0}%", self.disabled_value.get() * 100.0),
                        ),
                    )),
                ])),
            )
            .debug_name("slider-story"),
        )
    }

    fn tag(&self) -> &'static str {
        "SliderStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; value)
    }
}

/// `RangeSliderStory`: two thumbs over the same scale.
pub struct RangeSliderStory {
    pub options: GalleryOptions,
    values: State<RangeValue>,
}

impl RangeSliderStory {
    /// Slider bounds.
    pub const RANGE: (f64, f64) = (0.0, 100.0);
    /// Slider step.
    pub const STEP: f64 = 1.0;
    /// The thumbs never cross.
    pub const MINIMUM_SPAN: f64 = 1.0;

    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            values: State::new(RangeValue::new(20.0, 65.0)),
        }
    }

    pub fn values(&self) -> RangeValue {
        self.values.get()
    }

    /// Writes a range, keeping the authored ordering and minimum span.
    pub fn set_values(&self, lower: f64, upper: f64) {
        let lower = lower.clamp(Self::RANGE.0, Self::RANGE.1);
        let upper = upper.clamp(Self::RANGE.0, Self::RANGE.1);
        let (lower, upper) = if lower <= upper {
            (lower, upper)
        } else {
            (upper, lower)
        };
        let upper = upper.max(lower + Self::MINIMUM_SPAN);
        self.values.set(RangeValue::new(lower, upper));
    }

    pub fn readout(&self) -> String {
        let values = self.values.get();
        format!("{:.0} – {:.0}", values.lower, values.upper)
    }
}

impl View for RangeSliderStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let slider = AnyView::new(
            RangeSlider::new(self.values.binding())
                .range(Self::RANGE)
                .step(Self::STEP)
                .minimum_span(Self::MINIMUM_SPAN)
                .enabled(self.options.is_enabled)
                .frame_width(240.0)
                .debug_name("range-slider-control"),
        );
        AnyView::new(StorySection::new(
            "Bounded span",
            "Each thumb is independently focusable. The upper thumb cannot pass the lower one plus the minimum span.",
            field_row("Visible range", slider, self.readout()),
        ))
    }

    fn tag(&self) -> &'static str {
        "RangeSliderStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; values)
    }
}

/// `TextFieldStory`: single-line entry with a live echo.
pub struct TextFieldStory {
    pub options: GalleryOptions,
    text: State<TextBuffer>,
    secret: State<TextBuffer>,
    locked: State<TextBuffer>,
    limited: State<TextBuffer>,
    unicode: State<TextBuffer>,
}

impl TextFieldStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            text: State::new(TextBuffer::new("Cove")),
            secret: State::new(TextBuffer::new("correct horse")),
            locked: State::new(TextBuffer::new("Read only value")),
            limited: State::new(TextBuffer::new("cove-sdk")),
            unicode: State::new(TextBuffer::new("Cove 输入法 🙂")),
        }
    }

    pub fn text(&self) -> String {
        self.text.get().string_value()
    }

    pub fn set_text(&self, value: impl AsRef<str>) {
        self.text.set(TextBuffer::new(value.as_ref()));
    }

    pub fn readout(&self) -> String {
        let text = self.text.get().string_value();
        if text.is_empty() {
            "Empty".to_owned()
        } else {
            format!("{} characters", text.chars().count())
        }
    }
}

impl View for TextFieldStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let field = AnyView::new(
            TextField::new("Project name", self.text.binding())
                .behavior(TextFieldBehavior {
                    disabled: !self.options.is_enabled,
                    ..TextFieldBehavior::new()
                })
                .frame_width(240.0)
                .debug_name("text-field-control"),
        );
        let disabled = !self.options.is_enabled;
        let secret = AnyView::new(
            TextField::new("Password", self.secret.binding())
                .behavior(TextFieldBehavior {
                    secure: true,
                    clearable: true,
                    disabled,
                    ..TextFieldBehavior::new()
                })
                .frame_width(240.0)
                .debug_name("text-field-secure"),
        );
        let locked = AnyView::new(
            TextField::new("Read only", self.locked.binding())
                .behavior(TextFieldBehavior {
                    read_only: true,
                    disabled,
                    ..TextFieldBehavior::new()
                })
                .frame_width(240.0)
                .debug_name("text-field-read-only"),
        );
        let limited = AnyView::new(
            TextField::new("Handle", self.limited.binding())
                .behavior(TextFieldBehavior {
                    max_length: Some(12),
                    clearable: true,
                    disabled,
                    ..TextFieldBehavior::new()
                })
                .decoration(TextFieldDecoration {
                    show_word_limit: true,
                    prefix: Some("@".to_string()),
                    ..TextFieldDecoration::default()
                })
                .frame_width(240.0)
                .debug_name("text-field-limited"),
        );
        let unicode = AnyView::new(
            TextField::new("Message", self.unicode.binding())
                .behavior(TextFieldBehavior {
                    clearable: true,
                    disabled,
                    ..TextFieldBehavior::new()
                })
                .frame_width(240.0)
                .debug_name("text-field-unicode"),
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
                        "Input & clear",
                        "Typing writes straight into the story's buffer. Selection, undo and the caret survive a rename of the placeholder.",
                        {
                            let field = AnyView::new(
                                TextField::new("Project name", self.text.binding())
                                    .behavior(TextFieldBehavior {
                                        clearable: true,
                                        disabled,
                                        ..TextFieldBehavior::new()
                                    })
                                    .frame_width(240.0)
                                    .debug_name("text-field-control"),
                            );
                            field_row("Project name", field, self.readout())
                        },
                    )),
                    AnyView::new(StorySection::new(
                        "Password & read-only",
                        "A secure field never exposes its text; a read-only field keeps its selection and caret.",
                        AnyView::new(BoxView::new(
                            FlexDirection::Row,
                            Align::FlexStart,
                            Justify::FlexStart,
                            Wrap::Wrap,
                            16.0,
                            AnyView::new(TupleView::new(vec![
                                field_row(
                                    "Password",
                                    secret,
                                    format!(
                                        "{} characters",
                                        self.secret.get().string_value().chars().count()
                                    ),
                                ),
                                field_row(
                                    "Read only",
                                    locked,
                                    self.locked.get().string_value(),
                                ),
                            ])),
                        )),
                    )),
                    AnyView::new(StorySection::new(
                        "Decorations & limits",
                        "A prefix and a word limit decorate the field; the buffer stops at the limit.",
                        field_row(
                            "Handle",
                            limited,
                            format!(
                                "{} / 12",
                                self.limited.get().string_value().chars().count()
                            ),
                        ),
                    )),
                    AnyView::new(StorySection::new(
                        "Unicode editing",
                        "Grapheme clusters, combining marks and emoji move as one caret step.",
                        field_row(
                            "Message",
                            unicode,
                            format!(
                                "{} characters",
                                self.unicode.get().string_value().chars().count()
                            ),
                        ),
                    )),
                ])),
            )
            .debug_name("text-field-story"),
        )
    }

    fn tag(&self) -> &'static str {
        "TextFieldStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; text)
    }
}

/// `SearchFieldStory`: query entry with suggestions.
pub struct SearchFieldStory {
    pub options: GalleryOptions,
    query: State<TextBuffer>,
    submitted: State<String>,
}

impl SearchFieldStory {
    /// The suggestions the field offers.
    pub const SUGGESTIONS: [&'static str; 3] = ["accordion", "avatar", "badge"];

    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            query: State::new(TextBuffer::empty()),
            submitted: State::new(String::new()),
        }
    }

    pub fn query(&self) -> String {
        self.query.get().string_value()
    }

    pub fn submitted(&self) -> String {
        self.submitted.get()
    }

    pub fn readout(&self) -> String {
        let submitted = self.submitted.get();
        if submitted.is_empty() {
            "No search yet".to_owned()
        } else {
            format!("Searched “{submitted}”")
        }
    }
}

impl View for SearchFieldStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let submitted = self.submitted.clone();
        let field = AnyView::new(
            SearchField::new(self.query.binding())
                .placeholder("Search components")
                .suggestions(Self::SUGGESTIONS.iter().map(|s| (*s).to_owned()).collect())
                .enabled(self.options.is_enabled)
                .on_submit({
                    let submitted = submitted.clone();
                    move |value| submitted.set(value)
                })
                .on_suggestion_selected(move |value| submitted.set(value))
                .frame_width(240.0)
                .debug_name("search-field-control"),
        );
        AnyView::new(StorySection::new(
            "Query",
            "Return submits the current text; clicking a suggestion submits that suggestion. Escape clears the query.",
            field_row("Search", field, self.readout()),
        ))
    }

    fn tag(&self) -> &'static str {
        "SearchFieldStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; query, submitted)
    }
}

/// `TextAreaStory`: the multiline form of the text field.
pub struct TextAreaStory {
    pub options: GalleryOptions,
    text: State<TextBuffer>,
}

impl TextAreaStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            text: State::new(TextBuffer::new("First line\nSecond line")),
        }
    }

    pub fn text(&self) -> String {
        self.text.get().string_value()
    }

    pub fn readout(&self) -> String {
        format!("{} lines", self.text.get().string_value().lines().count())
    }
}

impl View for TextAreaStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let field = AnyView::new(
            TextField::new("Notes", self.text.binding())
                .multiline()
                .behavior(TextFieldBehavior {
                    disabled: !self.options.is_enabled,
                    ..TextFieldBehavior::new()
                })
                .frame_width(300.0)
                .debug_name("text-area-control"),
        );
        AnyView::new(StorySection::new(
            "Multiple lines",
            "Return inserts a newline instead of submitting. The field grows to its maximum visible lines and then scrolls.",
            field_row("Notes", field, self.readout()),
        ))
    }

    fn tag(&self) -> &'static str {
        "TextAreaStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; text)
    }
}

/// `NumberFieldStory`: a bounded numeric value with steppers.
pub struct NumberFieldStory {
    pub options: GalleryOptions,
    value: State<f32>,
}

impl NumberFieldStory {
    /// Field bounds.
    pub const RANGE: (f32, f32) = (0.0, 128.0);
    /// Stepper increment.
    pub const STEP: f32 = 1.0;

    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            value: State::new(24.0),
        }
    }

    pub fn value(&self) -> f32 {
        self.value.get()
    }

    /// Writes a value clamped to the field's bounds.
    pub fn set_value(&self, value: f32) {
        self.value.set(value.clamp(Self::RANGE.0, Self::RANGE.1));
    }

    pub fn readout(&self) -> String {
        format!("{:.1} pt", self.value.get())
    }
}

impl View for NumberFieldStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let field = AnyView::new(
            NumberField::new(self.value.binding())
                .decimals(1)
                .range(Some(Self::RANGE.0), Some(Self::RANGE.1))
                .step(Self::STEP)
                .shows_stepper(true)
                .enabled(self.options.is_enabled)
                .frame_width(140.0)
                .debug_name("number-field-control"),
        );
        AnyView::new(StorySection::new(
            "Bounded number",
            "Up and Down step by one. Typed values are clamped to 0…128 when the edit commits.",
            field_row("Corner radius", field, self.readout()),
        ))
    }

    fn tag(&self) -> &'static str {
        "NumberFieldStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; value)
    }
}

/// `SelectStory`: one choice out of a fixed list.
pub struct SelectStory {
    pub options: GalleryOptions,
    selection: State<String>,
}

impl SelectStory {
    /// The options, one of which is disabled.
    pub const OPTIONS: [(&'static str, &'static str, bool); 4] = [
        ("compact", "Compact", true),
        ("regular", "Regular", true),
        ("large", "Large", true),
        ("unavailable", "Unavailable", false),
    ];

    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            selection: State::new("regular".to_owned()),
        }
    }

    pub fn selection(&self) -> String {
        self.selection.get()
    }

    /// Selects an option unless it is disabled.
    pub fn select(&self, id: &str) -> bool {
        match Self::OPTIONS.iter().find(|(value, _, _)| *value == id) {
            Some((value, _, true)) => {
                self.selection.set((*value).to_owned());
                true
            }
            _ => false,
        }
    }

    pub fn readout(&self) -> String {
        format!("Selected: {}", self.selection.get())
    }
}

impl View for SelectStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let options = Self::OPTIONS
            .iter()
            .map(|(value, label, enabled)| SelectOption::new((*value).to_owned(), *label, *enabled))
            .collect();
        let select = AnyView::new(
            Select::new(self.selection.binding(), options)
                .enabled(self.options.is_enabled)
                .width(200.0)
                .max_visible_rows(4)
                .debug_name("select-control"),
        );
        AnyView::new(StorySection::new(
            "Fixed choices",
            "Return opens the menu, arrows move the highlight and Return commits. The disabled row is skipped.",
            field_row("Density", select, self.readout()),
        ))
    }

    fn tag(&self) -> &'static str {
        "SelectStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; selection)
    }
}

/// `ColorFieldStory`: colour with a channel readout.
pub struct ColorFieldStory {
    pub options: GalleryOptions,
    color: State<Color>,
}

impl ColorFieldStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            color: State::new(Color::from_rgba8(56, 132, 255, 255)),
        }
    }

    pub fn color(&self) -> Color {
        self.color.get()
    }

    pub fn set_color(&self, color: Color) {
        self.color.set(color);
    }

    pub fn readout(&self) -> String {
        let color = self.color.get();
        format!(
            "#{:02X}{:02X}{:02X}",
            (color.r * 255.0).round() as u8,
            (color.g * 255.0).round() as u8,
            (color.b * 255.0).round() as u8
        )
    }
}

impl View for ColorFieldStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let field = AnyView::new(
            ColorField::new(self.color.binding())
                .show_alpha(true)
                .shows_inline_channels(true)
                .enabled(self.options.is_enabled)
                .debug_name("color-field-control"),
        );
        AnyView::new(StorySection::new(
            "Accent colour",
            "The swatch opens the channel editor. Hex entry accepts six or eight digits.",
            field_row("Accent", field, self.readout()),
        ))
    }

    fn tag(&self) -> &'static str {
        "ColorFieldStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; color)
    }
}

/// `Vec3FieldStory`: three linked numeric fields.
pub struct Vec3FieldStory {
    pub options: GalleryOptions,
    x: State<f32>,
    y: State<f32>,
    z: State<f32>,
}

impl Vec3FieldStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            x: State::new(0.0),
            y: State::new(1.0),
            z: State::new(0.0),
        }
    }

    pub fn value(&self) -> (f32, f32, f32) {
        (self.x.get(), self.y.get(), self.z.get())
    }

    pub fn set_value(&self, x: f32, y: f32, z: f32) {
        self.x.set(x);
        self.y.set(y);
        self.z.set(z);
    }

    pub fn readout(&self) -> String {
        format!("x {:.2} · y {:.2} · z {:.2}", self.x.get(), self.y.get(), self.z.get())
    }
}

impl View for Vec3FieldStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let field = AnyView::new(
            Vec3Field::new(self.x.binding(), self.y.binding(), self.z.binding())
                .decimals(2)
                .enabled(self.options.is_enabled)
                .debug_name("vec3-field-control"),
        );
        AnyView::new(StorySection::new(
            "Vector",
            "Tab moves across the axes. Copy and paste operate on the whole triple.",
            field_row("Direction", field, self.readout()),
        ))
    }

    fn tag(&self) -> &'static str {
        "Vec3FieldStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; x, y, z)
    }
}

/// `JsonFieldStory`: a validating document field.
pub struct JsonFieldStory {
    pub options: GalleryOptions,
    text: State<TextBuffer>,
    committed: State<String>,
}

impl JsonFieldStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            text: State::new(TextBuffer::new("{\"name\": \"Cove\"}")),
            committed: State::new(String::new()),
        }
    }

    pub fn text(&self) -> String {
        self.text.get().string_value()
    }

    pub fn committed(&self) -> String {
        self.committed.get()
    }

    pub fn readout(&self) -> String {
        let committed = self.committed.get();
        if committed.is_empty() {
            "Not committed".to_owned()
        } else {
            format!("Committed {} bytes", committed.len())
        }
    }
}

impl View for JsonFieldStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let committed = self.committed.clone();
        let field = AnyView::new(
            JsonField::new(self.text.binding())
                .placeholder("{\"key\": value}")
                .min_height(120.0)
                .enabled(self.options.is_enabled)
                .on_commit(move |buffer| committed.set(buffer.string_value()))
                .debug_name("json-field-control"),
        );
        AnyView::new(StorySection::new(
            "Document",
            "Editing keeps the draft. Commit normalises the formatting and reports the byte count.",
            field_row("Payload", field, self.readout()),
        ))
    }

    fn tag(&self) -> &'static str {
        "JsonFieldStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; text, committed)
    }
}

/// `AssetRefStory`: an asset slot with a picker.
pub struct AssetRefStory {
    pub options: GalleryOptions,
    value: State<Option<AssetRef>>,
}

impl AssetRefStory {
    /// The assets the picker offers.
    pub fn picker_assets() -> Vec<AssetRef> {
        vec![
            AssetRef::new("mesh.rock", "Rock", "Mesh").with_subtitle("2.4 MB"),
            AssetRef::new("material.sand", "Sand", "Material"),
            AssetRef::new("image.grain", "Grain", "Image").with_subtitle("512 × 512"),
        ]
    }

    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            value: State::new(Some(AssetRef::new("mesh.rock", "Rock", "Mesh"))),
        }
    }

    pub fn value(&self) -> Option<AssetRef> {
        self.value.get()
    }

    pub fn set_value(&self, asset: Option<AssetRef>) {
        self.value.set(asset);
    }

    pub fn readout(&self) -> String {
        match self.value.get() {
            Some(asset) => format!("{} · {}", asset.name, asset.kind),
            None => "Empty slot".to_owned(),
        }
    }
}

impl View for AssetRefStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let field = AnyView::new(
            AssetRefField::new(self.value.binding())
                .picker_options(Self::picker_assets())
                .placeholder("Drop a mesh")
                .enabled(self.options.is_enabled)
                .debug_name("asset-ref-control"),
        );
        AnyView::new(StorySection::new(
            "Asset slot",
            "Drop a compatible file or choose one from the picker. Clearing the slot returns it to the placeholder.",
            field_row("Source", field, self.readout()),
        ))
    }

    fn tag(&self) -> &'static str {
        "AssetRefStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; value)
    }
}

/// The two sliders side by side, as the controls page groups them.
pub fn slider_pair(slider: AnyView, range: AnyView) -> AnyView {
    field_grid(vec![slider, range])
}
