# Toolbar / AppBar

`ToolbarCommands::try_new` 按分配空间测量命令；横向按宽度，纵向按高度，把低优先级命令移入 More actions。溢出按钮与菜单共享展示状态。

`ToolbarAxis::Vertical` 使用上下方向键。AppBar 的操作结果与标题由调用方状态驱动；Gallery 示例保留相同响应式更新。

共享 Rust 实现覆盖所有宿主平台。自动化场景验证与原生视觉、键盘及 IME 验收分别记录在 Gallery 验收表中。
