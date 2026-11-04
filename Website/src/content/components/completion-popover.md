# CompletionPopover

补全列表不抢走编辑器焦点。最多显示十项，选中项上方保留五项上下文；可显示详情、文档和位置计数。输入框管理方向键、Return、Tab 和 Escape。

```rust
use cove_ui_compose::primitives::controls::{CompletionPopover, TextCompletionItem};

let mut item = TextCompletionItem::new("print", "print", "print(value)");
item.detail = Some("Writes a value to standard output".into());
let list = CompletionPopover::new(vec![item], 0).on_accept(|item| {
    let _inserted = item.insert_text;
});
```

Swift 没有单独的同名 Gallery story。本页以 Swift `CompletionPopover.swift` 为组件基准，补充真实输入框接入及二十项滚动示例；Text Editor 页保留 Swift 原有 print / private 候选。

共享 Rust 实现适用于所有宿主。结构测试与 macOS 原生操作验收分别记录在 `Docs/gallery-audit.md`；输入事件注入不代表原生键盘或 IME 已验收。
