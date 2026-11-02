mod support;
use cove_composition::{AnyView, Scope, View};
use cove_examples::gallery_content::catalog::GalleryPage;
use cove_examples::gallery_content::gallery_view::GalleryView;
use cove_ui_compose::theme::styles::appearance_modifier::Appearance;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::time::Instant;
use support::GalleryScene;

struct Shared(Rc<GalleryView>);
impl View for Shared {
    fn body(&self, cx: &Scope) -> AnyView {
        View::body(self.0.as_ref(), cx)
    }
}

#[test]
#[ignore]
fn shaping_probe() {
    let mode = if std::env::var_os("ZZ_SERIAL").is_some() { "serial" } else { "batched" };
    for page in [
        GalleryPage::Introduction,
        GalleryPage::Markdown,
        GalleryPage::List,
        GalleryPage::TextEditor,
        GalleryPage::Chart,
        GalleryPage::Table,
        GalleryPage::DataTable,
    ] {
        let (mut total, mut shape) = (0.0, 0.0);
        let rounds = 5;
        for round in 0..=rounds {
            let gallery = Rc::new(GalleryView::new(page, Appearance::Light, 1.0));
            cove_text::SHAPE_NANOS.store(0, Ordering::Relaxed);
            cove_text::SHAPE_CALLS.store(0, Ordering::Relaxed);
            let start = Instant::now();
            GalleryScene::run(1280.0, 820.0, move |_| AnyView::new(Shared(gallery.clone())), |scene| {
                scene.settle_n(1);
            });
            if round > 0 {
                total += start.elapsed().as_secs_f64() * 1e3;
                shape += cove_text::SHAPE_NANOS.load(Ordering::Relaxed) as f64 / 1e6;
            }
        }
        eprintln!("{mode} {page:?}: cold mount={:.2}ms shaping={:.2}ms texts={}", total / rounds as f64, shape / rounds as f64, cove_text::SHAPE_CALLS.load(Ordering::Relaxed));
    }
}
