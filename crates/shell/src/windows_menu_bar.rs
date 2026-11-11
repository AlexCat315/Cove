//! A WinUI MenuBar hosted in the SDL window on the platform thread.
mod abi;
mod application;
mod runtime;
mod title_bar;

pub(crate) use runtime::{MenuRuntime, available};

use crate::{Sdl3Shell, WindowAppearance};
use abi::*;
use cove_platform::{NativeMenuBarSnapshot, NativeMenuKind, NativeMenuSnapshot, NativeShortcut};
use runtime::{factory, object, take_object};
use std::cell::{Cell, RefCell};
use std::ptr::{null_mut, write};
use std::rc::{Rc, Weak};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU32, Ordering},
};
use title_bar::TitleBar;
use windows::Foundation::{EventHandler, IClosable};
use windows::Graphics::RectInt32;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    GA_ROOTOWNER, GetAncestor, GetClientRect, MSG, WM_DPICHANGED, WM_KEYDOWN, WM_SIZE,
    WM_SYSKEYDOWN,
};
use windows::core::imp::IGenericFactory;
use windows::core::{
    Error, GUID, HRESULT, HSTRING, IInspectable, IUnknown, IUnknown_Vtbl, Interface, Result,
};

type Subclass = unsafe extern "system" fn(Raw, u32, usize, isize, usize, usize) -> isize;

#[link(name = "user32")]
unsafe extern "system" {
    fn GetKeyState(key: i32) -> i16;
}

#[link(name = "comctl32")]
unsafe extern "system" {
    fn SetWindowSubclass(hwnd: Raw, proc: Subclass, id: usize, data: usize) -> i32;
    fn RemoveWindowSubclass(hwnd: Raw, proc: Subclass, id: usize) -> i32;
    fn DefSubclassProc(hwnd: Raw, message: u32, wparam: usize, lparam: isize) -> isize;
}

type Choices = Arc<Mutex<Vec<i64>>>;

struct Island(IClosable);

impl Drop for Island {
    fn drop(&mut self) {
        let _ = self.0.Close();
    }
}

struct MenuState {
    hwnd: HWND,
    site: IDesktopSite,
    title_bar: Rc<TitleBar>,
    menu: IFrameworkElement,
    menu_height: Rc<Cell<i32>>,
    available: Cell<RectInt32>,
    island_bounds: Cell<RectInt32>,
    input_bounds: Cell<Option<RectInt32>>,
    choices: Choices,
    shortcuts: Vec<((usize, u8), i64)>,
}

thread_local! {
    static ACTIVE_MENU: RefCell<Weak<MenuState>> = const { RefCell::new(Weak::new()) };
}

impl MenuState {
    fn resize(&self) -> Result<()> {
        let mut rect = RECT::default();
        // SAFETY: the parent window is alive while the site is attached.
        unsafe { GetClientRect(self.hwnd, &mut rect)? };
        if rect.right == 0 || rect.bottom == 0 {
            return Ok(());
        }
        let (height, left, right) = self.title_bar.metrics()?;
        let scale = self.scale();
        let left = left + (8.0 * scale).round() as i32;
        let drag_space = (48.0 * scale).round() as i32;
        let available = RectInt32 {
            X: left,
            Y: 0,
            Width: (rect.right - left - right - drag_space).max(0),
            Height: height.max(0),
        };
        self.available.set(available);
        self.move_island(available)?;
        self.sync_input()?;
        self.menu_height.set(available.Height);
        Ok(())
    }

    fn scale(&self) -> f64 {
        // SAFETY: the parent window remains live while the menu is attached.
        f64::from(unsafe { GetDpiForWindow(self.hwnd) }.max(96)) / 96.0
    }

    fn move_island(&self, bounds: RectInt32) -> Result<()> {
        if bounds == self.island_bounds.get() {
            return Ok(());
        }
        // SAFETY: bounds are in physical pixels in the parent client area.
        unsafe { (self.site.vtable().move_and_resize)(self.site.as_raw(), bounds).ok()? };
        self.island_bounds.set(bounds);
        Ok(())
    }

    fn sync_input(&self) -> Result<()> {
        let available = self.available.get();
        let mut width = 0.0;
        // SAFETY: WinUI supplies the arranged width of the menu on this thread.
        unsafe {
            (self.menu.vtable().actual_width)(self.menu.as_raw(), &mut width).ok()?;
        }
        let width = (width * self.scale()).ceil() as i32;
        let input = RectInt32 {
            Width: width.clamp(0, available.Width),
            ..available
        };
        if input.Width > 0 && input.Height > 0 {
            self.move_island(input)?;
        }
        if self.input_bounds.get() != Some(input) {
            self.title_bar.set_menu_region(input)?;
            self.input_bounds.set(Some(input));
        }
        Ok(())
    }
}

pub(crate) struct WindowsMenuBar {
    state: Rc<MenuState>,
    theme: IFrameworkElement,
    layout_token: Option<i64>,
    _island: Island,
}

impl WindowsMenuBar {
    pub fn install(
        hwnd: usize,
        snapshot: &NativeMenuBarSnapshot,
        appearance: WindowAppearance,
        menu_height: Rc<Cell<i32>>,
        previous: Option<&Self>,
    ) -> Result<Self> {
        let hwnd = HWND(hwnd as Raw);
        let window_id = WindowId {
            value: hwnd.0 as usize as u64,
        };
        let source = activate::<IXamlSource>("Microsoft.UI.Xaml.Hosting.DesktopWindowXamlSource")?;
        let island = Island(source.cast()?);
        // SAFETY: WindowId carries the HWND of the owning SDL window.
        unsafe {
            (source.vtable().initialize)(source.as_raw(), window_id).ok()?;
        }
        let site = object(&source, source.vtable().site_bridge)?.cast()?;
        let bar = construct::<IMenuBarFactory>()?;
        let mut actions = MenuActions::default();
        let headings = bar.cast::<IMenuBar>()?;
        let collection = object(&headings, headings.vtable().items)?;
        if !snapshot.application_items.items.is_empty() {
            add_heading(
                &collection,
                &snapshot.app_name,
                &snapshot.application_items,
                &mut actions,
            )?;
        }
        for menu in &snapshot.menus {
            add_heading(&collection, &menu.title, &menu.items, &mut actions)?;
        }
        let content = menu_surface(&bar.cast()?)?;
        let theme = content.cast()?;
        // SAFETY: the content is a UIElement on the same XAML thread.
        unsafe { (source.vtable().set_content)(source.as_raw(), content.as_raw()).ok()? };
        let previous = previous.filter(|menu| menu.state.hwnd == hwnd);
        let title_bar = match previous {
            Some(menu) => menu.state.title_bar.clone(),
            None => Rc::new(TitleBar::attach(window_id)?),
        };
        let state = MenuState {
            hwnd,
            site,
            title_bar,
            menu: bar.cast()?,
            menu_height,
            available: Cell::default(),
            island_bounds: Cell::default(),
            input_bounds: Cell::default(),
            choices: actions.choices,
            shortcuts: actions.shortcuts,
        };
        let mut menu = Self {
            state: Rc::new(state),
            theme,
            layout_token: None,
            _island: island,
        };
        menu.set_appearance(appearance)?;
        let handler = EventHandler::<IInspectable>::new(|_, _| {
            if let Some(menu) = ACTIVE_MENU.with(|slot| slot.borrow().upgrade()) {
                menu.sync_input()?;
            }
            Ok(())
        });
        let mut token = 0;
        // SAFETY: the root keeps the callback alive until the token is removed.
        unsafe {
            (menu.theme.vtable().add_layout_updated)(
                menu.theme.as_raw(),
                handler.as_raw(),
                &mut token,
            )
            .ok()?;
        }
        menu.layout_token = Some(token);
        // SAFETY: the menu state stays alive until the subclass is removed.
        unsafe {
            (menu.state.site.vtable().show)(menu.state.site.as_raw()).ok()?;
            if SetWindowSubclass(hwnd.0, subclass, menu.id(), menu.id()) == 0 {
                return Err(Error::from_thread());
            }
        }
        if let Err(error) = menu.state.resize() {
            if let Some(previous) = previous {
                previous.state.input_bounds.set(None);
                let _ = previous.state.resize();
            }
            return Err(error);
        }
        ACTIVE_MENU.with(|slot| *slot.borrow_mut() = Rc::downgrade(&menu.state));
        Ok(menu)
    }

    fn id(&self) -> usize {
        Rc::as_ptr(&self.state) as usize
    }

    pub fn hwnd(&self) -> usize {
        self.state.hwnd.0 as usize
    }

    pub fn take_choices(&self) -> Vec<i64> {
        std::mem::take(&mut *self.state.choices.lock().unwrap_or_else(|e| e.into_inner()))
    }

    pub fn set_appearance(&self, appearance: WindowAppearance) -> Result<()> {
        self.state.title_bar.set_appearance(appearance)?;
        let theme = match appearance {
            WindowAppearance::Light => 1,
            WindowAppearance::Dark => 2,
        };
        // SAFETY: ElementTheme uses a 32-bit discriminant.
        unsafe { (self.theme.vtable().set_requested_theme)(self.theme.as_raw(), theme).ok() }
    }
}

#[derive(Default)]
struct MenuActions {
    choices: Choices,
    shortcuts: Vec<((usize, u8), i64)>,
}

impl Drop for WindowsMenuBar {
    fn drop(&mut self) {
        // SAFETY: the shell drops the menu before destroying the window.
        unsafe {
            if let Some(token) = self.layout_token {
                let _ = (self.theme.vtable().remove_layout_updated)(self.theme.as_raw(), token);
            }
            RemoveWindowSubclass(self.state.hwnd.0, subclass, self.id());
        }
    }
}

fn menu_surface(bar: &IUIElement) -> Result<IUIElement> {
    // The menu sizes to its buttons and shares the window's background.
    let source = HSTRING::from(
        r#"<StackPanel xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" Orientation="Horizontal"/>"#,
    );
    let reader = factory::<IXamlReaderStatics>("Microsoft.UI.Xaml.Markup.XamlReader")?;
    let mut result = null_mut();
    // SAFETY: Load accepts a borrowed HSTRING and returns an owned reference.
    unsafe {
        (reader.vtable().load)(
            reader.as_raw(),
            std::mem::transmute_copy(&source),
            &mut result,
        )
        .ok()?;
    }
    let panel = take_object(result)?.cast::<IPanel>()?;
    let children = object(&panel, panel.vtable().children)?;
    // SAFETY: Panel.Children is a vector of UIElements.
    unsafe { append(&children, bar)? };
    panel.cast()
}

fn activate<T: Interface>(class: &str) -> Result<T> {
    factory::<IGenericFactory>(class)?.ActivateInstance()
}

fn construct<F: ControlFactory>() -> Result<IUnknown> {
    let factory = factory::<F>(F::CLASS)?;
    let mut inner = null_mut();
    let mut result = null_mut();
    // SAFETY: the factory creates a non-aggregated object and returns owned references.
    let status = unsafe { factory.create(&mut inner, &mut result) };
    if !inner.is_null() {
        drop(take_object(inner)?);
    }
    status.ok()?;
    take_object(result)
}

fn set_text(
    this: &impl Interface,
    setter: unsafe extern "system" fn(Raw, Raw) -> HRESULT,
    value: &str,
) -> Result<()> {
    let value = HSTRING::from(value);
    // SAFETY: HSTRING has transparent representation over its WinRT handle.
    unsafe { setter(this.as_raw(), std::mem::transmute_copy(&value)).ok() }
}

fn add_heading(
    collection: &IUnknown,
    title: &str,
    items: &NativeMenuSnapshot,
    state: &mut MenuActions,
) -> Result<()> {
    let heading = construct::<IMenuHeadingFactory>()?.cast::<IMenuHeading>()?;
    set_text(&heading, heading.vtable().set_title, title)?;
    add_items(&object(&heading, heading.vtable().items)?, items, state)?;
    // SAFETY: this vector is MenuBar.Items, of type IVector<MenuBarItem>.
    unsafe { append(collection, &heading) }
}

fn add_items(
    collection: &IUnknown,
    snapshot: &NativeMenuSnapshot,
    state: &mut MenuActions,
) -> Result<()> {
    for item in &snapshot.items {
        let control = match &item.kind {
            NativeMenuKind::Separator => construct::<ISeparatorFactory>()?,
            NativeMenuKind::Submenu(children) => {
                let submenu =
                    activate::<IMenuSubItem>("Microsoft.UI.Xaml.Controls.MenuFlyoutSubItem")?;
                set_text(&submenu, submenu.vtable().set_text, &item.title)?;
                add_items(&object(&submenu, submenu.vtable().items)?, children, state)?;
                submenu.cast()?
            }
            NativeMenuKind::Regular => {
                let control = if item.checked {
                    let control = construct::<IToggleFactory>()?;
                    let toggle = control.cast::<IChecked>()?;
                    // SAFETY: the interface is ToggleMenuFlyoutItem's checked property.
                    unsafe { (toggle.vtable().set_checked)(toggle.as_raw(), true).ok()? };
                    control
                } else {
                    construct::<IMenuItemFactory>()?
                };
                let action = control.cast::<IMenuItem>()?;
                set_text(&action, action.vtable().set_text, &item.title)?;
                if let Some(key) = item.shortcut.as_ref()
                    && let Some(binding) = shortcut(key)
                {
                    set_text(
                        &action,
                        action.vtable().set_accelerator_text,
                        &shortcut_label(key),
                    )?;
                    if item.enabled {
                        state.shortcuts.push((binding, item.id));
                    }
                }
                let handler = ClickHandler::new(state.choices.clone(), item.id);
                let mut token = 0;
                // SAFETY: WinUI retains the handler until the control is released.
                unsafe {
                    (action.vtable().add_click)(action.as_raw(), handler.as_raw(), &mut token)
                        .ok()?
                };
                control
            }
        };
        if !item.enabled && !matches!(item.kind, NativeMenuKind::Separator) {
            let control = control.cast::<IControl>()?;
            // SAFETY: all interactive menu items derive from Control.
            unsafe { (control.vtable().set_enabled)(control.as_raw(), false).ok()? };
        }
        // SAFETY: both flyout collections take the default MenuFlyoutItemBase interface.
        unsafe { append(collection, &control.cast::<IMenuItemBase>()?)? };
    }
    Ok(())
}

fn shortcut(shortcut: &NativeShortcut) -> Option<(usize, u8)> {
    if shortcut.meta {
        return None;
    }
    let key = shortcut.key.to_ascii_uppercase();
    let virtual_key = match key.as_str() {
        " " | "SPACE" => 0x20,
        "ENTER" | "\r" => 0x0d,
        "ESCAPE" | "\u{1b}" => 0x1b,
        "BACKSPACE" | "\u{8}" => 0x08,
        "DELETE" | "\u{7f}" => 0x2e,
        _ if key.len() == 1 && key.as_bytes()[0].is_ascii_alphanumeric() => {
            key.as_bytes()[0] as usize
        }
        _ => {
            let function = key.strip_prefix('F')?.parse::<usize>().ok()?;
            if !(1..=24).contains(&function) {
                return None;
            }
            0x6f + function
        }
    };
    let modifiers =
        u8::from(shortcut.ctrl) | (u8::from(shortcut.alt) << 1) | (u8::from(shortcut.shift) << 2);
    Some((virtual_key, modifiers))
}

fn shortcut_label(key: &NativeShortcut) -> String {
    let mut title = String::new();
    for (enabled, name) in [
        (key.ctrl, "Ctrl+"),
        (key.alt, "Alt+"),
        (key.shift, "Shift+"),
    ] {
        if enabled {
            title.push_str(name);
        }
    }
    match key.key.as_str() {
        " " => title.push_str("Space"),
        "\r" => title.push_str("Enter"),
        "\u{1b}" => title.push_str("Esc"),
        "\u{8}" => title.push_str("Backspace"),
        "\u{7f}" => title.push_str("Delete"),
        key => title.push_str(&key.to_ascii_uppercase()),
    }
    title
}

unsafe extern "system" fn subclass(
    hwnd: Raw,
    message: u32,
    wparam: usize,
    lparam: isize,
    _id: usize,
    data: usize,
) -> isize {
    // SAFETY: SetWindowSubclass stores the live, thread-confined state.
    let menu = unsafe { &*(data as *const MenuState) };
    // SAFETY: let SDL update the client rect before reading it.
    let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    if matches!(message, WM_SIZE | WM_DPICHANGED) {
        let _ = menu.resize();
    }
    result
}

pub(super) fn dispatch_shortcut(message: &MSG) -> bool {
    if !matches!(message.message, WM_KEYDOWN | WM_SYSKEYDOWN) || message.lParam.0 & (1 << 30) != 0 {
        return false;
    }
    let Some(menu) = ACTIVE_MENU.with(|slot| slot.borrow().upgrade()) else {
        return false;
    };
    // SAFETY: the SDL message carries the destination HWND. WinUI can direct
    // keys to the island child or to an owned popup instead of the SDL HWND.
    let root = unsafe { GetAncestor(message.hwnd, GA_ROOTOWNER) };
    if message.hwnd != menu.hwnd && root != menu.hwnd {
        return false;
    }
    // SAFETY: GetKeyState reads the message thread's modifier states.
    let modifiers = unsafe {
        u8::from(GetKeyState(0x11) < 0)
            | (u8::from(GetKeyState(0x12) < 0) << 1)
            | (u8::from(GetKeyState(0x10) < 0) << 2)
    };
    let key = (message.wParam.0, modifiers);
    if let Some((_, id)) = menu.shortcuts.iter().find(|(binding, _)| *binding == key) {
        choose(&menu.choices, *id);
        return true;
    }
    false
}

fn choose(choices: &Choices, id: i64) {
    choices.lock().unwrap_or_else(|e| e.into_inner()).push(id);
    Sdl3Shell::wake();
}

#[repr(C)]
struct ClickHandler {
    vtable: &'static ClickVtbl,
    refs: AtomicU32,
    choices: Choices,
    id: i64,
}

#[repr(C)]
struct ClickVtbl {
    base: IUnknown_Vtbl,
    invoke: unsafe extern "system" fn(Raw, Raw, Raw) -> HRESULT,
}

impl ClickHandler {
    const IID: GUID = GUID::from_u128(0xdae23d85_69ca_5bdf_805b_6161a3a215cc);
    const AGILE: GUID = GUID::from_u128(0x94ea2b94_e9cc_49e0_c0ff_ee64ca8f5b90);
    const VTABLE: ClickVtbl = ClickVtbl {
        base: IUnknown_Vtbl {
            QueryInterface: Self::query,
            AddRef: Self::add_ref,
            Release: Self::release,
        },
        invoke: Self::invoke,
    };

    fn new(choices: Choices, id: i64) -> IUnknown {
        let handler = Box::new(Self {
            vtable: &Self::VTABLE,
            refs: AtomicU32::new(1),
            choices,
            id,
        });
        // SAFETY: the first field is a COM vtable and the initial ref is transferred.
        unsafe { IUnknown::from_raw(Box::into_raw(handler).cast()) }
    }

    unsafe extern "system" fn query(this: Raw, iid: *const GUID, output: *mut Raw) -> HRESULT {
        if iid.is_null() || output.is_null() {
            return HRESULT(0x8000_4003_u32 as i32);
        }
        // SAFETY: COM supplies a readable IID and a writable output.
        unsafe {
            if [IUnknown::IID, Self::IID, Self::AGILE].contains(&*iid) {
                Self::add_ref(this);
                write(output, this);
                HRESULT(0)
            } else {
                write(output, null_mut());
                HRESULT(0x8000_4002_u32 as i32)
            }
        }
    }

    unsafe extern "system" fn add_ref(this: Raw) -> u32 {
        unsafe { &*this.cast::<Self>() }
            .refs
            .fetch_add(1, Ordering::Relaxed)
            + 1
    }

    unsafe extern "system" fn release(this: Raw) -> u32 {
        let count = unsafe { &*this.cast::<Self>() }
            .refs
            .fetch_sub(1, Ordering::AcqRel)
            - 1;
        if count == 0 {
            drop(unsafe { Box::from_raw(this.cast::<Self>()) });
        }
        count
    }

    unsafe extern "system" fn invoke(this: Raw, _sender: Raw, _args: Raw) -> HRESULT {
        let handler = unsafe { &*this.cast::<Self>() };
        choose(&handler.choices, handler.id);
        HRESULT(0)
    }
}
