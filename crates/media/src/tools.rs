//! Finds the FFmpeg command-line tools that export and probing rely on.
//!
//! `cove-media` links FFmpeg as libraries, but exporting media and reading
//! container metadata from a packaged app also needs the `ffmpeg` and `ffprobe`
//! executables. Resolving them here keeps every app on one lookup order instead
//! of re-inventing its own path scraping:
//!
//! 1. explicit `COVE_FFMPEG_EXECUTABLE` / `COVE_FFPROBE_EXECUTABLE`,
//! 2. an explicit directory: `COVE_FFMPEG_DIR`,
//! 3. the directory built by the `ffmpeg-tools` feature (pinned release),
//! 4. `PATH`.
//!
//! A tool is only accepted when the file exists and is executable, so a broken
//! `PATH` entry surfaces as a clear error rather than a spawn failure later.

use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

const FFMPEG: &str = "ffmpeg";
const FFPROBE: &str = "ffprobe";

/// Why a tool could not be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolError {
    /// No candidate path for the tool existed.
    NotFound {
        /// Which executable was missing.
        tool: &'static str,
        /// The paths that were considered, in lookup order.
        searched: Vec<PathBuf>,
    },
    /// A candidate exists but cannot be executed.
    NotExecutable {
        /// The rejected path.
        path: PathBuf,
    },
    /// The tool exists but refused to run.
    Failed {
        /// The tool that failed.
        path: PathBuf,
        /// Its diagnostics.
        message: String,
    },
}

impl fmt::Display for ToolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { tool, searched } => {
                write!(formatter, "no executable `{tool}` was found")?;
                for path in searched {
                    write!(formatter, "\n  tried {}", path.display())?;
                }
                Ok(())
            }
            Self::NotExecutable { path } => {
                write!(formatter, "`{}` is not an executable file", path.display())
            }
            Self::Failed { path, message } => {
                write!(formatter, "`{}` failed: {message}", path.display())
            }
        }
    }
}

impl std::error::Error for ToolError {}

/// Every location the resolver may consider.
///
/// Discovery reads this from the environment; tests build it directly, which
/// keeps them independent of ambient variables.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolSearch {
    /// An explicit `ffmpeg` path.
    pub ffmpeg: Option<PathBuf>,
    /// An explicit `ffprobe` path.
    pub ffprobe: Option<PathBuf>,
    /// Directories to scan for both tools, in order.
    pub directories: Vec<PathBuf>,
    /// The `PATH` value to fall back to.
    pub path: Option<OsString>,
}

impl ToolSearch {
    /// The default order: overrides, explicit directory, bundled build, `PATH`.
    pub fn from_environment() -> Self {
        let mut directories = Vec::new();
        if let Some(directory) = std::env::var_os("COVE_FFMPEG_DIR") {
            directories.push(PathBuf::from(directory));
        }
        if let Some(directory) =
            bundled_directory().filter(|directory| directories.first() != Some(directory))
        {
            directories.push(directory);
        }

        Self {
            ffmpeg: std::env::var_os("COVE_FFMPEG_EXECUTABLE").map(PathBuf::from),
            ffprobe: std::env::var_os("COVE_FFPROBE_EXECUTABLE").map(PathBuf::from),
            directories,
            path: std::env::var_os("PATH"),
        }
    }
}

/// The directory the `ffmpeg-tools` feature built, if that feature was on.
fn bundled_directory() -> Option<PathBuf> {
    option_env!("COVE_FFMPEG_CLI_DIR").map(PathBuf::from)
}

/// A resolved pair of FFmpeg command-line tools.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FfmpegTools {
    ffmpeg: PathBuf,
    ffprobe: PathBuf,
}

impl FfmpegTools {
    /// Resolves the tools from the environment, see [`ToolSearch`] for the order.
    pub fn discover() -> Result<Self, ToolError> {
        resolve(&ToolSearch::from_environment())
    }

    /// Resolves the pair from an `ffmpeg` path, deriving `ffprobe` beside it.
    pub fn from_ffmpeg(path: impl Into<PathBuf>) -> Result<Self, ToolError> {
        resolve(&ToolSearch {
            ffmpeg: Some(path.into()),
            ..ToolSearch::default()
        })
    }

    /// The `ffmpeg` executable.
    pub fn ffmpeg(&self) -> &Path {
        &self.ffmpeg
    }

    /// The `ffprobe` executable.
    pub fn ffprobe(&self) -> &Path {
        &self.ffprobe
    }

    /// The first line of `ffmpeg -version`, e.g. `ffmpeg version 9.0 ...`.
    pub fn version(&self) -> Result<String, ToolError> {
        first_line(&self.ffmpeg, "-version")
    }

    /// The first line of `ffprobe -version`.
    pub fn probe_version(&self) -> Result<String, ToolError> {
        first_line(&self.ffprobe, "-version")
    }
}

/// Resolves a pair of tools from an explicit search description.
pub fn resolve(search: &ToolSearch) -> Result<FfmpegTools, ToolError> {
    if let Some(ffmpeg) = &search.ffmpeg {
        require_executable(ffmpeg)?;
    }
    if let Some(ffprobe) = &search.ffprobe {
        require_executable(ffprobe)?;
    }

    let mut searched = Vec::new();

    if let (Some(ffmpeg), Some(ffprobe)) = (&search.ffmpeg, &search.ffprobe) {
        return Ok(FfmpegTools {
            ffmpeg: ffmpeg.clone(),
            ffprobe: ffprobe.clone(),
        });
    }

    // One side was pinned: the other must live beside it, never elsewhere, or the
    // pair could mix releases.
    if let Some(ffmpeg) = &search.ffmpeg {
        let ffprobe = sibling(ffmpeg, FFPROBE);
        searched.push(ffprobe.clone());
        require_executable(&ffprobe)?;
        return Ok(FfmpegTools {
            ffmpeg: ffmpeg.clone(),
            ffprobe,
        });
    }
    if let Some(ffprobe) = &search.ffprobe {
        let ffmpeg = sibling(ffprobe, FFMPEG);
        searched.push(ffmpeg.clone());
        require_executable(&ffmpeg)?;
        return Ok(FfmpegTools {
            ffmpeg,
            ffprobe: ffprobe.clone(),
        });
    }

    for directory in &search.directories {
        if let Some(tools) = pair_in(directory, &mut searched) {
            return Ok(tools);
        }
    }

    if let Some(path) = &search.path {
        for directory in std::env::split_paths(path) {
            if let Some(tools) = pair_in(&directory, &mut searched) {
                return Ok(tools);
            }
        }
    }

    Err(ToolError::NotFound {
        tool: FFMPEG,
        searched,
    })
}

/// Both tools from one directory, or `None` so the caller keeps looking.
fn pair_in(directory: &Path, searched: &mut Vec<PathBuf>) -> Option<FfmpegTools> {
    let ffmpeg = candidate(directory, FFMPEG);
    let ffprobe = candidate(directory, FFPROBE);
    searched.push(ffmpeg.clone());
    searched.push(ffprobe.clone());

    if is_executable(&ffmpeg) && is_executable(&ffprobe) {
        Some(FfmpegTools { ffmpeg, ffprobe })
    } else {
        None
    }
}

/// The path a tool would have in a directory, including Windows' `.exe`.
fn candidate(directory: &Path, tool: &str) -> PathBuf {
    let path = directory.join(tool);
    if cfg!(windows) {
        path.with_extension("exe")
    } else {
        path
    }
}

/// `<dir>/ffmpeg` -> `<dir>/ffprobe`, keeping any extension.
fn sibling(path: &Path, tool: &str) -> PathBuf {
    match path.extension() {
        Some(extension) => path.with_file_name(format!("{tool}.{}", extension.to_string_lossy())),
        None => path.with_file_name(tool),
    }
}

fn require_executable(path: &Path) -> Result<(), ToolError> {
    if is_executable(path) {
        Ok(())
    } else {
        Err(ToolError::NotExecutable {
            path: path.to_path_buf(),
        })
    }
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn first_line(path: &Path, argument: &str) -> Result<String, ToolError> {
    let output = Command::new(path)
        .arg(argument)
        .output()
        .map_err(|error| ToolError::Failed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;

    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(ToolError::Failed {
            path: path.to_path_buf(),
            message: if message.is_empty() {
                format!("exit status {}", output.status)
            } else {
                message
            },
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// A scratch directory removed when the test ends, even on panic.
    struct Scratch {
        path: PathBuf,
    }

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir()
                .join(format!("cove-media-tools-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("scratch directory");
            Self { path }
        }

        fn executable(&self, name: &str, body: &str) -> PathBuf {
            self.executable_in(&self.path, name, body)
        }

        fn executable_in(&self, directory: &Path, name: &str, body: &str) -> PathBuf {
            let path = directory.join(name);
            fs::write(&path, body).expect("write the tool");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
                    .expect("mark executable");
            }
            path
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn an_explicit_pair_wins_over_every_other_candidate() {
        let scratch = Scratch::new("explicit");
        let decoy = scratch.path.join("decoy");
        fs::create_dir_all(&decoy).expect("decoy directory");
        scratch.executable_in(&decoy, "ffmpeg", "#!/bin/sh\nexit 1\n");
        scratch.executable_in(&decoy, "ffprobe", "#!/bin/sh\nexit 1\n");

        let ffmpeg = scratch.executable("ffmpeg", "#!/bin/sh\necho ffmpeg version 9.0\n");
        let ffprobe = scratch.executable("ffprobe", "#!/bin/sh\necho ffprobe version 9.0\n");

        let tools = resolve(&ToolSearch {
            ffmpeg: Some(ffmpeg.clone()),
            ffprobe: Some(ffprobe.clone()),
            directories: vec![decoy],
            path: None,
        })
        .expect("the explicit pair resolves");

        assert_eq!(tools.ffmpeg(), ffmpeg);
        assert_eq!(tools.ffprobe(), ffprobe);
    }

    #[test]
    fn a_pinned_ffmpeg_derives_ffprobe_beside_it() {
        let scratch = Scratch::new("sibling");
        let ffmpeg = scratch.executable("ffmpeg", "#!/bin/sh\nexit 0\n");
        scratch.executable("ffprobe", "#!/bin/sh\nexit 0\n");
        scratch.executable("unrelated", "#!/bin/sh\nexit 0\n");

        let tools = FfmpegTools::from_ffmpeg(ffmpeg.clone()).expect("the sibling resolves");
        assert_eq!(tools.ffmpeg(), ffmpeg);
        assert_eq!(tools.ffprobe(), scratch.path.join("ffprobe"));
    }

    #[test]
    fn a_missing_ffprobe_is_reported_with_the_sibling_path() {
        let scratch = Scratch::new("missing-probe");
        let ffmpeg = scratch.executable("ffmpeg", "#!/bin/sh\nexit 0\n");
        scratch.executable("unrelated", "#!/bin/sh\nexit 0\n");

        let error = resolve(&ToolSearch {
            ffmpeg: Some(ffmpeg),
            ..ToolSearch::default()
        })
        .expect_err("a missing ffprobe must be an error");

        let message = error.to_string();
        assert!(message.contains("ffprobe"), "{message}");
        assert!(message.contains("not an executable file"), "{message}");
    }

    #[test]
    fn directories_are_searched_in_order() {
        let scratch = Scratch::new("order");
        let incomplete = scratch.path.join("incomplete");
        let complete = scratch.path.join("complete");
        fs::create_dir_all(&incomplete).expect("first directory");
        fs::create_dir_all(&complete).expect("second directory");
        scratch.executable_in(&incomplete, "ffmpeg", "#!/bin/sh\nexit 0\n");
        let ffmpeg = scratch.executable_in(&complete, "ffmpeg", "#!/bin/sh\nexit 0\n");
        scratch.executable_in(&complete, "ffprobe", "#!/bin/sh\nexit 0\n");

        let tools = resolve(&ToolSearch {
            directories: vec![incomplete, complete.clone()],
            path: None,
            ..ToolSearch::default()
        })
        .expect("the complete directory resolves");

        assert_eq!(tools.ffmpeg(), ffmpeg);
        assert_eq!(tools.ffprobe(), complete.join("ffprobe"));
    }

    #[test]
    fn path_is_the_last_resort() {
        let scratch = Scratch::new("path");
        scratch.executable("ffmpeg", "#!/bin/sh\nexit 0\n");
        let ffprobe = scratch.executable("ffprobe", "#!/bin/sh\nexit 0\n");

        let tools = resolve(&ToolSearch {
            path: Some(scratch.path.as_os_str().to_owned()),
            ..ToolSearch::default()
        })
        .expect("PATH holds both tools");

        assert_eq!(tools.ffmpeg(), scratch.path.join("ffmpeg"));
        assert_eq!(tools.ffprobe(), ffprobe);
    }

    #[test]
    fn version_reads_the_first_line_of_ffmpeg_output() {
        let scratch = Scratch::new("version");
        let ffmpeg = scratch.executable(
            "ffmpeg",
            "#!/bin/sh\nprintf 'ffmpeg version 9.0.1\\nbuilt with clang\\n'\n",
        );
        scratch.executable("ffprobe", "#!/bin/sh\nexit 0\n");

        let tools = FfmpegTools::from_ffmpeg(ffmpeg).expect("the pair resolves");
        assert_eq!(
            tools.version().expect("version runs"),
            "ffmpeg version 9.0.1"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_file_without_the_execute_bit_is_rejected() {
        let scratch = Scratch::new("permissions");
        let ffmpeg = scratch.path.join("ffmpeg");
        fs::write(&ffmpeg, "#!/bin/sh\nexit 0\n").expect("write the tool");

        let error = FfmpegTools::from_ffmpeg(ffmpeg.clone())
            .expect_err("a non-executable file must be rejected");
        assert_eq!(error, ToolError::NotExecutable { path: ffmpeg });
    }

    #[test]
    fn a_missing_tool_lists_every_path_it_tried() {
        let scratch = Scratch::new("not-found");
        let empty = scratch.path.join("empty");
        fs::create_dir_all(&empty).expect("empty directory");

        let error = resolve(&ToolSearch {
            directories: vec![empty.clone()],
            ..ToolSearch::default()
        })
        .expect_err("nothing resolves in an empty directory");

        match error {
            ToolError::NotFound { tool, searched } => {
                assert_eq!(tool, FFMPEG);
                assert_eq!(searched, vec![empty.join("ffmpeg"), empty.join("ffprobe")]);
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[cfg(feature = "ffmpeg-tools")]
    #[test]
    fn the_pinned_build_is_discoverable_without_any_override() {
        let tools = FfmpegTools::discover().expect("the ffmpeg-tools build resolves");
        assert!(
            tools
                .ffmpeg()
                .starts_with(bundled_directory().expect("a bundled directory")),
            "{}",
            tools.ffmpeg().display()
        );
        assert!(
            tools
                .version()
                .expect("version runs")
                .starts_with("ffmpeg version"),
        );
    }
}
