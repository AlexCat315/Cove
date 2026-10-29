`View` 是所有 UI 元素的根协议。它只要求一个 `body`，`body` 由 `ViewGraph` 读取并物化为节点，应用代码不应直接调用。

```swift
struct Greeting: View {
    let name: String

    var body: some View {
        Text("Hello, \(name)")
    }
}
```

`Never` 也遵循 `View`，因此 `Text`、`Image` 这类原始视图可以用 `Never` 作为 `Body`：它们不组合子视图，而是直接产出节点。
