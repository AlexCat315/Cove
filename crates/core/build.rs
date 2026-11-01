fn main() {
    let source = "native/render-kernels/RenderKernels.c";
    let headers = "native/render-kernels/include";
    println!("cargo:rerun-if-changed={source}");
    println!("cargo:rerun-if-changed=native/text_kernels.c");
    println!("cargo:rerun-if-changed=native/shape_kernels.c");
    println!("cargo:rerun-if-changed={headers}");
    cc::Build::new()
        .file(source)
        .file("native/text_kernels.c")
        .file("native/shape_kernels.c")
        .include(headers)
        .std("c11")
        .opt_level(3)
        .compile("cove_render_kernels");
    let target = std::env::var("CARGO_CFG_TARGET_FAMILY").unwrap();
    if target == "unix" {
        println!("cargo:rustc-link-lib=m");
    }
}
