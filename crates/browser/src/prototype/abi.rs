//! Translation of the `@_cdecl` exports at the bottom of
//! `Browser/Sources/CoveUIBrowserPrototype/main.swift`.
//!
//! These are the functions the JavaScript host calls. Names, argument order and
//! units are the ABI contract and must not change. Pointer-returning functions
//! return the buffer the corresponding call last published; the bytes stay
//! valid until the next mutation of that buffer.

use std::os::raw::c_char;

use crate::export_buffer::{self, ExportBuffer};
use crate::prototype::with_app;

/// Publishes the frame for `width`×`height` logical points.
#[unsafe(no_mangle)]
pub extern "C" fn cove_render(width: f32, height: f32) -> i32 {
    #[cfg(target_arch = "wasm32")]
    return with_app(|app| app.render_web(width, height));
    #[cfg(not(target_arch = "wasm32"))]
    {
        with_app(|app| app.render(width, height));
        0
    }
}

#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn cove_webgpu_enable(bgra: bool) -> bool {
    use cove_graphics::TextureFormat;
    with_app(|app| {
        app.enable_webgpu(if bgra {
            TextureFormat::Bgra8Unorm
        } else {
            TextureFormat::Rgba8Unorm
        })
    })
}

#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn cove_webgpu_disable() {
    with_app(|app| app.disable_webgpu());
}

#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn cove_webgpu_complete(token: u32, measured: u32, nanos: u64) {
    cove_graphics::webgpu::complete(token, (measured != 0).then_some(nanos));
}

#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn cove_webgpu_pass_timing(token: u32, index: u32, nanos: u64) {
    cove_graphics::webgpu::pass_timing(token, index, nanos);
}

/// Whether animations or queued composition need another animation frame.
#[unsafe(no_mangle)]
pub extern "C" fn cove_needs_frame() -> i32 {
    with_app(|app| i32::from(app.needs_frame()))
}

/// Sets the device pixel ratio used for glyph rasterisation (clamped 1…4).
#[unsafe(no_mangle)]
pub extern "C" fn cove_set_scale(scale: f32) {
    if scale.is_finite() {
        with_app(|app| app.set_raster_scale(scale));
    }
}

/// Reports the canvas's top-left in the page viewport, in CSS pixels.
#[unsafe(no_mangle)]
pub extern "C" fn cove_set_content_origin(x: f32, y: f32) {
    with_app(|app| app.set_content_origin(x, y));
}

/// Allocates a transfer buffer the host fills with font bytes.
///
/// # Safety
/// The caller owns the allocation and must pass the same pointer and length to
/// [`cove_font_load`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cove_font_alloc(count: i32) -> *mut u8 {
    match usize::try_from(count) {
        // SAFETY: validated size; ownership transfers to the caller per the contract.
        Ok(count) => unsafe { export_buffer::allocate_export_allocation(count) },
        Err(_) => std::ptr::null_mut(),
    }
}

/// Loads font bytes previously written into a [`cove_font_alloc`] buffer.
/// Returns the new font id, or 0 when the font is rejected.
///
/// # Safety
/// `pointer` must come from [`cove_font_alloc`] with at least `count` readable
/// bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cove_font_load(pointer: *const u8, count: i32) -> i32 {
    let Ok(count) = usize::try_from(count) else {
        return 0;
    };
    if pointer.is_null() || count == 0 || count > 32 * 1024 * 1024 {
        return 0;
    }
    // SAFETY: the caller guarantees `count` readable bytes at `pointer`.
    let bytes = unsafe { std::slice::from_raw_parts(pointer, count) };
    with_app(|app| app.append_font(bytes).unwrap_or(0) as i32)
}

#[unsafe(no_mangle)]
pub extern "C" fn cove_atlas() -> *mut u8 {
    with_app(|app| app.atlas_buffer.pointer())
}

#[unsafe(no_mangle)]
pub extern "C" fn cove_atlas_size() -> i32 {
    with_app(|app| app.atlas_buffer.count() as i32)
}

/// RGBA pixels retained by the most recently rendered draw list.
#[unsafe(no_mangle)]
pub extern "C" fn cove_raster_asset(id: u32) -> *const u8 {
    with_app(|app| app.raster_asset_pixels(id))
}

/// Dispatches a synthetic pointer event. `kind`: 0 move, 1 down, 2 up, 3 click.
#[unsafe(no_mangle)]
pub extern "C" fn cove_pointer(kind: i32, x: f32, y: f32) {
    with_app(|app| app.pointer(kind, x, y));
}

/// Dispatches DOM wheel deltas in pixels, lines or pages (modes 0, 1 and 2).
#[unsafe(no_mangle)]
pub extern "C" fn cove_wheel(mode: u32, dx: f32, dy: f32, x: f32, y: f32, time: f64) {
    use cove_platform::ScrollUnit;
    let unit = match mode {
        0 => ScrollUnit::Points,
        1 => ScrollUnit::Lines,
        2 => ScrollUnit::Pages,
        _ => return,
    };
    with_app(|app| app.wheel(unit, dx, dy, x, y, time));
}

/// Returns three floats (caret x, top y and line height) in surface points,
/// or null if the focused node does not accept text. Read before the next call.
#[unsafe(no_mangle)]
pub extern "C" fn cove_text_input_area() -> *const f32 {
    with_app(|app| app.text_input_area())
}

/// Dispatches a synthetic key event.
#[unsafe(no_mangle)]
pub extern "C" fn cove_key(code: u32, modifiers: u32, down: i32, repeated: i32) {
    with_app(|app| app.key(code, modifiers as u16, down != 0, repeated != 0));
}

/// Dispatches text input (`editing != 0`) or a committed text event.
///
/// # Safety
/// `pointer` must be readable for `count` bytes of UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cove_text(pointer: *const c_char, count: i32, editing: i32) {
    let Ok(count) = usize::try_from(count) else {
        return;
    };
    if pointer.is_null() || (count == 0 && editing == 0) || count > 32768 {
        return;
    }
    // SAFETY: the caller guarantees `count` readable UTF-8 bytes.
    let bytes = unsafe { std::slice::from_raw_parts(pointer.cast::<u8>(), count) };
    let Ok(text) = std::str::from_utf8(bytes) else {
        return;
    };
    with_app(|app| app.text(text, editing != 0));
}

#[unsafe(no_mangle)]
pub extern "C" fn cove_vertices() -> *mut u8 {
    with_app(|app| app.vertex_buffer.pointer())
}

#[unsafe(no_mangle)]
pub extern "C" fn cove_vertex_bytes() -> i32 {
    with_app(|app| app.vertex_buffer.count() as i32)
}

#[unsafe(no_mangle)]
pub extern "C" fn cove_indices() -> *mut u8 {
    with_app(|app| app.index_buffer.pointer())
}

/// Index count, not byte count: the host uploads `u32` indices.
#[unsafe(no_mangle)]
pub extern "C" fn cove_index_count() -> i32 {
    with_app(|app| (app.index_buffer.count() / std::mem::size_of::<u32>()) as i32)
}

#[unsafe(no_mangle)]
pub extern "C" fn cove_snapshot() -> *mut u8 {
    with_app(|app| app.snapshot_buffer.pointer())
}

#[unsafe(no_mangle)]
pub extern "C" fn cove_snapshot_size() -> i32 {
    with_app(|app| app.snapshot_buffer.count() as i32)
}

#[unsafe(no_mangle)]
pub extern "C" fn cove_response() -> *mut u8 {
    with_app(|app| app.response_buffer.pointer())
}

#[unsafe(no_mangle)]
pub extern "C" fn cove_response_size() -> i32 {
    with_app(|app| app.response_buffer.count() as i32)
}

#[unsafe(no_mangle)]
pub extern "C" fn cove_probe_register() {
    with_app(|app| app.register_probe());
}

#[unsafe(no_mangle)]
pub extern "C" fn cove_probe_disconnect() {
    with_app(|app| app.disconnect_probe());
}

#[unsafe(no_mangle)]
pub extern "C" fn cove_counter_action(action: u32) {
    with_app(|app| app.counter_action(action));
}

/// Allocates a transfer buffer for one DevTools request.
///
/// # Safety
/// The caller owns the allocation and must release it with [`cove_free`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cove_alloc(count: i32) -> *mut u8 {
    match usize::try_from(count) {
        Ok(count) if count <= 1024 * 1024 => unsafe {
            // SAFETY: size was validated; ownership transfers to the caller.
            export_buffer::allocate_export_allocation(count)
        },
        _ => std::ptr::null_mut(),
    }
}

/// Releases a buffer from [`cove_alloc`].
///
/// # Safety
/// `pointer` and `count` must describe one live [`cove_alloc`] allocation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cove_free(pointer: *mut u8, count: i32) {
    let Ok(count) = usize::try_from(count) else {
        return;
    };
    // SAFETY: the caller's contract guarantees a live allocation of `count` bytes.
    unsafe { export_buffer::free_export_allocation(pointer, count) };
}

/// Handles one DevTools request written into a [`cove_alloc`] buffer.
///
/// # Safety
/// `pointer` must be readable for `count` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cove_dispatch(pointer: *const u8, count: i32) {
    let Ok(count) = usize::try_from(count) else {
        return;
    };
    if pointer.is_null() || count == 0 || count > 1024 * 1024 {
        return;
    }
    // SAFETY: the caller guarantees `count` readable bytes at `pointer`.
    let bytes = unsafe { std::slice::from_raw_parts(pointer, count) };
    with_app(|app| app.dispatch(bytes));
}

/// Handles one DevTools request from bytes the host already owns.
pub fn dispatch_bytes(bytes: &[u8]) {
    with_app(|app| app.dispatch(bytes));
}

/// Re-exported so hosts linking this crate as a library can build their own
/// buffers around [`cove_render`].
pub type Buffer = ExportBuffer;

/// Publishes a frame for hosts that link this crate as a library instead of
/// going through the C ABI.
pub fn render(width: f32, height: f32) {
    with_app(|app| app.render(width, height));
}
