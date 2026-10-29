//! Immutable diagnostic intervals and their squiggle/decoration geometry.

use cove_core::{DrawList, Point, TextLayoutResult};
use crate::theme::Theme;
use std::ops::Range;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextDiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

/// How the range is marked: a squiggle, faded glyphs for unused code, or a
/// line through deprecated names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextDiagnosticStyle {
    Underline,
    Faded,
    StruckThrough,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextDiagnostic {
    pub utf8_range: Range<usize>,
    pub severity: TextDiagnosticSeverity,
    pub message: String,
    pub style: TextDiagnosticStyle,
}

impl TextDiagnostic {
    pub fn new(
        utf8_range: Range<usize>,
        severity: TextDiagnosticSeverity,
        message: impl Into<String>,
    ) -> Self {
        Self {
            utf8_range,
            severity,
            message: message.into(),
            style: TextDiagnosticStyle::Underline,
        }
    }
}

/// An immutable interval index. Build when a diagnostic reply changes; paint
/// and hover query only overlapping ranges, including zero-width diagnostics.
#[derive(Clone)]
pub struct TextDiagnostics {
    root: Option<Rc<DiagnosticInterval>>,
    /// Whether any range fades its glyphs, so painting can skip the lookup.
    pub fades_text: bool,
}

impl PartialEq for TextDiagnostics {
    fn eq(&self, other: &Self) -> bool {
        match (&self.root, &other.root) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        }
    }
}

impl Eq for TextDiagnostics {}

impl TextDiagnostics {
    pub fn empty() -> Self {
        Self::new(&[])
    }

    pub fn new(diagnostics: &[TextDiagnostic]) -> Self {
        let fades_text = diagnostics
            .iter()
            .any(|diagnostic| diagnostic.style == TextDiagnosticStyle::Faded);
        let mut sorted = diagnostics.to_vec();
        sorted.sort_by_key(|diagnostic| diagnostic.utf8_range.start);
        Self {
            root: DiagnosticInterval::build(&sorted),
            fades_text,
        }
    }

    pub fn overlapping(&self, range: Range<usize>) -> Vec<TextDiagnostic> {
        let mut result = Vec::new();
        if let Some(root) = &self.root {
            root.collect(range, &mut result);
        }
        result
    }
}

struct DiagnosticInterval {
    value: TextDiagnostic,
    left: Option<Rc<DiagnosticInterval>>,
    right: Option<Rc<DiagnosticInterval>>,
    maximum_end: usize,
}

impl DiagnosticInterval {
    fn new(
        value: TextDiagnostic,
        left: Option<Rc<DiagnosticInterval>>,
        right: Option<Rc<DiagnosticInterval>>,
    ) -> Self {
        let maximum_end = (value.utf8_range.start + 1)
            .max(value.utf8_range.end)
            .max(left.as_ref().map_or(0, |node| node.maximum_end))
            .max(right.as_ref().map_or(0, |node| node.maximum_end));
        Self {
            value,
            left,
            right,
            maximum_end,
        }
    }

    fn build(values: &[TextDiagnostic]) -> Option<Rc<DiagnosticInterval>> {
        fn subtree(
            values: &[TextDiagnostic],
            range: Range<usize>,
        ) -> Option<Rc<DiagnosticInterval>> {
            if range.is_empty() {
                return None;
            }
            let middle = range.start + (range.end - range.start) / 2;
            Some(Rc::new(DiagnosticInterval::new(
                values[middle].clone(),
                subtree(values, range.start..middle),
                subtree(values, (middle + 1)..range.end),
            )))
        }
        subtree(values, 0..values.len())
    }

    fn collect(&self, range: Range<usize>, result: &mut Vec<TextDiagnostic>) {
        if self.maximum_end <= range.start {
            return;
        }
        if let Some(left) = &self.left {
            left.collect(range.clone(), result);
        }
        if self.value.utf8_range.start >= range.end {
            return;
        }
        if self
            .value
            .utf8_range
            .end
            .max(self.value.utf8_range.start + 1)
            > range.start
        {
            result.push(self.value.clone());
        }
        if let Some(right) = &self.right {
            right.collect(range, result);
        }
    }
}

pub struct TextDiagnosticSegment {
    pub diagnostic: TextDiagnostic,
    pub x: f32,
    pub width: f32,
    pub baseline_y: f32,
    pub middle_y: f32,
}

pub struct TextDiagnosticGeometry;

impl TextDiagnosticGeometry {
    pub fn segments(
        layout: &TextLayoutResult,
        diagnostics: &TextDiagnostics,
        line_height: f32,
    ) -> Vec<TextDiagnosticSegment> {
        let mut result = Vec::new();
        for line in &layout.lines {
            let first = line.start_cluster as usize;
            let last = line.end_cluster as usize;
            for diagnostic in diagnostics.overlapping(first..first.max(last)) {
                let x = |byte: usize| -> f32 {
                    if byte <= first {
                        return 0.0;
                    }
                    if byte >= last {
                        return line.width;
                    }
                    // A multi-byte or multi-glyph cluster is one underline span.
                    line.glyphs
                        .iter()
                        .rev()
                        .find(|glyph| glyph.cluster as usize <= byte)
                        .map_or(0.0, |glyph| glyph.x)
                };
                let lower = x(first.max(diagnostic.utf8_range.start));
                let upper_byte = last.min(diagnostic.utf8_range.end);
                let upper = if upper_byte >= last {
                    line.width
                } else {
                    line.glyphs
                        .iter()
                        .find(|glyph| glyph.cluster as usize >= upper_byte)
                        .map_or(line.width, |glyph| glyph.x)
                };
                result.push(TextDiagnosticSegment {
                    diagnostic,
                    x: lower,
                    width: 6.0_f32.max(upper - lower),
                    baseline_y: line.top_y + line_height - 2.0,
                    middle_y: line.top_y + line_height / 2.0,
                });
            }
        }
        result
    }

    pub fn draw(
        segments: &[TextDiagnosticSegment],
        origin: (f32, f32),
        opacity: f32,
        theme: &Theme,
        list: &mut DrawList,
    ) {
        for segment in segments {
            let color = match segment.diagnostic.severity {
                TextDiagnosticSeverity::Error => theme.colors.error,
                TextDiagnosticSeverity::Warning => theme.colors.warning,
                TextDiagnosticSeverity::Information => theme.colors.accent,
                TextDiagnosticSeverity::Hint => theme.colors.on_surface_muted,
            };
            let start = origin.0 + segment.x;
            let y = origin.1 + segment.baseline_y;
            if segment.diagnostic.style == TextDiagnosticStyle::StruckThrough {
                let _ = list.add_line(
                    Point::new(start, origin.1 + segment.middle_y),
                    Point::new(start + segment.width, origin.1 + segment.middle_y),
                    1.0,
                    theme.colors.on_surface_muted.multiplied_alpha(opacity),
                );
            }
            // Unused or deprecated code that is merely noted needs no squiggle.
            if segment.diagnostic.style != TextDiagnosticStyle::Underline
                && matches!(
                    segment.diagnostic.severity,
                    TextDiagnosticSeverity::Hint | TextDiagnosticSeverity::Information
                )
            {
                continue;
            }
            let mut x = start;
            let mut up = false;
            while x < start + segment.width {
                let end = (start + segment.width).min(x + 2.0);
                let _ = list.add_line(
                    Point::new(x, y + if up { 1.5 } else { 0.0 }),
                    Point::new(end, y + if up { 0.0 } else { 1.5 }),
                    1.0,
                    color.multiplied_alpha(opacity),
                );
                x = end;
                up = !up;
            }
        }
    }
}
