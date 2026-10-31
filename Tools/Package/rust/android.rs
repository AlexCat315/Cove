//! Android: the Rust library as `libCoveAndroidApp.so`, staged Gradle inputs,
//! APK assembly and ADB deployment.

use crate::process::{self, cargo};
use crate::project::Project;
use crate::{Mode, Result, Target};
use std::path::{Path, PathBuf};
use std::process::Command;

const TARGET: &str = "aarch64-linux-android";
const API_LEVEL: u32 = 28;
const ACTIVITY: &str = "com.cove.probe.MainActivity";

fn home() -> PathBuf {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .unwrap_or_default()
}

fn sdk() -> PathBuf {
    std::env::var_os("ANDROID_HOME")
        .or_else(|| std::env::var_os("ANDROID_SDK_ROOT"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if cfg!(target_os = "macos") {
                home().join("Library/Android/sdk")
            } else if cfg!(windows) {
                home().join("AppData/Local/Android/Sdk")
            } else {
                home().join("Android/Sdk")
            }
        })
}

/// `ANDROID_NDK_HOME`, `ANDROID_NDK_ROOT`, then the newest SDK-managed r30.
fn ndk() -> Result<PathBuf> {
    let ndk = match std::env::var_os("ANDROID_NDK_HOME")
        .or_else(|| std::env::var_os("ANDROID_NDK_ROOT"))
    {
        Some(path) => PathBuf::from(path),
        None => {
            let mut installed: Vec<PathBuf> = std::fs::read_dir(sdk().join("ndk"))
                .map_err(|_| "no NDK found; set ANDROID_NDK_HOME to an r30 NDK")?
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .filter(|path| {
                    path.file_name()
                        .is_some_and(|name| name.to_string_lossy().starts_with("30."))
                })
                .collect();
            installed.sort();
            installed
                .pop()
                .ok_or("no r30 NDK under the Android SDK; set ANDROID_NDK_HOME")?
        }
    };
    let properties = std::fs::read_to_string(ndk.join("source.properties"))
        .map_err(|error| format!("{}: {error}", ndk.display()))?;
    if !properties.lines().any(|line| {
        line.split_once('=').is_some_and(|(key, value)| {
            key.trim() == "Pkg.Revision" && value.trim().starts_with("30.")
        })
    }) {
        return Err(format!("{} is not an r30 NDK", ndk.display()));
    }
    Ok(ndk)
}

/// The single LLVM host toolchain inside the NDK.
fn llvm(ndk: &Path) -> Result<PathBuf> {
    let prebuilt = ndk.join("toolchains/llvm/prebuilt");
    let mut hosts: Vec<PathBuf> = std::fs::read_dir(&prebuilt)
        .map_err(|error| format!("{}: {error}", prebuilt.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_dir())
        .collect();
    match hosts.len() {
        1 => Ok(hosts.remove(0)),
        _ => Err(format!(
            "expected one host toolchain in {}",
            prebuilt.display()
        )),
    }
}

/// Builds the library and stages `lib/` and `assets/` for Gradle; writes
/// `Platforms/Android/cove.properties`. Returns the staged mode directory.
pub fn prepare(project: &Project, mode: Mode) -> Result<PathBuf> {
    let platform = project.platforms("Android");
    if !platform.join("settings.gradle.kts").is_file() {
        return Err(format!(
            "{} is missing; copy it from Templates/app",
            platform.display()
        ));
    }
    let ndk = ndk()?;
    let llvm = llvm(&ndk)?;
    let bin = llvm.join("bin");
    let (clang, ar) = if cfg!(windows) {
        (format!("{TARGET}{API_LEVEL}-clang.cmd"), "llvm-ar.exe")
    } else {
        (format!("{TARGET}{API_LEVEL}-clang"), "llvm-ar")
    };
    let mut command = cargo();
    command
        .current_dir(&project.root)
        .env("ANDROID_NDK_HOME", &ndk)
        .env("CC_aarch64_linux_android", bin.join(&clang))
        .env(
            "CXX_aarch64_linux_android",
            bin.join(clang.replace("clang", "clang++")),
        )
        .env("AR_aarch64_linux_android", bin.join(ar))
        .env(
            "CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER",
            bin.join(&clang),
        )
        .args([
            "rustc",
            "--package",
            &project.package,
            "--lib",
            "--crate-type",
            "cdylib",
            "--target",
            TARGET,
        ]);
    if mode.optimized() {
        command.arg("--release");
    }
    process::run(&mut command)?;

    let generated = project.target_dir.join("cove/android/generated");
    let staged = generated.join(mode.name());
    process::remove_dir(&staged)?;
    let libraries = staged.join("lib/arm64-v8a");
    process::copy(
        &project
            .artifacts(mode, Some(TARGET))
            .join(format!("lib{}.so", project.lib()?)),
        &libraries.join("libCoveAndroidApp.so"),
    )?;
    process::copy(
        &llvm.join("sysroot/usr/lib/aarch64-linux-android/libc++_shared.so"),
        &libraries.join("libc++_shared.so"),
    )?;
    // cove-http loads OpenSSL at run time; Android ships none, so the app
    // bundles an arm64 build or reports https as unavailable.
    match std::env::var_os("COVE_OPENSSL_ANDROID_DIR") {
        Some(directory) => {
            for library in ["libssl.so", "libcrypto.so"] {
                process::copy(
                    &Path::new(&directory).join(library),
                    &libraries.join(library),
                )?;
            }
        }
        None => eprintln!("COVE_OPENSSL_ANDROID_DIR is not set: https is unavailable in this APK"),
    }
    let assets = staged.join("assets");
    process::write(
        &assets.join("launch.json"),
        r#"{"mode":"application","component":"","project":null}"#,
    )?;
    for font in ["NotoEmoji.ttf", "OFL-Emoji.txt"] {
        process::copy(
            &project.cove.join("Text/Fonts").join(font),
            &assets.join("Fonts").join(font),
        )?;
    }
    project.stage_resources(&assets)?;

    let properties = [
        (
            "runtime",
            project
                .cove
                .join("Hosts/Android/Runtime")
                .display()
                .to_string(),
        ),
        ("generated", generated.display().to_string()),
        (
            "gradle",
            project
                .target_dir
                .join("cove/android/gradle")
                .display()
                .to_string(),
        ),
        ("package", project.package.clone()),
        ("id", project.id.clone()),
        ("name", project.name.clone()),
        ("version", project.version.clone()),
        ("build", project.build.to_string()),
    ];
    let mut text = String::from("# Written by cargo xtask android-prepare.\n");
    for (key, value) in properties {
        text.push_str(&format!("{key}={}\n", escape(&value)));
    }
    process::write(&platform.join("cove.properties"), text)?;
    let local = platform.join("local.properties");
    if !local.exists() && sdk().is_dir() {
        process::write(
            &local,
            format!("sdk.dir={}\n", escape(&sdk().display().to_string())),
        )?;
    }
    Ok(staged)
}

/// `.properties` values are ISO-8859-1 with backslash escapes.
fn escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            ' '..='~' => escaped.push(character),
            _ => {
                let mut units = [0u16; 2];
                for unit in character.encode_utf16(&mut units) {
                    escaped.push_str(&format!("\\u{unit:04X}"));
                }
            }
        }
    }
    escaped
}

pub fn package(project: &Project, target: &Target, output: &Path) -> Result<PathBuf> {
    let mode = target.mode();
    prepare(project, mode)?;
    let platform = project.platforms("Android");
    let mut gradle = Command::new(platform.join(if cfg!(windows) {
        "gradlew.bat"
    } else {
        "gradlew"
    }));
    let task = format!(
        "assemble{}{}",
        mode.name()[..1].to_uppercase(),
        &mode.name()[1..]
    );
    gradle
        .current_dir(&platform)
        .env("ANDROID_HOME", sdk())
        .args([task.as_str(), "-PcovePrepared=1"]);
    if let (Some(keystore), Some(alias), Some(store), Some(key)) = (
        &target.keystore,
        &target.key_alias,
        &target.store_password_env,
        &target.key_password_env,
    ) {
        gradle
            .arg(format!("-PcoveKeystore={}", keystore.display()))
            .arg(format!("-PcoveKeyAlias={alias}"))
            .arg(format!("-PcoveStorePasswordEnv={store}"))
            .arg(format!("-PcoveKeyPasswordEnv={key}"));
    }
    process::run(&mut gradle)?;
    let apk = project
        .target_dir
        .join("cove/android/gradle/app/outputs/apk")
        .join(mode.name())
        .join(format!("app-{}.apk", mode.name()));
    let product = output.join(format!("{}-{}.apk", project.package, mode.name()));
    process::copy(&apk, &product)?;
    Ok(product)
}

fn adb(device: Option<&str>) -> Command {
    let mut command = Command::new(sdk().join("platform-tools").join(if cfg!(windows) {
        "adb.exe"
    } else {
        "adb"
    }));
    if let Some(serial) = device {
        command.args(["-s", serial]);
    }
    command
}

pub fn launch(project: &Project, apk: &Path, device: Option<&str>) -> Result<()> {
    process::run(adb(device).args(["install", "-r"]).arg(apk))?;
    process::run(adb(device).args([
        "shell",
        "am",
        "start",
        "-n",
        &format!("{}/{ACTIVITY}", project.id),
    ]))
}

pub fn devices() -> Result<()> {
    process::run(adb(None).args(["devices", "-l"]))
}

#[cfg(test)]
mod tests {
    use super::escape;

    #[test]
    fn properties_escape_backslashes_and_non_ascii() {
        assert_eq!(escape(r"C:\Cove"), r"C:\\Cove");
        assert_eq!(escape("你好"), "\\u4F60\\u597D");
        assert_eq!(escape("A b=c"), "A b=c");
    }
}
