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

use super::showcase::{note, row, stack};
use cove_composition::Scope;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_core::{Binding, State};
use cove_core::{Font, FontWeight, Point};
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use cove_ui_compose::modifiers::style::AppearanceExt;
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::controls::{Button, Checkbox};
use cove_ui_compose::primitives::layout::ResizableEditor;
use cove_ui_compose::primitives::media::{Icon, ui_icon_catalog as icons};
use cove_ui_compose::primitives::overlays::tooltip::{Tooltip, TooltipAnchor};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::text::code::{
    folding_options::{TextEditorFolding, TextFoldRegion},
    gutter_options::{TextBreakpoint, TextEditorGutter, TextGutterMarker},
};
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_diagnostic::{
    TextDiagnostic, TextDiagnosticSeverity, TextDiagnostics,
};
use cove_ui_compose::primitives::text::text_field::text_field_observation::TextFieldHoverAnchor;
use cove_ui_compose::primitives::text::text_field::{
    TextField, TextFieldAxis, TextFieldBehavior, TextFieldCaretState, TextFieldCodeEditing,
    TextFieldEvents, TextFieldLayout, TextFieldNavigationRequest,
};
use cove_ui_compose::theme::styles::components::{AnyTextFieldStyle, EditorTextFieldStyle};
use cove_ui_compose::theme::{SemanticColorRef, SemanticFontRef};
use cove_ui_compose::{AnyView, LayoutDebugExt, View};
use cove_ui_compose::{BoxView, TupleView};
use std::rc::Rc;

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
    source: State<TextBuffer>,
    caret: State<TextFieldCaretState>,
    hover: State<Option<TextFieldHoverAnchor>>,
    navigation: State<TextFieldNavigationRequest>,
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
            source: State::new(TextBuffer::new(SAMPLE_SOURCE)),
            caret: State::new(TextFieldCaretState::INITIAL),
            hover: State::new(None),
            navigation: State::new(TextFieldNavigationRequest::default()),
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
        self.source.get().string_value()
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
        let lexical = tokens(&source);
        self.fold_regions.set(folds(&source, &lexical));
        self.tokens.set(lexical);
        self.source.set(TextBuffer::new(&source));
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
        let lexical = tokens(&source);
        self.fold_regions.set(folds(&source, &lexical));
        self.tokens.set(lexical);
        self.source.set(TextBuffer::new(&source));
        self.breakpoints.set(Vec::new());
        self.fold_regions.set(Vec::new());
        let buffer = self.source.get();
        let index = buffer.line_range(count / 2).end;
        let request = self.navigation.get().caret_request_id.unwrap_or(0) + 1;
        self.navigation.set(TextFieldNavigationRequest {
            focus_request_id: Some(request),
            caret_request_id: Some(request),
            caret_request_index: Some(index),
        });
        self.status.set(format!(
            "{count} lines · caret at line {} · type, scroll, undo and redo",
            count / 2 + 1
        ));
    }

    /// The line count of the current document.
    pub fn line_count(&self) -> usize {
        self.source.get().line_count()
    }

    /// The sample diagnostic: a warning spanning the first line.
    ///
    /// The range is computed from that line's byte extent, so it tracks edits on
    /// line 0 rather than assuming a fixed length.
    pub fn diagnostic(&self) -> (std::ops::Range<usize>, &'static str) {
        let source = self.source.get();
        let range = source.line_range(0);
        (
            source.utf8_offset_for_character_index(range.start)
                ..source.utf8_offset_for_character_index(range.end),
            "Sample warning: this declaration is unused.",
        )
    }

    /// The folds this document would produce, which is what code mode shows.
    pub fn computed_folds(&self) -> Vec<FoldRegion> {
        folds(&self.source(), &self.tokens.get())
    }

    pub fn set_code_mode(&self, code: bool) {
        self.is_code_mode.set(code);
    }

    pub fn toggle_bookmark(&self) {
        self.is_bookmarked.update(|value| *value = !*value);
    }
}

impl View for TextEditorStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let enabled = self.options.is_enabled;
        let code_mode = self.is_code_mode();
        let token_state = self.tokens.clone();
        let folds_state = self.fold_regions.clone();
        let caret = self.caret.clone();
        let hover = self.hover.clone();
        let edit_hover = self.hover.clone();
        let navigation = self.navigation.clone();
        let jump = Button::title("Go to beginning", move || {
            let id = navigation.get().caret_request_id.unwrap_or(0) + 1;
            navigation.set(TextFieldNavigationRequest {
                focus_request_id: Some(id),
                caret_request_id: Some(id),
                caret_request_index: Some(0),
            });
        })
        .enabled(enabled);
        let source = self.source.clone();
        let lexical = self.tokens.clone();
        let regions = self.fold_regions.clone();
        let load_navigation = self.navigation.clone();
        let status = self.status.clone();
        let stress_breakpoints = self.breakpoints.clone();
        let stress = Button::title("Load 200,000 lines", move || {
            let text = (0..STRESS_LINES)
                .map(|i| format!("let value{i} = {i}"))
                .collect::<Vec<_>>()
                .join("\n");
            let buffer = TextBuffer::new(&text);
            let index = buffer.line_range(STRESS_TARGET_LINE).end;
            source.set(buffer);
            lexical.set(Vec::new());
            regions.set(Vec::new());
            stress_breakpoints.set(Vec::new());
            hover.set(None);
            let id = load_navigation.get().caret_request_id.unwrap_or(0) + 1;
            load_navigation.set(TextFieldNavigationRequest {
                focus_request_id: None,
                caret_request_id: Some(id),
                caret_request_index: Some(index),
            });
            status
                .set("200,000 lines · caret at line 100,001 · type, scroll, undo and redo".into());
        })
        .enabled(enabled && code_mode);
        let mut markers = Vec::new();
        if self.is_bookmarked() && self.source.get().line_count() > 1 {
            let bookmark = self.is_bookmarked.clone();
            let status = self.status.clone();
            markers.push(TextGutterMarker::new(
                "bookmark",
                1,
                "Remove bookmark",
                icons::status::bookmark(),
                None,
                move || {
                    bookmark.set(false);
                    status.set("Bookmark removed".into());
                },
            ));
        }
        if let Some(function) = self
            .fold_regions()
            .into_iter()
            .find(|region| region.first_line > 0)
        {
            let status = self.status.clone();
            markers.push(TextGutterMarker::new(
                "run",
                function.first_line,
                "Preview Greeting.message",
                icons::editor::play(),
                None,
                move || status.set("Greeting.message preview: Hello, Cove".into()),
            ));
        }
        if self.shows_diagnostic() {
            let (_, message) = self.diagnostic();
            let status = self.status.clone();
            let navigation = self.navigation.clone();
            markers.push(TextGutterMarker::new(
                "warning",
                0,
                message,
                icons::status::alert_triangle(),
                None,
                move || {
                    status.set(message.into());
                    let id = navigation.get().caret_request_id.unwrap_or(0) + 1;
                    navigation.set(TextFieldNavigationRequest {
                        focus_request_id: None,
                        caret_request_id: Some(id),
                        caret_request_index: Some(0),
                    });
                },
            ));
        }
        let breakpoints = self.breakpoints.clone();
        let write_breakpoints = self.breakpoints.clone();
        let gutter = TextEditorGutter {
            breakpoints: Some(Binding::new(
                move || {
                    breakpoints
                        .get()
                        .into_iter()
                        .map(TextBreakpoint::new)
                        .collect()
                },
                move |values: Vec<TextBreakpoint>| {
                    write_breakpoints.set(values.into_iter().map(|v| v.line).collect())
                },
            )),
            execution_line: self.paused_preview().then_some(4),
            markers,
            ..TextEditorGutter::new()
        };
        let mut code = TextFieldCodeEditing::default();
        if code_mode {
            code.shows_line_numbers = true;
            code.indentation_width = Some(4);
            code.gutter = gutter;
            code.folding = TextEditorFolding {
                regions: self
                    .fold_regions()
                    .into_iter()
                    .map(|v| TextFoldRegion::new(v.first_line, v.last_line))
                    .collect(),
                ..TextEditorFolding::new()
            };
            if self.shows_diagnostic() {
                let (range, message) = self.diagnostic();
                code.diagnostics = TextDiagnostics::new(&[TextDiagnostic::new(
                    range,
                    TextDiagnosticSeverity::Warning,
                    message,
                )]);
            }
            code.syntax_revision = Some(self.source.get().identity() as u64);
            code.on_request_completion = Some(Rc::new(|_, reply| {
                use cove_ui_compose::primitives::text::text_completion_rebase::TextCompletionItem;
                let mut print = TextCompletionItem::new("print", "print", "print(value)");
                print.detail = Some("Writes a value to standard output".into());
                let mut private = TextCompletionItem::new("private", "private", "private ");
                private.detail = Some("Access control".into());
                reply(vec![print, private]);
            }));
            let status = self.status.clone();
            code.on_accept_completion = Some(Rc::new(move |item| {
                status.set(format!("Accepted {}", item.label))
            }));
        }
        let mut field = TextField::new(
            if code_mode {
                "Swift source"
            } else {
                "Plain text document"
            },
            self.source.binding(),
        )
        .layout(TextFieldLayout {
            axis: TextFieldAxis::Vertical,
            max_visible_lines: 24,
            wraps_lines: !code_mode,
            ..TextFieldLayout::default()
        })
        .behavior(TextFieldBehavior {
            disabled: !enabled,
            ..TextFieldBehavior::default()
        })
        .style(AnyTextFieldStyle::new(EditorTextFieldStyle))
        .code_editing(code)
        .events(TextFieldEvents {
            on_change: Some(Rc::new(move |buffer| {
                edit_hover.set(None);
                if buffer.utf8_length() > SYNTAX_LIMIT {
                    folds_state.set(Vec::new());
                    token_state.set(Vec::new());
                } else {
                    let source = buffer.string_value();
                    let lexical = tokens(&source);
                    folds_state.set(folds(&source, &lexical));
                    token_state.set(lexical);
                }
            })),
            on_caret_change: Some(Rc::new(move |value| {
                if caret.get() != value {
                    caret.set(value);
                }
            })),
            on_hover_change: Some(Rc::new({
                let hover = self.hover.clone();
                move |value| {
                    if hover.get() != value {
                        hover.set(value);
                    }
                }
            })),
            ..TextFieldEvents::default()
        });
        field.navigation = self.navigation.get();
        let lexical = self.tokens.get();
        let field = cove_ui_compose::theme::ThemeReader::new(move |theme| {
            let mut field = field.clone();
            if code_mode {
                let lexical = lexical.clone();
                let light = theme.colors.background.r > 0.5;
                field.code_editing.syntax_color_at_utf8_offset = Some(Rc::new(move |_, offset| {
                    token_at(offset, &lexical).map(|t| {
                        let (r, g, b) = token_color(t.kind, light);
                        cove_core::Color::from_rgba8(r, g, b, 255)
                    })
                }));
            }
            let font = if code_mode {
                Font::monospaced(15.0, FontWeight::Regular)
            } else {
                theme.typography.body.font.clone()
            };
            AnyView::new(cove_ui_compose::adaptive::SurfaceReader::new(
                move |surface| {
                    let mut field = field.clone();
                    field.layout.wraps_lines = !code_mode
                        || surface.viewport.width_class() == cove_core::SizeClass::Compact;
                    AnyView::new(
                        AnyView::new(field)
                            .font(font.clone())
                            .line_height(if code_mode { 24.0 } else { 22.0 }),
                    )
                },
            ))
        });
        let text = self.source.get();
        let (line, column) =
            text.line_and_column(self.caret.get().caret_index.min(text.character_count()));
        let (line, column) = (line + 1, column + 1);
        let code_state = self.is_code_mode.binding();
        let header = BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::Wrap,
            8.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(Icon::new(icons::editor::code(), 16.0)),
                AnyView::new(
                    Text::new(if code_mode {
                        "Greeting.swift"
                    } else {
                        "Notes.txt"
                    })
                    .truncated()
                    .semantic_font(SemanticFontRef::label())
                    .flex_with(1.0, 1.0, Some(0.0))
                    .frame_min_width(60.0),
                ),
                AnyView::new(
                    Checkbox::labeled_from_boolean("Code mode", code_state).enabled(enabled),
                ),
            ])),
        )
        .padding_edges(14.0, 8.0)
        .semantic_background(SemanticColorRef::surface_raised());
        let status_bar = row(vec![
            AnyView::new(note(format!("Ln {line}, Col {column}")).debug_name("text-editor-caret")),
            note(if code_mode { "Spaces: 4" } else { "Soft wrap" }),
            note("UTF-8"),
            note(if code_mode { "Swift" } else { "Plain text" }),
        ])
        .padding_edges(14.0, 6.0)
        .semantic_background(SemanticColorRef::surface_raised());
        let document = AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(header),
                    AnyView::new(
                        ResizableEditor::new(
                            AnyView::new(field)
                                .frame(None, None, None, Some(0.0), None, None)
                                .flex(1.0)
                                .debug_name("gallery-text-editor"),
                        )
                        .initial_height(320.0)
                        .resizable(enabled)
                        .resize_label("Editor height"),
                    ),
                    AnyView::new(status_bar),
                ])),
            )
            .semantic_background(SemanticColorRef::surface_sunken())
            .corner_radius(8.0),
        );
        let document = cove_ui_compose::theme::ThemeReader::new(move |theme| {
            AnyView::new(document.clone().border_color(theme.colors.border, 1.0))
        });
        let mut controls = vec![AnyView::new(jump), AnyView::new(stress)];
        if code_mode {
            controls.push(AnyView::new(
                Checkbox::labeled_from_boolean("Sample warning", self.shows_diagnostic.binding())
                    .enabled(enabled),
            ));
            controls.push(AnyView::new(
                Checkbox::labeled_from_boolean(
                    "Paused line preview",
                    self.paused_preview.binding(),
                )
                .enabled(enabled),
            ));
        }
        let mut content = vec![
            row(controls),
            AnyView::new(document),
            AnyView::new(note(self.status()).debug_name("text-editor-status")),
            note(
                "F9 toggles a breakpoint · Primary-Alt-Left/Right folds or expands · gutter icons expose named actions · touch uses a line menu",
            ),
        ];
        if code_mode
            && self.shows_diagnostic()
            && let Some(hover) = self.hover.get()
            && text.line_index_for_character_index(hover.character_index) == 0
        {
            let (_, message) = self.diagnostic();
            content.push(AnyView::new(
                Tooltip::anchored(
                    TooltipAnchor::Point(Point {
                        x: hover.window_x,
                        y: hover.window_y,
                    }),
                    note(message)
                        .padding(10.0)
                        .semantic_background(SemanticColorRef::surface_floating()),
                )
                .presented(true),
            ));
        }
        AnyView::new(StorySection::new(
            "Text editing",
            "A multiline editor for prose and code. Code mode adds line numbers, indentation, diagnostics and completion; plain mode soft-wraps and removes code chrome.",
            stack(content),
        ))
    }

    fn tag(&self) -> &'static str {
        "TextEditorStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; source, caret, hover, navigation, status, is_code_mode, shows_diagnostic, paused_preview, is_bookmarked, breakpoints, fold_regions, tokens)
    }
}
