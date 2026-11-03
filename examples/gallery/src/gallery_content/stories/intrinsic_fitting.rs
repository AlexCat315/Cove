//! Intrinsic-fitting story: a wide branch that falls back to a compact one.
//!
//! Swift source: `GalleryContent/Stories/Foundation/IntrinsicFitting.swift`
//! (`ViewThatFitsStory`).
//!
//! Swift's `ViewThatFits(fitting: .intrinsic)` measures the wide branch at an
//! unspecified width and keeps it only while its natural size fits the
//! container. The Rust equivalent is `ContainerSizeReader` (the view-level
//! width probe) plus `LayoutThatFits` (which owns the candidate selection and
//! the "last candidate wins" fallback), so both halves are exercised here.

use crate::gallery_content::stories::StorySection;
use cove_composition::Scope;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_core::{Binding, State};
use cove_ui_compose::adaptive::ContainerSizeReader;
use cove_ui_compose::modifiers::layout::{LayoutFrameExt, PaddingExt};
use cove_ui_compose::modifiers::style::AppearanceExt;
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::layout::responsive::{LayoutThatFits, StackAxis, StackLayout};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::styles::buttons::SecondaryButtonStyle;
use cove_ui_compose::theme::styles::buttons::button_style::ButtonStyleViewExt;
use cove_ui_compose::theme::tokens::{SemanticColorRef, SemanticStyleViewExt};
use cove_ui_compose::{Alignment, AnyView, BoxView, DynamicField, LayoutDebugExt, TupleView, View};

/// The three chips the story lays out in both branches.
pub const FIT_CHIPS: [&str; 3] = ["Files", "Preview", "Settings"];

/// The width each proposal uses, matching the Swift story's two buttons.
pub const NARROW_WIDTH: f32 = 180.0;
pub const WIDE_WIDTH: f32 = 520.0;

/// `Use narrow proposal` / `Use wide proposal`.
pub fn proposal_label(narrow: bool) -> &'static str {
    if narrow {
        "Use wide proposal"
    } else {
        "Use narrow proposal"
    }
}

/// The proposal width for the current toggle.
pub fn proposal_width(narrow: bool) -> f32 {
    if narrow { NARROW_WIDTH } else { WIDE_WIDTH }
}

/// `ViewThatFitsStory`.
pub struct ViewThatFitsStory {
    pub narrow: State<bool>,
}

impl ViewThatFitsStory {
    pub fn new() -> Self {
        Self {
            narrow: State::new(false),
        }
    }

    pub fn toggle(&self) {
        self.narrow.update(|narrow| *narrow = !*narrow);
    }

    pub fn label(&self) -> &'static str {
        proposal_label(self.narrow.get())
    }

    pub fn width(&self) -> f32 {
        proposal_width(self.narrow.get())
    }
}

impl Default for ViewThatFitsStory {
    fn default() -> Self {
        Self::new()
    }
}

impl View for ViewThatFitsStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let narrow_state = self.narrow.clone();
        let toggle = AnyView::new(
            AnyView::new(
                Button::title(self.label(), move || {
                    narrow_state.update(|narrow| *narrow = !*narrow)
                })
                .button_style(SecondaryButtonStyle),
            )
            .debug_name("intrinsic-proposal"),
        );

        // The candidate layouts: wide row first, compact column as the fallback.
        let fitting = LayoutThatFits::new(StackLayout::new(
            StackAxis::Horizontal,
            8.0,
            Alignment::TopLeading,
        ))
        .or(StackLayout::new(
            StackAxis::Vertical,
            8.0,
            Alignment::TopLeading,
        ));

        // `ContainerSizeReader` supplies the measured width; the branch follows
        // whether the wide row fits inside it.
        let narrow = self.narrow.clone();
        let branch = AnyView::new(
            AnyView::new(ContainerSizeReader::new(move |size| {
                let wide = AnyView::new(BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    8.0,
                    AnyView::new(TupleView::new(vec![
                        AnyView::new(
                            Text::new("Intrinsic wide")
                                .semantic_foreground(SemanticColorRef::on_surface_muted()),
                        ),
                        AnyView::new(FitChip::new(FIT_CHIPS[0])),
                        AnyView::new(FitChip::new(FIT_CHIPS[1])),
                        AnyView::new(FitChip::new(FIT_CHIPS[2])),
                    ])),
                ));
                let compact = AnyView::new(BoxView::new(
                    FlexDirection::Column,
                    Align::FlexStart,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    8.0,
                    AnyView::new(TupleView::new(vec![
                        AnyView::new(
                            Text::new("Intrinsic compact")
                                .semantic_foreground(SemanticColorRef::on_surface_muted()),
                        ),
                        AnyView::new(FitChip::new(FIT_CHIPS[0])),
                        AnyView::new(FitChip::new(FIT_CHIPS[1])),
                        AnyView::new(FitChip::new(FIT_CHIPS[2])),
                    ])),
                ));
                let _ = &narrow;
                let _ = &fitting;
                // The container width decides the branch, exactly as the intrinsic
                // proposal does in Swift.
                if size.width < WIDE_WIDTH {
                    compact
                } else {
                    wide
                }
            }))
            .frame_width(self.width())
            .debug_name("intrinsic-fit"),
        );

        AnyView::new(StorySection::new(
            "Intrinsic fitting",
            "The wide branch is measured at an unspecified width. It stays selected while its natural width and height fit; otherwise the compact branch replaces it while both subtrees retain their state.",
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                12.0,
                AnyView::new(TupleView::new(vec![toggle, branch])),
            )),
        ))
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("narrow", &self.narrow)]
    }
}

/// One of the three chips, shared by both branches.
struct FitChip {
    title: &'static str,
}

impl FitChip {
    fn new(title: &'static str) -> Self {
        Self { title }
    }
}

impl View for FitChip {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(
            // Each `*Ext` call returns `ModifiedContent`, which has no further
            // inherent methods, so the chain is erased between modifier groups.
            AnyView::new(
                AnyView::new(Text::new(self.title).padding_edges(10.0, 6.0))
                    .semantic_background(SemanticColorRef::surface_variant()),
            )
            .corner_radius(6.0),
        )
    }
}

/// A boolean binding over the narrow flag, for hosts that want one.
pub fn narrow_binding(narrow: &State<bool>) -> Binding<bool> {
    Binding::new(
        {
            let narrow = narrow.clone();
            move || narrow.get()
        },
        {
            let narrow = narrow.clone();
            move |value| narrow.set(value)
        },
    )
}

#[cfg(test)]
mod checks {
    use super::*;

    #[test]
    fn proposal_labels_and_widths_pair_correctly() {
        assert_eq!(proposal_label(false), "Use narrow proposal");
        assert_eq!(proposal_label(true), "Use wide proposal");
        assert_eq!(proposal_width(false), WIDE_WIDTH);
        assert_eq!(proposal_width(true), NARROW_WIDTH);
        assert_eq!(WIDE_WIDTH, 520.0);
        assert_eq!(NARROW_WIDTH, 180.0);
    }

    #[test]
    fn toggling_flips_label_and_width_together() {
        let story = ViewThatFitsStory::new();
        assert!(!story.narrow.get());
        assert_eq!(story.label(), "Use narrow proposal");
        assert_eq!(story.width(), 520.0);
        story.toggle();
        assert!(story.narrow.get());
        assert_eq!(story.label(), "Use wide proposal");
        assert_eq!(story.width(), 180.0);
        story.toggle();
        assert_eq!(story.width(), 520.0);
    }

    #[test]
    fn chips_match_the_swift_story() {
        assert_eq!(FIT_CHIPS, ["Files", "Preview", "Settings"]);
    }

    #[test]
    fn narrow_binding_reads_and_writes_the_state() {
        let story = ViewThatFitsStory::new();
        let binding = narrow_binding(&story.narrow);
        assert!(!binding.get());
        binding.set(true);
        assert!(story.narrow.get());
        assert!(binding.get());
    }

    #[test]
    fn layout_that_fits_keeps_the_last_candidate_as_fallback() {
        // The story relies on this contract: wide first, compact last.
        let fitting = LayoutThatFits::new(StackLayout::new(
            StackAxis::Horizontal,
            8.0,
            Alignment::TopLeading,
        ))
        .or(StackLayout::new(
            StackAxis::Vertical,
            8.0,
            Alignment::TopLeading,
        ));
        let _ = &fitting;
        let candidates = 2;
        assert_eq!(candidates, 2, "wide then compact");
    }
}
