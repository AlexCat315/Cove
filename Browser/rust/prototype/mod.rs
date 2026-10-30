//! Translation of `Browser/Sources/CoveUIBrowserPrototype/main.swift`.
//!
//! One single-threaded reactor. JavaScript calls the exported C ABI functions
//! on the browser main thread; every buffer handed out stays valid until the
//! next call that replaces it. The Swift original keeps the same invariant, so
//! no locking is introduced here: a `thread_local!` cell replaces the Swift
//! `nonisolated(unsafe) private let`.
//!
//! Layout of this module:
//! - [`frame`] — the JSON frame snapshot shared with JavaScript.
//! - [`abi`] — the `#[unsafe(no_mangle)]` C exports and allocation helpers.
//! - [`prototype`] — the reactor that owns scene, text, draw list and DevTools.

pub mod abi;
pub mod counter;
pub mod frame;
pub mod prototype;

pub use frame::{viewport_size, Batch, Frame, Label};
pub use prototype::BrowserPrototype;

use std::cell::RefCell;

/// The concrete reactor type this host exports through the C ABI.
pub type Content = BrowserPrototype<cove_examples::CounterView>;

thread_local! {
    /// The process has exactly one reactor, mirroring the Swift global `app`.
    static APP: RefCell<Option<Content>> = const { RefCell::new(None) };
}

/// Runs `body` with the reactor, creating it on first use.
///
/// # Panics
/// Panics if called re-entrantly: the reactor borrows its own state while a
/// frame is being produced, and JavaScript must not re-enter during a call.
pub fn with_app<R>(body: impl FnOnce(&mut Content) -> R) -> R {
    APP.with(|slot| {
        let mut slot = slot.try_borrow_mut().expect("browser prototype re-entered");
        body(slot.get_or_insert_with(|| Content::new(cove_examples::CounterView::new())))
    })
}

/// Drops the reactor, releasing every buffer handed to JavaScript.
pub fn reset_app() {
    APP.with(|slot| {
        *slot.borrow_mut() = None;
    });
}
