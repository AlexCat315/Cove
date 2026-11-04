# SplitButton

主按钮执行默认操作，箭头打开共享菜单。整个控件只有一个 Tab 停靠点；左右键在两个区域之间移动，停靠点随焦点更新。禁用会阻止两个区域。

```rust
use cove_ui_compose::primitives::controls::SplitButton;
use cove_ui_compose::primitives::overlays::menu::{MenuEntry, MenuItem};

let save = SplitButton::new("Save", vec![
    MenuEntry::Item(MenuItem::new(1, "Save as…", || {})),
    MenuEntry::Item(MenuItem::new(2, "Export", || {})),
], || {}).menu_width(200.0);
```

Gallery 对应 Swift `ControlStories.swift` 的 Save / Save as / Export / Discard，结果由同一状态输出。

共享 Rust 实现适用于所有宿主。结构测试与 macOS 原生操作验收分别记录在 `Docs/gallery-audit.md`；输入事件注入不代表原生键盘或 IME 已验收。
