use super::{Action, Capture, Geometry, ListedTarget, Rect, Scope, Target};
use base64::{Engine, engine::general_purpose::STANDARD};
use cove_devtools_protocol::control::Fault;
use serde_json::{Value, json};
use std::sync::Mutex;
use std::time::Duration;

pub(super) struct Driver {
    endpoint: String,
    session: String,
    platform: String,
    agent: ureq::Agent,
    window: Mutex<Option<String>>,
}

impl Driver {
    pub(super) fn parse(config: String) -> Result<Self, Fault> {
        let parts: Vec<_> = config.split(',').collect();
        let [platform, endpoint, session] = parts.as_slice() else {
            return Err(Fault::new(
                "invalid_driver",
                "Use platform,http://127.0.0.1:port,session-id.",
            ));
        };
        if !matches!(*platform, "web" | "ios" | "android")
            || !crate::local_http(endpoint)
            || !identifier(session)
        {
            return Err(Fault::new(
                "invalid_driver",
                "Invalid WebDriver platform, local endpoint or session ID.",
            ));
        }
        Ok(Self {
            endpoint: endpoint.trim_end_matches('/').to_string(),
            session: session.to_string(),
            platform: platform.to_string(),
            agent: ureq::Agent::config_builder()
                .proxy(None)
                .max_redirects(0)
                .timeout_global(Some(Duration::from_secs(10)))
                .http_status_as_error(false)
                .build()
                .into(),
            window: Mutex::new(None),
        })
    }

    fn web_window(&self) -> Result<String, Fault> {
        let mut expected = self.window.lock().unwrap();
        let value = self.request("/window", None)?;
        let handle = value
            .as_str()
            .filter(|handle| !handle.is_empty())
            .ok_or_else(|| {
                Fault::new("target_gone", "The browser session has no current window.")
            })?;
        if expected
            .as_deref()
            .is_some_and(|expected| expected != handle)
        {
            return Err(Fault::new(
                "target_changed",
                "Restore the browser session's selected tab.",
            ));
        }
        expected.get_or_insert_with(|| handle.to_owned());
        Ok(handle.to_owned())
    }

    fn check_window(&self) -> Result<(), Fault> {
        if self.platform == "web" {
            self.web_window()?;
        }
        Ok(())
    }

    pub(super) fn target(&self, index: usize) -> ListedTarget {
        ListedTarget {
            target: Target::Driver(index),
            title: format!("{} · WebDriver {index}", self.platform),
            platform: self.platform.clone(),
            native_elements: true,
        }
    }

    fn request(&self, command: &str, body: Option<Value>) -> Result<Value, Fault> {
        let url = format!("{}/session/{}{}", self.endpoint, self.session, command);
        let mut response = if let Some(body) = body {
            self.agent.post(&url).send_json(body)
        } else {
            self.agent.get(&url).call()
        }
        .map_err(|_| {
            Fault::new(
                "driver_unreachable",
                "Cannot reach the selected WebDriver session.",
            )
        })?;
        let status = response.status();
        let data: Value = response
            .body_mut()
            .with_config()
            .limit(48 * 1024 * 1024)
            .read_json()
            .map_err(|_| {
                Fault::new(
                    "driver_response",
                    "Invalid or oversized WebDriver response.",
                )
            })?;
        if !status.is_success() {
            return Err(Fault::new(
                data["value"]["error"].as_str().unwrap_or("driver_error"),
                data["value"]["message"]
                    .as_str()
                    .unwrap_or("WebDriver rejected the operation.")
                    .chars()
                    .take(512)
                    .collect::<String>(),
            ));
        }
        Ok(data.get("value").cloned().unwrap_or(Value::Null))
    }

    fn geometry(&self) -> Result<Geometry, Fault> {
        let (dimensions, context) =
            if self.platform == "web" {
                let window = self.web_window()?;
                let size = self.request("/execute/sync", Some(json!({
                "script": "return {width: innerWidth, height: innerHeight, url: location.href}",
                "args": [],
            })))?;
                let context = format!("{window}:{}", size["url"].as_str().unwrap_or_default());
                (size, context)
            } else {
                (self.request("/window/rect", None)?, self.session.clone())
            };
        let dimension = |key: &str| -> Result<u32, Fault> {
            dimensions[key]
                .as_u64()
                .filter(|value| (1..=16384).contains(value))
                .map(|value| value as u32)
                .ok_or_else(|| {
                    Fault::new(
                        "driver_geometry",
                        "WebDriver did not report a usable viewport.",
                    )
                })
        };
        Ok(Geometry {
            coordinates: Rect {
                x: 0,
                y: 0,
                width: dimension("width")?,
                height: dimension("height")?,
            },
            window: None,
            display: None,
            context: Some(context),
        })
    }

    pub(super) fn capture(&self, scope: Scope) -> Result<Capture, Fault> {
        if self.platform == "web" && matches!(scope, Scope::Display) {
            return Err(Fault::new(
                "unavailable",
                "Use a desktop display target for system chrome and IME candidates.",
            ));
        }
        let geometry = self.geometry()?;
        let data = self.request("/screenshot", None)?;
        if self.geometry()? != geometry {
            return Err(Fault::new(
                "geometry_changed",
                "The WebDriver target changed during capture.",
            ));
        }
        let bytes = STANDARD
            .decode(data.as_str().unwrap_or_default())
            .map_err(|_| Fault::new("driver_response", "WebDriver did not return a screenshot."))?;
        let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|error| Fault::new("driver_response", error.to_string()))?;
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(160 * 1024 * 1024);
        limits.max_image_width = Some(16384);
        limits.max_image_height = Some(16384);
        reader.limits(limits);
        let pixels = reader
            .decode()
            .map_err(|error| Fault::new("driver_response", error.to_string()))?
            .into_rgba8();
        Ok(Capture {
            pixels,
            geometry,
            source: if self.platform == "web" {
                "browser_viewport"
            } else {
                "device_screenshot"
            },
        })
    }

    pub(super) fn source(&self) -> Result<Value, Fault> {
        self.check_window()?;
        let source = self.request("/source", None)?;
        if source
            .as_str()
            .is_some_and(|value| value.len() > 384 * 1024)
        {
            return Err(Fault::new(
                "result_too_large",
                "The platform tree exceeds the response budget. Query an element instead.",
            ));
        }
        Ok(json!({"source": source, "backend": "webdriver"}))
    }

    pub(super) fn find(&self, using: &str, value: &str) -> Result<Value, Fault> {
        if value.is_empty()
            || value.len() > 2048
            || !matches!(
                using,
                "css selector"
                    | "xpath"
                    | "accessibility id"
                    | "id"
                    | "class name"
                    | "tag name"
                    | "link text"
                    | "partial link text"
            )
        {
            return Err(Fault::new(
                "invalid_selector",
                "Unsupported WebDriver selector.",
            ));
        }
        self.check_window()?;
        let elements = self.request("/elements", Some(json!({"using": using, "value": value})))?;
        let [element] = elements.as_array().map(Vec::as_slice).unwrap_or_default() else {
            return Err(Fault::new(
                "ambiguous_target",
                "The native selector must match exactly one element.",
            ));
        };
        let element = element[ELEMENT]
            .as_str()
            .filter(|element| identifier(element))
            .ok_or_else(|| {
                Fault::new(
                    "driver_response",
                    "WebDriver did not return a valid element ID.",
                )
            })?;
        Ok(json!({"element": element}))
    }

    pub(super) fn perform(
        &self,
        action: Action,
        geometry: Option<&Geometry>,
    ) -> Result<&'static str, Fault> {
        if let Some(previous) = geometry {
            if self.geometry()? != *previous {
                return Err(Fault::new(
                    "geometry_changed",
                    "The WebDriver target changed. Take a new capture.",
                ));
            }
        } else {
            self.check_window()?;
        }
        let pointer = |actions| json!({"type":"pointer", "id":"cove-pointer", "parameters":{"pointerType": if self.platform == "web" {"mouse"} else {"touch"}}, "actions":actions});
        let position = |x, y, duration| json!({"type":"pointerMove","origin":"viewport","x":x,"y":y,"duration":duration});
        let actions = match action {
            Action::Click { x, y, button } => pointer(vec![
                position(x, y, 0),
                json!({"type":"pointerDown","button":button}),
                json!({"type":"pointerUp","button":button}),
            ]),
            Action::Drag { x, y, to_x, to_y } => pointer(vec![
                position(x, y, 0),
                json!({"type":"pointerDown","button":0}),
                position(to_x, to_y, 250),
                json!({"type":"pointerUp","button":0}),
            ]),
            Action::Scroll { x, y, dx, dy } if self.platform == "web" => json!({
                "type":"wheel","id":"cove-wheel","actions":[{"type":"scroll","origin":"viewport","x":x,"y":y,"deltaX":dx,"deltaY":dy,"duration":180}]
            }),
            Action::Scroll { x, y, dx, dy } => {
                let bounds = geometry
                    .ok_or_else(|| Fault::new("invalid_request", "Scroll requires a capture."))?
                    .coordinates;
                let to_x = (x - dx).clamp(0, bounds.width as i32 - 1);
                let to_y = (y - dy).clamp(0, bounds.height as i32 - 1);
                pointer(vec![
                    position(x, y, 0),
                    json!({"type":"pointerDown","button":0}),
                    position(to_x, to_y, 250),
                    json!({"type":"pointerUp","button":0}),
                ])
            }
            Action::Key { key, modifiers } => {
                let keys: Vec<_> = modifiers
                    .iter()
                    .chain(std::iter::once(&key))
                    .map(|key| web_key(key))
                    .collect::<Result<_, _>>()?;
                let actions = keys
                    .iter()
                    .map(|value| json!({"type":"keyDown","value":value}))
                    .chain(
                        keys.iter()
                            .rev()
                            .map(|value| json!({"type":"keyUp","value":value})),
                    )
                    .collect::<Vec<_>>();
                json!({"type":"key","id":"cove-keyboard","actions":actions})
            }
            Action::Text { text } => {
                let active = self.request("/element/active", None)?;
                let id = active[ELEMENT]
                    .as_str()
                    .filter(|id| identifier(id))
                    .ok_or_else(|| {
                        Fault::new("focus_changed", "WebDriver has no focused element.")
                    })?;
                self.request(&format!("/element/{id}/value"), Some(json!({"text": text})))?;
                return Ok("webdriver_text");
            }
            Action::Activate { element } => {
                if !identifier(&element) {
                    return Err(Fault::new(
                        "invalid_request",
                        "Invalid WebDriver element ID.",
                    ));
                }
                self.request(&format!("/element/{element}/click"), Some(json!({})))?;
                return Ok("webdriver_element");
            }
        };
        let result = self.request("/actions", Some(json!({"actions":[actions]})));
        if result.is_err() {
            let _ = self
                .agent
                .delete(&format!(
                    "{}/session/{}/actions",
                    self.endpoint, self.session
                ))
                .call();
        }
        result.map(|_| {
            if self.platform == "web" {
                "browser_input"
            } else {
                "device_input"
            }
        })
    }
}

const ELEMENT: &str = "element-6066-11e4-a52e-4f735466cecf";

fn identifier(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 256
        && !matches!(id, "." | "..")
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
}

#[cfg(test)]
mod tests {
    #[test]
    fn webdriver_element_ids_remain_single_path_segments() {
        assert!(super::identifier("f.81C2.d.49AA.e.3"));
        for id in [".", "..", "a/b", "a\\b", "a%2fb", "a?b"] {
            assert!(!super::identifier(id));
        }
    }
}

fn web_key(code: &str) -> Result<String, Fault> {
    let key = match code {
        "Backspace" => "\u{e003}",
        "Tab" => "\u{e004}",
        "Enter" => "\u{e007}",
        "Shift" => "\u{e008}",
        "Control" => "\u{e009}",
        "Alt" => "\u{e00a}",
        "Escape" => "\u{e00c}",
        "Space" => " ",
        "PageUp" => "\u{e00e}",
        "PageDown" => "\u{e00f}",
        "End" => "\u{e010}",
        "Home" => "\u{e011}",
        "ArrowLeft" => "\u{e012}",
        "ArrowUp" => "\u{e013}",
        "ArrowRight" => "\u{e014}",
        "ArrowDown" => "\u{e015}",
        "Delete" => "\u{e017}",
        "Meta" => "\u{e03d}",
        _ => {
            let key = code
                .strip_prefix("Key")
                .or_else(|| code.strip_prefix("Digit"))
                .unwrap_or(code);
            if key.len() == 1 && key.as_bytes()[0].is_ascii_alphanumeric() {
                return Ok(key.to_ascii_lowercase());
            }
            return Err(Fault::new(
                "unsupported_key",
                "The requested key is not mapped.",
            ));
        }
    };
    Ok(key.to_owned())
}
