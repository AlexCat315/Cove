//! The hand-written Vulkan bindings must agree with the bundled header.
//!
//! A wrong `sType` is not a compile error: the driver reads the structure as
//! something else. This compares every `VK_` constant in `sys.rs` with the
//! value `vulkan_core.h` assigns, as text, so it runs on every host whether or
//! not the Vulkan backend is compiled there.

use std::collections::HashMap;

fn parse_integer(text: &str) -> Option<i128> {
    let text = text.trim().trim_end_matches(['U', 'L', 'u', 'l', ',']);
    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        i128::from_str_radix(hex, 16).ok()
    } else {
        text.replace('_', "").parse().ok()
    }
}

fn header_values(header: &str) -> HashMap<String, i128> {
    let mut values = HashMap::new();
    let mut aliases = Vec::new();
    for line in header.lines() {
        let line = line.trim();
        let (name, value) = if let Some(rest) = line.strip_prefix("#define ") {
            match rest.split_once(char::is_whitespace) {
                Some(pair) => pair,
                None => continue,
            }
        } else if let Some((name, value)) = line.split_once('=') {
            (name.trim(), value.trim())
        } else {
            continue;
        };
        if !name.starts_with("VK_") || name.contains(char::is_whitespace) {
            continue;
        }
        let value = value.trim_end_matches(',').trim();
        if let Some(number) = parse_integer(value) {
            values.entry(name.to_owned()).or_insert(number);
        } else if value.starts_with("VK_") {
            aliases.push((name.to_owned(), value.to_owned()));
        }
    }
    for (alias, target) in aliases {
        if let Some(value) = values.get(&target).copied() {
            values.entry(alias).or_insert(value);
        }
    }
    values
}

#[test]
fn binding_constants_match_the_bundled_header() {
    let root = env!("CARGO_MANIFEST_DIR");
    let header =
        std::fs::read_to_string(format!("{root}/native/vulkan/include/vulkan/vulkan_core.h"))
            .expect("the Vulkan header ships with the crate");
    let bindings = std::fs::read_to_string(format!("{root}/src/vulkan/sys.rs"))
        .expect("the Vulkan bindings exist");
    let expected = header_values(&header);

    let mut checked = 0;
    let mut mismatches = Vec::new();
    for line in bindings.lines() {
        let Some(rest) = line.trim().strip_prefix("pub const VK_") else {
            continue;
        };
        let Some((name, rest)) = rest.split_once(':') else {
            continue;
        };
        let Some((ty, value)) = rest.split_once('=') else {
            continue;
        };
        let name = format!("VK_{name}");
        let Some(value) = parse_integer(value.trim().trim_end_matches(';')) else {
            continue;
        };
        let Some(&header_value) = expected.get(&name) else {
            continue;
        };
        // Unsigned bindings of negative enumerants keep the two's-complement bits.
        let header_value = match ty.trim() {
            "u32" if header_value < 0 => header_value & 0xFFFF_FFFF,
            "u64" if header_value < 0 => header_value & 0xFFFF_FFFF_FFFF_FFFF,
            _ => header_value,
        };
        checked += 1;
        if value != header_value {
            mismatches.push(format!("{name}: bindings {value}, header {header_value}"));
        }
    }
    assert!(
        checked > 100,
        "the comparison covers the bindings (checked {checked})"
    );
    assert!(
        mismatches.is_empty(),
        "Vulkan constants disagree with vulkan_core.h:\n{}",
        mismatches.join("\n")
    );
}
