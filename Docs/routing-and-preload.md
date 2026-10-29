# 页面切换与滚动缓冲

`NavigationStack` 已提供压栈、返回、保留路径内页面和初始路径恢复。平级页面选择使用 `RouteOutlet`：它在独立组合 scope 中读取当前路由，只构建当前目的地，路由 key 改变时卸载旧页面。它不提供 URL 匹配，也不自动缓存所有页面。

```swift
@TrackStateReads
struct ApplicationPages: View {
    @State var page: Page = .home

    var body: some View {
        Row {
            Sidebar(selection: Binding(get: { page }, set: { if let next = $0 { page = next } }),
                    sections: sections)
            RouteOutlet(selection: $page) { route in
                route.makeContent()
            }
        }
    }
}
```

外壳只传递 binding，避免在构造外壳时读取 `page`。`@TrackStateReads` 使 `@State` 按实际读取的 scope 失效；普通 `@State` 仍保留所属视图必刷新的语义，因此单独套一个 outlet 不能阻止父视图自身重组。需要跨页面保留的数据放在会话或模型里；需要压栈保留页面时使用 `NavigationStack`。

Gallery 已按此方式拆分。Sidebar 的行 key 包括选中、展开、可见、Tab stop 和输入代次。选择变化可以复用其他行；新的父输入会更新代次，确保自定义 slot、菜单、动作和绑定能够刷新。行样式和布局回调不再重复订阅全局页面值。

标题、工具栏和页脚留在故事路由之外；切页只替换故事的滚动容器与内容，工具栏控件的节点身份保持不变。测试场景为 Button → Slider，旧页先完成两次绘制预热。本机 Release 单次测量为重组约 22.3 ms、Yoga 约 4.0 ms、绘制列表约 2.35 ms，三阶段 CPU 合计约 28.6 ms。这个合成回归不等于原 trace 的同场景复测，也不包含无障碍、GPU 提交和呈现；冷页面构建仍超过 16.7 ms，仍需针对构建与资源准备继续优化。

```swift
VirtualStack(items, id: \.id, rowHeight: 40,
             overscan: 3, preload: 0.3) { item in
    ItemRow(item: item)
}
```

`preload: 0.3` 按逻辑高度在视口两侧各扩展约 15%，提前创建、布局内容。固定行高、已知可变行高和估算行高都支持；可变行高也按高度比例扩展，不把一个巨大行当成若干普通行。范围会取整到完整行，再按 `overscan` 在每侧增加最小行数，因此小视口可能超过 130%。比例限制为 0…4，非有限值按 0.3 处理；`overscan: 0, preload: 0` 关闭额外区域。

实时滚动位置与组合状态分开保存。可见区域接近已准备范围的边缘时提前补充窗口，普通像素滚动只改变场景变换；视口尺寸、数据、行高、文字缩放与程序化跳转仍会更新。窗口补充时，显式 `MemoizedView` key 保留未变行的内容，只有新行需要构建。

这里准备的是视图、布局和测量，不是把屏外像素送到 GPU。屏外绘制仍按真实视口裁剪；第一次页面构建、字形/图像资源和可见几何的录制仍有成本，不能仅靠路由与 overscan 保证所有帧低于 16.7 ms。

`MemoizedView(key:content:)` 是显式的输入相等契约。key 必须包含影响内容和动作的全部输入；闭包或模型改变时也必须改变 key。普通视图不使用此契约，环境变化和子视图自身状态变化仍能穿过边界刷新。

验证使用 `cove test`。路由回归验证 Gallery 外壳保持不变、98 个 destination 中仅 2 个重组；滚动回归覆盖三类行高、缓冲内零重建、补充时保留行不重建，以及现有跳转、缩放、估算收敛、Sidebar 键盘与自定义 slot 行为。实现为平台共享代码；当前实机测试为 macOS。
