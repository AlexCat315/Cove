#[test]
fn welcome_resource_is_utf8() {
    let text = std::str::from_utf8(include_bytes!("../Assets/welcome.txt")).expect("UTF-8");
    assert!(!text.trim().is_empty());
}
