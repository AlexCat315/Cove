`Text` 是静态文本原始视图。它通过 Yoga 的 measure 回调参与 flexbox 布局：父容器给出的宽度会触发换行，未约束时报告单行自然宽度。

```swift
Text("Build finished", alignment: .leading, lineLimit: 2)
```

初始化器接受 `String` 或 `LocalizedStringKey`，并可指定对齐、颜色与行数上限。`lineLimit(_:)` 返回新的 `Text` 值，而不是修改原值。

`truncated()` 把标签限制为单行并追加省略号，同时提供完整文本提示与原始无障碍名称。截断不会拆开字形簇；提示只在文本确实被裁切时出现。
