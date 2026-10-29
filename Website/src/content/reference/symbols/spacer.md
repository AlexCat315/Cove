`Spacer` 是吸收空闲空间的弹性占位。它把 `flexGrow` 设为 1，因此会沿父容器主轴吞掉剩余空间。

```swift
Row {
    Text("Left")
    Spacer()
    Text("Right")
}
```

`minLength` 大于零时作为 flex-basis 写入，保证占位在空间不足时仍保留一段最小长度。
