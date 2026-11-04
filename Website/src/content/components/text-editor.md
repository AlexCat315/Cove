# Text Editor

TextField 的垂直模式同时支持正文与代码。代码模式接入行号、语法配色、缩进、断点、折叠、诊断和补全；正文模式软换行。切换模式保留同一文档及编辑节点的撤销历史。

```rust
use cove_core::State;
use cove_ui_compose::primitives::layout::ResizableEditor;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_field::{TextField, TextFieldCodeEditing};

let source = State::new(TextBuffer::new("let value = 1"));
let editor = TextField::new("Swift source", source.binding()).multiline()
    .code_editing(TextFieldCodeEditing {shows_line_numbers:true,
        indentation_width:Some(4), ..Default::default()});
let resizable = ResizableEditor::new(editor).initial_height(320.0);
```

Gallery 对齐 Swift `TextEditorStory.swift` 的文档头、320pt 编辑区、行列状态、Code mode、诊断、暂停行、书签/运行标记、诊断悬停、print/private 补全及 200,000 行导航。大文档超过 300,000 字节后跳过示例词法扫描，行列读取使用 rope 索引。

共享 Rust 实现适用于所有宿主。结构测试与 macOS 原生操作验收分别记录在 `Docs/gallery-audit.md`；输入事件注入不代表原生键盘或 IME 已验收。
