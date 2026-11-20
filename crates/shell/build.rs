//! Builds SDL3 from `third-party/sdl3` and the shell bridge.

use cove_native_build as native;
use std::env;

/// SDL tears its Cocoa field editor down while the input method still owns
/// marked text, and AppKit commits that raw pinyin through `insertText:` as
/// the responder goes away. Clicking elsewhere mid-composition then writes
/// `dsaasd` into the document. SDL exposes no "discard composition" call, so
/// vendor the two-line guard here; the submodule stays pinned upstream.
fn patch_sdl_cocoa_ime() {
    let root = native::third_party("sdl3", "CMakeLists.txt");
    let path = root.join("src/video/cocoa/SDL_cocoakeyboard.m");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    if source.contains("COVE_IME_TEARDOWN") {
        return;
    }

    let mut patched = source.clone();
    let ivar_old = "    Uint64 _pendingTimestamp;\n}";
    let ivar_new =
        "    Uint64 _pendingTimestamp;\n    BOOL _suppressInsert; // COVE_IME_TEARDOWN\n}";
    assert!(
        patched.contains(ivar_old),
        "SDL text-field responder shape changed"
    );
    patched = patched.replacen(ivar_old, ivar_new, 1);

    let decl_old = "- (void)doCommandBySelector:(SEL)myselector;";
    let decl_new = "- (void)doCommandBySelector:(SEL)myselector;\n- (void)suppressInsert;";
    assert!(
        patched.contains(decl_old),
        "SDL responder declaration changed"
    );
    patched = patched.replacen(decl_old, decl_new, 1);

    let insert_old = "    const char *str;\n\n    DEBUG_IME(@\"insertText: %@ replacementRange: (%d, %d)\", aString,";
    let insert_new = "    const char *str;\n\n    /* The input method can commit marked text while the field editor is torn\n       down. That commit is not user intent. */\n    if (_suppressInsert) {\n        return;\n    }\n\n    DEBUG_IME(@\"insertText: %@ replacementRange: (%d, %d)\", aString,";
    assert!(patched.contains(insert_old), "SDL insertText shape changed");
    patched = patched.replacen(insert_old, insert_new, 1);

    let unmark_old = "- (void)unmarkText\n{";
    let unmark_new =
        "- (void)suppressInsert\n{\n    _suppressInsert = YES;\n}\n\n- (void)unmarkText\n{";
    assert!(patched.contains(unmark_old), "SDL unmarkText shape changed");
    patched = patched.replacen(unmark_old, unmark_new, 1);

    let stop_old = "        if (data && data.fieldEdit) {\n            [data.fieldEdit removeFromSuperview];\n            data.fieldEdit = nil;\n        }";
    let stop_new = "        if (data && data.fieldEdit) {\n            /* Drop the marked composition without committing it. */\n            [data.fieldEdit suppressInsert];\n            [data.fieldEdit unmarkText];\n            [data.fieldEdit removeFromSuperview];\n            data.fieldEdit = nil;\n        }";
    assert!(
        patched.contains(stop_old),
        "SDL StopTextInput shape changed"
    );
    patched = patched.replacen(stop_old, stop_new, 1);

    std::fs::write(&path, patched)
        .unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
}

fn main() {
    for path in [
        "native",
        "native/sdl3/csdl3_link_anchor.c",
        "native/sdl3/include/sdl3_shim.h",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    if os == "macos" {
        patch_sdl_cocoa_ime();
    }

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
    // Screen-colour sampling is pure Rust. The Linux backends load libX11 with
    // dlopen and talk to the XDG portal over the session bus, so the bridge
    // needs no X11 headers or link-time libraries on any target.
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
        // apple.m uses `UTType.folder` / `UTType.item` /
        // `initForOpeningContentTypes:asCopy:` which require the iOS 14 SDK
        // availability. Without an explicit floor the cc build defaults below
        // 14.0 and fails to compile apple.m; only pin it when the caller has
        // not already set a deployment target.
        if os == "ios" && std::env::var_os("IPHONEOS_DEPLOYMENT_TARGET").is_none() {
            apple.flag("-miphoneos-version-min=14.0");
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
