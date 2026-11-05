//! Bundles, signs and deploys Cove applications. Cargo builds the code; this
//! crate does what Cargo does not: `.app`/APK assembly, signing and device
//! installation. Applications call [`main`] from their `xtask` crate.

mod android;
mod apple;
mod audit;
mod desktop;
mod process;
mod project;
mod setup_config;

use clap::{Args, Parser, Subcommand, ValueEnum};
use project::Project;
use std::path::PathBuf;
use std::process::ExitCode;

pub(crate) type Result<T> = std::result::Result<T, String>;

#[derive(Parser)]
#[command(name = "cargo xtask", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Prepare native build tools and SDKs. Does not compile FFmpeg.
    Setup {
        #[arg(long)]
        media: bool,
        /// Check without installing or changing system configuration.
        #[arg(long)]
        check: bool,
        /// Show the host installation plan without executing it.
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        target: Vec<String>,
    },
    /// Check the Rust workspace layout and retained Swift migration mappings.
    Audit {
        #[arg(long)]
        json: bool,
    },
    /// Build and bundle the application.
    Package(Target),
    /// Bundle, install and launch the application.
    Run(Target),
    /// Build the Android library and stage the Gradle inputs.
    AndroidPrepare {
        #[arg(long, value_enum, default_value_t = Mode::Debug)]
        mode: Mode,
        #[arg(long)]
        package: Option<String>,
    },
    /// List devices and simulators.
    Devices {
        #[arg(value_enum)]
        platform: Platform,
    },
}

#[derive(Args)]
pub(crate) struct Target {
    /// Defaults to the host platform.
    #[arg(value_enum)]
    platform: Option<Platform>,
    #[arg(long)]
    package: Option<String>,
    #[arg(long, conflicts_with = "profile")]
    release: bool,
    /// Optimized and debuggable.
    #[arg(long)]
    profile: bool,
    #[arg(long)]
    output: Option<PathBuf>,
    /// ADB serial, simulator UDID or iOS device identifier.
    #[arg(long)]
    device: Option<String>,
    /// Build for the iOS Simulator.
    #[arg(long)]
    simulator: bool,
    /// Code-signing identity (Apple).
    #[arg(long)]
    identity: Option<String>,
    /// Provisioning profile embedded in device builds (iOS).
    #[arg(long)]
    provisioning_profile: Option<PathBuf>,
    /// Release keystore (Android); the debug key signs otherwise.
    #[arg(long, requires_all = ["key_alias", "store_password_env", "key_password_env"])]
    keystore: Option<PathBuf>,
    #[arg(long)]
    key_alias: Option<String>,
    #[arg(long)]
    store_password_env: Option<String>,
    #[arg(long)]
    key_password_env: Option<String>,
    /// Arguments passed to a desktop application.
    #[arg(last = true)]
    arguments: Vec<String>,
}

impl Target {
    fn mode(&self) -> Mode {
        if self.release {
            Mode::Release
        } else if self.profile {
            Mode::Profile
        } else {
            Mode::Debug
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum Platform {
    #[value(alias = "macos")]
    Mac,
    #[value(alias = "win")]
    Windows,
    Linux,
    Android,
    Ios,
}

impl Platform {
    fn host() -> Self {
        if cfg!(target_os = "macos") {
            Self::Mac
        } else if cfg!(windows) {
            Self::Windows
        } else {
            Self::Linux
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Mac => "mac",
            Self::Windows => "windows",
            Self::Linux => "linux",
            Self::Android => "android",
            Self::Ios => "ios",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum Mode {
    Debug,
    Profile,
    Release,
}

impl Mode {
    pub fn optimized(self) -> bool {
        self != Self::Debug
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Profile => "profile",
            Self::Release => "release",
        }
    }
}

pub fn main() -> ExitCode {
    finish(Cli::parse())
}

/// Entry point for Cargo's external `cargo-cove` subcommand protocol.
pub fn cargo_main() -> ExitCode {
    let mut arguments: Vec<_> = std::env::args_os().collect();
    if arguments.get(1).is_some_and(|arg| arg == "cove") {
        arguments.remove(1);
    }
    finish(Cli::parse_from(arguments))
}

fn finish(cli: Cli) -> ExitCode {
    match execute(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn execute(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Setup {
            media,
            check,
            dry_run,
            target,
        } => {
            let targets = target.clone();
            let mut report = cove_native_build::setup::run(&cove_native_build::setup::Options {
                media,
                check,
                dry_run,
                targets: target,
            })
            .map_err(|e| e.to_string())?;
            let project = std::env::current_dir().map_err(|e| e.to_string())?;
            if !check && !dry_run && report["ready"] == true {
                // Setup edits the project's Cargo config, which is tracked inside this
                // workspace, so the modified path must be visible in the report.
                if let Some(path) = setup_config::configure_android(&project, &targets)? {
                    report["cargo_config_updated"] = path.display().to_string().into();
                }
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
            if !dry_run && report["ready"] != true {
                return Err(
                    "native tools or SDKs are not ready; run cargo cove setup --media".into(),
                );
            }
            Ok(())
        }
        Command::Audit { json } => audit::run(json),
        Command::Package(target) => {
            let platform = target.platform.unwrap_or_else(Platform::host);
            let project = Project::load(target.package.as_deref())?;
            let product = bundle(&project, platform, &target)?;
            println!("{}", product.display());
            Ok(())
        }
        Command::Run(target) => {
            let platform = target.platform.unwrap_or_else(Platform::host);
            let project = Project::load(target.package.as_deref())?;
            let product = bundle(&project, platform, &target)?;
            match platform {
                Platform::Android => android::launch(&project, &product, target.device.as_deref()),
                Platform::Ios => apple::launch_ios(&project, &product, &target),
                Platform::Mac => apple::launch_mac(&project, &product, &target.arguments),
                Platform::Windows | Platform::Linux => {
                    desktop::launch(&project, &product, &target.arguments)
                }
            }
        }
        Command::AndroidPrepare { mode, package } => {
            android::prepare(&Project::load(package.as_deref())?, mode).map(drop)
        }
        Command::Devices { platform } => match platform {
            Platform::Android => android::devices(),
            Platform::Ios => apple::devices(),
            _ => Err("devices lists android or ios targets".into()),
        },
    }
}

fn bundle(project: &Project, platform: Platform, target: &Target) -> Result<PathBuf> {
    let host = Platform::host();
    if matches!(
        platform,
        Platform::Mac | Platform::Windows | Platform::Linux
    ) && platform != host
    {
        return Err(format!(
            "{} bundles are built on a {} host",
            platform.name(),
            platform.name()
        ));
    }
    if platform == Platform::Ios && host != Platform::Mac {
        return Err("iOS bundles are built on macOS".into());
    }
    let output = project.output(target.output.as_deref(), platform.name(), target.mode());
    match platform {
        Platform::Mac => apple::bundle_mac(project, target, &output),
        Platform::Ios => apple::bundle_ios(project, target, &output),
        Platform::Android => android::package(project, target, &output),
        Platform::Windows => desktop::bundle_windows(project, target, &output),
        Platform::Linux => desktop::bundle_linux(project, target, &output),
    }
}
