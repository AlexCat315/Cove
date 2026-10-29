`Column` 是固定 `flexDirection: .column` 的 `Box`，只保留交叉轴对齐与间距两个参数。

```swift
Column(alignment: .leading, spacing: 8) {
    Text("Settings")
    Text("Changes apply immediately")
}
```

`alignment` 默认为 `.leading`，写入 Yoga 的 `alignItems`。需要主轴分布、换行或反向排列时改用 `Box`。
