//! Scene inspection: snapshots, temporary styles and read-only diagnostics.
mod diagnostics;
mod editor;
mod inspector;
pub use cove_agent::ui_tools as tools;

pub use diagnostics::{
    DiagnosticCompactNode, DiagnosticError, DiagnosticExtensionSource, DiagnosticGeometryIssue,
    DiagnosticLayoutSummary, DiagnosticMemorySnapshot, DiagnosticNode, DiagnosticOverflowIssue,
    DiagnosticOverview, DiagnosticPage, DiagnosticPresentation, DiagnosticSemanticNode, DiagnosticStateSource, Diagnostics,
    ExtensionRegistry, builtin_extension_registry,
    DiagnosticsStateQuery, MemoryPressure, TraceCapture, TraceComparison, TracePhase, TracePhaseChange, TraceQuery,
    UIDiagnosticCompactNode, UIDiagnosticGeometryIssue, UIDiagnosticLayoutSummary,
    UIDiagnosticMemorySnapshot, UIDiagnosticNode, UIDiagnosticOverflowIssue, UIDiagnosticOverview,
    UIDiagnosticPage, UIDiagnosticPresentation, UIDiagnosticTool, UIDiagnosticTools, UIDiagnostics,
    UIStateQuery, UITraceCapture, UITraceComparison, UITracePhase, UITracePhaseChange,
    UITraceQuery,
};
pub use editor::{
    DebugPaint, MAX_BASELINES, MAX_GUIDELINE_NODES, MAX_OVERSIZED_IMAGES, MAX_REPAINT_NODES,
    OVERSIZED_IMAGE_AREA_RATIO, REPAINT_PALETTE, SceneEditor, repaint_color,
};
pub(crate) use inspector::encode_frame;
pub use inspector::{DebugAttachmentKey, SceneInspector, TreePageError};
pub use tools::{Tool, Tools, default_skill_library};
