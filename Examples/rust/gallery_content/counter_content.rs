//! Adapts the portable counter's custom text primitives to the native font service.
//!
//! Swift source: `GalleryContent/CounterContent.swift`. The counter authors
//! [`CounterTextRun`] attachments and leaves painting to the host; this wrapper
//! supplies the painter that shapes each run into the shared font atlas.

use cove_core::DrawList;
use cove_core::scene::{Node, Point};
use cove_text::{DrawListText, TextEnvironmentHolder};
use cove_ui_compose::{AnyView, PrimitiveView, View, ViewExt, ViewKind};

use crate::counter::{CounterText, CounterTextRun, CounterView, ProvidedPainter};

/// The shared counter with the native font service attached.
pub struct CounterContent {
    counter: CounterView,
    painter: ProvidedPainter,
}

impl CounterContent {
    pub fn new(counter: CounterView) -> Self {
        Self {
            counter,
            painter: ProvidedPainter::new(paint_with_font_service),
        }
    }

    pub fn counter(&self) -> &CounterView {
        &self.counter
    }

    /// Checkpoint of the wrapped counter, so a host can persist this exact view.
    pub fn checkpoint(&self) -> std::collections::BTreeMap<String, String> {
        self.counter.checkpoint()
    }

    pub fn restore(&self, values: &std::collections::BTreeMap<String, String>) -> bool {
        self.counter.restore(values)
    }
}

impl Default for CounterContent {
    fn default() -> Self {
        Self::new(CounterView::new())
    }
}

/// Shapes `run` into the current [`cove_text::TextEnvironment`] atlas and paints it.
///
/// `origin` is the node's top-left corner; `run.inset` is the authored leading
/// inset the counter applies inside its padding.
fn paint_with_font_service(run: &CounterTextRun, list: &mut DrawList, origin: Point) {
    let Some(environment) = TextEnvironmentHolder::current() else {
        return;
    };
    let font = cove_core::Font::system(run.size, cove_core::FontWeight::Regular);
    let layout = environment.cached_layout(
        &run.text,
        Some(&font),
        Some(run.size * 1.3),
        f32::INFINITY,
        cove_text::TextAlignment::Leading,
        0.0,
    );
    let _ = list.add_text(
        &layout,
        Point::new(origin.x + run.inset, origin.y),
        run.color,
        environment.atlas_texture_id,
        Some(&environment.atlas),
        None,
    );
}

impl View for CounterContent {
    fn body(&self) -> AnyView {
        AnyView::new(CounterHost {
            counter: self.counter.clone(),
            painter: self.painter.clone(),
        })
    }
}

/// Re-composes the counter body with the painter provided to its subtree.
struct CounterHost {
    counter: CounterView,
    painter: ProvidedPainter,
}

impl CounterHost {
    fn body_view(&self) -> AnyView {
        self.counter.body()
    }
}

impl PrimitiveView for CounterHost {
    fn make_node(&self) -> Node {
        Node::new()
    }

    fn update_node(&self, _node: &Node) {}
}

impl View for CounterHost {
    fn body(&self) -> AnyView {
        let provided = self.painter.clone();
        AnyView::new(
            BodyProxy {
                body: self.body_view(),
            }
            .composition_local(CounterText::painter(), Some(provided)),
        )
    }

    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Component
    }
}

/// A view whose body is already materialised.
struct BodyProxy {
    body: AnyView,
}

impl View for BodyProxy {
    fn body(&self) -> AnyView {
        self.body.clone()
    }
}
