//! `cove` — create, build and run cross-platform Cove applications.
//!
//! Swift source: `Entry.swift`. Every dispatch case, printed message and exit
//! status matches the original: a `ToolError` prints one line to stderr and
//! exits `1`; a failing child process propagates its own status.
//!
//! The Swift entry also drove logging (`CoveLogging`) and the SDK re-exec
//! bootstrap (`SDKCommand`); both still run here, with the SDK dispatch first so
//! a newer installed SDK owns flag grammar it introduced.

mod cli;
mod development;
mod download;
mod installation;
mod platforms_android;
mod platforms_desktop;
mod platforms_ios;
mod project;
mod project_sdk;
mod project_tasks;
mod sha256;
mod support;
mod toolchains;

use cli::{AppPlatform, Arguments, Operation, OptionKey};
use development::Inspector;
use platforms_android::{apk::BuildAndroidApplication, AndroidArtifact};
use platforms_desktop::{DesktopBuild, SwiftPackage};
use platforms_ios::IOSBuild;
use project::{AppProject, CreateApplication, PackageTests};
use project_sdk::{CoveSDK, SDKRelease, SDKStore};
use project_tasks::{ProjectTaskRunner, ProjectWorkspace, WorkspaceDependencies};
use std::process::ExitCode;
use support::{Files, Shell, ToolError};

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    match run(&raw) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run(raw: &[String]) -> Result<u8, ToolError> {
    if raw == ["--version"] {
        println!("cove {}", SDKRelease::current()?.version);
        return Ok(0);
    }
    // The installed SDK selected by cove.json or --sdk-root re-executes the
    // command so a newer toolchain owns its own flags.
    SDKCommandBootstrap::bootstrap(raw)?;
    if raw.is_empty()
        || raw
            .iter()
            .take_while(|token| token.as_str() != "--")
            .any(|token| token == "--help")
    {
        println!("{}", usage());
        return Ok(0);
    }
    let arguments = Arguments::parse(raw)?;
    SDKCommandBootstrap::forward(&arguments, raw)?;
    execute(&arguments)
}

fn execute(arguments: &Arguments) -> Result<u8, ToolError> {
    match arguments.operation {
        Operation::Task => {
            let root = CoveSDK::locate(arguments, None, &support::environment())?;
            ProjectTaskRunner::new(arguments, &root).run()?;
            Ok(0)
        }
        Operation::SelfUpdate => {
            installation::CLIUpdate::run(arguments.subject.as_deref())?;
            Ok(0)
        }
        Operation::InstallCli => {
            installation::CLIInstaller::install(
                &installation::InstallationReceipt::executable_directory(),
                &arguments.directory(),
                std::path::Path::new(arguments.required(OptionKey::BinDirectory)?),
            )?;
            Ok(0)
        }
        Operation::PackageCli => {
            installation::CLIArchive::create(
                &CoveSDK::locate(arguments, None, &support::environment())?,
                &arguments
                    .url(OptionKey::Output)
                    .unwrap_or_else(|| arguments.directory().join("dist")),
            )?;
            Ok(0)
        }
        Operation::PackageNative => {
            installation::NativeArchive::create(
                &CoveSDK::locate(arguments, None, &support::environment())?,
                &arguments
                    .url(OptionKey::Output)
                    .unwrap_or_else(|| arguments.directory().join("dist")),
                arguments,
            )?;
            Ok(0)
        }
        Operation::PackageBuild => {
            let root = CoveSDK::locate(arguments, None, &support::environment())?;
            let mut command = SwiftPackage::command(
                &arguments.directory(),
                arguments.configuration,
                "build",
                arguments.url(OptionKey::ScratchPath).as_deref(),
            );
            if let Some(product) = arguments.value(OptionKey::Product) {
                command.push("--product".into());
                command.push(product.into());
            }
            if let Some(target) = arguments.value(OptionKey::Target) {
                command.push("--target".into());
                command.push(target.into());
            }
            if let Some(triple) = arguments.value(OptionKey::Triple) {
                command.push("--triple".into());
                command.push(triple.into());
            }
            if let Some(sdk) = arguments.value(OptionKey::Sdk) {
                command.push("--sdk".into());
                command.push(sdk.into());
            }
            if arguments.flags.contains(&cli::Flag::BuildTests) {
                command.push("--build-tests".into());
            }
            let build_environment = SwiftPackage::environment(&root)?;
            Shell::run(
                &command,
                None,
                &build_environment,
                false,
                false,
                support::StdoutTarget::StandardOutput,
            )?;
            if let Some(result) = arguments.url(OptionKey::ResultFile) {
                let mut paths = command;
                paths.push("--show-bin-path".into());
                let products = Shell::run(
                    &paths,
                    None,
                    &build_environment,
                    true,
                    false,
                    support::StdoutTarget::StandardOutput,
                )?;
                #[derive(serde::Serialize)]
                struct Products {
                    products: String,
                }
                Files::json(&Products { products }, &result)?;
            }
            Ok(0)
        }
        Operation::Create => {
            CreateApplication::run(arguments, &CoveSDK::locate(arguments, None, &support::environment())?)?;
            Ok(0)
        }
        Operation::Clean => {
            let project = AppProject::new(&arguments.directory())?;
            project.clean()?;
            println!("Cleaned {}", project.build_directory().display());
            Ok(0)
        }
        Operation::Test => {
            PackageTests::new(arguments)?.run()?;
            Ok(0)
        }
        Operation::Build | Operation::Run => {
            let project = AppProject::new(&arguments.directory())?;
            let root = CoveSDK::locate(arguments, Some(&project), &support::environment())?;
            project.require(arguments.platform)?;
            if let Some(workspace) = ProjectWorkspace::find(&project.directory)? {
                WorkspaceDependencies::refresh(&workspace, &root, &support::environment())?;
                ProjectTaskRunner::new(arguments, &root).run_before_build(&workspace)?;
            }
            if arguments.platform == AppPlatform::Android {
                BuildAndroidApplication::run(arguments, &root, Some(&project))?;
            } else if arguments.platform == AppPlatform::Ios {
                IOSBuild::new(project, &root, arguments).run()?;
            } else {
                DesktopBuild::new(project, &root, arguments).run()?;
            }
            Ok(0)
        }
        Operation::Devtools => {
            let project = AppProject::new(&arguments.directory())?;
            let root = CoveSDK::locate(arguments, Some(&project), &support::environment())?;
            Inspector::attach(arguments, &project, &root)?;
            Ok(0)
        }
        Operation::AndroidPrepare => {
            let project = AppProject::new(&arguments.directory())?;
            let root = CoveSDK::locate(arguments, Some(&project), &support::environment())?;
            BuildAndroidApplication::prepare(arguments, &root, &project)?;
            Ok(0)
        }
        Operation::SdkInstall => {
            let subject = arguments
                .subject
                .clone()
                .ok_or_else(|| ToolError::invalid("Supply a project directory"))?;
            SDKStore::new(None).install(&subject)?;
            Ok(0)
        }
        Operation::SdkUse => {
            let store = SDKStore::new(None);
            let subject = arguments
                .subject
                .clone()
                .ok_or_else(|| ToolError::invalid("Supply a project directory"))?;
            let root = if SDKRelease::is_version(&subject) {
                store.installed(&subject)
            } else {
                arguments.directory()
            };
            if SDKRelease::is_version(&subject) {
                let commands = installation::InstallationReceipt::read(&store.directory)
                    .map(|receipt| receipt.commands)
                    .unwrap_or_else(|_| installation::Installation::commands());
                installation::CLIInstaller::install(&root, &store.directory, &commands)?;
            } else {
                store.select(&root)?;
            }
            let cwd = std::env::current_dir().unwrap_or_else(|_| ".".into());
            if let Ok(project) = AppProject::new(&cwd) {
                project.pin(&SDKRelease::read(&root)?.version)?;
            }
            Ok(0)
        }
        Operation::SdkList => {
            SDKStore::new(None).list()?;
            Ok(0)
        }
        Operation::Setup => {
            toolchains::ToolchainSetup::run(
                arguments,
                &CoveSDK::locate(arguments, None, &support::environment())?,
            )?;
            Ok(0)
        }
        Operation::Doctor => {
            cli::Diagnostics::doctor(
                arguments,
                &CoveSDK::locate(arguments, None, &support::environment())?,
            )?;
            Ok(0)
        }
        Operation::Devices => {
            cli::Diagnostics::devices(arguments)?;
            Ok(0)
        }
        Operation::Showcase => {
            platforms_android::apk::showcase(
                arguments,
                &CoveSDK::locate(arguments, None, &support::environment())?,
            )?;
            Ok(0)
        }
        Operation::AndroidExport => {
            AndroidArtifact::export(
                arguments,
                &CoveSDK::locate(arguments, None, &support::environment())?,
            )?;
            Ok(0)
        }
        Operation::AndroidCompile => {
            platforms_android::compile(
                arguments,
                &CoveSDK::locate(arguments, None, &support::environment())?,
            )?;
            Ok(0)
        }
    }
}

/// Re-executes the command with the SDK the project selects.
struct SDKCommandBootstrap;

impl SDKCommandBootstrap {
    fn bootstrap(raw: &[String]) -> Result<(), ToolError> {
        let Some(verb) = raw.first() else {
            return Ok(());
        };
        if ["self", "sdk", "internal"].contains(&verb.as_str()) {
            return Ok(());
        }
        let store = SDKStore::new(None);
        if let Some(root) = project_sdk::SDKCommand::selected_sdk(raw, &store, &support::environment())?
        {
            project_sdk::SDKCommand::invoke(&root, raw)?;
        }
        Ok(())
    }

    fn forward(arguments: &Arguments, raw: &[String]) -> Result<(), ToolError> {
        project_sdk::SDKCommand::forward(arguments, raw)
    }
}

/// `cove --help`, matching the Swift usage string verbatim.
fn usage() -> &'static str {
    "Cove — create, build and run cross-platform Swift applications

  cove create app <directory> [--id com.example.app] [--name 'My App']
  cove build [directory] [--mac|--win|--linux|--android|--ios|--web] [--debug|--profile|--release]
  cove run   [directory] [--mac|--win|--linux|--android|--ios|--web] [--debug|--profile|--release]
  cove test  [directory] [--debug|--profile|--release] [--filter TEST] [--no-parallel]
              [--build-only|--skip-build] [--testing-library xctest|swift-testing]
              [--timeout SECONDS] [--event-output FILE] [--scratch-path DIRECTORY]
  cove clean [directory]
  cove task [NAME] [directory] [--sdk-root PATH] [-- task arguments]
  cove self update [VERSION]
  cove sdk install VERSION|latest
  cove sdk use VERSION|PATH | cove sdk list
  cove setup [--mac|--win|--linux|--android|--ios]
  cove doctor [--mac|--win|--linux|--android|--ios]
  cove devices [--android|--ios]
  cove devtools [directory] [--session ID] [--no-open]
  cove devtools [directory] [--session ID] list | query [TOOL] [JSON|@FILE]
                         | click NODE_ID [--capture OUT.jpg] | activate NODE_ID [increment|decrement] | reveal NODE_ID
                         | input JSON|@FILE [--capture OUT.jpg]
                         | capture OUT.jpg [--settle-ms N] [--scale 1-3] [--node NODE_ID | --region X,Y,W,H] | resize W H [--capture OUT.jpg] | timings OUT.json FRAMES WARMUP

Directory defaults to the current project; platform defaults to the host; configuration defaults to debug.
One cove.json owns application identity, SDK location and targets; UI lives in Sources/AppContent.
Test accepts application projects and plain Swift packages; plain packages keep their native .build cache.
Desktop builds run on their matching host. Android uses Swift 6.4.1 + Android SDK, NDK r30 and JDK 21.
iOS uses Xcode, SDL/UIKit and Metal; Web has a portable rendering prototype.

SDK selection: --sdk-root PATH, COVE_SDK_ROOT, cove.json's toolchain.sdkVersion, or the installed SDK selection.
Run: --devtools [--no-open] opens the Inspector; --profile enables optimized diagnostics.
Run: arguments after -- go to the desktop application, e.g. cove run CoveUI/Examples -- --component list --width 390.
DevTools actions resolve the live session; query with a bare JSON object runs query_ui_nodes.
Create: --platforms mac,android,ios selects native project templates.
Build/run: --output PATH; Android: --android-sdk PATH --ndk PATH --device SERIAL
iOS: --simulator UDID (default: booted); physical device: --device ID --team TEAM_ID
Android signing: --keystore FILE --key-alias ALIAS --store-password-env NAME --key-password-env NAME
Release APKs use a development key unless custom signing is provided.
Plugin: compiles Sources/ with CovePluginKit into an Embedded Swift WebAssembly Component.

Framework development:
  cove showcase [gallery|counter] --android [--component NAME] [--release]"
}
