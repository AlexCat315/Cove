fn main() {
    let source = "Sources/CTextSelection/Selection.c";
    let headers = "Sources/CTextSelection/include";
    println!("cargo:rerun-if-changed={source}");
    println!("cargo:rerun-if-changed={headers}");
    cc::Build::new()
        .file(source)
        .include(headers)
        .warnings(true)
        .compile("cove_text_selection");
}
