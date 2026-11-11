//! Windows keeps the caption controls and drag behavior for the WinUI menu row.
use super::abi::*;
use super::runtime::{factory, object, take_object};
use crate::WindowAppearance;
use std::cell::Cell;
use std::ptr::null_mut;
use windows::Foundation::PropertyValue;
use windows::Graphics::RectInt32;
use windows::core::{HRESULT, HSTRING, IUnknown, Interface, Result};

const PASSTHROUGH: i32 = 9;
const TALL: i32 = 1;

pub(super) struct TitleBar {
    bar: IAppWindowTitleBar,
    theme: IAppWindowTitleBar3,
    appearance: Cell<Option<WindowAppearance>>,
    clear: IColorReference,
    input: INonClientInput,
    _app: IAppWindow,
}

impl TitleBar {
    pub fn attach(window: WindowId) -> Result<Self> {
        let apps = factory::<IAppWindowStatics>("Microsoft.UI.Windowing.AppWindow")?;
        let inputs =
            factory::<INonClientInputStatics>("Microsoft.UI.Input.InputNonClientPointerSource")?;
        let app =
            window_object(&apps, apps.vtable().get_from_window_id, window)?.cast::<IAppWindow>()?;
        let bar = object(&app, app.vtable().title_bar)?.cast::<IAppWindowTitleBar>()?;
        let height = bar.cast::<IAppWindowTitleBar2>()?;
        let theme = bar.cast::<IAppWindowTitleBar3>()?;
        let clear = transparent_color()?;
        let input = window_object(&inputs, inputs.vtable().get_for_window_id, window)?.cast()?;
        // SAFETY: extend before requesting Windows' taller caption buttons.
        unsafe { (bar.vtable().set_extends)(bar.as_raw(), true).ok()? };
        let title = Self {
            bar,
            theme,
            appearance: Cell::new(None),
            clear,
            input,
            _app: app,
        };
        unsafe { (height.vtable().set_preferred_height)(height.as_raw(), TALL).ok()? };
        Ok(title)
    }

    pub fn metrics(&self) -> Result<(i32, i32, i32)> {
        let (mut height, mut left, mut right) = (0, 0, 0);
        // SAFETY: the caption supplies client coordinates in physical pixels.
        unsafe {
            let table = self.bar.vtable();
            let raw = self.bar.as_raw();
            (table.height)(raw, &mut height).ok()?;
            (table.left_inset)(raw, &mut left).ok()?;
            (table.right_inset)(raw, &mut right).ok()?;
        }
        Ok((height, left, right))
    }

    pub fn set_appearance(&self, appearance: WindowAppearance) -> Result<()> {
        if self.appearance.get() == Some(appearance) {
            return Ok(());
        }
        let theme = match appearance {
            WindowAppearance::Light => 2,
            WindowAppearance::Dark => 3,
        };
        // SAFETY: Windows owns glyphs and interaction colors. The idle button
        // area uses the same window background as the rest of the caption.
        unsafe {
            (self.theme.vtable().set_preferred_theme)(self.theme.as_raw(), theme).ok()?;
            let table = self.bar.vtable();
            let raw = self.bar.as_raw();
            (table.set_button_background)(raw, self.clear.as_raw()).ok()?;
            (table.set_inactive_button_background)(raw, self.clear.as_raw()).ok()?;
        }
        self.appearance.set(Some(appearance));
        Ok(())
    }

    pub fn set_menu_region(&self, bounds: RectInt32) -> Result<()> {
        // SAFETY: the input source copies the rectangle. The caption continues
        // to handle dragging and the system buttons outside this region.
        unsafe {
            let table = self.input.vtable();
            let raw = self.input.as_raw();
            if bounds.Width > 0 && bounds.Height > 0 {
                (table.set_region)(raw, PASSTHROUGH, 1, &bounds).ok()
            } else {
                (table.clear_region)(raw, PASSTHROUGH).ok()
            }
        }
    }
}

impl Drop for TitleBar {
    fn drop(&mut self) {
        // SAFETY: the shell drops the menu before it destroys the SDL HWND.
        unsafe {
            let _ = (self.input.vtable().clear_region)(self.input.as_raw(), PASSTHROUGH);
            let _ = (self.bar.vtable().reset)(self.bar.as_raw());
        }
    }
}

fn window_object(
    factory: &impl Interface,
    get: unsafe extern "system" fn(Raw, WindowId, *mut Raw) -> HRESULT,
    window: WindowId,
) -> Result<IUnknown> {
    let mut raw = null_mut();
    // SAFETY: the method belongs to this factory and returns one reference.
    unsafe { get(factory.as_raw(), window, &mut raw).ok()? };
    take_object(raw)
}

fn transparent_color() -> Result<IColorReference> {
    let helper = factory::<IXamlBindingHelper>("Microsoft.UI.Xaml.Markup.XamlBindingHelper")?;
    let value = PropertyValue::CreateString(&HSTRING::from("#00000000"))?;
    let name = HSTRING::from("Windows.UI.Color");
    let mut result = null_mut();
    // SAFETY: TypeName contains a borrowed HSTRING and the metadata kind.
    unsafe {
        let ty = TypeName {
            name: std::mem::transmute_copy(&name),
            kind: 1,
        };
        (helper.vtable().convert_value)(helper.as_raw(), ty, value.as_raw(), &mut result).ok()?;
    }
    take_object(result)?.cast()
}
