//! The program a terminal session runs, and its errors.

use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

/// Errors raised while starting a terminal program.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TerminalError {
    ExecutableNotFound(String),
    /// `code` is `errno` on macOS/Linux and a Win32 error / HRESULT on Windows.
    SpawnFailed(i32),
    UnsupportedPlatform,
    AlreadyStarted,
}

impl std::fmt::Display for TerminalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExecutableNotFound(name) => write!(f, "Executable not found: {name}"),
            Self::SpawnFailed(code) => {
                write!(f, "Could not start the terminal program (error {code})")
            }
            Self::UnsupportedPlatform => {
                f.write_str("Pseudo terminals are not available on this platform")
            }
            Self::AlreadyStarted => f.write_str("The terminal session is already running"),
        }
    }
}

impl std::error::Error for TerminalError {}

/// The program a `TerminalSession` runs on its pseudo terminal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalCommand {
    /// Absolute path, or a name resolved through `PATH`.
    pub executable: String,
    pub arguments: Vec<String>,
    /// Entries layered over the inherited environment. `TERM` and `COLORTERM`
    /// describe the emulator and are set unless overridden here.
    pub environment: BTreeMap<String, String>,
    /// `None` inherits the host process's working directory.
    pub working_directory: Option<PathBuf>,
}

impl TerminalCommand {
    pub fn new(executable: impl Into<String>) -> Self {
        Self {
            executable: executable.into(),
            arguments: Vec::new(),
            environment: BTreeMap::new(),
            working_directory: None,
        }
    }

    pub fn with_arguments(mut self, arguments: Vec<String>) -> Self {
        self.arguments = arguments;
        self
    }

    pub fn with_environment(mut self, environment: BTreeMap<String, String>) -> Self {
        self.environment = environment;
        self
    }

    pub fn with_working_directory(mut self, directory: impl Into<PathBuf>) -> Self {
        self.working_directory = Some(directory.into());
        self
    }

    /// The user's login shell (`$SHELL -l`, falling back to `/bin/sh`); Windows
    /// PowerShell on Windows.
    pub fn login_shell() -> Self {
        #[cfg(windows)]
        {
            Self::new("powershell.exe").with_arguments(vec!["-NoLogo".into()])
        }
        #[cfg(not(windows))]
        {
            let shell = std::env::var("SHELL")
                .ok()
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "/bin/sh".into());
            Self::new(shell).with_arguments(vec!["-l".into()])
        }
    }

    /// Environment handed to the child: inherited, emulator identity, overrides.
    pub fn resolved_environment(&self) -> BTreeMap<String, String> {
        self.resolved_environment_from(std::env::vars())
    }

    /// Same, from an explicit inherited environment (tests and hosts inject
    /// their own).
    pub fn resolved_environment_from(
        &self,
        inherited: impl IntoIterator<Item = (String, String)>,
    ) -> BTreeMap<String, String> {
        let mut result: BTreeMap<String, String> = inherited.into_iter().collect();
        result.insert("TERM".into(), "xterm-256color".into());
        result.insert("COLORTERM".into(), "truecolor".into());
        result.insert("TERM_PROGRAM".into(), "CoveUI".into());
        if !result.contains_key("LANG")
            && !result.contains_key("LC_ALL")
            && !result.contains_key("LC_CTYPE")
        {
            result.insert("LANG".into(), "en_US.UTF-8".into());
        }
        for (key, value) in &self.environment {
            result.insert(key.clone(), value.clone());
        }
        result
    }

    /// Absolute executable path; bare names are looked up in `PATH` (and, on
    /// Windows, completed with the `PATHEXT` extensions).
    pub fn resolved_executable(&self) -> Option<String> {
        self.resolved_executable_with(
            std::env::var("PATH").ok().as_deref(),
            std::env::var("PATHEXT").ok().as_deref(),
        )
    }

    #[cfg_attr(
        unix,
        allow(unused_variables, reason = "PATHEXT is a Windows-only search rule")
    )]
    pub fn resolved_executable_with(
        &self,
        path: Option<&str>,
        path_extensions: Option<&str>,
    ) -> Option<String> {
        #[cfg(windows)]
        {
            let separator = ';';
            let slashes = ['/', '\\'];
            let extensions: Vec<String> = if self.executable.contains('.') {
                vec![String::new()]
            } else {
                let mut all = vec![String::new()];
                all.extend(
                    path_extensions
                        .unwrap_or(".COM;.EXE;.BAT;.CMD")
                        .split(separator)
                        .map(str::to_owned),
                );
                all
            };
            // CreateProcess and cmd re-parse the first token of the command line;
            // forward slashes in the executable path can be mistaken for a switch
            // and corrupt cmd's /c parsing.
            let windows_path = |value: String| value.replace('/', "\\");
            let runnable = |candidate: &str| Path::new(candidate).is_file();
            let default_path = "/usr/bin:/bin";
            if self.executable.contains(slashes) {
                return runnable(&self.executable).then(|| windows_path(self.executable.clone()));
            }
            for directory in path.unwrap_or(default_path).split(separator) {
                for suffix in &extensions {
                    let candidate =
                        windows_path(format!("{directory}/{}", self.executable) + suffix);
                    if runnable(&candidate) {
                        return Some(candidate);
                    }
                }
            }
            None
        }
        #[cfg(not(windows))]
        {
            let separator = ':';
            let slashes = ['/'];
            let windows_path = |value: String| value;
            let executable = OsStr::new(&self.executable);
            let is_runnable = |candidate: &Path| is_executable(candidate);
            let default_path = "/usr/bin:/bin";
            if self.executable.contains(slashes) {
                return is_runnable(Path::new(&self.executable))
                    .then(|| windows_path(self.executable.clone()));
            }
            for directory in path.unwrap_or(default_path).split(separator) {
                let candidate =
                    windows_path(format!("{directory}/{}", executable.to_string_lossy()));
                if is_runnable(Path::new(&candidate)) {
                    return Some(candidate);
                }
            }
            None
        }
    }
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// `NAME=value` pairs in a stable order, with a `Vec` of owned `CString`s kept
/// alive for the duration of `execve`.
#[cfg(unix)]
pub(super) fn environment_c_strings(
    environment: &BTreeMap<String, String>,
) -> Vec<std::ffi::CString> {
    environment
        .iter()
        .filter_map(|(key, value)| std::ffi::CString::new(format!("{key}={value}")).ok())
        .collect()
}

/// Owned, NUL-terminated strings that outlive a `fork`.
#[cfg(unix)]
pub(super) fn string_c_strings(values: &[OsString]) -> Vec<std::ffi::CString> {
    values
        .iter()
        .map(|value| {
            std::ffi::CString::new(value.as_encoded_bytes())
                .unwrap_or_else(|_| std::ffi::CString::new("").expect("empty CString is valid"))
        })
        .collect()
}
