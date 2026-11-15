# Table

`Table` 是唯一的表格组件：同一套类型覆盖普通记录切片、上万行的持久数据集、冻结列、列序调整、多选与单元格编辑。不存在第二种表格模型。

```rust
let table = Table::new(records, |row: &InventoryRecord| row.id, columns)?
    .options(TableOptions {
        selection: Some(Selection::single(selected.binding())),
        sort: Some(sort.binding()),
        frozen_column_count: 1,
        ..TableOptions::default()
    });
```

列与行的 ID 必须唯一：重复的行 id 会让两行争用同一个选中项，因此构造返回 `Result`。

## 一种模型，两条构造路径

`Table::new(records, row_id, columns)` 是常用入口。视图在首次挂载时把记录灌入自己的 `TableModel`，之后的重渲染、排序和行编辑都作用在这一个模型上，不会复制整张表。

当记录需要在多个视图之间共享，或调用方已经持有模型时，使用：

```rust
let model = TableModel::try_new(records, |row: &InventoryRecord| row.id)?;
let table = Table::try_new(model, columns)?;
```

`Table::model()` 返回底层 `Rc<TableModel<Record, Id>>`；`Table::columns()` 返回声明顺序的列。两者是同一个模型上的读取入口，没有第二套状态。

## 选项

`TableOptions` 合并了所有可调项：

| 字段 | 作用 |
| --- | --- |
| `selection` | 外部 `Selection<Id>`；缺省为挂载会话内的多选 |
| `sort` | 受控排序绑定；模型仍是唯一事实来源，用户点击表头时回写 |
| `cursor` | 单元格游标绑定，用于观察或移动焦点 |
| `column_widths` / `column_visibility` / `column_order` | 列宽、可见性与显示顺序 |
| `layout` | 行高、表头高、隔行底色、分隔线内缩、 `fills_width` 与空状态标题 |
| `frozen_column_count` | 冻结的前导列数量；视口不足时自动解冻 |
| `navigation` | 按行 ID 和可选的列 ID 滚动定位，每个请求 ID 执行一次 |
| `is_enabled` | 禁用后整体降透明度并退出输入发现 |

`layout.fills_width` 默认为 `true`：最后一个弹性列吸收剩余宽度，表格以卡片右边缘结束，而不是留下一条空白。

## 表头与排序

带 `compare` 的表头可排序，激活顺序为升序、降序、回到源序。表头也可以拖动调整列序。

相等比较在两个方向上都保留源序，因此稳定排序不会打乱比较器分不开的行。排序按列 id 在**所有已授权列**中查找，所以隐藏被排序的列不会悄悄丢掉它的排序。

`Table::sort_direction(column_id)` 报告某一列当前的排序方向。受控的 `sort` 绑定在构造选项时与首次挂载时都会同步到模型，模型本身也可以直接调用 `set_sort`。

表头是真正的控件：指针点击与 `Return`/`Space`/`KeypadEnter` 都能触发，并忽略重复按键。

## 列宽、可见性与列序

每列宽度受自身 `TableColumnLayout` 的 `min_width`/`max_width` 约束，非有限值回落到最小值。拖拽手柄支持指针拖动、方向键步进与辅助功能的值设置。

`ColumnVisibility::Optional` 的列在宽度不足时按 `priority` 从低到高隐藏。`ColumnVisibility::Hidden` 默认隐藏，可在 Columns 中显式启用。工具栏左侧显示总行数。

调用方提供 `column_widths` / `column_visibility` / `column_order` 绑定时使用它们；否则宽度、可见性与顺序属于本次挂载的会话，重渲染后保留。表头拖拽与 Alt+Left/Right 写入显示顺序，冻结列与滚动列不会互换。

## 行、选中与空数据

行体走虚拟化窗口，只挂载可见行列及少量 overscan，因此 200,000 行 × 100 列也只创建几十个单元格。固定行高由 `TableLayout::row_height` 给出；`Table::row_height` 可为每行提供独立高度，文本缩放与触摸最小高度仍会参与计算。

选中通过 `Selection<Id>` 传入（与 `List` 一致），未提供时用会话内的选中集合。选中的行整行铺满，前导泳道左缘的 3pt 强调条与选中底色共同标记，列边界笔直贯穿每一行；强调条只画在起始列为 0 的泳道上，冻结边界后的移动泳道不会重复第二条。`TableLayout::is_striped` 打开隔行底色。

单击选中行并设置单元格游标。双击或按 Return 在可编辑单元格打开草稿，在普通单元格调用 `on_activate`。方向键、Page Up/Down、Home/End 移动游标，`Cmd/Ctrl+A` 全选。

记录为空时表头仍然保留，表体换成紧凑的 `TableLayout::empty_title` 空状态，说明如何恢复。

## 行内编辑

列通过 `TableTextEditing` 提供字段的读取、写入和可选验证。F2 打开当前单元格并选中原值。Return 提交，Escape 取消，Tab 和 Shift+Tab 提交后在同一行跳到下一或上一可见的可编辑列。验证失败时保留草稿，并在单元格边框和提示中显示错误。

```rust
let quantity = TableColumn::new("quantity", "Quantity", |row: &InventoryRecord| {
    AnyView::new(Text::new(row.quantity.to_string()))
})
.text_editing(
    TableTextEditing::new(
        |row: &InventoryRecord| row.quantity.to_string(),
        |row: &mut InventoryRecord, value| {
            if let Ok(quantity) = value.parse::<u32>() {
                row.quantity = quantity;
            }
        },
    )
    .validate(|value| value.parse::<u32>().is_err().then(|| "请输入非负整数".into())),
);
```

## 视觉

桌面默认行高 36pt、表头 32pt，单元格和表头水平内边距 12pt。行高与表头高度随文字缩放，并遵循当前平台的最小触摸尺寸。表头和单元格内容垂直居中，排序箭头贴近标题，数字列的标题与内容共用右边缘。草稿使用单元格自身的边框。

## 可访问性

容器带 table 角色，每个单元格带 cell 角色并把所属列标题作为标签，因此读屏在读出数值前会先报出它属于哪一列。排序表头是 button 角色，标签为「Sort by 列名」，并报告当前的排序选中状态。禁用表格会同时移出指针与可访问性发现。

`TreeTable` 在相同的表格几何上增加层级列、展开状态和方向键导航。见 [TreeTable](/docs/tree-table)。

共享 Rust 实现覆盖所有宿主平台。自动化回归与原生视觉、键盘及 IME 验收分别记录在 Gallery 验收表中。
