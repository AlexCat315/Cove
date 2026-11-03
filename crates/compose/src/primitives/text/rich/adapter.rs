//! Styled-run adaptation for the single Text primitive.
use super::flow::{RichParagraph, RichParagraphSpan};
use super::paragraph::{RichTextSurface, TextRole};
use super::run::TextRun;
use crate::primitives::text::text::Text;
use cove_core::Font;
use cove_core::scene::Node;
use cove_text::TextEnvironmentHolder;
/// Distinct link destinations, in first-appearance order, matching each span's
/// `link` index.
fn destinations_of(runs: &[TextRun]) -> Vec<String> {
    let mut destinations: Vec<String> = Vec::new();
    for run in runs {
        if let Some(link) = run.link.as_ref()
            && !destinations.contains(link)
        {
            destinations.push(link.clone());
        }
    }
    destinations
}

impl Text {
    pub(crate) fn runs_paragraph(&self, font: Font, line_height: f32) -> RichParagraph {
        let runs = self.runs.as_deref().unwrap_or_default();
        let destinations = destinations_of(runs);
        RichParagraph {
            spans: runs
                .iter()
                .map(|run| RichParagraphSpan {
                    text: run.text.clone(),
                    font: font.clone(),
                    line_height,
                    traits: run.traits,
                    link: run.link.as_ref().and_then(|link| {
                        destinations.iter().position(|candidate| candidate == link)
                    }),
                })
                .collect(),
            alignment: self.alignment,
            wraps: self.role != TextRole::CodeBlock,
        }
    }
    pub(crate) fn runs_surface(&self, node: &Node) -> RichTextSurface {
        let (font, line_height) = TextEnvironmentHolder::current()
            .map(|environment| {
                let metrics = Self::metrics_for_node(node, &environment);
                (metrics.font, metrics.line_height)
            })
            .unwrap_or_else(|| {
                let body = crate::theme::theme_for_node(node)
                    .typography
                    .resolved_body();
                (body.font.clone(), body.line_height)
            });
        RichTextSurface {
            paragraph: self.runs_paragraph(font, line_height),
            role: self.role.clone(),
            destinations: destinations_of(self.runs.as_deref().unwrap_or_default()),
            on_open_link: self.on_open_link.clone(),
            line_limit: self.line_limit,
            truncates: self.truncates,
            is_selectable: self.is_selectable,
        }
    }
}
