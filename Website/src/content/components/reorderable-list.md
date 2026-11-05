# ReorderableList

`ReorderableList` 是按拖拽手柄排序的虚拟化列表。顺序由调用方持有：拖动结束时 `on_move(from, to)` 收到源索引和移除该行后的目标索引，列表本身只负责交互与投影。

```rust
use cove_ui::prelude::*;

fn frames(cx: &Scope) -> impl View {
    let items = cx.state(Vec::<String>::new);
    ReorderableList::new(
        items.get(),
        |item: &String| item.clone(),
        |item: &String, state: RowState| AnyView::new(Text::new(item.clone())),
    )
    .on_move(move |from, to| {
        let mut next = items.get();
        let moved = next.remove(from);
        next.insert(to.min(next.len()), moved);
        items.set(next);
    })
    .row_height(36.0)
    .frame_size(320.0, 220.0)
}
```

只有行尾 44pt 的手柄参与拖拽，行内容自己的按钮和点击不受影响。`row_label` 为辅助技术提供行标题；手柄按 Slider 角色播报当前位次。`is_enabled(false)` 保留列表外观并移除拖拽与键盘操作。

## 拖拽

按下手柄后，被拖动的行变为半透明占位，浮起的副本跟随指针，并在视口内沿主轴夹紧。列表不再有插入线：拖动过程中直接重排——被拖动行的槽位跟着落点走，它越过的每一行反向让出一个步长，画面上看到的就是松手之后会出现的顺序。落点回到原位置时所有行回到各自的槽位，也不回调。

指针停在视口主轴两端时会自动滚动，离边缘越近速度越快；偏移在内容末尾处停止，不会滚过最后一行。拖动距离按内容坐标计算，因此自动滚动后落点仍与指针一致。滚动新进入视口的行按当前投影就位，不会把已经让开的行挤回去。

松手不改动画面：投影已经停在落点，所以释放只是把帧权交还给布局并写入新顺序，任何行都不会再滑第二次；浮起副本用弹簧落到目标槽位，占位行的透明度在落位过程中回到满值，减少“副本消失、行才出现”的跳变。Escape 取消时投影弹回、副本飞回原槽位。开启减弱动效时直接落位。父级在落位过程中重建列表不会打断动画，弹簧会从上一次的位置继续。

`row_height` / `row_spacing` 是主轴上的行尺寸，自动滚动、投影位移与落点计算使用同一组值；`row_height` 在触摸输入下会抬升到最小控件尺寸。

## 轴向

`.axis(ScrollAxis::Horizontal)` 让滚动、拖拽、实时重排和落位动画整体转到横轴：卡片左右拖动，被越过的卡片左右让位，手柄与键盘行为不变。默认 `Vertical`，`Both` 按纵向处理。

```rust
ReorderableList::new(cards, |card: &String| card.clone(), row_content)
    .on_move(move |from, to| cards.set(moved(cards.get(), from, to)))
    .row_height(112.0)
    .row_spacing(8.0)
    .axis(ScrollAxis::Horizontal)
```

## 键盘与辅助功能

手柄可聚焦：方向键沿主轴把行前移或后移（纵向列表用上/下，横向列表用左/右，两组都能用），Escape 取消拖拽并恢复原顺序。辅助技术通过 increment/decrement 执行同样的移动。

列表只挂载可见窗口和越界外扩的行；被拖动的行与最近一次键盘移动的行始终保留，指针捕获和焦点因此可以跨越自动滚动。Gallery 的 Reordering 页展示纵向与横向两组实时重排与顺序读数。
