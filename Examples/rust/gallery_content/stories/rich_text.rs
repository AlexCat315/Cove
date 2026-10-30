//! Rich-text stories: inline styles, mixed scripts and directionality.
//!
//! Swift source: `GalleryContent/Stories/Data/RichTextStory.swift`.

use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_core::{State, TextAlignment};
use cove_ui_compose::modifiers::layout::FlexExt;
use cove_ui_compose::primitives::text::rich::rich_text::RichText;
use cove_ui_compose::primitives::text::rich::run::{RichTextRun, RichTextTraits};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::{SemanticColorRef, SemanticStyleViewExt};
use cove_ui_compose::{AnyView, BoxView, DynamicField, LayoutDebugExt, TupleView, View};

use crate::gallery_content::stories::StorySection;

/// The "Inline styles" sample paragraph.
pub fn inline_styles_sample() -> Vec<RichTextRun> {
    vec![
        RichTextRun::new("CoveUI lays out "),
        RichTextRun::with_traits("bold", RichTextTraits::BOLD),
        RichTextRun::new(", "),
        RichTextRun::with_traits("italic", RichTextTraits::ITALIC),
        RichTextRun::new(", "),
        RichTextRun::with_traits("both at once", RichTextTraits::BOLD | RichTextTraits::ITALIC),
        RichTextRun::new(", "),
        RichTextRun::with_traits("inline code", RichTextTraits::CODE),
        RichTextRun::new(" and "),
        RichTextRun::with_traits("struck text", RichTextTraits::STRIKETHROUGH),
        RichTextRun::new(" in one paragraph. Links such as the "),
        RichTextRun::new("layout guide").link("guide://layout"),
        RichTextRun::new(" wrap with the words around them, even when a long link like "),
        RichTextRun::new("https://github.com/swiftlang/swift-markdown")
            .link("https://github.com/swiftlang/swift-markdown"),
        RichTextRun::new(" has to break across lines."),
    ]
}

/// The "Mixed scripts" sample paragraph.
pub fn mixed_script_sample() -> Vec<RichTextRun> {
    vec![
        RichTextRun::new("中文段落按字断行，标点不会出现在行首。混排 "),
        RichTextRun::with_traits("English words", RichTextTraits::BOLD),
        RichTextRun::new(" 时仍在空格处断开，"),
        RichTextRun::with_traits("RichTextRun", RichTextTraits::CODE),
        RichTextRun::new(" 与"),
        RichTextRun::new("链接").link("guide://cjk"),
        RichTextRun::new("也一起换行。"),
    ]
}

/// The three right-to-left paragraphs.
pub fn right_to_left_samples() -> Vec<Vec<RichTextRun>> {
    vec![
        vec![
            RichTextRun::new("שלום "),
            RichTextRun::with_traits("CoveUI", RichTextTraits::BOLD),
            RichTextRun::new(" — גרסה 2.0 זמינה עכשיו."),
        ],
        vec![
            RichTextRun::new("مرحبا بالعالم: "),
            RichTextRun::with_traits("RichText", RichTextTraits::CODE),
            RichTextRun::new(" يدعم 123 رقمًا."),
        ],
        vec![
            RichTextRun::new("An English line that mentions "),
            RichTextRun::with_traits("עברית", RichTextTraits::ITALIC),
            RichTextRun::new(" stays left aligned."),
        ],
    ]
}

/// `RichTextStory`.
pub struct RichTextStory {
    pub last_opened: State<String>,
}

impl RichTextStory {
    pub fn new() -> Self {
        Self {
            last_opened: State::new("none".into()),
        }
    }

    pub fn open(&self, destination: &str) {
        self.last_opened.set(destination.to_string());
    }

    /// `Last opened: {destination}`
    pub fn last_opened_line(&self) -> String {
        format!("Last opened: {}", self.last_opened.get())
    }
}

impl Default for RichTextStory {
    fn default() -> Self {
        Self::new()
    }
}

impl View for RichTextStory {
    fn body(&self) -> AnyView {
        let opened = self.last_opened.clone();
        let inline = AnyView::new(
            RichText::new(inline_styles_sample())
                .on_open_link(move |destination| opened.set(destination.to_string())),
        );
        let opened_caption = {
            let opened = self.last_opened.clone();
            AnyView::new(
                Text::new(format!("Last opened: {}", opened.get()))
                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
            )
        };

        let mixed = {
            let opened = self.last_opened.clone();
            AnyView::new(
                RichText::new(mixed_script_sample())
                    .on_open_link(move |destination| opened.set(destination.to_string())),
            )
        };

        let rtl = AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            12.0,
            AnyView::new(TupleView::new(
                right_to_left_samples()
                    .into_iter()
                    .map(|runs| {
                        let opened = self.last_opened.clone();
                        AnyView::new(
                            RichText::new(runs)
                                .on_open_link(move |destination| {
                                    opened.set(destination.to_string())
                                }),
                        )
                    })
                    .collect(),
            )),
        ));

        // Type roles: the paragraph inherits the surrounding style and ink.
        let roles = AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            10.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(RichText::new(vec![
                    RichTextRun::new("A "),
                    RichTextRun::with_traits("headline", RichTextTraits::ITALIC),
                    RichTextRun::new(" paragraph"),
                ])),
                AnyView::new(
                    RichText::new(vec![
                        RichTextRun::new("A muted caption with "),
                        RichTextRun::with_traits("code", RichTextTraits::CODE),
                        RichTextRun::new("."),
                    ])
                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
                ),
                AnyView::new(
                    RichText::new(vec![
                        RichTextRun::new("Centred text, with "),
                        RichTextRun::with_traits("emphasis", RichTextTraits::BOLD),
                        RichTextRun::new("."),
                    ])
                    .alignment(TextAlignment::Center),
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
                        "Inline styles",
                        "Runs share one flow: weight, slant, monospaced chips, strike lines and underlined links sit on a common baseline and wrap together.",
                        AnyView::new(BoxView::new(
                            FlexDirection::Column,
                            Align::Stretch,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            8.0,
                            AnyView::new(TupleView::new(vec![inline, opened_caption])),
                        )),
                    )),
                    AnyView::new(StorySection::new(
                        "Mixed scripts",
                        "Ideographs break between characters with the usual punctuation rules; Latin words still break at spaces.",
                        mixed,
                    )),
                    AnyView::new(StorySection::new(
                        "Right-to-left paragraphs",
                        "Each paragraph takes its direction from its first strong character: Hebrew and Arabic lines align to the trailing edge, embedded Latin words and numbers keep their own order.",
                        rtl,
                    )),
                    AnyView::new(StorySection::new(
                        "Type roles",
                        "The paragraph takes the surrounding text style and foreground, scaled once by TextScaler.",
                        roles,
                    )),
                ])),
            )
            .flex(0.0)
            .debug_name("rich-text-story"),
        )
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("lastOpened", &self.last_opened)]
    }
}

#[cfg(test)]
mod checks {
    use super::*;

    #[test]
    fn inline_sample_matches_the_swift_runs() {
        let runs = inline_styles_sample();
        assert_eq!(runs.len(), 15);
        assert_eq!(runs[0].text, "CoveUI lays out ");
        assert_eq!(runs[1].text, "bold");
        assert!(runs[1].traits.contains(RichTextTraits::BOLD));
        assert!(!runs[1].traits.contains(RichTextTraits::ITALIC));
        // "both at once" carries both traits.
        assert_eq!(runs[5].text, "both at once");
        assert!(runs[5].traits.contains(RichTextTraits::BOLD));
        assert!(runs[5].traits.contains(RichTextTraits::ITALIC));
        assert!(runs[7].traits.contains(RichTextTraits::CODE));
        assert!(runs[9].traits.contains(RichTextTraits::STRIKETHROUGH));
        // Links carry their destination.
        assert_eq!(runs[11].link.as_deref(), Some("guide://layout"));
        assert_eq!(
            runs[13].link.as_deref(),
            Some("https://github.com/swiftlang/swift-markdown")
        );
        // Untrait-ed runs stay plain.
        assert_eq!(runs[0].traits, RichTextTraits::default());
        assert_eq!(runs[0].link, None);
    }

    #[test]
    fn mixed_script_sample_keeps_cjk_and_a_link() {
        let runs = mixed_script_sample();
        assert_eq!(runs.len(), 7);
        assert!(runs[0].text.starts_with("中文段落"));
        assert!(runs[1].traits.contains(RichTextTraits::BOLD));
        assert!(runs[3].traits.contains(RichTextTraits::CODE));
        assert_eq!(runs[5].link.as_deref(), Some("guide://cjk"));
    }

    #[test]
    fn right_to_left_samples_are_three_paragraphs() {
        let samples = right_to_left_samples();
        assert_eq!(samples.len(), 3);
        // Hebrew, Arabic and a Latin-first line respectively.
        assert!(samples[0][0].text.starts_with('ש'));
        assert!(samples[1][0].text.starts_with('م'));
        assert!(samples[2][0].text.starts_with("An English"));
        assert!(samples[2][1].traits.contains(RichTextTraits::ITALIC));
    }

    #[test]
    fn link_handling_records_the_destination() {
        let story = RichTextStory::new();
        assert_eq!(story.last_opened_line(), "Last opened: none");
        story.open("guide://layout");
        assert_eq!(story.last_opened_line(), "Last opened: guide://layout");
        story.open("https://github.com/swiftlang/swift-markdown");
        assert_eq!(
            story.last_opened_line(),
            "Last opened: https://github.com/swiftlang/swift-markdown"
        );
    }
}
