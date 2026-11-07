# 动效

默认动效以弹簧为主：可被打断、保留速度，落位时带轻微回弹。所有动效在系统开启「减弱动态效果」时直接落到终态。

## 滑动选中

`SlidingSelection` 在一组行后面只画一个选中底，选中项改变时底块以弹簧滑到新位置。位置相对宿主测量，滚动祖先不会让底块追赶内容。行用 `SlidingSelectionMark(true)` 标记自己；`is_in_sliding_selection(node)` 为真时，行只画悬停与按压，静止的选中底交给宿主。

```rust
use cove_ui::prelude::*;
use cove_ui::compose::animation::geometry::{SlidingSelection, SlidingSelectionStyle};

fn group(rows: AnyView) -> impl View {
    SlidingSelection::new(
        |theme| SlidingSelectionStyle {
            fill: theme.colors.surface_raised,
            corner_radius: theme.radius.pill,
            shadow: theme.elevation.low,
        },
        rows,
    )
}
```

Sidebar 的选中胶囊与 ButtonGroup（分段控件）的拇指都基于它。

## 悬停浮起

`hover_lift()` 让表面在指针下以弹簧放大到 1.015 并加深阴影到 `elevation.high`，离开后回落。宽表面用 `hover_lift_scaled(1.004)` 之类更小的比例。

## 级联进入

`cascade_in()` 让同一批挂载的表面依次进入：从底边放大、淡入，间隔 45ms，最多延迟 0.36s。挂载顺序即树顺序，因此页面自上而下、同一行自左向右。

```rust
use cove_ui::prelude::*;
use cove_ui::compose::animation::CascadeInExt;
use cove_ui::compose::animation::interaction::HoverLiftExt;

fn card(content: impl View + 'static) -> impl View {
    content.hover_lift().cascade_in()
}
```

## 控件

| 控件 | 动效 |
| --- | --- |
| 按钮 | 按下即时压缩到 `press_scale`；松手按欠阻尼弹簧回弹，底色同步缓出 |
| 开关 | 拇指欠阻尼弹簧行程；按住时拇指向行程方向拉伸 5pt |
| 复选框 | 勾按路径逐段画出，取消时反向收回 |
| 菜单 / 弹出层 | 从触发角弹簧放大进入，0.14s 退出 |
| 数字 | `NumericText` 按弹簧滚动到新值 |
