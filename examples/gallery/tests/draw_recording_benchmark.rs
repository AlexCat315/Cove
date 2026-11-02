//! Draw-list recording cost on heavy gallery pages. Ignored by default; run with
//! `cargo test -p cove-examples --release --test draw_recording_benchmark -- --ignored --nocapture`.
//!
//! Reports, per page: a full re-record with every layer cache invalidated, the
//! first frame after that (containers recomposed from cached children), a
//! steady-state frame on a reused list, and the draw hooks alone.
mod support;

use cove_composition::{AnyView, Scope, View};
use cove_core::DrawList;
use cove_core::scene::{LayerAwareNodeRenderer, RenderObject};
use cove_examples::gallery_content::catalog::GalleryPage;
use cove_examples::gallery_content::gallery_view::GalleryView;
use cove_ui_compose::theme::styles::appearance_modifier::Appearance;
use std::rc::Rc;
use std::time::Instant;
use support::GalleryScene;

struct Shared(Rc<GalleryView>);
impl View for Shared {
    fn body(&self, cx: &Scope) -> AnyView {
        View::body(self.0.as_ref(), cx)
    }
}

fn invalidate(object: &RenderObject) {
    object.set_cache_invalid(true);
    for child in object.children() {
        invalidate(&child);
    }
}

fn frame() -> DrawList {
    let mut list = DrawList::default();
    list.pixel_scale = 2.0;
    list
}

#[test]
#[ignore]
fn draw_recording_cost() {
    for page in [
        GalleryPage::Introduction,
        GalleryPage::Chart,
        GalleryPage::Markdown,
        GalleryPage::List,
        GalleryPage::TextEditor,
        GalleryPage::Avatar,
    ] {
        let gallery = Rc::new(GalleryView::new(page, Appearance::Light, 1.0));
        GalleryScene::run(
            1280.0,
            820.0,
            move |_| AnyView::new(Shared(gallery.clone())),
            |scene| {
                scene.settle_n(10);
                let tree = scene.graph.render_tree();
                let mut renderer = LayerAwareNodeRenderer::new();
                renderer.render(tree, &mut frame());
                let rounds = 30;
                let (mut full, mut after, mut hooks) = (0.0, 0.0, 0.0);
                for _ in 0..rounds {
                    invalidate(&tree.root().expect("a mounted root"));
                    let start = Instant::now();
                    renderer.render(tree, &mut frame());
                    full += start.elapsed().as_secs_f64();
                    let start = Instant::now();
                    renderer.render(tree, &mut frame());
                    after += start.elapsed().as_secs_f64();
                }
                let mut list = frame();
                let steady_frames = 1000;
                let start = Instant::now();
                for _ in 0..steady_frames {
                    list.reset();
                    list.pixel_scale = 2.0;
                    renderer.render(tree, &mut list);
                }
                let steady = start.elapsed().as_secs_f64() / steady_frames as f64;
                let plan = tree.paint_plan();
                for _ in 0..rounds {
                    let mut list = frame();
                    let start = Instant::now();
                    for object in &plan.objects {
                        if let Some(node) = object.node()
                            && let Some(draw) = node.draw()
                        {
                            draw(&mut list, cove_core::Point::default());
                        }
                    }
                    hooks += start.elapsed().as_secs_f64();
                }
                let per = |total: f64| total / rounds as f64 * 1e3;
                eprintln!(
                    "{page:?}: objects={} full re-record={:.3}ms first frame after={:.3}ms \
                     steady={:.4}ms hooks={:.3}ms",
                    plan.objects.len(),
                    per(full),
                    per(after),
                    steady * 1e3,
                    per(hooks),
                );
            },
        );
    }
}
