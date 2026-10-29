`List` 是纵向虚拟化列表，支持单选与多选。只有可见行会被实例化。

```swift
@State private var selected: String?

List(projects, id: \.id, selection: $selected, rowHeight: 44) { project, isActive in
    Text(project.name)
}
```

`rowContent` 收到元素与“是否为当前行”的布尔值。传入 `Binding<ID?>` 为单选，传入 `Binding<Set<ID>>` 为多选；`onActivate` 在行被激活（双击或回车）时调用，`refresh` 接入下拉刷新。

`rowHeight` 是缩放前的逻辑高度，实际高度会叠加文本缩放，并在触摸输入下遵守宿主的最小控件尺寸。
