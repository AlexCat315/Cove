`Row` 是固定 `flexDirection: .row` 的 `Box`，只保留交叉轴对齐与间距两个参数。

```swift
Row(alignment: .center, spacing: 12) {
    Text("Build")
    Spacer()
    Button("Run") { }
}
```

`alignment` 默认为 `.center`，写入 Yoga 的 `alignItems`；`spacing` 大于零时写入 gap。需要反向排列、换行或主轴对齐时改用 `Box`。
