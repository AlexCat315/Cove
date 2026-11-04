//! Build-script support for the native libraries Cove compiles from the pinned
//! git submodules in `third-party/`.
//!
//! Every library is configured with CMake into the calling crate's `OUT_DIR`
//! and linked statically; nothing is looked up in a shared prebuilt directory.
//! [`cmake`] applies the per-target settings the crates share: the Release
//! profile, static position-independent archives, the Apple deployment target,
//! the Android NDK toolchain, the wasi-sdk toolchain and the MSVC C runtime.

use std::env;
use std::path::{Path, PathBuf};

pub mod gstreamer;
pub mod media_notices;

/// Minimum Apple OS version for every native archive.
pub const APPLE_DEPLOYMENT_TARGET: &str = "26.0";
/// Android API level the NDK toolchain targets.
pub const ANDROID_PLATFORM: &str = "26";

/// The workspace root, two levels above the calling crate's manifest.
pub fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(Path::parent)
        .expect("crates live two levels below the workspace root")
        .to_path_buf()
}

/// The checkout of `third-party/<name>`, which must contain `marker`.
pub fn third_party(name: &str, marker: &str) -> PathBuf {
    let path = workspace_root().join("third-party").join(name);
    assert!(
        path.join(marker).is_file(),
        "{} is missing; run `git submodule update --init third-party/{name}`",
        path.join(marker).display()
    );
    println!("cargo:rerun-if-changed={}", path.join(".git").display());
    path
}

fn target() -> String {
    env::var("TARGET").expect("TARGET")
}

fn target_os() -> String {
    env::var("CARGO_CFG_TARGET_OS").expect("CARGO_CFG_TARGET_OS")
}

fn is_msvc() -> bool {
    target().contains("msvc")
}

fn uses_static_crt() -> bool {
    env::var("CARGO_CFG_TARGET_FEATURE")
        .unwrap_or_default()
        .split(',')
        .any(|feature| feature == "crt-static")
}

/// The Android NDK named by `ANDROID_NDK_HOME` or `ANDROID_NDK_ROOT`.
pub fn android_ndk() -> PathBuf {
    env::var_os("ANDROID_NDK_HOME")
        .or_else(|| env::var_os("ANDROID_NDK_ROOT"))
        .map(PathBuf::from)
        .expect("Android builds require ANDROID_NDK_HOME or ANDROID_NDK_ROOT")
}

/// The wasi-sdk named by `COVE_WASI_SDK_PATH`.
pub fn wasi_sdk() -> PathBuf {
    env::var_os("COVE_WASI_SDK_PATH")
        .map(PathBuf::from)
        .expect("Wasm builds require COVE_WASI_SDK_PATH")
}

fn apple_arch() -> &'static str {
    if target().starts_with("aarch64") {
        "arm64"
    } else {
        "x86_64"
    }
}

/// A CMake build of `source` into `OUT_DIR/<name>`, configured for the target.
pub fn cmake(name: &str, source: &Path) -> cmake::Config {
    for variable in ["ANDROID_NDK_HOME", "ANDROID_NDK_ROOT", "COVE_WASI_SDK_PATH"] {
        println!("cargo:rerun-if-env-changed={variable}");
    }
    let target = target();
    let os = target_os();
    let mut config = cmake::Config::new(source);
    config
        .out_dir(PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join(name))
        .profile("Release")
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("CMAKE_POSITION_INDEPENDENT_CODE", "ON")
        .define("CMAKE_INSTALL_LIBDIR", "lib")
        .define("CMAKE_INSTALL_INCLUDEDIR", "include");
    match os.as_str() {
        "macos" => {
            config.define("CMAKE_OSX_DEPLOYMENT_TARGET", APPLE_DEPLOYMENT_TARGET);
        }
        "ios" => {
            let simulator = target.ends_with("-sim") || target.starts_with("x86_64");
            config
                .define("CMAKE_SYSTEM_NAME", "iOS")
                .define("CMAKE_OSX_DEPLOYMENT_TARGET", APPLE_DEPLOYMENT_TARGET)
                .define("CMAKE_OSX_ARCHITECTURES", apple_arch())
                .define(
                    "CMAKE_OSX_SYSROOT",
                    if simulator {
                        "iphonesimulator"
                    } else {
                        "iphoneos"
                    },
                );
        }
        "android" => {
            config
                .define(
                    "CMAKE_TOOLCHAIN_FILE",
                    android_ndk().join("build/cmake/android.toolchain.cmake"),
                )
                .define(
                    "ANDROID_ABI",
                    if target.starts_with("aarch64") {
                        "arm64-v8a"
                    } else {
                        "x86_64"
                    },
                )
                .define("ANDROID_PLATFORM", ANDROID_PLATFORM)
                .define("ANDROID_STL", "c++_static");
        }
        _ if target.starts_with("wasm") => {
            config.define(
                "CMAKE_TOOLCHAIN_FILE",
                wasi_sdk().join("share/cmake/wasi-sdk.cmake"),
            );
        }
        _ => {}
    }
    if is_msvc() {
        let crt = uses_static_crt();
        config.static_crt(crt).define(
            "CMAKE_MSVC_RUNTIME_LIBRARY",
            if crt {
                "MultiThreaded"
            } else {
                "MultiThreadedDLL"
            },
        );
    }
    config
}

/// Points a `cc` build at the wasi-sdk compiler when targeting Wasm.
pub fn configure_cc(build: &mut cc::Build, cpp: bool) {
    if target().starts_with("wasm") {
        let sdk = wasi_sdk();
        build
            .compiler(sdk.join(if cpp { "bin/clang++" } else { "bin/clang" }))
            .flag("--target=wasm32-wasi")
            .flag(format!(
                "--sysroot={}",
                sdk.join("share/wasi-sysroot").display()
            ));
    }
}

/// Links the static archive `name` from `dir`, accepting the spellings CMake
/// produces on each toolchain (`libname.a`, `name.lib`, `libname.lib`).
pub fn link_static(dir: &Path, name: &str) {
    let candidates = [
        (format!("lib{name}.a"), name.to_owned()),
        (format!("{name}.lib"), name.to_owned()),
        (format!("lib{name}.lib"), format!("lib{name}")),
        (format!("{name}-static.lib"), format!("{name}-static")),
    ];
    let (_, link) = candidates
        .iter()
        .find(|(file, _)| dir.join(file).is_file())
        .unwrap_or_else(|| panic!("no static archive for {name} in {}", dir.display()));
    println!("cargo:rustc-link-search=native={}", dir.display());
    println!("cargo:rustc-link-lib=static={link}");
}

/// Whether `dir` holds a static archive for `name`.
pub fn has_static(dir: &Path, name: &str) -> bool {
    [
        format!("lib{name}.a"),
        format!("{name}.lib"),
        format!("lib{name}.lib"),
    ]
    .iter()
    .any(|file| dir.join(file).is_file())
}

/// Links the C++ runtime the static C++ archives need.
pub fn link_cxx_runtime() {
    let target = target();
    match target_os().as_str() {
        "macos" | "ios" => println!("cargo:rustc-link-lib=c++"),
        // The APK ships libc++_shared.so (Shell links it too). The NDK driver's
        // default search finds it after the API-level libc.so; an explicit
        // unversioned sysroot path would select the static libc.a instead.
        "android" => println!("cargo:rustc-link-lib=c++_shared"),
        _ if is_msvc() || target.starts_with("wasm") => {}
        _ => println!("cargo:rustc-link-lib=stdc++"),
    }
}
