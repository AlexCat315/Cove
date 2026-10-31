//! Command grammar shared by every platform backend.
//!
//! Swift sources: `CLI/Arguments.swift`, `CLI/Platform.swift`,
//! `CLI/Diagnostics.swift`.
//!
//! Platform selection, build configuration and the accepted flag/option matrix
//! match the original one-for-one, including every diagnostic message and the
//! order in which conflicting selections are rejected.

use crate::support::ToolError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Target platform of a build, run or package operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppPlatform {
    Mac,
    Win,
    Linux,
    Android,
    Ios,
    Web,
}

impl AppPlatform {
    /// Every platform spelling accepted on the command line.
    #[allow(dead_code)]
    pub const ALL: [Self; 6] = [
        Self::Mac,
        Self::Win,
        Self::Linux,
        Self::Android,
        Self::Ios,
        Self::Web,
    ];

    pub fn raw_value(self) -> &'static str {
        match self {
            Self::Mac => "mac",
            Self::Win => "win",
            Self::Linux => "linux",
            Self::Android => "android",
            Self::Ios => "ios",
            Self::Web => "web",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "mac" => Some(Self::Mac),
            "win" => Some(Self::Win),
            "linux" => Some(Self::Linux),
            "android" => Some(Self::Android),
            "ios" => Some(Self::Ios),
            "web" => Some(Self::Web),
            _ => None,
        }
    }

    /// The platform of the machine running the tool.
    pub fn host() -> Self {
        if cfg!(target_os = "macos") {
            Self::Mac
        } else if cfg!(windows) {
            Self::Win
        } else {
            Self::Linux
        }
    }

    /// Rejects platforms this host cannot produce, using the original wording.
    pub fn require_available(self) -> Result<(), ToolError> {
        match self {
            Self::Ios => {
                #[cfg(not(target_os = "macos"))]
                {
                    return Err(ToolError::invalid("iOS builds require macOS with Xcode"));
                }
                Ok(())
            }
            Self::Web => Err(ToolError::invalid(
                "Web currently has a portable rendering prototype, not the complete CoveUI application host. See CoveUI/Browser.",
            )),
            Self::Mac | Self::Win | Self::Linux => {
                if self != Self::host() {
                    return Err(ToolError::invalid(format!(
                        "Build and run {} applications on a {} host; desktop cross-compilation is not configured.",
                        self.raw_value(),
                        self.raw_value()
                    )));
                }
                Ok(())
            }
            Self::Android => {
                if cfg!(windows) {
                    return Err(ToolError::invalid(
                        "The Android build backend currently supports macOS and Linux hosts.",
                    ));
                }
                Ok(())
            }
        }
    }

    /// Folder name used by native platform projects.
    pub fn directory_name(self) -> &'static str {
        match self {
            Self::Mac => "macOS",
            Self::Win => "Windows",
            Self::Linux => "Linux",
            Self::Android => "Android",
            Self::Ios => "iOS",
            Self::Web => "Web",
        }
    }
}

/// Build configuration; `profile` keeps release optimisation with diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BuildConfiguration {
    Debug,
    Profile,
    Release,
}

impl BuildConfiguration {
    pub fn raw_value(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Profile => "profile",
            Self::Release => "release",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "debug" => Some(Self::Debug),
            "profile" => Some(Self::Profile),
            "release" => Some(Self::Release),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Operation {
    AndroidExport,
    InstallCli,
    PackageCli,
    PackageNative,
    PackageBuild,
    Create,
    Build,
    Run,
    Clean,
    Test,
    Fmt,
    Clippy,
    Check,
    Migration,
    Task,
    Doctor,
    Devices,
    Showcase,
    AndroidCompile,
    AndroidPrepare,
    Devtools,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OptionKey {
    HostPackage,
    ProjectAssets,
    BinDirectory,
    Simulator,
    Team,
    SdkRoot,
    Id,
    Name,
    Output,
    Device,
    Filter,
    Platforms,
    Session,
    AndroidSdk,
    Ndk,
    Keystore,
    KeyAlias,
    StorePassword,
    KeyPassword,
    Component,
    Product,
    ContentPackage,
    GameplayPackage,
    ScratchPath,
    ResultFile,
    TestingLibrary,
    Timeout,
    EventOutput,
    Triple,
    Sdk,
    Target,
    Architecture,
    Package,
}

impl OptionKey {
    pub const ALL: [Self; 33] = [
        Self::HostPackage,
        Self::ProjectAssets,
        Self::BinDirectory,
        Self::Simulator,
        Self::Team,
        Self::SdkRoot,
        Self::Id,
        Self::Name,
        Self::Output,
        Self::Device,
        Self::Filter,
        Self::Platforms,
        Self::Session,
        Self::AndroidSdk,
        Self::Ndk,
        Self::Keystore,
        Self::KeyAlias,
        Self::StorePassword,
        Self::KeyPassword,
        Self::Component,
        Self::Product,
        Self::ContentPackage,
        Self::GameplayPackage,
        Self::ScratchPath,
        Self::ResultFile,
        Self::TestingLibrary,
        Self::Timeout,
        Self::EventOutput,
        Self::Triple,
        Self::Sdk,
        Self::Target,
        Self::Architecture,
        Self::Package,
    ];

    pub fn raw_value(self) -> &'static str {
        match self {
            Self::HostPackage => "host-package",
            Self::ProjectAssets => "project-assets",
            Self::BinDirectory => "bin-directory",
            Self::Simulator => "simulator",
            Self::Team => "team",
            Self::SdkRoot => "sdk-root",
            Self::Id => "id",
            Self::Name => "name",
            Self::Output => "output",
            Self::Device => "device",
            Self::Filter => "filter",
            Self::Platforms => "platforms",
            Self::Session => "session",
            Self::AndroidSdk => "android-sdk",
            Self::Ndk => "ndk",
            Self::Keystore => "keystore",
            Self::KeyAlias => "key-alias",
            Self::StorePassword => "store-password-env",
            Self::KeyPassword => "key-password-env",
            Self::Component => "component",
            Self::Product => "product",
            Self::ContentPackage => "content-package",
            Self::GameplayPackage => "gameplay-package",
            Self::ScratchPath => "scratch-path",
            Self::ResultFile => "result-file",
            Self::TestingLibrary => "testing-library",
            Self::Timeout => "timeout",
            Self::EventOutput => "event-output",
            Self::Triple => "triple",
            Self::Sdk => "sdk",
            Self::Target => "target",
            Self::Architecture => "arch",
            Self::Package => "package",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.raw_value() == value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Flag {
    Devtools,
    NoOpen,
    NoParallel,
    SkipBuild,
    BuildOnly,
    BuildTests,
    Fix,
    AllTargets,
    Locked,
    Offline,
    Json,
    RequireComplete,
}

impl Flag {
    pub fn raw_value(self) -> &'static str {
        match self {
            Self::Devtools => "devtools",
            Self::NoOpen => "no-open",
            Self::NoParallel => "no-parallel",
            Self::SkipBuild => "skip-build",
            Self::BuildOnly => "build-only",
            Self::BuildTests => "build-tests",
            Self::Fix => "fix",
            Self::AllTargets => "all-targets",
            Self::Locked => "locked",
            Self::Offline => "offline",
            Self::Json => "json",
            Self::RequireComplete => "require-complete",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        [
            Self::Devtools,
            Self::NoOpen,
            Self::NoParallel,
            Self::SkipBuild,
            Self::BuildOnly,
            Self::BuildTests,
            Self::Fix,
            Self::AllTargets,
            Self::Locked,
            Self::Offline,
            Self::Json,
            Self::RequireComplete,
        ]
        .into_iter()
        .find(|candidate| candidate.raw_value() == value)
    }

    /// The Cargo flag this passes through, when it has one.
    pub fn cargo_argument(self) -> Option<&'static str> {
        match self {
            Self::AllTargets => Some("--all-targets"),
            Self::Locked => Some("--locked"),
            Self::Offline => Some("--offline"),
            _ => None,
        }
    }
}

/// One AI/automation client request against the project's live DevTools
/// session. The session file is resolved by the CLI, so callers never copy
/// endpoints or tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevToolsClient {
    /// The repository tool operation (`cove-dev devtools <operation>`).
    pub operation: String,
    pub arguments: Vec<String>,
}

impl DevToolsClient {
    /// Returns `None` for a word that is not an action (it is then the project
    /// directory); returns an error for an action with the wrong arguments.
    pub fn parse(action: &str, arguments: &[String]) -> Result<Option<Self>, ToolError> {
        let build = |operation: &str, arguments: Vec<String>| {
            Ok(Some(Self {
                operation: operation.into(),
                arguments,
            }))
        };
        match action {
            "list" => {
                if !arguments.is_empty() {
                    return Err(ToolError::invalid("Use cove devtools list"));
                }
                build("query-ui", vec!["--list".into()])
            }
            "query" => {
                // A bare JSON object queries nodes, the most common AI request.
                if arguments.is_empty() {
                    build("query-ui", vec!["--list".into()])
                } else if arguments[0].starts_with('{') {
                    let mut forwarded = vec!["query_ui_nodes".to_string()];
                    forwarded.extend(arguments.iter().cloned());
                    build("query-ui", forwarded)
                } else {
                    build("query-ui", arguments.to_vec())
                }
            }
            "click" => {
                if !capturing(arguments) {
                    return Err(ToolError::invalid(
                        "Use cove devtools click NODE_ID [--capture OUT.jpg]",
                    ));
                }
                require_node_id(&arguments[0])?;
                let mut forwarded = vec!["--click-node".to_string()];
                forwarded.extend(arguments.iter().cloned());
                build("input-ui", forwarded)
            }
            "input" => {
                if !capturing(arguments) {
                    return Err(ToolError::invalid(
                        "Use cove devtools input JSON|@FILE [--capture OUT.jpg]",
                    ));
                }
                let mut forwarded = vec!["--input".to_string()];
                forwarded.extend(arguments.iter().cloned());
                build("input-ui", forwarded)
            }
            "activate" => {
                if arguments.is_empty() || arguments.len() > 2 {
                    return Err(ToolError::invalid(
                        "Use cove devtools activate NODE_ID [activate|increment|decrement]",
                    ));
                }
                require_node_id(&arguments[0])?;
                build("activate-ui", arguments.to_vec())
            }
            "reveal" => {
                if arguments.len() != 1 {
                    return Err(ToolError::invalid("Use cove devtools reveal NODE_ID"));
                }
                require_node_id(&arguments[0])?;
                build("activate-ui", vec![arguments[0].clone(), "reveal".into()])
            }
            "capture" => build("capture-ui", arguments.to_vec()),
            "resize" => {
                let counted =
                    arguments.len() == 2 || (arguments.len() == 4 && arguments[2] == "--capture");
                let sized = arguments.iter().take(2).all(|value| {
                    value
                        .parse::<f64>()
                        .map(|number| number.is_finite() && (1.0..=16_384.0).contains(&number))
                        .unwrap_or(false)
                });
                if !counted || !sized {
                    return Err(ToolError::invalid(
                        "Use cove devtools resize WIDTH HEIGHT [--capture OUT.jpg]",
                    ));
                }
                build("resize-window", arguments.to_vec())
            }
            "timings" => build("record-timings", arguments.to_vec()),
            _ => Ok(None),
        }
    }
}

/// Node ids come from `query`; an empty or non-numeric one is a typo or an
/// empty query result, reported here rather than as a host schema error.
fn require_node_id(id: &str) -> Result<(), ToolError> {
    if id.is_empty() || !id.chars().all(|character| character.is_ascii_digit()) {
        return Err(ToolError::invalid(format!(
            "NODE_ID must be a node id from cove devtools query (got \"{id}\")"
        )));
    }
    Ok(())
}

/// One operand, optionally followed by `--capture OUT.jpg`.
fn capturing(arguments: &[String]) -> bool {
    arguments.len() == 1 || (arguments.len() == 3 && arguments[1] == "--capture")
}

/// Parsed command line. Platform backends consume the same request.
#[derive(Debug, Clone)]
pub struct Arguments {
    pub operation: Operation,
    pub task_name: Option<String>,
    pub subject: Option<String>,
    pub platform: AppPlatform,
    pub requested_platform: Option<AppPlatform>,
    pub configuration: BuildConfiguration,
    pub flags: BTreeSet<Flag>,
    /// Arguments after `--` on `run`, passed unchanged to the desktop application.
    pub application_arguments: Vec<String>,
    /// A DevTools client action on `devtools`; `None` opens the Inspector.
    pub client: Option<DevToolsClient>,
    values: BTreeMap<OptionKey, String>,
}

impl Arguments {
    pub fn parse(raw: &[String]) -> Result<Self, ToolError> {
        let mut tokens: std::collections::VecDeque<String> = raw.iter().cloned().collect();
        let verb = tokens
            .pop_front()
            .ok_or_else(|| ToolError::invalid("Supply a command; use cove --help"))?;
        let mut task_name = None;
        let operation = match verb.as_str() {
            "create" => {
                if tokens.pop_front().as_deref() != Some("app") {
                    return Err(ToolError::invalid("Use cove create app <directory>"));
                }
                Operation::Create
            }
            "build" => Operation::Build,
            "run" => Operation::Run,
            "clean" => Operation::Clean,
            "test" => Operation::Test,
            "fmt" => Operation::Fmt,
            "clippy" => Operation::Clippy,
            "check" => Operation::Check,
            "migration" => match tokens.pop_front().as_deref() {
                Some("status") => Operation::Migration,
                _ => return Err(ToolError::invalid("Use cove migration status [--json]")),
            },
            "task" => {
                if let Some(first) = tokens.front() {
                    if !first.starts_with("--") {
                        task_name = tokens.pop_front();
                    }
                }
                Operation::Task
            }
            "doctor" => Operation::Doctor,
            "devices" => Operation::Devices,
            "showcase" => Operation::Showcase,
            "devtools" => Operation::Devtools,
            "internal" => match tokens.pop_front().as_deref() {
                Some("install-cli") => Operation::InstallCli,
                Some("package-cli") => Operation::PackageCli,
                Some("package-native") => Operation::PackageNative,
                Some("build-package") => Operation::PackageBuild,
                Some("android-export") => Operation::AndroidExport,
                Some("android-compile") => Operation::AndroidCompile,
                Some("android-prepare") => Operation::AndroidPrepare,
                _ => return Err(ToolError::invalid("Unknown internal operation")),
            },
            other => {
                return Err(ToolError::invalid(format!(
                    "Unknown command '{other}'; use cove --help"
                )));
            }
        };

        let mut values: BTreeMap<OptionKey, String> = BTreeMap::new();
        let mut subject: Option<String> = None;
        let mut flags: BTreeSet<Flag> = BTreeSet::new();
        let mut platform: Option<AppPlatform> = None;
        let mut configuration: Option<BuildConfiguration> = None;
        let mut application_arguments: Vec<String> = Vec::new();
        let mut client: Option<DevToolsClient> = None;

        while let Some(token) = tokens.pop_front() {
            if token == "--"
                && matches!(
                    operation,
                    Operation::Run
                        | Operation::Task
                        | Operation::Fmt
                        | Operation::Clippy
                        | Operation::Check
                )
            {
                application_arguments = tokens.iter().cloned().collect();
                break;
            } else if operation == Operation::Devtools {
                let rest: Vec<String> = tokens.iter().cloned().collect();
                if let Some(action) = DevToolsClient::parse(&token, &rest)? {
                    client = Some(action);
                    break;
                }
            }
            let stripped = token.strip_prefix("--");
            if let Some(stripped) = stripped {
                if let Some(candidate) = AppPlatform::parse(stripped) {
                    if platform.is_some() {
                        return Err(ToolError::invalid("Select exactly one platform"));
                    }
                    platform = Some(candidate);
                    continue;
                }
                if let Some(flag) = Flag::parse(stripped) {
                    if !allowed_flags(operation).contains(&flag) || !flags.insert(flag) {
                        return Err(ToolError::invalid(format!(
                            "Unexpected or duplicate flag: {token}"
                        )));
                    }
                    continue;
                }
            }
            if token == "--debug" || token == "--release" || token == "--profile" {
                if configuration.is_some() {
                    return Err(ToolError::invalid(
                        "Select exactly one of --debug, --profile or --release",
                    ));
                }
                configuration = BuildConfiguration::parse(&token[2..]);
                continue;
            }
            if let Some(stripped) = token.strip_prefix("--") {
                if let Some(key) = OptionKey::parse(stripped) {
                    let value = tokens
                        .front()
                        .cloned()
                        .filter(|value| !value.is_empty() && !value.starts_with("--"));
                    if !allowed_options(operation).contains(&key)
                        || values.contains_key(&key)
                        || value.is_none()
                    {
                        return Err(ToolError::invalid(format!(
                            "Unknown, duplicate or missing value for {token}"
                        )));
                    }
                    values.insert(key, tokens.pop_front().expect("value checked above"));
                    continue;
                }
            }
            if !token.starts_with("--") && subject.is_none() {
                subject = Some(token);
            } else {
                return Err(ToolError::invalid(format!("Unexpected argument: {token}")));
            }
        }

        if [Operation::Create, Operation::InstallCli].contains(&operation) && subject.is_none() {
            return Err(ToolError::invalid("Supply a project directory"));
        }
        if [
            Operation::Doctor,
            Operation::Devices,
            Operation::AndroidCompile,
        ]
        .contains(&operation)
            && subject.is_some()
        {
            return Err(ToolError::invalid(
                "This command does not take a project directory",
            ));
        }
        if [
            Operation::Create,
            Operation::Clean,
            Operation::Test,
            Operation::Task,
        ]
        .contains(&operation)
            && platform.is_some()
        {
            return Err(ToolError::invalid(format!(
                "{verb} is project-wide and does not take a platform flag"
            )));
        }
        if configuration.is_some()
            && ![
                Operation::Build,
                Operation::Run,
                Operation::Test,
                Operation::Task,
                Operation::Showcase,
                Operation::AndroidCompile,
                Operation::AndroidExport,
                Operation::AndroidPrepare,
                Operation::PackageBuild,
            ]
            .contains(&operation)
        {
            return Err(ToolError::invalid(format!(
                "{verb} does not take a build configuration"
            )));
        }

        let selected_platform = platform.unwrap_or_else(|| {
            if [
                Operation::AndroidCompile,
                Operation::AndroidExport,
                Operation::AndroidPrepare,
                Operation::Showcase,
            ]
            .contains(&operation)
            {
                AppPlatform::Android
            } else {
                AppPlatform::host()
            }
        });

        if [Operation::PackageNative].contains(&operation)
            && (values.contains_key(&OptionKey::Sdk)
                || values.contains_key(&OptionKey::Architecture))
        {
            let sdk = values.get(&OptionKey::Sdk);
            let architecture = values.get(&OptionKey::Architecture);
            let valid_sdk = sdk
                .map(|value| value == "iphoneos" || value == "iphonesimulator")
                .unwrap_or(true);
            let valid_architecture = architecture
                .map(|value| value == "arm64" || value == "x86_64")
                .unwrap_or(true);
            let forbidden_pair = sdk.map(String::as_str) == Some("iphoneos")
                && architecture.map(String::as_str) == Some("x86_64");
            if !(selected_platform == AppPlatform::Ios
                && valid_sdk
                && valid_architecture
                && !forbidden_pair)
            {
                return Err(ToolError::invalid(
                    "Select a valid iOS SDK and architecture",
                ));
            }
        }
        if [
            Operation::AndroidCompile,
            Operation::AndroidExport,
            Operation::AndroidPrepare,
            Operation::Showcase,
        ]
        .contains(&operation)
            && selected_platform != AppPlatform::Android
        {
            return Err(ToolError::invalid(
                "This operation currently requires --android",
            ));
        }
        let android_only = [
            OptionKey::AndroidSdk,
            OptionKey::Ndk,
            OptionKey::Keystore,
            OptionKey::KeyAlias,
            OptionKey::StorePassword,
            OptionKey::KeyPassword,
            OptionKey::Component,
        ];
        if selected_platform != AppPlatform::Android
            && android_only.iter().any(|key| values.contains_key(key))
        {
            return Err(ToolError::invalid("Android options require --android"));
        }
        if operation == Operation::Build
            && selected_platform == AppPlatform::Android
            && values.contains_key(&OptionKey::Device)
        {
            return Err(ToolError::invalid("Android --device is a run option"));
        }
        if values.contains_key(&OptionKey::Device)
            && ![AppPlatform::Android, AppPlatform::Ios].contains(&selected_platform)
        {
            return Err(ToolError::invalid("--device requires --android or --ios"));
        }
        if (values.contains_key(&OptionKey::Simulator) || values.contains_key(&OptionKey::Team))
            && selected_platform != AppPlatform::Ios
        {
            return Err(ToolError::invalid("--simulator and --team require --ios"));
        }
        if values.contains_key(&OptionKey::Simulator) && values.contains_key(&OptionKey::Device) {
            return Err(ToolError::invalid("Select either --simulator or --device"));
        }
        if configuration == Some(BuildConfiguration::Release) && flags.contains(&Flag::Devtools) {
            return Err(ToolError::invalid("DevTools requires --debug or --profile"));
        }
        if flags.contains(&Flag::NoOpen)
            && operation == Operation::Run
            && !flags.contains(&Flag::Devtools)
        {
            return Err(ToolError::invalid("--no-open requires --devtools"));
        }
        if !application_arguments.is_empty()
            && [AppPlatform::Android, AppPlatform::Ios, AppPlatform::Web]
                .contains(&selected_platform)
        {
            return Err(ToolError::invalid(
                "Application arguments after -- are supported for desktop runs",
            ));
        }
        if client.is_some() && flags.contains(&Flag::NoOpen) {
            return Err(ToolError::invalid(
                "--no-open applies to the Inspector, not to DevTools client actions",
            ));
        }

        Ok(Self {
            application_arguments,
            client,
            flags,
            operation,
            task_name,
            subject,
            platform: selected_platform,
            requested_platform: platform,
            configuration: configuration.unwrap_or(BuildConfiguration::Debug),
            values,
        })
    }

    pub fn value(&self, option: OptionKey) -> Option<&str> {
        self.values.get(&option).map(String::as_str)
    }

    pub fn required(&self, option: OptionKey) -> Result<&str, ToolError> {
        self.value(option)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ToolError::invalid(format!("Supply --{}", option.raw_value())))
    }

    pub fn url(&self, option: OptionKey) -> Option<PathBuf> {
        self.value(option).map(standardized)
    }

    pub fn directory(&self) -> PathBuf {
        match &self.subject {
            Some(subject) => standardized(subject),
            None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }
}

/// Foundation's `standardizedFileURL` for a filesystem path.
pub fn standardized(value: impl AsRef<Path>) -> PathBuf {
    let value = value.as_ref();
    let absolute = if value.is_absolute() {
        value.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(value)
    };
    let mut result = PathBuf::new();
    for component in absolute.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                result.pop();
            }
            other => result.push(other.as_os_str()),
        }
    }
    result
}

fn allowed_flags(operation: Operation) -> BTreeSet<Flag> {
    match operation {
        Operation::Run => [Flag::Devtools, Flag::NoOpen, Flag::Locked, Flag::Offline]
            .into_iter()
            .collect(),
        Operation::Devtools => [Flag::NoOpen].into_iter().collect(),
        Operation::Build => [Flag::Locked, Flag::Offline].into_iter().collect(),
        Operation::Test => [
            Flag::NoParallel,
            Flag::SkipBuild,
            Flag::BuildOnly,
            Flag::Locked,
            Flag::Offline,
        ]
        .into_iter()
        .collect(),
        Operation::Fmt => [Flag::Fix, Flag::Locked].into_iter().collect(),
        Operation::Clippy => [Flag::AllTargets, Flag::Locked, Flag::Offline, Flag::Fix]
            .into_iter()
            .collect(),
        Operation::Check => [Flag::AllTargets, Flag::Locked, Flag::Offline]
            .into_iter()
            .collect(),
        Operation::Migration => [Flag::Json, Flag::RequireComplete].into_iter().collect(),
        Operation::PackageBuild => [Flag::BuildTests].into_iter().collect(),
        _ => BTreeSet::new(),
    }
}

fn allowed_options(operation: Operation) -> BTreeSet<OptionKey> {
    let common = [OptionKey::SdkRoot];
    let android = [
        OptionKey::AndroidSdk,
        OptionKey::Ndk,
        OptionKey::Keystore,
        OptionKey::KeyAlias,
        OptionKey::StorePassword,
        OptionKey::KeyPassword,
    ];
    let union = |extra: &[OptionKey]| -> BTreeSet<OptionKey> {
        common
            .iter()
            .chain(extra.iter())
            .copied()
            .collect::<BTreeSet<_>>()
    };
    let with_android = |extra: &[OptionKey]| -> BTreeSet<OptionKey> {
        common
            .iter()
            .chain(android.iter())
            .chain(extra.iter())
            .copied()
            .collect()
    };
    match operation {
        Operation::Create => union(&[OptionKey::Id, OptionKey::Name, OptionKey::Platforms]),
        Operation::Task => union(&[]),
        Operation::Build => with_android(&[
            OptionKey::Output,
            OptionKey::Device,
            OptionKey::Simulator,
            OptionKey::Team,
            OptionKey::Package,
        ]),
        Operation::Run => with_android(&[
            OptionKey::Output,
            OptionKey::Device,
            OptionKey::Simulator,
            OptionKey::Team,
            OptionKey::Package,
        ]),
        Operation::Fmt | Operation::Clippy | Operation::Check => {
            union(&[OptionKey::Package, OptionKey::Target])
        }
        Operation::Migration => union(&[]),
        Operation::Test => union(&[
            OptionKey::Filter,
            OptionKey::ScratchPath,
            OptionKey::TestingLibrary,
            OptionKey::Timeout,
            OptionKey::EventOutput,
            OptionKey::Package,
        ]),
        Operation::Clean => BTreeSet::new(),
        Operation::InstallCli => [OptionKey::BinDirectory].into_iter().collect(),
        Operation::PackageCli => union(&[OptionKey::Output]),
        Operation::PackageNative => union(&[
            OptionKey::Output,
            OptionKey::Sdk,
            OptionKey::Architecture,
            OptionKey::Ndk,
            OptionKey::AndroidSdk,
        ]),
        Operation::PackageBuild => union(&[
            OptionKey::Product,
            OptionKey::Target,
            OptionKey::Triple,
            OptionKey::Sdk,
            OptionKey::ScratchPath,
            OptionKey::ResultFile,
        ]),
        Operation::Doctor => union(&[OptionKey::AndroidSdk, OptionKey::Ndk]),
        Operation::Devices => [OptionKey::AndroidSdk].into_iter().collect(),
        Operation::Showcase => {
            with_android(&[OptionKey::Output, OptionKey::Component, OptionKey::Device])
        }
        Operation::Devtools => union(&[OptionKey::Session]),
        Operation::AndroidPrepare => union(&[OptionKey::Ndk]),
        Operation::AndroidExport => with_android(&[
            OptionKey::Id,
            OptionKey::Name,
            OptionKey::Output,
            OptionKey::HostPackage,
            OptionKey::GameplayPackage,
            OptionKey::ProjectAssets,
        ]),
        Operation::AndroidCompile => union(&[
            OptionKey::Ndk,
            OptionKey::Product,
            OptionKey::ContentPackage,
            OptionKey::GameplayPackage,
            OptionKey::HostPackage,
            OptionKey::ScratchPath,
            OptionKey::ResultFile,
        ]),
    }
}

/// Host and device diagnostics (`cove doctor`, `cove devices`).
pub struct Diagnostics;

impl Diagnostics {
    pub fn doctor(arguments: &Arguments, root: &Path) -> Result<(), ToolError> {
        crate::toolchains::ToolchainSetup::report(arguments, root)?;
        println!(
            "Toolchain checks passed. cove build drives Cargo; platform packaging validates application linking."
        );
        Ok(())
    }

    pub fn devices(arguments: &Arguments) -> Result<(), ToolError> {
        if arguments.platform == AppPlatform::Ios {
            arguments.platform.require_available()?;
            crate::support::Shell::run(
                &[
                    "xcrun".into(),
                    "simctl".into(),
                    "list".into(),
                    "devices".into(),
                    "available".into(),
                ],
                None,
                &crate::support::environment(),
                false,
                false,
                crate::support::StdoutTarget::StandardOutput,
            )?;
            crate::support::Shell::run(
                &[
                    "xcrun".into(),
                    "devicectl".into(),
                    "list".into(),
                    "devices".into(),
                ],
                None,
                &crate::support::environment(),
                false,
                false,
                crate::support::StdoutTarget::StandardOutput,
            )?;
            return Ok(());
        }
        if arguments.platform != AppPlatform::Android {
            return Err(ToolError::invalid("Use cove devices --android or --ios"));
        }
        // Device enumeration does not need Swift's Android SDK, NDK or JDK.
        let sdk = crate::platforms_android::AndroidToolchain::sdk_directory(arguments);
        crate::support::Shell::run(
            &[
                sdk.join("platform-tools/adb").display().to_string(),
                "devices".into(),
                "-l".into(),
            ],
            None,
            &crate::support::environment(),
            false,
            false,
            crate::support::StdoutTarget::StandardOutput,
        )?;
        Ok(())
    }
}
