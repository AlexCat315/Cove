fn main() {
    let root = "native/system";
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    let mut build = cc::Build::new();
    build.include(format!("{root}/include")).std("c11");
    if std::env::var("CARGO_CFG_TARGET_FAMILY").unwrap() == "unix" {
        build.define("_POSIX_C_SOURCE", "200809L");
        build.define("_DEFAULT_SOURCE", None);
        build.define("_DARWIN_C_SOURCE", None);
    }
    for file in ["Permissions.c", "FileLock.c", "Identity.c", "Process.c"] {
        if os == "wasi" && matches!(file, "FileLock.c" | "Process.c") {
            continue;
        }
        let source = format!("{root}/{file}");
        println!("cargo:rerun-if-changed={source}");
        build.file(source);
    }
    println!("cargo:rerun-if-changed=native/native_files.c");
    build.file("native/native_files.c");
    println!("cargo:rerun-if-changed={root}/include/CoveSystem.h");
    build.compile("cove_system");
    if os == "windows" {
        println!("cargo:rustc-link-lib=advapi32");
    }
    if os == "macos" {
        println!("cargo:rerun-if-changed=native/trash.m");
        cc::Build::new()
            .file("native/trash.m")
            .compile("cove_trash");
        println!("cargo:rustc-link-lib=framework=Foundation");
    }
}
