//! Verifies the bundled welcome resource survives packaging, matching
//! `AppContentTests/Resources.swift`.

#[test]
fn welcome_resource_is_packaged() {
    let bytes = include_bytes!("../Assets/welcome.txt");
    let text = std::str::from_utf8(bytes).expect("welcome.txt must be UTF-8");
    assert_eq!(text.trim(), "Welcome to CoveUI · 你好，CoveUI");
}
