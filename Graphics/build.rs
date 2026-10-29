fn main(){println!("cargo:rerun-if-changed=rust/ui_kernels.c");cc::Build::new().file("rust/ui_kernels.c").std("c11").opt_level(3).compile("cove_ui_kernels");}
