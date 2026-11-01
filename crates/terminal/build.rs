//! Builds libghostty-vt for `cove-terminal` from `third-party/ghostty` with
//! `zig build -Demit-lib-vt` into `OUT_DIR`.

use std::{env, path::PathBuf, process::Command};

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

fn main() {
    println!("cargo:rerun-if-env-changed=COVE_ZIG");
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../third-party/ghostty");
    for path in ["build.zig", "build.zig.zon", "include/ghostty/vt.h"] {
        println!("cargo:rerun-if-changed={}", source.join(path).display());
    }
    assert!(
        source.join("build.zig").is_file(),
        "{} is missing; run `git submodule update --init third-party/ghostty`",
        source.join("build.zig").display()
    );
    let target = env::var("TARGET").expect("TARGET");
    let os = env::var("CARGO_CFG_TARGET_OS").expect("CARGO_CFG_TARGET_OS");
    let zig_target = zig_target(&target).unwrap_or_else(|| {
        panic!("libghostty-vt has no Zig target for {target}; add one in crates/terminal/build.rs")
    });

    let prefix = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("ghostty-vt");
    let mut build = Command::new(env::var_os("COVE_ZIG").unwrap_or_else(|| "zig".into()));
    build
        .current_dir(&source)
        .args([
            "build",
            "-Demit-lib-vt=true",
            "-Demit-xcframework=false",
            "-Demit-macos-app=false",
            "-Doptimize=ReleaseFast",
        ])
        .arg(format!("-Dtarget={zig_target}"))
        .arg("--prefix")
        .arg(&prefix)
        .arg("--cache-dir")
        .arg(prefix.join("zig-cache"));
    let status = build
        .status()
        .unwrap_or_else(|error| panic!("could not run zig to build libghostty-vt: {error}"));
    assert!(
        status.success(),
        "zig build of libghostty-vt failed: {build:?}"
    );

    println!("cargo:include={}", source.join("include").display());
    println!(
        "cargo:rustc-link-search=native={}",
        prefix.join("lib").display()
    );
    let library = if os == "windows" {
        "ghostty-vt-static"
    } else {
        "ghostty-vt"
    };
    println!("cargo:rustc-link-lib=static={library}");
    if os == "windows" {
        // Zig's Windows build uses the system CSPRNG and process APIs.
        for library in ["advapi32", "bcrypt", "userenv"] {
            println!("cargo:rustc-link-lib={library}");
        }
    } else if os != "macos" && os != "ios" {
        println!("cargo:rustc-link-lib=m");
    }
}
