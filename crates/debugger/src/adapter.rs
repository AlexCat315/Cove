//! Debug adapter discovery.
use crate::backend::{BackendKind, NativeBackend};
use crate::error::{DebuggerError, Result};
use crate::process::run_capture;
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(5);
const VERSION_TIMEOUT: Duration = Duration::from_secs(5);
const STATUS_TIMEOUT: Duration = Duration::from_secs(3);

/// A verified `lldb-dap` executable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Adapter {
    pub path: String,
    pub version: String,
}

impl Adapter {
    pub fn program(&self) -> &Path {
        Path::new(&self.path)
    }

    fn from_path(path: PathBuf) -> Result<Self> {
        // A bare program name is a PATH lookup; every other candidate must be
        // an existing file so a stale absolute path is never executed.
        let from_path_search = path
            .parent()
            .is_none_or(|parent| parent.as_os_str().is_empty());
        if !from_path_search && !path.is_file() {
            return Err(DebuggerError::unavailable(format!(
                "{} is not a file.",
                path.display()
            )));
        }
        let output = run_capture(&path, &["--version"], VERSION_TIMEOUT)?;
        if output.timed_out {
            return Err(DebuggerError::unavailable(format!(
                "{} --version timed out.",
                path.display()
            )));
        }
        if !output.status.success() {
            let detail = output
                .stderr
                .lines()
                .chain(output.stdout.lines())
                .find(|line| !line.trim().is_empty())
                .unwrap_or("no output");
            return Err(DebuggerError::unavailable(format!(
                "{} --version failed: {detail}",
                path.display()
            )));
        }
        let version = output
            .stdout
            .lines()
            .chain(output.stderr.lines())
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        Ok(Self {
            path: path.to_string_lossy().into_owned(),
            version: if version.is_empty() {
                "lldb-dap".to_owned()
            } else {
                version
            },
        })
    }
}

/// What the controller can offer before a session exists.
#[derive(Clone, Debug, Serialize)]
pub struct Availability {
    /// In-process liblldb, preferred when present.
    pub native: Option<NativeBackend>,
    /// Why liblldb could not be loaded, when it could not.
    pub native_reason: Option<String>,
    pub adapter: Option<Adapter>,
    pub developer_mode: Option<bool>,
    pub platform: String,
    pub reason: Option<String>,
}

impl Availability {
    /// Finds the adapter once for this controller process.
    ///
    /// `COVE_LLDB_DAP` is authoritative when set: a broken override is reported
    /// instead of silently trying another binary.
    pub fn discover() -> Self {
        let (native, native_reason) = match NativeBackend::discover() {
            Ok(native) => (Some(native), None),
            Err(reason) => (None, Some(reason)),
        };
        let mut availability = Self::discover_adapter();
        if native.is_some() {
            // A missing lldb-dap is not a problem while liblldb is usable.
            availability.reason = None;
        }
        availability.native = native;
        availability.native_reason = native_reason;
        availability
    }

    /// An availability with no native backend, for adapter-only callers.
    pub fn dap(adapter: Option<Adapter>, developer_mode: Option<bool>, platform: &str) -> Self {
        Self {
            native: None,
            native_reason: None,
            adapter,
            developer_mode,
            platform: platform.to_owned(),
            reason: None,
        }
    }

    fn discover_adapter() -> Self {
        let developer_mode = developer_mode();
        if let Some(override_path) = std::env::var_os("COVE_LLDB_DAP") {
            let path = PathBuf::from(override_path);
            return match Adapter::from_path(path.clone()) {
                Ok(adapter) => Self {
                    native: None,
                    native_reason: None,
                    adapter: Some(adapter),
                    developer_mode,
                    platform: std::env::consts::OS.to_owned(),
                    reason: None,
                },
                Err(error) => Self {
                    native: None,
                    native_reason: None,
                    adapter: None,
                    developer_mode,
                    platform: std::env::consts::OS.to_owned(),
                    reason: Some(format!(
                        "COVE_LLDB_DAP points to {}, which is not a usable lldb-dap: {}",
                        path.display(),
                        error.message()
                    )),
                },
            };
        }
        let mut reason = None;
        let deadline = Instant::now() + Duration::from_secs(12);
        for path in candidates() {
            if Instant::now() >= deadline {
                reason.get_or_insert_with(|| {
                    "Adapter discovery exceeded its 12-second bound.".to_owned()
                });
                break;
            }
            match Adapter::from_path(path.clone()) {
                Ok(adapter) => {
                    return Self {
                        native: None,
                        native_reason: None,
                        adapter: Some(adapter),
                        developer_mode,
                        platform: std::env::consts::OS.to_owned(),
                        reason: None,
                    };
                }
                Err(error) if reason.is_none() && path.exists() => {
                    reason = Some(error.message().to_owned());
                }
                Err(_) => {}
            }
        }
        Self {
            native: None,
            native_reason: None,
            adapter: None,
            developer_mode,
            platform: std::env::consts::OS.to_owned(),
            reason: Some(reason.unwrap_or_else(|| {
                "lldb-dap was not found. Install LLVM or the Xcode Command Line Tools, or set COVE_LLDB_DAP."
                    .to_owned()
            })),
        }
    }

    pub fn is_available(&self) -> bool {
        self.native.is_some() || self.adapter.is_some()
    }

    /// The backend a new session would use.
    pub fn backend(&self) -> Option<BackendKind> {
        if self.native.is_some() {
            Some(BackendKind::Native)
        } else if self.adapter.is_some() {
            Some(BackendKind::Dap)
        } else {
            None
        }
    }

    /// A truthful caveat about attach rights, or `None`.
    pub fn warning(&self) -> Option<String> {
        if !self.is_available() {
            return self.reason.clone();
        }
        if self.platform == "macos" && self.developer_mode == Some(false) {
            return Some(
                "Developer mode is disabled, so macOS can deny task_for_pid attach. Enable it with `sudo DevToolsSecurity -enable` or run a target signed with get-task-allow."
                    .to_owned(),
            );
        }
        None
    }
}

fn candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = run_capture(
            Path::new("/usr/bin/xcrun"),
            &["--find", "lldb-dap"],
            DISCOVERY_TIMEOUT,
        ) && output.status.success()
        {
            let found = output.stdout.trim();
            if !found.is_empty() {
                paths.push(PathBuf::from(found));
            }
        }
    }
    for path in known_paths() {
        paths.push(path);
    }
    // The last resort is a PATH lookup, verified by `--version` like the rest.
    paths.push(PathBuf::from(if cfg!(windows) {
        "lldb-dap.exe"
    } else {
        "lldb-dap"
    }));
    let mut seen = BTreeSet::new();
    paths
        .into_iter()
        .filter(|path| {
            let key = path.to_string_lossy().into_owned();
            seen.insert(key)
        })
        .collect()
}

#[cfg(target_os = "macos")]
fn known_paths() -> Vec<PathBuf> {
    let mut paths = vec![
        PathBuf::from("/Library/Developer/CommandLineTools/usr/bin/lldb-dap"),
        PathBuf::from("/Applications/Xcode.app/Contents/Developer/usr/bin/lldb-dap"),
        PathBuf::from("/Applications/Xcode-beta.app/Contents/Developer/usr/bin/lldb-dap"),
        PathBuf::from("/opt/homebrew/bin/lldb-dap"),
        PathBuf::from("/opt/homebrew/opt/llvm/bin/lldb-dap"),
        PathBuf::from("/usr/local/bin/lldb-dap"),
        PathBuf::from("/usr/local/opt/llvm/bin/lldb-dap"),
    ];
    if let Ok(entries) = std::fs::read_dir("/Applications") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("Xcode") && name.ends_with(".app") {
                paths.push(entry.path().join("Contents/Developer/usr/bin/lldb-dap"));
            }
        }
    }
    paths
}

#[cfg(target_os = "linux")]
fn known_paths() -> Vec<PathBuf> {
    let mut paths = vec![
        PathBuf::from("/usr/bin/lldb-dap"),
        PathBuf::from("/usr/local/bin/lldb-dap"),
        PathBuf::from("/opt/llvm/bin/lldb-dap"),
    ];
    for root in ["/usr/lib", "/usr/lib64"] {
        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                if name.to_string_lossy().starts_with("llvm-") {
                    paths.push(entry.path().join("bin/lldb-dap"));
                }
            }
        }
    }
    paths
}

#[cfg(windows)]
fn known_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for variable in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(root) = std::env::var_os(variable) {
            paths.push(PathBuf::from(root).join("LLVM/bin/lldb-dap.exe"));
        }
    }
    paths
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn known_paths() -> Vec<PathBuf> {
    Vec::new()
}

#[cfg(target_os = "macos")]
fn developer_mode() -> Option<bool> {
    let output = run_capture(
        Path::new("/usr/sbin/DevToolsSecurity"),
        &["-status"],
        STATUS_TIMEOUT,
    )
    .ok()?;
    let text = format!("{} {}", output.stdout, output.stderr).to_lowercase();
    if text.contains("disabled") {
        Some(false)
    } else if text.contains("enabled") {
        Some(true)
    } else {
        None
    }
}

#[cfg(not(target_os = "macos"))]
fn developer_mode() -> Option<bool> {
    None
}
