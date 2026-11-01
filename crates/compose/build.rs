fn main() {
    let source = "native/text-selection/Selection.c";
    let headers = "native/text-selection/include";
    println!("cargo:rerun-if-changed={source}");
    println!("cargo:rerun-if-changed={headers}");
    cc::Build::new()
        .file(source)
        .include(headers)
        .warnings(true)
        .compile("cove_text_selection");
}
