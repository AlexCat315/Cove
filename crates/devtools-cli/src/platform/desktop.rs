use super::{Action, Capture, Geometry, ListedTarget, Rect, Scope, Target, unavailable};
use cove_devtools_protocol::control::Fault;
use enigo::{Axis, Button, Direction, Enigo, Key, Keyboard, Mouse, Settings};
use image::{RgbaImage, imageops};
use xcap::{Monitor, Window};
mod accessibility;
#[cfg(target_os = "macos")]
mod macos_capture;

fn os(error: impl std::fmt::Display) -> Fault {
    Fault::new("platform_error", error.to_string())
}

fn display_bounds(display: &Monitor) -> Result<Rect, Fault> {
    Ok(Rect {
        x: display.x().map_err(os)?,
        y: display.y().map_err(os)?,
        width: display.width().map_err(os)?,
        height: display.height().map_err(os)?,
    })
}

fn window_bounds(window: &Window) -> Result<Rect, Fault> {
    Ok(Rect {
        x: window.x().map_err(os)?,
        y: window.y().map_err(os)?,
        width: window.width().map_err(os)?,
        height: window.height().map_err(os)?,
    })
}

pub(super) fn list() -> Vec<ListedTarget> {
    let _dpi = Dpi::enter();
    let os = std::env::consts::OS;
    let mut result = Vec::new();
    for window in Window::all().unwrap_or_default() {
        let (Ok(pid), Ok(id), Ok(title)) = (window.pid(), window.id(), window.title()) else {
            continue;
        };
        if !title.is_empty() && !window.is_minimized().unwrap_or(true) {
            result.push(ListedTarget {
                target: Target::Window { pid, id },
                title,
                platform: os.into(),
                native_elements: true,
            });
        }
    }
    for display in Monitor::all().unwrap_or_default() {
        if let Ok(id) = display.id() {
            result.push(ListedTarget {
                target: Target::Display(id),
                title: display
                    .friendly_name()
                    .unwrap_or_else(|_| format!("Display {id}")),
                platform: os.into(),
                native_elements: false,
            });
        }
    }
    result
}

fn window(target: &Target) -> Result<Option<Window>, Fault> {
    let Target::Window { pid, id } = *target else {
        return Ok(None);
    };
    Window::all()
        .map_err(os)?
        .into_iter()
        .find(|window| {
            window.id().ok() == Some(id)
                && window.pid().ok() == Some(pid)
                && !window.is_minimized().unwrap_or(true)
        })
        .map(Some)
        .ok_or_else(|| Fault::new("target_gone", "The selected window is closed or minimized."))
}

fn geometry(
    target: &Target,
    scope: Scope,
    window: Option<&Window>,
    displays: &[Monitor],
) -> Result<Geometry, Fault> {
    let window = window.map(window_bounds).transpose()?;
    let display = match *target {
        Target::Display(id) => displays
            .iter()
            .find(|display| display.id().ok() == Some(id)),
        _ if matches!(scope, Scope::Display) => {
            let window = window.ok_or_else(unavailable)?;
            displays
                .iter()
                .filter_map(|display| {
                    let overlap = window.intersection(display_bounds(display).ok()?)?;
                    Some((
                        u64::from(overlap.width) * u64::from(overlap.height),
                        display,
                    ))
                })
                .max_by_key(|(area, _)| *area)
                .map(|(_, display)| display)
        }
        _ => None,
    };
    let coordinates = if let Some(display) = display {
        display_bounds(display)?
    } else if matches!(scope, Scope::Window) && !matches!(target, Target::Display(_)) {
        window.ok_or_else(unavailable)?
    } else {
        return Err(Fault::new(
            "target_gone",
            "The display is no longer available.",
        ));
    };
    let mut monitors = Vec::new();
    for display in displays {
        let bounds = display_bounds(display)?;
        if coordinates.intersection(bounds).is_some() {
            monitors.push((
                display.id().map_err(os)?,
                bounds,
                display.scale_factor().ok(),
            ));
        }
    }
    monitors.sort_by_key(|(id, _, _)| *id);
    Ok(Geometry {
        coordinates,
        window,
        display: display
            .map(|display| display.id())
            .transpose()
            .map_err(os)?,
        context: Some(format!("{monitors:?}")),
    })
}

pub(super) fn capture(target: &Target, scope: Scope) -> Result<Capture, Fault> {
    let _dpi = Dpi::enter();
    let selected = window(target)?;
    let displays = Monitor::all().map_err(os)?;
    let captured = geometry(target, scope, selected.as_ref(), &displays)?;
    let area = captured.coordinates;
    if area.width == 0
        || area.height == 0
        || u64::from(area.width) * u64::from(area.height) > 40_000_000
    {
        return Err(Fault::new(
            "capture_size",
            "The capture region is empty or exceeds the pixel budget.",
        ));
    }
    let mut pieces = Vec::new();
    for display in displays {
        let bounds = display_bounds(&display)?;
        let Some(overlap) = area.intersection(bounds) else {
            continue;
        };
        let image = capture_monitor(&display, bounds)?;
        let scale_x = f64::from(image.width()) / f64::from(bounds.width);
        let scale_y = f64::from(image.height()) / f64::from(bounds.height);
        let x = (f64::from(overlap.x - bounds.x) * scale_x).round() as u32;
        let y = (f64::from(overlap.y - bounds.y) * scale_y).round() as u32;
        let width = ((f64::from(overlap.width) * scale_x).round() as u32)
            .min(image.width().saturating_sub(x));
        let height = ((f64::from(overlap.height) * scale_y).round() as u32)
            .min(image.height().saturating_sub(y));
        if width > 0 && height > 0 {
            pieces.push((
                overlap,
                imageops::crop_imm(&image, x, y, width, height).to_image(),
            ));
        }
    }
    let pixels = match pieces.as_slice() {
        [] => {
            return Err(Fault::new(
                "not_visible",
                "No display contains any part of the capture region.",
            ));
        }
        [(rect, image)] if *rect == area => image.clone(),
        _ => {
            // Normalize each monitor to desktop coordinates before stitching.
            let mut output = RgbaImage::new(area.width, area.height);
            for (rect, piece) in pieces {
                let piece = imageops::resize(
                    &piece,
                    rect.width,
                    rect.height,
                    imageops::FilterType::Triangle,
                );
                imageops::overlay(
                    &mut output,
                    &piece,
                    i64::from(rect.x - area.x),
                    i64::from(rect.y - area.y),
                );
            }
            output
        }
    };
    if geometry(
        target,
        scope,
        window(target)?.as_ref(),
        &Monitor::all().map_err(os)?,
    )? != captured
    {
        return Err(Fault::new(
            "geometry_changed",
            "The target or display moved during capture.",
        ));
    }
    Ok(Capture {
        pixels,
        geometry: captured,
        source: "system_compositor",
    })
}

#[cfg(not(target_os = "macos"))]
fn capture_monitor(display: &Monitor, _: Rect) -> Result<RgbaImage, Fault> {
    display.capture_image().map_err(os)
}

#[cfg(target_os = "macos")]
fn capture_monitor(_: &Monitor, bounds: Rect) -> Result<RgbaImage, Fault> {
    macos_capture::capture(bounds)
}

pub(super) fn native_elements(target: &Target) -> Result<serde_json::Value, Fault> {
    native_request(target, accessibility::Request::List)
}

pub(super) fn find(target: &Target, using: &str, value: &str) -> Result<serde_json::Value, Fault> {
    native_request(target, accessibility::Request::Find { using, value })
}

fn native_request(
    target: &Target,
    request: accessibility::Request<'_>,
) -> Result<serde_json::Value, Fault> {
    let _dpi = Dpi::enter();
    let Target::Window { pid, id } = *target else {
        return Err(unavailable());
    };
    window(target)?;
    accessibility::execute(pid, id, request)
}

#[cfg(target_os = "linux")]
pub(super) fn process_elements(pid: u32) -> Result<serde_json::Value, Fault> {
    accessibility::execute(pid, 0, accessibility::Request::List)
}

#[cfg(target_os = "linux")]
pub(super) fn find_process_element(
    pid: u32,
    using: &str,
    value: &str,
) -> Result<serde_json::Value, Fault> {
    accessibility::execute(pid, 0, accessibility::Request::Find { using, value })
}

#[cfg(target_os = "linux")]
pub(super) fn activate_process(pid: u32, action: Action) -> Result<&'static str, Fault> {
    let Action::Activate { element } = action else {
        return Err(unavailable());
    };
    accessibility::execute(pid, 0, accessibility::Request::Activate(&element))?;
    Ok("system_accessibility")
}

pub(super) fn perform(
    target: &Target,
    action: Action,
    previous: Option<&Geometry>,
) -> Result<&'static str, Fault> {
    if let Action::Activate { element } = action {
        native_request(target, accessibility::Request::Activate(&element))?;
        return Ok("system_accessibility");
    }
    let _dpi = Dpi::enter();
    let window = window(target)?;
    if let Some(previous) = previous {
        let scope = if previous.display.is_some() {
            Scope::Display
        } else {
            Scope::Window
        };
        let current = geometry(target, scope, window.as_ref(), &Monitor::all().map_err(os)?)?;
        if *previous != current {
            return Err(Fault::new(
                "geometry_changed",
                "The target or display moved. Take a new capture.",
            ));
        }
    }
    if let Some(window) = &window {
        let point = match action {
            Action::Click { x, y, .. }
            | Action::Drag { x, y, .. }
            | Action::Scroll { x, y, .. } => Some((x, y)),
            _ => None,
        };
        if let Some((x, y)) = point {
            let display_scope = previous.is_some_and(|geometry| geometry.display.is_some());
            if !target_at_point(window, x, y, display_scope)? {
                return Err(Fault::new(
                    "target_occluded",
                    "The selected window is not under the captured point.",
                ));
            }
        } else {
            if !is_focused(window)? {
                request_foreground(window)?;
            }
            if !is_focused(window)? {
                return Err(Fault::new(
                    "focus_changed",
                    "The OS did not focus the selected window.",
                ));
            }
        }
    } else if matches!(action, Action::Key { .. } | Action::Text { .. }) {
        return Err(Fault::new(
            "target_required",
            "Select a window or device for keyboard input.",
        ));
    }
    let mut input = Enigo::new(&Settings::default()).map_err(os)?;
    match action {
        Action::Click { x, y, button } => {
            move_pointer(&mut input, x, y)?;
            let button = [Button::Left, Button::Middle, Button::Right][usize::from(button)];
            input.button(button, Direction::Click).map_err(os)?;
        }
        Action::Drag { x, y, to_x, to_y } => {
            move_pointer(&mut input, x, y)?;
            input.button(Button::Left, Direction::Press).map_err(os)?;
            let movement = (1..=8).try_for_each(|step| {
                std::thread::sleep(std::time::Duration::from_millis(16));
                move_pointer(
                    &mut input,
                    x + (to_x - x) * step / 8,
                    y + (to_y - y) * step / 8,
                )
            });
            let release = input.button(Button::Left, Direction::Release).map_err(os);
            movement.and(release)?;
        }
        Action::Scroll { x, y, dx, dy } => {
            move_pointer(&mut input, x, y)?;
            let steps = |delta: i32| delta.signum() * delta.unsigned_abs().div_ceil(40) as i32;
            if dx != 0 {
                input.scroll(steps(dx), Axis::Horizontal).map_err(os)?;
            }
            if dy != 0 {
                input.scroll(steps(dy), Axis::Vertical).map_err(os)?;
            }
        }
        Action::Key { key, modifiers } => {
            let raw = super::keys::letter_or_digit(&key);
            let key = if raw.is_none() {
                Some(key_code(&key)?)
            } else {
                None
            };
            let modifiers = modifiers
                .iter()
                .map(|key| key_code(key))
                .collect::<Result<Vec<_>, _>>()?;
            let mut held = Vec::new();
            let result = modifiers
                .into_iter()
                .try_for_each(|modifier| {
                    input.key(modifier, Direction::Press).map_err(os)?;
                    held.push(modifier);
                    Ok(())
                })
                .and_then(|()| match (raw, key) {
                    (Some(code), _) => input
                        .raw(desktop_raw_code(code), Direction::Click)
                        .map_err(os),
                    (_, Some(key)) => input.key(key, Direction::Click).map_err(os),
                    _ => unreachable!(),
                });
            let mut release = Ok(());
            for modifier in held.into_iter().rev() {
                if let Err(error) = input.key(modifier, Direction::Release) {
                    release = Err(os(error));
                }
            }
            result.and(release)?;
        }
        Action::Text { text } => {
            input.text(&text).map_err(os)?;
            return Ok("system_text");
        }
        Action::Activate { .. } => unreachable!(),
    }
    Ok("system_input")
}

fn key_code(code: &str) -> Result<Key, Fault> {
    Ok(match code {
        "Shift" => Key::Shift,
        "Control" => Key::Control,
        "Alt" => Key::Alt,
        "Meta" => Key::Meta,
        "Enter" => Key::Return,
        "Escape" => Key::Escape,
        "Tab" => Key::Tab,
        "Backspace" => Key::Backspace,
        "Delete" => Key::Delete,
        "ArrowUp" => Key::UpArrow,
        "ArrowDown" => Key::DownArrow,
        "ArrowLeft" => Key::LeftArrow,
        "ArrowRight" => Key::RightArrow,
        "Home" => Key::Home,
        "End" => Key::End,
        "PageUp" => Key::PageUp,
        "PageDown" => Key::PageDown,
        "Space" => Key::Space,
        _ => {
            return Err(Fault::new(
                "unsupported_key",
                "The requested key is not mapped.",
            ));
        }
    })
}

fn desktop_raw_code(code: u16) -> u16 {
    if cfg!(target_os = "linux") {
        code + 8
    } else {
        code
    }
}

fn is_focused(window: &Window) -> Result<bool, Fault> {
    if !window.is_focused().map_err(os)? {
        return Ok(false);
    }
    #[cfg(target_os = "macos")]
    {
        let pid = window.pid().map_err(os)?;
        let top = Window::all()
            .map_err(os)?
            .into_iter()
            .filter(|candidate| {
                candidate.pid().ok() == Some(pid) && !candidate.is_minimized().unwrap_or(true)
            })
            .filter_map(|candidate| Some((candidate.z().ok()?, candidate.id().ok()?)))
            .max_by_key(|(z, _)| *z);
        return Ok(top.is_some_and(|(_, id)| window.id().ok() == Some(id)));
    }
    #[cfg(not(target_os = "macos"))]
    Ok(true)
}

#[cfg(target_os = "windows")]
fn request_foreground(window: &Window) -> Result<(), Fault> {
    let handle = window.id().map_err(os)? as usize as *mut _;
    // SAFETY: the window handle was obtained from the OS enumeration.
    unsafe { windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow(handle) };
    Ok(())
}

#[cfg(target_os = "macos")]
fn request_foreground(window: &Window) -> Result<(), Fault> {
    let pid = window.pid().map_err(os)? as i32;
    objc2::rc::autoreleasepool(|_| {
        // SAFETY: these AppKit methods take an enumerated process ID and activate its application.
        unsafe {
            use objc2::{class, msg_send, rc::Retained, runtime::AnyObject};
            let class = class!(NSRunningApplication);
            let application: Option<Retained<AnyObject>> =
                msg_send![class, runningApplicationWithProcessIdentifier: pid];
            if let Some(application) = application {
                let _: bool = msg_send![&*application, activateWithOptions: 0usize];
            }
        }
    });
    Ok(())
}

#[cfg(target_os = "linux")]
fn request_foreground(_: &Window) -> Result<(), Fault> {
    // The compositor decides which client may activate a window. The caller can
    // focus it explicitly or use a display capture for coordinate operations.
    Ok(())
}

#[cfg(target_os = "windows")]
fn target_at_point(window: &Window, x: i32, y: i32, display_scope: bool) -> Result<bool, Fault> {
    use windows_sys::Win32::{
        Foundation::POINT,
        UI::WindowsAndMessaging::{
            GA_ROOT, GetAncestor, GetWindowThreadProcessId, WindowFromPoint,
        },
    };
    let mut pid = 0;
    // SAFETY: the calls look up a window at a desktop point and fill a valid out pointer.
    let root = unsafe {
        let hit = WindowFromPoint(POINT { x, y });
        GetWindowThreadProcessId(hit, &mut pid);
        GetAncestor(hit, GA_ROOT)
    };
    Ok(if display_scope {
        pid != 0 && pid == window.pid().map_err(os)?
    } else {
        !root.is_null() && root as usize as u32 == window.id().map_err(os)?
    })
}

#[cfg(not(target_os = "windows"))]
fn target_at_point(window: &Window, x: i32, y: i32, display_scope: bool) -> Result<bool, Fault> {
    let pid = window.pid().map_err(os)?;
    let id = window.id().map_err(os)?;
    let top = Window::all()
        .map_err(os)?
        .into_iter()
        .filter(|window| {
            !window.is_minimized().unwrap_or(true)
                && window_bounds(window).is_ok_and(|rect| rect.contains(x, y))
        })
        .filter_map(|window| Some((window.z().ok()?, window.pid().ok()?, window.id().ok()?)))
        .max_by_key(|(z, _, _)| *z);
    Ok(top.is_some_and(|(_, owner, hit)| {
        if display_scope {
            owner == pid
        } else {
            hit == id
        }
    }))
}

#[cfg(target_os = "windows")]
fn move_pointer(_: &mut Enigo, x: i32, y: i32) -> Result<(), Fault> {
    // SAFETY: screen coordinates have already been checked against the selected display.
    if unsafe { windows_sys::Win32::UI::WindowsAndMessaging::SetCursorPos(x, y) } == 0 {
        Err(os(std::io::Error::last_os_error()))
    } else {
        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
fn move_pointer(input: &mut Enigo, x: i32, y: i32) -> Result<(), Fault> {
    input.move_mouse(x, y, enigo::Coordinate::Abs).map_err(os)
}

#[cfg(target_os = "windows")]
struct Dpi(windows_sys::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT);
#[cfg(target_os = "windows")]
impl Dpi {
    fn enter() -> Self {
        use windows_sys::Win32::UI::HiDpi::{
            DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext,
        };
        // SAFETY: the predefined value is a valid DPI awareness context.
        Self(unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) })
    }
}
#[cfg(target_os = "windows")]
impl Drop for Dpi {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: the context was returned for this thread by Windows.
            unsafe { windows_sys::Win32::UI::HiDpi::SetThreadDpiAwarenessContext(self.0) };
        }
    }
}
#[cfg(not(target_os = "windows"))]
struct Dpi;
#[cfg(not(target_os = "windows"))]
impl Dpi {
    fn enter() -> Self {
        Self
    }
}
