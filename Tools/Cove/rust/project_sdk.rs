//! SDK selection, releases, stores and native artifact installation.
//!
//! Swift sources: `Project/SDK/Artifacts.swift`, `Project/SDK/Command.swift`,
//! `Project/SDK/Download.swift`, `Project/SDK/Location.swift`,
//! `Project/SDK/Release.swift`, `Project/SDK/Store.swift`.
//!
//! `Bundle.module` has no Rust counterpart; `SDKRelease::current` resolves the
//! checked-in `SDK.json` from `COVE_SDK_ROOT` or the nearest enclosing
//! checkout.

use crate::cli::{standardized, Arguments, Operation, OptionKey};
use crate::download;
use crate::installation::{
    CLIUpdate, Installation, InstallationReceipt, SDKPayload, TarArchive,
};
use crate::project::AppProject;
use crate::support::{environment, Files, Shell, StdoutTarget, ToolError};
use cove_system::files::FileLock;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Numeric ordering used by Swift's `compare(options: .numeric)`.
pub fn numeric_compare(lhs: &str, rhs: &str) -> std::cmp::Ordering {
    fn tokens(value: &str) -> Vec<(bool, String)> {
        let mut result: Vec<(bool, String)> = Vec::new();
        let mut current = String::new();
        let mut digits = false;
        for character in value.chars() {
            let is_digit = character.is_ascii_digit();
            if !current.is_empty() && is_digit != digits {
                result.push((digits, std::mem::take(&mut current)));
            }
            digits = is_digit;
            current.push(character);
        }
        if !current.is_empty() {
            result.push((digits, current));
        }
        result
    }
    fn compare_numbers(lhs: &str, rhs: &str) -> std::cmp::Ordering {
        let lhs = lhs.trim_start_matches('0');
        let rhs = rhs.trim_start_matches('0');
        lhs.len()
            .cmp(&rhs.len())
            .then_with(|| lhs.cmp(rhs))
    }
    let left = tokens(lhs);
    let right = tokens(rhs);
    for (lhs, rhs) in left.iter().zip(right.iter()) {
        let ordering = match (lhs.0, rhs.0) {
            (true, true) => compare_numbers(&lhs.1, &rhs.1),
            (false, false) => lhs.1.cmp(&rhs.1),
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
        };
        if ordering != std::cmp::Ordering::Equal {
            return ordering;
        }
    }
    left.len().cmp(&right.len())
}

/// `Tools/Cove/Sources/CoveCLI/SDK.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SDKRelease {
    pub version: String,
    pub swift: String,
    pub repository: String,
}

impl SDKRelease {
    pub const MANIFEST: &'static str = "Tools/Cove/Sources/CoveCLI/SDK.json";
    pub const LATEST_TAG: &'static str = "latest";

    pub fn current() -> Result<Self, ToolError> {
        if let Some(root) = std::env::var_os("COVE_SDK_ROOT") {
            if Path::new(&root).join(Self::MANIFEST).is_file() {
                return Self::read(Path::new(&root));
            }
        }
        let mut candidates = Vec::new();
        if let Ok(executable) = std::env::current_exe() {
            candidates.push(executable);
        }
        if let Ok(directory) = std::env::current_dir() {
            candidates.push(directory);
        }
        for candidate in candidates {
            let mut directory = candidate.as_path();
            loop {
                if directory.join(Self::MANIFEST).is_file() {
                    return Self::read(directory);
                }
                match directory.parent() {
                    Some(parent) if parent != directory => directory = parent,
                    _ => break,
                }
            }
        }
        Err(ToolError::invalid(
            "Cannot locate the Cove SDK manifest; set COVE_SDK_ROOT to a checkout",
        ))
    }

    pub fn read(root: &Path) -> Result<Self, ToolError> {
        let file = root.join(Self::MANIFEST);
        let data = std::fs::read(&file)
            .map_err(|error| ToolError::invalid(format!("{}: {error}", file.display())))?;
        serde_json::from_slice(&data)
            .map_err(|error| ToolError::invalid(format!("{}: {error}", file.display())))
    }

    pub fn is_version(value: &str) -> bool {
        crate::project::is_dotted_version(value)
    }

    pub fn is_version_requirement(value: &str) -> bool {
        Self::is_version(value) || value == Self::LATEST_TAG
    }

    /// Resolves GitHub's "latest" release pointer by following the redirect
    /// from /releases/latest to /releases/tag/vX.Y.Z.
    pub fn latest_version(repository: &str) -> Result<String, ToolError> {
        let base = repository.replace(".git", "");
        let resolved = download::redirected_url(&format!("{base}/releases/latest"))?;
        let tag = resolved
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or_default();
        let version = tag.strip_prefix('v').unwrap_or(tag);
        if !Self::is_version(version) {
            return Err(ToolError::invalid(format!(
                "The release channel did not resolve to a version: {tag}"
            )));
        }
        Ok(version.to_string())
    }
}

/// Native artifact bundles published beside an SDK release.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeArtifacts {
    pub version: String,
    pub platforms: BTreeMap<String, NativeArchive>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeArchive {
    pub url: String,
    pub sha256: String,
}

impl NativeArtifacts {
    pub fn directory(root: &Path, target: &str) -> PathBuf {
        if target == CLIUpdate::platform() {
            root.to_path_buf()
        } else {
            root.join("Artifacts/native").join(target)
        }
    }

    pub fn bundles_for(target: &str) -> Vec<String> {
        let mut bundles = vec![
            "Native/vendor/SDL3".to_string(),
            "Native/vendor/lunasvg".to_string(),
            "Native/vendor/plutovg".to_string(),
            "Native/vendor/webp".to_string(),
            "Native/vendor/sharpyuv".to_string(),
            "vendor/CFreeType".to_string(),
            "vendor/CHarfBuzz".to_string(),
        ];
        if !(target.starts_with("ios-") || target.starts_with("android-")) {
            bundles.push("vendor/GhosttyVt".to_string());
        }
        bundles
    }

    pub fn is_prepared(root: &Path, target: &str) -> bool {
        let base = Self::directory(root, target);
        Self::bundles_for(target).iter().all(|bundle| {
            base.join(format!("{bundle}.artifactbundle/info.json"))
                .is_file()
        })
    }

    /// Packaged SDKs deliberately omit native sources. A checkout must
    /// bootstrap itself without downloading the release it is building.
    pub fn has_source_inputs(root: &Path) -> bool {
        ["Native/third-party/CMakeLists.txt", "third-party/CMakeLists.txt"]
            .iter()
            .all(|path| root.join(path).is_file())
    }

    /// Returns whether the host bundles are now prepared.
    pub fn install(
        root: &Path,
        target: &str,
        cache_directory: &Path,
    ) -> Result<bool, ToolError> {
        let lock = FileLock::acquire(&root.join(format!("Artifacts/locks/{target}.lock")))
            .map_err(|error| ToolError::invalid(error.to_string()))?;
        let result = Self::install_locked(root, target, cache_directory);
        drop(lock);
        result
    }

    fn install_locked(
        root: &Path,
        target: &str,
        cache_directory: &Path,
    ) -> Result<bool, ToolError> {
        if Self::is_prepared(root, target) {
            return Ok(true);
        }
        if Self::has_source_inputs(root) {
            return Ok(false);
        }
        let file = root.join("Artifacts/native.json");
        if !file.is_file() {
            return Ok(false);
        }
        let data = std::fs::read(&file)
            .map_err(|error| ToolError::invalid(format!("{}: {error}", file.display())))?;
        let manifest: Self = serde_json::from_slice(&data)
            .map_err(|error| ToolError::invalid(format!("{}: {error}", file.display())))?;
        let archive = manifest
            .platforms
            .get(target)
            .filter(|archive| is_checksum(&archive.sha256));
        let archive = match archive {
            Some(archive) if manifest.version == SDKRelease::read(root)?.version => archive,
            _ => {
                return Err(ToolError::invalid(format!(
                    "This SDK has no native libraries for {target}; install a complete SDK release"
                )))
            }
        };
        let cache = cache_directory
            .join("cache/native")
            .join(&archive.sha256);
        let tar = cache.join("native.tar.gz");
        download::file(&archive.url, &tar, &archive.sha256)?;
        let staging = cache.join(unique_token());
        let _ = Files::remove(&staging);
        let result = (|| {
            TarArchive::extract(&tar, &staging)?;
            let version = std::fs::read_to_string(staging.join("VERSION"))
                .map_err(|error| ToolError::invalid(error.to_string()))?
                .trim()
                .to_string();
            if version != manifest.version {
                return Err(ToolError::invalid(
                    "Native archive and SDK versions disagree",
                ));
            }
            let base = Self::directory(root, target);
            for path in Self::bundles_for(target) {
                let relative = format!("{path}.artifactbundle");
                let source = staging.join(&relative);
                if !source.join("info.json").is_file() {
                    return Err(ToolError::invalid(format!(
                        "Native archive is missing {relative}"
                    )));
                }
                let destination = base.join(&relative);
                Files::directory(destination.parent().unwrap_or(&base))?;
                // Parallel project builds may prepare the same SDK. Never
                // replace a published bundle.
                match std::fs::rename(&source, &destination) {
                    Ok(()) => {}
                    Err(error) => {
                        if !destination.join("info.json").is_file() {
                            return Err(ToolError::invalid(error.to_string()));
                        }
                    }
                }
            }
            Ok(true)
        })();
        let _ = Files::remove(&staging);
        result
    }
}

fn is_checksum(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn unique_token() -> String {
    format!(
        "{:x}-{:x}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or_default()
    )
}

/// Downloads a released SDK payload and validates it before installation.
pub struct SDKDownload;

impl SDKDownload {
    pub fn fetch(version: &str, staging: &Path) -> Result<PathBuf, ToolError> {
        let repository = SDKRelease::current()?
            .repository
            .replace(".git", "");
        let base = format!("{repository}/releases/download/v{version}/");
        let name = format!("cove-{}.tar.gz", CLIUpdate::platform());
        Files::directory(staging)?;
        let sums = staging.join("SHA256SUMS");
        download::metadata(&format!("{base}SHA256SUMS"), &sums)?;
        let contents = std::fs::read_to_string(&sums)
            .map_err(|error| ToolError::invalid(format!("{}: {error}", sums.display())))?;
        let digest = CLIUpdate::checksum(&name, &contents)?;
        let archive = staging.join(&name);
        download::file(&format!("{base}{name}"), &archive, &digest)?;
        let expanded = staging.join("expanded");
        TarArchive::extract(&archive, &expanded)?;
        let payload = expanded.join("cove");
        SDKPayload::validate(&payload, version)?;
        Ok(payload)
    }
}

/// Locates the SDK a command should use.
pub struct CoveSDK;

impl CoveSDK {
    pub fn swift() -> String {
        std::env::var("COVE_SWIFT_EXECUTABLE").unwrap_or_else(|_| "swift".into())
    }

    pub fn locate(
        arguments: &Arguments,
        project: Option<&AppProject>,
        environment: &BTreeMap<String, String>,
    ) -> Result<PathBuf, ToolError> {
        let owned_project;
        let project = match project {
            Some(project) => Some(project),
            None => {
                owned_project = Self::project_if_present(&arguments.directory())?;
                owned_project.as_ref()
            }
        };
        let store = SDKStore::new(None);
        let local = project.and_then(|project| {
            SDKStore::selection(&project.directory.join(".cove/sdk.json"))
        });
        let required = project.map(|project| project.configuration.toolchain.sdk_version.clone());
        let selected = store.selected().filter(|root| {
            required.is_none()
                || SDKRelease::read(root)
                    .map(|release| Some(release.version) == required)
                    .unwrap_or(false)
        });
        let override_path = arguments.url(OptionKey::SdkRoot).or_else(|| {
            environment.get("COVE_SDK_ROOT").map(|value| standardized(value))
        });
        let explicit = override_path.clone().or(local);
        let executable_directory = InstallationReceipt::executable_directory();
        let bundled = Self::is_sdk(&executable_directory).then(|| executable_directory.clone());
        let development = std::env::current_exe()
            .ok()
            .and_then(|executable| Self::checkout_containing(&executable));
        let mut root: Option<PathBuf>;
        if required.as_deref() == Some(SDKRelease::LATEST_TAG) {
            // "latest" floats to the newest installed SDK; the explicit
            // override always wins.
            root = explicit
                .or_else(|| Self::newest_installed(&store))
                .or(bundled);
        } else {
            let pinned = required
                .as_ref()
                .map(|version| store.installed(version))
                .filter(|path| Self::is_sdk(path));
            let own = [bundled, development].into_iter().flatten().find(|root| {
                required.is_none()
                    || SDKRelease::read(root)
                        .map(|release| Some(release.version) == required)
                        .unwrap_or(false)
            });
            root = explicit.or(selected).or(pinned).or(own);
        }
        if root.is_none() {
            if let Some(required) = &required {
                let version = store.install(required)?;
                root = Some(store.installed(&version));
            }
        }
        let Some(root) = root.filter(|root| Self::is_sdk(root)) else {
            return Err(ToolError::invalid(format!(
                "Install the SDK with cove sdk install {}, or select a checkout with --sdk-root",
                required.unwrap_or_else(|| "VERSION".into())
            )));
        };
        let release = SDKRelease::read(&root)?;
        if override_path.is_none() {
            if let Some(required) = &required {
                if required != SDKRelease::LATEST_TAG && &release.version != required {
                    return Err(ToolError::invalid(format!(
                        "Project requires SDK {required}; selected checkout provides {}",
                        release.version
                    )));
                }
            }
        }
        Ok(root)
    }

    fn project_if_present(directory: &Path) -> Result<Option<AppProject>, ToolError> {
        if !directory.join("cove.json").is_file() {
            return Ok(None);
        }
        AppProject::new(directory).map(Some)
    }

    /// Highest-version SDK among installed copies and the current selection;
    /// ties keep the explicit selection.
    fn newest_installed(store: &SDKStore) -> Option<PathBuf> {
        let mut candidates: Vec<PathBuf> = store.selected().into_iter().collect();
        candidates.extend(store.installed_urls());
        candidates.retain(|candidate| Self::is_sdk(candidate));
        candidates.into_iter().max_by(|lhs, rhs| {
            let left = SDKRelease::read(lhs)
                .map(|release| release.version)
                .unwrap_or_else(|_| "0.0.0".into());
            let right = SDKRelease::read(rhs)
                .map(|release| release.version)
                .unwrap_or_else(|_| "0.0.0".into());
            numeric_compare(&left, &right)
        })
    }

    pub fn is_sdk(root: &Path) -> bool {
        SDKRelease::read(root).is_ok() && root.join("Package.swift").is_file()
    }

    fn checkout_containing(source: &Path) -> Option<PathBuf> {
        let mut directory = source.parent()?.to_path_buf();
        loop {
            if Self::is_sdk(&directory) {
                return Some(directory);
            }
            let Some(parent) = directory.parent() else {
                return None;
            };
            if parent == directory {
                return None;
            }
            directory = parent.to_path_buf();
        }
    }
}

/// Installed SDK versions and the selected one.
pub struct SDKStore {
    pub directory: PathBuf,
}

impl SDKStore {
    pub fn new(directory: Option<PathBuf>) -> Self {
        Self {
            directory: directory.unwrap_or_else(Installation::directory),
        }
    }

    pub fn selected(&self) -> Option<PathBuf> {
        Self::selection(&self.directory.join("sdk.json"))
    }

    pub fn installed(&self, version: &str) -> PathBuf {
        self.directory.join("versions").join(version)
    }

    /// Each version contains its CLI, framework sources and host libraries.
    pub fn installed_urls(&self) -> Vec<PathBuf> {
        let parent = self.directory.join("versions");
        Files::children(&parent)
            .unwrap_or_default()
            .into_iter()
            .filter(|child| {
                child
                    .file_name()
                    .map(|name| !name.to_string_lossy().starts_with('.'))
                    .unwrap_or(false)
            })
            .filter(|child| CoveSDK::is_sdk(child))
            .collect()
    }

    pub fn selection(at: &Path) -> Option<PathBuf> {
        let data = std::fs::read(at).ok()?;
        let selection: Selection = serde_json::from_slice(&data).ok()?;
        Some(PathBuf::from(selection.path))
    }

    pub fn select(&self, root: &Path) -> Result<(), ToolError> {
        if !CoveSDK::is_sdk(root) {
            return Err(ToolError::invalid(format!(
                "Not a Cove SDK: {}",
                root.display()
            )));
        }
        Files::json(
            &Selection {
                path: standardized(root).display().to_string(),
            },
            &self.directory.join("sdk.json"),
        )?;
        println!("Selected Cove SDK: {}", root.display());
        Ok(())
    }

    pub fn list(&self) -> Result<(), ToolError> {
        let installed = self.directory.join("versions");
        if installed.exists() {
            for sdk in Files::children(&installed)?
                .into_iter()
                .filter(|sdk| CoveSDK::is_sdk(sdk))
            {
                println!("{}", sdk.display());
            }
        }
        match self.selected() {
            Some(path) => println!("Selected: {}", path.display()),
            None => println!("Selected: none"),
        }
        Ok(())
    }

    /// Installs the SDK; "latest" is resolved first. Returns the concrete
    /// version that was installed.
    pub fn install(&self, requested: &str) -> Result<String, ToolError> {
        let repository = SDKRelease::current()?.repository;
        let version = if requested == SDKRelease::LATEST_TAG {
            SDKRelease::latest_version(&repository)?
        } else {
            requested.to_string()
        };
        if !SDKRelease::is_version(&version) {
            return Err(ToolError::invalid(
                "Use cove sdk install VERSION or latest",
            ));
        }
        let lock = FileLock::acquire(
            &self
                .directory
                .join("locks")
                .join(format!("sdk-{version}.lock")),
        )
        .map_err(|error| ToolError::invalid(error.to_string()))?;
        let target = self.installed(&version);
        if CoveSDK::is_sdk(&target) {
            drop(lock);
            return Ok(version);
        }
        let staging = self.directory.join("downloads").join(unique_token());
        if let Some(parent) = staging.parent() {
            Files::directory(parent)?;
        }
        let result = (|| {
            let payload = SDKDownload::fetch(&version, &staging)?;
            Files::directory(target.parent().unwrap_or(&self.directory))?;
            let commands = InstallationReceipt::read(&self.directory)
                .map(|receipt| receipt.commands)
                .unwrap_or_else(|_| Installation::commands());
            let receipt = InstallationReceipt {
                version: version.clone(),
                directory: self.directory.clone(),
                commands,
            };
            Files::json(&receipt, &payload.join(InstallationReceipt::FILENAME))?;
            match std::fs::rename(&payload, &target) {
                Ok(()) => {}
                Err(error) => {
                    if !CoveSDK::is_sdk(&target) {
                        return Err(ToolError::invalid(error.to_string()));
                    }
                }
            }
            if self.selected().is_none() {
                self.select(&target)?;
            }
            println!("Installed Cove SDK {version}: {}", target.display());
            Ok(version.clone())
        })();
        let _ = Files::remove(&staging);
        drop(lock);
        result
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Selection {
    path: String,
}

/// Dispatches commands to the project's selected SDK CLI.
pub struct SDKCommand;

impl SDKCommand {
    /// Select the project's tool before interpreting flags introduced by a
    /// newer SDK.
    #[allow(dead_code)]
    pub fn bootstrap(raw: &[String]) -> Result<(), ToolError> {
        let Some(verb) = raw.first() else {
            return Ok(());
        };
        if ["self", "sdk", "internal"].contains(&verb.as_str()) {
            return Ok(());
        }
        let store = SDKStore::new(None);
        if let Some(root) = Self::selected_sdk(raw, &store, &environment())? {
            Self::invoke(&root, raw)?;
        }
        Ok(())
    }

    pub fn selected_sdk(
        raw: &[String],
        store: &SDKStore,
        environment: &BTreeMap<String, String>,
    ) -> Result<Option<PathBuf>, ToolError> {
        if let Some(index) = raw.iter().position(|token| token == "--sdk-root") {
            if let Some(value) = raw.get(index + 1) {
                return Ok(Some(standardized(value)));
            }
        }
        if let Some(path) = environment.get("COVE_SDK_ROOT") {
            return Ok(Some(standardized(path)));
        }
        let values: BTreeSet<String> = OptionKey::ALL
            .iter()
            .map(|option| format!("--{}", option.raw_value()))
            .collect();
        let tokens: Vec<&String> = raw
            .iter()
            .take_while(|token| token.as_str() != "--")
            .collect();
        let mut paths = Vec::new();
        for index in 1..tokens.len() {
            if !tokens[index].starts_with("--") && !values.contains(tokens[index - 1].as_str()) {
                paths.push(standardized(tokens[index]));
            }
        }
        let cwd = standardized(".");
        let project = paths
            .iter()
            .chain(std::iter::once(&cwd))
            .find(|path| path.join("cove.json").is_file())
            .cloned();
        let required = project
            .as_ref()
            .and_then(|project| std::fs::read(project.join("cove.json")).ok())
            .and_then(|data| serde_json::from_slice::<serde_json::Value>(&data).ok())
            .and_then(|json| {
                json.get("toolchain")
                    .and_then(|toolchain| toolchain.get("sdkVersion"))
                    .and_then(|version| version.as_str())
                    .map(str::to_string)
            });
        if let Some(project) = &project {
            if let Some(local) = SDKStore::selection(&project.join(".cove/sdk.json")) {
                return Ok(Some(local));
            }
        }
        let bundled = InstallationReceipt::executable_directory();
        let mut candidates: Vec<PathBuf> = store.selected().into_iter().collect();
        candidates.extend(store.installed_urls());
        candidates.push(bundled);
        candidates.retain(|candidate| CoveSDK::is_sdk(candidate));
        let Some(required) = required else {
            return Ok(candidates.into_iter().next());
        };
        if required == SDKRelease::LATEST_TAG {
            return Ok(candidates.into_iter().max_by(|lhs, rhs| {
                let left = SDKRelease::read(lhs)
                    .map(|release| release.version)
                    .unwrap_or_else(|_| "0.0.0".into());
                let right = SDKRelease::read(rhs)
                    .map(|release| release.version)
                    .unwrap_or_else(|_| "0.0.0".into());
                numeric_compare(&left, &right)
            }));
        }
        if let Some(root) = candidates.into_iter().find(|root| {
            SDKRelease::read(root)
                .map(|release| release.version == required)
                .unwrap_or(false)
        }) {
            return Ok(Some(root));
        }
        let version = store.install(&required)?;
        Ok(Some(store.installed(&version)))
    }

    pub fn forward(arguments: &Arguments, raw: &[String]) -> Result<(), ToolError> {
        match arguments.operation {
            Operation::SelfUpdate
            | Operation::InstallCli
            | Operation::PackageCli
            | Operation::PackageNative
            | Operation::SdkInstall
            | Operation::SdkUse
            | Operation::SdkList
            | Operation::Clean
            | Operation::Devices => return Ok(()),
            _ => {}
        }
        let root = CoveSDK::locate(arguments, None, &environment())?;
        Self::invoke(&root, raw)
    }

    pub fn invoke(root: &Path, raw: &[String]) -> Result<(), ToolError> {
        let release = SDKRelease::read(root)?;
        #[cfg_attr(not(windows), allow(unused_mut))]
        let mut binary = SDKPayload::executable(root);
        #[cfg(windows)]
        {
            if !binary.is_file() {
                binary = root.join("cove.cmd");
            }
        }
        let current = InstallationReceipt::executable_directory();
        let different_checkout = CoveSDK::is_sdk(&current)
            && current
                .canonicalize()
                .unwrap_or_else(|_| current.clone())
                != root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let self_version = SDKRelease::current().ok().map(|release| release.version);
        if self_version.as_deref() == Some(release.version.as_str()) && !different_checkout {
            return Ok(());
        }
        if !binary.is_file() {
            return Err(ToolError::invalid(format!(
                "The selected SDK is missing its CLI: {}",
                binary.display()
            )));
        }
        let mut environment = environment();
        let root_path = root.display().to_string();
        if environment.get("COVE_DISPATCHED_SDK") == Some(&root_path) {
            return Err(ToolError::invalid(format!(
                "SDK CLI and manifest versions disagree at {}",
                root.display()
            )));
        }
        environment.insert("COVE_SDK_ROOT".into(), root_path.clone());
        environment.insert("COVE_DISPATCHED_SDK".into(), root_path);
        let mut command = vec![binary.display().to_string()];
        command.extend(raw.iter().cloned());
        let (_, _, status) = Shell::run_status(
            &command,
            None,
            &environment,
            false,
            false,
            StdoutTarget::StandardOutput,
            None,
            true,
        )?;
        std::process::exit(status);
    }
}
