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
        "native text build failed: {description}"
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
        "COVE_TEXT_ARTIFACT_ROOT",
        "ANDROID_NDK_HOME",
        "ANDROID_NDK_ROOT",
        "COVE_WASI_SDK_PATH",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    for path in [
        "native/native.c",
        "native/native.h",
        "native/apple.m",
        "native/CMakeLists.txt",
        "../../third-party/freetype/.git",
        "../../third-party/harfbuzz/.git",
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
    let artifacts = env::var_os("COVE_TEXT_ARTIFACT_ROOT")
        .map(PathBuf::from)
        .map(|p| if p.is_absolute() { p } else { root.join(p) })
        .unwrap_or_else(|| root.join("vendor"));
    let ft = artifacts.join("CFreeType.artifactbundle").join(&variant);
    let hb = artifacts.join("CHarfBuzz.artifactbundle").join(&variant);
    let msvc = target.contains("msvc");
    let ft_name = if msvc {
        "freetype.lib"
    } else {
        "libfreetype.a"
    };
    let hb_name = if msvc {
        "harfbuzz.lib"
    } else {
        "libharfbuzz.a"
    };
    let (ft_include, hb_include, ft_lib, hb_lib) =
        if ft.join("lib").join(ft_name).is_file() && hb.join("lib").join(hb_name).is_file() {
            (
                ft.join("include"),
                hb.join("include"),
                ft.join("lib"),
                hb.join("lib"),
            )
        } else {
            assert!(
                root.join("third-party/freetype/CMakeLists.txt").is_file()
                    && root.join("third-party/harfbuzz/CMakeLists.txt").is_file(),
                "initialize FreeType and HarfBuzz git submodules"
            );
            let out = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("text-native");
            let prefix = out.join("install");
            let mut configure = Command::new("cmake");
            configure
                .arg("-S")
                .arg(root.join("crates/text/native"))
                .arg("-B")
                .arg(&out)
                .arg(format!("-DCOVE_TEXT_PREFIX={}", prefix.display()));
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
                .args(["--config", "Release", "--parallel"])
                .arg(env::var("NUM_JOBS").unwrap_or_else(|_| "4".into())));
            (
                prefix.join("include/freetype2"),
                prefix.join("include/harfbuzz"),
                prefix.join("lib"),
                prefix.join("lib"),
            )
        };
    let mut build = cc::Build::new();
    build
        .file("native/native.c")
        .include("native")
        .include(ft_include)
        .include(hb_include)
        .std("c11")
        .opt_level(3);
    if target.starts_with("wasm")
        && let Some(sdk) = env::var_os("COVE_WASI_SDK_PATH")
    {
        let sdk = PathBuf::from(sdk);
        build
            .compiler(sdk.join("bin/clang"))
            .flag("--target=wasm32-wasi")
            .flag(format!(
                "--sysroot={}",
                sdk.join("share/wasi-sysroot").display()
            ));
    }
    build.compile("cove_text_bridge");
    if os == "macos" || os == "ios" {
        cc::Build::new()
            .file("native/apple.m")
            .include("native")
            .flag("-fobjc-arc")
            .compile("cove_text_apple");
        for framework in ["Foundation", "CoreText", "CoreGraphics"] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
    }
    println!("cargo:rustc-link-search=native={}", hb_lib.display());
    println!("cargo:rustc-link-search=native={}", ft_lib.display());
    println!("cargo:rustc-link-lib=static=harfbuzz");
    println!("cargo:rustc-link-lib=static=freetype");
    if os == "macos" || os == "ios" {
        println!("cargo:rustc-link-lib=c++");
    } else if os == "android" {
        // The APK ships libc++_shared.so (Shell links it too). The NDK driver's
        // default search finds it after the API-level libc.so; an explicit
        // unversioned sysroot path would select the static libc.a instead.
        println!("cargo:rustc-link-lib=c++_shared");
    } else if !msvc && !target.starts_with("wasm") {
        println!("cargo:rustc-link-lib=stdc++");
    }
    if os != "windows" {
        println!("cargo:rustc-link-lib=m");
    }
}
