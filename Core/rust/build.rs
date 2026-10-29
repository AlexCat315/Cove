fn main() {
    let source = "Sources/CRenderKernels/RenderKernels.c";
    let headers = "Sources/CRenderKernels/include";
    println!("cargo:rerun-if-changed={source}");
    println!("cargo:rerun-if-changed={headers}");
    cc::Build::new()
        .file(source)
        .include(headers)
        .std("c11")
        .opt_level(3)
        .compile("cove_render_kernels");
    let target = std::env::var("CARGO_CFG_TARGET_FAMILY").unwrap();
    if target == "unix" {
        println!("cargo:rustc-link-lib=m");
    }
}
