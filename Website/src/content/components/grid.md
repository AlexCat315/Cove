# Grid / named columns

`Grid::try_new(columns, content)` 按列放置内容，`.spacing(column, row)` 设置列、行间距。`GridColumn::named` 关联稳定 id、标题和可见性策略；重复或无效的列配置在建立布局前返回错误。

必需列保留声明的宽度，超出视口时可以横向滚动。自动列按可用宽度隐藏，调用方可通过 `.column_visibility(...)` 提供覆盖选择；隐藏后仍保留单元格局部状态。
