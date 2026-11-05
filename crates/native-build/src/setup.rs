//! Explicit host-tool installation and discovery shared by the CLI and builders.
//! Ordinary Cargo build scripts only discover tools; installation is opt-in.
use serde_json::{Value, json};
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

mod sdk;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Host {
    Mac,
    Windows,
    Linux,
}
impl Host {
    pub fn current() -> io::Result<Self> {
        match env::consts::OS {
            "macos" => Ok(Self::Mac),
            "windows" => Ok(Self::Windows),
            "linux" => Ok(Self::Linux),
            other => Err(io::Error::other(format!(
                "Cove setup does not support host {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Options {
    pub media: bool,
    pub check: bool,
    pub dry_run: bool,
    pub targets: Vec<String>,
}

#[derive(Clone, Copy)]
struct Tool {
    name: &'static str,
    candidates: &'static [&'static str],
    minimum: (u32, u32, u32),
}
const BASE: &[Tool] = &[
    Tool {
        name: "git",
        candidates: &["git"],
        minimum: (2, 25, 0),
    },
    Tool {
        name: "cmake",
        candidates: &["cmake"],
        minimum: (3, 20, 0),
    },
    Tool {
        name: "ninja",
        candidates: &["ninja", "ninja-build"],
        minimum: (1, 8, 0),
    },
];
const MEDIA: &[Tool] = &[
    Tool {
        name: "make",
        candidates: &["make", "gmake"],
        minimum: (3, 81, 0),
    },
    Tool {
        name: "nasm",
        candidates: &["nasm"],
        minimum: (2, 13, 0),
    },
    // bindgen links libclang to generate the FFmpeg bindings.
    Tool {
        name: "clang",
        candidates: &["clang"],
        minimum: (9, 0, 0),
    },
];

pub fn cache_dir() -> io::Result<PathBuf> {
    if let Some(path) = env::var_os("COVE_CACHE_DIR") {
        return Ok(PathBuf::from(path));
    }
    let root = if cfg!(windows) {
        env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        env::var_os("HOME").map(|p| PathBuf::from(p).join("Library/Caches"))
    } else {
        env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|p| PathBuf::from(p).join(".cache")))
    };
    root.map(|p| p.join("Cove"))
        .ok_or_else(|| io::Error::other("no user cache directory; set COVE_CACHE_DIR"))
}
fn state_path() -> io::Result<PathBuf> {
    Ok(cache_dir()?.join(format!(
        "setup-{}-{}.json",
        env::consts::OS,
        env::consts::ARCH
    )))
}
pub fn state() -> io::Result<Value> {
    match fs::read(state_path()?) {
        Ok(data) => serde_json::from_slice(&data).map_err(io::Error::other),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            Ok(json!({"schema":1,"paths":[],"environment":{}}))
        }
        Err(e) => Err(e),
    }
}
pub fn setting(name: &str) -> io::Result<Option<String>> {
    Ok(env::var(name).ok().or_else(|| {
        state()
            .ok()
            .and_then(|s| s["environment"][name].as_str().map(str::to_owned))
    }))
}

fn extra_paths() -> io::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for prefix in ["/opt/homebrew", "/usr/local"] {
        for tail in ["opt/nasm/bin", "bin"] {
            paths.push(Path::new(prefix).join(tail));
        }
    }
    if cfg!(windows) {
        for variable in ["LOCALAPPDATA", "ProgramFiles"] {
            if let Some(root) = env::var_os(variable) {
                for tail in [
                    "Microsoft/WinGet/Links",
                    "Microsoft/WindowsApps",
                    "CMake/bin",
                    "Git/cmd",
                    "LLVM/bin",
                ] {
                    paths.push(PathBuf::from(&root).join(tail));
                }
            }
        }
        for tail in ["msys64/ucrt64/bin", "msys64/usr/bin"] {
            paths.push(windows_drive().join(tail));
        }
    }
    Ok(paths)
}
pub fn environment() -> io::Result<Vec<(String, String)>> {
    let saved = state()?;
    let mut paths = extra_paths()?;
    if let Some(values) = saved["paths"].as_array() {
        paths.extend(values.iter().filter_map(Value::as_str).map(PathBuf::from));
    }
    if let Some(path) = saved["environment"]["PATH"].as_str() {
        paths.extend(env::split_paths(path));
    }
    if let Some(path) = env::var_os("PATH") {
        paths.extend(env::split_paths(&path));
    }
    paths.retain(|p| p.is_dir());
    let path = env::join_paths(paths)
        .map_err(io::Error::other)?
        .to_string_lossy()
        .into_owned();
    let mut values = vec![("PATH".into(), path)];
    if let Some(environment) = saved["environment"].as_object() {
        values.extend(
            environment
                .iter()
                .filter(|(k, _)| !k.eq_ignore_ascii_case("PATH"))
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.into()))),
        );
    }
    Ok(values)
}
pub fn apply(command: &mut Command) -> io::Result<()> {
    command.envs(environment()?);
    Ok(())
}
pub fn command(program: &str) -> io::Result<Command> {
    let saved = state()?;
    let preferred = saved["tools"]
        .as_array()
        .and_then(|tools| tools.iter().find(|tool| tool["name"] == program))
        .and_then(|tool| tool["path"].as_str())
        .map(PathBuf::from)
        .filter(|p| p.is_file());
    let path = preferred
        .or(find(program)?)
        .or(if program == "pkg-config" {
            find("pkgconf")?
        } else {
            None
        })
        .unwrap_or_else(|| PathBuf::from(program));
    let mut command = Command::new(path);
    apply(&mut command)?;
    Ok(command)
}
pub fn find(program: &str) -> io::Result<Option<PathBuf>> {
    let path = environment()?
        .into_iter()
        .find(|(k, _)| k == "PATH")
        .map(|(_, v)| v)
        .unwrap_or_default();
    for directory in env::split_paths(&path) {
        let file = directory.join(program);
        for candidate in [
            file.clone(),
            file.with_extension("exe"),
            file.with_extension("cmd"),
            file.with_extension("bat"),
        ] {
            if candidate.is_file() {
                return Ok(Some(candidate));
            }
        }
    }
    Ok(None)
}
fn version(text: &str) -> Option<(u32, u32, u32)> {
    for token in text.split(|c: char| !(c.is_ascii_digit() || c == '.')) {
        let mut parts = token.split('.');
        if let (Some(a), Some(b)) = (parts.next(), parts.next())
            && let (Ok(a), Ok(b)) = (a.parse(), b.parse())
        {
            return Some((a, b, parts.next().and_then(|c| c.parse().ok()).unwrap_or(0)));
        }
    }
    None
}
fn inspect(tool: Tool) -> io::Result<Value> {
    for name in tool.candidates {
        let Some(path) = find(name)? else { continue };
        let output = command(
            path.to_str()
                .ok_or_else(|| io::Error::other("non-UTF-8 tool path"))?,
        )?
        .arg("--version")
        .output()?;
        let text = String::from_utf8_lossy(&output.stdout);
        let found = version(&text);
        if output.status.success() && found.is_some_and(|v| v >= tool.minimum) {
            return Ok(
                json!({"name":tool.name,"path":path,"version":text.lines().next().unwrap_or_default(),"ready":true}),
            );
        }
    }
    Ok(
        json!({"name":tool.name,"minimum":format!("{}.{}.{}",tool.minimum.0,tool.minimum.1,tool.minimum.2),"ready":false}),
    )
}
pub fn host_triple() -> io::Result<String> {
    let output = checked_output(Command::new("rustc").arg("-vV"))?;
    output
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .map(str::to_owned)
        .ok_or_else(|| io::Error::other("rustc host triple missing"))
}
pub fn run(options: &Options) -> io::Result<Value> {
    let host = Host::current()?;
    let mut targets = options.targets.clone();
    if targets.is_empty() {
        targets.push(host_triple()?);
    }
    sdk::validate(host, &targets)?;
    let tools: Vec<_> = BASE
        .iter()
        .chain(if options.media {
            MEDIA.iter()
        } else {
            [].iter()
        })
        .copied()
        .collect();
    let compiler = sdk::inspect_compiler(host)?;
    let mut report: Vec<_> = tools
        .iter()
        .map(|t| {
            if host == Host::Mac && !compiler && t.name == "git" {
                Ok(json!({"name":"git","ready":false}))
            } else {
                inspect(*t)
            }
        })
        .collect::<io::Result<_>>()?;
    let missing = report.iter().any(|r| r["ready"] != true);
    let actions = installation_plan(host, options.media, &targets);
    if options.dry_run {
        return Ok(
            json!({"dry_run":true,"tools":report,"compiler":compiler,"targets":targets,"actions":actions}),
        );
    }
    if !options.check {
        fs::create_dir_all(cache_dir()?)?;
        if missing || !compiler || !sdk::check(host, &targets)? {
            install_host(host, options.media)?;
        }
        let mut saved = state()?;
        sdk::prepare(host, &targets, &mut saved)?;
        save(&saved)?;
        report = tools
            .iter()
            .map(|t| inspect(*t))
            .collect::<io::Result<_>>()?;
    }
    let compiler = sdk::inspect_compiler(host)?;
    let sdk = sdk::check(host, &targets)?;
    let ready = report.iter().all(|r| r["ready"] == true) && compiler && sdk;
    let result = json!({
        "ready": ready,
        "tools": report,
        "compiler": compiler,
        "sdk": sdk,
        "targets": targets,
    });
    if !options.check && ready {
        let mut saved = state()?;
        saved["schema"] = json!(1);
        saved["tools"] = json!(report);
        saved["targets"] = json!(targets);
        saved["paths"] = json!(
            report
                .iter()
                .filter_map(|t| t["path"]
                    .as_str()
                    .and_then(|p| Path::new(p).parent())
                    .map(|p| p.to_path_buf()))
                .collect::<Vec<_>>()
        );
        save(&saved)?;
    }
    Ok(result)
}
pub fn installation_plan(host: Host, media: bool, targets: &[String]) -> Value {
    json!({"host":format!("{host:?}"),"media":media,"targets":targets,
        "manager":match host {Host::Mac=>"Homebrew and Apple softwareupdate / xcodebuild",Host::Windows=>"WinGet, MSYS2 and Visual Studio Installer",Host::Linux=>"apt-get / dnf / pacman / zypper / apk"},
        "requires_elevation":true,
        "sdk":"host compiler/SDK; Android SDK+NDK for android targets; full Xcode for iOS"})
}
fn save(value: &Value) -> io::Result<()> {
    let path = state_path()?;
    fs::create_dir_all(
        path.parent()
            .ok_or_else(|| io::Error::other("setup state parent"))?,
    )?;
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(value).map_err(io::Error::other)?,
    )?;
    fs::rename(temporary, path)
}
fn install_host(host: Host, media: bool) -> io::Result<()> {
    match host {
        Host::Mac => {
            sdk::install_apple_clt()?;
            if find("brew")?.is_none() {
                let script = cache_dir()?.join("install-homebrew.sh");
                download(
                    "https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh",
                    &script,
                )?;
                checked(command("bash")?.arg(script).env("NONINTERACTIVE", "1"))?;
            }
            let mut brew = command("brew")?;
            brew.args(["install", "cmake", "ninja", "git"]);
            if media {
                brew.args(["nasm"]);
            }
            checked(&mut brew)?;
        }
        Host::Windows => {
            for (tool, id) in [(BASE[1], "Kitware.CMake"), (BASE[0], "Git.Git")] {
                if inspect(tool)?["ready"] != true {
                    winget_install(id, None)?;
                }
            }
            if media {
                let bash = windows_drive().join("msys64/usr/bin/bash.exe");
                if !bash.is_file() {
                    winget_install("MSYS2.MSYS2", None)?;
                }
                checked(Command::new(bash).env("MSYSTEM", "MSYS").args([
                    "-lc",
                    "pacman -Sy --needed --noconfirm make nasm clang ninja",
                ]))?;
            } else if inspect(BASE[2])?["ready"] != true {
                winget_install("Ninja-build.Ninja", None)?;
            }
        }
        Host::Linux => {
            let (manager, packages) = linux_packages(media)?;
            let mut install = privileged(manager)?;
            match manager {
                "apt-get" => {
                    checked(privileged(manager)?.arg("update"))?;
                    install.args(["install", "-y"]);
                }
                "dnf" => {
                    install.args(["install", "-y"]);
                }
                "pacman" => {
                    install.args(["-S", "--needed", "--noconfirm"]);
                }
                "zypper" => {
                    install.args(["--non-interactive", "install"]);
                }
                "apk" => {
                    install.arg("add");
                }
                _ => return Err(io::Error::other("unsupported package manager")),
            }
            checked(install.args(packages))?;
        }
    }
    Ok(())
}
fn windows_drive() -> PathBuf {
    PathBuf::from(format!(
        "{}/",
        env::var_os("SystemDrive")
            .unwrap_or_else(|| "C:".into())
            .to_string_lossy()
            .trim_end_matches(['/', '\\'])
    ))
}
pub(super) fn winget_install(id: &str, extra: Option<&str>) -> io::Result<()> {
    if find("winget")?.is_none() {
        checked(command("powershell")?.args(["-NoProfile","-NonInteractive","-Command", "$ErrorActionPreference='Stop'; [Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12; Install-PackageProvider NuGet -Force -Scope CurrentUser; Install-Module Microsoft.WinGet.Client -Force -Repository PSGallery -Scope CurrentUser; Import-Module Microsoft.WinGet.Client; Repair-WinGetPackageManager"]))?;
    }
    let mut install = command("winget")?;
    install.args([
        "install",
        "--id",
        id,
        "--exact",
        "--source",
        "winget",
        "--silent",
        "--accept-source-agreements",
        "--accept-package-agreements",
    ]);
    if let Some(extra) = extra {
        install.args(["--override", extra]);
    }
    checked(&mut install)
}
fn linux_packages(media: bool) -> io::Result<(&'static str, Vec<&'static str>)> {
    for manager in ["apt-get", "dnf", "pacman", "zypper", "apk"] {
        if find(manager)?.is_some() {
            let mut packages = match manager {
                "apt-get" => vec![
                    "build-essential",
                    "cmake",
                    "ninja-build",
                    "git",
                    "curl",
                    "ca-certificates",
                ],
                "dnf" => vec![
                    "gcc",
                    "gcc-c++",
                    "make",
                    "cmake",
                    "ninja-build",
                    "git",
                    "curl",
                    "ca-certificates",
                ],
                "pacman" => vec![
                    "base-devel",
                    "cmake",
                    "ninja",
                    "git",
                    "curl",
                    "ca-certificates",
                ],
                "zypper" => vec![
                    "gcc",
                    "gcc-c++",
                    "make",
                    "cmake",
                    "ninja",
                    "git",
                    "curl",
                    "ca-certificates",
                ],
                _ => vec![
                    "build-base",
                    "cmake",
                    "ninja",
                    "git",
                    "curl",
                    "ca-certificates",
                ],
            };
            if media {
                packages.extend(match manager {
                    "apt-get" => vec![
                        "pkg-config",
                        "libasound2-dev",
                        "nasm",
                        "clang",
                        "libclang-dev",
                    ],
                    "dnf" => vec![
                        "pkgconf-pkg-config",
                        "alsa-lib-devel",
                        "nasm",
                        "clang",
                        "clang-devel",
                    ],
                    "pacman" => vec!["pkgconf", "alsa-lib", "nasm", "clang"],
                    "zypper" => vec![
                        "pkgconf-pkg-config",
                        "alsa-devel",
                        "nasm",
                        "clang",
                        "clang-devel",
                    ],
                    _ => vec!["pkgconf", "alsa-lib-dev", "nasm", "clang", "clang-dev"],
                });
            }
            return Ok((manager, packages));
        }
    }
    Err(io::Error::other(
        "no supported Linux package manager (apt-get, dnf, pacman, zypper, apk)",
    ))
}
pub(super) fn privileged(program: &str) -> io::Result<Command> {
    let root = Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .is_some_and(|o| o.stdout == b"0\n");
    let mut command = if root {
        command(program)?
    } else {
        let mut c = command("sudo")?;
        c.arg(program);
        c
    };
    apply(&mut command)?;
    Ok(command)
}
pub(super) fn checked(command: &mut Command) -> io::Result<()> {
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "setup command {command:?} failed with {status}"
        )))
    }
}
pub(super) fn checked_output(command: &mut Command) -> io::Result<String> {
    let output = command.output()?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(io::Error::other(format!(
            "setup command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )))
    }
}
pub(super) fn download(url: &str, path: &Path) -> io::Result<()> {
    fs::create_dir_all(
        path.parent()
            .ok_or_else(|| io::Error::other("download parent"))?,
    )?;
    checked(
        command("curl")?
            .args(["--fail", "--location", "--retry", "3", "--output"])
            .arg(path)
            .arg(url),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_real_tool_version_formats() {
        for (text, expected) in [
            ("GNU Make 4.4.1", (4, 4, 1)),
            ("NASM version 2.16.01 compiled on Apr 21 2024", (2, 16, 1)),
            ("cygwin32-make 3.81", (3, 81, 0)),
            ("clang version 18.1.8", (18, 1, 8)),
            ("cmake version 4.0.1", (4, 0, 1)),
        ] {
            assert_eq!(version(text), Some(expected));
        }
        assert_eq!(version("not installed"), None);
    }
    #[test]
    fn every_host_has_an_installation_plan_and_mobile_sdk_paths() {
        for host in [Host::Mac, Host::Windows, Host::Linux] {
            let plan = installation_plan(host, true, &["aarch64-linux-android".into()]);
            assert!(plan["manager"].as_str().expect("manager").len() > 5);
            assert!(plan["sdk"].as_str().expect("SDK").contains("Android"));
        }
    }
}
