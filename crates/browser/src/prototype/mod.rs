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

#![allow(clippy::module_inception)]

pub mod abi;
pub mod counter;
pub mod frame;
pub mod prototype;
pub mod root_content;

pub use frame::{Batch, Frame, Label, viewport_size};
pub use prototype::BrowserPrototype;
pub use root_content::RootContent;

use std::cell::RefCell;

/// The concrete reactor type this host exports through the C ABI. Its root is
/// the application registered by `cove_ui::application!`, or the counter when no
/// application module has been initialized.
pub type Content = BrowserPrototype<RootContent>;

/// The layout debug name recorded by `.debug_name(_)`, read back from a node.
///
/// The DevTools protocol identifies nodes by this name for `select.node` and
/// recording replay, so the host must resolve it the same way the inspector
/// does: through the layout debug attachment.
pub fn node_debug_name(node: &cove_core::scene::Node) -> Option<String> {
    node.attachment::<String>(cove_composition::LayoutDebugAttachmentKey::DEBUG_NAME)
}

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
        body(slot.get_or_insert_with(|| Content::new(RootContent::from_registration())))
    })
}

/// Drops the reactor, releasing every buffer handed to JavaScript.
pub fn reset_app() {
    APP.with(|slot| {
        *slot.borrow_mut() = None;
    });
}
