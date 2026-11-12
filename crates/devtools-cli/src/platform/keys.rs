/// Physical positions named by the standard KeyA/ Digit0 codes.
pub(super) fn letter_or_digit(code: &str) -> Option<u16> {
    let code = code
        .strip_prefix("Key")
        .or_else(|| code.strip_prefix("Digit"))
        .unwrap_or(code);
    let [byte] = *code.as_bytes() else {
        return None;
    };
    let byte = byte.to_ascii_uppercase();
    let index = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
        .iter()
        .position(|&key| key == byte)?;
    #[cfg(target_os = "macos")]
    let codes = [
        0, 11, 8, 2, 14, 3, 5, 4, 34, 38, 40, 37, 46, 45, 31, 35, 12, 15, 1, 17, 32, 9, 13, 7, 16,
        6, 29, 18, 19, 20, 21, 23, 22, 26, 28, 25,
    ];
    #[cfg(not(target_os = "macos"))]
    let codes = [
        30, 48, 46, 32, 18, 33, 34, 35, 23, 36, 37, 38, 50, 49, 24, 25, 16, 19, 31, 20, 22, 47, 17,
        45, 21, 44, 11, 2, 3, 4, 5, 6, 7, 8, 9, 10,
    ];
    Some(codes[index])
}
