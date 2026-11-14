//! Data story: Markdown source editing and rendering.
//!
//! Swift source: `GalleryContent/Stories/Data/MarkdownStory.swift`.
//!
//! Three authored documents feed one `MarkdownView`: a full release-notes
//! document, a compact install snippet, and an editable draft. The parser is
//! exercised separately; what this story pins is that each document survives
//! parsing with the block structure it advertises.

use cove_composition::Scope;
use cove_core::State;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::layout::{ScrollAxis, ScrollView};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::text::markdown::markdown_view::MarkdownView;
use cove_ui_compose::primitives::text::markdown::parse::blocks;
use cove_ui_compose::theme::tokens::SemanticColorRef;
use cove_ui_compose::{AnyView, BoxView, TupleView, View};

use super::feedback::StorySection;
use crate::gallery_content::catalog::GalleryOptions;

/// The full release-notes document, including a task list, a quote, a code
/// block, a table and a rule.
pub const DOCUMENT: &str = r#"# Release notes

CoveUI **0.10** brings *rich text* to every surface. Paragraphs mix `inline code`, ~~old names~~ and [links](https://github.com/swiftlang/swift-markdown) in one flow, and long lines wrap at the container width.

## What changed

1. Markdown parsing through swift-markdown (CommonMark + GFM)
2. Code blocks keep their lines and scroll sideways
3. Tables size columns to their content

- Bullets nest
  - with a lighter marker
    - three levels deep
- [x] Task lists show their state
- [ ] Unchecked items stay muted

> Quotes keep a rule on the leading edge and a quieter colour,
> even across **several** lines.

```swift
let view = MarkdownView(notes) { destination in router.open(destination) }
```

| Block | Phone | Desktop |
| :--- | :---: | ---: |
| Paragraph | wraps | wraps |
| Code | scrolls | scrolls |
| Table | scrolls | fits |

---

中文段落同样按字断行：**粗体**、*斜体* 与 `代码` 混排。
"#;

/// The compact install snippet.
pub const COMPACT: &str = r#"### Install
Run `cove build` then open the **Gallery**:
```sh
cove run CoveUI/Examples --mac --devtools -- --component markdown --width 390
```"#;

/// The two-item draft the editor starts with.
pub const EDITABLE: &str = r#"### Notes
Markdown is edited with the same text field as every other format.
- one
- two"#;

/// The line height the editor uses, in points.
pub const EDITOR_LINE_HEIGHT: f32 = 20.0;
/// The editor's indentation width, in spaces.
pub const EDITOR_INDENTATION: usize = 2;

/// `MarkdownStory`: an editor over one draft and a rendered document.
pub struct MarkdownStory {
    pub options: GalleryOptions,
    draft: State<String>,
    last_opened: State<String>,
}

impl MarkdownStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            draft: State::new(EDITABLE.to_owned()),
            last_opened: State::new("none".to_owned()),
        }
    }

    pub fn draft(&self) -> String {
        self.draft.get()
    }

    pub fn set_draft(&self, text: impl Into<String>) {
        self.draft.set(text.into());
    }

    pub fn last_opened(&self) -> String {
        self.last_opened.get()
    }

    /// Records the link the reader opened.
    pub fn record_opened(&self, destination: impl Into<String>) {
        self.last_opened.set(destination.into());
    }

    /// How many blocks each document parses into.
    pub fn block_count(&self, source: &str) -> usize {
        blocks(source).len()
    }
}

impl View for MarkdownStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let opened = self.last_opened.clone();
        let viewer = MarkdownView::new(DOCUMENT).on_open_link(move |destination| {
            opened.set(destination.to_owned());
        });

        let caption = Text::new(format!("Last opened: {}", self.last_opened()))
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted());

        AnyView::new(BoxView::new(
            cove_core::scene::FlexDirection::Column,
            cove_core::scene::Align::Stretch,
            cove_core::scene::Justify::FlexStart,
            cove_core::scene::Wrap::NoWrap,
            24.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(StorySection::new(
                    "Source editing",
                    "Markdown uses the same TextField as every other text format.",
                    AnyView::new(Text::new(self.draft())),
                )),
                AnyView::new(StorySection::new(
                    "Document",
                    "Headings, inline styles, nested and task lists, quotes, a scrolling code block, a table and a rule — rendered from one Markdown string.",
                    AnyView::new(TupleView::new(vec![
                        // The rendered document grows to its content (roughly
                        // 685pt here), so the 420pt frame is a scroll viewport,
                        // not a clamp: without the scroll view the overflow
                        // paints straight through the card and caption.
                        AnyView::new(
                            AnyView::new(ScrollView::new(
                                ScrollAxis::Vertical,
                                AnyView::new(viewer),
                            ))
                            .frame_height(420.0),
                        ),
                        AnyView::new(caption),
                    ])),
                )),
            ])),
        ))
    }

    fn tag(&self) -> &'static str {
        "MarkdownStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; draft, last_opened)
    }
}
