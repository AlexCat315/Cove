//! Controls story: the multiline text editor.
//!
//! Swift source: `GalleryContent/Stories/Controls/TextEditorStory.swift`.
//!
//! A general multiline editor. The same `TextField` supports prose (soft wrap, no
//! chrome) and code (line numbers, diagnostics, completion) without changing
//! component.
//!
//! The story carries its own small Swift lexer, because production script editing
//! uses Tree-sitter and LSP while this showcase must not pull a parser in. That
//! lexer is the interesting logic here: it is byte-oriented, guards on document
//! size, and its brace-fold scan deliberately ignores braces inside strings and
//! comments.

use cove_core::State;
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::SemanticColorRef;
use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView, View};

use super::feedback::StorySection;
use crate::gallery_content::catalog::GalleryOptions;

/// The document the editor opens with.
pub const SAMPLE_SOURCE: &str = "struct Greeting {\n    let name: String\n\n    func message() -> String {\n        return name\n    }\n}\n";

/// The line count of the stress sample.
pub const STRESS_LINES: usize = 200_000;
/// The line the stress sample scrolls to (0-based), reported as line 100,001.
pub const STRESS_TARGET_LINE: usize = 100_000;
/// Documents larger than this are not tokenised at all.
///
/// The 200k-line stress sample must not walk the whole document on every edit;
/// syntax colouring is intentionally a demo-scale feature.
pub const SYNTAX_LIMIT: usize = 300_000;

/// Which colour class a token belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Keyword,
    String,
    Comment,
    Number,
    Type,
}

/// One token's byte range and class.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub start: usize,
    pub end: usize,
    pub kind: TokenKind,
}

impl Token {
    pub fn contains(&self, offset: usize) -> bool {
        offset >= self.start && offset < self.end
    }

    pub fn range(&self) -> std::ops::Range<usize> {
        self.start..self.end
    }
}

/// The Swift keywords the lexer highlights, in byte order so `binary_search`
/// is valid. The Swift original listed `self` before `Self`, which is not byte
/// order, so the list is re-sorted here and the membership test stays a binary
/// search.
pub const KEYWORDS: [&str; 59] = [
    "Self",
    "actor",
    "any",
    "as",
    "async",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "continue",
    "default",
    "defer",
    "deinit",
    "do",
    "else",
    "enum",
    "extension",
    "fallthrough",
    "false",
    "fileprivate",
    "final",
    "for",
    "func",
    "guard",
    "if",
    "import",
    "in",
    "init",
    "inout",
    "internal",
    "is",
    "lazy",
    "let",
    "nil",
    "open",
    "operator",
    "override",
    "private",
    "protocol",
    "public",
    "repeat",
    "required",
    "return",
    "self",
    "some",
    "static",
    "struct",
    "switch",
    "throw",
    "throws",
    "true",
    "try",
    "typealias",
    "unowned",
    "var",
    "weak",
    "where",
    "while",
];

/// Whether `word` is a highlighted keyword.
pub fn is_keyword(word: &str) -> bool {
    KEYWORDS.binary_search(&word).is_ok()
}

fn is_digit(byte: u8) -> bool {
    byte.is_ascii_digit()
}

fn is_word_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_word(byte: u8) -> bool {
    is_word_start(byte) || is_digit(byte)
}

/// Tokenises Swift source.
///
/// Returns nothing above `SYNTAX_LIMIT` bytes, so a huge document costs nothing
/// rather than freezing the editor on every keystroke.
///
/// A line comment runs to the newline but not through it. A string honours
/// backslash escapes, so an escaped quote does not terminate it.
pub fn tokens(source: &str) -> Vec<Token> {
    let bytes = source.as_bytes();
    if bytes.len() > SYNTAX_LIMIT {
        return Vec::new();
    }
    let mut result = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        // Line comment: consume up to, not including, the newline.
        if byte == b'/' && index + 1 < bytes.len() && bytes[index + 1] == b'/' {
            let start = index;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            result.push(Token {
                start,
                end: index,
                kind: TokenKind::Comment,
            });
            continue;
        }
        if byte == b'"' {
            let start = index;
            index += 1;
            while index < bytes.len() {
                if bytes[index] == b'\\' {
                    // Skip the escaped byte; the `min` keeps a trailing
                    // backslash from running past the end.
                    index = (index + 2).min(bytes.len());
                } else if bytes[index] == b'"' {
                    index += 1;
                    break;
                } else {
                    index += 1;
                }
            }
            result.push(Token {
                start,
                end: index,
                kind: TokenKind::String,
            });
            continue;
        }
        if is_digit(byte) {
            let start = index;
            while index < bytes.len() && is_digit(bytes[index]) {
                index += 1;
            }
            result.push(Token {
                start,
                end: index,
                kind: TokenKind::Number,
            });
            continue;
        }
        if is_word_start(byte) {
            let start = index;
            while index < bytes.len() && is_word(bytes[index]) {
                index += 1;
            }
            // Every byte in the range is ASCII, so this slice is a valid `str`.
            let word = &source[start..index];
            if is_keyword(word) {
                result.push(Token {
                    start,
                    end: index,
                    kind: TokenKind::Keyword,
                });
            } else if bytes[start].is_ascii_uppercase() {
                result.push(Token {
                    start,
                    end: index,
                    kind: TokenKind::Type,
                });
            }
            continue;
        }
        index += 1;
    }
    result
}

/// A foldable line span.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FoldRegion {
    pub first_line: usize,
    pub last_line: usize,
}

/// Finds brace-delimited fold regions, ignoring braces inside strings and
/// comments.
///
/// A region is emitted only when the closing brace lands on a later line; a
/// single-line `{ }` is not worth folding.
pub fn folds(source: &str, tokens: &[Token]) -> Vec<FoldRegion> {
    let bytes = source.as_bytes();
    if bytes.len() > SYNTAX_LIMIT {
        return Vec::new();
    }
    // Byte offset → line index, built once so the scan stays linear.
    let mut line_of_byte = vec![0usize; bytes.len() + 1];
    let mut line = 0;
    for (index, byte) in bytes.iter().enumerate() {
        line_of_byte[index] = line;
        if *byte == b'\n' {
            line += 1;
        }
    }
    line_of_byte[bytes.len()] = line;

    let mut stack: Vec<usize> = Vec::new();
    let mut regions = Vec::new();
    let mut token_index = 0;
    for offset in 0..bytes.len() {
        while token_index < tokens.len() && tokens[token_index].end <= offset {
            token_index += 1;
        }
        if token_index < tokens.len()
            && tokens[token_index].contains(offset)
            && matches!(
                tokens[token_index].kind,
                TokenKind::String | TokenKind::Comment
            )
        {
            continue;
        }
        if bytes[offset] == b'{' {
            stack.push(offset);
        } else if bytes[offset] == b'}'
            && let Some(start) = stack.pop()
        {
            let first = line_of_byte[start];
            let last = line_of_byte[offset];
            if first < last {
                regions.push(FoldRegion {
                    first_line: first,
                    last_line: last,
                });
            }
        }
    }
    regions.sort_by_key(|region| region.first_line);
    regions
}

/// The colour a token kind takes in light or dark appearance.
pub fn token_color(kind: TokenKind, is_light: bool) -> (u8, u8, u8) {
    match (kind, is_light) {
        (TokenKind::Keyword, true) => (0x86, 0x3D, 0xA0),
        (TokenKind::Keyword, false) => (0xC6, 0x91, 0xDD),
        (TokenKind::String, true) => (0x36, 0x73, 0x3B),
        (TokenKind::String, false) => (0xA0, 0xC9, 0x85),
        (TokenKind::Comment, true) => (0x6C, 0x75, 0x80),
        (TokenKind::Comment, false) => (0x8B, 0x94, 0xA1),
        (TokenKind::Number, true) => (0x9B, 0x64, 0x20),
        (TokenKind::Number, false) => (0xDC, 0xAC, 0x77),
        (TokenKind::Type, true) => (0x2D, 0x73, 0x9D),
        (TokenKind::Type, false) => (0x78, 0xB9, 0xE8),
    }
}

/// The token covering `offset`, found by binary search.
pub fn token_at(offset: usize, tokens: &[Token]) -> Option<&Token> {
    let mut low = 0;
    let mut high = tokens.len();
    while low < high {
        let middle = (low + high) / 2;
        let token = &tokens[middle];
        if offset < token.start {
            high = middle;
        } else if offset >= token.end {
            low = middle + 1;
        } else {
            return Some(token);
        }
    }
    None
}

/// `TextEditorStory`: prose and code modes over one document.
pub struct TextEditorStory {
    pub options: GalleryOptions,
    source: State<String>,
    status: State<String>,
    is_code_mode: State<bool>,
    shows_diagnostic: State<bool>,
    paused_preview: State<bool>,
    is_bookmarked: State<bool>,
    breakpoints: State<Vec<usize>>,
    fold_regions: State<Vec<FoldRegion>>,
    tokens: State<Vec<Token>>,
}

impl TextEditorStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            source: State::new(SAMPLE_SOURCE.to_owned()),
            status: State::new(
                "Press Control-Space at the end of a line to request completion".to_owned(),
            ),
            is_code_mode: State::new(true),
            shows_diagnostic: State::new(false),
            paused_preview: State::new(false),
            is_bookmarked: State::new(true),
            breakpoints: State::new(vec![3]),
            fold_regions: State::new(vec![
                FoldRegion {
                    first_line: 0,
                    last_line: 6,
                },
                FoldRegion {
                    first_line: 3,
                    last_line: 5,
                },
            ]),
            tokens: State::new(tokens(SAMPLE_SOURCE)),
        }
    }

    pub fn source(&self) -> String {
        self.source.get()
    }

    pub fn status(&self) -> String {
        self.status.get()
    }

    pub fn is_code_mode(&self) -> bool {
        self.is_code_mode.get()
    }

    pub fn shows_diagnostic(&self) -> bool {
        self.shows_diagnostic.get()
    }

    pub fn paused_preview(&self) -> bool {
        self.paused_preview.get()
    }

    pub fn is_bookmarked(&self) -> bool {
        self.is_bookmarked.get()
    }

    pub fn breakpoints(&self) -> Vec<usize> {
        self.breakpoints.get()
    }

    pub fn fold_regions(&self) -> Vec<FoldRegion> {
        self.fold_regions.get()
    }

    pub fn tokens(&self) -> Vec<Token> {
        self.tokens.get()
    }

    /// Replaces the document and re-tokenises it.
    pub fn set_source(&self, source: impl Into<String>) {
        let source = source.into();
        self.tokens.set(tokens(&source));
        self.source.set(source);
    }

    /// Replaces the document with `count` generated lines and jumps the caret.
    ///
    /// Breakpoints and folds are cleared because their line numbers no longer
    /// refer to anything meaningful.
    pub fn load_lines(&self, count: usize) {
        let source = (0..count)
            .map(|index| format!("let value{index} = {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        self.tokens.set(tokens(&source));
        self.source.set(source);
        self.breakpoints.set(Vec::new());
        self.fold_regions.set(Vec::new());
        self.status.set(format!(
            "{count} lines · caret at line {} · type, scroll, undo and redo",
            count / 2 + 1
        ));
    }

    /// The line count of the current document.
    pub fn line_count(&self) -> usize {
        self.source.get().lines().count()
    }

    /// The sample diagnostic: a warning spanning the first line.
    ///
    /// The range is computed from that line's byte extent, so it tracks edits on
    /// line 0 rather than assuming a fixed length.
    pub fn diagnostic(&self) -> (std::ops::Range<usize>, &'static str) {
        let source = self.source.get();
        let end = source.find('\n').unwrap_or(source.len());
        (0..end, "Sample warning: this declaration is unused.")
    }

    /// The folds this document would produce, which is what code mode shows.
    pub fn computed_folds(&self) -> Vec<FoldRegion> {
        folds(&self.source.get(), &self.tokens.get())
    }

    pub fn toggle_bookmark(&self) {
        self.is_bookmarked.update(|value| *value = !*value);
    }
}

impl View for TextEditorStory {
    fn body(&self) -> AnyView {
        let status = Text::new(self.status())
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted());

        let detail = Text::new(
            "A multiline editor for prose and code. Code mode adds line numbers, indentation, diagnostics and completion; plain mode soft-wraps and removes code chrome.",
        )
        .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
        .semantic_foreground(SemanticColorRef::on_surface_muted());

        AnyView::new(StorySection::new(
            "Text editing",
            "Line numbers, diagnostics and completion in code mode; soft wrap with no chrome in plain mode.",
            AnyView::new(BoxView::new(
                cove_core::scene::FlexDirection::Column,
                cove_core::scene::Align::Stretch,
                cove_core::scene::Justify::FlexStart,
                cove_core::scene::Wrap::NoWrap,
                8.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(detail),
                    AnyView::new(AnyView::new(status).debug_name("text-editor-status")),
                ])),
            )),
        ))
    }

    fn tag(&self) -> &'static str {
        "TextEditorStory"
    }
}
