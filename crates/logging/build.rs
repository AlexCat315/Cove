fn main() {
    let root = "native/logging";
    let source = format!("{root}/Router.c");
    println!("cargo:rerun-if-changed={source}");
    println!("cargo:rerun-if-changed={root}/include/CoveLog.h");
    cc::Build::new()
        .include(format!("{root}/include"))
        .std("c11")
        .file(&source)
        .compile("cove_log");
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "android" {
        println!("cargo:rustc-link-lib=log");
    }
}
