# ReorderableList

`ReorderableList` 是按拖拽手柄排序的虚拟化纵向列表。顺序由调用方持有：拖动结束时 `on_move(from, to)` 收到源索引和移除该行后的目标索引，列表本身只负责交互与投影。

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

按下手柄后，被拖动的行变为半透明占位，浮起的副本跟随指针，并在视口内上下夹紧。拖动越过一行时出现 2pt 强调色插入线；落点在原位置时不显示插入线，也不回调。

指针停在视口上下边缘时会自动滚动，离边缘越近速度越快；偏移在内容末尾处停止，不会滚过最后一行。拖动距离按内容坐标计算，因此自动滚动后落点仍与指针一致。

行高与行距由 `row_height` / `row_spacing` 决定；自动滚动与落点计算使用同一组值。`row_height` 在触摸输入下会抬升到最小控件尺寸。

## 键盘与辅助功能

手柄可聚焦：方向键上/下移动一行，Escape 取消拖拽并恢复原顺序。辅助技术通过 increment/decrement 执行同样的移动。

列表只挂载可见窗口和越界外扩的行；被拖动的行与最近一次键盘移动的行始终保留，指针捕获和焦点因此可以跨越自动滚动。Gallery 的 Reordering 页展示拖拽、插入线与顺序读数。
