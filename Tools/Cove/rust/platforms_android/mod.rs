//! Android build backend: the Swift Android SDK, NDK overlays and compilation.
//!
//! Swift sources: `Platforms/Android/Compilation/Toolchain.swift`,
//! `Platforms/Android/Compilation/NDK.swift`,
//! `Platforms/Android/Compilation/SDKOverlay.swift`,
//! `Platforms/Android/Compilation/NativeDependencies.swift`,
//! `Platforms/Android/Compilation/Options.swift`.

pub mod apk;
pub mod launch;

pub use launch::{SceneLaunch, SceneLaunchMode};

use crate::cli::{Arguments, OptionKey};
use crate::project::ApplicationIdentity;
use crate::project_sdk::NativeArtifacts;
use crate::support::{Files, Shell, StdoutTarget, ToolError, environment};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// The verified Swift Android SDK and its target triple.
#[derive(Debug, Clone)]
pub struct AndroidToolchain {
    pub root: PathBuf,
    pub host_package: PathBuf,
    pub cache: PathBuf,
    pub sdk: PathBuf,
    pub ndk: PathBuf,
}

impl AndroidToolchain {
    pub const SDK_ID: &'static str = "swift-6.4.1-RELEASE_android";
    /// The Swift Android triple; retained for the packaging report.
    pub const TRIPLE: &'static str = "aarch64-unknown-linux-android28";
    /// The Rust target Cargo builds for.
    pub const CARGO_TARGET: &'static str = "aarch64-linux-android";

    pub fn new(root: &Path, arguments: &Arguments) -> Result<Self, ToolError> {
        let host_package = arguments
            .url(OptionKey::HostPackage)
            .unwrap_or_else(|| root.join("Hosts/Android"));
        let cache = root.join("build/android-validation");
        let sdk = Self::sdk_directory(arguments);
        let ndk = Self::ndk_directory(arguments, root);
        let revision_file = ndk.join("source.properties");
        let revision = std::fs::read_to_string(&revision_file)
            .map_err(|error| ToolError::invalid(format!("{}: {error}", revision_file.display())))?;
        if !matches_pkg_revision_30(&revision) {
            return Err(ToolError::invalid(
                "Android builds require NDK r30; supply --ndk",
            ));
        }
        Ok(Self {
            root: root.to_path_buf(),
            host_package,
            cache,
            sdk,
            ndk,
        })
    }

    /// `--android-sdk`, `ANDROID_HOME`, `ANDROID_SDK_ROOT`, then the default
    /// location for this host.
    pub fn sdk_directory(arguments: &Arguments) -> PathBuf {
        if let Some(url) = arguments.url(OptionKey::AndroidSdk) {
            return url;
        }
        let environment = environment();
        if let Some(value) = environment
            .get("ANDROID_HOME")
            .or_else(|| environment.get("ANDROID_SDK_ROOT"))
        {
            return PathBuf::from(value);
        }
        let home = home_directory();
        if cfg!(target_os = "macos") {
            home.join("Library/Android/sdk")
        } else {
            home.join("Android/Sdk")
        }
    }

    /// `--ndk`, `ANDROID_NDK_HOME`, the newest installed `30.*`, then the
    /// SDK-managed fallback.
    pub fn ndk_directory(arguments: &Arguments, root: &Path) -> PathBuf {
        if let Some(url) = arguments.url(OptionKey::Ndk) {
            return url;
        }
        if let Some(value) = environment().get("ANDROID_NDK_HOME") {
            return PathBuf::from(value);
        }
        let installed =
            Files::children(&Self::sdk_directory(arguments).join("ndk")).unwrap_or_default();
        let mut candidates: Vec<PathBuf> = installed
            .into_iter()
            .filter(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().starts_with("30."))
                    .unwrap_or(false)
            })
            .collect();
        candidates.sort();
        if let Some(selected) = candidates.pop() {
            return selected;
        }
        root.join("build/android-validation/android-ndk-r30")
    }

    /// The single LLVM host directory inside the NDK.
    pub fn ndk_host(&self) -> Result<PathBuf, ToolError> {
        let hosts: Vec<PathBuf> = Files::children(&self.ndk.join("toolchains/llvm/prebuilt"))?
            .into_iter()
            .filter(|path| path.is_dir())
            .collect();
        match hosts.len() {
            1 => Ok(hosts.into_iter().next().expect("length checked")),
            _ => Err(ToolError::invalid("Expected one NDK host toolchain")),
        }
    }

    /// Builds a product for the Android triple and returns the bin path.
    pub fn compile(
        &self,
        product: &str,
        configuration: crate::cli::BuildConfiguration,
        content: Option<&Path>,
        gameplay: Option<&Path>,
        scratch: Option<&Path>,
    ) -> Result<PathBuf, ToolError> {
        Files::directory(&self.cache)?;
        let artifacts = NativeDependencies::new(self.clone()).prepare(false)?;
        let mut build_environment = environment();
        build_environment.insert("COVE_SDK_ROOT".into(), self.root.display().to_string());
        build_environment.insert("COVE_NATIVE_LINK_ORDER".into(), "1".into());
        build_environment.insert("ANDROID_NDK_HOME".into(), self.ndk.display().to_string());
        build_environment.insert("ANDROID_NDK_ROOT".into(), self.ndk.display().to_string());
        build_environment.insert(
            "COVE_RUNTIME_ARTIFACT_ROOT".into(),
            artifact_root(
                &artifacts.engine.display().to_string(),
                &self.root.display().to_string(),
            ),
        );
        build_environment.insert(
            "COVE_TEXT_ARTIFACT_ROOT".into(),
            artifact_root(
                &artifacts.text.display().to_string(),
                &self.root.display().to_string(),
            ),
        );
        if let Some(content) = content {
            build_environment.insert(
                "COVE_ANDROID_UI_PACKAGE".into(),
                content.display().to_string(),
            );
        }
        if let Some(gameplay) = gameplay {
            build_environment.insert(
                "COVE_ANDROID_GAME_PACKAGE".into(),
                gameplay.display().to_string(),
            );
        }
        // Cargo selects the NDK linker through the target-scoped variables the
        // triple maps to: `aarch64-linux-android` becomes
        // `CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER`.
        let target_key = Self::CARGO_TARGET.replace('-', "_").to_uppercase();
        let ndk_bin = self.ndk_host()?.join("bin");
        let linker = ndk_bin.join("aarch64-linux-android28-clang");
        build_environment.insert(
            format!("CARGO_TARGET_{target_key}_LINKER"),
            linker.display().to_string(),
        );
        build_environment.insert(
            "CC_aarch64_linux_android".into(),
            linker.display().to_string(),
        );
        build_environment.insert(
            "AR_aarch64_linux_android".into(),
            ndk_bin.join("llvm-ar").display().to_string(),
        );
        let mut command = crate::platforms_desktop::CargoPackage::command(
            &self.host_package,
            configuration,
            "build",
            scratch,
        );
        command.push("--target".into());
        command.push(Self::CARGO_TARGET.into());
        command.push("--bin".into());
        command.push(product.into());
        Shell::run(
            &command,
            None,
            &build_environment,
            false,
            false,
            StdoutTarget::StandardOutput,
        )?;
        let target = scratch
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.host_package.join("target"));
        Ok(target.join(Self::CARGO_TARGET).join(
            if configuration == crate::cli::BuildConfiguration::Debug {
                "debug"
            } else {
                "release"
            },
        ))
    }
}

fn home_directory() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// `Pkg.Revision\s*=\s*30\.`
fn matches_pkg_revision_30(contents: &str) -> bool {
    contents.lines().any(|line| {
        let Some(rest) = line.trim_start().strip_prefix("Pkg.Revision") else {
            return false;
        };
        let Some(rest) = rest.trim_start().strip_prefix('=') else {
            return false;
        };
        rest.trim_start().starts_with("30.")
    })
}

/// Builds the vendor archives the Android host package links against.
pub struct NativeDependencies {
    pub toolchain: AndroidToolchain,
}

/// Engine and text vendor directories.
pub struct NativeDependencyArtifacts {
    pub engine: PathBuf,
    pub text: PathBuf,
}

impl NativeDependencies {
    pub fn new(toolchain: AndroidToolchain) -> Self {
        Self { toolchain }
    }

    pub fn prepare(&self, from_source: bool) -> Result<NativeDependencyArtifacts, ToolError> {
        if !from_source
            && NativeArtifacts::install(
                &self.toolchain.root,
                "android-arm64",
                &crate::installation::Installation::directory(),
            )?
        {
            let base = NativeArtifacts::directory(&self.toolchain.root, "android-arm64");
            return Ok(NativeDependencyArtifacts {
                engine: base.join("Native/vendor"),
                text: base.join("vendor"),
            });
        }
        if !NativeArtifacts::has_source_inputs(&self.toolchain.root) {
            return Err(ToolError::invalid(
                "Missing Android native libraries; reinstall a complete SDK release. Native compilation requires a source checkout.",
            ));
        }
        let engine = self.toolchain.cache.join("native");
        let fonts = self.toolchain.cache.join("fonts-cmake");
        let parallelism = environment()
            .get("CMAKE_BUILD_PARALLEL_LEVEL")
            .cloned()
            .unwrap_or_else(|| "4".into());
        self.configure(
            &self.toolchain.root.join("Native/third-party"),
            &engine,
            &["-DCOVE_BUILD_OPENEXR=OFF"],
        )?;
        Shell::run(
            &[
                "cmake".into(),
                "--build".into(),
                engine.display().to_string(),
                "--target".into(),
                "stage_sdl3".into(),
                "stage_image_decode".into(),
                "--parallel".into(),
                parallelism.clone(),
            ],
            None,
            &environment(),
            false,
            false,
            StdoutTarget::StandardOutput,
        )?;
        Shell::run(
            &[
                "cmake".into(),
                "--install".into(),
                engine.display().to_string(),
            ],
            None,
            &environment(),
            false,
            false,
            StdoutTarget::StandardOutput,
        )?;
        self.configure(&self.toolchain.root.join("third-party"), &fonts, &[])?;
        Shell::run(
            &[
                "cmake".into(),
                "--build".into(),
                fonts.display().to_string(),
                "--target".into(),
                "stage_freetype".into(),
                "stage_harfbuzz".into(),
                "--parallel".into(),
                parallelism,
            ],
            None,
            &environment(),
            false,
            false,
            StdoutTarget::StandardOutput,
        )?;
        Shell::run(
            &[
                "cmake".into(),
                "--install".into(),
                fonts.display().to_string(),
            ],
            None,
            &environment(),
            false,
            false,
            StdoutTarget::StandardOutput,
        )?;
        Ok(NativeDependencyArtifacts {
            engine: engine.join("vendor"),
            text: fonts.join("vendor"),
        })
    }

    fn configure(
        &self,
        source: &Path,
        directory: &Path,
        options: &[&str],
    ) -> Result<(), ToolError> {
        let mut command: Vec<String> = vec![
            "cmake".into(),
            "-S".into(),
            source.display().to_string(),
            "-B".into(),
            directory.display().to_string(),
            "-G".into(),
            "Ninja".into(),
            format!(
                "-DCMAKE_TOOLCHAIN_FILE={}/build/cmake/android.toolchain.cmake",
                self.toolchain.ndk.display()
            ),
            "-DANDROID_ABI=arm64-v8a".into(),
            "-DANDROID_PLATFORM=android-28".into(),
            "-DANDROID_STL=c++_shared".into(),
            "-DCMAKE_BUILD_TYPE=Release".into(),
            "-DCMAKE_POSITION_INDEPENDENT_CODE=ON".into(),
            format!("-DCOVE_VENDOR_DIR={}/vendor", directory.display()),
            format!("-DCMAKE_INSTALL_PREFIX={}/install", directory.display()),
        ];
        command.extend(options.iter().map(|option| (*option).to_string()));
        Shell::run(
            &command,
            None,
            &environment(),
            false,
            false,
            StdoutTarget::StandardOutput,
        )?;
        Ok(())
    }
}

/// `cove build`/`cove run`/`cove showcase` options shared by APK packaging.
pub struct BuildOptions {
    pub configuration: crate::cli::BuildConfiguration,
    pub signing: Option<crate::platforms_android::apk::APKSigning>,
    pub install: bool,
    pub serial: Option<String>,
}

impl BuildOptions {
    pub fn new(arguments: &Arguments) -> Result<Self, ToolError> {
        Ok(Self {
            configuration: arguments.configuration,
            signing: crate::platforms_android::apk::APKSigning::parse(arguments)?,
            install: matches!(
                arguments.operation,
                crate::cli::Operation::Run | crate::cli::Operation::Showcase
            ),
            serial: arguments.value(OptionKey::Device).map(str::to_string),
        })
    }

    /// `--output` must name an `.apk` file.
    pub fn output(
        &self,
        arguments: &Arguments,
        default_output: &Path,
    ) -> Result<PathBuf, ToolError> {
        let output = arguments
            .url(OptionKey::Output)
            .unwrap_or_else(|| default_output.to_path_buf());
        if output
            .extension()
            .map(|extension| extension.to_string_lossy().to_lowercase())
            != Some("apk".into())
        {
            return Err(ToolError::invalid("--output must name an .apk file"));
        }
        Ok(output)
    }
}

/// The JSON record written beside a built APK.
#[derive(Serialize)]
pub struct BuildReport {
    pub application: ApplicationIdentity,
    pub source: String,
    pub apk: String,
    pub configuration: String,
    pub signing: String,
    pub target: &'static str,
    #[serde(rename = "swiftSDK")]
    pub swift_sdk: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scripts: Option<usize>,
}

/// `cove internal android-compile`.
pub fn compile(arguments: &Arguments, root: &Path) -> Result<(), ToolError> {
    #[derive(Serialize)]
    struct ResultRecord {
        products: String,
    }
    let toolchain = AndroidToolchain::new(root, arguments)?;
    let products = toolchain.compile(
        arguments.required(OptionKey::Product)?,
        arguments.configuration,
        arguments.url(OptionKey::ContentPackage).as_deref(),
        arguments.url(OptionKey::GameplayPackage).as_deref(),
        arguments.url(OptionKey::ScratchPath).as_deref(),
    )?;
    match arguments.url(OptionKey::ResultFile) {
        Some(result) => Files::json(
            &ResultRecord {
                products: products.display().to_string(),
            },
            &result,
        ),
        None => {
            println!("{}", products.display());
            Ok(())
        }
    }
}

/// `cove internal android-export`.
pub struct AndroidArtifact;

impl AndroidArtifact {
    pub fn export(arguments: &Arguments, root: &Path) -> Result<(), ToolError> {
        let toolchain = AndroidToolchain::new(root, arguments)?;
        let options = BuildOptions::new(arguments)?;
        let identity = ApplicationIdentity {
            id: arguments.required(OptionKey::Id)?.to_string(),
            name: arguments.required(OptionKey::Name)?.to_string(),
        };
        identity.validate()?;
        let output = PathBuf::from(arguments.required(OptionKey::Output)?);
        let products = toolchain.compile(
            "CoveAndroidApp",
            options.configuration,
            None,
            arguments.url(OptionKey::GameplayPackage).as_deref(),
            None,
        )?;
        apk::APKPackage::new(
            toolchain,
            identity,
            options.configuration,
            options.signing,
            None,
        )
        .build(
            &products,
            &SceneLaunch::new(
                SceneLaunchMode::Game,
                "introduction",
                Some("CoveProject".into()),
            ),
            &output,
            arguments.url(OptionKey::ProjectAssets).as_deref(),
        )
    }
}

/// SwiftPM only accepts package-relative binary target paths, so shared cache
/// directories are resolved against the package that declares the target.
fn artifact_root(path: &str, base: &str) -> String {
    let target: Vec<&str> = path.split('/').collect();
    let prefix: Vec<&str> = base.split('/').collect();
    let mut shared = 0;
    while shared < target.len() && shared < prefix.len() && target[shared] == prefix[shared] {
        shared += 1;
    }
    prefix[shared..]
        .iter()
        .map(|_| "..")
        .chain(target[shared..].iter().copied())
        .collect::<Vec<_>>()
        .join("/")
}
