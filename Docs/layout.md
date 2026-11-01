# 布局

Cove 使用 Taffy 0.15.1 的 Flexbox、CSS Grid 和 `calc`。布局实现共用于桌面、Android、iOS 和 Wasm，没有平台专用的布局后备算法。

## 尺寸

`cove_core::scene::LayoutStyleValue` 是统一尺寸类型，`FrameDimension` 是它的别名。百分比按 `0..100` 表达，固定长度使用逻辑点。

```rust
use cove_core::scene::{LayoutCalc, LayoutStyleValue as Value};
use cove_ui_compose::modifiers::layout::{FrameValues, LayoutValuesExt};

let content = content.frame_values(FrameValues {
    width: Some(Value::calc(100.0, -24.0)), // 100% - 24pt
    max_width: Some(Value::Calc(LayoutCalc::new(80.0, 0.0).with_maximum(640.0))),
    ..Default::default()
});
```

- `Auto`、`Points`、`Percent`、`Calc` 用于宽高、flex basis、定位、padding、margin、gap 和最小/最大尺寸。
- `Calc` 表达 `percent * containing_size / 100 + points`，可加逻辑点上下限。表达式在布局时求值，窗口尺寸、样式或方向变化会按布局缓存契约重新计算。
- `MinContent`、`MaxContent`、`FitContent`、`FitContentLimit(LayoutCalc)` 用于宽高与 flex basis。文本的最小内容宽度取不可换行片段的最大宽度。
- `padding_value`、`margin_value`、`position_value`、`gap_value` 和 `flex_basis_value` 暴露表达式修饰符；底层节点提供对应的 `set_*_value`。

当前不解析 CSS 字符串，也不提供任意表达式 AST。Taffy 的最小/最大尺寸类型只接受长度、百分比和 auto；向这些约束、边距或 gap 传入内容尺寸关键字会报错。保留 Cove 既有默认值：列方向、`flex-shrink: 0`、零自动最小尺寸、未设置的 margin 为零。

## CSS Grid

`cove_composition::CssGrid`（也由 `cove_ui_compose` 导出）直接使用保留布局树。`GridStyle` 配置显式与隐式轨道、自动流、命名线、子项行列放置以及对齐。

```rust
use cove_composition::CssGrid;
use cove_core::scene::{GridStyle, GridTrack};

let grid = CssGrid::new(GridStyle {
    columns: vec![GridTrack::fr(1.0).into(), GridTrack::fr(2.0).into()],
    ..Default::default()
}, content);
```

`GridTrack::minmax` 配合 `GridBreadth` 表达最小/最大轨道；`GridTemplateTrack::Repeat` 支持固定次数、`AutoFill`、`AutoFit`。轨道长度支持点、百分比和混合表达式。`grid_style` 修饰符设置子项的 `column`、`row`、跨度和自身对齐；行号遵循 CSS 的一基索引与负数索引。

普通 `Grid` 保留 `GridColumn` 和命名列显隐 API，使用 Taffy 求解轨道及跨格放置，通过测量代理连接现有 `LayoutSubview` 契约。固定尺寸子视图保留自身尺寸。自适应列组仍是 Cove 的策略，最终轨道由 Taffy 布局；CSS 的轨道分配语义可能与旧的平均分摊算法不同。

`LazyGrid` 保留按行/列窗口构建、overscan、固定/变动行高和滚动范围计算。原有 `GridTrackResolver` 仍供虚拟化与其他表格策略使用；普通 `Grid` 不再依赖它手算轨道宽度。
