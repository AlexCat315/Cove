//! Window-level presentation a widget may ask the host for: full screen and
//! floating (always-on-top) windows such as picture-in-picture.
//!
//! Hosts install a [`WindowPresenter`]; without one every request reports that
//! it was not honoured, and widgets fall back to an in-window presentation.
use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::AnyView;

/// A small window kept above other windows.
#[derive(Clone)]
pub struct FloatingWindowRequest {
    pub title: String,
    pub width: f32,
    pub height: f32,
    pub content: AnyView,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FloatingWindowId(pub u64);

pub trait WindowPresenter {
    /// Returns whether the request was honoured.
    fn set_fullscreen(&self, fullscreen: bool) -> bool;
    fn is_fullscreen(&self) -> bool;
    fn open_floating(&self, request: FloatingWindowRequest) -> Option<FloatingWindowId>;
    fn close_floating(&self, id: FloatingWindowId);
    /// `false` once the window was closed, including by the user.
    fn is_floating_open(&self, id: FloatingWindowId) -> bool;
}

thread_local! {
    static PRESENTER: RefCell<Option<Rc<dyn WindowPresenter>>> = const { RefCell::new(None) };
}

/// The host's [`WindowPresenter`] for the UI thread.
pub struct WindowPresentation;

impl WindowPresentation {
    pub fn current() -> Option<Rc<dyn WindowPresenter>> {
        PRESENTER.with(|slot| slot.borrow().clone())
    }

    pub fn set_current(presenter: Option<Rc<dyn WindowPresenter>>) {
        PRESENTER.with(|slot| *slot.borrow_mut() = presenter);
    }

    pub fn is_installed() -> bool {
        Self::current().is_some()
    }

    pub fn set_fullscreen(fullscreen: bool) -> bool {
        Self::current().is_some_and(|presenter| presenter.set_fullscreen(fullscreen))
    }

    pub fn is_fullscreen() -> bool {
        Self::current().is_some_and(|presenter| presenter.is_fullscreen())
    }

    pub fn open_floating(request: FloatingWindowRequest) -> Option<FloatingWindowId> {
        Self::current().and_then(|presenter| presenter.open_floating(request))
    }

    pub fn close_floating(id: FloatingWindowId) {
        if let Some(presenter) = Self::current() {
            presenter.close_floating(id);
        }
    }

    pub fn is_floating_open(id: FloatingWindowId) -> bool {
        Self::current().is_some_and(|presenter| presenter.is_floating_open(id))
    }
}
