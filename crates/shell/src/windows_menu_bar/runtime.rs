//! Connects the SDL platform thread to the installed Windows App SDK framework.
use super::abi::*;
use windows::Foundation::IClosable;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::System::Memory::{GetProcessHeap, HEAP_FLAGS, HeapFree};
use windows::Win32::System::WinRT::{
    RO_INIT_SINGLETHREADED, RoGetActivationFactory, RoInitialize, RoUninitialize,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, MSG, MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, PM_REMOVE,
    PeekMessageW, PostQuitMessage, QS_ALLINPUT, TranslateMessage, WM_QUIT,
};
use windows::core::{Error, HRESULT, HSTRING, IUnknown, Interface, PCWSTR, PWSTR, Result, s, w};

const FAMILY: PCWSTR = w!("Microsoft.WindowsAppRuntime.1.8_8wekyb3d8bbwe");

type Create =
    unsafe extern "system" fn(Raw, PCWSTR, u64, u32, i32, PCWSTR, u32, *mut PWSTR) -> HRESULT;
type Add = unsafe extern "system" fn(PCWSTR, i32, u32, *mut Raw, *mut PWSTR) -> HRESULT;
type Remove = unsafe extern "system" fn(Raw) -> HRESULT;
type Delete = unsafe extern "system" fn(PCWSTR) -> HRESULT;

struct PackageApi {
    create: Create,
    add: Add,
    remove: Remove,
    delete: Delete,
}

impl PackageApi {
    fn load() -> Result<Self> {
        // SAFETY: these four documented functions are present in KernelBase
        // on Windows 11. Resolving them at runtime keeps older Windows loadable.
        unsafe {
            let module = GetModuleHandleW(w!("kernelbase.dll"))?;
            macro_rules! resolve {
                ($name:literal, $ty:ty) => {
                    core::mem::transmute::<unsafe extern "system" fn() -> isize, $ty>(
                        GetProcAddress(module, s!($name)).ok_or_else(Error::from_thread)?,
                    )
                };
            }
            Ok(Self {
                create: resolve!("TryCreatePackageDependency", Create),
                add: resolve!("AddPackageDependency", Add),
                remove: resolve!("RemovePackageDependency", Remove),
                delete: resolve!("DeletePackageDependency", Delete),
            })
        }
    }
}

pub fn available() -> bool {
    Package::resolve().is_ok()
}

struct Package {
    api: PackageApi,
    id: PWSTR,
    context: Raw,
}

impl Package {
    fn resolve() -> Result<Self> {
        let api = PackageApi::load()?;
        let mut id = PWSTR::null();
        // SAFETY: process lifetime, current user and architecture. The
        // installed 1.8 framework owns the WinUI activation registration.
        unsafe {
            (api.create)(
                core::ptr::null_mut(),
                FAMILY,
                0,
                0,
                0,
                PCWSTR::null(),
                0,
                &mut id,
            )
            .ok()?;
            Ok(Self {
                api,
                id,
                context: core::ptr::null_mut(),
            })
        }
    }

    fn attach() -> Result<Self> {
        let mut package = Self::resolve()?;
        // SAFETY: the package dependency ID is live until package is dropped.
        unsafe {
            (package.api.add)(
                PCWSTR(package.id.0),
                0,
                0,
                &mut package.context,
                core::ptr::null_mut(),
            )
            .ok()?;
        }
        Ok(package)
    }
}

impl Drop for Package {
    fn drop(&mut self) {
        // SAFETY: these values were returned by the dynamic dependency API.
        unsafe {
            if !self.context.is_null() {
                let _ = (self.api.remove)(self.context);
            }
            let _ = (self.api.delete)(PCWSTR(self.id.0));
            if let Ok(heap) = GetProcessHeap() {
                let _ = HeapFree(heap, HEAP_FLAGS(0), Some(self.id.0.cast()));
            }
        }
    }
}

struct Apartment;

impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: this balances RoInitialize on the same SDL thread.
        unsafe { RoUninitialize() };
    }
}

pub struct MenuRuntime {
    manager: Option<IClosable>,
    controller: Option<IDispatcherController>,
    application: Option<IApplication>,
    messages: Option<MessageHook>,
    _apartment: Apartment,
    _package: Package,
}

impl MenuRuntime {
    pub fn start() -> Result<Self> {
        let package = Package::attach()?;
        // SAFETY: all WinUI objects are created and used on the SDL thread.
        unsafe { RoInitialize(RO_INIT_SINGLETHREADED)? };
        let mut runtime = Self {
            manager: None,
            controller: None,
            application: None,
            messages: None,
            _apartment: Apartment,
            _package: package,
        };
        let dispatcher = factory::<IDispatcherStatics>("Microsoft.UI.Dispatching.DispatcherQueue")?;
        let mut current = core::ptr::null_mut();
        // SAFETY: the factory matches the class and current is writable.
        unsafe { (dispatcher.vtable().current)(dispatcher.as_raw(), &mut current).ok()? };
        if current.is_null() {
            let factory = factory::<IDispatcherControllerStatics>(
                "Microsoft.UI.Dispatching.DispatcherQueueController",
            )?;
            let created = object(&factory, factory.vtable().create_on_current_thread)?;
            runtime.controller = Some(created.cast()?);
        } else {
            // SAFETY: the static method returned an owned reference.
            drop(unsafe { IUnknown::from_raw(current) });
        }
        let application = super::application::create()?;
        let manager =
            factory::<IWindowsXamlManagerStatics>("Microsoft.UI.Xaml.Hosting.WindowsXamlManager")?;
        runtime.manager = Some(object(&manager, manager.vtable().initialize)?.cast()?);
        super::application::load_controls_resources(&application)?;
        runtime.application = Some(application);
        runtime.messages = Some(MessageHook::install()?);
        Ok(runtime)
    }
}

impl Drop for MenuRuntime {
    fn drop(&mut self) {
        if let Some(manager) = self.manager.take() {
            let _ = manager.Close();
        }
        let Some(controller) = self.controller.take() else {
            return;
        };
        let Ok(action) = object(&controller, controller.vtable().shutdown) else {
            return;
        };
        let Ok(status) = action.cast::<windows_future::IAsyncInfo>() else {
            return;
        };
        // WinUI posts its final dispatcher work before releasing the queue.
        let mut quit_code = None;
        while matches!(status.Status(), Ok(windows_future::AsyncStatus::Started)) {
            let mut message = MSG::default();
            // SAFETY: pump only the owning SDL thread while its queue drains.
            unsafe {
                if PeekMessageW(&mut message, None::<HWND>, 0, 0, PM_REMOVE).as_bool() {
                    if message.message == WM_QUIT {
                        quit_code = Some(message.wParam.0 as i32);
                    } else if self
                        .messages
                        .as_ref()
                        .is_none_or(|hook| !(hook.translate)(&mut message).as_bool())
                    {
                        let _ = TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                } else {
                    let _ = MsgWaitForMultipleObjectsEx(None, 25, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
                }
            }
        }
        if let Some(code) = quit_code {
            // SAFETY: restore the owning thread's quit message.
            unsafe { PostQuitMessage(code) };
        }
    }
}

type Translate = unsafe extern "system" fn(*mut MSG) -> windows::core::BOOL;
type SdlMessageHook = unsafe extern "C" fn(Raw, *mut MSG) -> bool;

unsafe extern "C" {
    fn SDL_SetWindowsMessageHook(callback: Option<SdlMessageHook>, data: Raw);
}

struct MessageHook {
    translate: Translate,
}

impl MessageHook {
    fn install() -> Result<Self> {
        // SAFETY: the Windows App SDK loads its windowing DLL with WinUI. SDL
        // calls this hook on the platform thread before dispatching messages.
        unsafe {
            let module = GetModuleHandleW(w!("Microsoft.UI.Windowing.Core.dll"))?;
            let address = GetProcAddress(module, s!("ContentPreTranslateMessage"))
                .ok_or_else(Error::from_thread)?;
            let translate =
                core::mem::transmute::<unsafe extern "system" fn() -> isize, Translate>(address);
            SDL_SetWindowsMessageHook(Some(pre_translate), address as Raw);
            Ok(Self { translate })
        }
    }
}

impl Drop for MessageHook {
    fn drop(&mut self) {
        // SAFETY: remove the hook before releasing the WinUI package graph.
        unsafe { SDL_SetWindowsMessageHook(None, core::ptr::null_mut()) };
    }
}

unsafe extern "C" fn pre_translate(translate: Raw, message: *mut MSG) -> bool {
    // SAFETY: SDL passes the saved WinUI export and a live MSG.
    let translate = unsafe { core::mem::transmute::<Raw, Translate>(translate) };
    !super::dispatch_shortcut(unsafe { &*message }) && !unsafe { translate(message) }.as_bool()
}

pub fn factory<T: Interface>(class: &str) -> Result<T> {
    // SAFETY: WinRT checks the IID against the named factory.
    unsafe { RoGetActivationFactory(&HSTRING::from(class)) }
}

pub fn object(
    this: &impl Interface,
    get: unsafe extern "system" fn(Raw, *mut Raw) -> HRESULT,
) -> Result<IUnknown> {
    let mut raw = core::ptr::null_mut();
    // SAFETY: callers supply the getter belonging to this interface.
    unsafe { get(this.as_raw(), &mut raw).ok()? };
    take_object(raw)
}

pub fn take_object(raw: Raw) -> Result<IUnknown> {
    if raw.is_null() {
        return Err(Error::new(
            HRESULT(0x8000_4003_u32 as i32),
            "WinUI returned a null object",
        ));
    }
    // SAFETY: the object was returned with an owned COM reference.
    Ok(unsafe { IUnknown::from_raw(raw) })
}
