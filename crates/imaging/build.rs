//! Builds plutovg, lunasvg and libwebp from `third-party/` and the image bridge.
//! PNG and JPEG are decoded by the bundled `stb_image.h`.

use cove_native_build as native;
use std::env;

fn main() {
    for path in [
        "native/image-decode",
        "native/orientation.c",
        "native/apple.m",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap();

    let plutovg = native::cmake("plutovg", &native::third_party("plutovg", "CMakeLists.txt"))
        .define("PLUTOVG_BUILD_EXAMPLES", "OFF")
        .build();
    let lunasvg = native::cmake("lunasvg", &native::third_party("lunasvg", "CMakeLists.txt"))
        .define("USE_SYSTEM_PLUTOVG", "ON")
        .define("plutovg_DIR", plutovg.join("lib/cmake/plutovg"))
        .define("LUNASVG_BUILD_EXAMPLES", "OFF")
        .define("LUNASVG_BUILD_TESTS", "OFF")
        .build();
    assert!(
        !lunasvg.join("build/plutovg").exists(),
        "lunasvg ignored third-party/plutovg and built its bundled copy"
    );
    let mut webp = native::cmake("libwebp", &native::third_party("libwebp", "CMakeLists.txt"));
    for option in [
        "WEBP_BUILD_ANIM_UTILS",
        "WEBP_BUILD_CWEBP",
        "WEBP_BUILD_DWEBP",
        "WEBP_BUILD_GIF2WEBP",
        "WEBP_BUILD_IMG2WEBP",
        "WEBP_BUILD_VWEBP",
        "WEBP_BUILD_WEBPINFO",
        "WEBP_BUILD_WEBPMUX",
        "WEBP_BUILD_EXTRAS",
        "WEBP_BUILD_LIBWEBPMUX",
        "WEBP_BUILD_WEBP_JS",
    ] {
        webp.define(option, "OFF");
    }
    let webp = webp.define("WEBP_ENABLE_SIMD", "ON").build();

    let mut bridge = cc::Build::new();
    bridge
        .cpp(true)
        .std("c++17")
        .file("native/image-decode/image_decode_bridge.cpp")
        .include("native/image-decode/include")
        .include(lunasvg.join("include/lunasvg"))
        .include(webp.join("include"))
        .define("LUNASVG_BUILD_STATIC", None)
        .opt_level(3);
    native::configure_cc(&mut bridge, true);
    bridge.compile("cove_image_decode_bridge");
    let mut kernels = cc::Build::new();
    kernels.file("native/orientation.c").std("c11").opt_level(3);
    native::configure_cc(&mut kernels, false);
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

    native::link_static(&lunasvg.join("lib"), "lunasvg");
    native::link_static(&plutovg.join("lib"), "plutovg");
    let webp_lib = webp.join("lib");
    native::link_static(&webp_lib, "webp");
    native::link_static(&webp_lib, "sharpyuv");
    if native::has_static(&webp_lib, "cpufeatures-webp") {
        native::link_static(&webp_lib, "cpufeatures-webp");
    }
    native::link_cxx_runtime();
    if os != "windows" {
        println!("cargo:rustc-link-lib=m");
    }
}
