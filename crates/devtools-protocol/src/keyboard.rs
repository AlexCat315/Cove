/// Map a DOM `KeyboardEvent.code` to a USB-HID scancode.
pub fn key_scancode(code: Option<&str>) -> Option<u32> {
    let code = code?;
    if code.is_empty() {
        return None;
    }
    if let Some(letter) = code.strip_prefix("Key")
        && letter.len() == 1
        && let Some(character) = letter.chars().next()
        && character.is_ascii_uppercase()
    {
        return Some(4 + character as u32 - 'A' as u32);
    }
    if let Some(digit) = code.strip_prefix("Digit")
        && let Ok(value) = digit.parse::<u32>()
        && value <= 9
    {
        return Some(if value == 0 { 39 } else { 29 + value });
    }
    if let Some(number) = code.strip_prefix('F')
        && let Ok(value) = number.parse::<u32>()
        && (1..=12).contains(&value)
    {
        return Some(57 + value);
    }
    if let Some(digit) = code.strip_prefix("Numpad")
        && let Ok(value) = digit.parse::<u32>()
        && value <= 9
    {
        return Some(if value == 0 { 98 } else { 88 + value });
    }
    Some(match code {
        "NumpadEnter" => 88,
        "Enter" => 40,
        "Escape" => 41,
        "Backspace" => 42,
        "Tab" => 43,
        "Space" => 44,
        "Minus" => 45,
        "Equal" => 46,
        "BracketLeft" => 47,
        "BracketRight" => 48,
        "Backslash" => 49,
        "Semicolon" => 51,
        "Quote" => 52,
        "Backquote" => 53,
        "Comma" => 54,
        "Period" => 55,
        "Slash" => 56,
        "CapsLock" => 57,
        "PrintScreen" => 70,
        "ScrollLock" => 71,
        "Pause" => 72,
        "Insert" => 73,
        "Home" => 74,
        "PageUp" => 75,
        "Delete" => 76,
        "End" => 77,
        "PageDown" => 78,
        "ArrowRight" => 79,
        "ArrowLeft" => 80,
        "ArrowDown" => 81,
        "ArrowUp" => 82,
        "NumLock" => 83,
        "NumpadDivide" => 84,
        "NumpadMultiply" => 85,
        "NumpadSubtract" => 86,
        "NumpadAdd" => 87,
        "NumpadDecimal" => 99,
        _ => return None,
    })
}
