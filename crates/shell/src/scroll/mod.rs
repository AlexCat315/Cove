//! AppKit scroll phases are retained by the native ordered SDL marker bridge.
///
/// Maps the phase value the AppKit monitor stores (`apple.m::phase_value`) onto
/// the platform enum. Public so the decoding contract can be asserted without a
/// live window: the marker never reaches a test otherwise.
pub fn phase(value: i32) -> cove_platform::ScrollPhase {
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
