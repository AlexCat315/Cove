//! A child process attached to a pseudo terminal.
//!
//! Output is read in the background and handed to `output` there; callers hop to
//! their own thread. Writes are serialised on a second thread so a slow program
//! cannot stall the UI, exactly as the Swift `PseudoTerminal` serialises them on
//! a private queue.

pub mod command;
pub mod windows_command_line;

#[cfg(windows)]
mod conpty;
#[cfg(unix)]
mod posix;

pub use command::{TerminalCommand, TerminalError};
pub use windows_command_line::{command_line_string, environment_block_utf16};

/// Grid and pixel size reported to the child.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PseudoTerminalSize {
    pub columns: usize,
    pub rows: usize,
    pub cell_width: usize,
    pub cell_height: usize,
}

impl PseudoTerminalSize {
    pub fn new(columns: usize, rows: usize, cell_width: usize, cell_height: usize) -> Self {
        Self {
            columns,
            rows,
            cell_width,
            cell_height,
        }
    }
}

/// Called on the reader thread with each chunk of program output.
pub type OutputHandler = Box<dyn Fn(&[u8]) + Send + 'static>;
/// Called once on the reader thread with the exit status.
pub type ExitHandler = Box<dyn Fn(i32) + Send + 'static>;

/// One platform's pseudo terminal.
enum Backend {
    #[cfg(unix)]
    Posix(posix::PosixPseudoTerminal),
    #[cfg(windows)]
    ConPty(conpty::ConPty),
}

pub struct PseudoTerminal {
    backend: Backend,
    process_id: i32,
}

impl PseudoTerminal {
    /// Spawns `command` attached to a fresh pseudo terminal.
    pub fn spawn(
        command: &TerminalCommand,
        size: PseudoTerminalSize,
        on_output: impl Fn(&[u8]) + Send + 'static,
        on_exit: impl Fn(i32) + Send + 'static,
    ) -> Result<Self, TerminalError> {
        let on_output: OutputHandler = Box::new(on_output);
        let on_exit: ExitHandler = Box::new(on_exit);
        #[cfg(unix)]
        {
            let backend = posix::PosixPseudoTerminal::spawn(command, size, on_output, on_exit)?;
            let process_id = backend.process_id;
            Ok(Self {
                backend: Backend::Posix(backend),
                process_id,
            })
        }
        #[cfg(windows)]
        {
            let backend = conpty::ConPty::spawn(command, size, on_output, on_exit)?;
            let process_id = backend.process_id;
            Ok(Self {
                backend: Backend::ConPty(backend),
                process_id,
            })
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = (command, size, on_output, on_exit);
            Err(TerminalError::UnsupportedPlatform)
        }
    }

    pub fn process_id(&self) -> i32 {
        self.process_id
    }

    /// Blocking; run it off the UI thread so a slow program cannot stall it.
    pub fn write(&self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        match &self.backend {
            #[cfg(unix)]
            Backend::Posix(backend) => backend.write(bytes),
            #[cfg(windows)]
            Backend::ConPty(backend) => backend.write(bytes),
        }
    }

    pub fn resize(&self, size: PseudoTerminalSize) {
        match &self.backend {
            #[cfg(unix)]
            Backend::Posix(backend) => backend.resize(size),
            #[cfg(windows)]
            Backend::ConPty(backend) => backend.resize(size),
        }
    }

    /// Hang up the program, as closing a terminal window does.
    pub fn terminate(&self) {
        match &self.backend {
            #[cfg(unix)]
            Backend::Posix(backend) => backend.terminate(),
            #[cfg(windows)]
            Backend::ConPty(backend) => backend.terminate(),
        }
    }
}

impl Drop for PseudoTerminal {
    fn drop(&mut self) {
        self.terminate();
    }
}
