use crate::Result;
use cove_devtools_cli::{RunningController, inspector_url};
use cove_devtools_protocol::control::ConnectionInfo;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub struct Session {
    controller: RunningController,
    interrupted: Arc<AtomicBool>,
}

impl Session {
    pub fn start(port: u16, web_root: Option<PathBuf>) -> Result<Self> {
        let interrupted = Arc::new(AtomicBool::new(false));
        let signal = interrupted.clone();
        ctrlc::set_handler(move || signal.store(true, Ordering::Release))
            .map_err(|error| error.to_string())?;
        let browser_app = web_root.is_some();
        let controller =
            RunningController::start(port, web_root).map_err(|error| error.to_string())?;
        let info = controller.connection();
        eprintln!("Inspector: {}", inspector_url(info));
        if browser_app {
            eprintln!("App: {}/app/#{}", info.endpoint, info.token);
        }
        Ok(Self {
            controller,
            interrupted,
        })
    }

    pub fn connection(&self) -> &ConnectionInfo {
        self.controller.connection()
    }

    pub fn interrupted(&self) -> bool {
        self.interrupted.load(Ordering::Acquire)
    }

    pub fn environment(&self) -> [(&'static str, &str); 3] {
        let info = self.connection();
        [
            ("COVE_DEVTOOLS", "1"),
            ("COVE_DEVTOOLS_ENDPOINT", &info.endpoint),
            ("COVE_DEVTOOLS_TOKEN", &info.token),
        ]
    }

    pub fn run(&self, command: &mut Command) -> Result<()> {
        // Mobile launch arguments can contain pairing credentials.
        let program = command.get_program().to_string_lossy().into_owned();
        let mut child = command
            .spawn()
            .map_err(|error| format!("{program}: {error}"))?;
        loop {
            if self.interrupted() {
                let _ = child.kill();
                let _ = child.wait();
                return Ok(());
            }
            match child
                .try_wait()
                .map_err(|error| format!("{program}: {error}"))?
            {
                Some(status) if status.success() => return Ok(()),
                Some(status) => return Err(format!("{program} exited with {status}")),
                None => std::thread::sleep(Duration::from_millis(100)),
            }
        }
    }
}

pub fn launch_desktop(command: &mut Command, devtools: Option<&Session>) -> Result<()> {
    if let Some(session) = devtools {
        command.envs(session.environment());
        session.run(command)
    } else {
        crate::process::run(command)
    }
}
