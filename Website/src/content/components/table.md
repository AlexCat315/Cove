# Table

`Table::new` 在普通记录切片上绘制固定表头与虚拟化行的网格。它是轻量的那个：记录以 `Vec<Record>` 传入，排序同步完成，会话由挂载自己持有。

```rust
let table = Table::new(records, |row: &InventoryRecord| row.id, columns)?;
let mut options = TableOptions::default();
options.selection = Some(Selection::single(selected.binding()));
options.sort = Some(sort.binding());
Table::new(records, |row: &InventoryRecord| row.id, columns)?.options(options);
```

列与行的 ID 必须唯一：重复的行 id 会让两行争用同一个选中项，因此构造返回 `Result`。

## 与 DataTable 的分界

判断标准只有一条：**数据的生命周期归谁。**

- 记录就是一份普通切片、跟着父视图重渲染走、以读为主 —— 用 `Table`。
- 数据要跨重渲染存活、上万行、需要后台排序、冻结列、拖拽换列序、多选与单元格编辑 —— 用 `DataTable`，它接收 `DataTableModel`。

不是为了两种外观，而是两种成本。`Table` 的用户代码不需要构造任何模型对象；`DataTable` 的用户必须自己持有并同步模型。

`TreeTable` 不是第三种并列选择：它是把首列换成层级列的 `DataTable`。`ChartDataTable` 也不是表格组件，而是 `Chart` 的无障碍数据镜像。

两者共用同一套类型：`TableColumn`、`TableColumnLayout`、`TableLayout`、`TableSort`、`ColumnVisibility`、`TableColumnPlan` 以及列宽拖拽手柄，所以可见性规划、自适应丢列与列宽约束在两种表里行为一致。

## 表头与排序

带 `compare` 的列是排序入口：`compare` 缺省时表头不可点击。激活顺序是升序、降序、回到源序。

相等比较在两个方向上都保留源序，因此稳定排序不会打乱比较器分不开的行。排序按列 id 在**所有已授权列**中查找，所以隐藏被排序的列不会悄悄丢掉它的排序。

表头是真正的控件：指针点击与 `Return`/`Space`/`KeypadEnter` 都能触发，并忽略重复按键。

## 列宽与可见性

每列宽度受自身 `TableColumnLayout` 的 `min_width`/`max_width` 约束，非有限值回落到最小值。拖拽手柄支持指针拖动、方向键步进与辅助功能的值设置，两侧共用同一实现。

`ColumnVisibility::Optional` 的列在宽度不足时被整列丢弃而不是裁剪，按 `priority` 从低到高让位。任一列是 Optional 时会出现列选择器。

调用方提供 `column_widths` / `column_visibility` 绑定时使用它们；否则宽度与可见性属于本次挂载的会话，重渲染后保留。

## 行、选中与空数据

行体走虚拟化列表，因此行数不影响挂载成本。选中通过 `Selection<Id>` 传入（与 `List` 一致），未提供时用会话内的单选。选中的行整行铺满，列边界才能笔直贯穿每一行；`TableLayout::is_striped` 打开隔行底色。

`on_activate` 在点击或回车激活某行时回调该行的记录。

记录为空时表头仍然保留，表体换成 `TableLayout::empty_title` 的空状态，说明如何恢复。

## 可访问性

容器带 table 角色，每个单元格带 cell 角色并把所属列标题作为标签，因此读屏在读出数值前会先报出它属于哪一列。排序表头是 button 角色，标签为「Sort by 列名」，并报告当前的排序选中状态。

共享 Rust 实现覆盖所有宿主平台。自动化回归与原生视觉、键盘及 IME 验收分别记录在 Gallery 验收表中。
