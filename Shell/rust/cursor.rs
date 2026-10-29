pub(crate) fn native_id(cursor: cove_platform::SystemCursor) -> i32 {
    use cove_platform::SystemCursor::*;
    match cursor { Arrow => 0, Pointer => 1, Ibeam => 2, Crosshair => 3, Wait => 4, Progress => 5, NotAllowed => 6, Move => 7, ResizeHorizontal => 8, ResizeVertical => 9, ResizeNwse => 10, ResizeNesw => 11 }
}
