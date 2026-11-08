use super::*;
use sha2::{Digest, Sha256};

pub(super) fn validate(host: Host, targets: &[String]) -> io::Result<()> {
    for target in targets {
        let supported = target.contains("android")
            || match host {
                Host::Mac => target.contains("apple"),
                Host::Windows => target.contains("windows"),
                Host::Linux => target.contains("linux") && !target.contains("android"),
            };
        if !supported {
            return Err(io::Error::other(format!(
                "SDK setup for {target} cannot run on {host:?}; use the target platform's host (Android is available on all three hosts)"
            )));
        }
    }
    Ok(())
}

pub(super) fn inspect_compiler(host: Host) -> io::Result<bool> {
    if host == Host::Mac
        && !Command::new("xcode-select")
            .arg("-p")
            .output()?
            .status
            .success()
    {
        return Ok(false);
    }
    if host == Host::Windows {
        return Ok(windows_environment().is_ok());
    }
    for name in ["cc", "clang", "gcc"] {
        if let Some(path) = find(name)?
            && command(path.to_string_lossy().as_ref())?
                .arg("--version")
                .output()?
                .status
                .success()
        {
            return Ok(true);
        }
    }
    Ok(false)
}
pub(super) fn check(host: Host, targets: &[String]) -> io::Result<bool> {
    for target in targets {
        if target.contains("android") {
            let Some(ndk) = setting("ANDROID_NDK_HOME")?.or(setting("ANDROID_NDK_ROOT")?) else {
                return Ok(false);
            };
            if !Path::new(&ndk).join("source.properties").is_file() {
                return Ok(false);
            }
        } else if target.contains("apple") {
            if host != Host::Mac {
                return Ok(false);
            }
            let sdk = if target.contains("ios") {
                if target.ends_with("-sim") || target.starts_with("x86_64") {
                    "iphonesimulator"
                } else {
                    "iphoneos"
                }
            } else {
                "macosx"
            };
            let output = command("xcrun")?
                .args(["--sdk", sdk, "--show-sdk-version"])
                .output()?;
            if !output.status.success()
                || version(&String::from_utf8_lossy(&output.stdout)).is_none_or(|v| v < (26, 0, 0))
            {
                return Ok(false);
            }
        } else if target.contains("windows") {
            if host != Host::Windows || windows_environment().is_err() {
                return Ok(false);
            }
        } else if host == Host::Linux {
            if let Ok(mut pc) = command("pkg-config") {
                if !pc.args(["--exists", "alsa"]).status()?.success() {
                    return Ok(false);
                }
            } else {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
pub(super) fn prepare(host: Host, targets: &[String], saved: &mut Value) -> io::Result<()> {
    if host == Host::Mac {
        install_apple_clt()?;
    }
    if host == Host::Windows {
        if windows_environment().is_err() {
            install_windows_sdk()?;
        }
        let vars = windows_environment()?;
        for (key, value) in vars {
            if key.eq_ignore_ascii_case("PATH") {
                merge_path(saved, &value)?;
            } else {
                saved["environment"][key] = json!(value);
            }
        }
    }
    if targets.iter().any(|target| target.contains("ios")) {
        if host != Host::Mac {
            return Err(io::Error::other("iOS SDKs require a macOS host"));
        }
        let available = command("xcrun")?
            .args(["--sdk", "iphoneos", "--show-sdk-version"])
            .output()?;
        if !available.status.success()
            || version(&String::from_utf8_lossy(&available.stdout)).is_none_or(|v| v < (26, 0, 0))
        {
            if find("xcodes")?.is_none() {
                checked(command("brew")?.args(["install", "xcodesorg/made/xcodes"]))?;
            }
            // Apple authentication and privileged installation are handled by xcodes.
            checked(command("xcodes")?.args(["install", "--latest"]))?;
            checked(command("xcodes")?.args(["select", "--latest"]))?;
            checked(privileged("xcodebuild")?.args(["-license", "accept"]))?;
            checked(privileged("xcodebuild")?.arg("-runFirstLaunch"))?;
        }
        checked(command("xcodebuild")?.args(["-downloadPlatform", "iOS"]))?;
    }
    if targets.iter().any(|target| target.contains("android")) {
        prepare_android(host, saved)?;
    }
    let host_triple = host_triple()?;
    for target in targets {
        if target != &host_triple {
            checked(command("rustup")?.args(["target", "add", target]))?;
        }
    }
    Ok(())
}

/// Keeps the pinned toolchain directories recorded by setup when the MSVC
/// environment contributes its own PATH. Replacing it made `zig` and `slangc`
/// undiscoverable immediately after `cargo cove setup` on Windows.
fn merge_path(saved: &mut Value, sdk_path: &str) -> io::Result<()> {
    let mut paths: Vec<PathBuf> = saved["environment"]["PATH"]
        .as_str()
        .map(|value| env::split_paths(value).collect())
        .unwrap_or_default();
    for entry in env::split_paths(sdk_path) {
        if !paths.contains(&entry) {
            paths.push(entry);
        }
    }
    saved["environment"]["PATH"] = json!(
        env::join_paths(&paths)
            .map_err(io::Error::other)?
            .to_string_lossy()
    );
    Ok(())
}

fn install_windows_sdk() -> io::Result<()> {
    let installer_root = PathBuf::from(
        env::var_os("ProgramFiles(x86)").unwrap_or_else(|| "C:/Program Files (x86)".into()),
    )
    .join("Microsoft Visual Studio/Installer");
    let existing = Command::new(installer_root.join("vswhere.exe"))
        .args(["-latest", "-products", "*", "-property", "installationPath"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .filter(|p| !p.is_empty());
    const COMPONENTS: &[&str] = &[
        "--add",
        "Microsoft.VisualStudio.Workload.VCTools",
        "--add",
        "Microsoft.VisualStudio.Component.VC.Tools.ARM64",
        "--includeRecommended",
    ];
    if let Some(path) = existing {
        // `--wait` is bootstrapper-only; VS Installer 4.11+ rejects it on `setup.exe modify`.
        checked(
            Command::new(installer_root.join("setup.exe"))
                .args(["modify", "--quiet", "--norestart", "--installPath"])
                .arg(path)
                .args(COMPONENTS),
        )
    } else {
        winget_install(
            "Microsoft.VisualStudio.2022.BuildTools",
            Some(
                "--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --add Microsoft.VisualStudio.Component.VC.Tools.ARM64 --includeRecommended",
            ),
        )
    }
}
pub(super) fn install_apple_clt() -> io::Result<()> {
    if Command::new("xcrun")
        .args(["--sdk", "macosx", "--show-sdk-version"])
        .output()
        .ok()
        .is_some_and(|o| {
            o.status.success()
                && version(&String::from_utf8_lossy(&o.stdout)).is_some_and(|v| v >= (26, 0, 0))
        })
    {
        return Ok(());
    }
    let marker = Path::new("/tmp/.com.apple.dt.CommandLineTools.installondemand.in-progress");
    fs::write(marker, b"")?;
    let listing = checked_output(Command::new("softwareupdate").arg("--list"));
    let _ = fs::remove_file(marker);
    let listing = listing?;
    let label=listing.lines().filter_map(|line|line.trim().strip_prefix("* Label: ")).rfind(|label|label.contains("Command Line Tools"))
        .ok_or_else(||io::Error::other("Apple softwareupdate did not offer Command Line Tools; install an Xcode version supporting SDK 26+"))?;
    checked(privileged("softwareupdate")?.args(["--install", label, "--agree-to-license"]))?;
    checked(privileged("xcode-select")?.args(["--switch", "/Library/Developer/CommandLineTools"]))
}
fn windows_environment() -> io::Result<Vec<(String, String)>> {
    let root = env::var_os("ProgramFiles(x86)").unwrap_or_else(|| "C:/Program Files (x86)".into());
    let vswhere = PathBuf::from(root).join("Microsoft Visual Studio/Installer/vswhere.exe");
    let directory = checked_output(Command::new(vswhere).args([
        "-latest",
        "-products",
        "*",
        "-requires",
        "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
        "-property",
        "installationPath",
    ]))?;
    let directory = directory.trim();
    if directory.is_empty() {
        return Err(io::Error::other(
            "Visual Studio C++ Build Tools are missing",
        ));
    }
    let architecture = if env::consts::ARCH == "aarch64" {
        "arm64"
    } else {
        "amd64"
    };
    let script = Path::new(directory).join("VC/Auxiliary/Build/vcvarsall.bat");
    let mut shell = Command::new("cmd");
    shell.args(["/d", "/s", "/c"]);
    // `cmd /s /c` strips one outer pair of quotes and reads the rest verbatim.
    // `Command::arg` would escape the inner quotes as `\"`, which cmd does not
    // understand, so the script path never ran and setup fell back to the
    // Visual Studio installer even with Build Tools present.
    let line = format!("\"\"{}\" {architecture} >nul && set\"", script.display());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        shell.raw_arg(line);
    }
    #[cfg(not(windows))]
    shell.arg(line);
    let output = checked_output(&mut shell)?;
    let mut values: Vec<(String, String)> = Vec::new();
    for line in output.lines() {
        if let Some((key, value)) = line.split_once('=')
            && [
                "PATH",
                "INCLUDE",
                "LIB",
                "LIBPATH",
                "WINDOWSSDKDIR",
                "WINDOWSSDKVERSION",
                "VCTOOLSINSTALLDIR",
                "VCINSTALLDIR",
            ]
            .contains(&key.to_ascii_uppercase().as_str())
        {
            values.push((
                if key.eq_ignore_ascii_case("path") {
                    "PATH".into()
                } else {
                    key.into()
                },
                value.into(),
            ));
        }
    }
    if !values
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("WindowsSdkDir"))
    {
        return Err(io::Error::other(
            "Visual Studio has no Windows SDK; install the C++ workload with recommended components",
        ));
    }
    Ok(values)
}

// Published SHA-256 values: https://developer.android.com/studio#command-tools
// Pinning an archive keeps SDK bootstrap reproducible even when 'latest' moves.
fn android_archive(host: Host, architecture: &str) -> (&'static str, &'static str) {
    match host {
        Host::Windows => (
            "commandlinetools-win-15859902_latest.zip",
            "90ae805d20434428bffcb699c290860f19bb5f66a67e6b330067e3de801fb04a",
        ),
        Host::Linux => (
            "commandlinetools-linux-15859902_latest.zip",
            "4e4c464f145a7512b57d088ac6c278c03c9eea610886b35a5e0804e74eedf583",
        ),
        Host::Mac if architecture == "aarch64" => (
            "commandlinetools-mac_arm64-15859902_latest.zip",
            "835b62a26162b229b441d1f6d4680383815a270809eb33522c0d480fa5002c4e",
        ),
        Host::Mac => (
            "commandlinetools-mac_x86_64-15859902_latest.zip",
            "c5a6378ab5cf7e0d5701921405115befff13e9ff7417fb588389338f8bd050f3",
        ),
    }
}
fn prepare_android(host: Host, saved: &mut Value) -> io::Result<()> {
    const NDK: &str = "30.0.16248370";
    let root = setting("ANDROID_HOME")?
        .or(setting("ANDROID_SDK_ROOT")?)
        .map(PathBuf::from)
        .unwrap_or(cache_dir()?.join("android-sdk"));
    let manager = root.join(if host == Host::Windows {
        "cmdline-tools/latest/bin/sdkmanager.bat"
    } else {
        "cmdline-tools/latest/bin/sdkmanager"
    });
    if !manager.is_file() {
        let (filename, checksum) = android_archive(host, env::consts::ARCH);
        let archive = cache_dir()?.join("downloads").join(filename);
        download(
            &format!("https://dl.google.com/android/repository/{filename}"),
            &archive,
        )?;
        let bytes = fs::read(&archive)?;
        let actual = format!("{:x}", Sha256::digest(&bytes));
        if actual != checksum {
            return Err(io::Error::other(format!(
                "Android command-line tools checksum mismatch: {actual}"
            )));
        }
        let staging = root.join("cmdline-tools/bootstrap");
        fs::create_dir_all(&staging)?;
        checked(
            command("cmake")?
                .args(["-E", "tar", "xf"])
                .arg(&archive)
                .current_dir(&staging),
        )?;
        let destination = root.join("cmdline-tools/latest");
        if destination.exists() {
            fs::remove_dir_all(&destination)?;
        }
        fs::rename(staging.join("cmdline-tools"), destination)?;
    }
    let mut java = java_home(host)?;
    if java.is_none() {
        match host {
            Host::Mac => checked(command("brew")?.args(["install", "openjdk@21"]))?,
            Host::Windows => winget_install("Microsoft.OpenJDK.21", None)?,
            Host::Linux => {
                let (manager, _) = super::linux_packages(false)?;
                let package = match manager {
                    "apt-get" => "openjdk-21-jdk-headless",
                    "dnf" => "java-21-openjdk-devel",
                    "pacman" => "jdk21-openjdk",
                    "zypper" => "java-21-openjdk-devel",
                    _ => "openjdk21-jdk",
                };
                let mut c = privileged(manager)?;
                c.args(match manager {
                    "apt-get" | "dnf" => vec!["install", "-y"],
                    "pacman" => vec!["-S", "--needed", "--noconfirm"],
                    "zypper" => vec!["--non-interactive", "install"],
                    _ => vec!["add"],
                });
                checked(c.arg(package))?;
            }
        }
        java = java_home(host)?;
    }
    let java = java.ok_or_else(|| {
        io::Error::other("JDK 21 installation did not produce a usable Java home")
    })?;
    let mut licenses = command(manager.to_string_lossy().as_ref())?;
    licenses
        .env("JAVA_HOME", &java)
        .arg(format!("--sdk_root={}", root.display()))
        .arg("--licenses")
        .stdin(Stdio::inherit());
    checked(&mut licenses)?;
    checked(
        command(manager.to_string_lossy().as_ref())?
            .env("JAVA_HOME", &java)
            .arg(format!("--sdk_root={}", root.display()))
            .args([
                "platform-tools",
                "platforms;android-36",
                "build-tools;36.0.0",
                &format!("ndk;{NDK}"),
            ]),
    )?;
    for (key, value) in [
        ("ANDROID_HOME", root.clone()),
        ("ANDROID_SDK_ROOT", root.clone()),
        ("ANDROID_NDK_HOME", root.join("ndk").join(NDK)),
        ("JAVA_HOME", java),
    ] {
        saved["environment"][key] = json!(value);
    }
    Ok(())
}

fn java_home(host: Host) -> io::Result<Option<PathBuf>> {
    let mut candidates = Vec::new();
    if let Some(home) = setting("JAVA_HOME")? {
        candidates.push(PathBuf::from(home));
    }
    if host == Host::Mac {
        if let Ok(result) = Command::new("/usr/libexec/java_home")
            .args(["-v", "21"])
            .output()
            && result.status.success()
        {
            candidates.push(PathBuf::from(
                String::from_utf8_lossy(&result.stdout).trim(),
            ));
        }
        for root in ["/opt/homebrew", "/usr/local"] {
            candidates
                .push(Path::new(root).join("opt/openjdk@21/libexec/openjdk.jdk/Contents/Home"));
        }
    }
    let directories = if host == Host::Windows {
        vec![
            PathBuf::from(env::var_os("ProgramFiles").unwrap_or_else(|| "C:/Program Files".into()))
                .join("Microsoft"),
        ]
    } else {
        vec![PathBuf::from("/usr/lib/jvm")]
    };
    for directory in directories {
        if let Ok(entries) = fs::read_dir(directory) {
            for entry in entries {
                let path = entry?.path();
                if path.is_dir() {
                    candidates.push(path);
                }
            }
        }
    }
    if let Some(java) = find("java")?
        && let Ok(path) = java.canonicalize()
        && let Some(home) = path.parent().and_then(Path::parent)
    {
        candidates.push(home.to_path_buf());
    }
    for home in candidates {
        let executable = home.join(if host == Host::Windows {
            "bin/java.exe"
        } else {
            "bin/java"
        });
        if executable.is_file() {
            let output = Command::new(executable).arg("-version").output()?;
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            if output.status.success() && version(&text).is_some_and(|v| v >= (21, 0, 0)) {
                return Ok(Some(home));
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sdk_archives_are_pinned_for_all_hosts_and_apple_architectures() {
        for (host, arch) in [
            (Host::Mac, "aarch64"),
            (Host::Mac, "x86_64"),
            (Host::Windows, "x86_64"),
            (Host::Linux, "x86_64"),
        ] {
            let (file, hash) = android_archive(host, arch);
            assert!(file.ends_with(".zip"));
            assert_eq!(hash.len(), 64);
            assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
        }
        assert_ne!(
            android_archive(Host::Mac, "aarch64"),
            android_archive(Host::Mac, "x86_64")
        );
    }
    #[test]
    fn msvc_path_keeps_pinned_tool_directories() {
        let pinned = env::join_paths(["/Cove/zig", "/Cove/slangc"])
            .expect("join pinned paths")
            .to_string_lossy()
            .into_owned();
        let sdk = env::join_paths(["/VC/bin", "/Cove/zig"])
            .expect("join SDK paths")
            .to_string_lossy()
            .into_owned();
        let mut saved = json!({"environment": {"PATH": pinned}});
        merge_path(&mut saved, &sdk).expect("merge");
        let paths: Vec<_> =
            env::split_paths(saved["environment"]["PATH"].as_str().expect("merged PATH")).collect();
        assert_eq!(
            paths,
            [
                PathBuf::from("/Cove/zig"),
                PathBuf::from("/Cove/slangc"),
                PathBuf::from("/VC/bin"),
            ]
        );
    }
}
