# TextArea / ResizableTextArea

容器管理高度，TextField 管理文本换行与内部滚动。拖动底部独立手柄改变高度，手柄也提供键盘与辅助功能调整。禁用时同时阻止编辑和调整高度。

```rust
use cove_core::State;
use cove_ui_compose::primitives::layout::ResizableTextArea;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;

let text = State::new(TextBuffer::new("Write a note"));
let area = ResizableTextArea::new("Write a note…", text.binding(),
    90.0, 320.0, false, "Editor height", Default::default(), None, None, None);
```

Gallery 对齐 Swift 的三行样本文本、90…320pt 高度、520pt 最大宽度和字符计数。

共享 Rust 实现适用于所有宿主。结构测试与 macOS 原生操作验收分别记录在 `Docs/gallery-audit.md`；输入事件注入不代表原生键盘或 IME 已验收。
