fn main() {
    let root = "Sources/CCoveSystem";
    let mut build = cc::Build::new();
    build.include(format!("{root}/include")).std("c11");
    if std::env::var("CARGO_CFG_TARGET_FAMILY").unwrap() == "unix" {
        build.define("_POSIX_C_SOURCE", "200809L");
        build.define("_DEFAULT_SOURCE", None);
        build.define("_DARWIN_C_SOURCE", None);
    }
    for file in ["Permissions.c", "FileLock.c", "Identity.c", "Process.c"] {
        let source = format!("{root}/{file}");
        println!("cargo:rerun-if-changed={source}");
        build.file(source);
    }
    println!("cargo:rerun-if-changed=rust/native_files.c");
    build.file("rust/native_files.c");
    println!("cargo:rerun-if-changed={root}/include/CoveSystem.h");
    build.compile("cove_system");
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        println!("cargo:rustc-link-lib=advapi32");
    }
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "macos" {
        println!("cargo:rerun-if-changed=rust/trash.m");
        cc::Build::new().file("rust/trash.m").compile("cove_trash");
        println!("cargo:rustc-link-lib=framework=Foundation");
    }
}
