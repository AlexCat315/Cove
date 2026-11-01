//! Theme story: live application tokens and typography roles.
//!
//! Swift source: `GalleryContent/Stories/Foundation/ThemeStory.swift`.
//!
//! The story edits application-level tokens (body size, corner radius, font
//! family, icon size) and shows a preview that inherits a *local* theme while
//! the surrounding gallery keeps its own.

use cove_composition::Scope;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_core::{Binding, Font, FontWeight, State};
use cove_ui_compose::modifiers::layout::{LayoutFrameExt, PaddingExt};
use cove_ui_compose::modifiers::style::AppearanceExt;
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::controls::fields::select::{Select, SelectOption};
use cove_ui_compose::primitives::controls::fields::slider::Slider;
use cove_ui_compose::primitives::controls::toggle::Checkbox;
use cove_ui_compose::primitives::feedback::Badge;
use cove_ui_compose::primitives::media::Icon;
use cove_ui_compose::primitives::media::ui_icon_catalog as icons;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_field::TextField;
use cove_ui_compose::theme::tokens::{SemanticColorRef, SemanticStyleViewExt, TextStyleToken};
use cove_ui_compose::theme::{Theme, ThemeViewExt};
use cove_ui_compose::{AnyView, BoxView, DynamicField, LayoutDebugExt, TupleView, View};
use std::rc::Rc;

use crate::gallery_content::stories::StorySection;

/// The font families the story offers, in order. `system` means "inherit".
pub const FAMILY_OPTIONS: [&str; 3] = ["system", "Helvetica Neue", "Menlo"];

/// Body-size slider bounds and step.
pub const BODY_SIZE_RANGE: (f64, f64) = (12.0, 24.0);
/// Radius slider bounds and step.
pub const RADIUS_RANGE: (f64, f64) = (0.0, 24.0);
/// The shared slider step for both controls.
pub const TOKEN_STEP: f64 = 1.0;

/// Icon sizes for the normal and "larger icons" cases.
pub const ICON_MEDIUM: f32 = 16.0;
pub const ICON_MEDIUM_LARGE: f32 = 24.0;

/// `Body · N pt`
pub fn body_size_label(size: f64) -> String {
    format!("Body · {} pt", size as i32)
}

/// `Radius · N pt`
pub fn radius_label(radius: f64) -> String {
    format!("Radius · {} pt", radius as i32)
}

/// Applies the story's token edits to a base theme.
///
/// Mirrors `configured(_:)`: family `None` for "system", body font at the chosen
/// size with a 1.45 line-height, `radius.md` from the slider, and the medium
/// icon size scaled when larger icons are on.
pub fn configured(
    base: &Theme,
    family: &str,
    body_size: f64,
    radius: f64,
    larger_icons: bool,
) -> Theme {
    let mut theme = base.clone();
    theme.typography.family = if family == "system" {
        None
    } else {
        Some(family.to_string())
    };
    let size = body_size as f32;
    theme.typography.body = TextStyleToken::new(
        Font::system(size.max(1.0), FontWeight::Regular),
        (size * 1.45).max(1.0),
        0.0,
    );
    theme.radius.md = radius.max(0.0) as f32;
    // `IconTheme` has a private `replacements` map, so the size is set in place
    // rather than through struct-update syntax.
    theme.icons.medium = if larger_icons {
        ICON_MEDIUM_LARGE
    } else {
        ICON_MEDIUM
    };
    theme
}

/// `ThemeStory`.
pub struct ThemeStory {
    pub body_size: State<f64>,
    pub radius: State<f64>,
    pub family: State<String>,
    pub larger_icons: State<bool>,
    pub name: State<String>,
    pub saved: State<i32>,
}

impl ThemeStory {
    pub fn new() -> Self {
        Self {
            body_size: State::new(16.0),
            radius: State::new(12.0),
            family: State::new("system".into()),
            larger_icons: State::new(false),
            name: State::new("Cove Studio".into()),
            saved: State::new(0),
        }
    }

    pub fn save(&self) {
        self.saved.update(|count| *count += 1);
    }

    /// The theme the preview inherits, derived from the ambient one.
    pub fn local_theme(&self) -> Rc<dyn Fn(&Theme) -> Theme + 'static> {
        let family = self.family.get();
        let body = self.body_size.get();
        let radius = self.radius.get();
        let larger = self.larger_icons.get();
        Rc::new(move |base: &Theme| configured(base, &family, body, radius, larger))
    }

    pub fn body_size_line(&self) -> String {
        body_size_label(self.body_size.get())
    }

    pub fn radius_line(&self) -> String {
        radius_label(self.radius.get())
    }
}

impl Default for ThemeStory {
    fn default() -> Self {
        Self::new()
    }
}

fn f64_binding(state: &State<f64>) -> Binding<f64> {
    Binding::new(
        {
            let state = state.clone();
            move || state.get()
        },
        {
            let state = state.clone();
            move |value| state.set(value)
        },
    )
}

impl View for ThemeStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let family_options = AnyView::new(
            Select::new(
                Binding::new(
                    {
                        let family = self.family.clone();
                        move || family.get()
                    },
                    {
                        let family = self.family.clone();
                        move |value| family.set(value)
                    },
                ),
                FAMILY_OPTIONS
                    .iter()
                    .map(|value| SelectOption::new((*value).to_string(), *value, true))
                    .collect(),
            )
            .frame_width(180.0)
            .debug_name("theme-family"),
        );

        let body_slider = AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::FlexStart,
            Justify::FlexStart,
            Wrap::NoWrap,
            6.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(Text::new(self.body_size_line())),
                AnyView::new(
                    Slider::new(f64_binding(&self.body_size))
                        .range(BODY_SIZE_RANGE)
                        .step(TOKEN_STEP)
                        .frame_width(180.0)
                        .debug_name("theme-body-size"),
                ),
            ])),
        ));

        let radius_slider = AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::FlexStart,
            Justify::FlexStart,
            Wrap::NoWrap,
            6.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(Text::new(self.radius_line())),
                AnyView::new(
                    Slider::new(f64_binding(&self.radius))
                        .range(RADIUS_RANGE)
                        .step(TOKEN_STEP)
                        .frame_width(150.0)
                        .debug_name("theme-radius"),
                ),
            ])),
        ));

        let larger_icons = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::NoWrap,
            8.0,
            AnyView::new(TupleView::new(vec![AnyView::new(
                Checkbox::labeled_from_boolean(
                    "Larger icons",
                    Binding::new(
                        {
                            let larger = self.larger_icons.clone();
                            move || larger.get()
                        },
                        {
                            let larger = self.larger_icons.clone();
                            move |value| larger.set(value)
                        },
                    ),
                )
                .debug_name("theme-larger-icons"),
            )])),
        ));

        let controls = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::Wrap,
            16.0,
            AnyView::new(TupleView::new(vec![
                family_options,
                body_slider,
                radius_slider,
                larger_icons,
            ])),
        ));

        // Typography roles, rendered against the ambient theme.
        let typography = AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            10.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(Text::new("Dashboard")),
                AnyView::new(Text::new("The quick brown fox · AV fi fl 0123456789")),
                AnyView::new(Text::new("Émile · 中文 · 🙂 👩🏽‍💻 🇨🇳 ❤️")),
                AnyView::new(
                    Text::new("Red 🙂 👩🏽‍💻 · color glyphs preserve their own palette")
                        .semantic_foreground(SemanticColorRef::error()),
                ),
                AnyView::new(Text::new("let value = 42")),
                AnyView::new(BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    20.0,
                    AnyView::new(TupleView::new(vec![
                        AnyView::new(Icon::new(icons::file::folder(), 16.0)),
                        AnyView::new(Icon::new(icons::file::folder(), 20.0)),
                        AnyView::new(Icon::new(icons::file::folder(), 24.0)),
                        AnyView::new(Icon::new(icons::file::folder(), 31.0)),
                        AnyView::new(
                            Text::new("Small · medium · large · explicit 31 pt")
                                .semantic_foreground(SemanticColorRef::on_surface_muted()),
                        ),
                    ])),
                )),
            ])),
        ));

        // The preview: a local theme provider around the token-driven content.
        let local =
            (self.local_theme())(&cove_ui_compose::theme::styles::default_light_theme::value());
        let preview = AnyView::new(
            PreviewCard {
                name: self.name.clone(),
                saved: self.saved.clone(),
            }
            .theme(local),
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
                        "Application design system",
                        "Change the application tokens here. The preview inherits a local theme; the surrounding gallery keeps its own theme.",
                        AnyView::new(BoxView::new(
                            FlexDirection::Column,
                            Align::Stretch,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            16.0,
                            AnyView::new(TupleView::new(vec![controls, preview])),
                        )),
                    )),
                    AnyView::new(StorySection::new(
                        "Typography & fallback",
                        "Semantic roles, explicit points and custom text-style tokens share the same font pipeline. Use Text in the gallery toolbar to check 125–200% scaling.",
                        typography,
                    )),
                ])),
            )
            .debug_name("theme-story"),
        )
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("bodySize", &self.body_size),
            DynamicField::state("radius", &self.radius),
            DynamicField::state("family", &self.family),
            DynamicField::state("largerIcons", &self.larger_icons),
            DynamicField::state("name", &self.name),
            DynamicField::state("saved", &self.saved),
        ]
    }
}

/// The preview card, which reads its tokens from the local theme.
struct PreviewCard {
    name: State<String>,
    saved: State<i32>,
}

impl View for PreviewCard {
    fn body(&self, _cx: &Scope) -> AnyView {
        let save_button = {
            let saved = self.saved.clone();
            AnyView::new(
                Button::title("Save changes", move || saved.update(|count| *count += 1))
                    .debug_name("theme-preview-save"),
            )
        };
        let saved_line = {
            let saved = self.saved.clone();
            AnyView::new(Text::new(saved.get().to_string()))
        };
        let name_field = {
            let name = self.name.clone();
            AnyView::new(
                TextField::new(
                    "Project name",
                    Binding::new(
                        {
                            let name = name.clone();
                            move || TextBuffer::from(name.get())
                        },
                        move |value: TextBuffer| name.set(value.string_value()),
                    ),
                )
                .debug_name("theme-preview-input"),
            )
        };

        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                12.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(BoxView::new(
                        FlexDirection::Row,
                        Align::Center,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        12.0,
                        AnyView::new(TupleView::new(vec![
                            AnyView::new(Icon::new(icons::file::folder(), 16.0)),
                            AnyView::new(Text::new("Project settings")),
                            AnyView::new(Badge::new("Local theme")),
                        ])),
                    )),
                    AnyView::new(Text::new(
                        "A body role belongs to your application. CoveUI supplies a default, and your design system sets the family, size and line height.",
                    )),
                    name_field,
                    AnyView::new(BoxView::new(
                        FlexDirection::Row,
                        Align::Center,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        12.0,
                        AnyView::new(TupleView::new(vec![
                            saved_line,
                            AnyView::new(
                                Text::new("changes saved")
                                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
                            ),
                            save_button,
                        ])),
                    )),
                ])),
            )
            .padding(20.0)
            .semantic_background(SemanticColorRef::surface())
            .corner_radius(10.0)
            .semantic_border(SemanticColorRef::border(), 1.0)
            .debug_name("theme-preview"),
        )
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; name, saved)
    }
}

#[cfg(test)]
mod checks {
    use super::*;

    use cove_ui_compose::theme::styles::default_light_theme;

    fn base() -> Theme {
        default_light_theme::value()
    }

    #[test]
    fn labels_format_without_decimals() {
        assert_eq!(body_size_label(16.0), "Body · 16 pt");
        assert_eq!(body_size_label(20.7), "Body · 20 pt");
        assert_eq!(radius_label(12.0), "Radius · 12 pt");
    }

    #[test]
    fn configured_sets_family_size_radius_and_icons() {
        let theme = configured(&base(), "Menlo", 20.0, 6.0, true);
        assert_eq!(theme.typography.family.as_deref(), Some("Menlo"));
        assert_eq!(theme.typography.body.font.size(), 20.0);
        // Line height is 1.45x the body size.
        assert!((theme.typography.body.line_height - 29.0).abs() < 1e-4);
        assert_eq!(theme.radius.md, 6.0);
        assert_eq!(theme.icons.medium, ICON_MEDIUM_LARGE);
    }

    #[test]
    fn system_family_clears_the_override() {
        let theme = configured(&base(), "system", 16.0, 12.0, false);
        assert_eq!(theme.typography.family, None, "system means inherit");
        assert_eq!(theme.icons.medium, ICON_MEDIUM);
    }

    #[test]
    fn slider_bounds_match_the_swift_story() {
        assert_eq!(BODY_SIZE_RANGE, (12.0, 24.0));
        assert_eq!(RADIUS_RANGE, (0.0, 24.0));
        assert_eq!(TOKEN_STEP, 1.0);
        assert_eq!(FAMILY_OPTIONS, ["system", "Helvetica Neue", "Menlo"]);
    }

    #[test]
    fn story_defaults_and_save_counter() {
        let story = ThemeStory::new();
        assert_eq!(story.body_size.get(), 16.0);
        assert_eq!(story.radius.get(), 12.0);
        assert_eq!(story.family.get(), "system");
        assert!(!story.larger_icons.get());
        assert_eq!(story.name.get(), "Cove Studio");
        assert_eq!(story.saved.get(), 0);
        story.save();
        story.save();
        assert_eq!(story.saved.get(), 2);
        assert_eq!(story.body_size_line(), "Body · 16 pt");
        assert_eq!(story.radius_line(), "Radius · 12 pt");
    }

    #[test]
    fn local_theme_reflects_the_current_tokens() {
        let story = ThemeStory::new();
        story.family.set("Menlo".into());
        story.body_size.set(22.0);
        story.radius.set(4.0);
        story.larger_icons.set(true);
        let apply = story.local_theme();
        let theme = apply(&base());
        assert_eq!(theme.typography.family.as_deref(), Some("Menlo"));
        assert_eq!(theme.typography.body.font.size(), 22.0);
        assert_eq!(theme.radius.md, 4.0);
        assert_eq!(theme.icons.medium, ICON_MEDIUM_LARGE);
    }
}
