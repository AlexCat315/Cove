`Binding` 是指向别处可变值的双向引用：读取调用 `get`，写入调用 `set`。

```swift
let binding = Binding<String>(
    get: { model.query },
    set: { model.query = $0 }
)
```

`@State` 的 `projectedValue` 就是一个 `Binding`，因此可以把可写引用传给子视图，而不必传递整个模型。`constant(_:)` 生成只读绑定，适合预览或默认参数。
