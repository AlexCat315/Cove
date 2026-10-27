# 视图与布局

Cove 的 UI 是声明式的：你描述界面"应该是什么样"，框架负责在状态变化时高效更新。

## 定义一个视图

所有视图遵循 `View` 协议，通过 `body` 组合其他视图：

```swift
import CoveUI

struct GreetingView: View {
  let name: String

  var body: some View {
    Text("你好，\(name)")
      .font(.title)
      .foregroundStyle(.cyan)
  }
}
```

## 状态驱动

使用 `@State` 持有可变状态，状态变化时视图自动重新计算：

```swift
struct CounterView: View {
  @State var count = 0

  var body: some View {
    VStack(spacing: 16) {
      Text("\(count)")
        .font(.system(size: 48, weight: .bold))

      Button("加一") { count += 1 }
        .padding(.horizontal, 24)
        .background(.cyan)
        .cornerRadius(999)
    }
  }
}
```

## 布局容器

Cove 使用基于 Yoga 的 Flexbox 布局，核心容器与 Web Flexbox 一一对应：

| 容器 | 对应 Flexbox | 典型用途 |
|------|-------------|---------|
| `VStack` | `flex-direction: column` | 垂直排列 |
| `HStack` | `flex-direction: row` | 水平排列 |
| `ZStack` | 堆叠（absolute） | 层叠覆盖 |
| `List` | 可滚动列表 | 大量同构数据 |

```swift
HStack(spacing: 12) {
  Avatar(url: user.avatar)
  VStack(alignment: .leading) {
    Text(user.name).font(.headline)
    Text(user.bio).font(.caption).foregroundStyle(.secondary)
  }
}
.padding(16)
```

## 修饰符链式调用

视图通过修饰符（modifier）调整外观与布局，顺序会影响结果：

```swift
Text("Cove")
  .padding(20)                 // 先内边距
  .background(.blue)           // 背景覆盖 padding 区域
  .cornerRadius(12)            // 再裁剪圆角
  .shadow(radius: 8)           // 最后投影
```

## 文本与字体

文本管线由 FreeType 与 HarfBuzz 驱动，支持复杂脚本、连字与 emoji：

```swift
Text("Hello 世界 🌊")
  .font(.custom("Inter", size: 18))
  .multilineTextAlignment(.center)
  .lineLimit(2)
```

## 图像

```swift
// 普通位图
Image("banner")

// GPU 生产的图像：直接进入绘制与资源持有流程
TextureImage(gpuTexture)

// 离屏渲染并返回快照
OffscreenView {
  ChartView(data: points)
}
```

## 列表与复用

对大量数据使用 `List`，框架会按需构建与回收视图：

```swift
List(items) { item in
  ItemRow(item: item)
}
```
