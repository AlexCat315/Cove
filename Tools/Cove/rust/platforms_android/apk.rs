//! APK packaging: assets, signing, device installation and the Gradle build.
//!
//! Swift sources: `Platforms/Android/APK/Assets.swift`,
//! `Platforms/Android/APK/Device.swift`, `Platforms/Android/APK/Package.swift`,
//! `Platforms/Android/APK/Signing.swift`.

use super::AndroidToolchain;
use crate::cli::{Arguments, OptionKey};
use crate::project::{
    AppProject, ApplicationIdentity, ApplicationTemplates, ProjectConfiguration, Toolchain,
};
use crate::project_sdk::SDKRelease;
use crate::support::{environment, Files, Shell, StdoutTarget, ToolError};
use super::launch::{SceneLaunch, SceneLaunchMode};
use cove_devtools_protocol::DevToolsLaunch;
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Signing material for a release APK.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct APKSigning {
    pub keystore: PathBuf,
    pub alias: String,
    pub store_password_variable: String,
    pub key_password_variable: String,
}

impl APKSigning {
    pub fn parse(arguments: &Arguments) -> Result<Option<Self>, ToolError> {
        let Some(keystore) = arguments.url(OptionKey::Keystore) else {
            if [OptionKey::KeyAlias, OptionKey::StorePassword, OptionKey::KeyPassword]
                .iter()
                .any(|option| arguments.value(*option).is_some())
            {
                return Err(ToolError::invalid("Signing options require --keystore"));
            }
            return Ok(None);
        };
        if !keystore.exists() {
            return Err(ToolError::invalid(format!(
                "Keystore does not exist: {}",
                keystore.display()
            )));
        }
        let signing = Self {
            keystore,
            alias: arguments.required(OptionKey::KeyAlias)?.to_string(),
            store_password_variable: arguments
                .value(OptionKey::StorePassword)
                .unwrap_or("COVE_ANDROID_STORE_PASSWORD")
                .to_string(),
            key_password_variable: arguments
                .value(OptionKey::KeyPassword)
                .unwrap_or("COVE_ANDROID_KEY_PASSWORD")
                .to_string(),
        };
        for variable in [
            &signing.store_password_variable,
            &signing.key_password_variable,
        ] {
            if environment().get(variable).is_none_or(String::is_empty) {
                return Err(ToolError::invalid(format!(
                    "Set the password environment variable {variable}"
                )));
            }
        }
        Ok(Some(signing))
    }

    /// The development key generated once per build cache.
    pub fn development(toolchain: &AndroidToolchain) -> Result<Self, ToolError> {
        let key = toolchain.cache.join("debug.keystore");
        if !key.exists() {
            Files::directory(&toolchain.cache)?;
            Shell::run(
                &[
                    "keytool".into(),
                    "-genkeypair".into(),
                    "-keystore".into(),
                    key.display().to_string(),
                    "-storepass".into(),
                    "android".into(),
                    "-keypass".into(),
                    "android".into(),
                    "-alias".into(),
                    "androiddebugkey".into(),
                    "-dname".into(),
                    "CN=Cove Android,O=Cove,C=CN".into(),
                    "-keyalg".into(),
                    "RSA".into(),
                    "-keysize".into(),
                    "2048".into(),
                    "-validity".into(),
                    "3650".into(),
                ],
                None,
                &environment(),
                false,
                false,
                StdoutTarget::StandardOutput,
            )?;
        }
        Ok(Self {
            keystore: key,
            alias: "androiddebugkey".into(),
            store_password_variable: "COVE_BUILD_STORE_PASSWORD".into(),
            key_password_variable: "COVE_BUILD_KEY_PASSWORD".into(),
        })
    }
}

/// Stages libraries, fonts and resource bundles for the Gradle build.
pub struct APKAssets;

impl APKAssets {
    pub fn stage(
        products: &Path,
        toolchain: &AndroidToolchain,
        launch: &SceneLaunch,
        project_assets: Option<&Path>,
        directory: &Path,
    ) -> Result<(), ToolError> {
        let assets = directory.join("assets");
        Files::json(launch, &assets.join("launch.json"))?;
        for name in ["NotoEmoji.ttf", "OFL-Emoji.txt"] {
            Files::copy(
                &toolchain.root.join("Text/Fonts").join(name),
                &assets.join("Fonts").join(name),
            )?;
        }
        let libraries = directory.join("lib/arm64-v8a");
        Files::copy(
            &products.join("libCoveAndroidApp.so"),
            &libraries.join("libCoveAndroidApp.so"),
        )?;
        Files::copy(
            &toolchain
                .ndk_host()?
                .join("sysroot/usr/lib/aarch64-linux-android/libc++_shared.so"),
            &libraries.join("libc++_shared.so"),
        )?;
        for bundle in Files::children(products)? {
            let extension = bundle
                .extension()
                .map(|extension| extension.to_string_lossy().into_owned())
                .unwrap_or_default();
            if !["bundle", "resources"].contains(&extension.as_str()) {
                continue;
            }
            let name = bundle
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            for file in Files::files(&bundle)? {
                let relative = Files::relative(&file, &bundle);
                let parts: Vec<&str> = relative.split('/').collect();
                if parts.contains(&"Shaders") && parts.contains(&"Slang") {
                    continue;
                }
                if let Some(native) = parts.iter().position(|part| *part == "Native") {
                    if native + 1 < parts.len() {
                        let target = parts[native + 1];
                        let prefix = parts[..=native].join("/");
                        let expected = if bundle.join(&prefix).join("spirv-mobile").exists() {
                            "spirv-mobile"
                        } else {
                            "spirv"
                        };
                        if ["metal", "dxil", "spirv", "spirv-mobile"].contains(&target)
                            && target != expected
                        {
                            continue;
                        }
                    }
                }
                Files::copy(&file, &assets.join(&name).join(&relative))?;
            }
        }
        if let Some(project_assets) = project_assets {
            for file in Files::files(project_assets)? {
                let relative = Files::relative(&file, project_assets);
                if relative.starts_with(".cove/static-sources/") {
                    continue;
                }
                if file
                    .file_name()
                    .map(|name| name == "static-scripts.json")
                    .unwrap_or(false)
                {
                    continue;
                }
                Files::copy(&file, &assets.join("CoveProject").join(&relative))?;
            }
        }
        Ok(())
    }
}

/// ADB installation and launch.
pub struct AndroidDevice;

impl AndroidDevice {
    pub fn install(
        apk: &Path,
        toolchain: &AndroidToolchain,
        serial: Option<&str>,
        identity: &ApplicationIdentity,
        launch: Option<&DevToolsLaunch>,
    ) -> Result<String, ToolError> {
        let adb = toolchain.sdk.join("platform-tools/adb").display().to_string();
        let listing = Shell::run(
            &[adb.clone(), "devices".into()],
            None,
            &environment(),
            true,
            false,
            StdoutTarget::StandardOutput,
        )?;
        let ready: Vec<String> = listing
            .lines()
            .skip(1)
            .filter_map(|line| {
                let fields: Vec<&str> = line.split_whitespace().collect();
                (fields.len() >= 2 && fields[1] == "device").then(|| fields[0].to_string())
            })
            .collect();
        let device = match serial {
            Some(serial) => serial.to_string(),
            None if ready.len() == 1 => ready[0].clone(),
            None => String::new(),
        };
        if device.is_empty() || !ready.contains(&device) {
            return Err(ToolError::invalid(
                "Connect an authorized device; use --device when multiple devices are connected",
            ));
        }
        let command = vec![adb.clone(), "-s".into(), device.clone()];
        let mut install = command.clone();
        install.extend([
            "install".to_string(),
            "--no-incremental".into(),
            "-r".into(),
            apk.display().to_string(),
        ]);
        Shell::run(
            &install,
            None,
            &environment(),
            false,
            false,
            StdoutTarget::StandardOutput,
        )?;
        let mut force_stop = command.clone();
        force_stop.extend([
            "shell".to_string(),
            "am".into(),
            "force-stop".into(),
            identity.id.clone(),
        ]);
        Shell::run(
            &force_stop,
            None,
            &environment(),
            false,
            false,
            StdoutTarget::StandardOutput,
        )?;
        let mut resolve = command.clone();
        resolve.extend([
            "shell".to_string(),
            "cmd".into(),
            "package".into(),
            "resolve-activity".into(),
            "--brief".into(),
            "-a".into(),
            "android.intent.action.MAIN".into(),
            "-c".into(),
            "android.intent.category.LAUNCHER".into(),
            identity.id.clone(),
        ]);
        let resolved = Shell::run(
            &resolve,
            None,
            &environment(),
            true,
            true,
            StdoutTarget::StandardOutput,
        )?;
        let activity = Self::launcher(&resolved, &identity.id)?;
        let mut start = command.clone();
        start.extend([
            "shell".to_string(),
            "am".into(),
            "start".into(),
            "-W".into(),
            "-n".into(),
            activity,
            "--es".into(),
            "cove_logging".into(),
            logging_payload(identity)?,
        ]);
        if let Some(launch) = launch {
            start.push("--es".into());
            start.push("cove_devtools".into());
            start.push(base64(&serde_json::to_vec(launch).map_err(|error| {
                ToolError::invalid(error.to_string())
            })?));
        }
        Shell::run(
            &start,
            None,
            &environment(),
            false,
            true,
            StdoutTarget::StandardOutput,
        )?;
        Ok(device)
    }

    pub fn launcher(listing: &str, application_id: &str) -> Result<String, ToolError> {
        let prefix = format!("{application_id}/");
        listing
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with(&prefix) && !line.contains(' '))
            .next_back()
            .map(str::to_string)
            .ok_or_else(|| {
                ToolError::invalid(format!(
                    "No launchable activity for {application_id}; check Platforms/Android/app/src/main/AndroidManifest.xml"
                ))
            })
    }
}

/// The launch arguments carry the application id and no storage directory.
fn logging_payload(identity: &ApplicationIdentity) -> Result<String, ToolError> {
    #[derive(serde::Serialize)]
    struct Configuration {
        storage: Storage,
    }
    #[derive(serde::Serialize)]
    struct Storage {
        directory: Option<String>,
    }
    #[derive(serde::Serialize)]
    struct Logging {
        application: String,
        configuration: Configuration,
    }
    let payload = Logging {
        application: identity.id.clone(),
        configuration: Configuration {
            storage: Storage { directory: None },
        },
    };
    let bytes =
        serde_json::to_vec(&payload).map_err(|error| ToolError::invalid(error.to_string()))?;
    Ok(base64(&bytes))
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        result.push(ALPHABET[(value >> 18) as usize & 0x3f] as char);
        result.push(ALPHABET[(value >> 12) as usize & 0x3f] as char);
        result.push(if chunk.len() > 1 {
            ALPHABET[(value >> 6) as usize & 0x3f] as char
        } else {
            '='
        });
        result.push(if chunk.len() > 2 {
            ALPHABET[value as usize & 0x3f] as char
        } else {
            '='
        });
    }
    result
}

/// Swift prepares its libraries/resources. Gradle owns the native build.
pub struct APKPackage {
    pub toolchain: AndroidToolchain,
    pub identity: ApplicationIdentity,
    pub mode: crate::cli::BuildConfiguration,
    pub signing: Option<APKSigning>,
    pub application: Option<AppProject>,
}

impl APKPackage {
    pub fn new(
        toolchain: AndroidToolchain,
        identity: ApplicationIdentity,
        mode: crate::cli::BuildConfiguration,
        signing: Option<APKSigning>,
        application: Option<AppProject>,
    ) -> Self {
        Self {
            toolchain,
            identity,
            mode,
            signing,
            application,
        }
    }

    pub fn build(
        &self,
        products: &Path,
        launch: &SceneLaunch,
        output: &Path,
        game_assets: Option<&Path>,
    ) -> Result<(), ToolError> {
        let application = self.packaging_project()?;
        self.stage(products, launch, &application, game_assets)?;
        let platform = application.directory.join("Platforms/Android");
        if !platform.join("gradlew").exists() {
            return Err(ToolError::invalid(
                "Missing Platforms/Android/gradlew; this application requires a native Android project",
            ));
        }
        let credentials = match &self.signing {
            Some(signing) => signing.clone(),
            None => APKSigning::development(&self.toolchain)?,
        };
        let mut build_environment = environment();
        build_environment.insert(
            "COVE_SDK_ROOT".into(),
            self.toolchain.root.display().to_string(),
        );
        build_environment.insert(
            "ANDROID_HOME".into(),
            self.toolchain.sdk.display().to_string(),
        );
        if self.signing.is_none() {
            build_environment.insert(credentials.store_password_variable.clone(), "android".into());
            build_environment.insert(credentials.key_password_variable.clone(), "android".into());
        }
        let task = format!("assemble{}", capitalize(self.mode.raw_value()));
        let mut command: Vec<String> = vec![
            "/bin/sh".into(),
            platform.join("gradlew").display().to_string(),
            "-p".into(),
            platform.display().to_string(),
            "--project-cache-dir".into(),
            application
                .build_directory()
                .join("android/gradle-cache")
                .display()
                .to_string(),
            "--no-daemon".into(),
            "--console=plain".into(),
            "-PcovePrepared=true".into(),
            format!("-PcoveKeystore={}", credentials.keystore.display()),
            format!("-PcoveKeyAlias={}", credentials.alias),
            format!(
                "-PcoveStorePasswordEnv={}",
                credentials.store_password_variable
            ),
            format!(
                "-PcoveKeyPasswordEnv={}",
                credentials.key_password_variable
            ),
            task,
        ];
        if cfg!(windows) {
            command[0] = platform.join("gradlew.bat").display().to_string();
        }
        Shell::run(
            &command,
            None,
            &build_environment,
            false,
            false,
            StdoutTarget::StandardOutput,
        )?;
        let folder = application
            .build_directory()
            .join("android/gradle/app/outputs/apk")
            .join(self.mode.raw_value());
        let metadata_file = folder.join("output-metadata.json");
        let data = std::fs::read(&metadata_file)
            .map_err(|error| ToolError::invalid(format!("{}: {error}", metadata_file.display())))?;
        let metadata: GradleOutput = serde_json::from_slice(&data)
            .map_err(|error| ToolError::invalid(format!("{}: {error}", metadata_file.display())))?;
        let Some(file_name) = metadata
            .elements
            .as_slice()
            .first()
            .filter(|_| metadata.elements.len() == 1)
            .map(|element| element.output_file.clone())
        else {
            return Err(ToolError::invalid(
                "Expected a single ARM64 APK in Gradle output metadata",
            ));
        };
        if Path::new(&file_name)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .as_deref()
            != Some(file_name.as_str())
        {
            return Err(ToolError::invalid(
                "Expected a single ARM64 APK in Gradle output metadata",
            ));
        }
        Files::copy(&folder.join(&file_name), output)?;
        let verifier = self.toolchain.sdk.join("build-tools/36.1.0/apksigner");
        Shell::run(
            &[verifier.display().to_string(), "verify".into(), output.display().to_string()],
            None,
            &environment(),
            false,
            false,
            StdoutTarget::StandardOutput,
        )?;
        Shell::run(
            &[
                self.toolchain
                    .sdk
                    .join("build-tools/36.1.0/zipalign")
                    .display()
                    .to_string(),
                "-c".into(),
                "-P".into(),
                "16".into(),
                "4".into(),
                output.display().to_string(),
            ],
            None,
            &environment(),
            false,
            false,
            StdoutTarget::StandardOutput,
        )?;
        println!(
            "Built signed APK: {} (configuration={})",
            output.display(),
            self.mode.raw_value()
        );
        Ok(())
    }

    pub fn stage(
        &self,
        products: &Path,
        launch: &SceneLaunch,
        application: &AppProject,
        game_assets: Option<&Path>,
    ) -> Result<(), ToolError> {
        let directory = application
            .build_directory()
            .join("android/generated")
            .join(self.mode.raw_value());
        Files::reset(&directory)?;
        APKAssets::stage(
            products,
            &self.toolchain,
            launch,
            game_assets,
            &directory,
        )
    }

    /// Generated packaging workspaces are owned by the SDK, not by a user.
    fn packaging_project(&self) -> Result<AppProject, ToolError> {
        if let Some(application) = &self.application {
            return Ok(application.clone());
        }
        let directory = self
            .toolchain
            .cache
            .join(format!("packaging-{}", self.identity.id));
        let configuration = ProjectConfiguration::new(
            self.identity.clone(),
            Toolchain {
                sdk_version: SDKRelease::read(&self.toolchain.root)?.version,
            },
        );
        ApplicationTemplates::install_android(&self.toolchain.root, &directory, &configuration)?;
        AppProject::new(&directory)
    }
}

#[derive(Deserialize)]
struct GradleOutput {
    #[serde(default)]
    elements: Vec<GradleElement>,
}

#[derive(Deserialize)]
struct GradleElement {
    #[serde(rename = "outputFile")]
    output_file: String,
}

fn capitalize(value: &str) -> String {
    let mut characters = value.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
        None => String::new(),
    }
}

/// `cove build`/`cove run` for an Android application project.
pub struct BuildAndroidApplication;

impl BuildAndroidApplication {
    pub fn prepare(arguments: &Arguments, root: &Path, project: &AppProject) -> Result<(), ToolError> {
        project.require(crate::cli::AppPlatform::Android)?;
        let toolchain = AndroidToolchain::new(root, arguments)?;
        let products = toolchain.compile(
            "CoveAndroidApp",
            arguments.configuration,
            Some(&project.directory),
            None,
            Some(&project.build_directory().join("android/swift")),
        )?;
        APKPackage::new(
            toolchain,
            project.configuration.application.clone(),
            arguments.configuration,
            None,
            Some(project.clone()),
        )
        .stage(
            &products,
            &SceneLaunch::new(SceneLaunchMode::Application, "introduction", None),
            project,
            None,
        )
    }

    pub fn run(
        arguments: &Arguments,
        root: &Path,
        project: Option<&AppProject>,
    ) -> Result<(), ToolError> {
        let options = super::BuildOptions::new(arguments)?;
        let showcase = arguments
            .subject
            .clone()
            .unwrap_or_else(|| "gallery".into());
        if project.is_none() && !["gallery", "counter"].contains(&showcase.as_str()) {
            return Err(ToolError::invalid("Showcase must be gallery or counter"));
        }
        let identity = match project {
            Some(project) => project.configuration.application.clone(),
            None => ApplicationIdentity {
                id: "com.cove.probe".into(),
                name: format!("CoveUI {}", capitalize(&showcase)),
            },
        };
        let toolchain = AndroidToolchain::new(root, arguments)?;
        let directory = match project {
            Some(project) => project
                .build_directory()
                .join("android")
                .join(options.configuration.raw_value()),
            None => toolchain.cache.clone(),
        };
        let output = options.output(arguments, &directory.join("Application.apk"))?;
        let products = toolchain.compile(
            "CoveAndroidApp",
            options.configuration,
            project.map(|project| project.directory.as_path()),
            None,
            project
                .map(|project| project.build_directory().join("android/swift"))
                .as_deref(),
        )?;
        let launch = SceneLaunch::new(
            match project {
                Some(_) => SceneLaunchMode::Application,
                None => SceneLaunchMode::Application,
            },
            arguments
                .value(OptionKey::Component)
                .unwrap_or("introduction"),
            None,
        );
        APKPackage::new(
            toolchain.clone(),
            identity.clone(),
            options.configuration,
            options.signing.clone(),
            project.cloned(),
        )
        .build(&products, &launch, &output, None)?;
        let report = super::BuildReport {
            application: identity.clone(),
            source: project
                .map(|project| project.directory.display().to_string())
                .unwrap_or_else(|| showcase.clone()),
            apk: output.display().to_string(),
            configuration: options.configuration.raw_value().into(),
            signing: if options.signing.is_none() {
                "development".into()
            } else {
                "custom".into()
            },
            target: AndroidToolchain::TRIPLE,
            swift_sdk: AndroidToolchain::SDK_ID,
            scripts: None,
        };
        Files::json(&report, &output.with_extension("json"))?;
        println!("Built Android application: {}", output.display());
        if options.install {
            match project {
                Some(project) => crate::development::AndroidRun::run(&output, project, &toolchain, arguments)?,
                None => {
                    AndroidDevice::install(
                        &output,
                        &toolchain,
                        options.serial.as_deref(),
                        &identity,
                        None,
                    )?;
                }
            }
        }
        Ok(())
    }
}

/// `cove showcase` currently targets Android only.
pub fn showcase(arguments: &Arguments, root: &Path) -> Result<(), ToolError> {
    if arguments.platform != crate::cli::AppPlatform::Android {
        return Err(ToolError::invalid(
            "Use cove run <SDK>/Examples for the desktop gallery; showcase currently targets --android",
        ));
    }
    BuildAndroidApplication::run(arguments, root, None)
}
