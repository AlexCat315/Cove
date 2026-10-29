`Box` 是显式指定 flex 方向的容器，`Row` 与 `Column` 都是它的薄封装。

```swift
Box(direction: .row, alignItems: .center, justifyContent: .spaceBetween, spacing: 8) {
    Text("Title")
    Spacer()
}
```

`direction`、`alignItems`、`justifyContent` 与 `wrap` 直接映射到 Yoga 的 flex 属性。`spacing` 大于零时写入所有方向的 gap。需要按九宫格语义对齐时，改用带 `alignment:` 的初始化器，它会把 `Alignment` 拆成主轴与交叉轴两组值。

容器本身不可命中，也不绘制背景；外观由修饰符负责。
