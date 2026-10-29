`SafeArea` 消费宿主提供的安全区内边距：只在选中的逻辑边上增加 padding，嵌套的 `SafeArea` 只会收到尚未消费的边。

```swift
SafeArea([.top, .horizontal]) {
    Column {
        Text("Content")
    }
}
```

`SafeAreaEdges` 是 `OptionSet`，提供 `.top`、`.leading`、`.bottom`、`.trailing` 以及 `.horizontal`、`.vertical`、`.all` 组合。桌面宿主默认内边距为零，平台边框与键盘几何由所属宿主提供。
