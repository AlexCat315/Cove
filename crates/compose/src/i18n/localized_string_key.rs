//! Typed localized string lookup with a readable key fallback.
//!
//! A key is either namespaced (`calendar.month.1`) or the English text itself
//! (`"Resize sheet"`). Resolution order, first hit wins:
//!
//! 1. the bundle a caller pinned with [`LocalizedStringKey::new`],
//! 2. the catalogs embedded from `resources/Locales`,
//! 3. resource files beside the executable,
//! 4. the English catalog, then the key unchanged.
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The catalogs compiled into the crate, keyed by language subtag.
const BUILTIN_CATALOGS: [(&str, &str); 2] = [
    ("en", include_str!("../../resources/Locales/en.json")),
    ("zh", include_str!("../../resources/Locales/zh-Hans.json")),
];

thread_local! {
    /// A per-thread locale that beats the process environment.
    ///
    /// Tests pin `en` so assertions do not depend on `LANG`; an application
    /// leaves it unset and follows the platform locale.
    static LOCALE_OVERRIDE: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Pins the locale for the current thread; `None` restores the environment.
pub fn set_locale_override(locale: Option<&str>) {
    LOCALE_OVERRIDE.with(|slot| *slot.borrow_mut() = locale.map(str::to_owned));
}

/// The locale pinned for the current thread, if any.
pub fn locale_override() -> Option<String> {
    LOCALE_OVERRIDE.with(|slot| slot.borrow().clone())
}

/// Resolves `key` for the current locale.
pub fn tr(key: &str) -> String {
    LocalizedStringKey::literal(key).resolved()
}

/// Substitutes `{name}` placeholders in a localized template.
pub fn format_template(template: &str, values: &[(&str, &str)]) -> String {
    let mut output = template.to_owned();
    for (name, value) in values {
        output = output.replace(&format!("{{{name}}}"), value);
    }
    output
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalizedStringKey {
    pub key: String,
    pub bundle: Option<PathBuf>,
    pub table: Option<String>,
}

impl LocalizedStringKey {
    pub fn new(key: impl Into<String>, bundle: Option<PathBuf>, table: Option<String>) -> Self {
        Self {
            key: key.into(),
            bundle,
            table,
        }
    }

    pub fn literal(key: impl Into<String>) -> Self {
        Self::new(key, None, None)
    }

    pub fn resolved(&self) -> String {
        self.resolved_for(&preferred_locales())
    }

    /// Resolves against an explicit locale chain, for tests and callers that
    /// already know the locale.
    pub fn resolved_for(&self, locales: &[String]) -> String {
        let roots = self
            .bundle
            .clone()
            .map(|root| vec![root])
            .unwrap_or_else(main_resource_roots);
        let pinned = self.bundle.is_some() || self.table.is_some();
        if pinned
            && let Some(value) = lookup_files(&roots, self.table.as_deref(), locales, &self.key)
        {
            return value;
        }
        for locale in locales {
            if let Some(value) = builtin_lookup(locale, &self.key) {
                return value;
            }
        }
        if !pinned && let Some(value) = lookup_files(&roots, None, locales, &self.key) {
            return value;
        }
        builtin_lookup("en", &self.key).unwrap_or_else(|| self.key.clone())
    }
}

impl From<&str> for LocalizedStringKey {
    fn from(value: &str) -> Self {
        Self::literal(value)
    }
}
impl From<String> for LocalizedStringKey {
    fn from(value: String) -> Self {
        Self::literal(value)
    }
}

fn builtin_catalogs() -> &'static BTreeMap<&'static str, BTreeMap<String, String>> {
    static CATALOGS: OnceLock<BTreeMap<&'static str, BTreeMap<String, String>>> = OnceLock::new();
    CATALOGS.get_or_init(|| {
        BUILTIN_CATALOGS
            .iter()
            .map(|(language, source)| {
                let entries = serde_json::from_str(source).unwrap_or_default();
                (*language, entries)
            })
            .collect()
    })
}

/// Looks `key` up in the embedded catalog for `locale`'s language subtag.
fn builtin_lookup(locale: &str, key: &str) -> Option<String> {
    let language = locale
        .split('-')
        .next()
        .unwrap_or(locale)
        .to_ascii_lowercase();
    builtin_catalogs()
        .get(language.as_str())
        .and_then(|catalog| catalog.get(key))
        .cloned()
}

/// Reads `key` from the resource files of every root, in locale order.
fn lookup_files(
    roots: &[PathBuf],
    table: Option<&str>,
    locales: &[String],
    key: &str,
) -> Option<String> {
    let table = table.unwrap_or("Localizable");
    for bundle in roots {
        for locale in locales {
            let path = bundle
                .join(format!("{locale}.lproj"))
                .join(format!("{table}.strings"));
            if let Some(value) = read_translation(&path, key) {
                return Some(value);
            }
        }
        for package_bundle in ["CoveUICompose.resources", "CoveUICompose.bundle"] {
            let bundle = bundle.join(package_bundle);
            for locale in locales {
                if let Some(value) = read_translation(
                    &bundle
                        .join(format!("{locale}.lproj"))
                        .join(format!("{table}.strings")),
                    key,
                ) {
                    return Some(value);
                }
            }
        }
        for name in [format!("{table}.strings"), format!("{table}.json")] {
            if let Some(value) = read_translation(&bundle.join(name), key) {
                return Some(value);
            }
        }
    }
    None
}

fn preferred_locales() -> Vec<String> {
    if let Some(locale) = LOCALE_OVERRIDE.with(|slot| slot.borrow().clone()) {
        return expand_locale(&locale);
    }
    let raw = std::env::var("LC_ALL")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var("LC_MESSAGES").ok().filter(|v| !v.is_empty()))
        .or_else(|| std::env::var("LANG").ok().filter(|v| !v.is_empty()))
        .unwrap_or_else(|| "en".into());
    expand_locale(&raw)
}

/// `zh_CN.UTF-8` becomes `["zh-CN", "zh"]`.
fn expand_locale(raw: &str) -> Vec<String> {
    let locale = raw
        .split(['.', '@'])
        .next()
        .unwrap_or(raw)
        .replace('_', "-");
    let mut locales = vec![locale.clone()];
    if let Some(language) = locale.split('-').next()
        && language != locale
    {
        locales.push(language.to_owned());
    }
    locales
}

fn main_resource_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(path) = std::env::var_os("PACKAGE_RESOURCE_BUNDLE_PATH") {
        roots.push(path.into());
    }
    if let Ok(executable) = std::env::current_exe()
        && let Some(parent) = executable.parent()
    {
        roots.push(parent.to_owned());
        roots.push(parent.join("Resources"));
        if parent.file_name().is_some_and(|name| name == "MacOS")
            && let Some(contents) = parent
                .parent()
                .filter(|path| path.file_name().is_some_and(|name| name == "Contents"))
        {
            roots.push(contents.join("Resources"));
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
        let units = bytes[2..].as_chunks::<2>().0.iter().map(|pair| {
            if little_endian {
                u16::from_le_bytes([pair[0], pair[1]])
            } else {
                u16::from_be_bytes([pair[0], pair[1]])
            }
        });
        return Some(
            char::decode_utf16(units)
                .map(|result| result.unwrap_or(char::REPLACEMENT_CHARACTER))
                .collect(),
        );
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
        if !parser.consume('=') {
            break;
        }
        parser.skip_trivia();
        let Some(value) = parser.quoted() else { break };
        parser.skip_trivia();
        if !parser.consume(';') {
            break;
        }
        values.insert(key, value);
    }
    values
}

struct StringsParser<'a> {
    source: &'a str,
    offset: usize,
}
impl StringsParser<'_> {
    fn consume(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.offset += expected.len_utf8();
            true
        } else {
            false
        }
    }
    fn peek(&self) -> Option<char> {
        self.source.get(self.offset..)?.chars().next()
    }
    fn skip_trivia(&mut self) -> bool {
        loop {
            while self.peek().is_some_and(char::is_whitespace) {
                let Some(character) = self.peek() else {
                    return false;
                };
                self.offset += character.len_utf8();
            }
            let Some(rest) = self.source.get(self.offset..) else {
                return false;
            };
            if rest.starts_with("//") {
                self.offset += rest.find('\n').unwrap_or(rest.len());
            } else if rest.starts_with("/*") {
                let Some(end) = rest.find("*/").map(|index| index + 2) else {
                    return false;
                };
                self.offset += end;
            } else {
                return true;
            }
        }
    }
    fn quoted(&mut self) -> Option<String> {
        if !self.consume('"') {
            return None;
        }
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
                            output
                                .push(char::from_u32(code).unwrap_or(char::REPLACEMENT_CHARACTER));
                        }
                        '\n' => {}
                        '\r' => {
                            self.consume('\n');
                        }
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
            if self.consume('\\')
                && (self.consume('u') || self.consume('U'))
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
            let Some(digit) = character.to_digit(8) else {
                break;
            };
            self.offset += character.len_utf8();
            value = value.checked_mul(8)?.checked_add(digit)?;
        }
        Some(value)
    }
}

#[cfg(test)]
mod checks {
    use super::*;

    fn zh() -> Vec<String> {
        vec!["zh-Hans".to_string(), "zh".to_string()]
    }

    fn en() -> Vec<String> {
        vec!["en".to_string()]
    }

    #[test]
    fn a_namespaced_key_resolves_per_locale() {
        let key = LocalizedStringKey::literal("calendar.month.1");
        assert_eq!(key.resolved_for(&zh()), "1月");
        assert_eq!(key.resolved_for(&en()), "January");
    }

    #[test]
    fn a_text_key_resolves_per_locale() {
        let key = LocalizedStringKey::literal("Scrollbar");
        assert_eq!(key.resolved_for(&zh()), "滚动条");
        assert_eq!(key.resolved_for(&en()), "Scrollbar");
    }

    #[test]
    fn a_language_subtag_finds_the_catalog() {
        let key = LocalizedStringKey::literal("calendar.weekday.3");
        assert_eq!(
            key.resolved_for(&["zh-CN".to_string(), "zh".to_string()]),
            "三"
        );
    }

    #[test]
    fn an_unknown_locale_falls_back_to_english() {
        let key = LocalizedStringKey::literal("calendar.month.5");
        assert_eq!(key.resolved_for(&["ja-JP".to_string()]), "May");
    }

    #[test]
    fn an_unknown_key_survives_unchanged() {
        let key = LocalizedStringKey::literal("not.a.key");
        assert_eq!(key.resolved_for(&zh()), "not.a.key");
    }

    #[test]
    fn both_catalogs_carry_the_same_keys() {
        let catalogs = builtin_catalogs();
        let english = catalogs.get("en").expect("the English catalog is embedded");
        let chinese = catalogs.get("zh").expect("the Chinese catalog is embedded");
        let mut missing: Vec<&String> = english
            .keys()
            .filter(|key| !chinese.contains_key(*key))
            .collect();
        missing.sort();
        assert!(missing.is_empty(), "untranslated keys: {missing:?}");
    }

    #[test]
    fn every_calendar_key_is_present() {
        let catalogs = builtin_catalogs();
        let english = catalogs.get("en").expect("the English catalog is embedded");
        for month in 1..=12 {
            for key in [
                format!("calendar.month.{month}"),
                format!("calendar.month_short.{month}"),
            ] {
                assert!(english.contains_key(&key), "{key} is missing");
            }
        }
        for weekday in 0..=6 {
            let key = format!("calendar.weekday.{weekday}");
            assert!(english.contains_key(&key), "{key} is missing");
        }
    }

    #[test]
    fn a_template_substitutes_every_placeholder() {
        set_locale_override(Some("zh-Hans"));
        let template = tr("calendar.month_title");
        set_locale_override(None);
        assert_eq!(
            format_template(&template, &[("month", "1月"), ("year", "2026")]),
            "2026年1月"
        );
        assert_eq!(
            format_template(
                "{month} {day}, {year}",
                &[("month", "May"), ("day", "5"), ("year", "2026")]
            ),
            "May 5, 2026"
        );
    }

    #[test]
    fn the_thread_locale_beats_the_environment() {
        set_locale_override(Some("zh-Hans"));
        assert_eq!(tr("calendar.month.1"), "1月");
        set_locale_override(Some("en"));
        assert_eq!(tr("calendar.month.1"), "January");
        set_locale_override(None);
        // Whatever `LANG` says, the key still resolves to something non-empty.
        assert!(!tr("calendar.month.1").is_empty());
    }

    #[test]
    fn a_pinned_bundle_reads_its_strings_file() {
        let directory = tempfile::tempdir().expect("a temp directory");
        let locale = directory.path().join("zh-Hans.lproj");
        std::fs::create_dir_all(&locale).expect("the locale directory");
        std::fs::write(
            locale.join("Localizable.strings"),
            "\"Scrollbar\" = \"来自应用\";\n",
        )
        .expect("the strings file");
        let key = LocalizedStringKey::new("Scrollbar", Some(directory.path().to_path_buf()), None);
        assert_eq!(
            key.resolved_for(&zh()),
            "来自应用",
            "an app-supplied bundle overrides the embedded catalog"
        );
    }
}
