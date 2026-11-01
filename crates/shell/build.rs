use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};
fn run(command: &mut Command) {
    let description = format!("{command:?}");
    assert!(
        command
            .status()
            .unwrap_or_else(|error| panic!("{description}: {error}"))
            .success(),
        "native shell build failed: {description}"
    );
}
fn variant(target: &str) -> String {
    let arch = if target.starts_with("aarch64") {
        "arm64"
    } else if target.starts_with("x86_64") {
        "x86_64"
    } else {
        "wasm32"
    };
    if target.contains("apple-darwin") {
        format!("macos-{arch}")
    } else if target.contains("apple-ios") {
        format!(
            "ios-{arch}{}",
            if target.ends_with("-sim") || arch == "x86_64" {
                "-simulator"
            } else {
                ""
            }
        )
    } else if target.contains("android") {
        format!("android-{arch}")
    } else if target.contains("windows") {
        format!("windows-{arch}")
    } else if target.contains("wasm") {
        "wasm32-wasi".into()
    } else {
        format!("linux-{}", if arch == "arm64" { "aarch64" } else { arch })
    }
}
fn ndk_root() -> PathBuf {
    env::var_os("ANDROID_NDK_HOME")
        .or_else(|| env::var_os("ANDROID_NDK_ROOT"))
        .map(PathBuf::from)
        .expect("Android text builds require ANDROID_NDK_HOME or ANDROID_NDK_ROOT")
}
fn main() {
    for name in [
        "COVE_SHELL_ARTIFACT_ROOT",
        "ANDROID_NDK_HOME",
        "ANDROID_NDK_ROOT",
        "COVE_WASI_SDK_PATH",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    for path in [
        "native",
        "native/sdl3/csdl3_link_anchor.c",
        "native/sdl3/include/sdl3_shim.h",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    let target = env::var("TARGET").unwrap();
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let variant = variant(&target);
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_path_buf();
    let artifacts = env::var_os("COVE_SHELL_ARTIFACT_ROOT")
        .map(PathBuf::from)
        .map(|p| if p.is_absolute() { p } else { root.join(p) })
        .unwrap_or_else(|| root.join("Native/vendor"));
    let msvc = target.contains("msvc");
    let names = ["SDL3"];
    let available = names.iter().all(|name| {
        artifacts
            .join(format!("{name}.artifactbundle"))
            .join(&variant)
            .join("lib")
            .join(if msvc {
                format!("{name}-static.lib")
            } else {
                format!("lib{name}.a")
            })
            .is_file()
    });
    let artifacts = if available {
        artifacts
    } else {
        let out = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("shell-native");
        let prefix = out.join("artifacts");
        let mut configure = Command::new("cmake");
        configure
            .arg("-S")
            .arg(root.join("crates/shell/native"))
            .arg("-B")
            .arg(&out)
            .arg(format!("-DCOVE_VENDOR_DIR={}", prefix.display()))
            .arg(format!("-DCOVE_TRIPLE={variant}"))
            .arg(format!("-DCOVE_SPM_TRIPLE={target}"));
        if os == "ios" {
            configure
                .args([
                    "-DCMAKE_SYSTEM_NAME=iOS",
                    "-DCMAKE_OSX_DEPLOYMENT_TARGET=26.0",
                ])
                .arg(format!(
                    "-DCMAKE_OSX_ARCHITECTURES={}",
                    if target.starts_with("aarch64") {
                        "arm64"
                    } else {
                        "x86_64"
                    }
                ))
                .arg(format!(
                    "-DCMAKE_OSX_SYSROOT={}",
                    if target.ends_with("-sim") || target.starts_with("x86_64") {
                        "iphonesimulator"
                    } else {
                        "iphoneos"
                    }
                ));
        } else if os == "macos" {
            configure
                .arg("-DCMAKE_OSX_DEPLOYMENT_TARGET=26.0")
                .arg(format!(
                    "-DCMAKE_OSX_ARCHITECTURES={}",
                    if target.starts_with("aarch64") {
                        "arm64"
                    } else {
                        "x86_64"
                    }
                ));
        } else if os == "android" {
            let ndk = ndk_root();
            configure
                .arg(format!(
                    "-DCMAKE_TOOLCHAIN_FILE={}",
                    ndk.join("build/cmake/android.toolchain.cmake").display()
                ))
                .arg(format!(
                    "-DANDROID_ABI={}",
                    if target.starts_with("aarch64") {
                        "arm64-v8a"
                    } else {
                        "x86_64"
                    }
                ))
                .args(["-DANDROID_PLATFORM=26", "-DANDROID_STL=c++_static"]);
        } else if target.starts_with("wasm") {
            let sdk = PathBuf::from(
                env::var_os("COVE_WASI_SDK_PATH")
                    .expect("Wasm text dependencies require COVE_WASI_SDK_PATH"),
            );
            configure.arg(format!(
                "-DCMAKE_TOOLCHAIN_FILE={}",
                sdk.join("share/cmake/wasi-sdk.cmake").display()
            ));
        } else if msvc {
            configure.arg("-A").arg(if target.starts_with("aarch64") {
                "ARM64"
            } else {
                "x64"
            });
        } else if target != env::var("HOST").unwrap() {
            let c = cc::Build::new().get_compiler();
            let cpp = cc::Build::new().cpp(true).get_compiler();
            configure
                .arg("-DCMAKE_SYSTEM_NAME=Linux")
                .arg(format!(
                    "-DCMAKE_SYSTEM_PROCESSOR={}",
                    if target.starts_with("aarch64") {
                        "aarch64"
                    } else {
                        "x86_64"
                    }
                ))
                .arg(format!("-DCMAKE_C_COMPILER={}", c.path().display()))
                .arg(format!("-DCMAKE_CXX_COMPILER={}", cpp.path().display()));
        }
        if msvc {
            configure.arg(format!(
                "-DCMAKE_MSVC_RUNTIME_LIBRARY={}",
                if env::var("CARGO_CFG_TARGET_FEATURE")
                    .unwrap_or_default()
                    .contains("crt-static")
                {
                    "MultiThreaded"
                } else {
                    "MultiThreadedDLL"
                }
            ));
        }
        run(&mut configure);
        run(Command::new("cmake")
            .arg("--build")
            .arg(&out)
            .args([
                "--config",
                "Release",
                "--target",
                "stage_sdl3",
                "--parallel",
            ])
            .arg(env::var("NUM_JOBS").unwrap_or_else(|_| "4".into())));
        prefix
    };
    let include = artifacts
        .join("SDL3.artifactbundle")
        .join(&variant)
        .join("include");
    let mut bridge = cc::Build::new();
    bridge
        .file("native/native.c")
        .file("native/sdl3/csdl3_link_anchor.c")
        .include("native")
        .include("native/sdl3/include")
        .include(&include)
        .std("c11")
        .opt_level(3)
        .compile("cove_shell_bridge");
    if os == "macos" || os == "ios" {
        cc::Build::new()
            .file("native/apple.m")
            .include("native")
            .include(&include)
            .flag("-fobjc-arc")
            .flag("-fblocks")
            .compile("cove_shell_apple");
        for framework in [
            "Foundation",
            "AVFoundation",
            "AudioToolbox",
            "CoreAudio",
            "CoreFoundation",
            "CoreGraphics",
            "CoreHaptics",
            "CoreMedia",
            "CoreVideo",
            "GameController",
            "Metal",
            "QuartzCore",
            "UniformTypeIdentifiers",
        ] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
        let extra = if os == "macos" {
            &["Carbon", "Cocoa", "ForceFeedback", "IOKit"][..]
        } else {
            &["UIKit", "CoreBluetooth", "CoreMotion", "OpenGLES"][..]
        };
        for framework in extra {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
        println!("cargo:rustc-link-lib=objc");
    } else if os == "windows" {
        for library in [
            "winmm", "setupapi", "imm32", "version", "ole32", "oleaut32", "uuid", "advapi32",
            "shell32", "user32", "gdi32", "d3d11", "dxgi", "d3d12", "cfgmgr32",
        ] {
            println!("cargo:rustc-link-lib={library}");
        }
    } else if os == "android" {
        for library in ["android", "log", "GLESv2", "OpenSLES", "dl"] {
            println!("cargo:rustc-link-lib={library}");
        }
    } else {
        for library in ["dl", "pthread", "m", "rt"] {
            println!("cargo:rustc-link-lib={library}");
        }
    }
    println!(
        "cargo:rustc-link-search=native={}",
        artifacts
            .join("SDL3.artifactbundle")
            .join(&variant)
            .join("lib")
            .display()
    );
    println!(
        "cargo:rustc-link-lib=static={}",
        if msvc { "SDL3-static" } else { "SDL3" }
    );
}
