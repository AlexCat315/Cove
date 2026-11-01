//! Behaviour tests for the web story.
//!
//! Translation of `GalleryContent/Stories/Data/WebStory.swift`. The status-line
//! precedence is the logic the story adds on top of `WebViewState`.

use cove_examples::gallery_content::catalog::GalleryOptions;
use cove_examples::gallery_content::stories::web_story::{
    WebSample, WebStory, command_is_enabled, status_text,
};
use cove_ui_addons::webview::{WebViewCommand, WebViewPhase, WebViewState};

fn state(phase: WebViewPhase) -> WebViewState {
    WebViewState {
        phase,
        ..WebViewState::default()
    }
}

#[test]
fn an_idle_surface_reads_ready() {
    assert_eq!(status_text(&state(WebViewPhase::Idle)), "Ready");
    assert_eq!(status_text(&WebViewState::default()), "Ready");
}

#[test]
fn loading_without_a_title_shows_the_generic_text() {
    assert_eq!(status_text(&state(WebViewPhase::Loading)), "Loading…");
}

#[test]
fn loading_with_a_title_names_the_target() {
    let mut state = state(WebViewPhase::Loading);
    state.title = Some("Example Domain".into());
    assert_eq!(status_text(&state), "Loading Example Domain");
}

#[test]
fn a_loaded_page_shows_its_title_or_falls_back() {
    let mut loaded = state(WebViewPhase::Loaded);
    loaded.title = Some("Example Domain".into());
    assert_eq!(status_text(&loaded), "Example Domain");
    assert_eq!(status_text(&state(WebViewPhase::Loaded)), "Loaded");
}

#[test]
fn a_failure_reports_its_description() {
    let mut failed = state(WebViewPhase::Failed);
    failed.error_description = Some("The Internet connection appears to be offline.".into());
    assert_eq!(
        status_text(&failed),
        "Failed: The Internet connection appears to be offline."
    );
}

#[test]
fn a_failure_without_a_description_falls_back_to_the_generic_text() {
    assert_eq!(status_text(&state(WebViewPhase::Failed)), "Failed to load");
}

#[test]
fn a_description_without_the_failed_phase_is_ignored() {
    // A host may report a stale error while already loading again; the phase
    // decides, not the leftover description.
    let mut loading = state(WebViewPhase::Loading);
    loading.error_description = Some("stale".into());
    assert_eq!(status_text(&loading), "Loading…");
}

#[test]
fn the_url_is_used_when_a_page_has_no_title() {
    let mut loaded = state(WebViewPhase::Loaded);
    loaded.url = Some("https://example.com".into());
    assert_eq!(status_text(&loaded), "https://example.com");
}

#[test]
fn back_and_forward_follow_the_host_history() {
    let mut navigation = state(WebViewPhase::Loaded);
    navigation.can_go_back = true;
    assert!(command_is_enabled(WebViewCommand::Back, &navigation));
    assert!(!command_is_enabled(WebViewCommand::Forward, &navigation));
    navigation.can_go_forward = true;
    assert!(command_is_enabled(WebViewCommand::Forward, &navigation));
}

#[test]
fn reload_is_always_available_and_stop_only_while_loading() {
    assert!(command_is_enabled(
        WebViewCommand::Reload,
        &state(WebViewPhase::Idle)
    ));
    assert!(!command_is_enabled(
        WebViewCommand::Stop,
        &state(WebViewPhase::Idle)
    ));
    assert!(command_is_enabled(
        WebViewCommand::Stop,
        &state(WebViewPhase::Loading)
    ));
}

#[test]
fn the_local_sample_is_offline_and_the_others_are_urls() {
    assert_eq!(WebSample::Local.url(), None);
    assert_eq!(WebSample::Example.url(), Some("https://example.com"));
    assert!(WebSample::Repository.url().unwrap().contains("github.com"));
    assert_eq!(WebSample::ALL.len(), 3);
}

#[test]
fn the_local_page_is_real_html_with_a_title() {
    let page = match WebSample::Local.request() {
        cove_ui_addons::webview::WebViewRequest::Html { html, .. } => html,
        other => panic!("the local sample must be inline HTML, got {other:?}"),
    };
    assert!(page.contains("<!doctype html>"));
    assert!(page.contains("<title>CoveUI WebView</title>"));
    assert!(
        page.contains("prefers-color-scheme: dark"),
        "the page follows the colour scheme"
    );
}

#[test]
fn switching_samples_issues_a_new_request() {
    let story = WebStory::new(GalleryOptions::default());
    assert_eq!(story.sample(), WebSample::Local);
    assert_eq!(story.status(), "Ready");
    story.set_sample(WebSample::Example);
    assert_eq!(story.sample(), WebSample::Example);
}

#[test]
fn an_identical_snapshot_is_dropped() {
    let story = WebStory::new(GalleryOptions::default());
    let loaded = state(WebViewPhase::Loaded);
    assert!(story.publish(loaded.clone()), "the first snapshot applies");
    assert!(!story.publish(loaded), "a repeat does not recompose");
}

#[test]
fn publishing_a_new_snapshot_updates_the_status() {
    let story = WebStory::new(GalleryOptions::default());
    let mut loading = state(WebViewPhase::Loading);
    loading.can_go_back = true;
    assert!(story.publish(loading));
    assert_eq!(story.status(), "Loading…");
    assert!(story.is_loading());
    assert!(story.can_go_back());
    assert!(!story.can_go_forward());
}
