//! Translate a complete DevTools action into events for a single Cove scene.
use cove_devtools_protocol::control::{Fault, RuntimeInput, Viewport};
use cove_platform::{
    InputEvent, KeyEvent, KeyModifiers, MouseButton, MouseButtonEvent, MouseMotionEvent,
    MouseWheelEvent, ScrollDevice, ScrollInput,
};

pub(crate) fn dispatch(
    action: RuntimeInput,
    captured: Option<Viewport>,
    current: Viewport,
    send: &dyn Fn(InputEvent),
) -> Result<(), Fault> {
    let point = |x, y| {
        if captured != Some(current) {
            return Err(Fault::new(
                "stale_capture",
                "Capture the current viewport before sending pointer input.",
            ));
        }
        if !current.contains(x, y) {
            return Err(invalid("Point is outside the viewport."));
        }
        Ok(())
    };
    let button_event = |x, y, button| MouseButtonEvent {
        x,
        y,
        button,
        clicks: 1,
        ..MouseButtonEvent::default()
    };
    match action {
        RuntimeInput::Click { x, y, button } => {
            point(x, y)?;
            let button = match button {
                0 => MouseButton::Left,
                1 => MouseButton::Middle,
                2 => MouseButton::Right,
                _ => return Err(invalid("Unknown mouse button.")),
            };
            let event = button_event(x, y, button);
            send(InputEvent::MouseButtonDown(event.clone()));
            send(InputEvent::MouseButtonUp(event));
        }
        RuntimeInput::Drag { x, y, to_x, to_y } => {
            point(x, y)?;
            point(to_x, to_y)?;
            send(InputEvent::MouseButtonDown(button_event(
                x,
                y,
                MouseButton::Left,
            )));
            let steps = (((to_x - x).hypot(to_y - y) / 16.0).ceil() as u32).clamp(1, 32);
            let delta_x = (to_x - x) / steps as f32;
            let delta_y = (to_y - y) / steps as f32;
            for step in 1..=steps {
                send(InputEvent::MouseMotion(MouseMotionEvent {
                    x: x + delta_x * step as f32,
                    y: y + delta_y * step as f32,
                    delta_x,
                    delta_y,
                    ..MouseMotionEvent::default()
                }));
            }
            send(InputEvent::MouseButtonUp(button_event(
                to_x,
                to_y,
                MouseButton::Left,
            )));
        }
        RuntimeInput::Scroll { x, y, dx, dy } => {
            point(x, y)?;
            if dx.unsigned_abs() > 10_000 || dy.unsigned_abs() > 10_000 {
                return Err(invalid("Scroll delta exceeds the supported range."));
            }
            send(InputEvent::MouseWheel(MouseWheelEvent {
                x: -(dx as f32),
                y: -(dy as f32),
                mouse_x: Some(x),
                mouse_y: Some(y),
                input: ScrollInput {
                    device: ScrollDevice::Wheel,
                    ..ScrollInput::default()
                },
            }));
        }
        RuntimeInput::Key { key, modifiers } => {
            let event = key_event(&key, &modifiers)?;
            send(InputEvent::KeyDown(event.clone()));
            send(InputEvent::KeyUp(event));
        }
        RuntimeInput::Text { text } if !text.is_empty() && text.len() <= 16_384 => {
            send(InputEvent::TextInput(text));
        }
        RuntimeInput::Text { .. } => return Err(invalid("Invalid text length.")),
    }
    Ok(())
}

fn invalid(message: &str) -> Fault {
    Fault::new("invalid_request", message)
}

fn key_event(key: &str, modifiers: &[String]) -> Result<KeyEvent, Fault> {
    if modifiers.len() > 4 {
        return Err(invalid("Too many key modifiers."));
    }
    let mut held = KeyModifiers::default();
    for modifier in modifiers {
        held = held
            | match modifier.as_str() {
                "Shift" => KeyModifiers::LEFT_SHIFT,
                "Control" => KeyModifiers::LEFT_CTRL,
                "Alt" => KeyModifiers::LEFT_ALT,
                "Meta" => KeyModifiers::LEFT_GUI,
                _ => return Err(invalid("Unknown key modifier.")),
            };
    }
    // Cove uses SDL's USB HID scancodes and SDL keycodes.
    let (scancode, keycode) = match key {
        "Enter" => (40, 13),
        "Escape" => (41, 27),
        "Backspace" => (42, 8),
        "Tab" => (43, 9),
        "Space" | " " => (44, 32),
        "Delete" => (76, 127),
        key => {
            let scancode = match key {
                "Home" => 74,
                "PageUp" => 75,
                "End" => 77,
                "PageDown" => 78,
                "ArrowRight" => 79,
                "ArrowLeft" => 80,
                "ArrowDown" => 81,
                "ArrowUp" => 82,
                "Control" => 224,
                "Shift" => 225,
                "Alt" => 226,
                "Meta" => 227,
                _ => match key.as_bytes() {
                    [letter] if letter.is_ascii_alphabetic() => {
                        let code = u32::from(letter.to_ascii_lowercase());
                        return Ok(KeyEvent {
                            scancode: code - u32::from(b'a') + 4,
                            keycode: code,
                            modifiers: held,
                            is_repeat: false,
                        });
                    }
                    [digit @ b'0'..=b'9'] => {
                        let scancode = if *digit == b'0' {
                            39
                        } else {
                            u32::from(*digit - b'1') + 30
                        };
                        return Ok(KeyEvent {
                            scancode,
                            keycode: u32::from(*digit),
                            modifiers: held,
                            is_repeat: false,
                        });
                    }
                    _ => {
                        return Err(Fault::new(
                            "unsupported_key",
                            "The requested key is not mapped.",
                        ));
                    }
                },
            };
            (scancode, (1 << 30) | scancode)
        }
    };
    Ok(KeyEvent {
        scancode,
        keycode,
        modifiers: held,
        is_repeat: false,
    })
}
