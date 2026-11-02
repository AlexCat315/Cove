//! Builds SDL3 from `third-party/sdl3` and the shell bridge.

use cove_native_build as native;
use std::env;

fn main() {
    for path in [
        "native",
        "native/sdl3/csdl3_link_anchor.c",
        "native/sdl3/include/sdl3_shim.h",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap();

    let mut sdl = native::cmake("sdl3", &native::third_party("sdl3", "CMakeLists.txt"));
    sdl.define("SDL_STATIC", "ON")
        .define("SDL_SHARED", "OFF")
        .define("SDL_TEST_LIBRARY", "OFF")
        .define("SDL_TESTS", "OFF")
        .define("SDL_EXAMPLES", "OFF");
    if os == "android" {
        // The Cove activity owns `JNI_OnLoad`; SDL's registers under this name.
        sdl.cflag("-DJNI_OnLoad=CoveSDL_JNI_OnLoad");
    }
    let sdl = sdl.build();
    let include = sdl.join("include");
    let mut bridge = cc::Build::new();
    bridge
        .file("native/native.c")
        .file("native/sdl3/csdl3_link_anchor.c")
        .include("native")
        .include("native/sdl3/include")
        .include(&include)
        .std("c11")
        .opt_level(3);
    native::configure_cc(&mut bridge, false);
    // Debug builds abort on any platform call that strays off the main thread.
    let assert_main_thread = env::var("PROFILE").is_ok_and(|profile| profile == "debug");
    println!("cargo:rerun-if-env-changed=PROFILE");
    if assert_main_thread {
        bridge.define("COVE_ASSERT_MAIN_THREAD", None);
    }
    bridge.compile("cove_shell_bridge");
    if os == "macos" || os == "ios" {
        let mut apple = cc::Build::new();
        apple
            .file("native/apple.m")
            .include("native")
            .include(&include)
            .flag("-fobjc-arc")
            .flag("-fblocks");
        if assert_main_thread {
            apple.define("COVE_ASSERT_MAIN_THREAD", None);
        }
        apple.compile("cove_shell_apple");
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
    native::link_static(&sdl.join("lib"), "SDL3");
    println!("cargo:include={}", include.display());
}
