`ThemeReader` 在构建期读取当前主题，并把 `Theme` 传给内容闭包。它不插入额外的布局容器，只负责把主题值引入视图构建。

```swift
ThemeReader { theme in
    Row(spacing: theme.spacing.sm) {
        Text("Saved").foregroundColor(theme.colors.onSurfaceMuted)
        Spacer()
        Text("⌘S").foregroundColor(theme.colors.accent)
    }
}
```

需要在绘制期而不是构建期读取主题时，改用样式协议（`ButtonStyle`、`PanelStyle` 等）的配置对象。
