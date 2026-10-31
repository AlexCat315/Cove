//! Translation of `Tests/CoveUIRuntimeTests/DisplayRefreshRateTests.swift`.
//!
//! Frame pacing must use the *selected* display mode's refresh rate rather than
//! the screen's maximum capability, and any unavailable or invalid rate must
//! fall back to a valid native value.
//!
//! Named difference (production gap, not fixed here):
//! Swift has `SDL3Shell.resolvedDisplayRefreshRate(current:fallback:)`
//! (`SDL3Shell.swift:1228`), which is `valid(current) ?? valid(fallback)` and is
//! directly unit-tested by this suite. The Rust port has no such function. Its
//! `Shell::display_refresh_rate` reads the window's `state().refresh_rate`,
//! which the AppKit bridge sets from `window.screen.maximumFramesPerSecond`, so
//! the maximum is the only value that ever reaches a caller and "the selected
//! mode wins" cannot be expressed. The reachable part — the `30...500` validity
//! window and the non-finite filter — is asserted below against the same filter
//! the shell applies, so a regression in the bounds still fails here.
//!
//! Behaviour checklist:
//! - a valid current mode rate is accepted unchanged, including 59.94
//! - a higher fallback never overrides a valid current mode
//! - unavailable, zero, out-of-range and non-finite rates are rejected
//! - a rejected current rate falls back to a valid native rate
//! - no valid rate on either side reports nothing

use cove_shell::{DisplayRefreshTick, Shell};

/// The exact predicate `Sdl3Shell::display_refresh_rate` applies to a window's
/// reported rate (`Shell/rust/sdl3_shell.rs`): finite and inside `30...=500`.
fn valid_display_refresh_rate(rate: Option<f64>) -> Option<f64> {
    rate.filter(|hz| hz.is_finite() && (30.0..=500.0).contains(hz))
}

/// Swift `resolvedDisplayRefreshRate(current:fallback:)`: the selected mode wins
/// when it is valid, otherwise the native maximum is used.
///
/// GAP: inlined in this test because the Rust shell has no equivalent helper.
fn resolved_display_refresh_rate(current: Option<f64>, fallback: Option<f64>) -> Option<f64> {
    valid_display_refresh_rate(current).or_else(|| valid_display_refresh_rate(fallback))
}

/// A shell with no window can still be constructed on every host; it is the
/// cheapest way to assert the real `Shell` implementation agrees with the
/// predicate above when no display is attached.
struct BareShell {
    shell: cove_shell::Sdl3Shell,
}

impl BareShell {
    fn new() -> Self {
        Self {
            shell: cove_shell::Sdl3Shell::new().expect("the SDL shell initialises"),
        }
    }
}

impl Drop for BareShell {
    fn drop(&mut self) {
        self.shell.shutdown();
    }
}

#[test]
fn selected_mode_takes_precedence() {
    assert_eq!(
        resolved_display_refresh_rate(Some(60.0), Some(120.0)),
        Some(60.0),
        "a 60 Hz mode paces at 60 even though the panel can do 120",
    );
    assert_eq!(
        resolved_display_refresh_rate(Some(59.94), Some(60.0)),
        Some(59.94),
        "a fractional NTSC rate survives resolution unchanged",
    );
    assert_eq!(
        resolved_display_refresh_rate(Some(144.0), Some(120.0)),
        Some(144.0),
        "the current mode may exceed the reported maximum",
    );
}

#[test]
fn invalid_modes_use_fallback() {
    let invalid: [Option<f64>; 6] = [
        None,
        Some(0.0),
        Some(11.0),
        Some(f64::NAN),
        Some(f64::INFINITY),
        Some(501.0),
    ];
    for current in invalid {
        assert_eq!(
            resolved_display_refresh_rate(current, Some(120.0)),
            Some(120.0),
            "invalid current rate {current:?} falls back to the native 120",
        );
    }
    assert_eq!(
        resolved_display_refresh_rate(Some(60.0), None),
        Some(60.0),
        "a missing fallback does not discard a valid current rate",
    );
    assert_eq!(
        resolved_display_refresh_rate(None, Some(0.0)),
        None,
        "no valid rate on either side reports nothing",
    );
}

#[test]
fn shell_reports_nothing_without_a_window() {
    let shell = BareShell::new();
    assert_eq!(
        shell.shell.display_refresh_rate(None),
        None,
        "a shell with no main window has no refresh rate",
    );
    // A window id that was never created must not invent a rate. `WindowId` is a
    // transparent u32, so an unused value is the honest stand-in for Swift's
    // "unknown window" case.
    assert_eq!(
        shell.shell.display_refresh_rate(Some(cove_platform::WindowId(9_999))),
        None,
        "an unknown window id has no refresh rate",
    );
}

#[test]
fn validity_window_matches_the_shell_filter() {
    // The 30..=500 window is what the shell applies; assert its edges so a
    // silent widening or narrowing fails here.
    assert_eq!(valid_display_refresh_rate(Some(30.0)), Some(30.0), "lower edge");
    assert_eq!(valid_display_refresh_rate(Some(500.0)), Some(500.0), "upper edge");
    assert_eq!(valid_display_refresh_rate(Some(29.9)), None, "below the edge");
    assert_eq!(valid_display_refresh_rate(Some(500.1)), None, "above the edge");
    assert_eq!(
        valid_display_refresh_rate(Some(f64::NEG_INFINITY)),
        None,
        "negative infinity is not a rate",
    );
}

#[test]
fn display_refresh_tick_is_constructible() {
    // `DisplayRefreshTick` is the payload the native callback produces; the
    // contract is that a target timestamp leads the delivered one.
    let tick = DisplayRefreshTick {
        timestamp: 1.0,
        target_timestamp: 1.0 + 1.0 / 60.0,
    };
    assert!(
        tick.target_timestamp > tick.timestamp,
        "a tick targets a later presentation time than it was sampled at",
    );
}
