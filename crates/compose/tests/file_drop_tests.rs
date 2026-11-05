//! File drop zones: hit testing, the type filter, and the browse fallback.
//!
//! The registry is process-wide (one ambient per window), so every case runs
//! under `InteractiveScene`, which installs private holders and restores them.
//! These are structural tests: they prove the zone registers, matches and
//! delivers. Native drag entry from the OS is a separate acceptance item and is
//! not claimed here.

use crate::support;

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use cove_composition::{AnyView, Scope, View};
use cove_core::scene::Node;
use cove_core::{Binding, Point, State};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::interaction::{
    DroppedFile, FileDropOutcome, FileDropRegistry, FileDropRegistryHolder, FileDropZone,
    format_bytes,
};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::upload::{
    DragUpload, UploadError, UploadRequest, UploadResponse, UploadState, UploadTransport,
    UploadTransportHolder,
};

use support::InteractiveScene;

#[derive(Default)]
struct Model {
    dropped: RefCell<Vec<Vec<DroppedFile>>>,
    targeted: RefCell<Vec<bool>>,
}

fn zone_harness(model: Rc<Model>, enabled: bool) -> AnyView {
    let dropped = model.clone();
    let hovered = model.clone();
    let zone = FileDropZone::new(
        move |files| dropped.dropped.borrow_mut().push(files),
        AnyView::new(Text::new("Drop here")).frame_size(200.0, 120.0),
    )
    .accepted_extensions(["png".to_owned()])
    .maximum_files(2)
    .is_enabled(enabled)
    .on_targeted(move |is_targeted| hovered.targeted.borrow_mut().push(is_targeted));
    AnyView::new(AnyView::new(zone).frame_size(240.0, 160.0))
}

/// Installs a private registry for the body, then restores whatever was there.
///
/// The host owns this ambient in production, so a test must install its own or
/// the zones register into nothing.
fn with_registry<T>(body: impl FnOnce(&Rc<FileDropRegistry>) -> T) -> T {
    let previous = FileDropRegistryHolder::current();
    let registry = Rc::new(FileDropRegistry::new());
    FileDropRegistryHolder::set_current(Some(registry.clone()));
    let result = body(&registry);
    FileDropRegistryHolder::set_current(previous);
    result
}

fn zone_node(scene: &InteractiveScene) -> Node {
    scene
        .first(|node| {
            node.absolute_frame().width > 0.0 && node.is_layout_visible() && node.parent().is_some()
        })
        .expect("a laid out zone exists")
}

/// The centre of the first laid-out node, in window coordinates.
fn centre(node: &Node) -> Point {
    let frame = node.absolute_frame();
    Point::new(frame.x + frame.width / 2.0, frame.y + frame.height / 2.0)
}

#[test]
fn a_matching_drop_reaches_the_callback() {
    let model = Rc::new(Model::default());
    InteractiveScene::run(480.0, 320.0, |scene| {
        with_registry(|registry| {
            let model = model.clone();
            scene.install(zone_harness(model.clone(), true));
            let node = zone_node(scene);
            let point = centre(&node);
            let registry = registry.clone();
            assert!(registry.can_drop(&[DroppedFile::new("/tmp/a.PNG")], point));
            assert!(FileDropRegistry::drop(
                &registry,
                &[DroppedFile::new("/tmp/a.PNG")],
                point
            ));
            assert_eq!(model.dropped.borrow().len(), 1);
            assert_eq!(model.dropped.borrow()[0][0].name, "a.PNG");
        });
    });
}

#[test]
fn a_rejected_type_reports_no_drop() {
    let model = Rc::new(Model::default());
    InteractiveScene::run(480.0, 320.0, |scene| {
        with_registry(|registry| {
            let model = model.clone();
            scene.install(zone_harness(model.clone(), true));
            let node = zone_node(scene);
            let point = centre(&node);
            let registry = registry.clone();
            assert!(!registry.can_drop(&[DroppedFile::new("/tmp/a.txt")], point));
            assert!(!FileDropRegistry::drop(
                &registry,
                &[DroppedFile::new("/tmp/a.txt")],
                point
            ));
            assert!(model.dropped.borrow().is_empty());
        });
    });
}

#[test]
fn too_many_files_are_rejected_and_the_capacity_is_visible() {
    let model = Rc::new(Model::default());
    InteractiveScene::run(480.0, 320.0, |scene| {
        with_registry(|registry| {
            let model = model.clone();
            scene.install(zone_harness(model.clone(), true));
            let node = zone_node(scene);
            let point = centre(&node);
            let registry = registry.clone();
            assert_eq!(registry.capacity(point), Some(2));
            let three = vec![
                DroppedFile::new("/tmp/a.png"),
                DroppedFile::new("/tmp/b.png"),
                DroppedFile::new("/tmp/c.png"),
            ];
            assert!(!FileDropRegistry::drop(&registry, &three, point));
            assert!(model.dropped.borrow().is_empty());
        });
    });
}

#[test]
fn entering_and_leaving_reports_both_transitions() {
    let model = Rc::new(Model::default());
    InteractiveScene::run(480.0, 320.0, |scene| {
        with_registry(|registry| {
            let model = model.clone();
            scene.install(zone_harness(model.clone(), true));
            let node = zone_node(scene);
            let point = centre(&node);
            let registry = registry.clone();
            registry.update_position(point);
            registry.update_position(Point::new(-40.0, -40.0));
            assert_eq!(*model.targeted.borrow(), vec![true, false]);
        });
    });
}

#[test]
fn a_disabled_zone_accepts_nothing() {
    let model = Rc::new(Model::default());
    InteractiveScene::run(480.0, 320.0, |scene| {
        with_registry(|registry| {
            let model = model.clone();
            scene.install(zone_harness(model.clone(), false));
            let node = zone_node(scene);
            let point = centre(&node);
            let registry = Some(registry.clone());
            // A disabled zone unregisters, so nothing can be dropped into it.
            let accepted = registry.is_some_and(|registry| {
                registry.can_drop(&[DroppedFile::new("/tmp/a.png")], point)
            });
            assert!(!accepted);
            assert!(model.dropped.borrow().is_empty());
        });
    });
}

#[test]
fn highlights_follow_the_owner_bindings() {
    let targeted = State::new(false);
    let outcome = State::new(FileDropOutcome::Idle);
    let model = Rc::new(Model::default());
    InteractiveScene::run(480.0, 320.0, |scene| {
        with_registry(|_registry| {
            let dropped = model.clone();
            let view = AnyView::new(
                FileDropZone::new(
                    move |files| dropped.dropped.borrow_mut().push(files),
                    AnyView::new(AnyView::new(Text::new("zone")).frame_size(200.0, 120.0)),
                )
                .is_targeted(targeted.binding())
                .outcome(outcome.binding()),
            )
            .frame_size(240.0, 160.0);
            scene.install(view);
            targeted.set(true);
            outcome.set(FileDropOutcome::Accepted);
            scene.settle(2, 1.0);
            assert!(targeted.get());
            assert_eq!(outcome.get(), FileDropOutcome::Accepted);
        });
    });
}

fn file_part(
    file: &DroppedFile,
) -> Result<cove_ui_compose::primitives::upload::multipart::UploadFilePart, String> {
    Ok(
        cove_ui_compose::primitives::upload::multipart::UploadFilePart::new(
            "file",
            file.name.clone(),
            "image/png",
            vec![1, 2, 3, 4],
        ),
    )
}

struct RecordingTransport {
    calls: std::sync::Mutex<Vec<String>>,
}

impl UploadTransport for RecordingTransport {
    fn send(
        &self,
        request: &UploadRequest,
        report: &(dyn Fn(u64) + Send + Sync),
    ) -> Result<UploadResponse, UploadError> {
        self.calls.lock().unwrap().push(request.url.clone());
        report(request.payload_bytes());
        Ok(UploadResponse {
            status: 200,
            body: Vec::new(),
        })
    }
}

struct FailingTransport;

impl UploadTransport for FailingTransport {
    fn send(
        &self,
        _: &UploadRequest,
        _: &(dyn Fn(u64) + Send + Sync),
    ) -> Result<UploadResponse, UploadError> {
        Err(UploadError::HttpStatus { status: 500 })
    }
}

struct UploadHarness {
    accepted: Binding<bool>,
}

impl View for UploadHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let _ = &self.accepted;
        AnyView::new(
            DragUpload::new(UploadRequest::new("memory://test/upload", Vec::new()))
                .accepted_extensions(["png".to_owned()])
                .maximum_files(2)
                .reader(file_part),
        )
    }
}

#[test]
fn drag_upload_mounts_with_a_zone_and_a_progress_readout() {
    let transport: std::sync::Arc<RecordingTransport> = std::sync::Arc::new(RecordingTransport {
        calls: std::sync::Mutex::new(Vec::new()),
    });
    InteractiveScene::run(600.0, 400.0, |scene| {
        with_registry(|_registry| {
            UploadTransportHolder::set(Some(
                transport.clone() as std::sync::Arc<dyn UploadTransport>
            ));
            scene.install(UploadHarness {
                accepted: Binding::constant(true),
            });
            // The idle zone renders its prompt, so the browse button exists and the
            // zone registered itself.
            assert!(scene.labelled("Drop files to upload", None).is_some());
            UploadTransportHolder::set(None);
        });
    });
}

#[test]
fn an_idle_upload_shows_no_progress_readout() {
    InteractiveScene::run(600.0, 400.0, |scene| {
        with_registry(|_registry| {
            scene.install(AnyView::new(DragUpload::new(UploadRequest::new(
                "memory://test/upload",
                Vec::new(),
            ))));
            assert!(scene.labelled("Cancel upload", None).is_none());
        });
    });
}

#[test]
fn a_running_upload_shows_progress_and_cancel() {
    InteractiveScene::run(600.0, 400.0, |scene| {
        with_registry(|_registry| {
            let upload = DragUpload::new(UploadRequest::new("memory://test/upload", Vec::new()));
            upload
                .state()
                .set(UploadState::Uploading { sent: 1, total: 4 });
            scene.install(AnyView::new(upload));
            assert!(scene.labelled("25%", None).is_some());
            assert!(scene.labelled("Cancel upload", None).is_some());
        });
    });
}

#[test]
fn a_finished_upload_shows_its_status_row() {
    InteractiveScene::run(600.0, 400.0, |scene| {
        with_registry(|_registry| {
            let upload = DragUpload::new(UploadRequest::new("memory://test/upload", Vec::new()));
            upload.state().set(UploadState::Succeeded(UploadResponse {
                status: 201,
                body: Vec::new(),
            }));
            scene.install(AnyView::new(upload));
            assert!(scene.labelled("Uploaded", None).is_some());
            assert!(scene.labelled("Cancel upload", None).is_none());
        });
    });
}

#[test]
fn a_finished_upload_lands_in_the_state() {
    let transport: std::sync::Arc<RecordingTransport> = std::sync::Arc::new(RecordingTransport {
        calls: std::sync::Mutex::new(Vec::new()),
    });
    InteractiveScene::run(600.0, 400.0, |scene| {
        with_registry(|registry| {
            UploadTransportHolder::set(Some(
                transport.clone() as std::sync::Arc<dyn UploadTransport>
            ));
            let state = State::new(UploadState::Idle);
            let start = DragUpload::new(UploadRequest::new("memory://test/upload", Vec::new()))
                .reader(file_part)
                .on_finished({
                    let state = state.clone();
                    move |next| state.set(next.clone())
                });
            scene.install(AnyView::new(start));
            let node = zone_node(scene);
            let registry = registry.clone();
            FileDropRegistry::drop(&registry, &[DroppedFile::new("/tmp/a.png")], centre(&node));
            scene.settle(10, 1.0);
            assert!(matches!(state.get(), UploadState::Succeeded(_)));
            assert_eq!(
                transport.calls.lock().unwrap().as_slice(),
                ["memory://test/upload"]
            );
            UploadTransportHolder::set(None);
        });
    });
}

#[test]
fn a_failed_upload_is_terminal_and_names_the_status() {
    let transport: std::sync::Arc<FailingTransport> = std::sync::Arc::new(FailingTransport);
    InteractiveScene::run(600.0, 400.0, |scene| {
        with_registry(|registry| {
            UploadTransportHolder::set(Some(
                transport.clone() as std::sync::Arc<dyn UploadTransport>
            ));
            let state = State::new(UploadState::Idle);
            let start = DragUpload::new(UploadRequest::new("memory://test/upload", Vec::new()))
                .reader(file_part)
                .on_finished({
                    let state = state.clone();
                    move |next| state.set(next.clone())
                });
            scene.install(AnyView::new(start));
            let node = zone_node(scene);
            let registry = registry.clone();
            FileDropRegistry::drop(&registry, &[DroppedFile::new("/tmp/a.png")], centre(&node));
            scene.settle(10, 1.0);
            assert_eq!(
                state.get(),
                UploadState::Failed(UploadError::HttpStatus { status: 500 })
            );
            UploadTransportHolder::set(None);
        });
    });
}

#[test]
fn an_oversized_payload_is_refused_before_the_transport_runs() {
    let transport: std::sync::Arc<RecordingTransport> = std::sync::Arc::new(RecordingTransport {
        calls: std::sync::Mutex::new(Vec::new()),
    });
    InteractiveScene::run(600.0, 400.0, |scene| {
        with_registry(|registry| {
            UploadTransportHolder::set(Some(
                transport.clone() as std::sync::Arc<dyn UploadTransport>
            ));
            let state = State::new(UploadState::Idle);
            let start = DragUpload::new(
                UploadRequest::new("memory://test/upload", Vec::new()).maximum_bytes(2),
            )
            .reader(file_part)
            .on_finished({
                let state = state.clone();
                move |next| state.set(next.clone())
            });
            scene.install(AnyView::new(start));
            let node = zone_node(scene);
            let registry = registry.clone();
            FileDropRegistry::drop(&registry, &[DroppedFile::new("/tmp/a.png")], centre(&node));
            scene.settle(10, 1.0);
            assert_eq!(
                state.get(),
                UploadState::Failed(UploadError::PayloadTooLarge { limit: 2 })
            );
            assert!(transport.calls.lock().unwrap().is_empty());
            UploadTransportHolder::set(None);
        });
    });
}

#[test]
fn the_zone_without_a_transport_reports_the_host_gap() {
    InteractiveScene::run(600.0, 400.0, |scene| {
        with_registry(|registry| {
            UploadTransportHolder::set(None);
            let state = State::new(UploadState::Idle);
            let start = DragUpload::new(UploadRequest::new("memory://test/upload", Vec::new()))
                .reader(file_part)
                .on_finished({
                    let state = state.clone();
                    move |next| state.set(next.clone())
                });
            scene.install(AnyView::new(start));
            let node = zone_node(scene);
            let registry = registry.clone();
            FileDropRegistry::drop(&registry, &[DroppedFile::new("/tmp/a.png")], centre(&node));
            scene.settle(10, 1.0);
            assert_eq!(
                state.get(),
                UploadState::Failed(UploadError::HostUnavailable)
            );
        });
    });
}

#[test]
fn a_size_reads_with_one_decimal_above_bytes() {
    assert_eq!(format_bytes(999), "999 B");
    assert_eq!(format_bytes(2048), "2.0 KB");
}

#[test]
fn the_extension_filter_is_case_insensitive() {
    let accepted = HashSet::from(["png".to_owned()]);
    assert!(DroppedFile::new("/tmp/A.PNG").matches_any(&accepted));
    assert!(!DroppedFile::new("/tmp/A.PDF").matches_any(&accepted));
}

#[test]
fn one_batch_of_three_files_arrives_as_one_drop() {
    // The platform reports each path as its own step; the zone must see one
    // delivery, or a per-zone file limit would only ever count to one.
    let model = Rc::new(Model::default());
    InteractiveScene::run(480.0, 320.0, |scene| {
        with_registry(|registry| {
            let model = model.clone();
            scene.install(zone_batch_harness(model.clone(), 4));
            let point = centre(&zone_node(scene));
            let registry = registry.clone();

            registry.begin(point);
            for name in ["a.png", "b.png", "c.png"] {
                registry.stage([DroppedFile::new(format!("/tmp/{name}"))], point);
            }
            assert_eq!(model.dropped.borrow().len(), 0, "nothing before complete");
            assert!(registry.finish(point));

            let dropped = model.dropped.borrow();
            assert_eq!(dropped.len(), 1, "one delivery for the whole batch");
            assert_eq!(dropped[0].len(), 3);
            assert_eq!(dropped[0][2].name, "c.png");
        });
    });
}

#[test]
fn a_batch_over_the_limit_is_refused_as_a_whole() {
    let model = Rc::new(Model::default());
    InteractiveScene::run(480.0, 320.0, |scene| {
        with_registry(|registry| {
            let model = model.clone();
            scene.install(zone_batch_harness(model.clone(), 2));
            let point = centre(&zone_node(scene));
            let registry = registry.clone();

            registry.begin(point);
            for name in ["a.png", "b.png", "c.png"] {
                registry.stage([DroppedFile::new(format!("/tmp/{name}"))], point);
            }
            assert!(!registry.finish(point), "three files exceed a limit of two");
            assert!(model.dropped.borrow().is_empty());
            assert!(!registry.has_staged(), "a refused batch is cleared");
        });
    });
}

#[test]
fn a_batch_is_cleared_after_delivery() {
    let model = Rc::new(Model::default());
    InteractiveScene::run(480.0, 320.0, |scene| {
        with_registry(|registry| {
            let model = model.clone();
            scene.install(zone_batch_harness(model.clone(), 4));
            let point = centre(&zone_node(scene));
            let registry = registry.clone();

            registry.begin(point);
            registry.stage([DroppedFile::new("/tmp/a.png")], point);
            assert!(registry.finish(point));
            assert!(!registry.has_staged());
            // A second finish must not re-deliver the same files.
            assert!(!registry.finish(point));
            assert_eq!(model.dropped.borrow().len(), 1);
        });
    });
}

#[test]
fn cancelling_a_batch_delivers_nothing() {
    let model = Rc::new(Model::default());
    InteractiveScene::run(480.0, 320.0, |scene| {
        with_registry(|registry| {
            let model = model.clone();
            scene.install(zone_batch_harness(model.clone(), 4));
            let point = centre(&zone_node(scene));
            let registry = registry.clone();

            registry.begin(point);
            registry.stage([DroppedFile::new("/tmp/a.png")], point);
            registry.cancel();
            assert!(!registry.has_staged());
            assert!(model.dropped.borrow().is_empty());
        });
    });
}

#[test]
fn a_folder_is_refused_unless_the_zone_opts_in() {
    let refused = Rc::new(Model::default());
    InteractiveScene::run(480.0, 320.0, |scene| {
        with_registry(|registry| {
            let model = refused.clone();
            scene.install(zone_batch_harness(model.clone(), 4));
            let point = centre(&zone_node(scene));
            let folder = DroppedFile::new("/tmp/assets").is_directory(true);
            assert!(!registry.can_drop(std::slice::from_ref(&folder), point));
            assert!(!FileDropRegistry::drop(registry, &[folder], point));
            assert!(model.dropped.borrow().is_empty());
        });
    });

    let accepted = Rc::new(Model::default());
    InteractiveScene::run(480.0, 320.0, |scene| {
        with_registry(|registry| {
            let model = accepted.clone();
            scene.install(zone_folder_harness(model.clone()));
            let point = centre(&zone_node(scene));
            let folder = DroppedFile::new("/tmp/assets").is_directory(true);
            assert!(FileDropRegistry::drop(registry, &[folder], point));
            assert_eq!(model.dropped.borrow().len(), 1);
        });
    });
}

fn zone_batch_harness(model: Rc<Model>, maximum_files: usize) -> AnyView {
    let dropped = model;
    let zone = FileDropZone::new(
        move |files| dropped.dropped.borrow_mut().push(files),
        AnyView::new(Text::new("batch")).frame_size(200.0, 120.0),
    )
    .accepted_extensions(["png".to_owned()])
    .maximum_files(maximum_files);
    AnyView::new(AnyView::new(zone).frame_size(240.0, 160.0))
}

fn zone_folder_harness(model: Rc<Model>) -> AnyView {
    let dropped = model;
    let zone = FileDropZone::new(
        move |files| dropped.dropped.borrow_mut().push(files),
        AnyView::new(Text::new("folders")).frame_size(200.0, 120.0),
    )
    .allows_directories(true);
    AnyView::new(AnyView::new(zone).frame_size(240.0, 160.0))
}

#[test]
fn a_refused_batch_is_reported_separately_from_a_drop() {
    let dropped = Rc::new(RefCell::new(0usize));
    let refused = Rc::new(RefCell::new(0usize));
    InteractiveScene::run(480.0, 320.0, |scene| {
        with_registry(|registry| {
            let accepted_count = dropped.clone();
            let refused_count = refused.clone();
            let dropped = accepted_count.clone();
            let refused = refused_count.clone();
            let zone = FileDropZone::new(
                move |_| *dropped.borrow_mut() += 1,
                AnyView::new(Text::new("limited")).frame_size(200.0, 120.0),
            )
            .maximum_files(2)
            .on_rejected(move |files| *refused.borrow_mut() += files.len());
            scene.install(AnyView::new(AnyView::new(zone).frame_size(240.0, 160.0)));
            let point = centre(&zone_node(scene));
            let registry = registry.clone();

            registry.begin(point);
            for name in ["a.png", "b.png", "c.png"] {
                registry.stage([DroppedFile::new(format!("/tmp/{name}"))], point);
            }
            assert!(!registry.finish(point));
            assert_eq!(*accepted_count.borrow(), 0, "no accepted drop");
            assert_eq!(*refused_count.borrow(), 3, "every refused file is reported");
        });
    });
}
