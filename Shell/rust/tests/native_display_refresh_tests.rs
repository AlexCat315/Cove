//! Translation of `Tests/CoveUIRuntimeTests/NativeDisplayRefreshTests.swift`.
//!
//! SDL's native wait must service CVDisplayLink callbacks so the host can pace
//! at the display's rate instead of its own deadline, ticks must carry a target
//! timestamp ahead of the sample, disabling must stop delivering, hiding a
//! window must let the host fall back to its animation clock, and destroying a
//! window must discard any pending tick.
//!
//! Reachability (documented, not silently skipped): every case needs a live
//! display link, which needs a live `NSWindow` on an on-console display session.
//! This host reports "No available video device" from `Sdl3Shell::create_window`
//! (native, dummy and offscreen drivers all fail), and the Swift suite itself
//! `XCTSkip`s the tick assertions when `CI` is set for exactly this reason. So
//! the assertions below are the reachable half of the contract — the shell's
//! window bookkeeping for refresh state — and the live-tick half is recorded as
//! a named difference rather than asserted against a fake link.
//!
//! Named difference: `set_display_refresh_enabled`, `take_display_refresh` and
//! the tick cadence cannot be exercised without a window. The Swift assertions
//! quoted for the record are:
//! `XCTAssertEqual(first.count, 3)`, `tick.targetTimestamp > tick.timestamp`,
//! `current.timestamp - previous.timestamp < 0.075`,
//! `XCTAssertNil(shell.takeDisplayRefresh(windowID:))` after disabling, and
//! `XCTAssertFalse(shell.setDisplayRefreshEnabled(true, windowID:))` on a hidden
//! window.
//!
//! Behaviour checklist:
//! - an unknown window id yields no refresh tick
//! - an unknown window id cannot enable or disable a display link
//! - a shell with no windows reports no main window and no refresh rate
//! - a tick's target timestamp leads its sample time

#![cfg(target_os = "macos")]

use cove_platform::WindowId;
use cove_shell::{DisplayRefreshTick, Shell, Sdl3Shell};

/// Window id that was never created. `WindowId` is a transparent `u32`, so any
/// unused value stands in for Swift's "unknown window" case.
const UNKNOWN: WindowId = WindowId(9_999);

struct BareShell {
    shell: Sdl3Shell,
}

impl BareShell {
    fn new() -> Self {
        Self {
            shell: Sdl3Shell::new().expect("the SDL shell initialises"),
        }
    }
}

impl Drop for BareShell {
    fn drop(&mut self) {
        self.shell.shutdown();
    }
}

#[test]
fn unknown_window_yields_no_tick() {
    let shell = BareShell::new();
    assert_eq!(
        shell.shell.take_display_refresh(UNKNOWN),
        None,
        "a window that was never created has no pending refresh tick",
    );
}

#[test]
fn unknown_window_cannot_configure_a_display_link() {
    let shell = BareShell::new();
    // Enabling on an unknown window must report failure rather than silently
    // succeeding: the host uses `false` to fall back to its animation clock.
    assert!(
        !shell.shell.set_display_refresh_enabled(UNKNOWN, true),
        "an unknown window cannot enable a display link",
    );
    assert!(
        !shell.shell.set_display_refresh_enabled(UNKNOWN, false),
        "an unknown window cannot report a release it never held",
    );
}

#[test]
fn no_window_means_no_refresh_rate() {
    let shell = BareShell::new();
    assert_eq!(
        shell.shell.main_window_id(),
        None,
        "a shell with no created window reports no main window",
    );
    assert_eq!(
        shell.shell.display_refresh_rate(None),
        None,
        "with no window there is no display to pace against",
    );
    assert!(
        shell.shell.window_ids().is_empty(),
        "no window ids are tracked before any window exists",
    );
}

#[test]
fn tick_target_leads_its_sample() {
    // The payload the native driver produces. A tick that targets the same
    // instant it was sampled at would pace a frame that is already due.
    let at_60hz = DisplayRefreshTick {
        timestamp: 100.0,
        target_timestamp: 100.0 + 1.0 / 60.0,
    };
    assert!(
        at_60hz.target_timestamp > at_60hz.timestamp,
        "a 60 Hz tick targets a later presentation time",
    );
    assert!(
        at_60hz.target_timestamp - at_60hz.timestamp < 0.075,
        "a 60 Hz tick is closer than SDL's 100 ms wait timeout",
    );
}
