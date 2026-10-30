//! iOS build backend: `xcodebuild` against the project's Xcode workspace.
//!
//! Swift source: `Platforms/iOS/ApplicationBuild.swift`.

use crate::cli::{Arguments, BuildConfiguration, Operation, OptionKey};
use crate::project::AppProject;
use crate::platforms_desktop::SwiftPackage;
use crate::support::{environment, Files, Shell, StdoutTarget, ToolError};
use crate::toolchains::{AppleTarget, NativeSDK};
use std::path::{Path, PathBuf};

/// `cove build`/`cove run` for an iOS application project.
pub struct IOSBuild {
    pub project: AppProject,
    pub root: PathBuf,
    pub arguments: Arguments,
}

impl IOSBuild {
    pub fn new(project: AppProject, root: &Path, arguments: &Arguments) -> Self {
        Self {
            project,
            root: root.to_path_buf(),
            arguments: arguments.clone(),
        }
    }

    pub fn run(&self) -> Result<(), ToolError> {
        let device = self.arguments.value(OptionKey::Device);
        let sdk = if device.is_none() {
            "iphonesimulator"
        } else {
            "iphoneos"
        };
        // A physical device is always arm64; a simulator follows the host.
        let architecture = if device.is_some() || cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "x86_64"
        };
        if device.is_some() && self.arguments.value(OptionKey::Team).is_none() {
            return Err(ToolError::invalid(
                "iOS device builds require --team APPLE_TEAM_ID and a configured signing identity",
            ));
        }
        let variant = format!(
            "ios-{architecture}{}",
            if device.is_none() { "-simulator" } else { "" }
        );
        NativeSDK::prepare(
            &self.root,
            Some(&AppleTarget {
                sdk: sdk.into(),
                architecture: architecture.into(),
            }),
            false,
        )?;
        let mut build_environment = SwiftPackage::environment(&self.root)?;
        build_environment.insert(
            "COVE_RUNTIME_ARTIFACT_ROOT".into(),
            format!("Artifacts/native/{variant}/Native/vendor"),
        );
        build_environment.insert(
            "COVE_TEXT_ARTIFACT_ROOT".into(),
            format!("Artifacts/native/{variant}/vendor"),
        );
        let derived = self.project.build_directory().join(&variant).join("xcode");
        let configuration = if self.arguments.configuration == BuildConfiguration::Debug {
            "Debug"
        } else {
            "Release"
        };
        let version = &self.project.configuration.version;
        let identity = &self.project.configuration.application;
        // The destination selects the app SDK. A global -sdk override also
        // forces host compiler plug-ins onto iOS, making Swift macros
        // unloadable.
        let mut command: Vec<String> = vec![
            "xcodebuild".into(),
            "-jobs".into(),
            "4".into(),
            "-project".into(),
            self.project
                .directory
                .join("Platforms/iOS/Application.xcodeproj")
                .display()
                .to_string(),
            "-scheme".into(),
            "Application".into(),
            "-configuration".into(),
            configuration.into(),
            "-destination".into(),
            format!(
                "generic/platform={}",
                if device.is_none() { "iOS Simulator" } else { "iOS" }
            ),
            "-derivedDataPath".into(),
            derived.display().to_string(),
            format!("ARCHS={architecture}"),
            format!("PRODUCT_BUNDLE_IDENTIFIER={}", identity.id),
            format!("INFOPLIST_KEY_CFBundleDisplayName={}", identity.name),
            format!("MARKETING_VERSION={}", version.name),
            format!("CURRENT_PROJECT_VERSION={}", version.build),
        ];
        match device {
            Some(_) => {
                command.push(format!(
                    "DEVELOPMENT_TEAM={}",
                    self.arguments.value(OptionKey::Team).unwrap_or_default()
                ));
                command.push("-allowProvisioningUpdates".into());
            }
            None => command.push("CODE_SIGNING_ALLOWED=NO".into()),
        }
        command.push("build".into());
        Shell::run(
            &command,
            None,
            &build_environment,
            false,
            false,
            StdoutTarget::StandardOutput,
        )?;
        let app = derived
            .join("Build/Products")
            .join(format!("{configuration}-{sdk}"))
            .join("Application.app");
        let requested = self.arguments.url(OptionKey::Output);
        let output = requested
            .clone()
            .unwrap_or_else(|| self.project.build_directory().join(&variant).join("Application.app"));
        if output.extension().map(|extension| extension != "app").unwrap_or(true) {
            return Err(ToolError::invalid("iOS --output must end in .app"));
        }
        // Never remove a user directory to assemble an artifact.
        if requested.is_some() && output.exists() {
            return Err(ToolError::invalid(format!(
                "Output exists: {}; choose another --output or run cove clean",
                output.display()
            )));
        }
        if requested.is_none() {
            Files::remove(&output)?;
        }
        Files::copy(&app, &output)?;
        println!("Built iOS application: {}", output.display());
        if self.arguments.operation == Operation::Run {
            self.launch(&output, device)?;
        }
        Ok(())
    }

    fn launch(&self, app: &Path, device: Option<&str>) -> Result<(), ToolError> {
        let id = &self.project.configuration.application.id;
        if let Some(device) = device {
            Shell::run(
                &[
                    "xcrun".into(),
                    "devicectl".into(),
                    "device".into(),
                    "install".into(),
                    "app".into(),
                    "--device".into(),
                    device.into(),
                    app.display().to_string(),
                ],
                None,
                &environment(),
                false,
                false,
                StdoutTarget::StandardOutput,
            )?;
            Shell::run(
                &[
                    "xcrun".into(),
                    "devicectl".into(),
                    "device".into(),
                    "process".into(),
                    "launch".into(),
                    "--device".into(),
                    device.into(),
                    id.clone(),
                ],
                None,
                &environment(),
                false,
                false,
                StdoutTarget::StandardOutput,
            )?;
            return Ok(());
        }
        let simulator = self
            .arguments
            .value(OptionKey::Simulator)
            .unwrap_or("booted");
        if simulator != "booted" {
            // bootstatus -b boots a shutdown simulator and waits for readiness.
            Shell::run(
                &[
                    "xcrun".into(),
                    "simctl".into(),
                    "bootstatus".into(),
                    simulator.into(),
                    "-b".into(),
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
                "xcrun".into(),
                "simctl".into(),
                "install".into(),
                simulator.into(),
                app.display().to_string(),
            ],
            None,
            &environment(),
            false,
            false,
            StdoutTarget::StandardOutput,
        )?;
        Shell::run(
            &[
                "xcrun".into(),
                "simctl".into(),
                "launch".into(),
                simulator.into(),
                id.clone(),
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
