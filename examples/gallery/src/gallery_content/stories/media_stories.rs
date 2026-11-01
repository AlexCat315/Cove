//! Feedback and media stories: skeleton placeholders and rating gradients.
//!
//! Swift sources: `GalleryContent/Stories/Feedback/{SkeletonStory,RatingSample}.swift`.

use cove_composition::Scope;
use cove_core::State;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_ui_compose::modifiers::layout::{LayoutFrameExt, PaddingExt};
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::controls::fields::Slider;
use cove_ui_compose::primitives::controls::rating::Rating;
use cove_ui_compose::primitives::controls::{Button, Checkbox};
use cove_ui_compose::primitives::data::avatar::Avatar;
use cove_ui_compose::primitives::media::{Skeleton, SkeletonShape};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::SemanticColorRef;
use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView, View};

use super::feedback::StorySection;
use crate::gallery_content::catalog::GalleryOptions;

/// A row of labelled checkboxes, matching the story's control cluster.
fn toggle_row(label: &'static str, is_on: State<bool>, is_enabled: bool) -> AnyView {
    let binding = is_on.binding();
    AnyView::new(BoxView::new(
        FlexDirection::Row,
        Align::Center,
        Justify::FlexStart,
        Wrap::NoWrap,
        8.0,
        AnyView::new(TupleView::new(vec![
            AnyView::new(Checkbox::from_boolean(binding).enabled(is_enabled)),
            AnyView::new(
                Text::new(label).semantic_font(cove_ui_compose::theme::SemanticFontRef::caption()),
            ),
        ])),
    ))
}

/// `SkeletonStory`: the placeholder preserves the content layout.
pub struct SkeletonStory {
    pub options: GalleryOptions,
    loading: State<bool>,
    animated: State<bool>,
    reduced_motion: State<bool>,
}

impl SkeletonStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            loading: State::new(true),
            animated: State::new(true),
            reduced_motion: State::new(false),
        }
    }

    pub fn is_loading(&self) -> bool {
        self.loading.get()
    }

    /// Flips between placeholder and content.
    pub fn toggle_loading(&self) {
        self.loading.update(|value| *value = !*value);
    }
}

impl View for SkeletonStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let enabled = self.options.is_enabled;
        let loading = self.loading.get();
        let animated = self.animated.get();
        let toggle_state = self.loading.clone();
        let label = if loading {
            "Show content"
        } else {
            "Load again"
        };

        let header_controls = AnyView::new(TupleView::new(vec![
            AnyView::new(
                Button::title(label, move || toggle_state.update(|value| *value = !*value))
                    .enabled(enabled)
                    .debug_name("skeleton-toggle"),
            ),
            toggle_row("Animate", self.animated.clone(), enabled),
            toggle_row("Reduce motion", self.reduced_motion.clone(), enabled),
        ]));

        let profile = if loading {
            AnyView::new(TupleView::new(vec![
                AnyView::new(Skeleton::new().configure(|skeleton| {
                    skeleton.appearance.shape = SkeletonShape::Circle;
                    skeleton.appearance.height = 48.0;
                    skeleton.motion.is_animated = animated;
                })),
                AnyView::new(
                    Skeleton::new()
                        .configure(|skeleton| {
                            skeleton.appearance.height = 18.0;
                            skeleton.motion.is_animated = animated;
                        })
                        .frame_width(180.0),
                ),
                AnyView::new(
                    Skeleton::new()
                        .configure(|skeleton| {
                            skeleton.appearance.height = 14.0;
                            skeleton.appearance.is_secondary = true;
                            skeleton.motion.is_animated = animated;
                        })
                        .frame_width(260.0),
                ),
            ]))
        } else {
            AnyView::new(TupleView::new(vec![
                AnyView::new(Avatar::new("Ada Lovelace")),
                AnyView::new(
                    Text::new("Ada Lovelace")
                        .semantic_font(cove_ui_compose::theme::SemanticFontRef::body_strong()),
                ),
                AnyView::new(
                    Text::new("Analytical Engine · Maintainer")
                        .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
                        .semantic_foreground(SemanticColorRef::on_surface_muted()),
                ),
            ]))
        };

        let placeholders = AnyView::new(TupleView::new(vec![
            AnyView::new(
                Skeleton::new()
                    .configure(|skeleton| {
                        skeleton.appearance.height = 100.0;
                        skeleton.appearance.shape = SkeletonShape::Rectangle(Some(12.0));
                        skeleton.motion.is_animated = animated;
                    })
                    .frame_width(380.0),
            ),
            AnyView::new(
                Skeleton::new()
                    .configure(|skeleton| {
                        skeleton.appearance.height = 16.0;
                        skeleton.motion.is_animated = animated;
                    })
                    .frame_width(340.0),
            ),
            AnyView::new(
                Skeleton::new()
                    .configure(|skeleton| {
                        skeleton.appearance.height = 16.0;
                        skeleton.appearance.is_secondary = true;
                        skeleton.motion.is_animated = animated;
                    })
                    .frame_width(260.0),
            ),
        ]));

        AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            24.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(StorySection::new(
                    "Loading a profile",
                    "The placeholder preserves the content layout. Switch to data, restart loading, or freeze the pulse to inspect its shapes.",
                    AnyView::new(BoxView::new(
                        FlexDirection::Column,
                        Align::Stretch,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        12.0,
                        AnyView::new(TupleView::new(vec![header_controls, profile])),
                    )),
                )),
                AnyView::new(StorySection::new(
                    "Content placeholders",
                    "Rectangles, rounded panels and circular avatars share a node-owned clock. Decorative shapes stay out of the accessibility tree.",
                    placeholders,
                )),
            ])),
        ))
    }

    fn tag(&self) -> &'static str {
        "SkeletonStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; loading, animated, reduced_motion)
    }
}

/// `RatingSample`: a rating strip with its details below it.
pub struct RatingSample {
    pub options: GalleryOptions,
    value: State<f64>,
    limit: State<f64>,
}

impl RatingSample {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            value: State::new(3.0),
            limit: State::new(5.0),
        }
    }

    pub fn value(&self) -> f64 {
        self.value.get()
    }
}

impl View for RatingSample {
    fn body(&self, _cx: &Scope) -> AnyView {
        let enabled = self.options.is_enabled;
        let value = self.value.get();
        let setter = self.value.clone();
        let rating = Rating::new("Rating", value, move |next| setter.set(next));

        let limit = Slider::new(self.limit.binding()).enabled(enabled);

        AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            8.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(AnyView::new(rating).debug_name("rating-strip")),
                AnyView::new(
                    BoxView::new(
                        FlexDirection::Row,
                        Align::Center,
                        Justify::FlexStart,
                        Wrap::Wrap,
                        12.0,
                        AnyView::new(TupleView::new(vec![
                            AnyView::new(limit),
                            AnyView::new(
                                Text::new(format!("{value:.1} of {:.0}", self.limit.get()))
                                    .semantic_font(
                                        cove_ui_compose::theme::SemanticFontRef::caption(),
                                    )
                                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
                            ),
                        ])),
                    )
                    .padding_edges(0.0, 4.0),
                ),
            ])),
        ))
    }

    fn tag(&self) -> &'static str {
        "RatingSample"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; value, limit)
    }
}
