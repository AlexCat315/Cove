//! Common controller interface for desktop and WebDriver targets.
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
mod desktop;
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
mod keys;
#[cfg(target_os = "linux")]
mod wayland;
mod webdriver;

use cove_devtools_protocol::control::Fault;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn contains(self, x: i32, y: i32) -> bool {
        let dx = i64::from(x) - i64::from(self.x);
        let dy = i64::from(y) - i64::from(self.y);
        dx >= 0 && dy >= 0 && dx < i64::from(self.width) && dy < i64::from(self.height)
    }

    pub fn intersection(self, other: Self) -> Option<Self> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = (i64::from(self.x) + i64::from(self.width))
            .min(i64::from(other.x) + i64::from(other.width));
        let bottom = (i64::from(self.y) + i64::from(self.height))
            .min(i64::from(other.y) + i64::from(other.height));
        let width = u32::try_from(right - i64::from(x)).ok()?;
        let height = u32::try_from(bottom - i64::from(y)).ok()?;
        (width > 0 && height > 0).then_some(Self {
            x,
            y,
            width,
            height,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Target {
    Window {
        pid: u32,
        id: u32,
    },
    Display(u32),
    Driver(usize),
    #[cfg(target_os = "linux")]
    Wayland,
    #[cfg(target_os = "linux")]
    AccessibleProcess(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Geometry {
    pub coordinates: Rect,
    pub window: Option<Rect>,
    pub display: Option<u32>,
    pub context: Option<String>,
}

pub struct Capture {
    pub pixels: image::RgbaImage,
    pub geometry: Geometry,
    pub source: &'static str,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    #[default]
    Window,
    Display,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Action {
    Click {
        x: i32,
        y: i32,
        #[serde(default)]
        button: u8,
    },
    Drag {
        x: i32,
        y: i32,
        to_x: i32,
        to_y: i32,
    },
    Scroll {
        x: i32,
        y: i32,
        dx: i32,
        dy: i32,
    },
    Key {
        key: String,
        #[serde(default)]
        modifiers: Vec<String>,
    },
    Text {
        text: String,
    },
    Activate {
        element: String,
    },
}

impl Action {
    pub fn validate(&self, geometry: Option<&Geometry>) -> Result<(), Fault> {
        let invalid = |message| Fault::new("invalid_request", message);
        let inside = |x, y| {
            let bounds = geometry
                .ok_or_else(|| invalid("Pointer operations require a capture ID."))?
                .coordinates;
            if bounds.contains(x, y) {
                Ok(())
            } else {
                Err(invalid("Point is outside the captured area."))
            }
        };
        match self {
            Self::Click { x, y, button } if *button <= 2 => inside(*x, *y),
            Self::Drag { x, y, to_x, to_y } => {
                inside(*x, *y)?;
                inside(*to_x, *to_y)
            }
            Self::Scroll { x, y, dx, dy }
                if dx.unsigned_abs() <= 10000 && dy.unsigned_abs() <= 10000 =>
            {
                inside(*x, *y)
            }
            Self::Key { key, modifiers }
                if key.len() <= 32
                    && !key.is_empty()
                    && modifiers.len() <= 4
                    && modifiers.iter().all(|key| {
                        matches!(key.as_str(), "Shift" | "Control" | "Alt" | "Meta")
                    }) =>
            {
                Ok(())
            }
            Self::Text { text } if !text.is_empty() && text.len() <= 16_384 => Ok(()),
            Self::Activate { element } if !element.is_empty() && element.len() <= 256 => Ok(()),
            _ => Err(invalid("Invalid input action.")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ListedTarget {
    pub target: Target,
    pub title: String,
    pub platform: String,
    pub native_elements: bool,
}

impl ListedTarget {
    pub fn id(&self) -> String {
        match self.target {
            Target::Window { pid, id } => format!("window:{pid}:{id}"),
            Target::Display(id) => format!("display:{id}"),
            Target::Driver(index) => format!("driver:{index}"),
            #[cfg(target_os = "linux")]
            Target::Wayland => "wayland:portal".into(),
            #[cfg(target_os = "linux")]
            Target::AccessibleProcess(pid) => format!("accessibility:{pid}"),
        }
    }

    pub fn describe(&self) -> Value {
        let capabilities = if self.native_elements {
            vec!["capture", "input", "native_elements"]
        } else {
            vec!["capture", "input"]
        };
        json!({
            "id": self.id(), "title": self.title, "platform": self.platform,
            "kind": match self.target {
                Target::Window { .. } => "window",
                Target::Display(_) => "display",
                Target::Driver(_) => "webdriver",
                #[cfg(target_os = "linux")]
                Target::Wayland => "portal",
                #[cfg(target_os = "linux")]
                Target::AccessibleProcess(_) => "process",
            }, "capabilities": capabilities,
        })
    }
}

pub struct Platform {
    drivers: Vec<webdriver::Driver>,
    #[cfg(target_os = "linux")]
    wayland: Option<wayland::Portal>,
}

impl Platform {
    pub fn new(drivers: Vec<String>) -> Result<Self, Fault> {
        Ok(Self {
            drivers: drivers
                .into_iter()
                .map(webdriver::Driver::parse)
                .collect::<Result<_, _>>()?,
            #[cfg(target_os = "linux")]
            wayland: wayland::Portal::for_session(),
        })
    }

    pub fn list(&self) -> Vec<ListedTarget> {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        let mut targets = desktop::list();
        #[cfg(target_os = "linux")]
        let mut targets = if self.wayland.is_some() {
            vec![wayland::Portal::target()]
        } else {
            desktop::list()
        };
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        let mut targets = Vec::new();
        targets.extend(
            self.drivers
                .iter()
                .enumerate()
                .map(|(index, driver)| driver.target(index)),
        );
        targets
    }

    #[cfg(target_os = "linux")]
    pub fn native_process(
        &self,
        registration: &cove_devtools_protocol::control::Registration,
    ) -> Option<Target> {
        self.wayland.as_ref()?;
        (registration.platform == "linux").then_some(Target::AccessibleProcess(registration.pid?))
    }

    fn driver(&self, index: usize) -> Result<&webdriver::Driver, Fault> {
        self.drivers.get(index).ok_or_else(unavailable)
    }

    pub fn capture(&self, target: &Target, scope: Scope) -> Result<Capture, Fault> {
        match *target {
            Target::Driver(index) => self.driver(index)?.capture(scope),
            #[cfg(target_os = "linux")]
            Target::Wayland => self.wayland.as_ref().ok_or_else(unavailable)?.capture(),
            #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
            _ => desktop::capture(target, scope),
            #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
            _ => Err(unavailable()),
        }
    }

    pub fn perform(
        &self,
        target: &Target,
        action: Action,
        geometry: Option<&Geometry>,
    ) -> Result<&'static str, Fault> {
        action.validate(geometry)?;
        match *target {
            Target::Driver(index) => self.driver(index)?.perform(action, geometry),
            #[cfg(target_os = "linux")]
            Target::Wayland => self
                .wayland
                .as_ref()
                .ok_or_else(unavailable)?
                .perform(action, geometry),
            #[cfg(target_os = "linux")]
            Target::AccessibleProcess(pid) => desktop::activate_process(pid, action),
            #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
            _ => desktop::perform(target, action, geometry),
            #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
            _ => Err(unavailable()),
        }
    }

    pub fn native_elements(&self, target: &Target) -> Result<Value, Fault> {
        match *target {
            Target::Driver(index) => self.driver(index)?.source(),
            #[cfg(target_os = "linux")]
            Target::AccessibleProcess(pid) => desktop::process_elements(pid),
            #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
            _ => desktop::native_elements(target),
            #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
            _ => Err(unavailable()),
        }
    }

    pub fn find_element(&self, target: &Target, using: &str, value: &str) -> Result<Value, Fault> {
        match *target {
            Target::Driver(index) => self.driver(index)?.find(using, value),
            #[cfg(target_os = "linux")]
            Target::AccessibleProcess(pid) => desktop::find_process_element(pid, using, value),
            #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
            _ => desktop::find(target, using, value),
            #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
            _ => Err(unavailable()),
        }
    }
}

fn unavailable() -> Fault {
    Fault::new(
        "unavailable",
        "This platform session does not provide this operation.",
    )
}
