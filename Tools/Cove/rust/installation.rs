//! CLI/SDK installation, packaging and updates.
//!
//! Swift sources: `Installation/Archive.swift`, `Installation/Directories.swift`,
//! `Installation/Installer.swift`, `Installation/NativeArchive.swift`,
//! `Installation/Receipt.swift`, `Installation/RuntimeLibraries.swift`,
//! `Installation/SDKPayload.swift`, `Installation/TarArchive.swift`,
//! `Installation/Update.swift`.

use crate::cli::{standardized, AppPlatform, Arguments, OptionKey};
use crate::download::Download;
use crate::platforms_android::{
    AndroidToolchain, NativeDependencies,
};
use crate::project_sdk::{
    numeric_compare, CoveSDK, NativeArchive as NativeArchiveMetadata, NativeArtifacts, SDKRelease,
    SDKStore,
};
use crate::sha256;
use crate::support::{environment, Files, Shell, StdoutTarget, ToolError};
use crate::toolchains::NativeSDK;
use cove_system::files::FileLock;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn unique_token() -> String {
    format!(
        "{:x}-{:x}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or_default()
    )
}

fn home_directory() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Default installation directories.
pub struct Installation;

impl Installation {
    pub fn commands() -> PathBuf {
        if cfg!(windows) {
            Self::directory().join("bin")
        } else {
            std::env::var_os("COVE_BIN_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| home_directory().join(".local/bin"))
        }
    }

    pub fn directory() -> PathBuf {
        if let Some(path) = std::env::var_os("COVE_HOME") {
            return PathBuf::from(path);
        }
        if cfg!(windows) {
            return std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(home_directory)
                .join("Cove");
        }
        if cfg!(target_os = "linux") {
            return std::env::var_os("XDG_DATA_HOME")
                .map(|path| PathBuf::from(path).join("cove"))
                .unwrap_or_else(|| home_directory().join(".local/share/cove"));
        }
        home_directory().join(".cove")
    }
}

/// Records which installation owns a version directory and its commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallationReceipt {
    pub version: String,
    pub directory: PathBuf,
    pub commands: PathBuf,
}

impl InstallationReceipt {
    pub const FILENAME: &'static str = "installation.json";

    pub fn executable_directory() -> PathBuf {
        if let Ok(executable) = std::env::current_exe() {
            let resolved = executable
                .canonicalize()
                .unwrap_or_else(|_| executable.clone());
            if let Some(parent) = resolved.parent() {
                return parent.to_path_buf();
            }
        }
        let args: Vec<String> = std::env::args().collect();
        let first = args.first().cloned().unwrap_or_else(|| "cove".into());
        standardized(&first)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }

    pub fn read(at: &Path) -> Result<Self, ToolError> {
        let file = at.join(Self::FILENAME);
        let data = std::fs::read(&file)
            .map_err(|error| ToolError::invalid(format!("{}: {error}", file.display())))?;
        serde_json::from_slice(&data)
            .map_err(|error| ToolError::invalid(format!("{}: {error}", file.display())))
    }
}

/// The platform tag used by release archives (`macos-arm64` etc.).
struct PlatformTag;

impl PlatformTag {
    fn architecture() -> &'static str {
        if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "x86_64"
        }
    }
}

pub struct CLIUpdate;

impl CLIUpdate {
    pub fn platform() -> String {
        let system = if AppPlatform::host() == AppPlatform::Mac {
            "macos"
        } else if AppPlatform::host() == AppPlatform::Win {
            "windows"
        } else {
            "linux"
        };
        format!("{system}-{}", PlatformTag::architecture())
    }

    pub fn run(version: Option<&str>) -> Result<(), ToolError> {
        let executable = InstallationReceipt::executable_directory();
        let receipt = InstallationReceipt::read(&executable).map_err(|_| {
            ToolError::invalid(
                if executable.to_string_lossy().contains("/Cellar/") {
                    "Update this installation with brew upgrade cove"
                } else {
                    "Use the package manager that installed cove, or install with the official standalone installer"
                },
            )
        })?;
        let repository = SDKRelease::current()?.repository.replace(".git", "");
        let version = match version {
            Some(requested) => {
                if !SDKRelease::is_version(requested) {
                    return Err(ToolError::invalid("Supply an exact CLI version"));
                }
                requested.to_string()
            }
            None => {
                let latest = SDKRelease::latest_version(&repository)?;
                if numeric_compare(&latest, &receipt.version) != std::cmp::Ordering::Greater {
                    return Err(ToolError::invalid(
                        "The release channel did not provide a newer supported version",
                    ));
                }
                latest
            }
        };
        if version == receipt.version {
            println!("cove {version} is current");
            return Ok(());
        }
        let store = SDKStore::new(Some(receipt.directory.clone()));
        store.install(&version)?;
        let payload = store.installed(&version);
        let binary = payload.join(executable_name());
        let actual = Shell::run(
            &[binary.display().to_string(), "--version".into()],
            None,
            &environment(),
            true,
            true,
            StdoutTarget::StandardOutput,
        )?;
        if actual != format!("cove {version}") {
            return Err(ToolError::invalid(
                "Downloaded CLI failed its startup check",
            ));
        }
        CLIInstaller::install(&payload, &receipt.directory, &receipt.commands)
    }

    pub fn checksum(name: &str, contents: &str) -> Result<String, ToolError> {
        let matches: Vec<&str> = contents
            .lines()
            .filter_map(|line| {
                let fields: Vec<&str> = line.split_whitespace().collect();
                if fields.len() == 2
                    && fields[1] == name
                    && fields[0].len() == 64
                    && fields[0].bytes().all(|byte| byte.is_ascii_hexdigit())
                {
                    Some(fields[0])
                } else {
                    None
                }
            })
            .collect();
        if matches.len() != 1 {
            return Err(ToolError::invalid(format!(
                "Release has no unique checksum for {name}"
            )));
        }
        Ok(matches[0].to_string())
    }
}

pub fn executable_name() -> &'static str {
    if cfg!(windows) {
        "cove.exe"
    } else {
        "cove"
    }
}

/// `cove internal package-cli`: builds and stages a complete SDK payload.
pub struct CLIArchive;

impl CLIArchive {
    pub fn create(root: &Path, output: &Path) -> Result<(), ToolError> {
        let package = root.join("Tools/Cove");
        let build = vec![
            CoveSDK::swift(),
            "build".into(),
            "--package-path".into(),
            package.display().to_string(),
            "--configuration".into(),
            "release".into(),
            "--product".into(),
            "cove".into(),
            "--jobs".into(),
            "4".into(),
        ];
        Shell::run(
            &build,
            None,
            &environment(),
            false,
            false,
            StdoutTarget::StandardOutput,
        )?;
        let mut path_command = build.clone();
        path_command.push("--show-bin-path".into());
        let binaries = PathBuf::from(
            Shell::run(
                &path_command,
                None,
                &environment(),
                true,
                true,
                StdoutTarget::StandardOutput,
            )?
            .trim(),
        );
        let staging = std::env::temp_dir().join(format!("cove-package-{}", unique_token()));
        let result = (|| {
            let payload = staging.join("cove");
            Files::directory(&payload)?;
            SDKPayload::stage_sources(root, &payload)?;
            NativeSDK::prepare(root, None, false)?;
            for path in NativeArtifacts::bundles_for(&CLIUpdate::platform()) {
                let relative = format!("{path}.artifactbundle");
                Files::copy(&root.join(&relative), &payload.join(&relative))?;
            }
            let manifest = root.join("Artifacts/native.json");
            if !manifest.is_file() {
                return Err(ToolError::invalid(
                    "Merge native release metadata into Artifacts/native.json before packaging the SDK",
                ));
            }
            Files::copy(&manifest, &payload.join("Artifacts/native.json"))?;
            let executable = binaries.join(executable_name());
            Files::copy(&executable, &payload.join(executable_name()))?;
            for file in Files::children(&binaries)? {
                let extension = file
                    .extension()
                    .map(|extension| extension.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if extension == "bundle" || extension == "resources" {
                    let name = file.file_name().unwrap_or_default();
                    Files::copy(&file, &payload.join(name))?;
                }
            }
            CLIRuntimeLibraries::stage(&executable, &payload)?;
            Files::text(
                &format!("{}\n", SDKRelease::current()?.version),
                &payload.join("VERSION"),
            )?;
            SDKPayload::validate(&payload, &SDKRelease::read(root)?.version)?;
            Shell::run(
                &[
                    payload.join(executable_name()).display().to_string(),
                    "--version".into(),
                ],
                None,
                &environment(),
                false,
                false,
                StdoutTarget::StandardOutput,
            )?;
            Files::directory(output)?;
            let archive = output.join(format!("cove-{}.tar.gz", CLIUpdate::platform()));
            let tar = TarArchive::executable();
            Shell::run(
                &[
                    tar,
                    "-C".into(),
                    staging.display().to_string(),
                    "-czf".into(),
                    archive.display().to_string(),
                    "cove".into(),
                ],
                None,
                &environment(),
                false,
                false,
                StdoutTarget::StandardOutput,
            )?;
            let checksum = sha256::hex_file(&archive)
                .map_err(|error| ToolError::invalid(error.to_string()))?;
            Files::text(
                &format!(
                    "{checksum}  {}\n",
                    archive.file_name().unwrap_or_default().to_string_lossy()
                ),
                &output.join("SHA256SUMS"),
            )?;
            println!("{}", archive.display());
            Ok(())
        })();
        let _ = Files::remove(&staging);
        result
    }
}

/// `cove internal package-native`: archives the host (or iOS/Android) bundles.
pub struct NativeArchive;

impl NativeArchive {
    pub fn create(root: &Path, output: &Path, arguments: &Arguments) -> Result<(), ToolError> {
        let target: String;
        let runtime: PathBuf;
        let text: PathBuf;
        if arguments.platform == AppPlatform::Android {
            target = "android-arm64".into();
            let toolchain = AndroidToolchain::new(root, arguments)?;
            let artifacts = NativeDependencies::new(toolchain).prepare(true)?;
            runtime = artifacts.engine;
            text = artifacts.text;
        } else {
            let apple = if arguments.platform == AppPlatform::Ios {
                let sdk = arguments
                    .value(OptionKey::Sdk)
                    .unwrap_or("iphonesimulator")
                    .to_string();
                let architecture = arguments
                    .value(OptionKey::Architecture)
                    .unwrap_or("arm64")
                    .to_string();
                if !["iphoneos", "iphonesimulator"].contains(&sdk.as_str())
                    || !["arm64", "x86_64"].contains(&architecture.as_str())
                    || (sdk == "iphoneos" && architecture != "arm64")
                {
                    return Err(ToolError::invalid("Invalid iOS native target"));
                }
                Some(NativeSDK::AppleTarget {
                    sdk,
                    architecture,
                })
            } else {
                None
            };
            target = apple
                .as_ref()
                .map(NativeSDK::AppleTarget::variant)
                .unwrap_or_else(CLIUpdate::platform);
            NativeSDK::prepare(root, apple, true)?;
            let base = NativeArtifacts::directory(root, &target);
            runtime = base.join("Native/vendor");
            text = base.join("vendor");
        }
        let staging = std::env::temp_dir().join(format!("cove-native-{}", unique_token()));
        let result = (|| {
            for path in NativeArtifacts::bundles_for(&target) {
                let name = Path::new(&path)
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let source = if path.starts_with("Native/") {
                    &runtime
                } else {
                    &text
                };
                Files::copy(
                    &source.join(format!("{name}.artifactbundle")),
                    &staging.join(format!("{path}.artifactbundle")),
                )?;
            }
            let release = SDKRelease::read(root)?;
            Files::text(&format!("{}\n", release.version), &staging.join("VERSION"))?;
            Files::directory(output)?;
            let name = format!("cove-native-{target}.tar.gz");
            let archive = output.join(&name);
            Shell::run(
                &[
                    TarArchive::executable(),
                    "-C".into(),
                    staging.display().to_string(),
                    "-czf".into(),
                    archive.display().to_string(),
                    ".".into(),
                ],
                None,
                &environment(),
                false,
                false,
                StdoutTarget::StandardOutput,
            )?;
            let repository = release.repository.replace(".git", "");
            let checksum =
                sha256::hex_file(&archive).map_err(|error| ToolError::invalid(error.to_string()))?;
            let mut platforms = std::collections::BTreeMap::new();
            platforms.insert(
                target.clone(),
                NativeArchiveMetadata {
                    url: format!(
                        "{repository}/releases/download/v{}/{name}",
                        release.version
                    ),
                    sha256: checksum,
                },
            );
            Files::json(
                &NativeArtifacts {
                    version: release.version,
                    platforms,
                },
                &output.join(format!("native-{target}.json")),
            )?;
            println!("{}", archive.display());
            Ok(())
        })();
        let _ = Files::remove(&staging);
        result
    }
}

/// Installs a validated CLI payload under `<directory>/versions/<version>`.
pub struct CLIInstaller;

impl CLIInstaller {
    pub fn install(payload: &Path, directory: &Path, commands: &Path) -> Result<(), ToolError> {
        let version = std::fs::read_to_string(payload.join("VERSION"))
            .map_err(|error| ToolError::invalid(error.to_string()))?
            .trim()
            .to_string();
        if !SDKRelease::is_version(&version) {
            return Err(ToolError::invalid("Invalid CLI version in installation payload"));
        }
        SDKPayload::validate(payload, &version)?;
        let lock = FileLock::acquire(
            &directory
                .join("locks")
                .join(format!("sdk-{version}.lock")),
        )
        .map_err(|error| ToolError::invalid(error.to_string()))?;
        let result = Self::install_locked(payload, directory, commands, &version);
        drop(lock);
        result
    }

    fn install_locked(
        payload: &Path,
        directory: &Path,
        commands: &Path,
        version: &str,
    ) -> Result<(), ToolError> {
        let binary = payload.join(executable_name());
        if !binary.is_file() {
            return Err(ToolError::invalid("CLI payload is missing its executable"));
        }
        let receipt = InstallationReceipt {
            version: version.to_string(),
            directory: directory.to_path_buf(),
            commands: commands.to_path_buf(),
        };
        let destination = directory.join("versions").join(version);
        if let Some(parent) = destination.parent() {
            Files::directory(parent)?;
        }
        if destination.exists() {
            let existing = InstallationReceipt::read(&destination)?;
            if existing.version != version
                || !same_path(&existing.directory, directory)
                || !same_path(&existing.commands, commands)
            {
                return Err(ToolError::invalid(
                    "Installation directory already contains an unmanaged version",
                ));
            }
            SDKPayload::validate(&destination, version)?;
        } else {
            let staging = directory
                .join("versions")
                .join(format!(".{}", unique_token()));
            let _ = Files::remove(&staging);
            Files::copy(payload, &staging)?;
            Files::json(&receipt, &staging.join(InstallationReceipt::FILENAME))?;
            std::fs::rename(&staging, &destination)
                .map_err(|error| ToolError::invalid(error.to_string()))?;
        }
        Self::activate(&destination, commands, directory)?;
        Files::json(
            &receipt,
            &directory.join(InstallationReceipt::FILENAME),
        )?;
        SDKStore::new(Some(directory.to_path_buf())).select(&destination)?;
        println!("Installed Cove SDK {version}: {}", commands.display());
        Ok(())
    }

    fn activate(version: &Path, commands: &Path, directory: &Path) -> Result<(), ToolError> {
        Files::directory(commands)?;
        if cfg!(windows) {
            if commands.join("cove.cmd").is_file() {
                let existing = InstallationReceipt::read(directory)?;
                if !same_path(&existing.directory, directory)
                    || !same_path(&existing.commands, commands)
                {
                    return Err(ToolError::invalid(
                        "The cove command is managed by another installation",
                    ));
                }
            }
            let executable = version
                .join("cove.exe")
                .display()
                .to_string()
                .replace('%', "%%");
            Files::text(
                &format!("@\"{executable}\" %*\r\n"),
                &commands.join("cove.cmd"),
            )?;
            return Ok(());
        }
        #[cfg(unix)]
        {
            let entry = commands.join("cove");
            if entry.exists() {
                let resolved = entry.canonicalize().unwrap_or_else(|_| entry.clone());
                let owner = resolved
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from("."));
                let existing = InstallationReceipt::read(&owner)?;
                if !same_path(&existing.directory, directory) {
                    return Err(ToolError::invalid(
                        "The cove command is managed by another installation",
                    ));
                }
            }
            let temporary = commands.join(format!(".cove-{}", unique_token()));
            let _ = Files::remove(&temporary);
            std::os::unix::fs::symlink(version.join("cove"), &temporary)
                .map_err(|error| ToolError::invalid(error.to_string()))?;
            std::fs::rename(&temporary, &entry).map_err(|error| {
                let _ = Files::remove(&temporary);
                ToolError::invalid(format!(
                    "Could not activate the new cove command: {error}"
                ))
            })?;
        }
        Ok(())
    }
}

fn same_path(lhs: &Path, rhs: &Path) -> bool {
    let left = lhs.canonicalize().unwrap_or_else(|_| standardized(lhs));
    let right = rhs.canonicalize().unwrap_or_else(|_| standardized(rhs));
    left == right
}

/// Copies an SDK payload's sources, excluding build output and vendored trees.
pub struct SDKPayload;

impl SDKPayload {
    pub fn stage_sources(root: &Path, payload: &Path) -> Result<(), ToolError> {
        const PATHS: [&str; 27] = [
            "Package.swift",
            "LICENSE",
            "NOTICE",
            "README.md",
            "CHANGELOG.md",
            "Sources",
            "Tests",
            "Core",
            "Compose",
            "Text",
            "Imaging",
            "Graphics",
            "Shell",
            "Runtime",
            "PlatformCore",
            "System",
            "Logging",
            "Hosts",
            "Tools",
            "Platforms",
            "Terminal",
            "Examples",
            "DevTools",
            "Docs",
            "Artifacts",
            "Native",
            "third-party",
        ];
        const EXCLUDED: [&str; 7] = [".git", ".build", ".cove", "build", "vendor", "node_modules", "dist"];
        for path in PATHS {
            let source = root.join(path);
            if !source.exists() {
                continue;
            }
            if source.is_file() {
                Files::copy(&source, &payload.join(path))?;
                continue;
            }
            Files::directory(&payload.join(path))?;
            copy_tree(&source, root, payload, &EXCLUDED)?;
        }
        // SDL's Java host is needed by Gradle; native dependency source trees
        // are not shipped.
        let java = "Native/third-party/sdl3/android-project/app/src/main/java";
        if root.join(java).exists() {
            Files::copy(&root.join(java), &payload.join(java))?;
        }
        Ok(())
    }

    pub fn validate(payload: &Path, version: &str) -> Result<(), ToolError> {
        let manifest_file = payload.join("Artifacts/native.json");
        let data = std::fs::read(&manifest_file)
            .map_err(|error| ToolError::invalid(format!("{}: {error}", manifest_file.display())))?;
        let manifest: NativeArtifacts = serde_json::from_slice(&data)
            .map_err(|error| ToolError::invalid(format!("{}: {error}", manifest_file.display())))?;
        let platform = CLIUpdate::platform();
        let version_file = std::fs::read_to_string(payload.join("VERSION"))
            .map_err(|error| ToolError::invalid(error.to_string()))?;
        let complete = CoveSDK::is_sdk(payload)
            && SDKRelease::read(payload)
                .map(|release| release.version == version)
                .unwrap_or(false)
            && manifest.version == version
            && manifest.platforms.contains_key(&platform)
            && version_file.trim() == version
            && Self::executable(payload).is_file()
            && NativeArtifacts::is_prepared(payload, &platform);
        if !complete {
            return Err(ToolError::invalid(format!(
                "Incomplete Cove SDK {version}: CLI, sources and host native libraries are required"
            )));
        }
        Ok(())
    }

    pub fn executable(root: &Path) -> PathBuf {
        root.join(executable_name())
    }
}

fn copy_tree(
    directory: &Path,
    root: &Path,
    payload: &Path,
    excluded: &[&str],
) -> Result<(), ToolError> {
    for entry in std::fs::read_dir(directory)
        .map_err(|error| ToolError::invalid(error.to_string()))?
    {
        let entry = entry.map_err(|error| ToolError::invalid(error.to_string()))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if excluded.contains(&name.as_str()) {
            continue;
        }
        let destination = payload.join(Files::relative(&path, root));
        if path.is_dir() {
            Files::directory(&destination)?;
            copy_tree(&path, root, payload, excluded)?;
        } else {
            Files::copy(&path, &destination)?;
        }
    }
    Ok(())
}

/// Archive extraction guarded against absolute and parent-directory entries.
pub struct TarArchive;

impl TarArchive {
    pub fn executable() -> String {
        if cfg!(windows) {
            std::env::var("SystemRoot")
                .unwrap_or_else(|_| "C:\\Windows".into())
                .replace('\\', "/")
                + "/System32/tar.exe"
        } else {
            "tar".into()
        }
    }

    pub fn extract(archive: &Path, directory: &Path) -> Result<(), ToolError> {
        Files::directory(directory)?;
        let entries = Shell::run(
            &[
                Self::executable(),
                "-tf".into(),
                archive.display().to_string(),
            ],
            None,
            &environment(),
            true,
            true,
            StdoutTarget::StandardOutput,
        )?;
        let safe = entries.lines().all(|entry| {
            !entry.starts_with('/')
                && !entry.contains('\\')
                && !entry.contains(':')
                && !entry.split('/').any(|component| component == "..")
        });
        if !safe {
            return Err(ToolError::invalid("Archive contains an invalid path"));
        }
        Shell::run(
            &[
                Self::executable(),
                "-xf".into(),
                archive.display().to_string(),
                "-C".into(),
                directory.display().to_string(),
            ],
            None,
            &environment(),
            false,
            false,
            StdoutTarget::StandardOutput,
        )?;
        Ok(())
    }
}

/// Stages the runtime libraries the CLI needs beside its executable.
pub struct CLIRuntimeLibraries;

impl CLIRuntimeLibraries {
    pub fn stage(executable: &Path, payload: &Path) -> Result<(), ToolError> {
        if cfg!(windows) {
            let search_path = environment()
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case("PATH"))
                .map(|(_, value)| value.clone())
                .unwrap_or_default();
            let executable_directory = executable
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            let mut paths: Vec<PathBuf> = vec![executable_directory];
            paths.extend(std::env::split_paths(&search_path));
            let prefixes = [
                "swift",
                "BlocksRuntime",
                "dispatch",
                "Foundation",
                "CoreFoundation",
                "_Foundation",
                "icu",
                "libxml2",
                "curl",
                "zlib",
                "vcruntime",
                "msvcp",
                "concrt",
            ];
            let mut copied: BTreeSet<String> = BTreeSet::new();
            for path in paths {
                if path.as_os_str().is_empty() {
                    continue;
                }
                let Ok(files) = Files::children(&path) else {
                    continue;
                };
                for file in files {
                    let name = file
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let lower = name.to_ascii_lowercase();
                    let matches = lower.ends_with(".dll")
                        && prefixes.iter().any(|prefix| {
                            lower.starts_with(&prefix.to_ascii_lowercase())
                        });
                    if matches && copied.insert(lower) {
                        Files::copy(&file, &payload.join(&name))?;
                    }
                }
            }
            return Ok(());
        }
        if cfg!(target_os = "linux") {
            let listing = Shell::run(
                &[
                    "ldd".into(),
                    executable.display().to_string(),
                ],
                None,
                &environment(),
                true,
                true,
                StdoutTarget::StandardOutput,
            )?;
            if listing.contains("not found") {
                return Err(ToolError::invalid("CLI has unresolved runtime libraries"));
            }
            let libraries = payload.join("lib");
            Files::directory(&libraries)?;
            let system = [
                "libc.so",
                "libm.so",
                "libpthread.so",
                "libdl.so",
                "librt.so",
                "ld-linux",
            ];
            for line in listing.lines() {
                let Some(component) = line
                    .split_whitespace()
                    .find(|component| component.starts_with('/'))
                else {
                    continue;
                };
                let file = PathBuf::from(component);
                let name = file.file_name().unwrap_or_default().to_string_lossy().into_owned();
                if system.iter().any(|prefix| name.starts_with(prefix)) {
                    continue;
                }
                let destination = libraries.join(&name);
                if !destination.exists() {
                    let resolved = file.canonicalize().unwrap_or_else(|_| file.clone());
                    Files::copy(&resolved, &destination)?;
                    Shell::run(
                        &[
                            "patchelf".into(),
                            "--set-rpath".into(),
                            "$ORIGIN".into(),
                            destination.display().to_string(),
                        ],
                        None,
                        &environment(),
                        false,
                        false,
                        StdoutTarget::StandardOutput,
                    )?;
                }
            }
            Shell::run(
                &[
                    "patchelf".into(),
                    "--set-rpath".into(),
                    "$ORIGIN/lib".into(),
                    payload.join(executable_name()).display().to_string(),
                ],
                None,
                &environment(),
                false,
                false,
                StdoutTarget::StandardOutput,
            )?;
            return Ok(());
        }
        let listing = Shell::run(
            &[
                "otool".into(),
                "-L".into(),
                executable.display().to_string(),
            ],
            None,
            &environment(),
            true,
            true,
            StdoutTarget::StandardOutput,
        )?;
        for line in listing.lines().skip(1) {
            let Some(name) = line.split_whitespace().next() else {
                continue;
            };
            if !(name.starts_with("/usr/lib/") || name.starts_with("/System/Library/")) {
                return Err(ToolError::invalid(format!(
                    "CLI requires an unbundled library: {name}"
                )));
            }
        }
        Ok(())
    }
}
