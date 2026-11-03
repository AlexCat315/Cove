# Menu / MenuItemContent

`MenuItemContent::new` 的样式标题保留字体与颜色，但移除片段中的链接，整行是唯一激活目标。`MenuItem::title` 继续用于辅助功能名称和键入搜索。

行级 `link` 可标识目的地址，激活仍执行该项 action。

共享 Rust 实现覆盖所有宿主平台。自动化场景验证与原生视觉、键盘及 IME 验收分别记录在 Gallery 验收表中。
