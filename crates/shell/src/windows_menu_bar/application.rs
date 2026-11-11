//! Gives the hosted WinUI application the SDK's Fluent XAML metadata provider.
use super::abi::*;
use super::runtime::{factory, object, take_object};
use std::cell::OnceCell;
use std::ptr::{null_mut, write};
use std::sync::atomic::{AtomicU32, Ordering};
use windows::core::imp::IGenericFactory;
use windows::core::{
    GUID, HRESULT, HSTRING, IInspectable, IInspectable_Vtbl, IUnknown, IUnknown_Vtbl, Interface,
    Result,
};

const APPLICATION: &str = "Microsoft.UI.Xaml.Application";
const METADATA: &str = "Microsoft.UI.Xaml.XamlTypeInfo.XamlControlsXamlMetaDataProvider";

pub fn create() -> Result<IApplication> {
    let statics = factory::<IXamlMetadataStatics>(METADATA)?;
    // SAFETY: initialize the registered WinUI provider on the XAML thread.
    unsafe { (statics.vtable().initialize)(statics.as_raw()).ok()? };
    let provider = factory::<IGenericFactory>(METADATA)?.ActivateInstance()?;
    let factory = factory::<IApplicationFactory>(APPLICATION)?;
    let mut inner = null_mut();
    let mut app = null_mut();
    let host = Box::into_raw(Box::new(ApplicationHost {
        vtable: &ApplicationHost::VTABLE,
        references: AtomicU32::new(1),
        inner: OnceCell::new(),
        provider,
    }));
    // SAFETY: transfer the initial ref to a guard. The composable factory
    // uses the host as the controlling IUnknown of the XAML application.
    let _host = unsafe { IUnknown::from_raw(host.cast()) };
    let status =
        unsafe { (factory.vtable().create)(factory.as_raw(), host.cast(), &mut inner, &mut app) };
    if !inner.is_null() {
        // SAFETY: the factory returns one non-delegating reference to the inner.
        let _ = unsafe { &*host }.inner.set(take_object(inner)?);
    }
    status.ok()?;
    take_object(app)?.cast()
}

pub fn load_controls_resources(app: &IApplication) -> Result<()> {
    let resources = object(app, app.vtable().resources)?.cast::<IResourceDictionary>()?;
    let merged = object(&resources, resources.vtable().merged)?;
    let controls = factory::<IGenericFactory>("Microsoft.UI.Xaml.Controls.XamlControlsResources")?
        .ActivateInstance::<IResourceDictionary>()?;
    // SAFETY: MergedDictionaries is IVector<ResourceDictionary>.
    unsafe { append(&merged, &controls) }
}

#[repr(C)]
struct ApplicationHost {
    vtable: &'static XamlMetadataProviderVtbl,
    references: AtomicU32,
    inner: OnceCell<IUnknown>,
    provider: IXamlMetadataProvider,
}

impl ApplicationHost {
    const VTABLE: XamlMetadataProviderVtbl = XamlMetadataProviderVtbl {
        base: IInspectable_Vtbl {
            base: IUnknown_Vtbl {
                QueryInterface: Self::query,
                AddRef: Self::add_ref,
                Release: Self::release,
            },
            GetIids: Self::get_iids,
            GetRuntimeClassName: Self::get_name,
            GetTrustLevel: Self::get_trust,
        },
        type_by_name: Self::type_by_name,
        type_by_full_name: Self::type_by_full_name,
        xmlns: Self::xmlns,
    };

    unsafe extern "system" fn query(this: Raw, iid: *const GUID, output: *mut Raw) -> HRESULT {
        if iid.is_null() || output.is_null() {
            return HRESULT(0x8000_4003_u32 as i32);
        }
        // SAFETY: COM passes a live interface, readable IID and writable output.
        unsafe {
            let host = &*this.cast::<Self>();
            if [IUnknown::IID, IInspectable::IID, IXamlMetadataProvider::IID].contains(&*iid) {
                Self::add_ref(this);
                write(output, this);
                return HRESULT(0);
            }
            if let Some(inner) = host.inner.get() {
                return (inner.vtable().QueryInterface)(inner.as_raw(), iid, output);
            }
            write(output, null_mut());
            HRESULT(0x8000_4002_u32 as i32)
        }
    }

    unsafe extern "system" fn add_ref(this: Raw) -> u32 {
        unsafe { &*this.cast::<Self>() }
            .references
            .fetch_add(1, Ordering::Relaxed)
            + 1
    }

    unsafe extern "system" fn release(this: Raw) -> u32 {
        let count = unsafe { &*this.cast::<Self>() }
            .references
            .fetch_sub(1, Ordering::AcqRel)
            - 1;
        if count == 0 {
            drop(unsafe { Box::from_raw(this.cast::<Self>()) });
        }
        count
    }

    unsafe extern "system" fn get_iids(
        this: Raw,
        count: *mut u32,
        output: *mut *mut GUID,
    ) -> HRESULT {
        let provider = &unsafe { &*this.cast::<Self>() }.provider;
        // SAFETY: forward COM's out parameters to the metadata provider.
        unsafe { (provider.vtable().base.GetIids)(provider.as_raw(), count, output) }
    }

    unsafe extern "system" fn get_name(_this: Raw, output: *mut Raw) -> HRESULT {
        if output.is_null() {
            return HRESULT(0x8000_4003_u32 as i32);
        }
        // SAFETY: transfer the owned HSTRING handle to the caller.
        unsafe {
            write(
                output,
                std::mem::transmute::<HSTRING, Raw>(HSTRING::from(APPLICATION)),
            )
        };
        HRESULT(0)
    }

    unsafe extern "system" fn get_trust(_this: Raw, output: *mut i32) -> HRESULT {
        if output.is_null() {
            return HRESULT(0x8000_4003_u32 as i32);
        }
        // SAFETY: COM supplies storage for TrustLevel::BaseTrust.
        unsafe { write(output, 0) };
        HRESULT(0)
    }

    unsafe extern "system" fn type_by_name(this: Raw, name: TypeName, output: *mut Raw) -> HRESULT {
        let provider = &unsafe { &*this.cast::<Self>() }.provider;
        // SAFETY: the aggregate exposes the same ABI as the SDK provider.
        unsafe { (provider.vtable().type_by_name)(provider.as_raw(), name, output) }
    }

    unsafe extern "system" fn type_by_full_name(this: Raw, name: Raw, output: *mut Raw) -> HRESULT {
        let provider = &unsafe { &*this.cast::<Self>() }.provider;
        // SAFETY: name is the borrowed HSTRING from WinUI.
        unsafe { (provider.vtable().type_by_full_name)(provider.as_raw(), name, output) }
    }

    unsafe extern "system" fn xmlns(this: Raw, count: *mut u32, output: *mut Raw) -> HRESULT {
        let provider = &unsafe { &*this.cast::<Self>() }.provider;
        // SAFETY: the SDK provider allocates and returns the WinRT array.
        unsafe { (provider.vtable().xmlns)(provider.as_raw(), count, output) }
    }
}
