//! Translation of `System/Sources/CoveProcess/NativeProcess.swift`.
//!
//! Owns the native child and its process group; pipe handles remain with the
//! session's readers and writer.
use super::command::ProcessCommand;
use os_pipe::{PipeReader, PipeWriter};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct StartedPipes {
    pub input: PipeWriter,
    pub output: PipeReader,
    pub errors: PipeReader,
}

struct Inner {
    child: Mutex<Option<crate::native::NativeChild>>,
    running: AtomicBool,
    pid: AtomicI32,
}

pub struct NativeProcess {
    inner: Arc<Inner>,
}

impl NativeProcess {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                child: Mutex::new(None),
                running: AtomicBool::new(false),
                pid: AtomicI32::new(0),
            }),
        }
    }

    pub fn process_identifier(&self) -> i32 {
        self.inner.pid.load(Ordering::Acquire)
    }

    pub fn is_running(&self) -> bool {
        self.inner.running.load(Ordering::Acquire)
    }

    /// Launches the child and returns the parent ends of its stdio pipes. Only
    /// the child keeps the opposite ends so readers observe EOF after exit.
    pub fn start(
        &self,
        command: &ProcessCommand,
        on_exit: Arc<dyn Fn(i32) + Send + Sync>,
    ) -> io::Result<StartedPipes> {
        let legacy = crate::process::ProcessCommand {
            executable: command.executable.clone(),
            arguments: command.arguments.iter().map(OsString::from).collect(),
            directory: command.directory.clone(),
            environment: command
                .environment
                .iter()
                .map(|(key, value)| (OsString::from(key), OsString::from(value)))
                .collect::<BTreeMap<_, _>>(),
        };
        let spawned = crate::native::spawn(&legacy)?;
        let pid = spawned.child.pid() as i32;
        self.inner.pid.store(pid, Ordering::Release);
        self.inner.running.store(true, Ordering::Release);
        *self
            .inner
            .child
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(spawned.child);

        let inner = self.inner.clone();
        let _ = std::thread::Builder::new()
            .name("cove.process.exit".into())
            .spawn(move || {
                loop {
                    let status = {
                        let mut slot = inner
                            .child
                            .lock()
                            .unwrap_or_else(|error| error.into_inner());
                        match slot.as_mut() {
                            Some(child) => child.try_wait().ok().flatten(),
                            None => break,
                        }
                    };
                    if let Some(status) = status {
                        inner.running.store(false, Ordering::Release);
                        on_exit(status);
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            });

        Ok(StartedPipes {
            input: spawned.input,
            output: spawned.output,
            errors: spawned.errors,
        })
    }

    pub fn terminate(&self, force: bool) {
        let pid = self.process_identifier();
        if pid <= 0 {
            return;
        }
        #[cfg(unix)]
        {
            // SAFETY: the pid was recorded from the child this object owns, and
            // the child was placed in its own process group at spawn.
            unsafe {
                let signal = if force { libc::SIGKILL } else { libc::SIGTERM };
                libc::kill(-pid, signal);
                libc::kill(pid, signal);
            }
        }
        let mut slot = self
            .inner
            .child
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(child) = slot.as_mut() {
            let _ = child.try_wait();
            if force {
                child.terminate();
            }
        }
        self.inner.running.store(false, Ordering::Release);
    }
}

impl Default for NativeProcess {
    fn default() -> Self {
        Self::new()
    }
}
