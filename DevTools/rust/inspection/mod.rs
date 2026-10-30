//! Scene inspection: snapshots, temporary styles and read-only diagnostics.
mod actions;
mod diagnostics;
mod editor;
mod inspector;
pub mod tools;

pub use actions::{UIAccessibilityActions, perform_accessibility_action};
pub use diagnostics::{
    DiagnosticCompactNode, DiagnosticError, DiagnosticGeometryIssue, DiagnosticLayoutSummary,
    DiagnosticMemorySnapshot, DiagnosticNode, DiagnosticOverflowIssue, DiagnosticOverview, DiagnosticPage,
    DiagnosticPresentation, Diagnostics, DiagnosticsStateQuery, TraceCapture, TraceComparison, TracePhase,
    TracePhaseChange, TraceQuery, UIDiagnosticCompactNode, UIDiagnosticGeometryIssue, UIDiagnosticLayoutSummary,
    UIDiagnosticMemorySnapshot, UIDiagnosticNode, UIDiagnosticOverview, UIDiagnosticPage, UIDiagnosticPresentation,
    UIDiagnosticTool, UIDiagnosticTools, UIDiagnosticOverflowIssue, UIDiagnostics, UIStateQuery, UITraceCapture,
    UITraceComparison, UITracePhase, UITracePhaseChange, UITraceQuery,
};
pub use editor::SceneEditor;
pub use inspector::{DebugAttachmentKey, SceneInspector};
pub use tools::{default_skill_library, Tool, Tools};
