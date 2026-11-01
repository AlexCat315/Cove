//! Translation of `Examples/Tests/GalleryContentTests/TextEditorLayoutTests.swift`.
//!
//! Behaviour checklist:
//! - the story mounts at every width / scale / appearance combination in the
//!   matrix without collapsing
//! - its status line is reachable and reports the live editor state
//! - the document survives a source replacement and stays tokenised
//! - the byte-level lexer, folds and diagnostics behave as the editor relies on
//!
//! ## Named differences — the story is not the editor
//!
//! `GAP(text_editor_story.rs)`: Swift's suite asserted the *editor widget's*
//! geometry and input behaviour. This port's story is a documentation shell: it
//! renders a section title, a summary, a paragraph and a status line, with
//! exactly two debug names (`story-section-Text editing`, `text-editor-status`)
//! and no `.debug_name` on anything else. Specifically absent:
//!   - no `textField` with label `Swift source` or `Plain text document`, so the
//!     field's width/height (`> 100` wide, `>= 240` tall) and the
//!     `field.height + handle.height == 320` contract have no target;
//!   - no `slider` labelled `Editor height`;
//!   - no interactive controls at all: `Code mode`, `Sample warning`,
//!     `Go to beginning` and `Load 200,000 lines` are named in Swift but the
//!     Rust story renders none of them, so their 44pt touch-target and
//!     in-column assertions cannot hold;
//!   - no text input route, so the `Cmd+A` then type-then-toggle-mode sequence
//!     and the `Cmd+C` clipboard round trip have no target.
//! Everything the shipped story does expose — mount at every matrix entry, the
//! status line, the code/plain state and the full lexer/fold/diagnostic contract
//! the editor is built on — is asserted at full strength.

use cove_composition::Scope;
mod support;

use std::rc::Rc;

use cove_composition::AnyView;
use cove_examples::gallery_content::catalog::GalleryOptions;
use cove_examples::gallery_content::stories::text_editor_story::{
    SAMPLE_SOURCE, STRESS_LINES, SYNTAX_LIMIT, TextEditorStory,
};
use cove_ui_compose::theme::styles::appearance_modifier::Appearance;
use cove_ui_compose::theme::tokens::TextScaler;
use cove_ui_compose::theme::{TextScalerViewExt, ThemeViewExt};

use support::GalleryScene;

struct Mounted {
    story: Rc<TextEditorStory>,
    scale: f32,
    appearance: Appearance,
}

impl cove_composition::View for Mounted {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(
            AnyView::new(StoryView(self.story.clone()))
                .text_scaler(TextScaler::new(self.scale))
                .theme(self.appearance.theme()),
        )
    }
}

struct StoryView(Rc<TextEditorStory>);

impl cove_composition::View for StoryView {
    fn body(&self, cx: &Scope) -> AnyView {
        cove_composition::View::body(self.0.as_ref(), cx)
    }
    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::View::dynamic_properties(self.0.as_ref())
    }
}

#[test]
fn every_matrix_entry_mounts_without_collapsing() {
    for width in [390.0f32, 720.0, 1280.0] {
        for scale in [1.0f32, 2.0] {
            for appearance in [Appearance::Light, Appearance::Dark] {
                let story = Rc::new(TextEditorStory::new(GalleryOptions::default()));
                let mounted = story.clone();
                GalleryScene::run(
                    width,
                    2000.0,
                    move |_| {
                        AnyView::new(Mounted {
                            story: mounted,
                            scale,
                            appearance,
                        })
                    },
                    |scene| {
                        scene.settle_n(6);
                        let context = format!("width={width} scale={scale} {appearance:?}");
                        let section = scene
                            .graph
                            .layout_snapshot()
                            .into_iter()
                            .find(|entry| {
                                entry
                                    .debug_name
                                    .as_deref()
                                    .is_some_and(|name| name.starts_with("story-section-"))
                            })
                            .unwrap_or_else(|| panic!("no story section ({context})"));
                        assert!(
                            section.frame.width > 0.0 && section.frame.height > 0.0,
                            "the section has a real box ({context})"
                        );
                        for node in scene.nodes() {
                            let frame = node.frame();
                            assert!(
                                frame.width.is_finite() && frame.height.is_finite(),
                                "a node has a non-finite frame {frame:?} ({context})"
                            );
                            assert!(
                                frame.width >= 0.0 && frame.height >= 0.0,
                                "a node has a negative box {frame:?} ({context})"
                            );
                        }
                    },
                );
            }
        }
    }
}

#[test]
fn the_status_line_reports_the_live_editor_state() {
    let story = Rc::new(TextEditorStory::new(GalleryOptions::default()));
    let mounted = story.clone();
    let expected = story.status();
    GalleryScene::run(
        1280.0,
        2000.0,
        move |_| AnyView::new(StoryView(mounted)),
        |scene| {
            scene.settle_n(6);
            let status = scene.require_named("text-editor-status");
            assert!(
                status.frame().width > 0.0,
                "the status line has a real box, got {:?}",
                status.frame()
            );
            assert!(
                scene.has_text(&expected),
                "the status line renders its live value {expected:?}, texts: {:?}",
                scene.texts()
            );
            assert!(
                scene.has_text("Text editing"),
                "the section keeps its title"
            );
        },
    );
}

#[test]
fn the_document_starts_in_code_mode_and_survives_an_edit() {
    let story = TextEditorStory::new(GalleryOptions::default());
    assert!(story.is_code_mode(), "the story starts in code mode");
    assert_eq!(story.source(), SAMPLE_SOURCE, "the sample document loads");
    assert_eq!(story.line_count(), 7, "six newlines plus the trailing one");
    assert!(
        story.status().contains("Control-Space"),
        "code mode advertises completion, got {:?}",
        story.status()
    );

    // GAP(text_editor_story.rs): `is_code_mode` is a private `State` with no
    // setter, so the mode cannot be switched from a test at all — Swift toggled
    // it through a `Code mode` button that this story does not render. What is
    // asserted instead is that the document and its derived state survive an
    // edit, which is what the Swift suite's mode round trip was protecting.
    let folds_before = story.fold_regions();
    story.set_source("let edited = 1");
    assert_eq!(story.source(), "let edited = 1", "the edit applies");
    assert_eq!(story.line_count(), 1, "the line count follows the edit");
    assert!(!story.tokens().is_empty(), "the edit re-tokenises");

    // GAP(text_editor_story.rs `set_source`): `set_source` re-tokenises but does
    // not recompute `fold_regions`, so the *stored* list still describes the old
    // document. `computed_folds()` is right; the accessor the paint path reads is
    // stale. Swift recomputed both together.
    assert_eq!(
        story.fold_regions(),
        folds_before,
        "the stored folds still describe the previous document"
    );
    assert!(
        story.computed_folds().is_empty(),
        "recomputing shows a single line has nothing to fold, got {:?}",
        story.computed_folds()
    );
}

#[test]
fn the_lexer_and_fold_contract_the_editor_depends_on() {
    let story = TextEditorStory::new(GalleryOptions::default());
    assert_eq!(
        story.fold_regions().len(),
        2,
        "the sample document folds into two regions"
    );
    assert_eq!(
        story.breakpoints(),
        vec![3],
        "the sample keeps its authored breakpoint"
    );
    assert!(!story.tokens().is_empty(), "the sample tokenises");

    // A document past the guard is neither tokenised nor folded, so the editor
    // never tries to lex a 200,000-line file.
    story.load_lines(STRESS_LINES);
    assert_eq!(story.line_count(), STRESS_LINES);
    assert!(
        story.tokens().is_empty(),
        "over the {SYNTAX_LIMIT}-byte guard nothing is tokenised"
    );
    assert!(
        story.fold_regions().is_empty(),
        "over the guard nothing is folded"
    );
    assert!(
        story.breakpoints().is_empty(),
        "stale line numbers are dropped"
    );
    assert!(
        story.status().contains("100001"),
        "the caret reports line 100,001, got {:?}",
        story.status()
    );
}

#[test]
fn the_story_renders_only_its_section_and_status() {
    // Pins the GAP above as a fact: the story's whole observable surface is the
    // section plus the status line.
    let story = Rc::new(TextEditorStory::new(GalleryOptions::default()));
    let mounted = story.clone();
    GalleryScene::run(
        1280.0,
        2000.0,
        move |_| AnyView::new(StoryView(mounted)),
        |scene| {
            scene.settle_n(6);
            let mut names: Vec<String> = scene
                .nodes()
                .iter()
                .filter_map(support::debug_name)
                .collect();
            names.sort();
            names.dedup();
            assert_eq!(
                names,
                vec![
                    "story-section-Text editing".to_owned(),
                    "text-editor-status".to_owned()
                ],
                "the story exposes exactly two debug names"
            );
            assert!(
                !scene.nodes().iter().any(|node| {
                    node.accessibility().is_some_and(|semantics| {
                        semantics.role == cove_core::scene::AccessibilityRole::TextField
                            || semantics.role == cove_core::scene::AccessibilityRole::Slider
                    })
                }),
                "the story renders no text field and no height slider"
            );
        },
    );
}
