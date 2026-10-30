//! Virtualized scrolling ranges, row metrics and row stacks.
pub mod estimated_stack;
pub mod fixed_stack;
pub mod row_index;
pub mod row_metrics;
pub mod variable_stack;
pub mod virtual_stack;
pub mod virtual_stack_body;
pub mod virtual_window;

pub use estimated_stack::{EstimatedRowHost, EstimatedRowMetrics, EstimatedVirtualStack};
pub use fixed_stack::{total_extent, visible_range, FixedVirtualStack};
pub use row_index::VariableRowIndex;
pub use row_metrics::VariableRowMetrics;
pub use variable_stack::VariableVirtualStack;
pub use virtual_stack::{VirtualRows, VirtualStack};
pub use virtual_stack_body::{resolve_frame, VirtualFrame};
pub use virtual_window::{VirtualRowKey, VirtualWindow};
