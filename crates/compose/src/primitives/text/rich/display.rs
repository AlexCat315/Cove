//! Visible-run adaptation for Text's shared line-limit and truncation builders.
use super::flow::{RichParagraph, RichParagraphSpan, RichTextFlow};
use crate::primitives::text::single_line::SingleLineText;
use cove_core::line_breaks::is_newline;
use cove_text::TextEnvironment;
use unicode_segmentation::UnicodeSegmentation;

fn prefix(source: &RichParagraph, bytes: usize, ellipsis: bool) -> RichParagraph {
    let mut remaining = bytes;
    let mut result = source.clone();
    result.spans.clear();
    for span in &source.spans {
        if remaining == 0 {
            break;
        }
        let count = remaining.min(span.text.len());
        let mut part = span.clone();
        part.text.truncate(count);
        result.spans.push(part);
        remaining -= count;
    }
    if ellipsis {
        if result.spans.is_empty() {
            if let Some(span) = source.spans.first() {
                let mut part = span.clone();
                part.text = "…".into();
                result.spans.push(part);
            }
        } else {
            result.spans.last_mut().unwrap().text.push('…');
        }
    }
    result
}

pub(super) fn single_line(source: &RichParagraph) -> RichParagraph {
    let mut result = source.clone();
    result.spans.clear();
    let text = source.text();
    let mut span_index = 0;
    let mut span_end = source.spans.first().map_or(0, |s| s.text.len());
    let mut pending_space = false;
    for (offset, grapheme) in text.grapheme_indices(true) {
        while offset >= span_end && span_index + 1 < source.spans.len() {
            span_index += 1;
            span_end += source.spans[span_index].text.len();
        }
        let Some(style) = source.spans.get(span_index) else {
            break;
        };
        if is_newline(grapheme) {
            pending_space = !result.spans.is_empty();
            continue;
        }
        let mut span = style.clone();
        span.text = if pending_space {
            format!(" {grapheme}")
        } else {
            grapheme.into()
        };
        pending_space = false;
        if let Some(previous) = result.spans.last_mut()
            && same_style(previous, &span)
        {
            previous.text.push_str(&span.text);
            continue;
        }
        result.spans.push(span);
    }
    result.wraps = false;
    result
}
fn same_style(a: &RichParagraphSpan, b: &RichParagraphSpan) -> bool {
    a.font == b.font && a.line_height == b.line_height && a.traits == b.traits && a.link == b.link
}

pub(crate) fn visible_paragraph(
    source: &RichParagraph,
    width: f32,
    line_limit: Option<usize>,
    truncates: bool,
    environment: &TextEnvironment,
) -> RichParagraph {
    if truncates {
        let normalized = single_line(source);
        let text = normalized.text();
        let candidate = |value: &str| {
            if value == text {
                normalized.clone()
            } else {
                prefix(
                    &normalized,
                    value.strip_suffix('…').unwrap_or(value).len(),
                    value.ends_with('…'),
                )
            }
        };
        let fitted = SingleLineText::fitting(&text, width, |value| {
            RichTextFlow::layout(&candidate(value), f32::INFINITY, environment).width
        });
        return candidate(&fitted);
    }
    if line_limit == Some(1) {
        return single_line(source);
    }
    if let Some(limit) = line_limit {
        if limit == 0 {
            return prefix(source, 0, false);
        }
        let flow = RichTextFlow::layout(source, width, environment);
        if flow.lines.len() > limit {
            let end = flow.lines[limit - 1].end_offset;
            let text = source.text();
            let end = text[..end]
                .trim_end_matches(['\r', '\n', '\u{2028}', '\u{2029}'])
                .len();
            return prefix(source, end, false);
        }
    }
    source.clone()
}
