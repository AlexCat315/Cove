//! Translation of NativeRHI/Metal: the concrete Metal backend.
//!
//! The module mirrors the Swift `Metal/` directory one file per module. Native
//! objects are reached through the `objc2-metal` bindings; every backend
//! failure is returned as an `RhiError` instead of being silently truncated,
//! and nothing here may be compiled on a target without Metal.
//!
//! Translated: `MetalFormats.swift` (`formats`), `MetalSubmissionStatus.swift`
//! (`submission_status`), `MetalLibraryCache.swift` (`library_cache`),
//! `MetalRenderBindings.swift` (`render_bindings`) and `MetalUploadRing.swift`
//! (`upload_ring`). The device, registries, acceleration structures, pipelines,
//! transfer and submit encoder follow in the same module and are added as they
//! are translated, so no placeholder module is declared here.
pub mod formats;

mod library_cache;
mod render_bindings;
mod submission_status;
mod upload_ring;

pub(crate) use library_cache::MetalLibraryCache;
pub(crate) use render_bindings::MetalRenderBindings;
pub(crate) use submission_status::MetalSubmissionStatus;
pub(crate) use upload_ring::{MetalFrameUploader, MetalUploadChunk};
