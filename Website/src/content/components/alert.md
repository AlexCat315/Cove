# Alert

`Alert::new(title)` 是页面内的反馈块：一个状态图标、标题、可选说明、可选操作与可选关闭。`message(...)` 补充说明，`tone(StatusTone)` 决定语义色，未设置时为 `Info`。

```rust
Alert::new("Reconnect to synchronize your workspace")
    .message("Your local changes are safe. Reconnect when the connection returns, or continue working offline.")
    .tone(StatusTone::Warning)
    .on_dismiss(move || dismissed.set(true))
    .dismiss_label("Dismiss interruption")
    .actions(BoxView::new(/* Reconnect、Work offline */))
```

## 外观

`AlertVariant` 决定块与背后表面的关系。默认 `Soft`：主题色以约 10% 混入 `surface`，再压一条 18% 的同色发丝边，块在任何背景上都有清晰轮廓，同时颜色只作为语义提示。`Outline` 不填充、只保留 35% 的同色描边，适合嵌在已有边框中。`Filled` 用整个 tone 色铺底，文字取该色上的可读墨色，只用于一条阻断性信息。

状态图标使用实心符号：勾、叉、感叹号与信息标记从实心圆形/三角形中挖空，因此在 16pt 下仍是一块清晰的记号，而不会读成一组细线。图标与标题首行垂直居中，说明与操作缩进到标题起点下方，形成“标记 + 一段文字”的结构。图标可用 `.icon(...)` 换成任意 `BundleImageResource`，或用 `.shows_icon(false)` 移除。

`AlertVariant::Soft` 的标题用 `on_surface`，说明用 `on_surface_variant`，关闭按钮用中性灰而不是 tone 色，让色相只集中在图标上。

## 密度与禁用

密度跟随继承的 `ControlSizeEnvironment`：Mini/Small 用 12pt 图标、8pt 内边距；Regular 用 16pt 图标、12pt 内边距；Large 用 24pt 图标、16pt 内边距。图标尺寸按 `IconMetrics` 与文字缩放同步。

`.enabled(false)` 把整块降到 60% 不透明度、语义状态置为禁用，并让关闭按钮不再响应。它不会清空标题或说明，便于保留上下文。

## 关闭与操作

`.on_dismiss(handler)` 在右上角显示一个 `x` 幽灵按钮；`.dismiss_label(...)` 同时作为悬停提示与无障碍名称，默认 `Dismiss`。按钮始终停在顶部，长标题与多行说明不会把它推走。

`.actions(view)` 接收一个已组合好的视图，放在说明下方；`ActionGroupLayout` 在窄容器里把操作堆叠换行，间距随密度变化。操作缩进与说明一致。

## 无障碍

整块语义为 `Status`，标题是 label，说明是 help，禁用状态通过 `is_enabled` 传递。装饰性的状态图标对辅助技术隐藏，语义由文字承担。

共享 Rust 实现覆盖所有宿主平台。自动化场景验证与原生视觉、交互验收分别记录在 Gallery 验收表中。
