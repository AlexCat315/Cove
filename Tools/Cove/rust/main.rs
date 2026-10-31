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
use platforms_android::{AndroidArtifact, apk::BuildAndroidApplication};
use platforms_desktop::{CargoPackage, DesktopBuild};
use platforms_ios::IOSBuild;
use project::{AppProject, CreateApplication, PackageTests};
use project_sdk::CoveSDK;
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
        println!("cove {}", env!("CARGO_PKG_VERSION"));
        return Ok(0);
    }
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
    execute(&arguments)
}

fn execute(arguments: &Arguments) -> Result<u8, ToolError> {
    match arguments.operation {
        Operation::Task => {
            let root = CoveSDK::locate(arguments, None, &support::environment())?;
            ProjectTaskRunner::new(arguments, &root).run()?;
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
            let scratch = arguments.url(OptionKey::ScratchPath);
            let mut command = CargoPackage::command(
                &arguments.directory(),
                arguments.configuration,
                "build",
                scratch.as_deref(),
            );
            if let Some(product) = arguments.value(OptionKey::Product) {
                command.push("--bin".into());
                command.push(product.into());
            }
            if let Some(target) = arguments.value(OptionKey::Target) {
                command.push("--package".into());
                command.push(target.into());
            }
            if let Some(triple) = arguments.value(OptionKey::Triple) {
                command.push("--target".into());
                command.push(triple.into());
            }
            if arguments.flags.contains(&cli::Flag::BuildTests) {
                command.push("--tests".into());
            }
            let build_environment = CargoPackage::environment(&root)?;
            Shell::run(
                &command,
                None,
                &build_environment,
                false,
                false,
                support::StdoutTarget::StandardOutput,
            )?;
            if let Some(result) = arguments.url(OptionKey::ResultFile) {
                #[derive(serde::Serialize)]
                struct Products {
                    products: String,
                }
                let products = CargoPackage::products(
                    &arguments.directory(),
                    arguments.configuration,
                    scratch.as_deref(),
                );
                Files::json(
                    &Products {
                        products: products.display().to_string(),
                    },
                    &result,
                )?;
            }
            Ok(0)
        }
        Operation::Create => {
            CreateApplication::run(
                arguments,
                &CoveSDK::locate(arguments, None, &support::environment())?,
            )?;
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
        Operation::Fmt | Operation::Clippy | Operation::Check => {
            CargoVerb::run(arguments)?;
            Ok(0)
        }
        Operation::Build | Operation::Run if !arguments.directory().join("cove.json").is_file() => {
            CargoVerb::run(arguments)?;
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
        Operation::Migration => {
            MigrationStatus::run(arguments)?;
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

/// `cove fmt`, `cove clippy`, `cove check`: thin Cargo passthrough.
///
/// These exist so one command works on every host with the same flags; they
/// never reimplement Cargo's own work.
struct CargoVerb;

impl CargoVerb {
    fn run(arguments: &Arguments) -> Result<(), ToolError> {
        let directory = arguments.directory();
        if !directory.join("Cargo.toml").is_file() {
            return Err(ToolError::invalid(format!(
                "{} must contain Cargo.toml",
                directory.display()
            )));
        }
        let (verb, extra): (&str, Vec<String>) = match arguments.operation {
            Operation::Fmt => {
                let mut extra = vec!["--all".into()];
                if !arguments.flags.contains(&cli::Flag::Fix) {
                    extra.push("--check".into());
                }
                ("fmt", extra)
            }
            Operation::Clippy => {
                // Severity stays the caller's: append `-- -D warnings` to fail
                // on lints, exactly as with Cargo itself.
                let mut extra = Vec::new();
                if arguments.flags.contains(&cli::Flag::Fix) {
                    extra.push("--fix".into());
                }
                ("clippy", extra)
            }
            Operation::Run => ("run", Vec::new()),
            Operation::Build => ("build", Vec::new()),
            _ => ("check", Vec::new()),
        };
        let mut command = vec![
            crate::platforms_desktop::cargo(),
            verb.into(),
            "--manifest-path".into(),
            directory.join("Cargo.toml").display().to_string(),
        ];
        if let Some(package) = arguments.value(OptionKey::Package) {
            command.push("--package".into());
            command.push(package.into());
        }
        if let Some(target) = arguments.value(OptionKey::Target) {
            command.push("--target".into());
            command.push(target.into());
        }
        if arguments.configuration != cli::BuildConfiguration::Debug
            && matches!(arguments.operation, Operation::Build | Operation::Run)
        {
            command.push("--release".into());
        }
        for flag in [cli::Flag::AllTargets, cli::Flag::Locked, cli::Flag::Offline] {
            if arguments.flags.contains(&flag) {
                if let Some(argument) = flag.cargo_argument() {
                    command.push(argument.into());
                }
            }
        }
        command.extend(extra);
        command.extend(arguments.application_arguments.iter().cloned());
        Shell::run(
            &command,
            Some(&directory),
            &support::environment(),
            false,
            false,
            support::StdoutTarget::StandardOutput,
        )?;
        Ok(())
    }
}

/// `cove migration status`: counts the Swift sources that still have no Rust
/// counterpart, from `Docs/rust-migration-files.json`.
struct MigrationStatus;

impl MigrationStatus {
    fn run(arguments: &Arguments) -> Result<(), ToolError> {
        let manifest = crate::support::environment()
            .get("COVE_SDK_ROOT")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::current_dir().ok())
            .ok_or_else(|| ToolError::invalid("Cannot determine the SDK root"))?;
        let inventory = manifest.join("Docs/rust-migration-files.json");
        let data = std::fs::read(&inventory)
            .map_err(|error| ToolError::invalid(format!("{}: {error}", inventory.display())))?;
        let document: serde_json::Value = serde_json::from_slice(&data)
            .map_err(|error| ToolError::invalid(format!("{}: {error}", inventory.display())))?;
        let entries = document
            .get("entries")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| ToolError::invalid("entries is missing from the inventory"))?;
        let mut production = 0usize;
        let mut pending = 0usize;
        for entry in entries {
            if entry.get("kind").and_then(serde_json::Value::as_str) != Some("production") {
                continue;
            }
            production += 1;
            if entry.get("status").and_then(serde_json::Value::as_str) == Some("pending") {
                pending += 1;
            }
        }
        if arguments.flags.contains(&cli::Flag::Json) {
            #[derive(serde::Serialize)]
            struct Report {
                production: usize,
                written: usize,
                pending: usize,
            }
            let report = Report {
                production,
                written: production - pending,
                pending,
            };
            println!(
                "{}",
                serde_json::to_string(&report)
                    .map_err(|error| ToolError::invalid(error.to_string()))?
            );
        } else {
            println!("production entries: {production}");
            println!("written: {}", production - pending);
            println!("pending: {pending}");
        }
        if arguments.flags.contains(&cli::Flag::RequireComplete) && pending != 0 {
            return Err(ToolError::invalid(format!(
                "{pending} production sources are not migrated"
            )));
        }
        Ok(())
    }
}

/// `cove --help`, matching the Swift usage string verbatim.
fn usage() -> &'static str {
    "Cove — create, build and run cross-platform Cove applications

  cove create app <directory> [--id com.example.app] [--name 'My App'] [--platforms mac,android,ios]
  cove build [directory] [--mac|--win|--linux|--android|--ios] [--debug|--release] [--package NAME]
  cove run   [directory] [--mac|--win|--linux|--android|--ios] [--debug|--release] [--package NAME]
  cove test  [directory] [--debug|--release] [--filter TEST] [--no-parallel]
              [--build-only|--skip-build] [--timeout SECONDS] [--scratch-path DIRECTORY]
  cove fmt   [directory] [--fix] [--locked]
  cove clippy [directory] [--all-targets] [--locked|--offline] [--fix] [--package NAME]
  cove check [directory] [--all-targets] [--locked|--offline] [--package NAME] [--target TRIPLE]
  cove clean [directory]
  cove task [NAME] [directory] [-- task arguments]
  cove doctor [--mac|--win|--linux|--android|--ios]
  cove devices [--android|--ios]
  cove migration status [--json] [--require-complete]
  cove devtools [directory] [--session ID] [--no-open]
  cove devtools [directory] [--session ID] list | query [TOOL] [JSON|@FILE]
                         | click NODE_ID [--capture OUT.jpg] | activate NODE_ID [increment|decrement] | reveal NODE_ID
                         | input JSON|@FILE [--capture OUT.jpg]
                         | capture OUT.jpg [--settle-ms N] [--scale 1-3] [--node NODE_ID | --region X,Y,W,H] | resize W H [--capture OUT.jpg] | timings OUT.json FRAMES WARMUP

Directory defaults to the current project; platform defaults to the host; configuration defaults to debug.
One cove.json owns application identity and targets; the application is a Cargo package.
fmt/clippy/check are thin Cargo passthroughs; build and test drive Cargo directly.
Desktop builds run on their matching host. Android cross-compiles with the NDK; iOS uses Xcode.
Run: --devtools [--no-open] opens the Inspector.
Run: arguments after -- go to the desktop application.
DevTools actions resolve the live session; query with a bare JSON object runs query_ui_nodes.
Create: --platforms mac,windows,linux,android,ios selects native project templates."
}
