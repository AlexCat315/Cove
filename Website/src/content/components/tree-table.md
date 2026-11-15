# TreeTable

`TreeTable` 用表格展示层级数据。首列为每级保留缩进，并为分支和叶节点保留等宽的展开控件槽。可见行复用 [Table](/docs/table) 的滚动、虚拟化、列宽与选中样式。

```rust
let table = TreeTable::try_new(
    roots,
    |node: &Asset| node.id.clone(),
    |node: &Asset| (!node.children.is_empty()).then(|| node.children.clone()),
    TreeTableColumn::new("name", "Name", |node: &Asset| {
        AnyView::new(Text::new(node.name.clone()).truncating_tail())
    }),
    [TreeTableColumn::new("kind", "Type", |node: &Asset| {
        AnyView::new(Text::new(node.kind.clone()))
    })],
)?
.options(TreeTableOptions {
    expanded: Some(expanded.binding()),
    selection: Some(Selection::single(selected.binding())),
    ..Default::default()
});
```

`expanded` 可绑定到 `BTreeSet<Id>`，`selection` 可由调用方控制。省略绑定时使用组件会话内的展开状态和单选。兄弟节点按声明顺序显示。

- 点击分支箭头可展开或收起。收起分支时，原先落在子节点的游标和选中项移到该分支。
- 首列聚焦时，→ 展开分支或进入第一个子节点，← 收起分支或回到父节点。在其他列中，左右方向键移动单元格游标。
- `indentation` 控制每级缩进，`layout` 继承表格的行高和表头设置。展开控件遵循宿主的最小触摸尺寸。
