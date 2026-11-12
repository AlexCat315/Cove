//! Explicit host-tool installation and discovery shared by the CLI and builders.
//! Ordinary Cargo build scripts only discover tools; installation is opt-in.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

mod sdk;
pub mod shell;

/// Zig release used by `crates/terminal/build.rs` and the CI toolchain.
const ZIG_VERSION: &str = "0.17.0";
/// Slang release required by `Tools/Shaders/build-native-ui.mjs`.
const SLANG_VERSION: &str = "2026.19";
/// wasi-sdk release used by every `wasm32-wasip*` build.
const WASI_SDK_VERSION: &str = "34.0";

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
    /// Also provision the pinned Slang/DXIL shader toolchain.
    pub shaders: bool,
    /// Provision the pinned wasi-sdk and a native Ninja for Wasm builds.
    pub wasm: bool,
    pub check: bool,
    pub dry_run: bool,
    pub targets: Vec<String>,
    /// Add Cove's pinned-tool directories to the user-level PATH. Opt-in:
    /// Cargo builds never need it, and a persistent PATH is a machine-wide
    /// change that must not happen silently.
    pub persist_path: bool,
    /// Remove only the entries Cove previously added to the user-level PATH.
    pub unpersist_path: bool,
}

/// Statements that expose Cove's pinned tools in the invoking shell without
/// changing any persistent configuration (`cargo cove env`).
pub fn shell_exports(shell: shell::Shell) -> io::Result<String> {
    Ok(shell::exports(&state()?, shell))
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
    // `cove-terminal`'s build script compiles libghostty-vt with `zig build`
    // on every build, so Zig is a base tool rather than an optional one.
    Tool {
        name: "zig",
        candidates: &["zig"],
        minimum: (0, 17, 0),
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

#[derive(Clone, Copy)]
struct PinnedTool {
    name: &'static str,
    env_key: &'static str,
    version: &'static str,
}
const PINNED_ZIG: PinnedTool = PinnedTool {
    name: "zig",
    env_key: "COVE_ZIG",
    version: ZIG_VERSION,
};
const PINNED_SLANG: PinnedTool = PinnedTool {
    name: "slangc",
    env_key: "SLANGC",
    version: SLANG_VERSION,
};
const PINNED_WASI_SDK: PinnedTool = PinnedTool {
    name: "wasi-sdk",
    env_key: "COVE_WASI_SDK_PATH",
    version: WASI_SDK_VERSION,
};

struct Archive {
    name: &'static str,
    url: String,
    checksum: &'static str,
    /// `github.com/.../releases/download` can fail with 5xx at the edge. The
    /// API asset endpoint redirects to the signed CDN object instead.
    fallback: Option<String>,
}

/// Release asset IDs for v2026.19. Accepted only over HTTPS and still verified
/// against the pinned SHA-256 after download.
fn slang_asset_id(name: &str) -> Option<u64> {
    Some(match name {
        "slang-2026.19-macos-aarch64.zip" => 598765289,
        "slang-2026.19-macos-x86_64.zip" => 598771849,
        "slang-2026.19-linux-aarch64.zip" => 598712114,
        "slang-2026.19-linux-x86_64.zip" => 598767514,
        "slang-2026.19-windows-aarch64.zip" => 598738790,
        "slang-2026.19-windows-x86_64.zip" => 598733870,
        _ => return None,
    })
}

/// Release asset IDs for wasi-sdk-34, used as the fallback when the
/// `github.com/.../releases/download` edge fails. Hashes are still verified.
fn wasi_asset_id(name: &str) -> Option<u64> {
    Some(match name {
        "wasi-sdk-34.0-arm64-linux.tar.gz" => 528366381,
        "wasi-sdk-34.0-arm64-macos.tar.gz" => 528366387,
        "wasi-sdk-34.0-arm64-windows.tar.gz" => 528366384,
        "wasi-sdk-34.0-x86_64-linux.tar.gz" => 528366474,
        "wasi-sdk-34.0-x86_64-macos.tar.gz" => 528366482,
        "wasi-sdk-34.0-x86_64-windows.tar.gz" => 528366541,
        _ => return None,
    })
}

/// Official, checksum-pinned archives. Hashes mirror the release manifests at
/// ziglang.org/download/index.json, the v2026.19 GitHub release digests and the
/// wasi-sdk-34 release asset digests; they are also used by CI.
fn pinned_archive(tool: PinnedTool, host: Host, arch: &str) -> Option<Archive> {
    let (name, checksum, base) = match (tool.name, host, arch) {
        ("zig", Host::Mac, "aarch64") => (
            "zig-aarch64-macos-0.17.0.tar.xz",
            "b607e9b9234790a008116ae5bdb71c6243b84b9fb42a53a9e70fde41c06c536a",
            format!("https://ziglang.org/download/{ZIG_VERSION}"),
        ),
        ("zig", Host::Mac, "x86_64") => (
            "zig-x86_64-macos-0.17.0.tar.xz",
            "4f9a1c5269aa17ebda5e6d3c2b89d6cbf36f7d2b22a0306e9ab98f25f95529c6",
            format!("https://ziglang.org/download/{ZIG_VERSION}"),
        ),
        ("zig", Host::Linux, "aarch64") => (
            "zig-aarch64-linux-0.17.0.tar.xz",
            "9e8d11661d4ae3bd57702a3832781e23ad151dde5798e16a5ccd503f65234ff8",
            format!("https://ziglang.org/download/{ZIG_VERSION}"),
        ),
        ("zig", Host::Linux, "x86_64") => (
            "zig-x86_64-linux-0.17.0.tar.xz",
            "1cbe9df9f27e6b78d14ccbca43b6703a404ef79ef1c463de901d7f088d4e2026",
            format!("https://ziglang.org/download/{ZIG_VERSION}"),
        ),
        ("zig", Host::Windows, "aarch64") => (
            "zig-aarch64-windows-0.17.0.zip",
            "0a59d91fa1cb40cf068e9b0954434ce973500c7a2ea749f1e01af62cdab52d26",
            format!("https://ziglang.org/download/{ZIG_VERSION}"),
        ),
        ("zig", Host::Windows, "x86_64") => (
            "zig-x86_64-windows-0.17.0.zip",
            "b5663f69581dcf391293fbf16c06cb80d81d806545ce618b4d0bab7f0eb8c428",
            format!("https://ziglang.org/download/{ZIG_VERSION}"),
        ),
        ("slangc", Host::Mac, "aarch64") => (
            "slang-2026.19-macos-aarch64.zip",
            "f77d89b91ec63698a7a8e5aa4316ce08b70d3211a236640ee4551f55be85d9ed",
            format!("https://github.com/shader-slang/slang/releases/download/v{SLANG_VERSION}"),
        ),
        ("slangc", Host::Mac, "x86_64") => (
            "slang-2026.19-macos-x86_64.zip",
            "c5e18233c0b90cc651c792ec4c4e4a467a9a101ffb6301643aab89c016b48e42",
            format!("https://github.com/shader-slang/slang/releases/download/v{SLANG_VERSION}"),
        ),
        ("slangc", Host::Linux, "aarch64") => (
            "slang-2026.19-linux-aarch64.zip",
            "39632fa19ca3ea9d65253c6f5450d1971a6f3e7b578a1e1998c7ce2b28048221",
            format!("https://github.com/shader-slang/slang/releases/download/v{SLANG_VERSION}"),
        ),
        ("slangc", Host::Linux, "x86_64") => (
            "slang-2026.19-linux-x86_64.zip",
            "758446aa36dd9dc985b6a93e79fff9ed7fc139180c89cb5d3f09427be5d0d2b7",
            format!("https://github.com/shader-slang/slang/releases/download/v{SLANG_VERSION}"),
        ),
        ("slangc", Host::Windows, "aarch64") => (
            "slang-2026.19-windows-aarch64.zip",
            "6ddc5a56d22690d2077f91edea884f88f8ea35cba4d2f717fc84bf1c2ded766f",
            format!("https://github.com/shader-slang/slang/releases/download/v{SLANG_VERSION}"),
        ),
        ("slangc", Host::Windows, "x86_64") => (
            "slang-2026.19-windows-x86_64.zip",
            "fc922f214b5d115f663932e59e80e0f6b6cc6fae140979cf307da6459b6f8f33",
            format!("https://github.com/shader-slang/slang/releases/download/v{SLANG_VERSION}"),
        ),
        ("wasi-sdk", Host::Mac, "aarch64") => (
            "wasi-sdk-34.0-arm64-macos.tar.gz",
            "9c59398106b417f8f14913380fdf0097a8cc0ff4af9eb3ce0065a859e88d49e9",
            format!("https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-34"),
        ),
        ("wasi-sdk", Host::Mac, "x86_64") => (
            "wasi-sdk-34.0-x86_64-macos.tar.gz",
            "87d27fa8adc68dee59bfbf2e22a6d34ef717c34d6bf1d8af2a56fc929d9ce0eb",
            format!("https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-34"),
        ),
        ("wasi-sdk", Host::Linux, "aarch64") => (
            "wasi-sdk-34.0-arm64-linux.tar.gz",
            "f7e243dff54d60bcc576e94d6166b69f410f2500ae4a9ceef34315be10e77971",
            format!("https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-34"),
        ),
        ("wasi-sdk", Host::Linux, "x86_64") => (
            "wasi-sdk-34.0-x86_64-linux.tar.gz",
            "b761e3a0721dbae9c09a0059e5fdb2bf917d1b4a8a7b430fb3b5aafb0984b2c4",
            format!("https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-34"),
        ),
        ("wasi-sdk", Host::Windows, "aarch64") => (
            "wasi-sdk-34.0-arm64-windows.tar.gz",
            "45e1c71f3e965621e7b98ebe1d37b0e4b1f77f3e8072113ffb4534e67b1a4b7c",
            format!("https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-34"),
        ),
        ("wasi-sdk", Host::Windows, "x86_64") => (
            "wasi-sdk-34.0-x86_64-windows.tar.gz",
            "cccb5c323a9b34f0349a9b09e8804a0a7632c68c3310f4b5f437ed57d7e71d8f",
            format!("https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-34"),
        ),
        _ => return None,
    };
    let fallback = match tool.name {
        "slangc" => slang_asset_id(name).map(|id| {
            format!("https://api.github.com/repos/shader-slang/slang/releases/assets/{id}")
        }),
        "wasi-sdk" => wasi_asset_id(name).map(|id| {
            format!("https://api.github.com/repos/WebAssembly/wasi-sdk/releases/assets/{id}")
        }),
        _ => None,
    };
    Some(Archive {
        name,
        url: format!("{base}/{name}"),
        checksum,
        fallback,
    })
}

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
pub fn state_path() -> io::Result<PathBuf> {
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
/// Process environment for third-party build scripts that cannot read the setup
/// state themselves: FFmpeg's `configure` runs under `sh` with the MSVC
/// toolchain and only sees the process environment. Windows media builds only;
/// other hosts and targets are left untouched.
pub fn apply_media_process() -> io::Result<()> {
    #[cfg(windows)]
    {
        if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
            || env::var("CARGO_FEATURE_BUILD").is_err()
        {
            return Ok(());
        }
        let msys = windows_drive().join("msys64");
        if !msys.join("usr/bin/sh.exe").is_file() {
            return Err(io::Error::other(
                "MSYS2 not found; run 'cargo cove setup --media'",
            ));
        }
        let (include, lib, msvc_path) = msvc_environment()?;
        let mut paths = Vec::new();
        for path in env::split_paths(&msvc_path) {
            let text = path.to_string_lossy();
            // Git's coreutils sh/link and any earlier MSYS entry would shadow the
            // MSVC linker or the MSYS tools appended below.
            if text.ends_with("Git\\usr\\bin")
                || text.ends_with("Git\\mingw64\\bin")
                || path.starts_with(&msys)
            {
                continue;
            }
            paths.push(path);
        }
        // fetch() shells out to git, which setup may only discover outside PATH
        // (Git\cmd and friends), so the injected PATH needs those directories too.
        for path in extra_paths()? {
            if path.starts_with(&msys) || !path.is_dir() {
                continue;
            }
            paths.push(path);
        }
        paths.push(msys.join("usr/bin"));
        paths.push(msys.join("ucrt64/bin"));
        let path = env::join_paths(paths).map_err(io::Error::other)?;
        // Build scripts are single-threaded, so mutating the process environment is
        // safe and is the only way to reach configure, make and bindgen from here.
        unsafe {
            env::set_var("PATH", path);
            env::set_var("INCLUDE", include);
            env::set_var("LIB", lib);
        }
        if env::var_os("LIBCLANG_PATH").is_none() {
            let libclang = msys.join("ucrt64/bin");
            if libclang.join("libclang.dll").is_file() {
                unsafe { env::set_var("LIBCLANG_PATH", libclang) };
            }
        }
        // MSYS2's libclang defaults to the GNU target and would parse FFmpeg's
        // headers against mingw's include tree instead of the Windows SDK.
        let target = "--target=x86_64-pc-windows-msvc";
        let extra = env::var("BINDGEN_EXTRA_CLANG_ARGS").unwrap_or_default();
        if !extra.contains(target) {
            let extra = if extra.is_empty() {
                target.to_string()
            } else {
                format!("{extra} {target}")
            };
            unsafe { env::set_var("BINDGEN_EXTRA_CLANG_ARGS", extra) };
        }
    }
    Ok(())
}
#[cfg(windows)]
fn msvc_environment() -> io::Result<(String, String, String)> {
    use std::os::windows::process::CommandExt;
    let root =
        env::var_os("ProgramFiles(x86)").unwrap_or_else(|| "C:\\Program Files (x86)".into());
    let vswhere = Path::new(&root).join("Microsoft Visual Studio/Installer/vswhere.exe");
    let output = Command::new(vswhere)
        .args([
            "-latest",
            "-products",
            "*",
            "-requires",
            "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
            "-property",
            "installationPath",
        ])
        .output()?;
    let installation = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if installation.is_empty() {
        return Err(io::Error::other(
            "Visual Studio C++ tools not found; run 'cargo cove setup --media'",
        ));
    }
    let vcvars = Path::new(&installation).join("VC/Auxiliary/Build/vcvars64.bat");
    // cmd does not understand Command's backslash-quote escaping, so the whole
    // command line goes in raw.
    let output = Command::new("cmd")
        .raw_arg(format!(
            "/c call \"{}\" >nul 2>&1 && set",
            vcvars.display()
        ))
        .output()?;
    let mut values = (String::new(), String::new(), String::new());
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        // Windows keeps the inherited name's case ("Path" in most shells), so
        // matching PATH exactly would silently drop the MSVC directories.
        match key.to_ascii_uppercase().as_str() {
            "INCLUDE" => values.0 = value.to_string(),
            "LIB" => values.1 = value.to_string(),
            "PATH" => values.2 = value.to_string(),
            _ => {}
        }
    }
    if values.0.is_empty() || values.1.is_empty() || values.2.is_empty() {
        return Err(io::Error::other(
            "vcvars64 did not report INCLUDE/LIB/PATH",
        ));
    }
    Ok(values)
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
/// Zig uses `zig version`; Slang uses `slangc -version`; everything else uses
/// the conventional `--version`.
fn version_arg(name: &str) -> &'static str {
    match name {
        "zig" => "version",
        "slangc" => "-version",
        _ => "--version",
    }
}

fn inspect(tool: Tool) -> io::Result<Value> {
    for name in tool.candidates {
        let Some(path) = find(name)? else { continue };
        let output = command(
            path.to_str()
                .ok_or_else(|| io::Error::other("non-UTF-8 tool path"))?,
        )?
        .arg(version_arg(tool.name))
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
/// How Cove's tools reach a shell. The report states this explicitly so a
/// developer never assumes `cargo cove setup` edited their machine's PATH.
fn shell_report() -> Value {
    json!({
        "build_scripts": "automatic: Cove injects the setup state into build-script child processes",
        "terminal": "not changed by default; use `cargo cove env` or `cargo cove setup --persist-path`",
    })
}

fn user_path_plan(options: &Options) -> Value {
    json!({
        "action": if options.persist_path {
            "persist"
        } else if options.unpersist_path {
            "unpersist"
        } else {
            "none"
        },
        "applied": false,
        "scope": if cfg!(windows) {
            "user-level PATH (HKCU\\Environment)"
        } else {
            "shell profile managed block"
        },
        "hint": "omit --persist-path to leave the machine untouched",
    })
}

pub fn run(options: &Options) -> io::Result<Value> {
    if options.persist_path && options.unpersist_path {
        return Err(io::Error::other(
            "--persist-path and --unpersist-path are mutually exclusive",
        ));
    }
    if options.unpersist_path && !options.check && !options.dry_run {
        shell::unpersist()?;
        return Ok(json!({
            "ready": true,
            "user_path": {"persistent": false, "action": "removed"},
            "shell": shell_report(),
        }));
    }
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
    let mut saved = state()?;
    let mut wasm = wasm_status(options.wasm);
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
    let actions = installation_plan(host, options.media, options.shaders, options.wasm, &targets);
    if options.dry_run {
        return Ok(json!({
            "dry_run": true,
            "tools": report,
            "compiler": compiler,
            "submodules": submodule_status()?,
            "shaders": shader_status(&saved, host)?,
            "wasm": wasm,
            "targets": targets,
            "actions": actions,
            "user_path": user_path_plan(options),
            "shell": shell_report(),
        }));
    }
    if !options.check {
        fs::create_dir_all(cache_dir()?)?;
        if missing || !compiler || !sdk::check(host, &targets)? {
            install_host(host, options.media)?;
        }
        if submodule_status()?["ready"] != true {
            checked(command("git")?.current_dir(project_root()).args([
                "submodule",
                "update",
                "--init",
                "--recursive",
            ]))?;
        }
        ensure_pinned(PINNED_ZIG, host, &mut saved)?;
        if options.shaders {
            ensure_pinned(PINNED_SLANG, host, &mut saved)?;
            install_dxil(host, &saved)?;
        }
        if options.wasm {
            ensure_wasi_sdk(host, &mut saved)?;
            record_native_ninja(&mut saved)?;
            wasm = wasm_status(true);
        }
        sdk::prepare(host, &targets, &mut saved)?;
        save(&saved)?;
        report = tools
            .iter()
            .map(|t| inspect(*t))
            .collect::<io::Result<_>>()?;
    }
    let compiler = sdk::inspect_compiler(host)?;
    let sdk = sdk::check(host, &targets)?;
    let submodules = submodule_status()?;
    let shaders = shader_status(&saved, host)?;
    let ready = report.iter().all(|r| r["ready"] == true)
        && compiler
        && sdk
        && submodules["ready"] == true
        && (!options.shaders || shaders["ready"] == true)
        && (!options.wasm || wasm["ready"] == true);
    let user_path = if options.persist_path && !options.check && !options.dry_run {
        if ready {
            let directories = shell::persist(&saved)?;
            json!({
                "persistent": true,
                "action": "persisted",
                "directories": directories
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>(),
                "restart_shell": true,
            })
        } else {
            json!({
                "persistent": false,
                "action": "skipped",
                "reason": "setup is not ready; no tools were persisted",
            })
        }
    } else {
        user_path_plan(options)
    };
    let result = json!({
        "ready": ready,
        "tools": report,
        "compiler": compiler,
        "sdk": sdk,
        "submodules": submodules,
        "shaders": shaders,
        "wasm": wasm,
        "targets": targets,
        "user_path": user_path,
        "shell": shell_report(),
    });
    if !options.check && ready {
        saved["schema"] = json!(1);
        saved["tools"] = json!(report);
        saved["targets"] = json!(targets);
        let mut directories: Vec<PathBuf> = report
            .iter()
            .filter_map(|t| {
                t["path"]
                    .as_str()
                    .and_then(|p| Path::new(p).parent())
                    .map(Path::to_path_buf)
            })
            .collect();
        directories.dedup();
        saved["paths"] = json!(directories);
        save(&saved)?;
    }
    Ok(result)
}
fn find_sdk_root(destination: &Path) -> Option<PathBuf> {
    if wasi_sdk_valid(destination) {
        return Some(destination.to_path_buf());
    }
    let entries = fs::read_dir(destination).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && wasi_sdk_valid(&path) {
            return Some(path);
        }
    }
    None
}

/// An already usable wasi-sdk, from the process environment or the setup state.
/// A bare path is not enough: the CMake toolchain, clang and the sysroot must
/// all be present, otherwise a `cargo build --target wasm32-wasip1` fails deep
/// inside a build script.
pub fn wasm_sdk_path() -> io::Result<Option<PathBuf>> {
    let root = env::var_os(PINNED_WASI_SDK.env_key)
        .map(PathBuf::from)
        .or_else(|| {
            setting(PINNED_WASI_SDK.env_key)
                .ok()
                .flatten()
                .map(PathBuf::from)
        });
    Ok(root.filter(|root| wasi_sdk_valid(root)))
}

fn wasi_sdk_valid(root: &Path) -> bool {
    let binary = |name: &str| {
        root.join("bin").join(if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.to_owned()
        })
    };
    binary("clang").is_file()
        && binary("clang++").is_file()
        && root.join("share/cmake").is_dir()
        && root.join("share/wasi-sysroot").is_dir()
}

/// Finds a Ninja that does not execute commands through a POSIX shell.
/// On Windows, MSYS2's Ninja runs commands with `/bin/sh`, which eats the
/// backslashes in `C:\...\clang.exe`; a native Win32 Ninja uses `cmd.exe`.
pub fn native_ninja() -> io::Result<PathBuf> {
    if let Some(path) = env::var_os("COVE_NINJA")
        .map(PathBuf::from)
        .or_else(|| {
            setting("COVE_NINJA")
                .ok()
                .flatten()
                .map(PathBuf::from)
        })
        .filter(|path| path.is_file())
    {
        return Ok(path);
    }
    let search = environment()?
        .into_iter()
        .find(|(key, _)| key == "PATH")
        .map(|(_, value)| value)
        .unwrap_or_default();
    let mut posix_shell = None;
    for directory in env::split_paths(&search) {
        for candidate in [directory.join("ninja"), directory.join("ninja.exe")] {
            if !candidate.is_file() {
                continue;
            }
            let normalized = candidate
                .to_string_lossy()
                .replace('\\', "/")
                .to_ascii_lowercase();
            let shell_based = cfg!(windows)
                && (normalized.contains("/msys64/")
                    || normalized.contains("/cygwin")
                    || normalized.contains("/usr/bin/"));
            if shell_based {
                posix_shell.get_or_insert(candidate);
            } else {
                return Ok(candidate);
            }
        }
    }
    posix_shell.ok_or_else(|| {
        io::Error::other(
            "a native Ninja is required for Wasm CMake builds; install Ninja and run `cargo cove setup --wasm`",
        )
    })
}

/// Downloads and unpacks the pinned wasi-sdk into the Cove cache when no usable
/// installation exists. The archive is verified against the release digest.
fn ensure_wasi_sdk(host: Host, saved: &mut Value) -> io::Result<Value> {
    if let Some(root) = wasm_sdk_path()? {
        saved["environment"][PINNED_WASI_SDK.env_key] = json!(root.display().to_string());
        save(saved)?;
        return Ok(json!({
            "name": "wasi-sdk", "ready": true,
            "path": root.display().to_string(), "source": "host",
        }));
    }
    let archive = pinned_archive(PINNED_WASI_SDK, host, env::consts::ARCH).ok_or_else(|| {
        io::Error::other(format!(
            "no pinned wasi-sdk archive for {}-{}",
            env::consts::OS,
            env::consts::ARCH
        ))
    })?;
    let file = cache_dir()?.join("downloads").join(archive.name);
    if !checksum_matches(&file, archive.checksum) {
        download_pinned(&archive, &file)?;
    }
    let bytes = fs::read(&file)?;
    let actual = format!("{:x}", Sha256::digest(&bytes));
    if actual != archive.checksum {
        return Err(io::Error::other(format!(
            "{} checksum mismatch: {actual}",
            archive.name
        )));
    }
    let destination = cache_dir()?
        .join("tools")
        .join(format!("wasi-sdk-{WASI_SDK_VERSION}"));
    if destination.exists() {
        fs::remove_dir_all(&destination)?;
    }
    fs::create_dir_all(&destination)?;
    // `cmake -E tar` is a native executable on every host. The MSYS tar on
    // Windows treats `C:\...` as an rsh-style remote path, so it cannot unpack
    // into the Cove cache; CMake also handles gzip transparently.
    checked(
        command("cmake")?
            .args(["-E", "tar", "xf"])
            .arg(&file)
            .current_dir(&destination),
    )?;
    // Release archives wrap the SDK in one top-level directory. Keep that
    // layout instead of stripping it, so no tar-specific flags are needed.
    let root = find_sdk_root(&destination).ok_or_else(|| {
        io::Error::other(format!(
            "wasi-sdk {} is incomplete in {}",
            archive.name,
            destination.display()
        ))
    })?;
    saved["environment"][PINNED_WASI_SDK.env_key] = json!(root.display().to_string());
    save(saved)?;
    Ok(json!({
        "name": "wasi-sdk", "ready": true,
        "path": root.display().to_string(),
        "source": archive.url, "checksum": archive.checksum,
    }))
}

fn wasm_status(enabled: bool) -> Value {
    if !enabled {
        return json!({"enabled": false});
    }
    let sdk = wasm_sdk_path().ok().flatten();
    let ninja = native_ninja();
    json!({
        "enabled": true,
        "ready": sdk.is_some() && ninja.is_ok(),
        "wasi_sdk": sdk.map(|path| path.display().to_string()),
        "ninja": ninja.as_ref().ok().map(|path| path.display().to_string()),
        "error": ninja.as_ref().err().map(|error| error.to_string()),
    })
}

fn record_native_ninja(saved: &mut Value) -> io::Result<()> {
    let path = native_ninja()?;
    saved["environment"]["COVE_NINJA"] = json!(path.display().to_string());
    save(saved)
}

pub fn installation_plan(host: Host, media: bool, shaders: bool, wasm: bool, targets: &[String]) -> Value {
    json!({"host":format!("{host:?}"),"media":media,"shaders":shaders,"wasm":wasm,"targets":targets,
        "manager":match host {Host::Mac=>"Homebrew and Apple softwareupdate / xcodebuild",Host::Windows=>"WinGet, MSYS2 and Visual Studio Installer",Host::Linux=>"apt-get / dnf / pacman / zypper / apk"},
        "requires_elevation":true,
        "sdk":"host compiler/SDK; Android SDK+NDK for android targets; full Xcode for iOS",
        "pinned":format!("Zig {ZIG_VERSION}, Slang {SLANG_VERSION} and wasi-sdk {WASI_SDK_VERSION} archives, SHA-256 verified in the Cove cache"),
        "submodules":"git submodule update --init --recursive"})
}
fn save(value: &Value) -> io::Result<()> {
    let path = state_path()?;
    let directory = create_cache_directory(path.parent().expect("setup state parent"))?;
    let path = directory.join(path.file_name().expect("setup state filename"));
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(value).map_err(io::Error::other)?,
    )?;
    fs::rename(temporary, path)
}

/// Creates a cache directory and returns the real path on Windows, where an
/// MSIX app can redirect writes under `LOCALAPPDATA` to a private directory.
pub fn create_cache_directory(path: &Path) -> io::Result<PathBuf> {
    fs::create_dir_all(path)?;
    if cfg!(windows) {
        fs::canonicalize(path)
    } else {
        Ok(path.to_path_buf())
    }
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
fn download_with(url: &str, path: &Path, header: Option<&str>, retries: &str) -> io::Result<()> {
    fs::create_dir_all(
        path.parent()
            .ok_or_else(|| io::Error::other("download parent"))?,
    )?;
    let mut curl = command("curl")?;
    curl.args([
        "--fail",
        "--location",
        "--retry",
        retries,
        "--retry-delay",
        "2",
    ]);
    if let Some(header) = header {
        curl.args(["--header", header]);
    }
    curl.args(["--output"]).arg(path).arg(url);
    checked(&mut curl).map_err(|error| io::Error::other(format!("download {url} failed: {error}")))
}
pub(super) fn download(url: &str, path: &Path) -> io::Result<()> {
    download_with(url, path, None, "5")
}

/// Tries the canonical release URL once, then the API asset endpoint when the
/// GitHub release edge answers 5xx. Both responses must pass the same SHA-256.
fn download_pinned(archive: &Archive, path: &Path) -> io::Result<()> {
    if let Ok(()) = download_with(&archive.url, path, None, "1") {
        return Ok(());
    }
    let Some(fallback) = &archive.fallback else {
        return Err(io::Error::other(format!(
            "download {} failed and this archive has no fallback endpoint",
            archive.url
        )));
    };
    download_with(
        fallback,
        path,
        Some("Accept: application/octet-stream"),
        "5",
    )
    .map_err(|error| {
        io::Error::other(format!(
            "download {} and fallback {fallback} both failed: {error}",
            archive.url
        ))
    })
}

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn submodule_status() -> io::Result<Value> {
    let mut git = command("git")?;
    let output = checked_output(git.current_dir(project_root()).args([
        "submodule",
        "status",
        "--recursive",
    ]));
    // A dry run can be used before Git or the platform tools are installed.
    let output = match output {
        Ok(output) => output,
        Err(error) => return Ok(json!({"ready": false, "error": error.to_string()})),
    };
    let mut missing = Vec::new();
    let mut mismatched = Vec::new();
    let mut conflicted = Vec::new();
    for line in output.lines() {
        let Some((_, path)) = line.split_once(' ') else {
            continue;
        };
        match line.as_bytes()[0] {
            b'-' => missing.push(path),
            b'+' => mismatched.push(path.rsplit_once(" (").map_or(path, |(path, _)| path)),
            b'U' => conflicted.push(path),
            _ => {}
        }
    }
    Ok(json!({
        "ready": missing.is_empty() && mismatched.is_empty() && conflicted.is_empty(),
        "total": output.lines().count(),
        "missing": missing,
        "mismatched": mismatched,
        "conflicted": conflicted,
    }))
}

fn shader_binary(saved: &Value) -> Option<PathBuf> {
    env::var_os("SLANGC")
        .map(PathBuf::from)
        .or_else(|| saved["environment"]["SLANGC"].as_str().map(PathBuf::from))
        .filter(|path| path.is_file())
        .or_else(|| find("slangc").ok().flatten())
}

fn dxil_runtime(host: Host) -> &'static str {
    match host {
        Host::Windows => "dxcompiler.dll",
        Host::Linux => "libdxcompiler.so",
        Host::Mac => "libdxcompiler.dylib",
    }
}

fn shader_status(saved: &Value, host: Host) -> io::Result<Value> {
    let path = shader_binary(saved);
    let runtime = dxil_runtime(host);
    let present = path
        .as_deref()
        .and_then(Path::parent)
        .is_some_and(|directory| directory.join(runtime).is_file());
    let dxil = if present {
        json!({"runtime": runtime, "ready": true})
    } else {
        json!({
            "runtime": runtime,
            "ready": false,
            "reason": if host == Host::Windows {
                "Windows SDK dxcompiler.dll missing; install the C++ workload"
            } else {
                "DXIL needs a DirectXShaderCompiler runtime; this host's pinned \
                 Slang archive does not bundle one and the Windows SDK copy is \
                 only available on Windows"
            }
        })
    };
    let Some(path) = path else {
        return Ok(json!({
            "ready": false,
            "slangc": {"ready": false, "expected": SLANG_VERSION},
            "dxil": dxil,
        }));
    };
    let Ok(output) = command(path.to_string_lossy().as_ref())?
        .arg(version_arg("slangc"))
        .output()
    else {
        return Ok(json!({
            "ready": false,
            "slangc": {
                "ready": false,
                "path": path.display().to_string(),
                "expected": SLANG_VERSION,
            },
            "dxil": dxil,
        }));
    };
    // slangc writes its version to stderr; build-native-ui.mjs combines both
    // streams for the same check.
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .trim()
    .to_owned();
    let ready = output.status.success() && text == SLANG_VERSION;
    Ok(json!({
        "ready": ready,
        "slangc": {
            "ready": ready,
            "path": path.display().to_string(),
            "version": text,
            "expected": SLANG_VERSION,
        },
        "dxil": dxil,
    }))
}

fn pinned_version_ok(tool: PinnedTool, path: &Path) -> bool {
    let Ok(output) = command(path.to_string_lossy().as_ref())
        .and_then(|mut command| command.arg(version_arg(tool.name)).output())
    else {
        return false;
    };
    // slangc reports -version on stderr; every other tool uses stdout.
    let text = if tool.name == "slangc" {
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    } else {
        String::from_utf8_lossy(&output.stdout).into_owned()
    };
    if !output.status.success() {
        return false;
    }
    if tool.name == "slangc" {
        return text.trim() == tool.version;
    }
    let minimum = BASE
        .iter()
        .find(|candidate| candidate.name == tool.name)
        .map(|candidate| candidate.minimum)
        .unwrap_or((0, 0, 0));
    version(&text).is_some_and(|found| found >= minimum)
}

fn locate_binary(directory: &Path, name: &str, depth: usize) -> io::Result<Option<PathBuf>> {
    if depth == 0 || !directory.is_dir() {
        return Ok(None);
    }
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            if let Some(found) = locate_binary(&path, name, depth - 1)? {
                return Ok(Some(found));
            }
        } else if let Some(file) = path.file_name().and_then(|value| value.to_str())
            && (file == name || file == format!("{name}.exe"))
        {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn checksum_matches(path: &Path, expected: &str) -> bool {
    fs::read(path)
        .map(|bytes| format!("{:x}", Sha256::digest(&bytes)) == expected)
        .unwrap_or(false)
}

fn record_pinned(tool: PinnedTool, binary: &Path, saved: &mut Value) -> io::Result<()> {
    let directory = binary
        .parent()
        .ok_or_else(|| io::Error::other("pinned tool has no parent directory"))?;
    let mut paths: Vec<PathBuf> = saved["environment"]["PATH"]
        .as_str()
        .map(|value| env::split_paths(value).collect())
        .unwrap_or_default();
    if !paths.iter().any(|path| path == directory) {
        paths.insert(0, directory.to_path_buf());
    }
    saved["environment"]["PATH"] = json!(
        env::join_paths(&paths)
            .map_err(io::Error::other)?
            .to_string_lossy()
    );
    saved["environment"][tool.env_key] = json!(binary.display().to_string());
    save(saved)
}

/// Downloads a pinned toolchain into the Cove cache, verifies its SHA-256,
/// unpacks it and records the absolute binary path in the setup state. Build
/// scripts read that state, so a plain `cargo build` finds the tool without the
/// developer exporting anything.
fn ensure_pinned(tool: PinnedTool, host: Host, saved: &mut Value) -> io::Result<Value> {
    let existing = env::var_os(tool.env_key)
        .map(PathBuf::from)
        .or_else(|| {
            saved["environment"][tool.env_key]
                .as_str()
                .map(PathBuf::from)
        })
        .filter(|path| path.is_file())
        .or_else(|| find(tool.name).ok().flatten());
    if let Some(path) = existing.filter(|path| pinned_version_ok(tool, path)) {
        record_pinned(tool, &path, saved)?;
        return Ok(json!({
            "name": tool.name,
            "ready": true,
            "path": path.display().to_string(),
            "source": "host",
        }));
    }
    let archive = pinned_archive(tool, host, env::consts::ARCH).ok_or_else(|| {
        io::Error::other(format!(
            "no pinned {} archive for {}-{}",
            tool.name,
            env::consts::OS,
            env::consts::ARCH
        ))
    })?;
    let file = cache_dir()?.join("downloads").join(archive.name);
    // A verified archive already in the cache is reusable: developers behind a
    // restricted network can place the official archive there by hand.
    if !checksum_matches(&file, archive.checksum) {
        download_pinned(&archive, &file)?;
    }
    let bytes = fs::read(&file)?;
    let actual = format!("{:x}", Sha256::digest(&bytes));
    if actual != archive.checksum {
        return Err(io::Error::other(format!(
            "{} checksum mismatch: {actual}",
            archive.name
        )));
    }
    let destination = cache_dir()?
        .join("tools")
        .join(format!("{}-{}", tool.name, tool.version));
    if destination.exists() {
        fs::remove_dir_all(&destination)?;
    }
    fs::create_dir_all(&destination)?;
    if archive.name.ends_with(".zip") {
        checked(
            command("cmake")?
                .args(["-E", "tar", "xf"])
                .arg(&file)
                .current_dir(&destination),
        )?;
    } else {
        checked(
            command("tar")?
                .args(["-xf"])
                .arg(&file)
                .arg("-C")
                .arg(&destination),
        )?;
    }
    let binary = locate_binary(&destination, tool.name, 4)?.ok_or_else(|| {
        io::Error::other(format!("{} not found inside {}", tool.name, archive.name))
    })?;
    record_pinned(tool, &binary, saved)?;
    Ok(json!({
        "name": tool.name,
        "ready": true,
        "path": binary.display().to_string(),
        "source": archive.url,
        "checksum": archive.checksum,
    }))
}

fn find_dxcompiler(directory: &Path, depth: usize) -> io::Result<Option<PathBuf>> {
    if depth == 0 || !directory.is_dir() {
        return Ok(None);
    }
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            if let Some(found) = find_dxcompiler(&path, depth - 1)? {
                return Ok(Some(found));
            }
        } else if path.file_name().and_then(|value| value.to_str()) == Some("dxcompiler.dll")
            && path
                .parent()
                .and_then(Path::file_name)
                .and_then(|value| value.to_str())
                == Some("x64")
        {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

/// Places a DXIL compiler beside `slangc`, which is where Slang looks for it
/// when emitting `-target dxil`. Windows takes it from the Windows SDK; the
/// other hosts use the DirectXShaderCompiler runtime if the pinned Slang
/// archive bundles one. A missing runtime is reported by `shader_status`.
fn install_dxil(host: Host, saved: &Value) -> io::Result<()> {
    let Some(slangc) = saved["environment"]["SLANGC"].as_str().map(PathBuf::from) else {
        return Ok(());
    };
    let directory = slangc
        .parent()
        .ok_or_else(|| io::Error::other("slangc has no parent directory"))?;
    let runtime = dxil_runtime(host);
    if directory.join(runtime).is_file() {
        return Ok(());
    }
    if host == Host::Windows {
        let kits = PathBuf::from(
            env::var_os("ProgramFiles(x86)").unwrap_or_else(|| "C:\\Program Files (x86)".into()),
        )
        .join("Windows Kits/10/bin");
        let compiler = find_dxcompiler(&kits, 5)?.ok_or_else(|| {
            io::Error::other("Windows SDK dxcompiler.dll missing; install the C++ workload")
        })?;
        fs::copy(&compiler, directory.join("dxcompiler.dll"))?;
        let validator = compiler.parent().map(|parent| parent.join("dxil.dll"));
        if let Some(validator) = validator.filter(|path| path.is_file()) {
            fs::copy(validator, directory.join("dxil.dll"))?;
        }
        return Ok(());
    }
    let archive_root = cache_dir()?
        .join("tools")
        .join(format!("slangc-{SLANG_VERSION}"));
    if let Some(library) = locate_binary(&archive_root, runtime, 4)? {
        fs::copy(library, directory.join(runtime))?;
    }
    Ok(())
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
            ("0.17.1", (0, 17, 1)),
            ("2026.19", (2026, 19, 0)),
        ] {
            assert_eq!(version(text), Some(expected));
        }
        assert_eq!(version("not installed"), None);
    }
    #[test]
    fn every_host_has_an_installation_plan_and_mobile_sdk_paths() {
        for host in [Host::Mac, Host::Windows, Host::Linux] {
            let plan = installation_plan(host, true, true, true, &["aarch64-linux-android".into()]);
            assert!(plan["manager"].as_str().expect("manager").len() > 5);
            assert!(plan["sdk"].as_str().expect("SDK").contains("Android"));
            assert!(plan["wasm"] == true);
            assert!(
                plan["pinned"]
                    .as_str()
                    .expect("pinned")
                    .contains(ZIG_VERSION)
            );
            assert!(plan["pinned"].as_str().unwrap().contains(WASI_SDK_VERSION));
        }
    }
    #[test]
    fn zig_is_a_required_tool() {
        let zig = BASE.iter().find(|tool| tool.name == "zig").expect("zig");
        assert_eq!(zig.minimum, (0, 17, 0));
    }
    #[test]
    fn every_host_and_architecture_has_a_verified_archive() {
        for tool in [PINNED_ZIG, PINNED_SLANG, PINNED_WASI_SDK] {
            for host in [Host::Mac, Host::Windows, Host::Linux] {
                for arch in ["aarch64", "x86_64"] {
                    let archive = pinned_archive(tool, host, arch).unwrap_or_else(|| {
                        panic!("missing archive for {} {host:?} {arch}", tool.name)
                    });
                    assert_eq!(archive.checksum.len(), 64);
                    assert!(
                        archive
                            .checksum
                            .bytes()
                            .all(|byte| byte.is_ascii_hexdigit())
                    );
                    assert!(archive.url.ends_with(archive.name));
                    assert!(archive.name.contains(tool.version));
                    if tool.name == "zig" {
                        assert!(archive.fallback.is_none());
                    } else {
                        let fallback = archive.fallback.as_ref().expect("API asset fallback");
                        assert!(fallback.starts_with("https://api.github.com/"));
                        assert!(
                            fallback.contains("/WebAssembly/wasi-sdk/")
                                || fallback.contains("/shader-slang/slang/")
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn unsupported_architectures_do_not_get_a_fabricated_archive() {
        assert!(pinned_archive(PINNED_ZIG, Host::Mac, "riscv64").is_none());
        assert!(pinned_archive(PINNED_SLANG, Host::Linux, "powerpc64").is_none());
        assert!(pinned_archive(PINNED_WASI_SDK, Host::Mac, "riscv64").is_none());
    }
    #[test]
    fn version_arguments_match_each_compiler() {
        assert_eq!(version_arg("zig"), "version");
        assert_eq!(version_arg("slangc"), "-version");
        assert_eq!(version_arg("cmake"), "--version");
    }
    #[test]
    fn checksum_matches_detects_a_corrupt_archive() {
        let path = std::env::temp_dir().join(format!("cove-hash-{}", std::process::id()));
        fs::write(&path, b"cove").expect("write archive");
        let digest = format!("{:x}", Sha256::digest(b"cove"));
        assert!(checksum_matches(&path, &digest));
        assert!(!checksum_matches(&path, &"0".repeat(64)));
        fs::remove_file(&path).expect("clean archive");
    }
    #[cfg(unix)]
    #[test]
    fn shader_status_requires_the_exact_pinned_slang_version() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("cove-slangc-{}", std::process::id()));
        fs::create_dir_all(&root).expect("create staging");
        let script = root.join("slangc");
        for (version, expected) in [("2026.20", false), ("2026.19", true)] {
            // The real slangc prints -version on stderr.
            fs::write(&script, format!("#!/bin/sh\necho {version} >&2\n"))
                .expect("write fake slangc");
            fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("chmod");
            let saved = json!({"environment": {"SLANGC": script.display().to_string()}});
            let status = shader_status(&saved, Host::Linux).expect("inspect slangc");
            assert_eq!(status["ready"], expected, "version {version}");
            assert_eq!(status["slangc"]["version"].as_str(), Some(version));
        }
        fs::remove_dir_all(&root).expect("clean staging");
    }
    #[test]
    fn locate_binary_finds_the_executable_inside_an_archive_root() {
        let root = std::env::temp_dir().join(format!("cove-locate-{}", std::process::id()));
        let nested = root.join("slang-2026.19/bin");
        fs::create_dir_all(&nested).expect("create staging directory");
        fs::write(nested.join("slangc"), b"binary").expect("write fake binary");
        assert_eq!(
            locate_binary(&root, "slangc", 4).expect("scan staging"),
            Some(nested.join("slangc"))
        );
        assert_eq!(
            locate_binary(&root, "slangc.exe", 4).expect("scan staging"),
            None
        );
        fs::remove_dir_all(&root).expect("clean staging");
    }
}
