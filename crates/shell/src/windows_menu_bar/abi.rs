//! The WinUI 3 methods used by the menu host. IIDs and slot order are from
//! Microsoft.UI.winmd and Microsoft.UI.Xaml.winmd in Windows App SDK 1.8.
use core::ffi::c_void;
use windows::Graphics::RectInt32;
use windows::core::{HRESULT, IInspectable_Vtbl, IUnknown, Interface};

pub type Raw = *mut c_void;
type GetObject = unsafe extern "system" fn(Raw, *mut Raw) -> HRESULT;
type PutObject = unsafe extern "system" fn(Raw, Raw) -> HRESULT;

macro_rules! interface {
    ($name:ident, $vtable:ident, $iid:literal, $($fields:tt)*) => {
        windows_core::imp::define_interface!($name, $vtable, $iid);
        #[repr(C)]
        pub struct $vtable {
            pub(super) base: IInspectable_Vtbl,
            $($fields)*
        }
    };
}

interface!(IDispatcherStatics, DispatcherStaticsVtbl, 0xcd3382ea_a455_5124_b63a_ca40d34ca23c,
    pub current: GetObject,
);
interface!(IDispatcherControllerStatics, DispatcherControllerStaticsVtbl, 0xf18d6145_722b_593d_bcf2_a61e713f0037,
    dedicated_thread: usize,
    pub create_on_current_thread: GetObject,
);
interface!(IDispatcherController, DispatcherControllerVtbl, 0xbce8178d_2183_584c_9e5b_f9366f6ae484,
    dispatcher: usize,
    pub shutdown: GetObject,
);
interface!(IWindowsXamlManagerStatics, WindowsXamlManagerStaticsVtbl, 0x56cb591d_de97_539f_881d_8ccdc44fa6c4,
    pub initialize: GetObject,
);
interface!(IApplicationFactory, ApplicationFactoryVtbl, 0x9fd96657_5294_5a65_a1db_4fea143597da,
    pub create: unsafe extern "system" fn(Raw, Raw, *mut Raw, *mut Raw) -> HRESULT,
);
interface!(IApplication, ApplicationVtbl, 0x06a8f4e7_1146_55af_820d_ebd55643b021,
    pub resources: GetObject,
);
interface!(IResourceDictionary, ResourceDictionaryVtbl, 0x1b690975_a710_5783_a6e1_15836f6186c2,
    source: usize,
    set_source: usize,
    pub merged: GetObject,
);
interface!(IXamlMetadataStatics, XamlMetadataStaticsVtbl, 0x2d7eb3fd_ecdb_5084_b7e0_12f9598381ef,
    pub initialize: unsafe extern "system" fn(Raw) -> HRESULT,
);
interface!(IXamlMetadataProvider, XamlMetadataProviderVtbl, 0xa96251f0_2214_5d53_8746_ce99a2593cd7,
    pub type_by_name: unsafe extern "system" fn(Raw, TypeName, *mut Raw) -> HRESULT,
    pub type_by_full_name: unsafe extern "system" fn(Raw, Raw, *mut Raw) -> HRESULT,
    pub xmlns: unsafe extern "system" fn(Raw, *mut u32, *mut Raw) -> HRESULT,
);
interface!(IXamlSource, XamlSourceVtbl, 0x553af92c_1381_51d6_bee0_f34beb042ea8,
    get_content: usize,
    pub set_content: PutObject,
    has_focus: usize,
    get_backdrop: usize,
    set_backdrop: usize,
    pub site_bridge: GetObject,
    focus_events_and_navigation: [usize; 5],
    pub initialize: unsafe extern "system" fn(Raw, WindowId) -> HRESULT,
);
interface!(IDesktopSite, DesktopSiteVtbl, 0xf0ae8750_905c_50a2_8a12_4545c6245bb4,
    before_move_and_resize: [usize; 7],
    pub move_and_resize: unsafe extern "system" fn(Raw, RectInt32) -> HRESULT,
    z_order: [usize; 3],
    pub show: unsafe extern "system" fn(Raw) -> HRESULT,
);

interface!(IXamlReaderStatics, XamlReaderStaticsVtbl, 0x82a4cd9e_435e_5aeb_8c4f_300cece45cae,
    pub load: unsafe extern "system" fn(Raw, Raw, *mut Raw) -> HRESULT,
);
interface!(IAppWindowStatics, AppWindowStaticsVtbl, 0x3c315c24_d540_5d72_b518_b226b83627cb,
    create: [usize; 3],
    pub get_from_window_id: unsafe extern "system" fn(Raw, WindowId, *mut Raw) -> HRESULT,
);
interface!(IAppWindow, AppWindowVtbl, 0xcfa788b3_643b_5c5e_ad4e_321d48a82acd,
    before_title_bar: [usize; 10],
    pub title_bar: GetObject,
);
interface!(IAppWindowTitleBar, AppWindowTitleBarVtbl, 0x5574efa2_c91c_5700_a363_539c71a7aaf4,
    before_button_background: [usize; 3],
    pub set_button_background: PutObject,
    before_inactive_background: [usize; 7],
    pub set_inactive_button_background: PutObject,
    before_set_extends: [usize; 7],
    pub set_extends: unsafe extern "system" fn(Raw, bool) -> HRESULT,
    foreground: [usize; 2],
    pub height: unsafe extern "system" fn(Raw, *mut i32) -> HRESULT,
    icon_and_inactive_colors: [usize; 6],
    pub left_inset: unsafe extern "system" fn(Raw, *mut i32) -> HRESULT,
    pub right_inset: unsafe extern "system" fn(Raw, *mut i32) -> HRESULT,
    pub reset: unsafe extern "system" fn(Raw) -> HRESULT,
);
interface!(IAppWindowTitleBar2, AppWindowTitleBar2Vtbl, 0x86faed38_748a_5b4b_9ccf_3ba0496c9041,
    preferred_height: usize,
    pub set_preferred_height: unsafe extern "system" fn(Raw, i32) -> HRESULT,
);
interface!(IAppWindowTitleBar3, AppWindowTitleBar3Vtbl, 0x07146e74_0410_5597_aba7_1af276d2ae07,
    preferred_theme: usize,
    pub set_preferred_theme: unsafe extern "system" fn(Raw, i32) -> HRESULT,
);
interface!(INonClientInputStatics, NonClientInputStaticsVtbl, 0x7d0b775c_1903_5dc7_bd2f_7a4b31f0cff2,
    pub get_for_window_id: unsafe extern "system" fn(Raw, WindowId, *mut Raw) -> HRESULT,
);
interface!(INonClientInput, NonClientInputVtbl, 0x471732b4_3d07_5104_b192_ebacf71e86df,
    dispatcher_and_clear_all: [usize; 2],
    pub clear_region: unsafe extern "system" fn(Raw, i32) -> HRESULT,
    get_region: usize,
    pub set_region: unsafe extern "system" fn(Raw, i32, u32, *const RectInt32) -> HRESULT,
);
interface!(IXamlBindingHelper, XamlBindingHelperVtbl, 0x93c7dad3_f9c2_5372_84dc_9e9c4661d083,
    before_convert_value: [usize; 5],
    pub convert_value: unsafe extern "system" fn(Raw, TypeName, Raw, *mut Raw) -> HRESULT,
);
interface!(
    IColorReference,
    ColorReferenceVtbl,
    0xab8e5d11_b0c1_5a21_95ae_f16bf3a37624,
);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WindowId {
    pub value: u64,
}

#[repr(C)]
pub struct TypeName {
    pub name: Raw,
    pub kind: i32,
}

pub trait ControlFactory: Interface {
    const CLASS: &'static str;
    unsafe fn create(&self, inner: *mut Raw, result: *mut Raw) -> HRESULT;
}

macro_rules! control_factory {
    ($name:ident, $vtable:ident, $iid:literal, $class:literal) => {
        interface!($name, $vtable, $iid,
            create: unsafe extern "system" fn(Raw, Raw, *mut Raw, *mut Raw) -> HRESULT,
        );
        impl ControlFactory for $name {
            const CLASS: &'static str = concat!("Microsoft.UI.Xaml.Controls.", $class);
            unsafe fn create(&self, inner: *mut Raw, result: *mut Raw) -> HRESULT {
                // SAFETY: the composable factory accepts null for the outer object.
                unsafe { (self.vtable().create)(self.as_raw(), core::ptr::null_mut(), inner, result) }
            }
        }
    };
}

control_factory!(
    IMenuBarFactory,
    MenuBarFactoryVtbl,
    0x76aa8759_04ee_5a4c_b98c_d03742d47cdb,
    "MenuBar"
);
control_factory!(
    IMenuHeadingFactory,
    MenuHeadingFactoryVtbl,
    0x87d02172_83cb_5459_940f_173f7501b300,
    "MenuBarItem"
);
control_factory!(
    IMenuItemFactory,
    MenuItemFactoryVtbl,
    0x9c3c9a1f_89af_521a_81a5_8a01db7a79af,
    "MenuFlyoutItem"
);
control_factory!(
    IToggleFactory,
    ToggleFactoryVtbl,
    0x426dfd57_6cc9_570f_950d_37437235dc89,
    "ToggleMenuFlyoutItem"
);
control_factory!(
    ISeparatorFactory,
    SeparatorFactoryVtbl,
    0x26156c9c_95ef_5e55_8342_773fc43baac3,
    "MenuFlyoutSeparator"
);

interface!(IMenuBar, MenuBarVtbl, 0xba97f337_8f1e_5141_b53f_e77a8ba3ebbd,
    pub items: GetObject,
);
interface!(IMenuHeading, MenuHeadingVtbl, 0xa7900980_51cc_531d_97c5_356b13573398,
    title: usize,
    pub set_title: PutObject,
    pub items: GetObject,
);
interface!(IMenuItem, MenuItemVtbl, 0x4252df5a_44f9_5ee8_b1cc_53de9aaa4095,
    text: usize,
    pub set_text: PutObject,
    command_parameter_and_icon: [usize; 6],
    accelerator_text: usize,
    pub set_accelerator_text: PutObject,
    template_settings: usize,
    pub add_click: unsafe extern "system" fn(Raw, Raw, *mut i64) -> HRESULT,
    remove_click: usize,
);
interface!(IMenuSubItem, MenuSubItemVtbl, 0x6b0688c1_47b0_53b5_b6f9_5ec5d6623b84,
    pub items: GetObject,
    text: usize,
    pub set_text: PutObject,
);
interface!(IChecked, CheckedVtbl, 0x1803f260_67e4_5bc1_a63a_123510167bb8,
    checked: usize,
    pub set_checked: unsafe extern "system" fn(Raw, bool) -> HRESULT,
);
interface!(IControl, ControlVtbl, 0x857d6e8a_d45a_5c69_a99c_bf6a5c54fb38,
    before_set_enabled: [usize; 23],
    pub set_enabled: unsafe extern "system" fn(Raw, bool) -> HRESULT,
);
interface!(IFrameworkElement, FrameworkElementVtbl, 0xfe08f13d_dc6a_5495_ad44_c2d8d21863b0,
    before_actual_width: [usize; 7],
    pub actual_width: unsafe extern "system" fn(Raw, *mut f64) -> HRESULT,
    before_set_requested_theme: [usize; 44],
    pub set_requested_theme: unsafe extern "system" fn(Raw, i32) -> HRESULT,
    before_layout_updated: [usize; 10],
    pub add_layout_updated: unsafe extern "system" fn(Raw, Raw, *mut i64) -> HRESULT,
    pub remove_layout_updated: unsafe extern "system" fn(Raw, i64) -> HRESULT,
);
interface!(
    IUIElement,
    UIElementVtbl,
    0xc3c01020_320c_5cf6_9d24_d396bbfa4d8b,
);
interface!(IPanel, PanelVtbl, 0x27a1b418_56f3_525e_b883_cefed905eed3,
    pub children: GetObject,
);
interface!(
    IMenuItemBase,
    MenuItemBaseVtbl,
    0x4bee2715_44a1_5f94_86e8_02ddbe3dc6b9,
);

#[repr(C)]
struct MenuVectorVtbl {
    base: IInspectable_Vtbl,
    before_append: [usize; 7],
    append: PutObject,
}

/// WinUI vectors accept the default interface of their runtime class in the
/// Append slot.
pub unsafe fn append(vector: &IUnknown, item: &impl Interface) -> windows::core::Result<()> {
    // SAFETY: the caller uses only the two menu collection properties above,
    // with the element interface corresponding to that collection.
    unsafe {
        let vtable = &**(vector.as_raw() as *const *const MenuVectorVtbl);
        (vtable.append)(vector.as_raw(), item.as_raw()).ok()
    }
}
