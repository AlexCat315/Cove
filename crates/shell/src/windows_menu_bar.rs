//! Win32 menu bar ownership and command delivery on the SDL platform thread.
use crate::{Sdl3Shell, ShellError};
use cove_platform::{NativeMenuBarSnapshot, NativeMenuKind, NativeMenuSnapshot, NativeShortcut};
use std::cell::RefCell;
use std::ffi::c_void;

type Handle = *mut c_void;
type Subclass = unsafe extern "system" fn(Handle, u32, usize, isize, usize, usize) -> isize;

#[link(name = "user32")]
unsafe extern "system" {
    fn CreateMenu() -> Handle;
    fn CreatePopupMenu() -> Handle;
    fn AppendMenuW(menu: Handle, flags: u32, id: usize, text: *const u16) -> i32;
    fn DestroyMenu(menu: Handle) -> i32;
    fn SetMenu(hwnd: Handle, menu: Handle) -> i32;
    fn GetMenu(hwnd: Handle) -> Handle;
    fn DrawMenuBar(hwnd: Handle) -> i32;
    fn GetKeyState(key: i32) -> i16;
}

#[link(name = "comctl32")]
unsafe extern "system" {
    fn SetWindowSubclass(hwnd: Handle, proc: Subclass, id: usize, data: usize) -> i32;
    fn RemoveWindowSubclass(hwnd: Handle, proc: Subclass, id: usize) -> i32;
    fn DefSubclassProc(hwnd: Handle, message: u32, wparam: usize, lparam: isize) -> isize;
}

const COMMAND_BASE: usize = 0x4000;
const MF_POPUP: u32 = 0x10;
const MF_SEPARATOR: u32 = 0x800;
const WM_COMMAND: u32 = 0x0111;
const WM_KEYDOWN: u32 = 0x0100;
const WM_SYSKEYDOWN: u32 = 0x0104;

struct Menu(Handle);

impl Menu {
    fn new(popup: bool) -> Result<Self, ShellError> {
        // SAFETY: no arguments, the call returns one owned menu.
        let menu = unsafe {
            if popup {
                CreatePopupMenu()
            } else {
                CreateMenu()
            }
        };
        if menu.is_null() {
            return Err(last_error("creating a Win32 menu"));
        }
        Ok(Self(menu))
    }
    fn append(&self, flags: u32, id: usize, title: &str) -> Result<(), ShellError> {
        let title: Vec<u16> = title
            .encode_utf16()
            .filter(|ch| *ch != 0)
            .chain([0])
            .collect();
        // SAFETY: the menu is live and Windows copies the NUL-terminated string.
        if unsafe { AppendMenuW(self.0, flags, id, title.as_ptr()) } == 0 {
            return Err(last_error("appending a Win32 menu item"));
        }
        Ok(())
    }
    fn append_popup(&self, title: &str, popup: Menu) -> Result<(), ShellError> {
        self.append(MF_POPUP, popup.0 as usize, &title.replace('&', "&&"))?;
        // The parent now owns and destroys the submenu.
        std::mem::forget(popup);
        Ok(())
    }
}

impl Drop for Menu {
    fn drop(&mut self) {
        // SAFETY: this object owns the menu and all appended popups.
        unsafe { DestroyMenu(self.0) };
    }
}

struct Command {
    id: i64,
    enabled: bool,
    shortcut: Option<(usize, u8)>,
}

#[derive(Default)]
struct Commands {
    items: Vec<Command>,
    choices: RefCell<Vec<i64>>,
}

pub struct WindowsMenuBar {
    hwnd: Handle,
    menu: Menu,
    commands: Box<Commands>,
}

impl WindowsMenuBar {
    pub fn install(hwnd: usize, bar: &NativeMenuBarSnapshot) -> Result<Self, ShellError> {
        let hwnd = hwnd as Handle;
        let menu = Menu::new(false)?;
        let mut commands = Box::<Commands>::default();
        if !bar.application_items.items.is_empty() {
            let application = popup(&bar.application_items, &mut commands.items)?;
            menu.append_popup(&bar.app_name, application)?;
        }
        for branch in &bar.menus {
            let children = popup(&branch.items, &mut commands.items)?;
            menu.append_popup(&branch.title, children)?;
        }
        let this = Self {
            hwnd,
            menu,
            commands,
        };
        // SAFETY: the HWND is owned by the SDL platform thread and the boxed
        // state stays stable until the subclass is removed.
        unsafe {
            if SetWindowSubclass(hwnd, subclass, this.id(), this.id()) == 0 {
                return Err(last_error("installing the Win32 menu command handler"));
            }
            if SetMenu(hwnd, this.menu.0) == 0 {
                return Err(last_error("attaching the Win32 menu bar"));
            }
            DrawMenuBar(hwnd);
        }
        Ok(this)
    }
    fn id(&self) -> usize {
        (&*self.commands as *const Commands) as usize
    }
    pub fn hwnd(&self) -> usize {
        self.hwnd as usize
    }
    pub fn take_choices(&self) -> Vec<i64> {
        self.commands.choices.take()
    }
}

impl Drop for WindowsMenuBar {
    fn drop(&mut self) {
        // SAFETY: the shell drops the bar before destroying its window.
        unsafe {
            RemoveWindowSubclass(self.hwnd, subclass, self.id());
            if GetMenu(self.hwnd) == self.menu.0 {
                SetMenu(self.hwnd, std::ptr::null_mut());
                DrawMenuBar(self.hwnd);
            }
        }
    }
}

fn popup(snapshot: &NativeMenuSnapshot, commands: &mut Vec<Command>) -> Result<Menu, ShellError> {
    let menu = Menu::new(true)?;
    for item in &snapshot.items {
        match &item.kind {
            NativeMenuKind::Separator => menu.append(MF_SEPARATOR, 0, "")?,
            NativeMenuKind::Submenu(children) => {
                menu.append_popup(&item.title, popup(children, commands)?)?;
            }
            NativeMenuKind::Regular => {
                let id = COMMAND_BASE + commands.len();
                if id > u16::MAX as usize {
                    return Err(ShellError::InitializationFailed(
                        "too many Win32 menu items".into(),
                    ));
                }
                let mut title = item.title.replace('&', "&&");
                let shortcut = item.shortcut.as_ref().and_then(shortcut);
                if let (Some(key), Some(_)) = (&item.shortcut, shortcut) {
                    title.push('\t');
                    if key.ctrl {
                        title.push_str("Ctrl+");
                    }
                    if key.alt {
                        title.push_str("Alt+");
                    }
                    if key.shift {
                        title.push_str("Shift+");
                    }
                    title.push_str(&key.key.to_ascii_uppercase());
                }
                let flags = u32::from(!item.enabled) | if item.checked { 8 } else { 0 };
                menu.append(flags, id, &title)?;
                commands.push(Command {
                    id: item.id,
                    enabled: item.enabled,
                    shortcut,
                });
            }
        }
    }
    Ok(menu)
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
    Some((
        virtual_key,
        u8::from(shortcut.ctrl) | (u8::from(shortcut.alt) << 1) | (u8::from(shortcut.shift) << 2),
    ))
}

unsafe extern "system" fn subclass(
    hwnd: Handle,
    message: u32,
    wparam: usize,
    lparam: isize,
    _id: usize,
    data: usize,
) -> isize {
    // SAFETY: SetWindowSubclass stores this live, thread-confined box.
    let commands = unsafe { &*(data as *const Commands) };
    let command = match message {
        WM_COMMAND if lparam == 0 && wparam >> 16 == 0 => (wparam & 0xffff)
            .checked_sub(COMMAND_BASE)
            .and_then(|id| commands.items.get(id)),
        WM_KEYDOWN | WM_SYSKEYDOWN if lparam & (1 << 30) == 0 => {
            // SAFETY: GetKeyState reads modifier states for the calling UI thread.
            let modifiers = unsafe {
                u8::from(GetKeyState(0x11) < 0)
                    | (u8::from(GetKeyState(0x12) < 0) << 1)
                    | (u8::from(GetKeyState(0x10) < 0) << 2)
            };
            commands
                .items
                .iter()
                .find(|item| item.enabled && item.shortcut == Some((wparam, modifiers)))
        }
        _ => None,
    };
    if let Some(command) = command.filter(|command| command.enabled) {
        commands.choices.borrow_mut().push(command.id);
        Sdl3Shell::wake();
        return 0;
    }
    // SAFETY: forward unhandled messages to the next subclass.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

fn last_error(context: &str) -> ShellError {
    ShellError::InitializationFailed(format!("{context}: {}", std::io::Error::last_os_error()))
}
