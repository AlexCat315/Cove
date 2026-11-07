# PropertyGrid

`PropertyGrid::new(sections)` 是两列检查器网格：调用方提供每行的值控件，组件负责标签列、分节卡片与折叠。内容列始终撑满滚动视口宽度（扣除稳定滚动条通道），不会收缩到内容固有宽度。

`PropertyGridRow::new(id, label, value)` 声明一行。`.layout(PropertyGridRowLayout::FullWidth)` 把标签放在值上方；默认 `TwoColumn` 为自适应——仅当行宽容纳 `label_width + 4 + min_value_width` 时标签与值并排，否则叠放。`.sizing(PropertyGridRowSizing::Fixed)` 把值槽固定到行高；默认 `Intrinsic` 以 `row_height` 为下限，披露编辑器可随内容伸缩。

`PropertyGridSection::new(id, title, rows)` 声明一节。`.collapsible(true)` 显示折叠箭头，`.starts_collapsed(true)` 设定初始折叠；`.children(...)` 嵌套子节（子节平铺在父节卡片内），`.badge(...)` / `.row_count(...)` 决定标题次级文字，`.header_leading(...)` / `.header_trailing(...)` / `.footer(...)` 附加自定义视图。折叠节不构建行。

`.controlled_collapsed(ids)` 交由调用方控制折叠，配合 `.on_section_collapse_changed(|id, collapsed| ...)` 回写；未提供时组件保留局部状态。优先级：受控集合 > 局部状态 > `starts_collapsed`。

尺寸默认值：`label_width` 96、`min_value_width` 220、`row_height` 24、`row_spacing` 1、`section_spacing` 10、`content_padding` 8。`.scroll_axes(...)` 选择滚动轴（默认 `Both`）；纵向与双向滚动预留稳定滚动条通道，横向滚动不预留。空网格显示 `.empty_text(...)`。

共享 Rust 实现覆盖所有宿主平台。自动化场景验证与原生视觉、键盘及 IME 验收分别记录在 Gallery 验收表中。