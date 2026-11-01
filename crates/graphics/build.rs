fn main() {
    println!("cargo:rerun-if-changed=native/ui_kernels.c");
    cc::Build::new()
        .file("native/ui_kernels.c")
        .std("c11")
        .opt_level(3)
        .compile("cove_ui_kernels");
}
