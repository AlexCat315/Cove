# Rating

`Rating` 显示和编辑星级，支持分数精度、自定义图形、只读和禁用状态。它使用受控数值与变化回调。

```rust
use cove_ui::prelude::*;

fn score(cx: &Scope) -> impl View {
    let value = cx.state(|| 2.5);
    Rating::new("Overall rating", value.get(), move |next| value.set(next))
        .selection(RatingSelection {
            precision: RatingPrecision::HALF,
            ..Default::default()
        })
}
```

`RatingSelection.maximum` 指定图形个数，范围为 1–20。`precision` 支持 WHOLE、HALF、QUARTER、TENTH，或 `RatingPrecision::divisions(n)`。`allows_clear` 为真时，再次点击已选分数可以清空。

悬停只预览，不写入数值；在控件内部松开指针才提交，移出后松开则取消。按 Escape 取消当前按压。一个评分控件占一个 Tab 停靠点：方向键按精度调整，Shift 加方向键按整颗调整，Home / End 到达两端。

`RatingAppearance.symbol` 可设置 `RatingSymbol::star()` 或 `RatingSymbol::heart()`。填充图形与轮廓图形共享位置，部分填充按分数裁切；从右到左布局同时镜像图形顺序、裁切方向和输入映射。`control_size`、`star_size` 和 `spacing` 控制密度。

只读状态通过 `RatingSelection { is_read_only: true, ..Default::default() }` 设置，保留精确显示值；禁用状态使用 `is_enabled: false`。两者不接受评分操作，且不保留过期的输入处理器。父视图更新不会丢失正在使用的控件会话。

Gallery 展示整星、半星、十分之一心形、只读、禁用、七颗和十颗星、清空、密度和从右到左布局。

宽评分在卡片内水平滚动，分数与清空操作放在图形下方，窄窗口不会挤出卡片。
