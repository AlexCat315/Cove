//! Builds FreeType and HarfBuzz from `third-party/` and the text bridge.

use cove_native_build as native;
use std::env;

fn main() {
    for path in ["native/native.c", "native/native.h", "native/apple.m"] {
        println!("cargo:rerun-if-changed={path}");
    }
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap();

    let freetype = native::cmake(
        "freetype",
        &native::third_party("freetype", "CMakeLists.txt"),
    )
    .define("FT_DISABLE_ZLIB", "ON")
    .define("FT_DISABLE_BZIP2", "ON")
    .define("FT_DISABLE_PNG", "ON")
    .define("FT_DISABLE_HARFBUZZ", "ON")
    .define("FT_DISABLE_BROTLI", "ON")
    .build();
    let freetype_lib = freetype.join("lib");
    let freetype_include = freetype.join("include/freetype2");
    let freetype_archive = ["libfreetype.a", "freetype.lib"]
        .iter()
        .map(|name| freetype_lib.join(name))
        .find(|path| path.is_file())
        .expect("FreeType installs a static archive");

    let harfbuzz = native::cmake(
        "harfbuzz",
        &native::third_party("harfbuzz", "CMakeLists.txt"),
    )
    .define("FREETYPE_LIBRARY", &freetype_archive)
    .define("FREETYPE_INCLUDE_DIR_ft2build", &freetype_include)
    .define("FREETYPE_INCLUDE_DIR_freetype2", &freetype_include)
    .define("HB_HAVE_FREETYPE", "ON")
    .define("HB_BUILD_TESTS", "OFF")
    .define("HB_BUILD_UTILS", "OFF")
    .define("HB_BUILD_SUBSET", "OFF")
    .define("HB_HAVE_GLIB", "OFF")
    .define("HB_HAVE_ICU", "OFF")
    .define("HB_HAVE_GRAPHITE2", "OFF")
    .define("HB_HAVE_CORETEXT", "OFF")
    .build();
    let harfbuzz_include = harfbuzz.join("include/harfbuzz");

    let mut bridge = cc::Build::new();
    bridge
        .file("native/native.c")
        .include("native")
        .include(&freetype_include)
        .include(&harfbuzz_include)
        .std("c11")
        .opt_level(3);
    native::configure_cc(&mut bridge, false);
    bridge.compile("cove_text_bridge");
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

    native::link_static(&harfbuzz.join("lib"), "harfbuzz");
    native::link_static(&freetype_lib, "freetype");
    native::link_cxx_runtime();
    if os != "windows" {
        println!("cargo:rustc-link-lib=m");
    }
    println!(
        "cargo:include={}",
        env::join_paths([&freetype_include, &harfbuzz_include])
            .unwrap()
            .to_string_lossy()
    );
}
