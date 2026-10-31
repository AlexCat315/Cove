//! Behaviour tests for the Markdown story.
//!
//! Translation of `GalleryContent/Stories/Data/MarkdownStory.swift`. Each authored
//! document advertises a block structure, so the tests assert that the parser
//! actually produces those blocks rather than trusting the prose.

use cove_examples::gallery_content::catalog::GalleryOptions;
use cove_examples::gallery_content::stories::markdown_story::{
    COMPACT, DOCUMENT, EDITABLE, EDITOR_INDENTATION, EDITOR_LINE_HEIGHT, MarkdownStory,
};

fn story() -> MarkdownStory {
    MarkdownStory::new(GalleryOptions::default())
}

#[test]
fn the_editor_starts_with_the_two_item_draft() {
    let story = story();
    assert!(story.draft().contains("### Notes"));
    assert!(story.draft().contains("- one"));
    assert!(story.draft().contains("- two"));
}

#[test]
fn the_story_starts_with_nothing_opened() {
    let story = story();
    assert_eq!(story.last_opened(), "none");
    story.record_opened("https://example.com");
    assert_eq!(story.last_opened(), "https://example.com");
}

#[test]
fn editing_replaces_the_draft() {
    let story = story();
    story.set_draft("# New\n\nBody.");
    assert_eq!(story.draft(), "# New\n\nBody.");
}

#[test]
fn the_full_document_parses_into_many_blocks() {
    let story = story();
    let count = story.block_count(DOCUMENT);
    assert!(
        count >= 10,
        "the release notes have many blocks, parsed {count}"
    );
}

#[test]
fn the_compact_snippet_parses_into_a_heading_paragraph_and_code() {
    let story = story();
    assert_eq!(
        story.block_count(COMPACT),
        3,
        "heading, paragraph, fenced code"
    );
}

#[test]
fn the_draft_parses_into_three_blocks() {
    let story = story();
    // The draft is a heading, the descriptive paragraph, then one list with two
    // items — the paragraph is easy to overlook when counting by eye.
    assert_eq!(story.block_count(EDITABLE), 3);
}

#[test]
fn the_documents_carry_the_features_they_advertise() {
    // These are the constructs the story's prose promises to render.
    assert!(DOCUMENT.contains("```swift"), "a fenced code block");
    assert!(DOCUMENT.contains("| Block |"), "a table");
    assert!(DOCUMENT.contains("- [x]"), "a checked task item");
    assert!(DOCUMENT.contains("- [ ]"), "an unchecked task item");
    assert!(DOCUMENT.contains("> "), "a quote");
    assert!(DOCUMENT.contains("**"), "bold text");
    assert!(DOCUMENT.contains("~~"), "strikethrough");
    assert!(
        DOCUMENT.contains("中文"),
        "a CJK paragraph for line breaking"
    );
}

#[test]
fn an_empty_document_parses_into_nothing() {
    let story = story();
    assert_eq!(story.block_count(""), 0);
}

#[test]
fn the_editor_uses_the_declared_metrics() {
    assert_eq!(EDITOR_LINE_HEIGHT, 20.0);
    assert_eq!(EDITOR_INDENTATION, 2);
}
