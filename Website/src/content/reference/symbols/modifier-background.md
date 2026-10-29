设置接收视图的背景填充。

```swift
Column(spacing: 8) { /* … */ }
    .padding(16)
    .background(Color(red: 39, green: 39, blue: 42))
```

三个重载分别接受具体 `Color`、`UILinearGradient` 与语义色引用 `SemanticColorRef`。语义色引用从当前 `Theme` 解析，换肤时自动跟随；`Color` 重载则按原样绘制。

背景绘制在子视图之下，不改变布局尺寸。
