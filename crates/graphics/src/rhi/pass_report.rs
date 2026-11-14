//! What one recorded frame actually asks the GPU to do, pass by pass.
//!
//! `CommandBuffer` is the record a frame builds before it is planned and
//! submitted, so this census is the authored view of the frame: which passes
//! exist, how many draws each carries, and the index ranges they reach. A
//! planner may inject barriers between these passes; barriers are not draws and
//! are deliberately absent here.
//!
//! The ranges matter for attribution: a compositing layer owns a contiguous
//! index range in the frame draw list, so a draw belongs to the layer whose
//! range contains it.

use super::commands::{CommandBuffer, RecordedCommand, RenderCommand};

/// Kind of work a pass performs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassKind {
    Render,
    Compute,
    Copy,
    AccelerationStructure,
}

impl PassKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Render => "render",
            Self::Compute => "compute",
            Self::Copy => "copy",
            Self::AccelerationStructure => "accelerationStructure",
        }
    }
}

/// An index window one draw covers, in the frame's index buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PassIndexRange {
    pub first: u64,
    pub count: u64,
}

impl PassIndexRange {
    pub fn new(first: u64, count: u64) -> Self {
        Self { first, count }
    }

    /// True when this range shares any index with `other`.
    pub fn intersects(self, other: Self) -> bool {
        self.first < other.first.saturating_add(other.count)
            && other.first < self.first.saturating_add(self.count)
    }
}

/// One pass in the recorded frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PassReport {
    /// The authored descriptor label, or `"<unnamed>"` when the pass named none.
    pub label: String,
    pub kind: PassKind,
    pub draws: u64,
    pub instances: u64,
    /// Vertices reached by non-indexed draws.
    pub vertices: u64,
    /// Indices reached by indexed draws, summed over draws.
    pub indices: u64,
    /// Index windows an indexed draw covers, in draw order. Bounded by the
    /// frame's own draw count; empty for non-indexed passes.
    pub ranges: Vec<PassIndexRange>,
}

impl PassReport {
    fn new(label: String, kind: PassKind) -> Self {
        Self {
            label,
            kind,
            draws: 0,
            instances: 0,
            vertices: 0,
            indices: 0,
            ranges: Vec::new(),
        }
    }
}

impl CommandBuffer {
    /// Pass-by-pass census of what this frame records.
    ///
    /// Passes appear in recording order, which is the order the renderer authored
    /// them in; the planner's injected barriers do not add rows.
    pub fn pass_reports(&self) -> Vec<PassReport> {
        self.pass_report_iter().collect()
    }

    pub(crate) fn pass_report_iter(&self) -> impl Iterator<Item = PassReport> + '_ {
        self.commands().iter().filter_map(|command| match command {
            RecordedCommand::RenderPass(record) => {
                let label = record
                    .descriptor
                    .label
                    .filter(|label| !label.is_empty())
                    .unwrap_or("<unnamed>")
                    .to_owned();
                let mut report = PassReport::new(label, PassKind::Render);
                for command in &record.body {
                    match command {
                        RenderCommand::Draw {
                            vertex_count,
                            instance_count,
                            ..
                        } => {
                            report.draws += 1;
                            report.instances += (*instance_count).max(0) as u64;
                            report.vertices += (*vertex_count).max(0) as u64;
                        }
                        RenderCommand::DrawIndexed(arguments) => {
                            report.draws += 1;
                            report.instances += arguments.instance_count.max(0) as u64;
                            report.indices += arguments.index_count.max(0) as u64;
                            if arguments.index_count > 0 {
                                report.ranges.push(PassIndexRange::new(
                                    arguments.first_index.max(0) as u64,
                                    arguments.index_count as u64,
                                ));
                            }
                        }
                        RenderCommand::DrawIndirect { draw_count, .. } => {
                            report.draws += 1;
                            report.instances += (*draw_count).max(0) as u64;
                        }
                        RenderCommand::DrawMeshTasks(_) => {
                            report.draws += 1;
                        }
                        _ => {}
                    }
                }
                Some(report)
            }
            RecordedCommand::ComputePass(record) => {
                let mut report = PassReport::new("computePass".to_owned(), PassKind::Compute);
                report.draws = record.body.len() as u64;
                Some(report)
            }
            RecordedCommand::CopyPass(record) => {
                let mut report = PassReport::new("copyPass".to_owned(), PassKind::Copy);
                report.draws = record.body.len() as u64;
                Some(report)
            }
            RecordedCommand::AccelerationStructureBuild(_) => {
                let mut report = PassReport::new(
                    "accelerationStructure".to_owned(),
                    PassKind::AccelerationStructure,
                );
                report.draws = 1;
                Some(report)
            }
            _ => None,
        })
    }
}
