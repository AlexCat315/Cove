`ScrollView` 提供带裁剪的滚动容器，可选择滚动轴、消费策略与滚动条占位方式。

```swift
ScrollView(.vertical) {
    Column(spacing: 12) {
        Text("Header")
        Text("Body")
    }
    .padding(16)
}
```

`axes` 为 `.vertical`、`.horizontal` 或 `.both`。`scrollOffset` 是双向的 `CGPoint` 绑定；`onViewportChange` 与 `onGeometryChange` 在视口或几何变化时回调；`scrollRequest` 提交一次声明式滚动。

`scrollbarGutter` 为 `.overlay` 时滚动条浮在内容上，为 `.stable` 时在尾侧预留固定通道，避免表单控件被滚动条覆盖。
