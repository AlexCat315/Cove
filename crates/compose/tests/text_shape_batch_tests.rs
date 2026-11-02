//! Labels queued for batch shaping must use the font that layout reads.

#[allow(dead_code)]
mod support;

use cove_composition::{AnyView, HorizontalAlignment, TupleView};
use cove_core::{Font, FontWeight};
use cove_text::TextEnvironmentHolder;
use cove_ui_compose::modifiers::style::AppearanceExt;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::Column;
use support::GlobalTestLock;

fn labels(words: &[&str], font: &Font) -> AnyView {
    AnyView::new(Column::new(
        HorizontalAlignment::Center,
        0.0,
        AnyView::new(TupleView::new(
            words
                .iter()
                .map(|word| AnyView::new(Text::new(*word).font(font.clone())))
                .collect(),
        )),
    ))
}

#[test]
fn queued_labels_use_the_font_from_their_modifiers() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let environment = TextEnvironmentHolder::current().unwrap();
            let graph = support::graph();
            let words = ["Alpha", "Bravo", "Charlie", "Delta", "Echo", "Foxtrot"];
            let font = Font::system(13.0, FontWeight::Medium);
            graph.install(labels(&words, &font), Default::default());
            graph.compute_layout(400.0, 400.0);
            for word in words {
                environment.shape(word, Some(&font));
            }
            assert_eq!(
                environment.shaping_cache_snapshot().entries,
                words.len(),
                "each label is shaped once, with its modified font"
            );
        });
    });
}

#[test]
fn a_settled_layout_queues_nothing() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let environment = TextEnvironmentHolder::current().unwrap();
            let graph = support::graph();
            let font = Font::system(13.0, FontWeight::Medium);
            graph.install(labels(&["Alpha", "Bravo"], &font), Default::default());
            graph.compute_layout(400.0, 400.0);
            let entries = environment.shaping_cache_snapshot().entries;
            graph.recomposer.commit_all();
            graph.compute_layout(400.0, 400.0);
            environment.shape("Golf", Some(&font));
            assert_eq!(
                environment.shaping_cache_snapshot().entries,
                entries + 1,
                "only the new text is shaped"
            );
        });
    });
}
