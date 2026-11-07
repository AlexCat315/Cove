# List / ListRow

`List`、`SectionedList` 与 `SwipeList` 是同一族虚拟化列表：同一个 macOS 内嵌列表盒（`surface` 圆角盒，内边距取自选中外观的 `horizontal_inset` / `vertical_inset`），同一条选中带，同一套行内容 API。

```rust
use cove_ui::prelude::*;

fn mailboxes(cx: &Scope) -> impl View {
    let selection = cx.state(|| Some(0usize));
    List::new((0..3).collect(), |id: &usize, _| {
        let row = match id {
            0 => ListRow::new("Inbox").icon(ui_icon_catalog::file::inbox()).badge("24"),
            1 => ListRow::new("Sent").icon(ui_icon_catalog::shape::send()).value("Yesterday"),
            _ => ListRow::new("Trash").icon(ui_icon_catalog::action::trash()).destructive(),
        };
        AnyView::new(row.accessory(ListRowAccessory::Disclosure))
    })
    .selection(Selection::single(selection.binding()))
    .row_height(36.0)
}
```

## 选中、悬停与焦点

- 选中：列表持有键盘焦点时为强调色圆角带、浅色墨水；焦点在别处时为中性灰带（`surface_pressed`），文字保持原墨水——与 macOS 列表一致。
- 悬停与按压：由行宿主实时绘制，几何与选中带相同（上下内缩 `spacing.xs`、圆角 `radius.md`），不会触及相邻行；已选中的行不再叠加悬停。
- 分隔线：最后一行与选中带上方不画。
- 滚动条为覆盖式，不占行宽，选中带在盒内左右对称。

## ListRow

| 方法 | 作用 |
| --- | --- |
| `icon(resource)` / `icon_tint(color)` | 前导图标，默认强调色，强调带上变为浅色 |
| `subtitle(text)` | 标题下的次要行 |
| `value(text)` | 尾部次要文字（计数、日期、设置值） |
| `badge(text)` | 尾部计数胶囊 |
| `accessory(ListRowAccessory)` | `Disclosure`（灰色箭头）、`Checkmark`（强调色勾）或 `Custom(view)` |
| `destructive()` | 标题与图标使用错误色 |
| `disabled(bool)` | 内容降为 40% 不透明度 |
| `leading(view)` / `trailing(view)` | 任意前导/尾部视图 |

所有部件根据所在的带自动取色，调用方不需要按 `RowState::is_selected` 自己选颜色。

## 自定义行内容

自定义内容读取 `ListRowContext`（`is_selected`、`is_emphasized`、`is_enabled`、`is_on_accent()`）决定墨水：

```rust
ListRowContextReader::new(|context: ListRowContext| {
    let tint = if context.is_on_accent() {
        SemanticColorRef::on_accent()
    } else {
        SemanticColorRef::accent()
    };
    Icon::semantic_with_ref(ui_icon_catalog::shape::circle(), SemanticIconSize::small(), tint)
})
```

`ListRowStyle` 的配置新增 `is_emphasized`，自定义行样式据此区分强调与中性选中。

共享 Rust 实现覆盖所有宿主平台。自动化场景验证与原生视觉、键盘及 IME 验收分别记录在 Gallery 验收表中。
