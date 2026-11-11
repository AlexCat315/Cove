# Popconfirm

`Popconfirm::new(trigger_title, icon, title, message, confirm_title, cancel_title, role, placement, is_enabled, on_confirm)` 在触发器旁弹出确认气泡；`Popconfirm::standard(trigger_title, title, on_confirm)` 提供 Swift 默认参数形式的简写。

```rust
use cove_ui_compose::primitives::overlays::{Popconfirm, PopoverPlacement};
use cove_ui_compose::primitives::controls::ButtonRole;

let confirm = Popconfirm::new(
    "Delete project",
    Some(ui_icon_catalog::action::trash()),
    "Delete this project?",
    "This removes the project and its local history.",
    "Remove",
    "Keep",
    ButtonRole::Destructive,
    PopoverPlacement::BottomLeading,
    true,
    || delete_project(),
);
```

确认气泡使用带指针的 bubble 表面并约束在窗口内。`ButtonRole::Destructive` 只把触发器图标与文字染成错误色，确认按钮保持独立样式。Cancel、Escape 或点击外部关闭气泡且不触发 `on_confirm`。

Gallery 的 Popconfirm 页包含 placement playground：十二个方位的确认气泡共用一条 Last action 回读；可用宽度不足 340pt 时退化为 Select 选择方位加单个预览气泡。

共享 Rust 实现覆盖所有宿主平台。自动化场景验证与原生视觉、键盘及 IME 验收分别记录在 Gallery 验收表中。