`withAnimation` 在 `body` 执行期间安装动画上下文。`body` 内的状态变更会捕获该动画；随后重组时，触碰可动画节点属性的 `apply` 路径会注册 `AnimationController`，而不是立即写入新值。

```swift
@State private var expanded = false

withAnimation(Animation(duration: 0.2, curve: .easeOut)) {
    expanded.toggle()
}
```

`completion` 在名义截止时间以 `.logicallyComplete` 触发，或在弹簧物理尾段结束后以 `.removed` 触发。若后续动画取代了这些控制器，回调不会执行——被中断的动画不算完成。
