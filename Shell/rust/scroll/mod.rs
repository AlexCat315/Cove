//! AppKit scroll phases are retained by the native ordered SDL marker bridge.
pub(crate) fn phase(value: i32) -> cove_platform::ScrollPhase {
    use cove_platform::ScrollPhase::*;
    match value {
        1 => MayBegin,
        2 => Began,
        3 => Changed,
        4 => Stationary,
        5 => Ended,
        6 => Cancelled,
        _ => None,
    }
}
