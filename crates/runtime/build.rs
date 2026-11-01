fn main() {
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    if os == "macos" || os == "ios" {
        println!("cargo:rerun-if-changed=native/lifecycle.m");
        println!("cargo:rerun-if-changed=native/accessibility.m");
        let mut build = cc::Build::new();
        build
            .file("native/lifecycle.m")
            .file("native/accessibility.m")
            .flag("-fobjc-arc");
        build.compile("cove_runtime_native");
        for framework in ["Foundation", "QuartzCore"] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
        println!(
            "cargo:rustc-link-lib=framework={}",
            if os == "macos" { "AppKit" } else { "UIKit" }
        );
    }
}
