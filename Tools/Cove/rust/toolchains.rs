//! Toolchain inspection and preparation.
//!
//! Swift sources: `Toolchains/Android.swift`, `Toolchains/Native.swift`,
//! `Toolchains/Requirements.swift`, `Toolchains/Setup.swift`.
//!
//! Every check reports the same name, readiness and remedy text as the original
//! so `cove doctor` output stays byte-identical.

use crate::cli::{AppPlatform, Arguments, OptionKey};
use crate::installation::CLIUpdate;
use crate::platforms_android::AndroidToolchain;
use crate::project_sdk::NativeArtifacts;
use crate::support::{Shell, StdoutTarget, ToolError, environment};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// One row of `cove doctor` output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolchainCheck {
    pub name: String,
    pub ready: bool,
    pub remedy: String,
}

impl ToolchainCheck {
    pub fn new(name: impl Into<String>, ready: bool, remedy: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ready,
            remedy: remedy.into(),
        }
    }

    pub fn executable(name: &str, remedy: &str) -> Self {
        Self::new(
            name,
            Shell::executable(name, &environment()).is_ok(),
            remedy,
        )
    }
}

/// Compiled-in Apple SDK selection used by native preparation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppleTarget {
    pub sdk: String,
    pub architecture: String,
}

impl AppleTarget {
    pub fn variant(&self) -> String {
        format!(
            "ios-{}{}",
            self.architecture,
            if self.sdk == "iphonesimulator" {
                "-simulator"
            } else {
                ""
            }
        )
    }
}

/// Shared native archive preparation (SDL3, lunasvg, plutovg, webp, fonts).
pub struct NativeSDK;

impl NativeSDK {
    pub fn prepare(
        root: &Path,
        apple: Option<&AppleTarget>,
        from_source: bool,
    ) -> Result<(), ToolError> {
        let target = apple
            .map(AppleTarget::variant)
            .unwrap_or_else(CLIUpdate::platform);
        if !from_source && NativeArtifacts::install(root, &target, &installation_cache())? {
            return Ok(());
        }
        if !NativeArtifacts::has_source_inputs(root) {
            return Err(ToolError::invalid(format!(
                "Missing native libraries for {target}; reinstall a complete SDK release. Building native libraries requires a source checkout."
            )));
        }
        let mut build_environment = environment();
        #[cfg(not(windows))]
        let _ = &mut build_environment;
        #[cfg(windows)]
        cove_tool_support::windows_build_environment::activate(&mut build_environment)
            .map_err(|error| ToolError::invalid(error.to_string()))?;
        for domain in ["Native", "Text"] {
            let native = domain == "Native";
            let source = if native {
                root.join("Native/third-party")
            } else {
                root.join("third-party")
            };
            let base = if native {
                root.join("Native")
            } else {
                root.to_path_buf()
            };
            let build = base.join("build").join(match apple {
                Some(apple) => apple.variant(),
                None => "native".into(),
            });
            let artifacts = NativeArtifacts::directory(root, &target);
            let vendor = artifacts.join(if native { "Native/vendor" } else { "vendor" });
            let mut names: Vec<&str> = if native {
                vec!["SDL3", "lunasvg", "plutovg", "webp", "sharpyuv"]
            } else {
                vec!["CFreeType", "CHarfBuzz"]
            };
            if !native && apple.is_none() {
                names.push("GhosttyVt");
            }
            let prepared = names.iter().all(|name| {
                vendor
                    .join(format!("{name}.artifactbundle/info.json"))
                    .is_file()
            });
            if prepared {
                continue;
            }
            let mut configure = vec![
                "cmake".to_string(),
                "-S".into(),
                source.display().to_string(),
                "-B".into(),
                build.display().to_string(),
                "-DCMAKE_BUILD_TYPE=Release".into(),
                format!("-DCOVE_VENDOR_DIR={}", vendor.display()),
            ];
            if let Some(apple) = apple {
                let sysroot = Shell::run(
                    &[
                        "xcrun".into(),
                        "--sdk".into(),
                        apple.sdk.clone(),
                        "--show-sdk-path".into(),
                    ],
                    None,
                    &build_environment,
                    true,
                    false,
                    StdoutTarget::StandardOutput,
                )?;
                configure.push("-G".into());
                configure.push("Ninja".into());
                configure.push("-DCMAKE_SYSTEM_NAME=iOS".into());
                configure.push(format!("-DCMAKE_OSX_SYSROOT={sysroot}"));
                configure.push(format!("-DCMAKE_OSX_ARCHITECTURES={}", apple.architecture));
                configure.push("-DCMAKE_OSX_DEPLOYMENT_TARGET=26.0".into());
            }
            Shell::run(
                &configure,
                None,
                &build_environment,
                false,
                false,
                StdoutTarget::StandardOutput,
            )?;
            let mut build_command = vec![
                "cmake".to_string(),
                "--build".into(),
                build.display().to_string(),
                "--config".into(),
                "Release".into(),
                "--parallel".into(),
                "4".into(),
            ];
            Shell::run(
                &build_command,
                None,
                &build_environment,
                false,
                false,
                StdoutTarget::StandardOutput,
            )?;
            if native {
                build_command.push("--target".into());
                build_command.push("stage_sdl3".into());
                build_command.push("stage_image_decode".into());
                Shell::run(
                    &build_command,
                    None,
                    &build_environment,
                    false,
                    false,
                    StdoutTarget::StandardOutput,
                )?;
            }
            Shell::run(
                &[
                    "cmake".into(),
                    "--install".into(),
                    build.display().to_string(),
                    "--config".into(),
                    "Release".into(),
                    "--prefix".into(),
                    build.join("install").display().to_string(),
                ],
                None,
                &build_environment,
                false,
                false,
                StdoutTarget::StandardOutput,
            )?;
        }
        Ok(())
    }
}

/// Where downloaded native archives are cached; mirrors `Installation.directory`.
fn installation_cache() -> PathBuf {
    crate::installation::Installation::directory()
}

/// `cove doctor --android` requirements: JDK 21, SDK packages, NDK r30.
pub struct AndroidRequirements;

impl AndroidRequirements {
    pub fn inspect(arguments: &Arguments, root: &Path) -> Vec<ToolchainCheck> {
        let sdk = AndroidToolchain::sdk_directory(arguments);
        let install = format!(
            "Install Android Studio / Command-line Tools from https://developer.android.com/studio. \
             In SDK Manager install platform-tools, platforms;android-36, build-tools;36.1.0 and NDK r30. \
             Command line: sdkmanager --sdk_root=\"{}\" \"platform-tools\" \"platforms;android-36\" \"build-tools;36.1.0\" \"ndk;30.0.16248370\". \
             Review licenses using sdkmanager --licenses.",
            sdk.display()
        );
        let mut checks = vec![ToolchainCheck::executable(
            "java",
            "Install JDK 21 (https://adoptium.net/) and set JAVA_HOME and PATH to it.",
        )];
        // Java writes its version to stderr; javac reports to stdout on supported JDKs.
        let javac = Shell::run(
            &["javac".into(), "-version".into()],
            None,
            &environment(),
            true,
            true,
            StdoutTarget::StandardOutput,
        )
        .unwrap_or_default();
        checks.push(ToolchainCheck::new(
            "JDK 21",
            javac.starts_with("javac 21."),
            "Select JDK 21 using JAVA_HOME and PATH; run javac -version to verify.",
        ));
        for path in [
            "platform-tools/adb",
            "platforms/android-36/android.jar",
            "build-tools/36.1.0/apksigner",
            "build-tools/36.1.0/zipalign",
        ] {
            let suffix = if cfg!(windows) {
                if path.ends_with("adb") || path.ends_with("zipalign") {
                    ".exe"
                } else if path.ends_with("apksigner") {
                    ".bat"
                } else {
                    ""
                }
            } else {
                ""
            };
            checks.push(ToolchainCheck::new(
                format!("Android SDK {path}"),
                sdk.join(format!("{path}{suffix}")).is_file(),
                install.clone(),
            ));
        }
        let ndk = AndroidToolchain::ndk_directory(arguments, root);
        let ndk_version =
            std::fs::read_to_string(ndk.join("source.properties")).unwrap_or_default();
        checks.push(ToolchainCheck::new(
            "NDK r30",
            matches_pkg_revision_30(&ndk_version),
            format!("{install} Or supply --ndk PATH / ANDROID_NDK_HOME."),
        ));
        let linker = ndk
            .join("toolchains/llvm/prebuilt")
            .join("bin/aarch64-linux-android28-clang");
        checks.push(ToolchainCheck::new(
            "NDK arm64 linker",
            linker.is_file(),
            "Install NDK r30 with the arm64 API 28 toolchain, or supply --ndk PATH / ANDROID_NDK_HOME.",
        ));
        checks
    }
}

/// `Pkg.Revision\s*=\s*30\.`
fn matches_pkg_revision_30(contents: &str) -> bool {
    for line in contents.lines() {
        let Some(rest) = line.trim_start().strip_prefix("Pkg.Revision") else {
            continue;
        };
        let rest = rest.trim_start();
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        if rest.trim_start().starts_with("30.") {
            return true;
        }
    }
    false
}

/// Host tool requirements plus target-specific extras.
pub struct ToolchainRequirements;

impl ToolchainRequirements {
    pub fn inspect(arguments: &Arguments, root: &Path) -> Vec<ToolchainCheck> {
        let common = "Install Git, CMake and Ninja. macOS: brew install git cmake ninja; \
             Ubuntu/Debian: sudo apt-get install git cmake ninja-build build-essential pkg-config. \
             Windows: winget install Git.Git Kitware.CMake Ninja-build.Ninja; reopen the terminal.";
        let apple = AppleTarget {
            sdk: arguments
                .value(OptionKey::Sdk)
                .unwrap_or("iphonesimulator")
                .to_string(),
            architecture: arguments
                .value(OptionKey::Architecture)
                .unwrap_or("arm64")
                .to_string(),
        };
        let target = match arguments.platform {
            AppPlatform::Android => "android-arm64".to_string(),
            AppPlatform::Ios => apple.variant(),
            _ => CLIUpdate::platform(),
        };
        let prebuilt = NativeArtifacts::is_prepared(root, &target) || {
            std::fs::read(root.join("Artifacts/native.json"))
                .ok()
                .and_then(|data| serde_json::from_slice::<NativeArtifacts>(&data).ok())
                .map(|manifest| manifest.platforms.contains_key(&target))
                .unwrap_or(false)
        };
        let mut commands = if prebuilt {
            vec!["git"]
        } else {
            let mut commands = vec!["git", "cmake", "ninja"];
            if ![AppPlatform::Ios, AppPlatform::Android].contains(&arguments.platform) {
                commands.push("zig");
            }
            commands
        }
        .into_iter()
        .map(|name| ToolchainCheck::executable(name, common))
        .collect::<Vec<_>>();

        let version = Shell::run(
            &[crate::platforms_desktop::cargo(), "--version".into()],
            None,
            &environment(),
            true,
            true,
            StdoutTarget::StandardOutput,
        )
        .unwrap_or_default();
        commands.push(ToolchainCheck::new(
            "Rust 1.90+ (edition 2024)",
            !version.trim().is_empty(),
            "Install Rust via https://rustup.rs; rust-toolchain.toml pins the channel.",
        ));

        if cfg!(target_os = "macos") {
            commands.push(ToolchainCheck::new(
                "Apple platform SDK",
                Shell::run(
                    &["xcrun".into(), "--show-sdk-path".into()],
                    None,
                    &environment(),
                    true,
                    true,
                    StdoutTarget::StandardOutput,
                )
                .is_ok(),
                "Run xcode-select --install, complete the Apple installer, then reopen the terminal.",
            ));
        } else if cfg!(windows) {
            let mut windows = environment();
            commands.push(ToolchainCheck::new(
                "Visual Studio C++ tools and Windows SDK",
                cove_tool_support::windows_build_environment::activate(&mut windows).is_ok(),
                "Install Visual Studio Build Tools with Desktop development with C++ and a Windows SDK.",
            ));
        } else {
            commands.push(ToolchainCheck::executable("cc", common));
        }

        if arguments.platform == AppPlatform::Ios {
            for name in ["xcodebuild", "xcrun"] {
                commands.push(ToolchainCheck::executable(
                    name,
                    "Install the full Xcode IDE (not only Command Line Tools) from the App Store.",
                ));
            }
        }
        if arguments.platform == AppPlatform::Android {
            commands.extend(AndroidRequirements::inspect(arguments, root));
        }
        commands
    }
}

/// The checks behind `cove doctor`.
pub struct ToolchainSetup;

impl ToolchainSetup {
    pub fn report(arguments: &Arguments, root: &Path) -> Result<(), ToolError> {
        arguments.platform.require_available()?;
        println!(
            "Cove SDK: {}\nHost: {}; target: {}",
            root.display(),
            AppPlatform::host().raw_value(),
            arguments.platform.raw_value()
        );
        let checks = ToolchainRequirements::inspect(arguments, root);
        for check in &checks {
            println!(
                "[{}] {}",
                if check.ready { "OK" } else { "MISSING" },
                check.name
            );
        }
        let remedies: BTreeSet<&String> = checks
            .iter()
            .filter(|check| !check.ready)
            .map(|check| &check.remedy)
            .collect();
        for remedy in &remedies {
            println!("\n{remedy}");
        }
        if !remedies.is_empty() {
            return Err(ToolError::invalid(
                "Complete the target-specific steps above, then rerun cove doctor.",
            ));
        }
        Ok(())
    }
}
