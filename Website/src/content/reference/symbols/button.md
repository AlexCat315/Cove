`Button` 是可点击控件。`label` 决定视觉内容，当前 `ButtonStyle` 决定它如何绘制、留白与呈现各交互状态，默认样式为 `PrimaryButtonStyle`。

```swift
Column(spacing: 8) {
    Button("Save") { save() }
    Button("Discard", role: .destructive) { discard() }
}
.buttonStyle(SecondaryButtonStyle())
```

标题便利初始化器把标题包成 `Text` 并启用尾部截断；图标便利初始化器接受 `ButtonIcon.Source`（纹理、文件或打包资源）。

`isLoading` 为真时按钮不可点击，标签被替换为 `Spinner`（图标按钮保留图标尺寸）。`tooltip` 交给当前样式呈现。
