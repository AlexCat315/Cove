# 平台自适应

组件从 `cove_core::Environment` 读取四类互相独立的事实，各管一种决策：

| 事实 | 字段 | 生命周期 | 决定 |
| --- | --- | --- | --- |
| 平台惯例 | `platform`、`design_language` | 进程内恒定 | 外观、手势、命中区 |
| 硬件 | `device_class` | 进程内恒定，折叠屏开合不变 | 默认惯例 |
| 窗口 | `width_class()`、`height_class()`、`posture`、`display_features`、`segments()` | 随时变化 | 布局结构 |
| 输入 | `input` | 随时变化 | hover、焦点环、`minimum_control_extent()` |

布局只看窗口。折叠屏展开后得到平板布局，是因为宽度变成了 `Expanded`，而不是设备变了。

## 窗口分级

| `WidthClass` | 宽度 (pt/dp) |
| --- | --- |
| `Compact` | < 600 |
| `Medium` | 600–839 |
| `Expanded` | 840–1199 |
| `Large` | 1200–1599 |
| `ExtraLarge` | ≥ 1600 |

`HeightClass`：`Compact` < 480，`Medium` 480–899，`Expanded` ≥ 900。`WidthClass` 可比较大小，例如 `env.width_class() < WidthClass::Expanded`。

## 折叠屏与双屏

```rust
use cove_core::{DeviceClass, FoldableKind, Posture};

AnyView::new(SurfaceReader::new(|env: &Environment| {
    let segments = env.segments();
    if segments.len() == 2 {
        // 半开的折痕或物理铰链把窗口分成两块：各放一个面板。
    }
    if env.posture == Posture::Tabletop {
        // 上半屏内容，下半屏操作。
    }
    AnyView::new(content())
}))
```

- `DeviceClass::Foldable(FoldableKind::Book | Flip | DualScreen)`，`is_handheld()` 对手机与折叠屏都为真。
- `DisplayFeature { kind, bounds, orientation, occludes, separates }`，坐标为窗口逻辑坐标。`occludes` 表示区域内不可绘制（物理铰链）；`separates` 表示它把窗口分成两段（半开的折痕或铰链）。平放的折痕不分段。
- `env.segment_for(rect)` 返回应承载该矩形的区段，`display_features::confine(rect, segment)` 把弹层移进区段，避免跨越折痕。
- 读取器：`PostureReader`、`DeviceClassReader`、`PlatformReader`、`DesignLanguageReader`、`WidthClassReader`、`HeightClassReader`、`OrientationReader`，各自只在对应值变化时重组。

## 宿主来源

| 平台 | 设备类别 | 姿态与显示特征 |
| --- | --- | --- |
| Android | Jetpack WindowManager 的 `FoldingFeature`；合上时沿用上次见到的折叠类型，铰链角度传感器作为后备；否则按 `smallestScreenWidthDp` 区分手机与平板 | `FoldingFeature` 的状态、方向、遮挡与分隔 |
| iOS / iPadOS | `SDL_IsTablet` 区分 iPhone 与 iPad，iPad 的 `platform` 为 `IPadOS` | 暂无公开数据源 |
| macOS / Linux | 桌面 | 无 |
| Windows | 桌面；平板模式且有内置触摸屏时 `input` 加入 `TOUCH` | 无 |

## 预览与验收

在桌面上以其他设备运行：

```sh
COVE_PREVIEW_DEVICE=fold-tabletop cargo run -p cove-examples
```

预设：`iphone`、`ipad-split`、`android-phone`、`android-tablet`、`fold-cover`、`fold-open`、`fold-book`、`fold-tabletop`、`flip-open`、`dual-screen`、`macos`、`windows`、`linux`。

运行中切换用 DevTools 的 `set_ui_environment`：`preset` 从完整设备开始（桌面会同时调整窗口尺寸），显式字段在其上覆盖，`clear` 先清空。返回值是下一帧发布的环境，包括区段。

```json
{ "preset": "dual-screen" }
{ "platform": "android", "posture": "book" }
{ "clear": true }
```

覆盖只替换宿主报告的事实，不验证宿主自身的上报是否正确；Android 折叠数据需要在折叠屏模拟器或真机上单独验收。
