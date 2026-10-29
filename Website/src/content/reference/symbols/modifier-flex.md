设置子视图在父容器中的伸缩比例，对应 flex-grow、flex-shrink 与 flex-basis。

```swift
Row(spacing: 12) {
    Text("Title").flex(1)
    Text("Detail").flex(2, basis: 0)
}
```

默认 `grow` 与 `shrink` 都是 1。传入 `basis` 时同时设置 flex-basis；需要“从零开始分配”的等分布局通常写成 `.flex(1, shrink: 1, basis: 0)`。
