`Theme` 是 Compose 层样式消费的设计 token 集合，按值传递。它包含六组核心尺度——颜色、排版、间距、圆角、阴影与动效——以及输入外观、图标与扩展槽位。

```swift
ThemeReader { theme in
    Column(spacing: theme.spacing.md) {
        Text("Title").foregroundColor(theme.colors.onSurface)
        Text("Body").foregroundColor(theme.colors.onSurfaceMuted)
    }
    .padding(theme.spacing.lg)
    .background(theme.colors.surface)
}
```

因为 `Theme` 与各 token 容器都是值类型，可以从内置预设复制出变体，只改需要的字段：

```swift
var brand = Theme.defaultDark
brand.colors.accent = Color(red: 34, green: 211, blue: 238)
```

整棵子树换肤用 `.theme(_:)` 或 `.appearance(.dark / .light)`；两者都通过 `ThemeEnvironment` 重新提供值。
