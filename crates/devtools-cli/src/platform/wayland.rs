//! Pair input and pixels with the same compositor-approved stream.
use super::{Action, Capture, Geometry, ListedTarget, Rect, Target, keys, unavailable};
use ashpd::desktop::{
    Session,
    remote_desktop::{
        DeviceType, KeyState, NotifyPointerAxisOptions, RemoteDesktop, SelectDevicesOptions,
    },
    screencast::{CursorMode, Screencast, SelectSourcesOptions, SourceType, Stream},
};
use cove_devtools_protocol::control::Fault;
use std::sync::Mutex;

mod frame;

#[derive(Default)]
pub(super) struct Portal {
    connection: Mutex<Option<Connection>>,
}

struct Connection {
    desktop: RemoteDesktop,
    screencast: Screencast,
    session: Session<RemoteDesktop>,
    stream: Stream,
    devices: DeviceType,
    id: String,
}

fn portal_error(error: impl std::fmt::Display) -> Fault {
    Fault::new("wayland_portal", error.to_string())
}

impl Portal {
    pub(super) fn for_session() -> Option<Self> {
        (std::env::var_os("WAYLAND_DISPLAY").is_some()
            || std::env::var_os("XDG_SESSION_TYPE").is_some_and(|value| value == "wayland"))
        .then(Self::default)
    }

    pub(super) fn target() -> ListedTarget {
        ListedTarget {
            target: Target::Wayland,
            title: "Wayland shared screen".into(),
            platform: "linux".into(),
            native_elements: false,
        }
    }

    pub(super) fn capture(&self) -> Result<Capture, Fault> {
        let mut slot = self
            .connection
            .try_lock()
            .map_err(|_| Fault::new("busy", "The Wayland portal is in use."))?;
        if slot.is_none() {
            *slot = Some(pollster::block_on(Connection::open())?);
        }
        let result = slot.as_ref().unwrap().capture();
        if result.is_err()
            && let Some(connection) = slot.take()
        {
            let _ = pollster::block_on(connection.session.close());
        }
        result
    }

    pub(super) fn perform(
        &self,
        action: Action,
        previous: Option<&Geometry>,
    ) -> Result<&'static str, Fault> {
        let slot = self
            .connection
            .try_lock()
            .map_err(|_| Fault::new("busy", "The Wayland portal is in use."))?;
        let connection = slot.as_ref().ok_or_else(|| {
            Fault::new(
                "target_unbound",
                "Capture the shared screen to authorize the Wayland session.",
            )
        })?;
        if previous.is_some_and(|previous| connection.geometry().as_ref().ok() != Some(previous)) {
            return Err(Fault::new(
                "geometry_changed",
                "The shared Wayland stream changed. Take a new capture.",
            ));
        }
        pollster::block_on(connection.perform(action))
    }
}

impl Connection {
    async fn open() -> Result<Self, Fault> {
        let desktop = RemoteDesktop::new().await.map_err(portal_error)?;
        let screencast = Screencast::new().await.map_err(portal_error)?;
        let session = desktop
            .create_session(Default::default())
            .await
            .map_err(portal_error)?;
        let selected = async {
            let devices = desktop
                .available_device_types()
                .await
                .map_err(portal_error)?
                & (DeviceType::KEYBOARD | DeviceType::POINTER);
            desktop
                .select_devices(
                    &session,
                    SelectDevicesOptions::default().set_devices(devices),
                )
                .await
                .map_err(portal_error)?
                .response()
                .map_err(portal_error)?;
            screencast
                .select_sources(
                    &session,
                    SelectSourcesOptions::default()
                        .set_sources(SourceType::MONITOR)
                        .set_multiple(false)
                        .set_cursor_mode(CursorMode::HIDDEN),
                )
                .await
                .map_err(portal_error)?
                .response()
                .map_err(portal_error)?;
            let response = desktop
                .start(&session, None, Default::default())
                .await
                .map_err(portal_error)?
                .response()
                .map_err(portal_error)?;
            let [stream] = response.streams() else {
                return Err(Fault::new(
                    "target_unbound",
                    "Select one display in the Wayland portal.",
                ));
            };
            Ok((stream.clone(), response.devices(), crate::random_id()?))
        }
        .await;
        match selected {
            Ok((stream, devices, id)) => Ok(Self {
                desktop,
                screencast,
                session,
                stream,
                devices,
                id,
            }),
            Err(error) => {
                let _ = session.close().await;
                Err(error)
            }
        }
    }

    fn geometry(&self) -> Result<Geometry, Fault> {
        let (width, height) = self
            .stream
            .size()
            .filter(|&(width, height)| width > 0 && height > 0)
            .ok_or_else(|| {
                Fault::new(
                    "unavailable",
                    "The portal did not report stream coordinates.",
                )
            })?;
        Ok(Geometry {
            coordinates: Rect {
                x: 0,
                y: 0,
                width: width as u32,
                height: height as u32,
            },
            window: None,
            display: None,
            context: Some(self.id.clone()),
        })
    }

    fn capture(&self) -> Result<Capture, Fault> {
        let fd = pollster::block_on(
            self.screencast
                .open_pipe_wire_remote(&self.session, Default::default()),
        )
        .map_err(portal_error)?;
        let pixels = frame::read(fd, self.stream.pipe_wire_node_id())?;
        let geometry = self.geometry()?;
        Ok(Capture {
            pixels,
            geometry,
            source: "wayland_screencast",
        })
    }

    async fn perform(&self, action: Action) -> Result<&'static str, Fault> {
        let pointer = matches!(
            action,
            Action::Click { .. } | Action::Drag { .. } | Action::Scroll { .. }
        );
        let required = if pointer {
            DeviceType::POINTER
        } else {
            DeviceType::KEYBOARD
        };
        if !self.devices.contains(required) {
            return Err(Fault::new(
                "permission_denied",
                "The Wayland portal did not grant the required input device.",
            ));
        }
        match action {
            Action::Click { x, y, button } => {
                self.move_pointer(x, y).await?;
                let button = [0x110, 0x112, 0x111][usize::from(button)];
                self.button(button, KeyState::Pressed).await?;
                self.button(button, KeyState::Released).await?;
            }
            Action::Drag { x, y, to_x, to_y } => {
                self.move_pointer(x, y).await?;
                self.button(0x110, KeyState::Pressed).await?;
                let result = async {
                    for step in 1..=8 {
                        std::thread::sleep(std::time::Duration::from_millis(16));
                        self.move_pointer(x + (to_x - x) * step / 8, y + (to_y - y) * step / 8)
                            .await?;
                    }
                    Ok(())
                }
                .await;
                let release = self.button(0x110, KeyState::Released).await;
                result.and(release)?;
            }
            Action::Scroll { x, y, dx, dy } => {
                self.move_pointer(x, y).await?;
                self.desktop
                    .notify_pointer_axis(
                        &self.session,
                        f64::from(dx),
                        f64::from(dy),
                        NotifyPointerAxisOptions::default().set_finish(true),
                    )
                    .await
                    .map_err(portal_error)?;
            }
            Action::Key { key, modifiers } => {
                let mut keys = modifiers
                    .iter()
                    .map(|key| keycode(key))
                    .collect::<Result<Vec<_>, _>>()?;
                keys.push(keycode(&key)?);
                let mut pressed = Vec::new();
                let result = async {
                    for code in keys {
                        self.key(code, KeyState::Pressed).await?;
                        pressed.push(code);
                    }
                    Ok(())
                }
                .await;
                let mut release = Ok(());
                for code in pressed.into_iter().rev() {
                    if let Err(error) = self.key(code, KeyState::Released).await {
                        release = Err(error);
                    }
                }
                result.and(release)?;
            }
            Action::Text { text } => {
                for character in text.chars() {
                    let scalar = character as i32;
                    let keysym = match character {
                        '\r' | '\n' => 0xff0d,
                        '\t' => 0xff09,
                        _ if scalar < 256 => scalar,
                        _ => 0x01000000 | scalar,
                    };
                    self.desktop
                        .notify_keyboard_keysym(
                            &self.session,
                            keysym,
                            KeyState::Pressed,
                            Default::default(),
                        )
                        .await
                        .map_err(portal_error)?;
                    self.desktop
                        .notify_keyboard_keysym(
                            &self.session,
                            keysym,
                            KeyState::Released,
                            Default::default(),
                        )
                        .await
                        .map_err(portal_error)?;
                }
                return Ok("wayland_keysym");
            }
            Action::Activate { .. } => return Err(unavailable()),
        }
        Ok("wayland_input")
    }

    async fn move_pointer(&self, x: i32, y: i32) -> Result<(), Fault> {
        self.desktop
            .notify_pointer_motion_absolute(
                &self.session,
                self.stream.pipe_wire_node_id(),
                f64::from(x),
                f64::from(y),
                Default::default(),
            )
            .await
            .map_err(portal_error)
    }

    async fn button(&self, button: i32, state: KeyState) -> Result<(), Fault> {
        self.desktop
            .notify_pointer_button(&self.session, button, state, Default::default())
            .await
            .map_err(portal_error)
    }

    async fn key(&self, code: i32, state: KeyState) -> Result<(), Fault> {
        self.desktop
            .notify_keyboard_keycode(&self.session, code, state, Default::default())
            .await
            .map_err(portal_error)
    }
}

fn keycode(key: &str) -> Result<i32, Fault> {
    if let Some(code) = keys::letter_or_digit(key) {
        return Ok(i32::from(code));
    }
    Ok(match key {
        "Escape" => 1,
        "Backspace" => 14,
        "Tab" => 15,
        "Enter" => 28,
        "Control" => 29,
        "Shift" => 42,
        "Alt" => 56,
        "Space" => 57,
        "Home" => 102,
        "ArrowUp" => 103,
        "PageUp" => 104,
        "ArrowLeft" => 105,
        "ArrowRight" => 106,
        "End" => 107,
        "ArrowDown" => 108,
        "PageDown" => 109,
        "Delete" => 111,
        "Meta" => 125,
        _ => {
            return Err(Fault::new(
                "unsupported_key",
                "The requested key is not mapped.",
            ));
        }
    })
}
