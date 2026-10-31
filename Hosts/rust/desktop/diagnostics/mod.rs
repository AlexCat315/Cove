//! Translation of Desktop/Sources/CoveUI/Diagnostics.

pub mod checkpoint;
pub mod frame_metrics;
pub mod performance_hud;
pub mod recording;
pub mod session;

pub use checkpoint::ApplicationCheckpoint;
pub use frame_metrics::FrameMetrics;
pub use integration::{DiagnosticMemorySnapshot, DiagnosticsSession};
pub use performance_hud::PerformanceHud;
pub use recording::{InputRecorder, InputRecording, RecordingControl};
pub use session::{ApplicationDiagnostics, DiagnosticsQueries};

pub mod integration;
