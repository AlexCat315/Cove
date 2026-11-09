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

## 跨折痕布局

```rust
use cove_ui_compose::primitives::layout::{PostureLayout, SegmentedLayout};

// 一块区段时显示 single；折痕或铰链把本视图分成两块时，各放一个面板。
SegmentedLayout::new(list_and_detail())
    .split(list(), detail());

// 桌面支架：上半屏内容、下半屏操作；书本：左右两页；其余显示 flat。
PostureLayout::new(player())
    .tabletop(video(), controls())
    .book(page_left(), page_right());
```

- 两者都量取自身在窗口中的位置，再与区段求交，所以放在侧栏旁边、只占部分窗口时也按真实位置切分；整个落在折痕一侧时不切分。
- 遮挡的铰链区域留空，不放内容。
- 首帧尚未量到位置，显示 single / flat，下一帧切换。
- `SegmentSplit::resolve(env, frame)` 给出同样的切分结果，可用于自定义布局。

## 导航

### ListDetail

```rust
use cove_ui_compose::primitives::navigation::{ListDetail, ListDetailState};

let state = cx.state(ListDetailState::<MailboxId>::default);
ListDetail::new(
    Binding::new(move || state.get(), move |value| state.set(value)),
    |selection| mailbox_list(selection),          // 行里 selection.set(Some(id))
    |id, context| mailbox(id, context),           // context.is_single_pane 时提供返回
)
.placeholder(empty_mailbox())
.list_width(320.0);
```

| 窗口 | 呈现 |
| --- | --- |
| 宽度 < 840，无折痕穿过 | 单栏：列表，选中后详情覆盖列表 |
| 宽度 ≥ 840 | 左列表（`list_width`）+ 发丝线 + 右详情 |
| 竖向折痕或铰链穿过本视图，两侧都 ≥ 240 | 分界落在折痕上；铰链区域留空 |
| 桌面支架（横向折痕分隔） | 上方详情，下方列表 |

- `ListDetailState { selection, showing_detail }` 与呈现无关：单栏里返回只清除 `showing_detail`，展开后详情仍显示原选择。
- 列表与详情在所有呈现中占同一个槽位，折叠、展开、旋转、缩放不会重建它们，滚动位置、草稿与焦点得以保留。
- `ListDetailPresentation::resolve(env, frame, list_width)` 给出同样的判断。

### AdaptiveTabs

```rust
AdaptiveTabs::try_new(selection, vec![
    TabItem::new(Tab::Home, "Home", home()),
    TabItem::new(Tab::Search, "Search", search()),
])?
```

| 环境 | 呈现 |
| --- | --- |
| 宽度 Compact | 底部 tab bar |
| 更宽，且为手机、折叠屏、平板或无 hover | 导航栏（rail） |
| 有指针的桌面 | 内容上方的 tab 条 |

选中页的内容在三种呈现中共用一个槽位，不会因切换呈现而重建。

## 组件形态

组件分三类：通用组件只随设计语言换样式和触摸尺寸；桌面惯用组件在手持设备上换成手机形态；手持惯用组件在桌面上换成对应物。完整对照见 `Docs/adaptive-platform.md` §5.3。

已适配：

| 组件 | 手持 | 桌面 / 平板 |
| --- | --- | --- |
| `Sidebar` | 导航列表：Cupertino 圆角分组、整行可点、尾部箭头；Material 整宽行。行高 ≥ 触摸目标，不保留选中高亮 | 侧栏面板与选中高亮 |
| `TextField`（单行） | 触摸表面上高度 ≥ 44（Material 48），文字垂直居中 | 设计尺寸 |

`Sidebar::adapts(false)` 保留作者指定的呈现。只适合一种设备的呈现用 `AdaptationMark::DesktopOnly` / `HandheldOnly` 标记在节点上，供审计识别。

### 适配审计

`cargo test -p cove-examples --test adaptation_audit` 在 iPhone、Android 手机、桌面支架、iPad 分屏、macOS 下渲染每一页，检查触摸目标、Compact 宽度横向溢出、手持设备上的桌面专属呈现与桌面上的手持专属呈现。计数与 `examples/gallery/tests/adaptation_baseline.txt` 比较，只能减少：

```sh
COVE_ADAPTATION_AUDIT_ONLY=iphone/textFields COVE_ADAPTATION_AUDIT_DETAIL=1 \
  cargo test -p cove-examples --test adaptation_audit -- --nocapture
COVE_ADAPTATION_AUDIT_WRITE=1 cargo test -p cove-examples --test adaptation_audit
```

## 弹层不跨折痕

框架内的弹层自动遵守区段，应用无需处理：

| 弹层 | 规则 |
| --- | --- |
| 菜单、Popover、上下文菜单、Hover card | 放进锚点所在的区段，在区段内翻转和夹紧 |
| Dialog、Sheet、Action sheet、Bottom sheet | 遮罩覆盖整个窗口，面板放在一个区段内：横向折痕取下半屏，纵向折痕取 leading 区段（RTL 时为右侧） |
| Toast | 与 Dialog 相同的区段 |

没有折痕或铰链时，行为与原来完全一致。

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

覆盖只替换宿主报告的事实，不验证宿主自身的上报是否正确。Android 的折叠数据可以在折叠屏模拟器上检查：每次窗口布局变化，日志 `com.cove.android.window` 都会输出设备类别、姿态与折痕，`adb emu fold`、`adb emu unfold`、`adb emu posture 2`、`adb emu rotate` 可切换形态。
