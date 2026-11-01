//! Verify the Rust ABI against the linked library, including after upgrades.
use super::*;
use std::ffi::{CStr, c_char};
unsafe extern "C" {
    fn ghostty_type_json() -> *const c_char;
}

#[test]
fn rust_layouts_match_the_linked_library_manifest() {
    // SAFETY: libghostty-vt returns a static, NUL-terminated manifest string.
    let json = unsafe { CStr::from_ptr(ghostty_type_json()) };
    let manifest: serde_json::Value =
        serde_json::from_slice(json.to_bytes()).expect("ABI manifest");
    let types = &manifest["types"];
    macro_rules! check {
        ($ty:ty $(, $field:ident)* $(,)?) => {{
            let name = stringify!($ty);
            let info = &types[name];
            assert_eq!(info["size"].as_u64(), Some(size_of::<$ty>() as u64), "{name} size");
            assert_eq!(info["align"].as_u64(), Some(align_of::<$ty>() as u64), "{name} alignment");
            $(assert_eq!(info["fields"][stringify!($field)]["offset"].as_u64(),
                Some(std::mem::offset_of!($ty, $field) as u64), "{name}.{} offset", stringify!($field));)*
        }};
    }
    check!(GhosttyString, ptr, len);
    check!(GhosttyColorRgb, r, g, b);
    check!(GhosttyBuffer, ptr, cap, len);
    check!(GhosttyStyleColorValue);
    check!(GhosttyStyleColor, tag, value);
    check!(
        GhosttyStyle,
        size,
        fg_color,
        bg_color,
        underline_color,
        bold,
        italic,
        faint,
        blink,
        inverse,
        invisible,
        strikethrough,
        overline,
        underline
    );
    check!(
        GhosttyRenderStateColors,
        size,
        background,
        foreground,
        cursor,
        cursor_has_value,
        palette
    );
    check!(
        GhosttyRenderStateCursor,
        size,
        viewport_has_value,
        viewport_x,
        viewport_y,
        wide_tail,
        visible,
        blinking,
        password_input,
        visual_style
    );
    check!(GhosttyTerminalScrollbar, total, offset, len);
    check!(GhosttyTerminalModeConfig, mode, value);
    check!(GhosttyGridRef, size, node, x, y);
    check!(GhosttySelection, size, start, end, rectangle);
    check!(GhosttyPointCoordinate, x, y);
    check!(GhosttyPointValue, coordinate);
    check!(GhosttyPoint, tag, value);
    check!(GhosttyTerminalScrollViewportValue, delta);
    check!(GhosttyTerminalScrollViewport, tag, value);
    check!(GhosttyRenderStateRowSelection, size, start_x, end_x);
    check!(
        GhosttyKittyGraphicsPlacementRenderInfo,
        size,
        pixel_width,
        pixel_height,
        grid_cols,
        grid_rows,
        viewport_col,
        viewport_row,
        viewport_visible,
        source_x,
        source_y,
        source_width,
        source_height
    );
    check!(
        GhosttyTerminalSelectionFormatOptions,
        size,
        emit,
        unwrap,
        trim,
        selection
    );
    check!(
        GhosttySelectionGestureGeometry,
        columns,
        cell_width,
        padding_left,
        screen_height
    );
    check!(GhosttySurfacePosition, x, y);
    check!(
        GhosttyMouseEncoderSize,
        size,
        screen_width,
        screen_height,
        cell_width,
        cell_height,
        padding_top,
        padding_bottom,
        padding_right,
        padding_left
    );
    check!(GhosttyMousePosition, x, y);
    check!(GhosttySysImage, width, height, data, data_len);
}
