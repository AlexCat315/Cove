//! Translation of Desktop/Sources/CoveUI/Application/Services.swift.
//!
//! Captures host-scoped services once and restores them on lifecycle teardown,
//! so a window that installed process-wide ambients cannot leak them into the
//! next run.

use crate::desktop::application::composition::{
    AppDisplayHandleHolder, AssetDropRegistry, AssetDropRegistryHolder,
};
use cove_core::scene::{
    ClipboardHolder, ClipboardRequester, ColorSampler, ColorSamplerCallback, FilePicker,
    FilePickerCallback, FocusChain, FocusChainHolder, InteractionRegistry,
    InteractionRegistryHolder, PointerCapture, PointerCaptureHolder, UIWorkSchedulerHolder,
};
use cove_imaging::{ImageAssetRegistry, ImageAssetRegistryHolder};
use cove_text::{TextEnvironment, TextEnvironmentHolder};
use std::rc::Rc;

pub struct ApplicationServices {
    text: Option<TextEnvironment>,
    scale: f32,
    interactions: Option<Rc<InteractionRegistry>>,
    focus: Option<Rc<FocusChain>>,
    capture: Option<Rc<PointerCapture>>,
    clipboard_read: Option<ClipboardRequester>,
    clipboard_write: Option<Rc<dyn Fn(&str)>>,
    sampler: Option<ColorSamplerCallback>,
    picker: Option<FilePickerCallback>,
    images: Option<std::sync::Arc<ImageAssetRegistry>>,
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
            sampler: color_sampler_handler::handler(),
            picker: file_picker_handler::handler(),
            images: ImageAssetRegistryHolder::current(),
            drops: AssetDropRegistryHolder::current(),
            work: work_scheduler::handler(),
            display: AppDisplayHandleHolder::current(),
        }
    }

    /// Restores every captured service to the value it held before the run.
    pub fn restore(self) {
        TextEnvironmentHolder::set_current(self.text);
        cove_core::set_content_scale(self.scale);
        InteractionRegistryHolder::set_current(self.interactions);
        FocusChainHolder::set_current(self.focus);
        PointerCaptureHolder::set_current(self.capture);
        ClipboardHolder::set_read_requester(self.clipboard_read.clone());
        ClipboardHolder::set_write(self.clipboard_write.clone());
        clipboard_handlers::remember(self.clipboard_read, self.clipboard_write);
        ColorSampler::set_requester(self.sampler.clone());
        color_sampler_handler::remember(self.sampler);
        FilePicker::set_presenter(self.picker.clone());
        file_picker_handler::remember(self.picker);
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
        static SLOT: RefCell<(Option<ClipboardRequester>, Option<Rc<dyn Fn(&str)>>)> =
            const { RefCell::new((None, None)) };
    }

    pub fn remember(read: Option<ClipboardRequester>, write: Option<Rc<dyn Fn(&str)>>) {
        SLOT.with(|slot| *slot.borrow_mut() = (read, write));
    }

    pub fn read() -> Option<ClipboardRequester> {
        SLOT.with(|slot| slot.borrow().0.clone())
    }

    pub fn write() -> Option<Rc<dyn Fn(&str)>> {
        SLOT.with(|slot| slot.borrow().1.clone())
    }
}

/// Screen-colour sampler registry, mirroring [`clipboard_handlers`] because
/// `ColorSampler` exposes no getter.
pub mod color_sampler_handler {
    use super::*;
    use std::cell::RefCell;

    thread_local! {
        static SLOT: RefCell<Option<ColorSamplerCallback>> = const { RefCell::new(None) };
    }

    pub fn remember(handler: Option<ColorSamplerCallback>) {
        SLOT.with(|slot| *slot.borrow_mut() = handler);
    }

    pub fn handler() -> Option<ColorSamplerCallback> {
        SLOT.with(|slot| slot.borrow().clone())
    }
}

/// Native open-panel registry, mirroring [`color_sampler_handler`] because
/// `FilePicker` exposes no getter.
pub mod file_picker_handler {
    use super::*;
    use std::cell::RefCell;

    thread_local! {
        static SLOT: RefCell<Option<FilePickerCallback>> = const { RefCell::new(None) };
    }

    pub fn remember(handler: Option<FilePickerCallback>) {
        SLOT.with(|slot| *slot.borrow_mut() = handler);
    }

    pub fn handler() -> Option<FilePickerCallback> {
        SLOT.with(|slot| slot.borrow().clone())
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
