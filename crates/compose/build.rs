use std::fs;
use std::path::PathBuf;

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

    generate_embedded_ui_icons();
}

/// Writes the `UIIcons` table into `OUT_DIR`.
///
/// The catalog used to resolve icons through `CARGO_MANIFEST_DIR`, which is the
/// build machine's source tree: fine under `cargo run`, empty inside an APK.
/// Embedding the SVG bytes keeps every platform on one lookup rule.
fn generate_embedded_ui_icons() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let icon_dir = manifest_dir.join("resources/UIIcons");
    println!("cargo:rerun-if-changed={}", icon_dir.display());

    let mut slugs: Vec<(String, PathBuf)> = fs::read_dir(&icon_dir)
        .unwrap_or_else(|error| panic!("read {}: {error}", icon_dir.display()))
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "svg"))
        .filter_map(|path| {
            let slug = path.file_stem()?.to_str()?.to_owned();
            Some((slug, path))
        })
        .collect();
    slugs.sort_by(|left, right| left.0.cmp(&right.0));
    assert!(
        !slugs.is_empty(),
        "no SVG icons found in {}",
        icon_dir.display()
    );

    let mut table = String::new();
    table.push_str("/// Icons compiled into the binary, looked up by catalog slug.\n");
    table.push_str("pub fn bytes(slug: &str) -> Option<&'static [u8]> {\n");
    table.push_str("    match slug {\n");
    for (slug, path) in &slugs {
        table.push_str(&format!(
            "        {slug:?} => Some(include_bytes!({path:?})),\n",
            path = path.display().to_string()
        ));
    }
    table.push_str("        _ => None,\n");
    table.push_str("    }\n");
    table.push_str("}\n\n");
    table.push_str(&format!(
        "/// Number of embedded icons.\npub const COUNT: usize = {};\n",
        slugs.len()
    ));

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("out dir"));
    fs::write(out_dir.join("ui_icons_embedded.rs"), table).expect("write embedded icon table");
}
