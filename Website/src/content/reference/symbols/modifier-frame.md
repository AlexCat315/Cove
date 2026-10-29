设置接收视图的尺寸约束。省略的维度保持内容自身的尺寸；显式使用 `.auto` 可以把某个轴交还给内容测量。

```swift
Row(spacing: 8) { /* … */ }
    .frame(minHeight: 44, maxHeight: 44)
```

`Float` 重载按逻辑点设置；`FrameDimension` 重载接受 `.points(_:)`、`.percent(_:)` 与 `.auto`；`framePercent(...)` 是百分比写法的便捷入口：

```swift
Column { /* … */ }.framePercent(width: 100)
```

尺寸写入会参与动画系统，因此可以配合 `withAnimation` 与 `.animation(_:value:)` 平滑过渡。
