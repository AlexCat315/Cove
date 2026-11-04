//! `DragUploadStory`: drop a file, watch it upload.
//!
//! Swift source: `GalleryContent/Stories/Data/DataStories.swift`. The Swift
//! gallery had no upload sample, so this page documents the Rust-only component
//! that closes the gap: the drop zone, the upload state machine and the progress
//! readout in one place.
//!
//! The transport is injected and never touches the network: a gallery page that
//! reached the internet on mount would be a side effect nobody asked for.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use cove_composition::Scope;
use cove_core::State;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_ui_compose::primitives::interaction::DroppedFile;
use cove_ui_compose::primitives::interaction::file_drop::format_bytes;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::upload::content_type_for;
use cove_ui_compose::primitives::upload::multipart::UploadFilePart;
use cove_ui_compose::primitives::upload::{
    DragUpload, UploadError, UploadRequest, UploadResponse, UploadState, UploadTransport,
    UploadTransportHolder,
};
use cove_ui_compose::theme::tokens::{
    SemanticColorRef, SemanticFontRef, SemanticStyleViewExt,
};
use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView, View};

use crate::gallery_content::catalog::GalleryOptions;
use crate::gallery_content::stories::StorySection;

/// A transport that accepts everything and reports a fixed receipt.
///
/// Deliberately synchronous and offline: the gallery verifies wiring, not a
/// server. A real host installs `HttpUploadTransport` instead.
struct SampleTransport {
    calls: AtomicU64,
}

impl UploadTransport for SampleTransport {
    fn send(
        &self,
        request: &UploadRequest,
        report: &(dyn Fn(u64) + Send + Sync),
    ) -> Result<UploadResponse, UploadError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        report(request.payload_bytes());
        let receipt = format!(
            "{{\"accepted\":{},\"bytes\":{}}}",
            request.files.len(),
            request.payload_bytes()
        );
        Ok(UploadResponse {
            status: 201,
            body: receipt.into_bytes(),
        })
    }
}

/// `DragUploadStory`: the drop zone, its upload and the accepted filter.
pub struct DragUploadStory {
    state: State<Option<UploadState>>,
    /// Files the limit refused, counted so a refusal is observable instead of
    /// looking like nothing happened.
    rejected: State<u32>,
    options: GalleryOptions,
}

impl DragUploadStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            state: State::new(None),
            rejected: State::new(0),
            options,
        }
    }

    /// The last finished upload, so a test can assert what the zone reported.
    pub fn last_state(&self) -> Option<UploadState> {
        self.state.get()
    }

    /// How many files the per-zone limit has refused.
    pub fn rejected_count(&self) -> u32 {
        self.rejected.get()
    }

    /// The extensions this page's zone accepts.
    pub fn accepted_extensions() -> Vec<String> {
        vec![
            "png".into(),
            "jpg".into(),
            "jpeg".into(),
            "webp".into(),
            "gif".into(),
        ]
    }

    /// What a dropped path becomes. The gallery reads real bytes when the path
    /// is readable and otherwise describes the file from its name alone, so a
    /// demo never fails because a sample file is absent.
    fn part(file: &DroppedFile) -> Result<UploadFilePart, String> {
        if file.is_directory {
            return Err(format!("{} is a folder, not a file", file.name));
        }
        let bytes = std::fs::read(&file.path).unwrap_or_default();
        Ok(UploadFilePart::new(
            "file",
            file.name.clone(),
            content_type_for(&file.name),
            bytes,
        ))
    }
}

impl View for DragUploadStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let options = self.options.clone();
        let transport: Arc<dyn UploadTransport> = Arc::new(SampleTransport {
            calls: AtomicU64::new(0),
        });
        UploadTransportHolder::set(Some(transport));

        let reported = self.state.clone();
        let request = UploadRequest::new("memory://gallery/upload", Vec::new())
            .field("file")
            .form_field("source", "gallery");
        let upload = DragUpload::new(request)
            .accepted_extensions(Self::accepted_extensions())
            .maximum_files(4)
            .is_enabled(options.is_enabled)
            .reader(Self::part)
            .on_finished(move |state| reported.set(Some(state.clone())))
            .on_rejected({
                let rejected = self.rejected.clone();
                move |files| rejected.set(rejected.get() + files.len() as u32)
            });

        let last = self
            .state
            .get()
            .map(|state| state.status_text().unwrap_or_default())
            .unwrap_or_else(|| "nothing yet".into());

        // A single-file zone with a tight filter: the same control, showing that
        // a rejection is a normal outcome rather than a failed upload.
        let rejecting_upload = {
            let request = UploadRequest::new("memory://gallery/rejected", Vec::new());
            DragUpload::new(request)
                .accepted_extensions(vec!["txt".to_owned()])
                .maximum_files(1)
                .reader(Self::part)
        };

        // No filter at all: every type is accepted, archives included. This is
        // the shape a caller uses when the server decides what it wants.
        let everything = {
            let request = UploadRequest::new("memory://gallery/any", Vec::new());
            DragUpload::new(request)
                .maximum_files(8)
                .reader(Self::part)
        };
        let archives = {
            let request = UploadRequest::new("memory://gallery/archive", Vec::new());
            DragUpload::new(request)
                .accepted_extensions(vec![
                    "zip".to_owned(),
                    "tar".to_owned(),
                    "gz".to_owned(),
                    "7z".to_owned(),
                ])
                .maximum_files(2)
                .reader(Self::part)
        };

        let filtered = Text::new(format!(
            "Accepted: {} · up to 4 files · limit {}",
            Self::accepted_extensions()
                .iter()
                .map(|extension| extension.to_ascii_uppercase())
                .collect::<Vec<_>>()
                .join(", "),
            format_bytes(cove_ui_compose::primitives::upload::DEFAULT_MAXIMUM_BYTES as u64)
        ))
        .semantic_font(SemanticFontRef::caption())
        .semantic_foreground(SemanticColorRef::on_surface_muted());
        let diagnostics = AnyView::new(
            Text::new(format!(
                "Last upload: {last} · Refused by limit: {}",
                self.rejected.get()
            ))
            .semantic_font(SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted()),
        );

        AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            18.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(StorySection::new(
                    "Drop to upload",
                    "Drop an image into the zone, or use Browse files when a pointer drag is not available. The zone rejects a type it does not accept and reports it before any bytes move.",
                    AnyView::new(BoxView::new(
                        FlexDirection::Column,
                        Align::Stretch,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        12.0,
                        AnyView::new(TupleView::new(vec![
                            AnyView::new(upload.debug_name("drag-upload")),
                            AnyView::new(filtered),
                            diagnostics,
                        ])),
                    )),
                )),
                AnyView::new(StorySection::new(
                    "Any type",
                    "An empty filter accepts every file, archives and unknown formats included. Type is derived from the name: an archive is sent as application/zip, its contents are never unpacked.",
                    AnyView::new(
                        BoxView::new(
                            FlexDirection::Column,
                            Align::Stretch,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            12.0,
                            AnyView::new(everything.debug_name("drag-upload-any")),
                        ),
                    ),
                )),
                AnyView::new(StorySection::new(
                    "Archives",
                    "A filter restricted to archive formats. The archive is uploaded as one file: unpacking is the receiving service's decision, not the drop zone's.",
                    AnyView::new(
                        BoxView::new(
                            FlexDirection::Column,
                            Align::Stretch,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            12.0,
                            AnyView::new(archives.debug_name("drag-upload-archive")),
                        ),
                    ),
                )),
                AnyView::new(StorySection::new(
                    "Narrow filter",
                    "The same control limited to one text file. A drop of any other type, and any folder, is refused before any bytes move, and the zone stays usable.",
                    AnyView::new(
                        BoxView::new(
                            FlexDirection::Column,
                            Align::Stretch,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            12.0,
                            AnyView::new(rejecting_upload.debug_name("drag-upload-rejected")),
                        ),
                    ),
                )),
            ])),
        ))
    }

    fn tag(&self) -> &'static str {
        "DragUploadStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; state)
    }
}
