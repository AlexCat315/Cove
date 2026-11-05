fn main() {
    let root = "native/logging";
    let source = format!("{root}/Router.c");
    println!("cargo:rerun-if-changed={source}");
    println!("cargo:rerun-if-changed={root}/include/CoveLog.h");
    let mut build = cc::Build::new();
    build
        .include(format!("{root}/include"))
        .std("c11")
        .file(&source);
    if std::env::var("CARGO_CFG_TARGET_FAMILY").unwrap() == "unix" {
        // Strict c11 hides the O_CLOEXEC and flock declarations the router uses.
        build.define("_POSIX_C_SOURCE", "200809L");
        build.define("_DEFAULT_SOURCE", None);
        build.define("_DARWIN_C_SOURCE", None);
    }
    build.compile("cove_log");
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "android" {
        println!("cargo:rustc-link-lib=log");
    }
}
