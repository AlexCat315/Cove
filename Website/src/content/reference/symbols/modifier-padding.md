在视图四周增加内边距。

```swift
Column(spacing: 8) { /* … */ }
    .padding(16)
```

三个重载分别接受统一值、`EdgeInsets` 与按轴拆分的水平/垂直值：

```swift
Text("Card").padding(horizontal: 12, vertical: 8)
```

内边距写入 Yoga 布局节点，参与布局测量，不影响视图身份。
