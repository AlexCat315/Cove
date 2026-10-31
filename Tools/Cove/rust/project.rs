//! Application projects: `cove.json`, the UI template, and package tests.
//!
//! Swift sources: `Project/Configuration.swift`, `Project/Create.swift`,
//! `Project/Identity.swift`, `Project/Templates.swift`,
//! `Project/Testing.swift`, `Project/Version.swift`.
//!
//! `cove.json` is decoded with serde instead of `Codable`; the accepted keys,
//! defaults and validation order are unchanged. Regular-expression checks are
//! implemented as explicit scanners because this crate does not depend on a
//! regular-expression engine.

use crate::cli::{AppPlatform, Arguments, Flag, OptionKey, standardized};
use crate::development::{Inspector, ManagedSession};
use crate::platforms_desktop::CargoPackage;
use crate::project_sdk::{CoveSDK, SDKRelease};
use crate::support::{Files, Shell, StdoutTarget, ToolError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Reverse-domain application identifier plus display name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationIdentity {
    pub id: String,
    pub name: String,
}

impl ApplicationIdentity {
    pub fn validate(&self) -> Result<(), ToolError> {
        if !is_reverse_domain(&self.id) {
            return Err(ToolError::invalid(
                "application.id must be a lowercase reverse-domain identifier, such as com.example.myapp",
            ));
        }
        if self.name.trim().is_empty() || self.name.chars().any(|character| character.is_control())
        {
            return Err(ToolError::invalid(
                "application.name must be a nonempty printable label",
            ));
        }
        Ok(())
    }
}

/// `^[a-z][a-z0-9]*(?:\.[a-z][a-z0-9]*)+$`
fn is_reverse_domain(value: &str) -> bool {
    let segments: Vec<&str> = value.split('.').collect();
    if segments.len() < 2 {
        return false;
    }
    segments.iter().all(|segment| {
        let mut characters = segment.chars();
        match characters.next() {
            Some(first) if first.is_ascii_lowercase() => {}
            _ => return false,
        }
        characters.all(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
    })
}

/// `^[A-Za-z][A-Za-z0-9_-]*$`
pub fn is_product_name(value: &str) -> bool {
    let mut characters = value.chars();
    match characters.next() {
        Some(first) if first.is_ascii_alphabetic() => {}
        _ => return false,
    }
    characters
        .all(|character| character.is_ascii_alphanumeric() || character == '_' || character == '-')
}

/// `^[A-Za-z_][A-Za-z0-9_-]*$`
pub fn is_task_product(value: &str) -> bool {
    let mut characters = value.chars();
    match characters.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    characters
        .all(|character| character.is_ascii_alphanumeric() || character == '_' || character == '-')
}

/// `^[A-Za-z_][A-Za-z0-9_]*$`
pub fn is_module_name(value: &str) -> bool {
    let mut characters = value.chars();
    match characters.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

/// `^[A-Za-z_][A-Za-z0-9_]*\.[A-Za-z_][A-Za-z0-9_]*$`
pub fn is_entry_name(value: &str) -> bool {
    match value.split_once('.') {
        Some((module, member)) => is_module_name(module) && is_module_name(member),
        None => false,
    }
}

/// `^[0-9]+\.[0-9]+\.[0-9]+(?:-[A-Za-z0-9.-]+)?$`
pub fn is_dotted_version(value: &str) -> bool {
    let (core, suffix) = match value.split_once('-') {
        Some((core, suffix)) => (core, Some(suffix)),
        None => (value, None),
    };
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() != 3
        || !parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return false;
    }
    match suffix {
        None => true,
        Some(suffix) => {
            !suffix.is_empty()
                && suffix.chars().all(|character| {
                    character.is_ascii_alphanumeric() || character == '.' || character == '-'
                })
        }
    }
}

/// `^\d+\.\d+\.\d+$`
pub fn is_application_version_name(value: &str) -> bool {
    let parts: Vec<&str> = value.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectConfiguration {
    pub application: ApplicationIdentity,
    pub toolchain: Toolchain,
    #[serde(default)]
    pub targets: Targets,
    pub version: ApplicationVersion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Toolchain {
    pub sdk_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Targets {
    #[serde(default = "default_platforms")]
    pub platforms: Vec<AppPlatform>,
    #[serde(default)]
    pub desktop_executable: Option<String>,
    #[serde(default)]
    pub desktop_helpers: Vec<String>,
}

fn default_platforms() -> Vec<AppPlatform> {
    vec![
        AppPlatform::Mac,
        AppPlatform::Win,
        AppPlatform::Linux,
        AppPlatform::Android,
        AppPlatform::Ios,
    ]
}

impl Default for Targets {
    fn default() -> Self {
        Self {
            platforms: default_platforms(),
            desktop_executable: None,
            desktop_helpers: Vec::new(),
        }
    }
}

impl Targets {
    pub fn validate(&self) -> Result<(), ToolError> {
        let executable = self
            .desktop_executable
            .clone()
            .unwrap_or_else(|| "Application".into());
        let unique: BTreeSet<&String> = self.desktop_helpers.iter().collect();
        if unique.len() != self.desktop_helpers.len()
            || self.desktop_helpers.contains(&executable)
            || !self
                .desktop_helpers
                .iter()
                .all(|helper| is_product_name(helper))
        {
            return Err(ToolError::invalid(
                "targets.desktopHelpers must contain unique executable product names",
            ));
        }
        let unique_platforms: BTreeSet<AppPlatform> = self.platforms.iter().copied().collect();
        if self.platforms.is_empty() || unique_platforms.len() != self.platforms.len() {
            return Err(ToolError::invalid(
                "targets.platforms must contain unique platform names",
            ));
        }
        if let Some(executable) = &self.desktop_executable {
            if !is_product_name(executable) {
                return Err(ToolError::invalid(
                    "targets.desktopExecutable must be an executable product name",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationVersion {
    #[serde(default = "default_version_name")]
    pub name: String,
    #[serde(default = "default_version_build")]
    pub build: i64,
}

fn default_version_name() -> String {
    "0.1.0".into()
}

fn default_version_build() -> i64 {
    1
}

impl Default for ApplicationVersion {
    fn default() -> Self {
        Self {
            name: default_version_name(),
            build: default_version_build(),
        }
    }
}

impl ApplicationVersion {
    pub fn validate(&self) -> Result<(), ToolError> {
        if !is_application_version_name(&self.name) || !(1..=2_100_000_000).contains(&self.build) {
            return Err(ToolError::invalid(
                "version requires a numeric major.minor.patch name and a positive build number",
            ));
        }
        Ok(())
    }
}

impl ProjectConfiguration {
    pub fn new(application: ApplicationIdentity, toolchain: Toolchain) -> Self {
        Self {
            application,
            toolchain,
            targets: Targets::default(),
            version: ApplicationVersion::default(),
        }
    }

    pub fn validate(&self) -> Result<(), ToolError> {
        self.application.validate()?;
        self.targets.validate()?;
        self.version.validate()?;
        if !SDKRelease::is_version_requirement(&self.toolchain.sdk_version) {
            return Err(ToolError::invalid(
                "toolchain.sdkVersion must be an exact SDK version or \"latest\"",
            ));
        }
        Ok(())
    }
}

/// An application directory: `cove.json` plus a Cargo package.
#[derive(Debug, Clone)]
pub struct AppProject {
    pub directory: PathBuf,
    pub configuration: ProjectConfiguration,
}

impl AppProject {
    pub fn new(at: &Path) -> Result<Self, ToolError> {
        let directory = standardized(at);
        let manifest = directory.join("cove.json");
        if !manifest.is_file() || !directory.join("Cargo.toml").is_file() {
            return Err(ToolError::invalid(format!(
                "{} must contain cove.json and Cargo.toml; use cove create app {}",
                directory.display(),
                directory.display()
            )));
        }
        let data = std::fs::read(&manifest)
            .map_err(|error| ToolError::invalid(format!("{}: {error}", manifest.display())))?;
        let configuration: ProjectConfiguration = serde_json::from_slice(&data)
            .map_err(|error| ToolError::invalid(format!("{}: {error}", manifest.display())))?;
        configuration.validate()?;
        Ok(Self {
            directory,
            configuration,
        })
    }

    pub fn desktop_product(&self) -> String {
        self.configuration
            .targets
            .desktop_executable
            .clone()
            .unwrap_or_else(|| "Application".into())
    }

    pub fn build_directory(&self) -> PathBuf {
        self.directory.join(".build")
    }

    pub fn require(&self, platform: AppPlatform) -> Result<(), ToolError> {
        platform.require_available()?;
        if !self.configuration.targets.platforms.contains(&platform) {
            return Err(ToolError::invalid(format!(
                "This application's targets.platforms does not include {}",
                platform.raw_value()
            )));
        }
        Ok(())
    }

    pub fn clean(&self) -> Result<(), ToolError> {
        let mut builds = vec![self.build_directory()];
        let legacy = self.directory.join(".cove/build");
        let legacy_is_link =
            is_symbolic_link(&legacy) || legacy.parent().map(is_symbolic_link).unwrap_or(false);
        if !legacy_is_link {
            builds.push(legacy);
        }
        for build in &builds {
            if is_symbolic_link(build) || build.parent().map(is_symbolic_link).unwrap_or(false) {
                return Err(ToolError::invalid(format!(
                    "Refusing to clean a symlink at {}",
                    build.display()
                )));
            }
            let sessions = build.join("devtools");
            if sessions.exists() {
                for directory in Files::children(&sessions)? {
                    let Ok(data) = std::fs::read(directory.join("session.json")) else {
                        continue;
                    };
                    let Ok(session) = serde_json::from_slice::<ManagedSession>(&data) else {
                        continue;
                    };
                    if Inspector::is_alive(&session).unwrap_or(false) {
                        return Err(ToolError::invalid(format!(
                            "Stop development session {} before cleaning this application's build cache",
                            session.runtime.id
                        )));
                    }
                }
            }
        }
        for build in &builds {
            Files::remove(build)?;
        }
        Ok(())
    }
}

fn is_symbolic_link(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}

/// `cove create app <directory>`.
pub struct CreateApplication;

impl CreateApplication {
    pub fn run(arguments: &Arguments, root: &Path) -> Result<(), ToolError> {
        let destination = arguments.directory();
        let slug: String = destination
            .file_name()
            .map(|name| name.to_string_lossy().to_lowercase())
            .unwrap_or_default()
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .collect();
        let identifier = if slug
            .chars()
            .next()
            .map(|character| character.is_ascii_alphabetic())
            .unwrap_or(false)
        {
            slug.clone()
        } else {
            format!("app{slug}")
        };
        let default_name = destination
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let identity = ApplicationIdentity {
            id: arguments
                .value(OptionKey::Id)
                .map(str::to_string)
                .unwrap_or_else(|| format!("com.example.{identifier}")),
            name: arguments
                .value(OptionKey::Name)
                .map(str::to_string)
                .unwrap_or(default_name),
        };
        let mut configuration = ProjectConfiguration::new(
            identity,
            Toolchain {
                sdk_version: SDKRelease::read(root)?.version,
            },
        );
        if let Some(platforms) = arguments.value(OptionKey::Platforms) {
            let mut selected = Vec::new();
            for value in platforms.split(',') {
                let platform = AppPlatform::parse(value)
                    .ok_or_else(|| ToolError::invalid(format!("Unknown platform: {value}")))?;
                if ![
                    AppPlatform::Mac,
                    AppPlatform::Win,
                    AppPlatform::Linux,
                    AppPlatform::Android,
                    AppPlatform::Ios,
                ]
                .contains(&platform)
                {
                    return Err(ToolError::invalid(format!(
                        "Platform template not implemented: {value}"
                    )));
                }
                selected.push(platform);
            }
            configuration.targets.platforms = selected;
        }
        configuration.validate()?;
        if destination.exists() {
            let empty = Files::children(&destination)
                .map(|contents| contents.is_empty())
                .unwrap_or(false);
            if !empty {
                return Err(ToolError::invalid(format!(
                    "Refusing to overwrite nonempty destination: {}",
                    destination.display()
                )));
            }
        }
        let templates = ApplicationTemplates::root(root);
        // The Rust template is a Cargo project. Swift sources, SwiftPM
        // manifests and Swift tests are not copied; `.rs` sources and
        // integration tests are emitted verbatim.
        for source in Files::files(&templates)? {
            let mut relative = Files::relative(&source, &templates);
            if relative.ends_with(".in") {
                relative = relative.trim_end_matches(".in").to_string();
            }
            // SwiftPM manifests, Swift sources and Swift test targets belong to
            // the pre-migration template and are never emitted.
            if relative == "Package.swift"
                || relative.ends_with(".swift")
                || relative.starts_with("Sources/AppContent/")
                || relative.starts_with("Tests/AppContentTests/")
            {
                continue;
            }
            if let Some(rest) = relative.strip_prefix("Platforms/") {
                let name = rest.split('/').next().unwrap_or_default();
                if !configuration
                    .targets
                    .platforms
                    .iter()
                    .any(|platform| platform.directory_name() == name)
                {
                    continue;
                }
            }
            if source
                .extension()
                .map(|extension| extension == "in")
                .unwrap_or(false)
            {
                let text = std::fs::read_to_string(&source).map_err(|error| {
                    ToolError::invalid(format!("{}: {error}", source.display()))
                })?;
                Files::text(
                    &text.replace("{{COVE_UI_PATH}}", &toml_path(root)),
                    &destination.join(&relative),
                )?;
            } else {
                Files::copy(&source, &destination.join(&relative))?;
            }
        }
        Files::json(&configuration, &destination.join("cove.json"))?;
        println!(
            "Created cross-platform Cove application: {}",
            destination.display()
        );
        println!("Edit src/app.rs, then: cove run {}", destination.display());
        Ok(())
    }
}

/// A path as it must appear inside a TOML basic string.
fn toml_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "\\\\")
}

/// The UI template bundled with the CLI.
///
/// Swift reads `Bundle.module`; the Rust crate has no resource bundle, so the
/// template is resolved from the SDK checkout that contains this source.
pub struct ApplicationTemplates;

impl ApplicationTemplates {
    pub fn root(root: &Path) -> PathBuf {
        root.join("Tools/Cove/Sources/CoveCLI/Templates/UI")
    }

    /// The template is the Gradle shell only. Java, JNI and the Swift package
    /// stay in sdk/Hosts/Android and are referenced by settings.gradle.
    pub fn install_android(
        root: &Path,
        at: &Path,
        configuration: &ProjectConfiguration,
    ) -> Result<(), ToolError> {
        let source = Self::root(root).join("Platforms/Android");
        Files::copy(&source, &at.join("Platforms/Android"))?;
        Files::json(configuration, &at.join("cove.json"))?;
        Ok(())
    }
}

/// `cove test`: needs a Cargo package, not application identity or launch targets.
pub struct PackageTests {
    pub command: Vec<String>,
    /// Helper binaries built first, so tests that launch one never run a stale build.
    pub helpers: Vec<Vec<String>>,
    pub environment: std::collections::BTreeMap<String, String>,
    pub timeout: Option<Duration>,
}

impl PackageTests {
    pub fn new(arguments: &Arguments) -> Result<Self, ToolError> {
        let directory = arguments.directory();
        if !directory.join("Cargo.toml").is_file() {
            return Err(ToolError::invalid(format!(
                "{} must contain Cargo.toml to run cove test",
                directory.display()
            )));
        }
        let project = if directory.join("cove.json").is_file() {
            Some(AppProject::new(&directory)?)
        } else {
            None
        };
        let root = CoveSDK::locate(arguments, project.as_ref(), &std::env::vars().collect())?;
        if arguments.flags.contains(&Flag::SkipBuild) && arguments.flags.contains(&Flag::BuildOnly)
        {
            return Err(ToolError::invalid(
                "Select either --build-only or --skip-build",
            ));
        }
        let scratch = arguments.url(OptionKey::ScratchPath);
        let mut command = CargoPackage::command(
            &directory,
            arguments.configuration,
            "test",
            scratch.as_deref(),
        );
        // `cargo test` filters by substring; empty means every test.
        if let Some(filter) = arguments.value(OptionKey::Filter) {
            command.push(filter.into());
        }
        if arguments.flags.contains(&Flag::NoParallel) {
            command.push("--test-threads=1".into());
        }
        let build = CargoPackage::command(
            &directory,
            arguments.configuration,
            "build",
            scratch.as_deref(),
        );
        if arguments.flags.contains(&Flag::SkipBuild) {
            command.push("--no-run".into());
        }
        if let Some(library) = arguments.value(OptionKey::TestingLibrary) {
            if !["libtest", "cargo-test"].contains(&library) {
                return Err(ToolError::invalid(
                    "--testing-library must be libtest or cargo-test",
                ));
            }
        }
        let timeout = match arguments.value(OptionKey::Timeout) {
            Some(value) => match value.parse::<f64>() {
                Ok(seconds) if seconds.is_finite() && seconds > 0.0 && seconds <= 86_400.0 => {
                    Some(Duration::from_secs_f64(seconds))
                }
                _ => {
                    return Err(ToolError::invalid(
                        "--timeout must be between 0 and 86400 seconds",
                    ));
                }
            },
            None => None,
        };
        let helpers = if arguments.flags.contains(&Flag::SkipBuild) {
            Vec::new()
        } else {
            project
                .as_ref()
                .map(|project| project.configuration.targets.desktop_helpers.clone())
                .unwrap_or_default()
                .into_iter()
                .map(|helper| {
                    let mut helper_command = build.clone();
                    helper_command.push("--bin".into());
                    helper_command.push(helper);
                    helper_command
                })
                .collect()
        };
        if arguments.flags.contains(&Flag::BuildOnly) {
            command = build;
            command.push("--tests".into());
        }
        let root_environment = CargoPackage::environment(&root)?;
        Ok(Self {
            command,
            helpers,
            environment: root_environment,
            timeout,
        })
    }

    pub fn run(&self) -> Result<(), ToolError> {
        for helper in &self.helpers {
            Shell::run(
                helper,
                None,
                &self.environment,
                false,
                false,
                StdoutTarget::StandardOutput,
            )?;
        }
        let (_, _, status) = Shell::run_status(
            &self.command,
            None,
            &self.environment,
            false,
            false,
            StdoutTarget::StandardOutput,
            self.timeout,
            true,
        )?;
        if status != 0 {
            return Err(ToolError::failed(
                self.command
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "cargo".into()),
                status,
            ));
        }
        Ok(())
    }
}
