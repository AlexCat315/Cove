//! Persistent UTF-8 rope used by buffers, editors, and rapid diagnostics.

pub mod rope_difference;
pub mod rope_editing;
pub mod rope_lookup;
pub mod rope_storage;

pub use rope_difference::common_bytes;
pub use rope_editing::{concatenate, join, replace_range, split};
pub use rope_storage::{
    RopeBranch, RopeChunk, RopeHandle, RopeLeaf, RopeMetrics, RopeNode, RopeStorage, empty_node,
    is_newline,
};
