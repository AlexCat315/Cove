//! Control stories: buttons, checkboxes, toggles.
//!
//! Swift source: `GalleryContent/Stories/Controls/ControlStories.swift`
//! (`ButtonStory`, `CheckboxStory`, `ToggleStory`). The remaining stories in that
//! Swift file are listed in [`crate::gallery_content::routing::StorySource`]
//! because they need widgets that have no view component yet.

use cove_composition::Scope;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_core::{Binding, State};
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::controls::toggle::{Checkbox, CheckboxState, Toggle};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::styles::buttons::button_style::{AnyButtonStyle, ButtonStyleViewExt};
use cove_ui_compose::theme::styles::buttons::{
    DestructiveButtonStyle, GhostButtonStyle, SecondaryButtonStyle, StatusButtonStyle,
    ToggleButtonStyle,
};
use cove_ui_compose::theme::tokens::{SemanticColorRef, SemanticStyleViewExt};
use cove_ui_compose::{AnyView, BoxView, DynamicField, LayoutDebugExt, TupleView, View};

use crate::gallery_content::catalog::GalleryOptions;
use crate::gallery_content::stories::StorySection;

/// The button variants the story shows, in declaration order.
pub const BUTTON_VARIANTS: [&str; 7] = [
    "Primary",
    "Secondary",
    "Danger",
    "Warning",
    "Success",
    "Info",
    "Ghost",
];

/// `ButtonStory`.
pub struct ButtonStory {
    pub options: GalleryOptions,
    pub clicks: State<i32>,
    pub selected: State<bool>,
}

impl ButtonStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            clicks: State::new(0),
            selected: State::new(false),
        }
    }

    pub fn click(&self) {
        self.clicks.update(|clicks| *clicks += 1);
    }

    pub fn actions_line(&self) -> String {
        format!("Actions: {}", self.clicks.get())
    }

    /// The toggle variant's label, which depends on selection.
    pub fn select_label(&self) -> &'static str {
        if self.selected.get() {
            "Selected"
        } else {
            "Select me"
        }
    }
}

impl View for ButtonStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let enabled = self.options.is_enabled;

        // Variants: Primary keeps the ambient style; the rest override it with a
        // subtree provider, so each button renders through its own style.
        let primary_clicks = self.clicks.clone();
        let variants = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::Wrap,
            10.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(variant_button(
                    BUTTON_VARIANTS[0],
                    enabled,
                    &primary_clicks,
                    None,
                    Some("gallery-primary-button"),
                )),
                AnyView::new(variant_button(
                    BUTTON_VARIANTS[1],
                    enabled,
                    &primary_clicks,
                    Some(AnyButtonStyle::new(SecondaryButtonStyle)),
                    None,
                )),
                AnyView::new(variant_button(
                    BUTTON_VARIANTS[2],
                    enabled,
                    &primary_clicks,
                    Some(AnyButtonStyle::new(DestructiveButtonStyle)),
                    None,
                )),
                AnyView::new(variant_button(
                    BUTTON_VARIANTS[3],
                    enabled,
                    &primary_clicks,
                    Some(AnyButtonStyle::new(StatusButtonStyle::new(
                        cove_ui_compose::primitives::feedback::StatusTone::Warning,
                    ))),
                    None,
                )),
                AnyView::new(variant_button(
                    BUTTON_VARIANTS[4],
                    enabled,
                    &primary_clicks,
                    Some(AnyButtonStyle::new(StatusButtonStyle::new(
                        cove_ui_compose::primitives::feedback::StatusTone::Success,
                    ))),
                    None,
                )),
                AnyView::new(variant_button(
                    BUTTON_VARIANTS[5],
                    enabled,
                    &primary_clicks,
                    Some(AnyButtonStyle::new(StatusButtonStyle::new(
                        cove_ui_compose::primitives::feedback::StatusTone::Info,
                    ))),
                    None,
                )),
                AnyView::new(variant_button(
                    BUTTON_VARIANTS[6],
                    enabled,
                    &primary_clicks,
                    Some(AnyButtonStyle::new(GhostButtonStyle)),
                    None,
                )),
            ])),
        ));

        let states = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::NoWrap,
            12.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(Button::title("Installing…", || {}).loading(true)),
                AnyView::new(
                    Button::title("Disabled", || {})
                        .enabled(false)
                        .button_style(SecondaryButtonStyle),
                ),
                AnyView::new(
                    Button::title(self.select_label(), {
                        let selected = self.selected.clone();
                        let clicks = self.clicks.clone();
                        move || {
                            selected.update(|value| *value = !*value);
                            clicks.update(|value| *value += 1);
                        }
                    })
                    .enabled(enabled)
                    .selected(self.selected.get())
                    .button_style(ToggleButtonStyle::default()),
                ),
                AnyView::new(
                    Text::new(self.actions_line())
                        .semantic_foreground(SemanticColorRef::on_surface_muted()),
                ),
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
                        "Variants",
                        "Visual treatments communicate action priority.",
                        variants,
                    )),
                    AnyView::new(StorySection::new(
                        "States",
                        "Loading blocks duplicate activation; selected and disabled states remain distinguishable.",
                        states,
                    )),
                ])),
            )
            .debug_name("button-story"),
        )
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("clicks", &self.clicks),
            DynamicField::state("selected", &self.selected),
        ]
    }
}

/// Builds one variant button, optionally under a style provider.
fn variant_button(
    label: &'static str,
    enabled: bool,
    clicks: &State<i32>,
    style: Option<AnyButtonStyle>,
    debug: Option<&'static str>,
) -> AnyView {
    let build = |clicks: State<i32>| {
        Button::title(label, move || clicks.update(|value| *value += 1)).enabled(enabled)
    };
    match style {
        Some(style) => AnyView::new(build(clicks.clone()).button_style_any(style)),
        None => match debug {
            Some(name) => AnyView::new(build(clicks.clone()).debug_name(name)),
            None => AnyView::new(build(clicks.clone())),
        },
    }
}

/// `CheckboxStory`.
pub struct CheckboxStory {
    pub options: GalleryOptions,
    pub checked: State<bool>,
    pub mixed: State<CheckboxState>,
}

impl CheckboxStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            checked: State::new(false),
            mixed: State::new(CheckboxState::Mixed),
        }
    }

    /// Resets the partially selected collection back to mixed.
    pub fn reset_mixed(&self) {
        self.mixed.set(CheckboxState::Mixed);
    }

    /// The mixed checkbox's label, which embeds the state description.
    pub fn mixed_label(&self) -> String {
        format!("Partially selected collection: {:?}", self.mixed.get())
    }
}

impl View for CheckboxStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let enabled = self.options.is_enabled;
        let checked = self.checked.clone();
        let mixed = self.mixed.clone();
        let selection = AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            12.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(
                    Checkbox::labeled_from_boolean(
                        "Accept the terms",
                        Binding::new(
                            {
                                let checked = checked.clone();
                                move || checked.get()
                            },
                            move |value| checked.set(value),
                        ),
                    )
                    .enabled(enabled),
                ),
                AnyView::new(
                    Checkbox::labeled(
                        self.mixed_label(),
                        Binding::new(
                            {
                                let mixed = mixed.clone();
                                move || mixed.get()
                            },
                            {
                                let mixed = mixed.clone();
                                move |value| mixed.set(value)
                            },
                        ),
                    )
                    .enabled(enabled),
                ),
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
                        "Selection",
                        "Tab focuses the checkbox; Space and Return activate it. The label stays with the box and the full name is available to assistive clients.",
                        selection,
                    )),
                ])),
            )
            .debug_name("checkbox-story"),
        )
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("checked", &self.checked),
            DynamicField::state("mixed", &self.mixed),
        ]
    }
}

/// `ToggleStory`.
pub struct ToggleStory {
    pub options: GalleryOptions,
    pub notifications: State<bool>,
    pub telemetry: State<bool>,
}

impl ToggleStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            notifications: State::new(true),
            telemetry: State::new(false),
        }
    }

    /// `Notifications: On · Usage analytics: Off`
    pub fn values_line(&self) -> String {
        format!(
            "Notifications: {} · Usage analytics: {}",
            if self.notifications.get() {
                "On"
            } else {
                "Off"
            },
            if self.telemetry.get() { "On" } else { "Off" }
        )
    }
}

impl View for ToggleStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let enabled = self.options.is_enabled;
        let notifications = self.notifications.clone();
        let telemetry = self.telemetry.clone();
        let settings = AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            12.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(
                    Toggle::labeled(
                        "Notifications",
                        Binding::new(
                            {
                                let notifications = notifications.clone();
                                move || notifications.get()
                            },
                            move |value| notifications.set(value),
                        ),
                    )
                    .enabled(enabled),
                ),
                AnyView::new(
                    Toggle::labeled(
                        "Usage analytics",
                        Binding::new(
                            {
                                let telemetry = telemetry.clone();
                                move || telemetry.get()
                            },
                            move |value| telemetry.set(value),
                        ),
                    )
                    .enabled(enabled),
                ),
                AnyView::new(
                    Text::new(self.values_line())
                        .semantic_foreground(SemanticColorRef::on_surface_muted()),
                ),
            ])),
        ));

        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                24.0,
                AnyView::new(TupleView::new(vec![AnyView::new(StorySection::new(
                    "Settings",
                    "A switch owns activation and animation; the label slot keeps the setting name beside it, and the application owns the value.",
                    settings,
                ))])),
            )
            .debug_name("toggle-story"),
        )
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("notifications", &self.notifications),
            DynamicField::state("telemetry", &self.telemetry),
        ]
    }
}

#[cfg(test)]
mod checks {
    use super::*;

    #[test]
    fn button_variants_match_the_swift_order() {
        assert_eq!(
            BUTTON_VARIANTS,
            [
                "Primary",
                "Secondary",
                "Danger",
                "Warning",
                "Success",
                "Info",
                "Ghost"
            ]
        );
    }

    #[test]
    fn clicks_accumulate_and_selection_toggles() {
        let story = ButtonStory::new(GalleryOptions::default());
        assert_eq!(story.actions_line(), "Actions: 0");
        story.click();
        story.click();
        assert_eq!(story.actions_line(), "Actions: 2");
        assert_eq!(story.select_label(), "Select me");
        story.selected.set(true);
        assert_eq!(story.select_label(), "Selected");
    }

    #[test]
    fn checkbox_story_starts_mixed_and_can_reset() {
        let story = CheckboxStory::new(GalleryOptions::default());
        assert!(!story.checked.get());
        assert_eq!(story.mixed.get(), CheckboxState::Mixed);
        assert_eq!(story.mixed_label(), "Partially selected collection: Mixed");
        story.mixed.set(CheckboxState::On);
        assert_eq!(story.mixed_label(), "Partially selected collection: On");
        story.reset_mixed();
        assert_eq!(story.mixed.get(), CheckboxState::Mixed);
    }

    #[test]
    fn toggle_story_defaults_and_line_format() {
        let story = ToggleStory::new(GalleryOptions::default());
        assert!(story.notifications.get());
        assert!(!story.telemetry.get());
        assert_eq!(
            story.values_line(),
            "Notifications: On · Usage analytics: Off"
        );
        story.notifications.set(false);
        story.telemetry.set(true);
        assert_eq!(
            story.values_line(),
            "Notifications: Off · Usage analytics: On"
        );
    }
}
