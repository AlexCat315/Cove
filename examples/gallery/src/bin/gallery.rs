//! The gallery, instrumented for `query_ui_memory`.
//!
//! Installing [`cove_alloc::TrackingAllocator`] as the process allocator makes
//! the desktop host publish live, peak, total and released heap bytes and blocks
//! alongside the process resident/virtual counters.

#[global_allocator]
static ALLOC: cove_alloc::TrackingAllocator = cove_alloc::TrackingAllocator::new();

fn main() -> Result<(), String> {
    cove_examples::run()
}
