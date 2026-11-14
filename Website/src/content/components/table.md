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
| `navigation` | 键盘导航行为 |
| `is_enabled` | 禁用后整体降透明度并退出输入发现 |

`layout.fills_width` 默认为 `true`：最后一个弹性列吸收剩余宽度，表格以卡片右边缘结束，而不是留下一条空白。

## 表头与排序

带 `compare` 的列是排序入口：`compare` 缺省时表头不可点击。激活顺序是升序、降序、回到源序。

相等比较在两个方向上都保留源序，因此稳定排序不会打乱比较器分不开的行。排序按列 id 在**所有已授权列**中查找，所以隐藏被排序的列不会悄悄丢掉它的排序。

`Table::sort_direction(column_id)` 报告某一列当前的排序方向。受控的 `sort` 绑定在构造选项时与首次挂载时都会同步到模型，模型本身也可以直接调用 `set_sort`。

表头是真正的控件：指针点击与 `Return`/`Space`/`KeypadEnter` 都能触发，并忽略重复按键。

## 列宽、可见性与列序

每列宽度受自身 `TableColumnLayout` 的 `min_width`/`max_width` 约束，非有限值回落到最小值。拖拽手柄支持指针拖动、方向键步进与辅助功能的值设置。

`ColumnVisibility::Optional` 的列在宽度不足时被整列丢弃而不是裁剪，按 `priority` 从低到高让位。任一列是 Optional 时会出现列选择器；表格工具栏隐藏了选择器旁边的列数摘要，因为工具栏左侧已经显示行数。

调用方提供 `column_widths` / `column_visibility` / `column_order` 绑定时使用它们；否则宽度、可见性与顺序属于本次挂载的会话，重渲染后保留。表头拖拽与 Alt+Left/Right 写入显示顺序，冻结列与滚动列不会互换。

## 行、选中与空数据

行体走虚拟化窗口，只挂载可见行列及少量 overscan，因此 200,000 行 × 100 列也只创建几十个单元格。固定行高由 `TableLayout::row_height` 给出；`Table::row_height` 可为每行提供独立高度，文本缩放与触摸最小高度仍会参与计算。

选中通过 `Selection<Id>` 传入（与 `List` 一致），未提供时用会话内的选中集合。选中的行整行铺满，左侧 3pt 强调条与选中底色共同标记，列边界笔直贯穿每一行；`TableLayout::is_striped` 打开隔行底色。

`on_activate` 在点击或回车激活某行时回调该行的记录。键盘导航覆盖方向键、Page Up/Down、Home/End、`Cmd/Ctrl+A` 全选，以及 F2 / Return 进入编辑。

记录为空时表头仍然保留，表体换成紧凑的 `TableLayout::empty_title` 空状态，说明如何恢复。

## 视觉

表格是一张卡片：16pt 单元格与表头内边距、44pt 行高、40pt 表头、大圆角与 1pt 边框。表头与工具栏共用 `surface_variant` 色带，表头下方以发丝线分隔；行分隔线通栏铺满，选择态使用强调条加浅色底，而不是整行反白。

## 可访问性

容器带 table 角色，每个单元格带 cell 角色并把所属列标题作为标签，因此读屏在读出数值前会先报出它属于哪一列。排序表头是 button 角色，标签为「Sort by 列名」，并报告当前的排序选中状态。禁用表格会同时移出指针与可访问性发现。

`TreeTable` 复用 `Table` 的全部几何与交互，只把首列换成层级列；`ChartDataTable` 不是表格组件，而是 `Chart` 的无障碍数据镜像。

共享 Rust 实现覆盖所有宿主平台。自动化回归与原生视觉、键盘及 IME 验收分别记录在 Gallery 验收表中。
