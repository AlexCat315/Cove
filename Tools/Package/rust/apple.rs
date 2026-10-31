//! macOS and iOS `.app` bundles, signing and simulator/device deployment.
//!
//! iOS runs the same Rust binary as the desktop: Shell enters UIKit through
//! `SDL_RunApp`, so no Xcode project is involved.

use crate::process;
use crate::project::Project;
use crate::{Result, Target};
use std::path::{Path, PathBuf};
use std::process::Command;

const MINIMUM_VERSION: &str = "26.0";

enum Plist<'a> {
    String(&'a str),
    Json(&'a str),
}

/// Writes `Info.plist` from the project's base (if any) plus Cove's keys.
fn info_plist(base: &Path, destination: &Path, entries: &[(&str, Plist)]) -> Result<()> {
    if base.is_file() {
        process::copy(base, destination)?;
    } else {
        process::write(
            destination,
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict/></plist>\n",
        )?;
    }
    for (key, value) in entries {
        let (kind, value) = match value {
            Plist::String(value) => ("-string", *value),
            Plist::Json(value) => ("-json", *value),
        };
        process::run(
            Command::new("plutil")
                .args(["-replace", key, kind, value])
                .arg(destination),
        )?;
    }
    Ok(())
}

fn identity_entries<'a>(
    project: &'a Project,
    executable: &'a str,
    build: &'a str,
) -> Vec<(&'static str, Plist<'a>)> {
    vec![
        ("CFBundlePackageType", Plist::String("APPL")),
        ("CFBundleIdentifier", Plist::String(&project.id)),
        ("CFBundleName", Plist::String(&project.name)),
        ("CFBundleDisplayName", Plist::String(&project.name)),
        ("CFBundleExecutable", Plist::String(executable)),
        (
            "CFBundleShortVersionString",
            Plist::String(&project.version),
        ),
        ("CFBundleVersion", Plist::String(build)),
        ("CFBundleInfoDictionaryVersion", Plist::String("6.0")),
    ]
}

fn sign(bundle: &Path, identity: &str, entitlements: Option<&Path>, hardened: bool) -> Result<()> {
    let mut command = Command::new("codesign");
    command.args(["--force", "--sign", identity]);
    if let Some(entitlements) = entitlements {
        command.arg("--entitlements").arg(entitlements);
    }
    if hardened {
        command.args(["--options", "runtime", "--timestamp"]);
    }
    process::run(command.arg(bundle))
}

pub fn bundle_mac(project: &Project, target: &Target, output: &Path) -> Result<PathBuf> {
    let executable = project.build_bin(target.mode(), None)?;
    let bin = project.bin()?;
    let app = output.join(format!("{}.app", project.name));
    process::remove_dir(&app)?;
    let contents = app.join("Contents");
    process::copy(&executable, &contents.join("MacOS").join(bin))?;
    let resources = contents.join("Resources");
    project.stage_resources(&resources)?;
    let platform = project.platforms("macOS");
    process::copy_tree(&platform.join("Resources"), &resources)?;
    let build = project.build.to_string();
    let mut entries = identity_entries(project, bin, &build);
    entries.push(("LSMinimumSystemVersion", Plist::String(MINIMUM_VERSION)));
    if resources.join("AppIcon.icns").is_file() {
        entries.push(("CFBundleIconFile", Plist::String("AppIcon")));
    }
    info_plist(
        &platform.join("Info.plist"),
        &contents.join("Info.plist"),
        &entries,
    )?;
    let identity = target.identity.as_deref().unwrap_or("-");
    let entitlements = platform.join("Entitlements.plist");
    sign(
        &app,
        identity,
        entitlements.is_file().then_some(entitlements.as_path()),
        identity != "-",
    )?;
    Ok(app)
}

pub fn launch_mac(project: &Project, app: &Path, arguments: &[String]) -> Result<()> {
    process::run(Command::new(app.join("Contents/MacOS").join(project.bin()?)).args(arguments))
}

pub fn bundle_ios(project: &Project, target: &Target, output: &Path) -> Result<PathBuf> {
    let triple = if target.simulator {
        "aarch64-apple-ios-sim"
    } else {
        "aarch64-apple-ios"
    };
    let executable = project.build_bin(target.mode(), Some(triple))?;
    let bin = project.bin()?;
    let app = output.join(format!("{}.app", project.name));
    process::remove_dir(&app)?;
    process::copy(&executable, &app.join(bin))?;
    project.stage_resources(&app)?;
    let platform = project.platforms("iOS");
    process::copy_tree(&platform.join("Resources"), &app)?;
    let build = project.build.to_string();
    let mut entries = identity_entries(project, bin, &build);
    entries.extend([
        ("MinimumOSVersion", Plist::String(MINIMUM_VERSION)),
        (
            "CFBundleSupportedPlatforms",
            Plist::Json(if target.simulator {
                "[\"iPhoneSimulator\"]"
            } else {
                "[\"iPhoneOS\"]"
            }),
        ),
        ("UIDeviceFamily", Plist::Json("[1,2]")),
        ("UIRequiredDeviceCapabilities", Plist::Json("[\"arm64\"]")),
        ("UILaunchScreen", Plist::Json("{}")),
        ("LSRequiresIPhoneOS", Plist::Json("true")),
    ]);
    info_plist(
        &platform.join("Info.plist"),
        &app.join("Info.plist"),
        &entries,
    )?;

    if target.simulator {
        sign(&app, "-", None, false)?;
        return Ok(app);
    }
    let identity = target
        .identity
        .as_deref()
        .ok_or("device builds need --identity \"Apple Development: ...\"")?;
    let profile = target
        .provisioning_profile
        .as_deref()
        .ok_or("device builds need --provisioning-profile PATH.mobileprovision")?;
    process::copy(profile, &app.join("embedded.mobileprovision"))?;
    let scratch = project.target_dir.join("cove/ios/signing");
    process::create_dir(&scratch)?;
    let decoded = scratch.join("profile.plist");
    let entitlements = scratch.join("entitlements.plist");
    process::run(
        Command::new("security")
            .args(["cms", "-D", "-i"])
            .arg(profile)
            .arg("-o")
            .arg(&decoded),
    )?;
    process::run(
        Command::new("plutil")
            .args(["-extract", "Entitlements", "xml1", "-o"])
            .arg(&entitlements)
            .arg(&decoded),
    )?;
    sign(&app, identity, Some(&entitlements), false)?;
    Ok(app)
}

pub fn launch_ios(project: &Project, app: &Path, target: &Target) -> Result<()> {
    if target.simulator {
        let device = target.device.as_deref().unwrap_or("booted");
        if device != "booted" {
            // Already-booted simulators report an error; installation decides.
            let _ = Command::new("xcrun")
                .args(["simctl", "boot", device])
                .status();
        }
        process::run(
            Command::new("xcrun")
                .args(["simctl", "install", device])
                .arg(app),
        )?;
        return process::run(Command::new("xcrun").args([
            "simctl",
            "launch",
            "--console-pty",
            device,
            &project.id,
        ]));
    }
    let device = target
        .device
        .as_deref()
        .ok_or("pass --device ID (see `cargo xtask devices ios`)")?;
    process::run(
        Command::new("xcrun")
            .args(["devicectl", "device", "install", "app", "--device", device])
            .arg(app),
    )?;
    process::run(Command::new("xcrun").args([
        "devicectl",
        "device",
        "process",
        "launch",
        "--device",
        device,
        &project.id,
    ]))
}

pub fn devices() -> Result<()> {
    process::run(Command::new("xcrun").args(["simctl", "list", "devices", "available"]))?;
    process::run(Command::new("xcrun").args(["devicectl", "list", "devices"]))
}
