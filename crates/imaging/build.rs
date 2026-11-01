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
        "native image build failed: {description}"
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
        "COVE_IMAGE_ARTIFACT_ROOT",
        "ANDROID_NDK_HOME",
        "ANDROID_NDK_ROOT",
        "COVE_WASI_SDK_PATH",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    for path in [
        "native/image-decode",
        "native/orientation.c",
        "native/apple.m",
        "native/CMakeLists.txt",
        "../../Native/third-party/cmake/BuildImageDecode.cmake",
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
    let artifacts = env::var_os("COVE_IMAGE_ARTIFACT_ROOT")
        .map(PathBuf::from)
        .map(|p| if p.is_absolute() { p } else { root.join(p) })
        .unwrap_or_else(|| root.join("Native/vendor"));
    let msvc = target.contains("msvc");
    let names = ["lunasvg", "plutovg", "webp", "sharpyuv"];
    let available = names.iter().all(|name| {
        artifacts
            .join(format!("{name}.artifactbundle"))
            .join(&variant)
            .join("lib")
            .join(if msvc {
                format!("{name}.lib")
            } else {
                format!("lib{name}.a")
            })
            .is_file()
    });
    let artifacts = if available {
        artifacts
    } else {
        let out = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("image-native");
        let prefix = out.join("artifacts");
        let mut configure = Command::new("cmake");
        configure
            .arg("-S")
            .arg(root.join("crates/imaging/native"))
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
                "stage_image_decode",
                "--parallel",
            ])
            .arg(env::var("NUM_JOBS").unwrap_or_else(|_| "4".into())));
        prefix
    };
    let mut bridge = cc::Build::new();
    bridge
        .cpp(true)
        .std("c++17")
        .file("native/image-decode/image_decode_bridge.cpp")
        .include("native/image-decode/include")
        .define("LUNASVG_BUILD_STATIC", None)
        .opt_level(3);
    for name in names {
        let path = artifacts
            .join(format!("{name}.artifactbundle"))
            .join(&variant);
        bridge.include(path.join("include"));
        println!(
            "cargo:rustc-link-search=native={}",
            path.join("lib").display()
        );
    }
    if target.starts_with("wasm")
        && let Some(sdk) = env::var_os("COVE_WASI_SDK_PATH")
    {
        let sdk = PathBuf::from(sdk);
        bridge
            .compiler(sdk.join("bin/clang++"))
            .flag("--target=wasm32-wasi")
            .flag(format!(
                "--sysroot={}",
                sdk.join("share/wasi-sysroot").display()
            ));
    }
    bridge.compile("cove_image_decode_bridge");
    let mut kernels = cc::Build::new();
    kernels.file("native/orientation.c").std("c11").opt_level(3);
    if target.starts_with("wasm")
        && let Some(sdk) = env::var_os("COVE_WASI_SDK_PATH")
    {
        let sdk = PathBuf::from(sdk);
        kernels
            .compiler(sdk.join("bin/clang"))
            .flag("--target=wasm32-wasi")
            .flag(format!(
                "--sysroot={}",
                sdk.join("share/wasi-sysroot").display()
            ));
    }
    kernels.compile("cove_image_kernels");
    if os == "macos" || os == "ios" {
        cc::Build::new()
            .file("native/apple.m")
            .include("native/image-decode/include")
            .flag("-fobjc-arc")
            .compile("cove_image_apple");
        for framework in ["Foundation", "ImageIO", "CoreGraphics"] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
    }
    for name in names {
        println!("cargo:rustc-link-lib=static={name}");
    }
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
