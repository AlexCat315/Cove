//! Feedback and media stories: skeleton placeholders and rating gradients.
//!
//! Swift sources: `GalleryContent/Stories/Feedback/{SkeletonStory,RatingSample}.swift`.

use cove_composition::Scope;
use cove_composition::layout::LayoutDirectionExt;
use cove_core::State;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_ui_compose::modifiers::layout::{LayoutFrameExt, PaddingExt};
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::controls::fields::Slider;
use cove_ui_compose::primitives::controls::rating::{
    Rating, RatingAppearance, RatingPrecision, RatingSelection, RatingSymbol,
};
use cove_ui_compose::primitives::controls::{Button, Checkbox};
use cove_ui_compose::primitives::data::avatar::Avatar;
use cove_ui_compose::primitives::media::{Skeleton, SkeletonShape};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::ControlSize;
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

/// `RatingStory`: precision, symbols, states, density and direction.
#[derive(Clone)]
pub struct RatingStory {
    pub options: GalleryOptions,
    whole: State<f64>,
    half: State<f64>,
    fine: State<f64>,
    rtl: State<f64>,
    densities: [State<f64>; 4],
    custom: State<f64>,
    ten: State<f64>,
}

impl RatingStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            whole: State::new(3.0),
            half: State::new(2.5),
            fine: State::new(4.4),
            rtl: State::new(2.5),
            densities: std::array::from_fn(|_| State::new(3.5)),
            custom: State::new(0.5),
            ten: State::new(7.0),
        }
    }

    pub fn whole(&self) -> f64 {
        self.whole.get()
    }

    pub fn half(&self) -> f64 {
        self.half.get()
    }

    pub fn fine(&self) -> f64 {
        self.fine.get()
    }

    pub fn rtl(&self) -> f64 {
        self.rtl.get()
    }

    fn labeled(&self, title: &str, detail: &'static str, body: AnyView) -> AnyView {
        AnyView::new(StorySection::new(title, detail, body))
    }

    fn strip(
        label: &'static str,
        value: f64,
        selection: RatingSelection,
        appearance: RatingAppearance,
        state: &State<f64>,
    ) -> AnyView {
        let setter = state.clone();
        let rating = Rating::new(label, value, move |next| setter.set(next))
            .selection(selection)
            .appearance(appearance);
        let mut details = vec![AnyView::new(
            Text::new(format!("{value:.2}"))
                .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
                .semantic_foreground(SemanticColorRef::on_surface_muted()),
        )];
        if selection.is_interactive() && selection.allows_clear {
            use cove_ui_compose::theme::styles::buttons::{ButtonStyleViewExt, GhostButtonStyle};
            let value = state.clone();
            details.push(AnyView::new(
                Button::title("Clear", move || value.set(0.0)).button_style(GhostButtonStyle),
            ));
        }
        let details = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::Wrap,
            8.0,
            AnyView::new(TupleView::new(details)),
        ));
        let strip = AnyView::new(cove_ui_compose::primitives::layout::ScrollView::horizontal(
            AnyView::new(rating).padding_edges(6.0, 0.0),
        ));
        AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            8.0,
            AnyView::new(TupleView::new(vec![strip, details])),
        ))
    }
}

impl View for RatingStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let enabled = self.options.is_enabled;

        let whole_selection = RatingSelection {
            is_enabled: enabled,
            ..RatingSelection::default()
        };
        let whole = AnyView::new(
            Self::strip(
                "Overall rating",
                self.whole.get(),
                whole_selection,
                RatingAppearance::default(),
                &self.whole,
            )
            .debug_name("rating-strip"),
        );

        let half = Self::strip(
            "Half-star rating",
            self.half.get(),
            RatingSelection {
                precision: RatingPrecision::HALF,
                is_enabled: enabled,
                ..RatingSelection::default()
            },
            RatingAppearance::default(),
            &self.half,
        );
        let ten = Self::strip(
            "Ten-star rating",
            self.ten.get(),
            RatingSelection {
                maximum: 10,
                is_enabled: enabled,
                ..Default::default()
            },
            RatingAppearance {
                size: Some(ControlSize::Small),
                ..Default::default()
            },
            &self.ten,
        );
        let half = AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            12.0,
            AnyView::new(TupleView::new(vec![half, ten])),
        ));

        let mut heart = RatingAppearance {
            symbol: RatingSymbol::heart(),
            filled: SemanticColorRef::error(),
            ..RatingAppearance::default()
        };
        heart.validate();
        let fine = Self::strip(
            "Tenth-star rating",
            self.fine.get(),
            RatingSelection {
                precision: RatingPrecision::divisions(10),
                is_enabled: enabled,
                ..RatingSelection::default()
            },
            heart,
            &self.fine,
        );

        let read_only = {
            let mut selection = RatingSelection {
                is_read_only: true,
                ..RatingSelection::default()
            };
            selection.validate();
            Self::strip(
                "Average customer score",
                4.25,
                selection,
                RatingAppearance {
                    filled: SemanticColorRef::success(),
                    ..RatingAppearance::default()
                },
                &self.whole,
            )
        };
        let disabled = Self::strip(
            "Archived score",
            3.0,
            RatingSelection {
                is_enabled: false,
                ..RatingSelection::default()
            },
            RatingAppearance::default(),
            &self.whole,
        );

        let density = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::Wrap,
            16.0,
            AnyView::new(TupleView::new(
                ControlSize::ALL
                    .iter()
                    .enumerate()
                    .map(|(index, size)| {
                        let mut appearance = RatingAppearance {
                            size: Some(*size),
                            ..RatingAppearance::default()
                        };
                        appearance.validate();
                        Self::strip(
                            [
                                "Mini rating",
                                "Small rating",
                                "Regular rating",
                                "Large rating",
                            ][index],
                            self.densities[index].get(),
                            RatingSelection {
                                is_enabled: enabled,
                                ..RatingSelection::default()
                            },
                            appearance,
                            &self.densities[index],
                        )
                    })
                    .collect(),
            )),
        ));

        let seven = {
            let mut appearance = RatingAppearance {
                star_size: Some(28.0),
                filled: SemanticColorRef::accent_secondary(),
                ..RatingAppearance::default()
            };
            appearance.validate();
            Self::strip(
                "Seven stars, minimum half",
                self.custom.get(),
                RatingSelection {
                    maximum: 7,
                    precision: RatingPrecision::HALF,
                    allows_clear: false,
                    is_enabled: enabled,
                    ..RatingSelection::default()
                },
                appearance,
                &self.custom,
            )
        };

        let density = AnyView::new(TupleView::new(vec![density, seven]));

        let rtl = AnyView::new(
            Self::strip(
                "RTL rating",
                self.rtl.get(),
                RatingSelection {
                    precision: RatingPrecision::HALF,
                    is_enabled: enabled,
                    ..RatingSelection::default()
                },
                RatingAppearance::default(),
                &self.rtl,
            )
            .layout_direction(cove_core::scene::Direction::Rtl),
        );

        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                24.0,
                AnyView::new(TupleView::new(vec![
                    self.labeled(
                        "Whole stars",
                        "Hover previews a score; releasing inside chooses it, and clicking the chosen score clears it.",
                        whole,
                    ),
                    self.labeled(
                        "Half stars & keyboard",
                        "One Tab stop per rating; arrows move by the precision step, Home and End reach the bounds.",
                        half,
                    ),
                    self.labeled(
                        "Fine precision & symbols",
                        "Precision divides each symbol; any filled/outline icon pair can replace the star.",
                        fine,
                    ),
                    self.labeled(
                        "Read-only & disabled",
                        "Read-only scores keep their exact fraction; disabled ratings take no input and leave the Tab order.",
                        AnyView::new(BoxView::new(
                            FlexDirection::Row,
                            Align::Center,
                            Justify::FlexStart,
                            Wrap::Wrap,
                            16.0,
                            AnyView::new(TupleView::new(vec![read_only, disabled])),
                        )),
                    ),
                    self.labeled(
                        "Density & appearance",
                        "Stars rasterize at their actual size and screen density.",
                        density,
                    ),
                    self.labeled(
                        "RTL",
                        "Stars run from the trailing edge and the arrow keys mirror.",
                        rtl,
                    ),
                ])),
            )
            .debug_name("rating-story"),
        )
    }

    fn tag(&self) -> &'static str {
        "RatingStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        let mut fields =
            cove_composition::dynamic_fields!(self; whole, half, fine, rtl, custom, ten);
        for (index, state) in self.densities.iter().enumerate() {
            fields.push(cove_composition::DynamicField::state(
                format!("density-{index}"),
                state,
            ));
        }
        fields
    }
}
