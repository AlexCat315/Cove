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
use crate::project_sdk::{CoveSDK, NativeArtifacts};
use crate::support::{environment, Files, Shell, StdoutTarget, ToolError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The verified Swift Android SDK and its target triple.
#[derive(Debug, Clone)]
pub struct AndroidToolchain {
    pub root: PathBuf,
    pub host_package: PathBuf,
    pub cache: PathBuf,
    pub sdk: PathBuf,
    pub ndk: PathBuf,
    pub swift: String,
}

impl AndroidToolchain {
    pub const SDK_ID: &'static str = "swift-6.4.1-RELEASE_android";
    pub const TRIPLE: &'static str = "aarch64-unknown-linux-android28";

    pub fn new(root: &Path, arguments: &Arguments) -> Result<Self, ToolError> {
        let host_package = arguments
            .url(OptionKey::HostPackage)
            .unwrap_or_else(|| root.join("Hosts/Android"));
        let cache = root.join("build/android-validation");
        let sdk = Self::sdk_directory(arguments);
        let ndk = Self::ndk_directory(arguments, root);
        let swift = environment()
            .get("COVE_SWIFT_EXECUTABLE")
            .cloned()
            .unwrap_or_else(|| "swift".into());
        let revision_file = ndk.join("source.properties");
        let revision = std::fs::read_to_string(&revision_file).map_err(|error| {
            ToolError::invalid(format!("{}: {error}", revision_file.display()))
        })?;
        if !matches_pkg_revision_30(&revision) {
            return Err(ToolError::invalid(
                "Swift 6.4 Android SDK requires NDK r30; supply --ndk",
            ));
        }
        let version = Shell::run(
            &[swift.clone(), "--version".into()],
            None,
            &environment(),
            true,
            false,
            StdoutTarget::StandardOutput,
        )?;
        if !version.contains("swift-6.4.1-RELEASE") {
            return Err(ToolError::invalid(
                "Select the Swift 6.4.1 release toolchain matching the Android SDK",
            ));
        }
        Ok(Self {
            root: root.to_path_buf(),
            host_package,
            cache,
            sdk,
            ndk,
            swift,
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
        let installed = Files::children(&Self::sdk_directory(arguments).join("ndk"))
            .unwrap_or_default();
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
        let hosts: Vec<PathBuf> =
            Files::children(&self.ndk.join("toolchains/llvm/prebuilt"))?
                .into_iter()
                .filter(|path| path.is_dir())
                .collect();
        match hosts.len() {
            1 => Ok(hosts.into_iter().next().expect("length checked")),
            _ => Err(ToolError::invalid("Expected one NDK host toolchain")),
        }
    }

    /// Builds a product with the Swift Android SDK and returns the bin path.
    pub fn compile(
        &self,
        product: &str,
        configuration: crate::cli::BuildConfiguration,
        content: Option<&Path>,
        gameplay: Option<&Path>,
        scratch: Option<&Path>,
    ) -> Result<PathBuf, ToolError> {
        Self::ensure_swift_sdk()?;
        Files::directory(&self.cache)?;
        let artifacts = NativeDependencies::new(self.clone()).prepare(false)?;
        let mut build_environment = environment();
        build_environment.insert("COVE_SDK_ROOT".into(), self.root.display().to_string());
        build_environment.insert("COVE_NATIVE_LINK_ORDER".into(), "1".into());
        build_environment.insert("ANDROID_NDK_HOME".into(), self.ndk.display().to_string());
        // SwiftPM resolves binary target paths against the consuming package
        // root, so shared cache locations are passed relative to that package.
        build_environment.insert(
            "COVE_RUNTIME_ARTIFACT_ROOT".into(),
            artifact_root(&artifacts.engine.display().to_string(), &self.root.display().to_string()),
        );
        build_environment.insert(
            "COVE_TEXT_ARTIFACT_ROOT".into(),
            artifact_root(&artifacts.text.display().to_string(), &self.root.display().to_string()),
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
        let mut command: Vec<String> = vec![
            self.swift.clone(),
            "build".into(),
            "--package-path".into(),
            self.host_package.display().to_string(),
            "--build-system".into(),
            "native".into(),
            "--swift-sdk".into(),
            Self::SDK_ID.into(),
            "--triple".into(),
            Self::TRIPLE.into(),
            "--static-swift-stdlib".into(),
            "--configuration".into(),
            configuration.swift_configuration().into(),
            "--jobs".into(),
            "4".into(),
        ];
        command.extend(self.compiler_options()?);
        command.extend(configuration.swift_flags());
        if let Some(scratch) = scratch {
            command.push("--scratch-path".into());
            command.push(scratch.display().to_string());
        }
        let mut build = command.clone();
        build.push("--product".into());
        build.push(product.into());
        Shell::run(
            &build,
            None,
            &build_environment,
            false,
            false,
            StdoutTarget::StandardOutput,
        )?;
        let mut paths = command;
        paths.push("--show-bin-path".into());
        let output = Shell::run(
            &paths,
            None,
            &build_environment,
            true,
            false,
            StdoutTarget::StandardOutput,
        )?;
        Ok(PathBuf::from(output.trim()))
    }

    /// Installs the pinned Swift Android SDK when it is missing.
    pub fn ensure_swift_sdk() -> Result<(), ToolError> {
        let installed = Shell::run(
            &[CoveSDK::swift(), "sdk".into(), "list".into()],
            None,
            &environment(),
            true,
            true,
            StdoutTarget::StandardOutput,
        )?;
        if installed.contains(Self::SDK_ID) {
            return Ok(());
        }
        Shell::run(
            &[
                CoveSDK::swift(),
                "sdk".into(),
                "install".into(),
                "https://download.swift.org/swift-6.4.1-release/android-sdk/swift-6.4.1-RELEASE/swift-6.4.1-RELEASE_android.artifactbundle.tar.gz".into(),
                "--checksum".into(),
                "a356b79f8f1daa6ae3a66f4c556966ca20c0208b3659805cf30a83a0a31d6772".into(),
            ],
            None,
            &environment(),
            false,
            false,
            StdoutTarget::StandardOutput,
        )?;
        Ok(())
    }

    /// SwiftPM toolset overrides binding the installed SDK to the selected NDK.
    pub fn compiler_options(&self) -> Result<Vec<String>, ToolError> {
        let host = self.ndk_host()?;
        let resources: Vec<PathBuf> =
            Files::children(&host.join("lib/clang"))?
                .into_iter()
                .filter(|path| path.join("include/stddef.h").is_file())
                .collect();
        let [resource] = resources.as_slice() else {
            return Err(ToolError::invalid(
                "Expected one NDK Clang resource directory",
            ));
        };
        let bin = host.join("bin");
        let resource_path = resource.display().to_string();
        let toolset = Toolset {
            schema_version: "1.0",
            c_compiler: Tool {
                path: Some(bin.join("clang").display().to_string()),
                extra_cli_options: Vec::new(),
            },
            cxx_compiler: Tool {
                path: Some(bin.join("clang++").display().to_string()),
                extra_cli_options: Vec::new(),
            },
            swift_compiler: Tool {
                path: None,
                extra_cli_options: vec![
                    "-Xcc".into(),
                    format!("-resource-dir={resource_path}"),
                    "-tools-directory".into(),
                    bin.display().to_string(),
                    "-Xclang-linker".into(),
                    format!("-resource-dir={resource_path}"),
                    "-L".into(),
                    resource.join("lib/linux/aarch64").display().to_string(),
                ],
            },
        };
        let overrides = serde_json::to_vec(&toolset)
            .map_err(|error| ToolError::invalid(error.to_string()))?;
        let directory = AndroidSDKOverlay::new(self.clone(), host).prepare(&overrides)?;
        Ok(vec![
            "--swift-sdks-path".into(),
            directory.display().to_string(),
        ])
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

#[derive(Serialize)]
struct Toolset {
    #[serde(rename = "schemaVersion")]
    schema_version: &'static str,
    #[serde(rename = "cCompiler")]
    c_compiler: Tool,
    #[serde(rename = "cxxCompiler")]
    cxx_compiler: Tool,
    #[serde(rename = "swiftCompiler")]
    swift_compiler: Tool,
}

#[derive(Serialize, Deserialize)]
struct Tool {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(rename = "extraCLIOptions", default)]
    extra_cli_options: Vec<String>,
}

/// A managed SDK view binds the installed Swift libraries to the selected NDK.
///
/// This never edits the installed SDK or the NDK, and different NDKs use
/// separate caches.
pub struct AndroidSDKOverlay {
    pub toolchain: AndroidToolchain,
    pub host: PathBuf,
}

impl AndroidSDKOverlay {
    pub fn new(toolchain: AndroidToolchain, host: PathBuf) -> Self {
        Self { toolchain, host }
    }

    pub fn prepare(&self, overrides: &[u8]) -> Result<PathBuf, ToolError> {
        let output = Shell::run(
            &[
                self.toolchain.swift.clone(),
                "sdk".into(),
                "configure".into(),
                "--show-configuration".into(),
                AndroidToolchain::SDK_ID.into(),
                AndroidToolchain::TRIPLE.into(),
            ],
            None,
            &environment(),
            true,
            true,
            StdoutTarget::StandardOutput,
        )?;
        let entries: BTreeMap<String, String> = output
            .lines()
            .filter_map(|line| {
                let (key, value) = line.split_once(':')?;
                Some((key.to_string(), value.trim().to_string()))
            })
            .collect();
        let (Some(dynamic), Some(static_libraries)) = (
            entries.get("swiftResourcesPath"),
            entries.get("swiftStaticResourcesPath"),
        ) else {
            return Err(ToolError::invalid(
                "The selected Swift Android SDK does not provide its library paths",
            ));
        };
        let mut source = PathBuf::from(dynamic);
        while !source.join("swift-sdk.json").is_file() {
            let Some(parent) = source.parent() else {
                return Err(ToolError::invalid(
                    "Cannot locate the installed Swift SDK metadata",
                ));
            };
            if parent == source {
                return Err(ToolError::invalid(
                    "Cannot locate the installed Swift SDK metadata",
                ));
            }
            source = parent.to_path_buf();
        }
        let key = fnv1a(&format!(
            "{}{}",
            self.toolchain.ndk.display(),
            source.display()
        ));
        let directory = self
            .toolchain
            .cache
            .join("swift-sdks")
            .join(format!("{key:x}"));
        let bundle = directory.join(
            source
                .parent()
                .and_then(Path::file_name)
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
        );
        let sdk = bundle.join(
            source
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
        );
        Files::directory(&sdk)?;
        let source_parent = source.parent().unwrap_or(Path::new("."));
        Files::copy(&source_parent.join("info.json"), &bundle.join("info.json"))?;
        Files::copy(&source.join("swift-sdk.json"), &sdk.join("swift-sdk.json"))?;
        for item in Files::children(&source)? {
            let name = item
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            if ["ndk-sysroot", "swift-sdk.json", "swift-toolset.json"].contains(&name.as_str()) {
                continue;
            }
            link(&item, &sdk.join(&name))?;
        }

        let toolset_file = source.join("swift-toolset.json");
        let data = std::fs::read(&toolset_file)
            .map_err(|error| ToolError::invalid(format!("{}: {error}", toolset_file.display())))?;
        let mut original: serde_json::Map<String, serde_json::Value> =
            serde_json::from_slice(&data)
                .map_err(|error| ToolError::invalid(format!("{}: {error}", toolset_file.display())))?;
        let replacements: serde_json::Map<String, serde_json::Value> =
            serde_json::from_slice(overrides)
                .map_err(|error| ToolError::invalid(error.to_string()))?;
        for (name, replacement) in replacements {
            if name == "schemaVersion" {
                continue;
            }
            let Some(replacement) = replacement.as_object() else {
                continue;
            };
            let mut tool = original
                .get(&name)
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            if let Some(path) = replacement.get("path") {
                tool.insert("path".into(), path.clone());
            }
            let mut options = tool
                .get("extraCLIOptions")
                .and_then(|value| value.as_array())
                .cloned()
                .unwrap_or_default();
            if let Some(extra) = replacement
                .get("extraCLIOptions")
                .and_then(|value| value.as_array())
            {
                options.extend(extra.iter().cloned());
            }
            tool.insert("extraCLIOptions".into(), serde_json::Value::Array(options));
            original.insert(name, serde_json::Value::Object(tool));
        }
        let mut compiler = original
            .get("swiftCompiler")
            .and_then(|value| value.as_object())
            .cloned()
            .unwrap_or_default();
        let mut options = compiler
            .get("extraCLIOptions")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();
        options.extend([
            serde_json::Value::String("-L".into()),
            serde_json::Value::String(
                Path::new(static_libraries)
                    .join("android")
                    .display()
                    .to_string(),
            ),
            serde_json::Value::String("-resource-dir".into()),
            serde_json::Value::String(static_libraries.clone()),
            serde_json::Value::String("-static-stdlib".into()),
        ]);
        compiler.insert("extraCLIOptions".into(), serde_json::Value::Array(options));
        original.insert(
            "swiftCompiler".into(),
            serde_json::Value::Object(compiler),
        );
        // `.sortedKeys` serialisation for a reproducible toolset file.
        Files::json(
            &serde_json::Value::Object(original),
            &sdk.join("swift-toolset.json"),
        )?;

        let sysroot = sdk.join("ndk-sysroot");
        Files::directory(&sysroot.join("usr/lib"))?;
        link(
            &self.host.join("sysroot/usr/include"),
            &sysroot.join("usr/include"),
        )?;
        for item in Files::children(&self.host.join("sysroot/usr/lib"))? {
            let name = item
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            link(&item, &sysroot.join("usr/lib").join(name))?;
        }
        for (folder, resources) in [("swift", dynamic), ("swift_static", static_libraries)] {
            let destination = sysroot
                .join("usr/lib")
                .join(folder)
                .join("android/aarch64/swiftrt.o");
            if let Some(parent) = destination.parent() {
                Files::directory(parent)?;
            }
            link(
                &Path::new(resources).join("android/aarch64/swiftrt.o"),
                &destination,
            )?;
        }
        Ok(directory)
    }
}

fn link(source: &Path, destination: &Path) -> Result<(), ToolError> {
    if std::fs::read_link(destination).ok().as_deref() == Some(source) {
        return Ok(());
    }
    Files::remove(destination)?;
    symbolic_link(source, destination)
}

#[cfg(unix)]
fn symbolic_link(source: &Path, destination: &Path) -> Result<(), ToolError> {
    std::os::unix::fs::symlink(source, destination)
        .map_err(|error| ToolError::invalid(error.to_string()))
}

#[cfg(windows)]
fn symbolic_link(source: &Path, destination: &Path) -> Result<(), ToolError> {
    if source.is_dir() {
        std::os::windows::fs::symlink_dir(source, destination)
    } else {
        std::os::windows::fs::symlink_file(source, destination)
    }
    .map_err(|error| ToolError::invalid(error.to_string()))
}

/// FNV-1a, matching the Swift overlay cache key.
fn fnv1a(value: &str) -> u64 {
    value.bytes().fold(14695981039346656037u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(1099511628211)
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
    pub fn output(&self, arguments: &Arguments, default_output: &Path) -> Result<PathBuf, ToolError> {
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
        apk::APKPackage::new(toolchain, identity, options.configuration, options.signing, None)
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
