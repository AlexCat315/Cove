`LayerRoot` 建立分层根：`content` 是常规内容层，`portals` 承载 sheet、对话框与菜单等浮层，使它们不受内容层裁剪与堆叠顺序影响。

```swift
LayerRoot {
    RootView()
}
```

只传 `content` 的初始化器在 `Portals == PortalHost` 时可用，会自动挂载 `PortalHost()`。需要自定义浮层宿主时使用双闭包初始化器。
