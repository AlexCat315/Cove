//! Typed localized string lookup with a readable key fallback.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalizedStringKey {
    pub key: String,
    pub bundle: Option<PathBuf>,
    pub table: Option<String>,
}

impl LocalizedStringKey {
    pub fn new(key: impl Into<String>, bundle: Option<PathBuf>, table: Option<String>) -> Self {
        Self { key: key.into(), bundle, table }
    }

    pub fn literal(key: impl Into<String>) -> Self {
        Self::new(key, None, None)
    }

    pub fn resolved(&self) -> String {
        let table = self.table.as_deref().unwrap_or("Localizable");
        let roots = self.bundle.clone().map(|root| vec![root]).unwrap_or_else(main_resource_roots);
        for bundle in roots {
            for locale in preferred_locales() {
                let path = bundle.join(format!("{locale}.lproj")).join(format!("{table}.strings"));
                if let Some(value) = read_translation(&path, &self.key) { return value; }
            }
            for package_bundle in ["CoveUICompose.resources", "CoveUICompose.bundle"] {
                let bundle = bundle.join(package_bundle);
                for locale in preferred_locales() {
                    if let Some(value) = read_translation(&bundle.join(format!("{locale}.lproj")).join(format!("{table}.strings")), &self.key) {
                        return value;
                    }
                }
            }
            for name in [format!("{table}.strings"), format!("{table}.json")] {
                if let Some(value) = read_translation(&bundle.join(name), &self.key) { return value; }
            }
        }
        self.key.clone()
    }
}

impl From<&str> for LocalizedStringKey {
    fn from(value: &str) -> Self { Self::literal(value) }
}
impl From<String> for LocalizedStringKey {
    fn from(value: String) -> Self { Self::literal(value) }
}

fn preferred_locales() -> Vec<String> {
    let raw = std::env::var("LC_ALL").ok().filter(|v| !v.is_empty())
        .or_else(|| std::env::var("LC_MESSAGES").ok().filter(|v| !v.is_empty()))
        .or_else(|| std::env::var("LANG").ok().filter(|v| !v.is_empty()))
        .unwrap_or_else(|| "en".into());
    let locale = raw.split(|character| character == '.' || character == '@').next().unwrap_or(&raw).replace('_', "-");
    let mut locales = vec![locale.clone()];
    if let Some(language) = locale.split('-').next() {
        if language != locale { locales.push(language.to_owned()); }
    }
    locales
}

fn main_resource_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(path) = std::env::var_os("PACKAGE_RESOURCE_BUNDLE_PATH") { roots.push(path.into()); }
    if let Ok(executable) = std::env::current_exe() {
        if let Some(parent) = executable.parent() {
            roots.push(parent.to_owned());
            roots.push(parent.join("Resources"));
            if parent.file_name().is_some_and(|name| name == "MacOS") {
                if let Some(contents) = parent.parent().filter(|path| path.file_name().is_some_and(|name| name == "Contents")) {
                    roots.push(contents.join("Resources"));
                }
            }
        }
    }
    roots
}

fn read_translation(path: &Path, key: &str) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    if path.extension().is_some_and(|ext| ext == "json") {
        let catalog: BTreeMap<String, String> = serde_json::from_slice(&bytes).ok()?;
        return catalog.get(key).cloned();
    }
    let source = decode_strings(&bytes)?;
    parse_strings_catalog(&source).remove(key)
}

fn decode_strings(bytes: &[u8]) -> Option<String> {
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        let little_endian = bytes[0] == 0xff;
        let units = bytes[2..].chunks_exact(2).map(|pair| {
            if little_endian { u16::from_le_bytes([pair[0], pair[1]]) } else { u16::from_be_bytes([pair[0], pair[1]]) }
        });
        return Some(char::decode_utf16(units).map(|result| result.unwrap_or(char::REPLACEMENT_CHARACTER)).collect());
    }
    Some(String::from_utf8_lossy(bytes).into_owned())
}

/// Parse the key/value subset of Apple's `.strings` property-list syntax.
fn parse_strings_catalog(source: &str) -> BTreeMap<String, String> {
    let mut parser = StringsParser { source, offset: 0 };
    let mut values = BTreeMap::new();
    while parser.skip_trivia() {
        let Some(key) = parser.quoted() else { break };
        parser.skip_trivia();
        if !parser.consume('=') { break; }
        parser.skip_trivia();
        let Some(value) = parser.quoted() else { break };
        parser.skip_trivia();
        if !parser.consume(';') { break; }
        values.insert(key, value);
    }
    values
}

struct StringsParser<'a> { source: &'a str, offset: usize }
impl StringsParser<'_> {
    fn consume(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) { self.offset += expected.len_utf8(); true } else { false }
    }
    fn peek(&self) -> Option<char> { self.source.get(self.offset..)?.chars().next() }
    fn skip_trivia(&mut self) -> bool {
        loop {
            while self.peek().is_some_and(char::is_whitespace) {
                let Some(character) = self.peek() else { return false };
                self.offset += character.len_utf8();
            }
            let Some(rest) = self.source.get(self.offset..) else { return false };
            if rest.starts_with("//") {
                self.offset += rest.find('\n').unwrap_or(rest.len());
            } else if rest.starts_with("/*") {
                let Some(end) = rest.find("*/").map(|index| index + 2) else { return false };
                self.offset += end;
            } else { return true; }
        }
    }
    fn quoted(&mut self) -> Option<String> {
        if !self.consume('"') { return None; }
        let mut output = String::new();
        loop {
            let ch = self.peek()?;
            self.offset += ch.len_utf8();
            match ch {
                '"' => return Some(output),
                '\\' => {
                    let escape = self.peek()?;
                    self.offset += escape.len_utf8();
                    match escape {
                        'n' => output.push('\n'),
                        'r' => output.push('\r'),
                        't' => output.push('\t'),
                        '"' => output.push('"'),
                        '\\' => output.push('\\'),
                        'u' | 'U' => {
                            output.push(self.unicode_escape()?);
                        }
                        '0'..='7' => {
                            let code = self.octal_escape(escape)?;
                            output.push(char::from_u32(code).unwrap_or(char::REPLACEMENT_CHARACTER));
                        }
                        '\n' => {}
                        '\r' => { self.consume('\n'); }
                        other => output.push(other),
                    }
                }
                other => output.push(other),
            }
        }
    }

    fn hex_escape(&mut self) -> Option<u32> {
        let mut value = 0u32;
        for _ in 0..4 {
            let character = self.peek()?;
            let digit = character.to_digit(16)?;
            self.offset += character.len_utf8();
            value = value.checked_mul(16)?.checked_add(digit)?;
        }
        Some(value)
    }

    fn unicode_escape(&mut self) -> Option<char> {
        let high = self.hex_escape()?;
        if (0xd800..=0xdbff).contains(&high) {
            let after_high = self.offset;
            if self.consume('\\') && (self.consume('u') || self.consume('U'))
                && let Some(low) = self.hex_escape()
                && (0xdc00..=0xdfff).contains(&low)
            {
                let scalar = 0x1_0000 + ((high - 0xd800) << 10) + (low - 0xdc00);
                return char::from_u32(scalar);
            }
            self.offset = after_high;
        }
        Some(char::from_u32(high).unwrap_or(char::REPLACEMENT_CHARACTER))
    }

    fn octal_escape(&mut self, first: char) -> Option<u32> {
        let mut value = first.to_digit(8)?;
        for _ in 1..3 {
            let Some(character) = self.peek() else { break };
            let Some(digit) = character.to_digit(8) else { break };
            self.offset += character.len_utf8();
            value = value.checked_mul(8)?.checked_add(digit)?;
        }
        Some(value)
    }
}
