//! Field stories: sliders, text input, numeric, select and value-bearing fields.
//!
//! Swift sources: `GalleryContent/Stories/Controls/ControlStories.swift` and
//! `GalleryContent/Stories/Data/DataStories.swift`. Each story owns live state
//! and renders the real widget, so the gallery page is the control itself rather
//! than a description of it.

use super::showcase::{bounded, note, stack};
use cove_composition::Scope;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_core::{Binding, Color, State};
use cove_ui_compose::modifiers::interaction::AccessibilityExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::modifiers::style::AppearanceExt;
use cove_ui_compose::primitives::controls::fields::asset_ref_field::AssetRefField;
use cove_ui_compose::primitives::controls::fields::asset_ref_model::AssetRef;
use cove_ui_compose::primitives::controls::fields::color_field::ColorField;
use cove_ui_compose::primitives::controls::fields::color_wheel::{ColorWheel, ColorWheelStyle};
use cove_ui_compose::primitives::controls::fields::json_field::JsonField;
use cove_ui_compose::primitives::controls::fields::numeric::{
    NumberField, NumberFieldOptions, NumberFieldPrefix, NumericTextFormat,
};
use cove_ui_compose::primitives::controls::fields::range_slider::{RangeSlider, RangeValue};
use cove_ui_compose::primitives::controls::fields::search_field::SearchField;
use cove_ui_compose::primitives::controls::fields::select::{Select, SelectOption};
use cove_ui_compose::primitives::controls::fields::slider::Slider;
use cove_ui_compose::primitives::controls::fields::vec3_field::Vec3Field;
use cove_ui_compose::primitives::controls::fields::vec3_model::{Vec3, Vec3Constraints};
use cove_ui_compose::primitives::controls::form::Field;
use cove_ui_compose::primitives::layout::ResizableTextArea;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_field::TextField;
use cove_ui_compose::primitives::text::text_field::text_field_options::{
    TextFieldAxis, TextFieldBehavior, TextFieldDecoration, TextFieldLayout,
};
use cove_ui_compose::theme::styles::components::{
    AnyTextFieldStyle, EditorTextFieldStyle, TextFieldStyle, TextFieldStyleConfiguration,
};
use cove_ui_compose::theme::tokens::SemanticColorRef;
use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView, View};

use super::feedback::StorySection;
use crate::gallery_content::catalog::GalleryOptions;

/// A labelled field: the shared field shell (label, control, muted readout) so
/// gallery pages show the real form-field chrome rather than a bespoke stack.
fn field_row(label: &str, control: AnyView, readout: String) -> AnyView {
    AnyView::new(Field::new(label, label, control).configure(|presentation| {
        presentation.help = readout;
    }))
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
    ticks: State<f64>,
    vertical: State<f64>,
    editing: State<bool>,
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
            ticks: State::new(40.0),
            vertical: State::new(0.5),
            editing: State::new(false),
        }
    }

    pub fn value(&self) -> f64 {
        self.value.get()
    }

    /// Writes a value clamped to the slider's bounds.
    pub fn set_value(&self, value: f64) {
        self.value.set(value.clamp(Self::RANGE.0, Self::RANGE.1));
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
        let editing = self.editing.clone();
        let slider = AnyView::new(
            Slider::new(self.binding())
                .range(Self::RANGE)
                .on_editing_changed(move |active| editing.set(active))
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
                        "Drag, use arrow keys, or jump to an endpoint with Home and End.",
                        field_row("Opacity", slider, format!("{}{}", self.readout(), if self.editing.get() { " · Editing" } else { "" })),
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
                        "Ticks",
                        "Discrete positions share one step for pointer and keyboard input.",
                        field_row("Level", AnyView::new(Slider::new(self.ticks.binding()).range((0.0, 100.0)).step(10.0).shows_ticks(true).enabled(self.options.is_enabled).frame_width(240.0).debug_name("slider-ticks")), format!("{:.0}", self.ticks.get())),
                    )),
                    AnyView::new(StorySection::new(
                        "Vertical",
                        "Higher values appear at the top.",
                        field_row("Gain", AnyView::new(Slider::new(self.vertical.binding()).axis(cove_ui_compose::primitives::controls::fields::SliderAxis::Vertical).enabled(self.options.is_enabled).frame_size(32.0, 160.0).debug_name("slider-vertical")), format!("{:.0}%", self.vertical.get() * 100.0)),
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
        cove_composition::dynamic_fields!(self; value, stepped, disabled_value, ticks, vertical, editing)
    }
}

/// `RangeSliderStory`: two thumbs over the same scale.
pub struct RangeSliderStory {
    pub options: GalleryOptions,
    values: State<RangeValue>,
    stepped: State<RangeValue>,
    bounded: State<RangeValue>,
    disabled: State<RangeValue>,
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
            stepped: State::new(RangeValue::new(20.0, 80.0)),
            bounded: State::new(RangeValue::new(20.0, 65.0)),
            disabled: State::new(RangeValue::new(30.0, 70.0)),
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
        let lower = lower.min(Self::RANGE.1 - Self::MINIMUM_SPAN);
        let upper = upper.max(lower + Self::MINIMUM_SPAN).min(Self::RANGE.1);
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
        AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            24.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(StorySection::new(
                    "Range",
                    "Two independently focusable endpoints.",
                    field_row("Visible range", slider, self.readout()),
                )),
                AnyView::new(StorySection::new(
                    "Stepped",
                    "Endpoints move in increments of ten.",
                    field_row(
                        "Range",
                        AnyView::new(
                            RangeSlider::new(self.stepped.binding())
                                .range(Self::RANGE)
                                .step(10.0)
                                .enabled(self.options.is_enabled)
                                .frame_width(240.0)
                                .debug_name("range-slider-stepped"),
                        ),
                        format!(
                            "{:.0} – {:.0}",
                            self.stepped.get().lower,
                            self.stepped.get().upper
                        ),
                    ),
                )),
                AnyView::new(StorySection::new(
                    "Minimum span",
                    "Keep at least twenty units between endpoints.",
                    field_row(
                        "Range",
                        AnyView::new(
                            RangeSlider::new(self.bounded.binding())
                                .range(Self::RANGE)
                                .minimum_span(20.0)
                                .enabled(self.options.is_enabled)
                                .frame_width(240.0)
                                .debug_name("range-slider-bounded"),
                        ),
                        format!(
                            "{:.0} – {:.0}",
                            self.bounded.get().lower,
                            self.bounded.get().upper
                        ),
                    ),
                )),
                AnyView::new(StorySection::new(
                    "Disabled",
                    "Values remain visible without accepting input.",
                    field_row(
                        "Range",
                        AnyView::new(
                            RangeSlider::new(self.disabled.binding())
                                .range(Self::RANGE)
                                .enabled(false)
                                .frame_width(240.0)
                                .debug_name("range-slider-disabled"),
                        ),
                        "30 – 70".into(),
                    ),
                )),
            ])),
        ))
    }

    fn tag(&self) -> &'static str {
        "RangeSliderStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; values, stepped, bounded, disabled)
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
    history: State<TextBuffer>,
    styled: State<TextBuffer>,
}

struct EditingFieldStyle;

impl TextFieldStyle for EditingFieldStyle {
    fn make_body(&self, configuration: TextFieldStyleConfiguration) -> AnyView {
        let inputs = configuration.theme.inputs;
        let background = if !configuration.is_enabled {
            inputs.background_disabled
        } else if configuration.is_editing {
            configuration.theme.colors.focus_ring.multiplied_alpha(0.08)
        } else {
            inputs.background
        };
        let border = if !configuration.is_enabled {
            inputs.border_disabled
        } else if configuration.is_focused {
            inputs.border_focused
        } else {
            inputs.border_color
        };
        AnyView::new(
            configuration
                .content
                .background_color(background)
                .border_color(
                    border,
                    if configuration.is_focused {
                        inputs.focus_ring_width
                    } else {
                        inputs.border_width
                    },
                )
                .corner_radius(inputs.radius),
        )
    }
}

impl TextFieldStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            text: State::new(TextBuffer::new("Cove")),
            secret: State::new(TextBuffer::new("correct horse")),
            locked: State::new(TextBuffer::new("Read only value")),
            limited: State::new(TextBuffer::new("cove-sdk")),
            unicode: State::new(TextBuffer::new("Cove e\u{301} 👩🏽‍💻 🇨🇳")),
            history: State::new(TextBuffer::new(
                "First draft line\nSecond line with more words\nThird line\nFourth line",
            )),
            styled: State::new(TextBuffer::new("Draft")),
        }
    }

    pub fn text(&self) -> String {
        self.text.get().string_value()
    }

    pub fn set_text(&self, value: impl AsRef<str>) {
        self.text.set(TextBuffer::new(value.as_ref()));
    }

    pub fn readout(&self) -> String {
        let text = self.text.get();
        if text.is_empty() {
            "Empty".to_owned()
        } else {
            format!("{} characters", text.character_count())
        }
    }
}

impl View for TextFieldStory {
    fn body(&self, _cx: &Scope) -> AnyView {
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
                        "Type, select or clear text to update the bound value.",
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
                                        self.secret.get().character_count()
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
                                self.limited.get().character_count()
                            ),
                        ),
                    )),
                    AnyView::new(StorySection::new(
                        "Navigation & history",
                        "Primary-Z undoes a typing run and Primary-Shift-Z redoes it. Option-arrows move by word, Command-arrows by line, PageUp/PageDown by viewport; Home and End stop on the displayed row.",
                        field_row(
                            "Draft",
                            AnyView::new(
                                TextField::new("Release notes", self.history.binding())
                                    .layout(TextFieldLayout {
                                        axis: TextFieldAxis::Vertical,
                                        max_visible_lines: 4,
                                        ..TextFieldLayout::default()
                                    })
                                    .behavior(TextFieldBehavior {
                                        clearable: true,
                                        disabled,
                                        ..TextFieldBehavior::new()
                                    })
                                    .frame_width(320.0)
                                    .debug_name("text-field-history"),
                            ),
                            format!(
                                "{} lines · {} characters",
                                self.history.get().line_count(),
                                self.history.get().character_count()
                            ),
                        ),
                    )),
                    AnyView::new(StorySection::new(
                        "Styles",
                        "Focus and editing update the chrome while keeping the input and selection.",
                        field_grid(vec![
                            field_row("Editing highlight", AnyView::new(TextField::new("Styled text", self.styled.binding())
                                .behavior(TextFieldBehavior { disabled, ..Default::default() })
                                .style(AnyTextFieldStyle::new(EditingFieldStyle))
                                .frame_width(240.0).debug_name("text-field-styled")), format!("{} characters", self.styled.get().character_count())),
                            field_row("Editor", AnyView::new(TextField::new("Editor style", self.styled.binding())
                                .behavior(TextFieldBehavior { disabled, ..Default::default() })
                                .style(AnyTextFieldStyle::new(EditorTextFieldStyle))
                                .frame_width(240.0).debug_name("text-field-editor-style")), format!("{} characters", self.styled.get().character_count())),
                        ]),
                    )),
                    AnyView::new(StorySection::new(
                        "Unicode editing",
                        "Grapheme clusters, combining marks and emoji move as one caret step.",
                        field_row(
                            "Message",
                            unicode,
                            format!(
                                "{} characters",
                                self.unicode.get().character_count()
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
        cove_composition::dynamic_fields!(self; text, secret, locked, limited, unicode, history, styled)
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
            text: State::new(TextBuffer::new(
                "Multiline text wraps within the field.\nTry editing, selecting, pasting and scrolling.\nThe resize grip belongs to the surrounding container.",
            )),
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
        AnyView::new(StorySection::new(
            "Resizable text area",
            "The container controls height; the text field controls its own scrolling.",
            stack(vec![
                bounded(
                    AnyView::new(ResizableTextArea::new(
                        "Write a note…",
                        self.text.binding(),
                        90.0,
                        320.0,
                        !self.options.is_enabled,
                        "Editor height",
                        Default::default(),
                        None,
                        None,
                        None,
                    ))
                    .debug_name("text-area-control"),
                    520.0,
                ),
                note(format!("{} characters", self.text.get().character_count())),
            ]),
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
    localized: State<f32>,
}

impl NumberFieldStory {
    /// Field bounds.
    pub const RANGE: (f32, f32) = (0.0, 10.0);
    /// Stepper increment.
    pub const STEP: f32 = 0.25;

    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            value: State::new(std::f32::consts::PI),
            localized: State::new(1.25),
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
        let enabled = self.options.is_enabled;
        let numeric = || {
            NumberField::new(self.value.binding())
                .enabled(enabled)
                .range(Some(0.0), Some(10.0))
                .step(0.25)
        };
        stack(vec![
            AnyView::new(StorySection::new(
                "Numeric editing",
                "Invalid drafts revert on blur. Arrow keys step; Shift multiplies the step and Option divides it. Drag a prefix to scrub; double-click to reset.",
                stack(vec![
                    AnyView::new(
                        numeric()
                            .shows_stepper(true)
                            .frame_width(220.0)
                            .debug_name("number-field-control"),
                    ),
                    AnyView::new(
                        numeric()
                            .prefix(NumberFieldPrefix::new("W").tint(SemanticColorRef::accent()))
                            .reset_value(1.0)
                            .frame_width(220.0)
                            .debug_name("number-scrub"),
                    ),
                    AnyView::new(
                        NumberField::new(Binding::constant(0.0))
                            .enabled(enabled)
                            .mixed_value_label("Mixed value")
                            .frame_width(220.0),
                    ),
                    AnyView::new(
                        NumberField::new(Binding::constant(42.0))
                            .enabled(false)
                            .frame_width(220.0),
                    ),
                    note(format!("Committed value: {}", self.value())),
                ]),
            )),
            AnyView::new(StorySection::new(
                "Locale decimals",
                "The same value uses a comma or point. Localized digits and signs parse strictly; grouping and trailing junk are rejected.",
                stack(vec![
                    AnyView::new(
                        NumberField::new(self.localized.binding())
                            .enabled(enabled)
                            .step(0.25)
                            .with_format(NumericTextFormat::new(',', 2))
                            .accessibility_label("Comma decimal")
                            .debug_name("number-comma"),
                    ),
                    AnyView::new(
                        NumberField::new(self.localized.binding())
                            .enabled(enabled)
                            .with_format(NumericTextFormat::invariant(2))
                            .accessibility_label("Point decimal")
                            .debug_name("number-point"),
                    ),
                    AnyView::new(
                        note(format!("Numeric value: {}", self.localized.get()))
                            .debug_name("locale-number-value"),
                    ),
                ]),
            )),
        ])
    }

    fn tag(&self) -> &'static str {
        "NumberFieldStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; value, localized)
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
        ("fast", "Fast", true),
        ("balanced", "Balanced", true),
        (
            "high",
            "High quality · International collaboration and workspace permissions 👩🏽‍💻",
            true,
        ),
        ("offline", "Unavailable", false),
    ];

    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            selection: State::new("high".to_owned()),
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
        AnyView::new(StorySection::new(
            "Typed options",
            "Open with Space or Return, move with arrows, select with Return and dismiss with Escape.",
            stack(vec![
                bounded(
                    AnyView::new(
                        Select::new(self.selection.binding(), options)
                            .enabled(self.options.is_enabled)
                            .debug_name("select-control"),
                    ),
                    360.0,
                ),
                note(self.readout()),
            ]),
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
            color: State::new(Color::from_rgba8(88, 126, 231, 255)),
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
        let picker = AnyView::new(StorySection::new(
            "Color picker",
            "Open the swatch to pick a hue and saturation on the wheel, then edit RGB, alpha and hexadecimal values.",
            stack(vec![
                AnyView::new(
                    ColorField::new(self.color.binding())
                        .enabled(self.options.is_enabled)
                        .shows_inline_values(true)
                        .debug_name("color-field-control"),
                ),
                AnyView::new(
                    ColorField::new(self.color.binding())
                        .enabled(self.options.is_enabled)
                        .show_alpha(false)
                        .debug_name("color-field-rgb"),
                ),
                AnyView::new(
                    BoxView::new(
                        FlexDirection::Column,
                        Align::Stretch,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        0.0,
                        AnyView::new(cove_ui_compose::EmptyView),
                    )
                    .frame_size(200.0, 60.0)
                    .background_color(self.color())
                    .corner_radius(8.0)
                    .debug_name("color-preview"),
                ),
            ]),
        ));
        let wheel = AnyView::new(StorySection::new(
            "Wheel",
            "Ring: drag the ring for hue and the square for saturation and value. Disk: angle is hue and radius is saturation. Arrow keys step hue and value; the bound alpha is kept.",
            stack(vec![
                AnyView::new(BoxView::new(
                    FlexDirection::Row,
                    Align::FlexStart,
                    Justify::FlexStart,
                    Wrap::Wrap,
                    20.0,
                    AnyView::new(TupleView::new(vec![
                        AnyView::new(
                            ColorWheel::new(self.color.binding())
                                .enabled(self.options.is_enabled)
                                .debug_name("color-wheel-ring"),
                        ),
                        AnyView::new(
                            ColorWheel::new(self.color.binding())
                                .enabled(self.options.is_enabled)
                                .style(ColorWheelStyle::Disk)
                                .debug_name("color-wheel-disk"),
                        ),
                    ])),
                )),
                note(self.readout()),
            ]),
        ));
        stack(vec![picker, wheel])
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
            x: State::new(1.0),
            y: State::new(2.0),
            z: State::new(3.0),
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
        format!(
            "x {:.2} · y {:.2} · z {:.2}",
            self.x.get(),
            self.y.get(),
            self.z.get()
        )
    }
}

impl View for Vec3FieldStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let enabled = self.options.is_enabled;
        AnyView::new(StorySection::new(
            "Transform",
            "Drag an axis to scrub (Shift for coarse, Option for fine); double-click to reset. Right-click to copy, paste or reset. Narrow rows stack all three axes.",
            stack(vec![
                AnyView::new(
                    Vec3Field::new(self.x.binding(), self.y.binding(), self.z.binding())
                        .options(NumberFieldOptions::default().enabled(enabled).step(0.1))
                        .constraints(Vec3Constraints {
                            reset: Some(Vec3::new(0.0, 0.0, 0.0)),
                            ..Default::default()
                        })
                        .debug_name("vec3-transform"),
                ),
                bounded(
                    AnyView::new(
                        Vec3Field::new(self.x.binding(), self.y.binding(), self.z.binding())
                            .options(NumberFieldOptions::default().enabled(enabled))
                            .mixed_axes(["x".to_owned(), "z".to_owned()].into_iter().collect())
                            .debug_name("vec3-mixed"),
                    ),
                    240.0,
                ),
                note(format!(
                    "Position: {}, {}, {}",
                    self.x.get(),
                    self.y.get(),
                    self.z.get()
                )),
            ]),
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

/// `NumericStory`: single values and three-axis vectors on one page.
pub struct NumericStory {
    pub options: GalleryOptions,
}

impl NumericStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self { options }
    }
}

impl View for NumericStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        stack(vec![
            AnyView::new(NumberFieldStory::new(self.options.clone())),
            AnyView::new(Vec3FieldStory::new(self.options.clone())),
        ])
    }

    fn tag(&self) -> &'static str {
        "NumericStory"
    }
}

/// `TextFieldsStory`: single-line, search, multiline and JSON entry on one page.
pub struct TextFieldsStory {
    pub options: GalleryOptions,
}

impl TextFieldsStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self { options }
    }
}

impl View for TextFieldsStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        stack(vec![
            AnyView::new(TextFieldStory::new(self.options.clone())),
            AnyView::new(SearchFieldStory::new(self.options.clone())),
            AnyView::new(TextAreaStory::new(self.options.clone())),
            AnyView::new(JsonFieldStory::new(self.options.clone())),
        ])
    }

    fn tag(&self) -> &'static str {
        "TextFieldsStory"
    }
}

/// The two sliders side by side, as the controls page groups them.
pub fn slider_pair(slider: AnyView, range: AnyView) -> AnyView {
    field_grid(vec![slider, range])
}
