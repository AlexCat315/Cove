CoveUI 是 Cove 的 Swift 跨平台 UI 框架。应用用声明式视图描述界面，所有平台共享同一套视图图、布局、文本与图像管线；绘制由 NativeRHI 落到 Metal、DirectX 12 或 Vulkan。

SDK 安装包提供 macOS arm64、Windows x86_64、Linux x86_64 三套主机产物。Apple 平台最低版本为 macOS 26 和 iOS 26。构建应用需要 Swift 6.4.1 与对应平台的编译工具。

## 应用入口

`Application` 负责窗口、设备、DrawList 绘制器、`TextEnvironment` 与 `ViewGraph` 的装配。最小的应用只需要一个根视图：

```swift
import CoveUI

struct Greeting: View {
    let name: String

    var body: some View {
        Column(spacing: 12) {
            Text("Hello, \(name)")
            Button("继续") { }
        }
    }
}

try Application.run(config: ApplicationConfiguration(title: "Greeting")) {
    Greeting(name: "Cove")
}
```

## 视图与视图图

`View` 只要求一个 `body`。`body` 由 `ViewGraph` 读取并物化为节点，应用代码不应直接调用它。

原始视图（`Text`、`Box`、`ScrollView` 等）遵循 `_PrimitiveView`，直接产出节点与 Yoga 布局节点；组合视图只负责拼装子视图。

## 布局

布局由 Yoga 的 flexbox 实现。`Box` 暴露 `direction`、`alignItems`、`justifyContent`、`wrap` 与 `spacing`，`Row` 与 `Column` 是固定方向的薄封装。尺寸与伸缩通过修饰符表达：

```swift
Row(spacing: 12) {
    Text("标题").flex(1)
    Button("操作") { }
}
.padding(16)
.frame(height: 44)
```

`frame` 接受点值与百分比（`FrameDimension`），`flex` 对应 flex-grow / flex-shrink / flex-basis。省略的维度保持内容自身的尺寸。

## 主题

`Theme` 是值类型，包含颜色、排版、间距、圆角、阴影与动效六组 token，另有输入外观、图标与扩展。`ThemeReader` 在构建期读取当前主题值：

```swift
ThemeReader { theme in
    Text("Accent").foregroundColor(theme.colors.accent)
}
```

需要换肤时提供一个新 `Theme` 值即可；各 token 容器同样是值类型，可以基于 `Theme.defaultDark` 复制并修改单个字段。

## 模块分层

| 模块 | 职责 |
| --- | --- |
| `CoveUIComposeCore` | `View` 协议、`ViewBuilder`、视图图、`Box` / `Row` / `Column` 等原始容器 |
| `CoveUICompose` | 修饰符、控件、主题、动画、滚动与文本编辑 |
| `CoveUICore` | 状态、绑定、几何、颜色与文本布局等共享值类型 |
| `CoveUIText` | 字体、塑形与段落布局 |
| `CoveUIGraphics` | NativeRHI 与 Metal / DirectX 12 / Vulkan 后端 |
| `CoveUIShell` | 窗口、原生菜单与显示器外壳 |
| `CoveUI` | 应用宿主，提供 `Application.run` |

## 平台

同一份 Compose 源码由桌面、浏览器与 Android 宿主复用（见 `Examples/Counter`）。宿主在绘制边界把文本塑形进共享字形图集，因此布局、命中测试与无障碍语义在各平台保持一致。

主机原生库随 SDK 安装包提供；Android 与 iOS 产物按目标下载到同一份共享 SDK。
