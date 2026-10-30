//! Translation of Desktop/Sources/CoveUI/Application/Services.swift.
//!
//! Captures host-scoped services once and restores them on lifecycle teardown,
//! so a window that installed process-wide ambients cannot leak them into the
//! next run.

use crate::desktop::application::composition::{
    AppDisplayHandleHolder, AssetDropRegistry, AssetDropRegistryHolder,
};
use cove_core::scene::{ClipboardHolder, FocusChain, FocusChainHolder, InteractionRegistry, InteractionRegistryHolder, PointerCapture, PointerCaptureHolder, UIWorkSchedulerHolder};
use cove_imaging::{ImageAssetRegistry, ImageAssetRegistryHolder};
use cove_text::{TextEnvironment, TextEnvironmentHolder};
use std::rc::Rc;

pub struct ApplicationServices {
    text: Option<TextEnvironment>,
    scale: f32,
    interactions: Option<Rc<InteractionRegistry>>,
    focus: Option<Rc<FocusChain>>,
    capture: Option<Rc<PointerCapture>>,
    clipboard_read: Option<Rc<dyn Fn() -> Option<String>>>,
    clipboard_write: Option<Rc<dyn Fn(&str)>>,
    images: Option<Rc<ImageAssetRegistry>>,
    drops: Option<Rc<AssetDropRegistry>>,
    work: Option<Rc<dyn Fn(Rc<dyn Fn()>)>>,
    display: Option<crate::desktop::display::AppDisplayHandle>,
}

impl ApplicationServices {
    pub fn capture() -> Self {
        Self {
            text: TextEnvironmentHolder::current(),
            scale: cove_core::content_scale(),
            interactions: InteractionRegistryHolder::current(),
            focus: FocusChainHolder::current(),
            capture: PointerCaptureHolder::current(),
            clipboard_read: clipboard_handlers::read(),
            clipboard_write: clipboard_handlers::write(),
            images: ImageAssetRegistryHolder::current(),
            drops: AssetDropRegistryHolder::current(),
            work: work_scheduler::handler(),
            display: AppDisplayHandleHolder::current(),
        }
    }

    /// Restores every captured service, in the reverse order of capture.
    pub fn restore(self) {
        TextEnvironmentHolder::set_current(self.text);
        cove_core::set_content_scale(self.scale);
        InteractionRegistryHolder::set_current(self.interactions);
        FocusChainHolder::set_current(self.focus);
        PointerCaptureHolder::set_current(self.capture);
        ClipboardHolder::set_read(self.clipboard_read.clone());
        ClipboardHolder::set_write(self.clipboard_write.clone());
        clipboard_handlers::remember(self.clipboard_read, self.clipboard_write);
        ImageAssetRegistryHolder::set_current(self.images);
        AssetDropRegistryHolder::set_current(self.drops);
        work_scheduler::remember(self.work.clone());
        UIWorkSchedulerHolder::set_enqueue(self.work);
        AppDisplayHandleHolder::set_current(self.display);
    }
}

/// Clipboard handler registry.
///
/// `ClipboardHolder` exposes set/read/write but no getter, so the host records
/// the handlers it installs and restores exactly those on teardown.
pub mod clipboard_handlers {
    use super::*;
    use std::cell::RefCell;

    thread_local! {
        static SLOT: RefCell<(
            Option<Rc<dyn Fn() -> Option<String>>>,
            Option<Rc<dyn Fn(&str)>>,
        )> = const { RefCell::new((None, None)) };
    }

    pub fn remember(
        read: Option<Rc<dyn Fn() -> Option<String>>>,
        write: Option<Rc<dyn Fn(&str)>>,
    ) {
        SLOT.with(|slot| *slot.borrow_mut() = (read, write));
    }

    pub fn read() -> Option<Rc<dyn Fn() -> Option<String>>> {
        SLOT.with(|slot| slot.borrow().0.clone())
    }

    pub fn write() -> Option<Rc<dyn Fn(&str)>> {
        SLOT.with(|slot| slot.borrow().1.clone())
    }
}

/// UI work scheduler registry, mirroring [`clipboard_handlers`] for the same
/// reason: `UIWorkSchedulerHolder` has no getter.
pub mod work_scheduler {
    use super::*;
    use std::cell::RefCell;

    thread_local! {
        static SLOT: RefCell<Option<Rc<dyn Fn(Rc<dyn Fn()>)>>> = const { RefCell::new(None) };
    }

    pub fn remember(enqueue: Option<Rc<dyn Fn(Rc<dyn Fn()>)>>) {
        SLOT.with(|slot| *slot.borrow_mut() = enqueue);
    }

    pub fn handler() -> Option<Rc<dyn Fn(Rc<dyn Fn()>)>> {
        SLOT.with(|slot| slot.borrow().clone())
    }
}
