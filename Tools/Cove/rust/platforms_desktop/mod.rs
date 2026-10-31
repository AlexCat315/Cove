//! Desktop build backend: Cargo invocation, app bundle assembly and signing.
//!
//! Swift sources: `Platforms/Desktop/Build.swift`,
//! `Platforms/Desktop/Bundle.swift`, `Platforms/Desktop/Launcher.swift`,
//! `Platforms/Desktop/NativeResources.swift`.

use crate::cli::{AppPlatform, Arguments, BuildConfiguration, OptionKey};
use crate::project::AppProject;
use crate::support::{Files, Shell, StdoutTarget, ToolError, environment};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The Cargo executable; `COVE_CARGO_EXECUTABLE` mirrors the wrapper scripts.
pub fn cargo() -> String {
    std::env::var("COVE_CARGO_EXECUTABLE").unwrap_or_else(|_| "cargo".into())
}

/// Cargo invocation shared by the desktop, task and test flows.
pub struct CargoPackage;

impl CargoPackage {
    /// Environment override so the built application resolves the same SDK
    /// checkout and native libraries.
    pub fn environment(root: &Path) -> Result<BTreeMap<String, String>, ToolError> {
        let mut environment = environment();
        environment.insert("COVE_SDK_ROOT".into(), root.display().to_string());
        if cfg!(target_os = "linux") {
            environment.insert("COVE_NATIVE_LINK_ORDER".into(), "1".into());
        }
        #[cfg(windows)]
        cove_tool_support::windows_build_environment::activate(&mut environment)
            .map_err(|error| ToolError::invalid(error.to_string()))?;
        Ok(environment)
    }

    pub fn command(
        directory: &Path,
        configuration: BuildConfiguration,
        verb: &str,
        scratch_path: Option<&Path>,
    ) -> Vec<String> {
        let mut command: Vec<String> = vec![
            cargo(),
            verb.into(),
            "--manifest-path".into(),
            directory.join("Cargo.toml").display().to_string(),
        ];
        if configuration != BuildConfiguration::Debug {
            command.push("--release".into());
        }
        if let Some(scratch_path) = scratch_path {
            command.push("--target-dir".into());
            command.push(scratch_path.display().to_string());
        }
        command
    }

    /// The directory holding the binaries of the last build with this target
    /// directory and configuration.
    pub fn products(
        directory: &Path,
        configuration: BuildConfiguration,
        scratch_path: Option<&Path>,
    ) -> PathBuf {
        let target = scratch_path
            .map(Path::to_path_buf)
            .unwrap_or_else(|| directory.join("target"));
        if configuration == BuildConfiguration::Debug {
            target.join("debug")
        } else {
            target.join("release")
        }
    }
}

/// `cove build`/`cove run` for macOS, Windows and Linux.
pub struct DesktopBuild {
    pub project: AppProject,
    pub root: PathBuf,
    pub arguments: Arguments,
}

impl DesktopBuild {
    pub fn new(project: AppProject, root: &Path, arguments: &Arguments) -> Self {
        Self {
            project,
            root: root.to_path_buf(),
            arguments: arguments.clone(),
        }
    }

    pub fn run(&self) -> Result<(), ToolError> {
        let binaries = self.compile()?;
        let artifact = DesktopPackage::new(self.project.clone(), self.arguments.platform)
            .assemble(
                &binaries,
                self.arguments.configuration,
                self.arguments.url(OptionKey::Output).as_deref(),
            )?;
        NativeDesktopResources::new(self.project.clone(), self.arguments.platform)
            .sign(&artifact, self.arguments.configuration)?;
        println!(
            "Built {} application: {}",
            self.arguments.platform.raw_value(),
            artifact.directory.display()
        );
        if self.arguments.operation == crate::cli::Operation::Run {
            crate::development::DesktopRun::run(
                &artifact.executable,
                &binaries,
                &self.project,
                &self.root,
                &self.arguments,
            )?;
        }
        Ok(())
    }

    fn compile(&self) -> Result<PathBuf, ToolError> {
        let package = DesktopLauncher::new(self.project.clone()).prepare()?;
        let scratch = self.project.build_directory();
        let command = CargoPackage::command(
            &package,
            self.arguments.configuration,
            "build",
            Some(&scratch),
        );
        let build_environment = CargoPackage::environment(&self.root)?;
        let mut product = command.clone();
        product.push("--bin".into());
        product.push(self.project.desktop_product());
        Shell::run(
            &product,
            None,
            &build_environment,
            false,
            false,
            StdoutTarget::StandardOutput,
        )?;
        for helper in &self.project.configuration.targets.desktop_helpers {
            let mut build = command.clone();
            build.push("--bin".into());
            build.push(helper.clone());
            Shell::run(
                &build,
                None,
                &build_environment,
                false,
                false,
                StdoutTarget::StandardOutput,
            )?;
        }
        Ok(CargoPackage::products(
            &package,
            self.arguments.configuration,
            Some(&scratch),
        ))
    }
}

/// An assembled desktop artifact.
pub struct DesktopArtifact {
    pub directory: PathBuf,
    pub executable: PathBuf,
}

/// Wraps built binaries in the platform's expected layout.
pub struct DesktopPackage {
    pub project: AppProject,
    pub platform: AppPlatform,
}

impl DesktopPackage {
    pub fn new(project: AppProject, platform: AppPlatform) -> Self {
        Self { project, platform }
    }

    pub fn assemble(
        &self,
        binaries: &Path,
        configuration: BuildConfiguration,
        output: Option<&Path>,
    ) -> Result<DesktopArtifact, ToolError> {
        let product = self.project.desktop_product();
        let is_mac = self.platform == AppPlatform::Mac;
        let destination = output.map(Path::to_path_buf).unwrap_or_else(|| {
            self.project
                .build_directory()
                .join(self.platform.raw_value())
                .join(configuration.raw_value())
                .join(format!("{product}{}", if is_mac { ".app" } else { "" }))
        });
        if is_mac
            && destination
                .extension()
                .map(|extension| extension != "app")
                .unwrap_or(true)
        {
            return Err(ToolError::invalid(
                "macOS --output must name an .app directory",
            ));
        }
        // Custom outputs must not replace a project, sources, or an arbitrary
        // existing directory.
        if destination.exists() {
            let marker = destination.join(if is_mac {
                "Contents/Resources/.cove-artifact"
            } else {
                ".cove-artifact"
            });
            if !marker.exists() {
                return Err(ToolError::invalid(format!(
                    "Refusing to replace an unowned output: {}",
                    destination.display()
                )));
            }
        }
        Files::reset(&destination)?;
        Files::text(
            "Generated by cove build\n",
            &destination.join(if is_mac {
                "Contents/Resources/.cove-artifact"
            } else {
                ".cove-artifact"
            }),
        )?;
        let executable_directory = if is_mac {
            destination.join("Contents/MacOS")
        } else {
            destination.clone()
        };
        let resource_directory = if is_mac {
            destination.join("Contents/Resources")
        } else {
            destination.clone()
        };
        let suffix = if self.platform == AppPlatform::Win {
            ".exe"
        } else {
            ""
        };
        let executable = executable_directory.join(format!("{product}{suffix}"));
        Files::copy(&binaries.join(format!("{product}{suffix}")), &executable)?;
        let helper_directory = if is_mac {
            destination.join("Contents/Helpers")
        } else {
            executable_directory.join("Tools")
        };
        for helper in &self.project.configuration.targets.desktop_helpers {
            let name = format!("{helper}{suffix}");
            Files::copy(&binaries.join(&name), &helper_directory.join(&name))?;
        }
        for child in Files::children(binaries)? {
            let extension = child
                .extension()
                .map(|extension| extension.to_string_lossy().into_owned())
                .unwrap_or_default();
            let name = child
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            if ["bundle", "resources"].contains(&extension.as_str()) {
                Files::copy(&child, &resource_directory.join(&name))?;
            } else if ["dylib", "dll", "so"].contains(&extension.as_str()) || name.contains(".so.")
            {
                Files::copy(&child, &executable_directory.join(&name))?;
            }
        }
        let artifact = DesktopArtifact {
            directory: destination.clone(),
            executable,
        };
        NativeDesktopResources::new(self.project.clone(), self.platform)
            .package(&artifact, configuration)?;
        Ok(artifact)
    }
}

/// The Cargo package that supplies the desktop executable. A generated
/// launcher package is no longer needed: the application crate is itself a
/// binary target named after `targets.desktopExecutable`.
pub struct DesktopLauncher {
    pub project: AppProject,
}

impl DesktopLauncher {
    pub fn new(project: AppProject) -> Self {
        Self { project }
    }

    pub fn prepare(&self) -> Result<PathBuf, ToolError> {
        let directory = self.project.directory.clone();
        if !directory.join("Cargo.toml").is_file() {
            return Err(ToolError::invalid(format!(
                "{} must contain Cargo.toml; run cove create app",
                directory.display()
            )));
        }
        Ok(directory)
    }
}

/// Platform resource packaging and code signing for a desktop artifact.
pub struct NativeDesktopResources {
    pub project: AppProject,
    pub platform: AppPlatform,
}

impl NativeDesktopResources {
    pub fn new(project: AppProject, platform: AppPlatform) -> Self {
        Self { project, platform }
    }

    fn directory(&self) -> PathBuf {
        self.project
            .directory
            .join("Platforms")
            .join(self.platform.directory_name())
    }

    pub fn package(
        &self,
        artifact: &DesktopArtifact,
        configuration: BuildConfiguration,
    ) -> Result<(), ToolError> {
        let resources = self.directory().join("Resources");
        let target = if self.platform == AppPlatform::Mac {
            artifact.directory.join("Contents/Resources")
        } else {
            artifact.directory.join("Resources")
        };
        if resources.exists() {
            for source in Files::children(&resources)? {
                if source
                    .file_name()
                    .map(|name| name == "README.md")
                    .unwrap_or(false)
                {
                    continue;
                }
                let name = source
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                Files::copy(&source, &target.join(name))?;
            }
        }
        match self.platform {
            AppPlatform::Mac => self.mac(artifact, configuration),
            AppPlatform::Win => {
                let manifest = self.directory().join("Application.manifest");
                Shell::run(
                    &[
                        "mt.exe".into(),
                        "-nologo".into(),
                        "-manifest".into(),
                        manifest.display().to_string(),
                        format!("-outputresource:{};#1", artifact.executable.display()),
                    ],
                    None,
                    &environment(),
                    false,
                    false,
                    StdoutTarget::StandardOutput,
                )
                .map(|_| ())
            }
            AppPlatform::Linux => self.linux(artifact),
            AppPlatform::Android | AppPlatform::Ios | AppPlatform::Web => Ok(()),
        }
    }

    fn mac(
        &self,
        artifact: &DesktopArtifact,
        configuration: BuildConfiguration,
    ) -> Result<(), ToolError> {
        let mut info = self.plist("Info.plist")?;
        let identity = &self.project.configuration.application;
        let version = &self.project.configuration.version;
        let managed: [(&str, String); 7] = [
            ("CFBundleIdentifier", identity.id.clone()),
            ("CFBundleName", identity.name.clone()),
            ("CFBundleDisplayName", identity.name.clone()),
            ("CFBundleExecutable", self.project.desktop_product()),
            ("CFBundlePackageType", "APPL".into()),
            ("CFBundleShortVersionString", version.name.clone()),
            ("CFBundleVersion", version.build.to_string()),
        ];
        for (key, value) in managed {
            if info.contains_key(key) {
                return Err(ToolError::invalid(format!(
                    "{key} is supplied by cove.json / the build; remove the duplicate from Platforms/macOS/Info.plist"
                )));
            }
            info.insert(key.into(), serde_json::Value::String(value));
        }
        if self.directory().join("Resources/AppIcon.icns").exists() {
            info.insert(
                "CFBundleIconFile".into(),
                serde_json::Value::String("AppIcon.icns".into()),
            );
        }
        write_plist(&info, &artifact.directory.join("Contents/Info.plist"))?;
        let mut entitlements = self.plist("Entitlements.plist")?;
        entitlements.insert(
            "com.apple.security.get-task-allow".into(),
            serde_json::Value::Bool(configuration != BuildConfiguration::Release),
        );
        write_plist(
            &entitlements,
            &self
                .project
                .build_directory()
                .join("mac")
                .join(configuration.raw_value())
                .join("Entitlements.plist"),
        )?;
        Ok(())
    }

    pub fn sign(
        &self,
        artifact: &DesktopArtifact,
        configuration: BuildConfiguration,
    ) -> Result<(), ToolError> {
        #[cfg(target_os = "macos")]
        {
            if self.platform == AppPlatform::Mac {
                let file = self
                    .project
                    .build_directory()
                    .join("mac")
                    .join(configuration.raw_value())
                    .join("Entitlements.plist");
                for helper in &self.project.configuration.targets.desktop_helpers {
                    let executable = artifact.directory.join("Contents/Helpers").join(helper);
                    Shell::run(
                        &[
                            "codesign".into(),
                            "--force".into(),
                            "--sign".into(),
                            "-".into(),
                            executable.display().to_string(),
                        ],
                        None,
                        &environment(),
                        false,
                        false,
                        StdoutTarget::StandardOutput,
                    )?;
                }
                Shell::run(
                    &[
                        "codesign".into(),
                        "--force".into(),
                        "--sign".into(),
                        "-".into(),
                        "--entitlements".into(),
                        file.display().to_string(),
                        artifact.directory.display().to_string(),
                    ],
                    None,
                    &environment(),
                    false,
                    false,
                    StdoutTarget::StandardOutput,
                )?;
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (artifact, configuration);
        }
        Ok(())
    }

    fn linux(&self, artifact: &DesktopArtifact) -> Result<(), ToolError> {
        let source = self.directory().join("Application.desktop");
        let contents = std::fs::read_to_string(&source)
            .map_err(|error| ToolError::invalid(format!("{}: {error}", source.display())))?;
        let lines: Vec<&str> = contents.lines().collect();
        let valid = lines.first() == Some(&"[Desktop Entry]")
            && !lines
                .iter()
                .any(|line| line.starts_with("Name=") || line.starts_with("Exec="));
        if !valid {
            return Err(ToolError::invalid(
                "Application.desktop requires [Desktop Entry]; Name and Exec are supplied by cove.json / the build",
            ));
        }
        let executable = artifact
            .executable
            .display()
            .to_string()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        let mut output = lines.join("\n");
        output.push_str(&format!(
            "\nName={}\nExec=\"{executable}\"\n",
            self.project.configuration.application.name
        ));
        Files::text(
            &output,
            &artifact
                .directory
                .join(format!("{}.desktop", self.project.desktop_product())),
        )
    }

    fn plist(&self, name: &str) -> Result<BTreeMap<String, serde_json::Value>, ToolError> {
        let file = self.directory().join(name);
        let contents = std::fs::read_to_string(&file)
            .map_err(|error| ToolError::invalid(format!("{}: {error}", file.display())))?;
        let value = parse_plist(&contents).ok_or_else(|| {
            ToolError::invalid(format!("{name} must contain a property-list dictionary"))
        })?;
        Ok(value)
    }
}

/// Minimal XML property-list reader for `<dict>` roots.
fn parse_plist(source: &str) -> Option<BTreeMap<String, serde_json::Value>> {
    let mut result = BTreeMap::new();
    let mut stack: Vec<String> = Vec::new();
    let mut pending_key: Option<String> = None;
    let mut rest = source;
    while let Some(start) = rest.find('<') {
        rest = &rest[start..];
        let end = rest.find('>')?;
        let tag = &rest[1..end];
        rest = &rest[end + 1..];
        let trimmed = tag.trim();
        if trimmed.starts_with("?") || trimmed.starts_with('!') {
            continue;
        }
        let closing = trimmed.starts_with('/');
        let name = trimmed
            .trim_start_matches('/')
            .split_whitespace()
            .next()
            .unwrap_or_default();
        match (name, closing) {
            ("dict", false) | ("array", false) => stack.push(name.to_string()),
            ("dict", true) | ("array", true) => {
                stack.pop();
            }
            ("key", false) => {
                let close = rest.find("</key>")?;
                pending_key = Some(decode_entities(&rest[..close]));
                rest = &rest[close + "</key>".len()..];
            }
            (_, false) => {
                let close = rest.find(&format!("</{name}>"))?;
                let text = decode_entities(&rest[..close]);
                rest = &rest[close + name.len() + 3..];
                if let Some(key) = pending_key.take() {
                    let value = match name {
                        "true" => serde_json::Value::Bool(true),
                        "false" => serde_json::Value::Bool(false),
                        "integer" => text
                            .trim()
                            .parse::<i64>()
                            .map(|number| serde_json::Value::from(number))
                            .unwrap_or(serde_json::Value::Null),
                        "real" => text
                            .trim()
                            .parse::<f64>()
                            .ok()
                            .and_then(serde_json::Number::from_f64)
                            .map(serde_json::Value::Number)
                            .unwrap_or(serde_json::Value::Null),
                        _ => serde_json::Value::String(text),
                    };
                    result.insert(key, value);
                }
            }
            _ => {}
        }
    }
    Some(result)
}

fn decode_entities(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn write_plist(value: &BTreeMap<String, serde_json::Value>, file: &Path) -> Result<(), ToolError> {
    let mut body = String::new();
    for (key, item) in value {
        body.push_str(&format!("\t<key>{}</key>\n", encode_entities(key)));
        body.push_str(&encode_value(item, 1));
    }
    let document = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n{body}</dict>\n</plist>\n"
    );
    if let Some(parent) = file.parent() {
        Files::directory(parent)?;
    }
    Files::text(&document, file)
}

fn encode_value(value: &serde_json::Value, depth: usize) -> String {
    let indent = "\t".repeat(depth);
    match value {
        serde_json::Value::Bool(true) => format!("{indent}<true/>\n"),
        serde_json::Value::Bool(false) => format!("{indent}<false/>\n"),
        serde_json::Value::Number(number) => {
            if number.is_f64() {
                format!("{indent}<real>{number}</real>\n")
            } else {
                format!("{indent}<integer>{number}</integer>\n")
            }
        }
        serde_json::Value::String(text) => {
            format!("{indent}<string>{}</string>\n", encode_entities(text))
        }
        serde_json::Value::Array(items) => {
            let mut result = format!("{indent}<array>\n");
            for item in items {
                result.push_str(&encode_value(item, depth + 1));
            }
            result.push_str(&format!("{indent}</array>\n"));
            result
        }
        serde_json::Value::Object(map) => {
            let mut result = format!("{indent}<dict>\n");
            for (key, item) in map {
                let inner = "\t".repeat(depth + 1);
                result.push_str(&format!("{inner}<key>{}</key>\n", encode_entities(key)));
                result.push_str(&encode_value(item, depth + 1));
            }
            result.push_str(&format!("{indent}</dict>\n"));
            result
        }
        serde_json::Value::Null => format!("{indent}<string></string>\n"),
    }
}

fn encode_entities(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
