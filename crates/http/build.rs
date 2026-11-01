fn main() {
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    let family = std::env::var("CARGO_CFG_TARGET_FAMILY").unwrap_or_default();
    let apple = matches!(os.as_str(), "macos" | "ios");
    let socket =
        std::env::var_os("CARGO_FEATURE_OPENSSL").is_some() || (family == "unix" && !apple);
    println!("cargo::rustc-check-cfg=cfg(cove_http_socket)");
    if socket && family == "unix" {
        println!("cargo:rustc-cfg=cove_http_socket");
    }
    println!("cargo:rerun-if-changed=native/openssl.c");
    println!("cargo:rerun-if-changed=native/apple.m");
    if socket && family == "unix" {
        let mut build = cc::Build::new();
        build.file("native/openssl.c").std("c11");
        build.define("_POSIX_C_SOURCE", "200809L");
        build.define("_DEFAULT_SOURCE", None);
        build.define("_DARWIN_C_SOURCE", None);
        build.compile("cove_http_openssl");
        if os == "linux" {
            println!("cargo:rustc-link-lib=dl");
        }
    }
    if apple {
        cc::Build::new()
            .file("native/apple.m")
            .flag("-fobjc-arc")
            .compile("cove_http_apple");
        println!("cargo:rustc-link-lib=framework=Foundation");
    }
    if os == "windows" {
        println!("cargo:rustc-link-lib=winhttp");
    }
}
