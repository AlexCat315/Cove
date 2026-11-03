# RangeSlider

`RangeSlider` 编辑一对有序的上下界，绑定类型为 `RangeValue`。单值设置使用 [Slider](/docs/slider)。独立的数据类型使单值与范围的读写不需要运行时模式判断。

```rust
use cove_ui::prelude::*;

fn visible_range(cx: &Scope) -> impl View {
    let values = cx.state(|| RangeValue::new(20.0, 65.0));
    RangeSlider::new(values.binding())
        .range((0.0, 100.0))
        .minimum_span(20.0)
        .frame_width(240.0)
}
```

按下轨道时选择最近的滑块；拖动会持续更新对应端点。`minimum_span` 限制最小间距，端点不会交叉或越界。`step` 同时约束拖动、键盘与无障碍增减操作。

两个端点各有一个 Tab 停靠点，并分别提供“Range start”和“Range end”的语义值。方向键调整当前端点，Home / End 到达允许的边界，仍然遵守最小间距。键盘焦点环只标出当前端点；指针点击不显示键盘焦点环。

水平布局遵循从左到右或从右到左的方向，绘制、指针映射与左右方向键保持一致。`enabled(false)` 禁止交互并移除 Tab 停靠点。默认白色滑块使用轮廓和局部阴影，能在浅色背景上辨认。

Gallery 分别展示基本范围、十单位步长、二十单位最小间距和禁用状态。顶部 Size、Scale、Disabled 与 Reset example 用于检查密度、文字缩放和重新挂载。
