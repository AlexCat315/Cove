# Chart

`Chart::new` 绘制折线、面积、柱状、散点和饼/环图，带刻度轴、可选的图例与点选。

```rust
let series = vec![
    ChartSeries::values(revenue, SemanticColorRef::accent(), ChartMark::Area,
                        "Revenue", ChartValueAxis::Left, ChartSeriesStyle::default()),
    ChartSeries::values(signups, SemanticColorRef::success(), ChartMark::Line,
                        "Signups", ChartValueAxis::Left,
                        ChartSeriesStyle::new(Some(2.0), Vec::new(), false, 3.5, 0.2, true)),
];
let mut configuration = ChartConfiguration::default();
configuration.legend.placement = ChartLegendPlacementState::bottom();
configuration.selection.point = Some(selected.binding());
Chart::new(series, ChartXDomain::Categorical(months),
           ChartGuides::default(), ChartStyle::default(), configuration);
```

## 坐标轴

x 域属于整张图表，因此柱状与散点系列共用同一根轴。两侧值轴各自拟合自己的序列：`ChartSeries.axis = ChartValueAxis::Right` 把序列移到右轴，右轴只在有序列读取时才存在并占用留白。刻度标签与轴标题用自己的 gutter，绘图区缩小到标签留下的空间。

`axis.y.kind`（以及 `axis.x.kind`）选择线性或对数间距。对数轴按十进位吸附，刻度落在 1、2、5 × 10ⁿ；非正值没有位置，贴到底边。

`ChartText` 使用当前 `TextEnvironment` 的真实字形尺寸测量标签，只有缺少文本环境（无头测试）时才退回字素近似。因此轴宽、绘图区与标签抽稀与实际渲染一致。

## 堆叠与方向

`ChartStyle::stacking` 让序列相互累积，正值从零向上、负值向下；`Percent` 把每个索引缩放到 100，轴读作占比。`ChartStyle::orientation = Horizontal` 把图表转向：分类沿左侧、数值沿底部，`axis.y` 仍然配置值轴。

## 交互

绑定 `configuration.viewport.range` 启用滚轮缩放、横向平移、`+`/`-` 与 `0`/双击复位；Alt-drag 显式平移。绑定 `configuration.selection.range` 让主拖动变为区间刷选，数值图返回 x 值、分类图返回零基索引。

绑定 `configuration.selection.point` 后即可悬停预览、点击固定、拖动刷值、方向键步进、Escape 清除。callout 在该 x 处列出每条可见序列。`selection.value_text` 与 `configuration.data_labels.format` 共同决定数值文本，前者是回退格式。

仅绑定 viewport 或刷选的图表同样可命中、聚焦与缩放，不需要点选绑定。

## 覆盖层

参考几何在不替换数据的前提上做注记：阈值是一条带标签的规则线，带状区是一段着色走廊，标注是钉在某个值上的说明，`ChartSeriesStyle::shows_values` 在标记旁写出数值。callout 是浮动卡片，标注是带描边的药丸，donut 中心是标题文字，其余为 caption。覆盖层整体对辅助技术隐藏：图表自身的读数已经逐个播报数值，重复播报会把每个值读两遍。

饼/环图的切片标签由引线连到环外，中心内容在 `inner_radius_ratio > 0` 时绘制。

## 图例

`ChartLegendPlacementState::bottom()` 显示换行图例：有空间时同行排布、整体换行；隐藏项保留位置、变暗并显示空心色块。图例项是真正的控件：指针点击、`Return`/`Space`/`KeypadEnter` 激活（忽略重复按键），并绘制焦点环。不提供 `legend.hidden_series` 时图表自己保存隐藏集合。

## 数据动画与缓存

`configuration.animation` 让数据变化插值过渡，包括被中断的情况；外层 `withAnimation` 事务作为回退。减少动态效果时直接到达目标值而不冻结数据。

已解析的几何按尺寸与数据签名缓存，因此指针移动只重绘不重建。签名覆盖数据、可见性、轴配置、样式、guides（阈值/带状区/标记）、每序列绘制样式、每序列值轴侧、分类标签与 caption 字号：编辑阈值、把序列切到右轴或改变曲线样式都会重建几何而不是沿用旧位置。

## 大规模数据与直方图

采样数超过绘图区像素宽度的序列按像素列抽取：每列保留最高与最低样本，包络得以保留而笔触成本受限。

`ChartHistogram::new(bins, Some(range)).build(samples, color, title)` 把样本计数进等宽区间并返回柱序列与边界标签，分布图无需手写计数。分箱是数据步骤，图表只负责画柱：配合 `shows_grid = false` 与 `bar_spacing = 0.0` 即可让柱体相邻。

## 可访问性

屏幕阅读器逐个 x 位置步进：每个位置是一个停靠点，播报其分类、该处所有可见序列的值以及序列中的位置。`accessibility.exposes_values` 关闭后只留一句话描述范围。`ChartDataTable` 提供同一份数据的表格形式，带 table/row/cell 角色，可按行列移动并听到数值所属的序列。

共享 Rust 实现覆盖所有宿主平台。自动化回归与原生视觉、键盘及 IME 验收分别记录在 Gallery 验收表中。
