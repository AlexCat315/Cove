//! A drop area that uploads what it receives.
//!
//! [`DragUpload`] composes the three pieces a caller would otherwise wire by
//! hand: the drop zone, the upload state machine and a progress readout. The
//! upload runs on a worker thread; its state is a `State<UploadState>`, which a
//! background thread may write because `State` invalidates subscribed views
//! through the host's work scheduler.
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use cove_composition::{AnyView, Scope, View};
use cove_core::State;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};

use crate::modifiers::layout::{FlexExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::{Button, ButtonRole};
use crate::primitives::feedback::{ProgressView, StatusTone};
use crate::primitives::interaction::{DroppedFile, FileDropOutcome, FileDropPrompt, FileDropZone};
use crate::primitives::media::Icon;
use crate::primitives::text::Text;
use crate::primitives::upload::multipart::UploadFilePart;
use crate::primitives::upload::{
    UploadError, UploadRequest, UploadResponse, UploadState, UploadTransport, UploadTransportHolder,
};
use crate::theme::styles::buttons::{ButtonStyleViewExt, GhostButtonStyle};
use crate::theme::{
    ControlSize, ControlSizeViewExt, SemanticColorRef, SemanticFontRef, Theme, ThemeReader,
};

/// How a dropped file becomes a multipart part.
///
/// Reading the file is host work — a sandboxed URL is not always a plain path —
/// so the caller supplies the reader. An error fails that upload with the
/// message instead of sending a partial form.
pub type FileReader = Arc<dyn Fn(&DroppedFile) -> Result<UploadFilePart, String> + Send + Sync>;

/// A drop area that uploads what it receives.
pub struct DragUpload {
    request: UploadRequest,
    accepted_extensions: Option<HashSet<String>>,
    maximum_files: usize,
    is_enabled: bool,
    reader: Option<FileReader>,
    on_finished: Option<Rc<dyn Fn(&UploadState)>>,
    on_rejected: Option<Rc<dyn Fn(&[DroppedFile])>>,
    boundary: String,
    /// Owned here, not in `body`: a `State` created during composition is
    /// recreated on every pass and the upload would report progress into a
    /// value nobody reads.
    state: State<UploadState>,
    targeted: State<bool>,
    outcome: State<FileDropOutcome>,
}

impl DragUpload {
    pub fn new(request: UploadRequest) -> Self {
        Self {
            request,
            accepted_extensions: None,
            maximum_files: 8,
            is_enabled: true,
            reader: None,
            on_finished: None,
            on_rejected: None,
            // Fixed per instance so a retry frames the body identically.
            boundary: "cove-upload-boundary".into(),
            state: State::new(UploadState::Idle),
            targeted: State::new(false),
            outcome: State::new(FileDropOutcome::Idle),
        }
    }

    /// The live upload state, so a caller can observe or reset it.
    pub fn state(&self) -> &State<UploadState> {
        &self.state
    }

    pub fn accepted_extensions(mut self, extensions: impl IntoIterator<Item = String>) -> Self {
        self.accepted_extensions = Some(
            extensions
                .into_iter()
                .map(|extension| extension.to_ascii_lowercase())
                .collect(),
        );
        self
    }

    pub fn maximum_files(mut self, maximum_files: usize) -> Self {
        self.maximum_files = maximum_files.max(1);
        self
    }

    pub fn is_enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn reader(
        mut self,
        reader: impl Fn(&DroppedFile) -> Result<UploadFilePart, String> + Send + Sync + 'static,
    ) -> Self {
        self.reader = Some(Arc::new(reader));
        self
    }

    pub fn boundary(mut self, boundary: impl Into<String>) -> Self {
        self.boundary = boundary.into();
        self
    }

    /// Runs once per finished upload, on the UI thread.
    pub fn on_finished(mut self, on_finished: impl Fn(&UploadState) + 'static) -> Self {
        self.on_finished = Some(Rc::new(on_finished));
        self
    }

    /// Runs when a drop is refused by the type filter, the file limit or a
    /// folder, with the files that were refused.
    pub fn on_rejected(mut self, on_rejected: impl Fn(&[DroppedFile]) + 'static) -> Self {
        self.on_rejected = Some(Rc::new(on_rejected));
        self
    }

    fn extensions(&self) -> Vec<String> {
        self.accepted_extensions
            .as_ref()
            .map(|set| {
                let mut extensions: Vec<String> = set.iter().cloned().collect();
                extensions.sort();
                extensions
            })
            .unwrap_or_default()
    }
}

impl View for DragUpload {
    fn body(&self, _cx: &Scope) -> AnyView {
        let state = &self.state;
        let targeted = &self.targeted;
        let outcome = &self.outcome;
        let current = state.get();
        let busy = current.is_busy();
        let supported = UploadState::is_supported();
        let is_enabled = self.is_enabled && supported && !busy;
        // The zone stays live without a transport: a drop must report that the
        // platform cannot upload, not vanish because the control disabled
        // itself. Only a caller-disabled or busy control refuses the drop.
        let zone_enabled = self.is_enabled && !busy;

        let start = UploadStart {
            request: Arc::new(UploadFiles {
                request: self.request.clone(),
                boundary: self.boundary.clone(),
            }),
            reader: self.reader.clone(),
            on_finished: self.on_finished.clone(),
            state: state.clone(),
        };
        let drop_start = start.clone();
        let browse_start = start.clone();
        let zone_targeted = targeted.clone();
        let zone_outcome = outcome.clone();
        let hover = targeted.clone();
        let verdict = outcome.clone();
        let accepted = self.accepted_extensions.clone();
        let capacity = self.maximum_files;

        let prompt = FileDropPrompt::new(move |files| browse_start.start(files))
            .title("Drop files to upload")
            .accepted_extensions(self.extensions())
            .maximum_files(self.maximum_files)
            .is_enabled(is_enabled);

        let zone = FileDropZone::new(move |files| drop_start.start(files), {
            let mut content = vec![AnyView::new(prompt)];
            content.push(AnyView::new(FileRows {
                files: self.request.files.clone(),
            }));
            AnyView::new(cove_composition::TupleView::new(content))
        })
        .maximum_files(self.maximum_files)
        .is_enabled(zone_enabled)
        .is_targeted(zone_targeted.binding())
        .outcome(zone_outcome.binding())
        .on_targeted(move |is_targeted| {
            hover.set(is_targeted);
            if !is_targeted {
                verdict.set(FileDropOutcome::Idle);
            }
        })
        .on_rejected({
            let on_rejected = self.on_rejected.clone();
            move |files: &[DroppedFile]| {
                if let Some(on_rejected) = on_rejected.as_ref() {
                    on_rejected(files);
                }
            }
        })
        .accepts(move |files: &[DroppedFile]| {
            if !zone_enabled || files.is_empty() || files.len() > capacity {
                return false;
            }
            accepted
                .as_ref()
                .is_none_or(|extensions| files.iter().all(|file| file.matches_any(extensions)))
        });

        let zone = match &self.accepted_extensions {
            Some(extensions) => zone.accepted_extensions(extensions.iter().cloned()),
            None => zone,
        };

        let mut rows = vec![AnyView::new(zone)];
        if !matches!(&current, UploadState::Idle) {
            rows.push(AnyView::new(UploadProgress {
                state: state.clone(),
                is_enabled: self.is_enabled,
            }));
        }
        if let Some(error) = unsupported_note(supported, self.is_enabled) {
            rows.push(AnyView::new(
                Text::new(error)
                    .semantic_font(SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::error()),
            ));
        }
        AnyView::new(crate::BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            12.0,
            AnyView::new(cove_composition::TupleView::new(rows)),
        ))
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; state, targeted, outcome)
    }
}

fn unsupported_note(supported: bool, is_enabled: bool) -> Option<&'static str> {
    if supported || !is_enabled {
        return None;
    }
    Some("Uploads are not available on this platform.")
}

/// The immutable part of the request, shared with the worker thread.
struct UploadFiles {
    request: UploadRequest,
    boundary: String,
}

/// Everything needed to start one upload from either entry point.
#[derive(Clone)]
struct UploadStart {
    request: Arc<UploadFiles>,
    reader: Option<FileReader>,
    on_finished: Option<Rc<dyn Fn(&UploadState)>>,
    state: State<UploadState>,
}

impl UploadStart {
    /// Reads the dropped files, then encodes and sends them.
    ///
    /// Reading happens on the worker thread too, so a large file never blocks
    /// the UI thread; the state moves to `Preparing` immediately so the control
    /// shows work in progress. The outcome is applied on the UI thread by
    /// `spawn_then`, which posts to this thread's inbox.
    fn start(&self, files: Vec<DroppedFile>) {
        if files.is_empty() || self.state.get().is_busy() {
            return;
        }
        let Some(transport) = UploadTransportHolder::current() else {
            self.fail(UploadError::HostUnavailable);
            return;
        };
        let Some(reader) = self.reader.clone() else {
            self.fail(UploadError::Network("no file reader is configured".into()));
            return;
        };
        self.state.set(UploadState::Preparing);
        let request = self.request.clone();
        let state = self.state.clone();
        let progress = self.state.clone();
        let on_finished = self.on_finished.clone();
        cove_core::work::spawn_then(
            move || upload_blocking(&request, &reader, &files, transport.as_ref(), &progress),
            move |result| {
                state.set(match result {
                    Ok(response) => UploadState::Succeeded(response),
                    Err(error) => UploadState::Failed(error),
                });
                if let Some(on_finished) = on_finished.as_ref() {
                    on_finished(&state.get());
                }
            },
        );
    }

    fn fail(&self, error: UploadError) {
        self.state.set(UploadState::Failed(error));
        self.finish();
    }

    fn finish(&self) {
        if let Some(on_finished) = self.on_finished.as_ref() {
            on_finished(&self.state.get());
        }
    }
}

/// Read, encode and send, entirely off the UI thread.
fn upload_blocking(
    request: &UploadFiles,
    reader: &FileReader,
    files: &[DroppedFile],
    transport: &dyn UploadTransport,
    progress: &State<UploadState>,
) -> Result<UploadResponse, UploadError> {
    let read = |file: &DroppedFile| reader(file).map_err(UploadError::Network);
    let parts = files
        .iter()
        .map(read)
        .collect::<Result<Vec<UploadFilePart>, UploadError>>()?;
    let mut upload = request.request.clone();
    upload.files = parts;
    if upload.is_oversized() {
        return Err(UploadError::PayloadTooLarge {
            limit: upload.maximum_bytes,
        });
    }
    upload.encode(&request.boundary)?;
    let total = upload.payload_bytes();
    let report = |sent: u64| {
        progress.set(UploadState::Uploading { sent, total });
    };
    progress.set(UploadState::Uploading { sent: 0, total });
    let response = transport.send(&upload, &report)?;
    if !response.is_success() {
        return Err(UploadError::HttpStatus {
            status: response.status,
        });
    }
    Ok(response)
}

/// The uploaded rows: what this request carries before a drop.
pub struct FileRows {
    pub files: Vec<UploadFilePart>,
}

impl View for FileRows {
    fn body(&self, _cx: &Scope) -> AnyView {
        let rows: Vec<AnyView> = self
            .files
            .iter()
            .map(|file| {
                AnyView::new(Text::new(format!(
                    "{} · {}",
                    file.file_name,
                    crate::primitives::interaction::file_drop::format_bytes(file.bytes.len() as u64)
                )))
            })
            .collect();
        AnyView::new(cove_composition::TupleView::new(rows))
    }
}

/// The live upload readout: progress and cancel while running, a tinted status
/// row once the upload reaches a terminal state.
struct UploadProgress {
    state: State<UploadState>,
    is_enabled: bool,
}

impl View for UploadProgress {
    fn body(&self, _cx: &Scope) -> AnyView {
        let current = self.state.get();
        let state = self.state.clone();
        let is_enabled = self.is_enabled;
        AnyView::new(ThemeReader::new(move |theme: &Theme| match &current {
            UploadState::Idle => empty_view(),
            UploadState::Succeeded(_) | UploadState::Failed(_) => {
                let tone = match &current {
                    UploadState::Succeeded(_) => StatusTone::Success,
                    _ => StatusTone::Danger,
                };
                let tint = tone.color().resolve(theme);
                let message = current.status_text().unwrap_or_default();
                let row = crate::BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    8.0,
                    AnyView::new(cove_composition::TupleView::new(vec![
                        AnyView::new(Icon::with_color(tone.icon(), 16.0, tint)),
                        AnyView::new(
                            Text::new(message)
                                .line_limit(Some(2))
                                .semantic_font(SemanticFontRef::body())
                                .semantic_foreground(tone.color())
                                .flex(1.0),
                        ),
                    ])),
                )
                .padding_edges(12.0, 10.0)
                .background_color(tint.multiplied_alpha(0.10))
                .corner_radius(theme.radius.md);
                AnyView::new(row)
            }
            UploadState::Preparing | UploadState::Uploading { .. } => {
                let message = match &current {
                    UploadState::Uploading { sent, total } if *total > 0 => {
                        format!("{:.0}%", (*sent as f64 / *total as f64) * 100.0)
                    }
                    UploadState::Preparing => "Preparing…".into(),
                    _ => "Uploading…".into(),
                };
                let state = state.clone();
                let cancel = Button::with_options(
                    Text::new("Cancel"),
                    move || {
                        if state.get().is_busy() {
                            state.set(UploadState::Failed(UploadError::Cancelled));
                        }
                    },
                    ButtonRole::Normal,
                    is_enabled,
                    false,
                    false,
                    None,
                )
                .accessibility_text("Cancel upload")
                .button_style(GhostButtonStyle);
                let cancel = AnyView::new(cancel.control_size(ControlSize::Small));
                let bar = AnyView::new(
                    ProgressView::new(current.progress())
                        .tone(StatusTone::Info)
                        .height(4.0)
                        .flex(1.0),
                );
                let row = crate::BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    12.0,
                    AnyView::new(cove_composition::TupleView::new(vec![
                        bar,
                        AnyView::new(
                            Text::new(message)
                                .alignment(cove_core::TextAlignment::Trailing)
                                .semantic_font(SemanticFontRef::label())
                                .semantic_foreground(SemanticColorRef::on_surface_variant()),
                        ),
                        cancel,
                    ])),
                )
                .padding_edges(12.0, 10.0)
                .background_color(theme.colors.surface_variant.multiplied_alpha(0.55))
                .corner_radius(theme.radius.md);
                AnyView::new(row)
            }
        }))
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; state)
    }
}

fn empty_view() -> AnyView {
    AnyView::new(cove_composition::TupleView::new(Vec::new()))
}
