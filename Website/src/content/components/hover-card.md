# HoverCard

`HoverCard::new(label, content)` 在指针停留达到 `open_delay` 后显示内容；离开入口到进入卡片之间由 `close_delay` 保留。

卡片中的控件属于入口的焦点作用域；Escape 关闭卡片并保留原焦点。触摸环境使用同一内容的 sheet。

共享 Rust 实现覆盖所有宿主平台。自动化场景验证与原生视觉、键盘及 IME 验收分别记录在 Gallery 验收表中。
