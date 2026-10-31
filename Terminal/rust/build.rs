//! Links libghostty-vt for `cove-terminal`.
//!
//! The emulator is Ghostty's VT library, vendored as a git submodule under
//! `third-party/ghostty` and shipped as a Zig-built static archive. A prebuilt
//! archive in `vendor/GhosttyVt.artifactbundle` is used when it exists for the
//! target; otherwise the archive is produced with `zig build -Demit-lib-vt`.
//! Both paths are required: the bundle only ever holds host-architecture
//! artefacts, while Linux and Windows builds are produced from source.

use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

/// Artifact bundle directory for a Rust target triple, mirroring
/// `third-party/cmake/BuildGhosttyVt.cmake`.
fn variant(target: &str) -> Option<String> {
    let arch = if target.starts_with("aarch64") {
        "arm64"
    } else if target.starts_with("x86_64") {
        "x86_64"
    } else {
        return None;
    };
    if target.contains("apple-darwin") {
        Some(format!("macos-{arch}"))
    } else if target.contains("windows") {
        Some(format!("windows-{arch}"))
    } else if target.contains("linux") {
        Some(format!(
            "linux-{}",
            if arch == "arm64" { "aarch64" } else { arch }
        ))
    } else {
        None
    }
}

/// Zig target string for a Rust target triple.
fn zig_target(target: &str) -> Option<&'static str> {
    Some(match target {
        "aarch64-apple-darwin" => "aarch64-macos.26.0",
        "x86_64-apple-darwin" => "x86_64-macos.26.0",
        "x86_64-unknown-linux-gnu" => "x86_64-linux-gnu",
        "aarch64-unknown-linux-gnu" => "aarch64-linux-gnu",
        "x86_64-pc-windows-msvc" => "x86_64-windows-msvc",
        "aarch64-pc-windows-msvc" => "aarch64-windows-msvc",
        "aarch64-linux-android" => "aarch64-linux-android",
        "x86_64-linux-android" => "x86_64-linux-android",
        "aarch64-apple-ios" => "aarch64-ios.26.0",
        "aarch64-apple-ios-sim" => "aarch64-ios.26.0-simulator",
        "x86_64-apple-ios" => "x86_64-ios.26.0-simulator",
        _ => return None,
    })
}

/// Builds the archive from the vendored Zig sources and returns its directory.
fn build_from_source(root: &Path, target: &str) -> PathBuf {
    let zig_target = zig_target(target).unwrap_or_else(|| {
        panic!(
            "libghostty-vt has no Zig target for {target}; add a mapping in Terminal/rust/build.rs"
        )
    });
    let source = root.join("third-party/ghostty");
    assert!(
        source.join("build.zig").is_file(),
        "libghostty-vt sources are missing; initialize the third-party/ghostty submodule"
    );
    let prefix = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("ghostty-vt");
    let cache = prefix.join("zig-cache");
    let mut build = Command::new(env::var_os("COVE_ZIG").unwrap_or_else(|| "zig".into()));
    build
        .current_dir(&source)
        .args(["build", "-Demit-lib-vt=true", "-Doptimize=ReleaseFast"])
        .arg(format!("-Dtarget={zig_target}"))
        .arg("--prefix")
        .arg(&prefix)
        .arg("--cache-dir")
        .arg(&cache);
    let status = build
        .status()
        .unwrap_or_else(|error| panic!("could not run zig to build libghostty-vt: {error}"));
    assert!(
        status.success(),
        "zig build of libghostty-vt failed: {build:?}"
    );
    prefix.join("lib")
}

fn main() {
    for name in [
        "COVE_ZIG",
        "COVE_GHOSTTY_VT_ROOT",
        "COVE_GHOSTTY_ARTIFACT_ROOT",
        "OUT_DIR",
        "TARGET",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    for path in [
        "rust/build.rs",
        "../third-party/ghostty/build.zig",
        "../third-party/ghostty/build.zig.zon",
        "../third-party/ghostty/include/ghostty/vt.h",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Terminal crate has a parent directory")
        .to_path_buf();
    let target = env::var("TARGET").expect("TARGET");
    let os = env::var("CARGO_CFG_TARGET_OS").expect("CARGO_CFG_TARGET_OS");
    let msvc = target.contains("msvc");

    let artifact_root = env::var_os("COVE_GHOSTTY_ARTIFACT_ROOT")
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                root.join(path)
            }
        })
        .unwrap_or_else(|| root.join("vendor"));
    let bundle = env::var_os("COVE_GHOSTTY_VT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| artifact_root.join("GhosttyVt.artifactbundle"));
    let archive_name = if msvc {
        "ghostty-vt-static.lib"
    } else {
        "libghostty-vt.a"
    };

    let (include, lib) = match variant(&target) {
        Some(variant) => {
            let variant_root = bundle.join(&variant);
            if variant_root.join("lib").join(archive_name).is_file() {
                (variant_root.join("include"), variant_root.join("lib"))
            } else {
                (
                    root.join("third-party/ghostty/include"),
                    build_from_source(&root, &target),
                )
            }
        }
        None => (
            root.join("third-party/ghostty/include"),
            build_from_source(&root, &target),
        ),
    };

    println!("cargo:include={}", include.display());
    println!("cargo:rustc-link-search=native={}", lib.display());
    println!("cargo:rustc-link-lib=static=ghostty-vt");

    if os == "windows" {
        // Zig's Windows build uses the system CSPRNG and process APIs.
        println!("cargo:rustc-link-lib=advapi32");
        println!("cargo:rustc-link-lib=bcrypt");
        println!("cargo:rustc-link-lib=userenv");
    } else if os != "macos" && os != "ios" {
        println!("cargo:rustc-link-lib=m");
    }
}
