# Slider

`Slider` 编辑一个数值。需要编辑上下界时，使用 [RangeSlider](/docs/range-slider)。两者共享轨道、滑块、密度和键盘焦点外观，但绑定的数据类型不同。

```rust
use cove_ui::prelude::*;

fn volume(cx: &Scope) -> impl View {
    let value = cx.state(|| 0.35);
    Slider::new(value.binding())
        .range((0.0, 1.0))
        .frame_width(240.0)
}
```

`range` 设置上下界，默认是 0 到 1。不设置 `step` 时，拖动连续更新；设置 `step(10.0)` 后，指针与键盘都写入同一组离散值。`shows_ticks(true)` 显示步进刻度；刻度过密或被滑块覆盖时自动省略。

```rust
use cove_ui::prelude::*;

fn level(cx: &Scope) -> impl View {
    let value = cx.state(|| 40.0);
    Slider::new(value.binding())
        .range((0.0, 100.0))
        .step(10.0)
        .shows_ticks(true)
        .frame_width(240.0)
}
```

`axis(SliderAxis::Vertical)` 使用垂直轨道，高值在上方；请为它提供高度，例如 `frame_size(32.0, 160.0)`。水平轨道遵循布局方向。`enabled(false)` 保留数值和外观，移除指针与键盘操作。

方向键调整一个步长，未配置步长时为范围的 1%；Page Up / Page Down 调整十个步长，Home / End 到达端点。键盘聚焦时显示焦点环。默认外观为 4pt 轨道与 26×18pt 白色胶囊拇指，拇指以柔和阴影和 0.5pt 细边从卡片上浮起；阴影只覆盖滑块，不给整条轨道添加背景。`control_size` 改变轨道和滑块的视觉尺寸。

`on_editing_changed` 在开始和结束指针编辑时回调，可用于预览或提交；数值本身始终通过绑定写回。Gallery 展示连续值、编辑状态、半步长、刻度、垂直和禁用示例。
