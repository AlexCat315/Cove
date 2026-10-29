`Application` 是高层应用宿主，隐藏窗口、图形设备、DrawList 绘制器、`TextEnvironment` 与 `ViewGraph` 的装配细节。

```swift
try Application.run(config: ApplicationConfiguration(title: "My App")) {
    RootView()
}
```

`run` 会启动主窗口并阻塞到窗口关闭。`onTick` 在每帧布局之前调用，`deltaTime` 是上一帧到当前帧的秒数；`onDisplayReady` 拿到 `AppDisplayHandle`，`onDiagnosticsReady` 拿到 `UIDiagnostics`。

`backend:` 可传入已有的 `Device`，与生产渲染器共享设备与 viewport。主窗口与辅助窗口共享 UI 资源，各自拥有 presentation target。
