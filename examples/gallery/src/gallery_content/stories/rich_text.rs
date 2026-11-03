//! Rich-text stories: inline styles, mixed scripts and directionality.
//!
//! Swift source: `GalleryContent/Stories/Data/RichTextStory.swift`.

use cove_composition::Scope;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_core::{State, TextAlignment};
use cove_ui_compose::modifiers::layout::FlexExt;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::text::rich::run::{TextRun, TextTraits};
use cove_ui_compose::theme::tokens::{SemanticColorRef, SemanticStyleViewExt};
use cove_ui_compose::{AnyView, BoxView, DynamicField, LayoutDebugExt, TupleView, View};

use crate::gallery_content::stories::StorySection;

/// The "Inline styles" sample paragraph.
pub fn inline_styles_sample() -> Vec<TextRun> {
    vec![
        TextRun::new("CoveUI lays out "),
        TextRun::with_traits("bold", TextTraits::BOLD),
        TextRun::new(", "),
        TextRun::with_traits("italic", TextTraits::ITALIC),
        TextRun::new(", "),
        TextRun::with_traits("both at once", TextTraits::BOLD | TextTraits::ITALIC),
        TextRun::new(", "),
        TextRun::with_traits("inline code", TextTraits::CODE),
        TextRun::new(" and "),
        TextRun::with_traits("struck text", TextTraits::STRIKETHROUGH),
        TextRun::new(" in one paragraph. Links such as the "),
        TextRun::new("layout guide").link("guide://layout"),
        TextRun::new(" wrap with the words around them, even when a long link like "),
        TextRun::new("https://github.com/swiftlang/swift-markdown")
            .link("https://github.com/swiftlang/swift-markdown"),
        TextRun::new(" has to break across lines."),
    ]
}

/// The "Mixed scripts" sample paragraph.
pub fn mixed_script_sample() -> Vec<TextRun> {
    vec![
        TextRun::new("中文段落按字断行，标点不会出现在行首。混排 "),
        TextRun::with_traits("English words", TextTraits::BOLD),
        TextRun::new(" 时仍在空格处断开，"),
        TextRun::with_traits("TextRun", TextTraits::CODE),
        TextRun::new(" 与"),
        TextRun::new("链接").link("guide://cjk"),
        TextRun::new("也一起换行。"),
    ]
}

/// The three right-to-left paragraphs.
pub fn right_to_left_samples() -> Vec<Vec<TextRun>> {
    vec![
        vec![
            TextRun::new("שלום "),
            TextRun::with_traits("CoveUI", TextTraits::BOLD),
            TextRun::new(" — גרסה 2.0 זמינה עכשיו."),
        ],
        vec![
            TextRun::new("مرحبا بالعالم: "),
            TextRun::with_traits("Text", TextTraits::CODE),
            TextRun::new(" يدعم 123 رقمًا."),
        ],
        vec![
            TextRun::new("An English line that mentions "),
            TextRun::with_traits("עברית", TextTraits::ITALIC),
            TextRun::new(" stays left aligned."),
        ],
    ]
}

/// `TextStory`.
pub struct TextStory {
    pub last_opened: State<String>,
}

impl TextStory {
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

impl Default for TextStory {
    fn default() -> Self {
        Self::new()
    }
}

impl View for TextStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let opened = self.last_opened.clone();
        let inline = AnyView::new(
            Text::runs(inline_styles_sample())
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
                Text::runs(mixed_script_sample())
                    .on_open_link(move |destination| opened.set(destination.to_string())),
            )
        };

        let rtl =
            AnyView::new(BoxView::new(
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
                            AnyView::new(Text::runs(runs).on_open_link(move |destination| {
                                opened.set(destination.to_string())
                            }))
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
                AnyView::new(Text::new("A plain paragraph")),
                AnyView::new(Text::runs(vec![
                    TextRun::new("A "),
                    TextRun::with_traits("headline", TextTraits::ITALIC),
                    TextRun::new(" paragraph"),
                ])),
                AnyView::new(
                    Text::runs(vec![
                        TextRun::new("A muted caption with "),
                        TextRun::with_traits("code", TextTraits::CODE),
                        TextRun::new("."),
                    ])
                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
                ),
                AnyView::new(
                    Text::runs(vec![
                        TextRun::new("Centred text, with "),
                        TextRun::with_traits("emphasis", TextTraits::BOLD),
                        TextRun::new("."),
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
            .debug_name("text-story"),
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
        assert!(runs[1].traits.contains(TextTraits::BOLD));
        assert!(!runs[1].traits.contains(TextTraits::ITALIC));
        // "both at once" carries both traits.
        assert_eq!(runs[5].text, "both at once");
        assert!(runs[5].traits.contains(TextTraits::BOLD));
        assert!(runs[5].traits.contains(TextTraits::ITALIC));
        assert!(runs[7].traits.contains(TextTraits::CODE));
        assert!(runs[9].traits.contains(TextTraits::STRIKETHROUGH));
        // Links carry their destination.
        assert_eq!(runs[11].link.as_deref(), Some("guide://layout"));
        assert_eq!(
            runs[13].link.as_deref(),
            Some("https://github.com/swiftlang/swift-markdown")
        );
        // Untrait-ed runs stay plain.
        assert_eq!(runs[0].traits, TextTraits::default());
        assert_eq!(runs[0].link, None);
    }

    #[test]
    fn mixed_script_sample_keeps_cjk_and_a_link() {
        let runs = mixed_script_sample();
        assert_eq!(runs.len(), 7);
        assert!(runs[0].text.starts_with("中文段落"));
        assert!(runs[1].traits.contains(TextTraits::BOLD));
        assert!(runs[3].traits.contains(TextTraits::CODE));
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
        assert!(samples[2][1].traits.contains(TextTraits::ITALIC));
    }

    #[test]
    fn link_handling_records_the_destination() {
        let story = TextStory::new();
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
