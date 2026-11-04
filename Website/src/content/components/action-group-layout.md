# ActionGroupLayout

`ActionGroupLayout::new(stacks)` 在按钮自然宽度无法同时容纳时堆叠操作；`stacks = true` 强制堆叠。`alignment` 和 `spacing` 控制对齐与间隔，leading/trailing 随布局方向解析。

堆叠后的测量使用实际分配宽度，因此长标题换行后按钮不会重叠。自定义布局的普通 `LayoutSubview::size_that_fits` 保留软尺寸提议，允许固定宽度、最小宽度和滚动内容超出提议；需要获取收窄后的换行高度时使用 `size_that_fits_constrained`。
