//! Bundles, signs and deploys Cove applications. Cargo builds the code; this
//! crate does what Cargo does not: `.app`/APK assembly, signing and device
//! installation. Applications call [`main`] from their `xtask` crate.

mod android;
mod apple;
mod audit;
mod desktop;
mod process;
mod project;

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
    match execute(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn execute(cli: Cli) -> Result<()> {
    match cli.command {
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
