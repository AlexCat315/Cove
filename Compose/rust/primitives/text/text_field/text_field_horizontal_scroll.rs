//! Translation of Primitives/Text/TextField/TextFieldHorizontalScroll.swift.
//!
//! Horizontal extent is learned from shaped rows; discovering a wide row never
//! requires scanning the complete document. An edit or typography change
//! invalidates the observed extent while preserving the position until the new
//! visible window can clamp it.
use crate::primitives::text::text_buffer::TextBuffer;
use crate::primitives::text::text_document_layout::TextLineGeometry;

#[derive(Clone, Debug, Default)]
pub struct HorizontalScrollState {
    buffer: Option<TextBuffer>,
    geometry: Option<TextLineGeometry>,
    pub offset: f32,
    pub viewport_width: f32,
    pub content_width: f32,
    pub leading_inset: f32,
}

impl HorizontalScrollState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn maximum(&self) -> f32 {
        (self.content_width - self.viewport_width).max(0.0)
    }

    /// Relearn the horizontal extent for the current buffer and typography.
    /// `reveal_caret` scrolls the caret into view, keeping an 8-point margin.
    pub fn synchronize(
        &mut self,
        buffer: &TextBuffer,
        geometry: &TextLineGeometry,
        viewport_width: f32,
        measured_width: f32,
        reveal_caret: Option<f32>,
    ) {
        let changed = self
            .buffer
            .as_ref()
            .is_none_or(|current| !current.has_same_contents(buffer))
            || self.geometry.as_ref() != Some(geometry);
        if changed {
            self.content_width = 0.0;
            self.buffer = Some(buffer.clone());
            self.geometry = Some(geometry.clone());
        }
        self.viewport_width = viewport_width.max(1.0);
        let margin = 8.0_f32.min(self.viewport_width / 4.0);
        self.content_width = self.content_width.max(measured_width + margin);
        if let Some(x) = reveal_caret {
            self.content_width = self.content_width.max(x + margin);
            if x < self.offset + margin {
                self.offset = (x - margin).max(0.0);
            } else if x > self.offset + self.viewport_width - margin {
                self.offset = x + margin - self.viewport_width;
            }
        }
        self.offset = self.offset.clamp(0.0, self.maximum());
    }
}
