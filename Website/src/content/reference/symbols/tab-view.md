`TabView` 是单面板可见的多面板容器：`tabs` 声明所有面板，`selection` 决定当前显示哪一个。

```swift
enum Tab: Hashable { case today, projects }

@State private var tab = Tab.today

TabView(selection: $tab, tabs: [
    TabItem("Today", id: Tab.today) { TodayScreen() },
    TabItem("Projects", id: Tab.projects) { ProjectsScreen() },
])
```

`TabItem` 的 `configure` 闭包可调整单项配置（例如 `$0.icon`）。`TabView` 还有 `variant`、`placement`、`overflow` 等属性，用来选择标签栏形态、放置位置与溢出行为。标签 ID 必须唯一，构造时会校验。
