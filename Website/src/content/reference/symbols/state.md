`@State` 是视图局部可变状态的属性包装器。写入 `wrappedValue` 会触发 `StateStorage.onChange`，组合运行时把它接到 `Recomposer.invalidate(scopeID:body:)`，只重组拥有该状态的视图作用域。

```swift
struct Counter: View {
    @State private var count = 0

    var body: some View {
        Button("Count: \(count)") { count += 1 }
    }
}
```

`projectedValue` 暴露一个 `Binding<Value>`，用于把可写引用传给子视图。

`@State(expose: true)` 把值以只读形式暴露给 DevTools；可选的 `summary` 闭包用于脱敏或缩写较大的值。`tracking` 与 `signal` 下标提供按字段细粒度的读取跟踪。
