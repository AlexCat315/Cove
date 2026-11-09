# 平台自适应设计

> 状态：P0、P1 已实现，P2 部分实现（0.3.0-dev，见 §14）。1.0.0 之前按本文直接做破坏性变更，不保留兼容层。

## 目标

一套应用代码，在 macOS、Windows、Linux、iOS、iPadOS、Android、Web 上分别得到符合该平台惯例的结构、外观与手感，并在折叠屏、双屏、平板多窗口这类运行时形态变化中保持状态连续。

不是目标：

- 像素级复刻系统控件。Cove 自绘，目标是"遵循惯例"，不是"冒充系统"。
- 让应用代码按平台分支。平台差异由框架吸收，应用只在确有需要时使用逃生口（§10）。

## 1. 现状

| 位置 | 现状 | 问题 |
|---|---|---|
| [`adaptive_environment.rs`](../crates/core/src/adaptive_environment.rs) | `Idiom { Phone, Tablet, Desktop }`、`SizeClass` 三档（600/1000）、`InputCapabilities`、安全区、键盘 | 无操作系统维度，分不出 iOS 与 Android；无设计语言；无姿态与显示特征（铰链、折痕） |
| `crates/compose/src` | 仅 [`touch_presentation.rs`](../crates/compose/src/primitives/overlays/modal/touch_presentation.rs) 按 `Idiom` 分支 | 其余组件只看宽度或 `TOUCH`，窄窗口得到的是挤窄的桌面 UI |
| [`runtime.rs`](../crates/hosts/src/desktop/application/runtime.rs) | iOS 与 Android 编译目标一律 `Idiom::Phone` | iPad 被当成手机 |
| [`SurfaceMetrics.java`](../crates/hosts/android/app/src/com/cove/probe/SurfaceMetrics.java) | `smallestScreenWidthDp >= 600` 判为 Tablet | 折叠屏展开/合上时 `Idiom` 在 Phone 与 Tablet 之间跳变，惯例随之跳变 |
| 主题 | 只有 `DefaultLight` / `DefaultDark` | 无平台设计语言 |
| [`split_compact.rs`](../crates/compose/src/primitives/layout/split_compact.rs)、`mobile_tab_bar`、`navigation_stack`、`bottom_sheet` | 零件存在，按宽度阈值切换 | 没有统一的语义入口；不感知铰链 |

## 2. 原则

1. **语义 API**：组件描述意图（"列表-详情导航"、"临时呈现"、"主操作"），不描述形态（"左侧 280pt 侧栏"）。
2. **四个正交维度**，每个决策只读它该读的那一个：

   | 维度 | 回答 | 生命周期 | 决定什么 |
   |---|---|---|---|
   | `Platform` / `DesignLanguage` | 遵循哪套惯例 | 进程内恒定（可覆盖） | 外观、手势、转场、文本选择 |
   | `DeviceClass` | 硬件是什么 | 进程内恒定 | 默认交互惯例、可用姿态 |
   | `WindowClass` + `Posture` + `DisplayFeature` | 当前有多少、什么形状的空间 | 运行时随时变化 | 布局结构 |
   | `InputCapabilities` | 现在怎么操作 | 运行时变化（外接键盘鼠标、Windows 平板模式） | 命中区、hover、焦点环 |

3. **布局只看窗口，不看设备**。折叠屏展开后得到平板布局，是因为窗口变成了 Expanded，不是因为设备变成了 Tablet。
4. **形态变化不丢状态**。折叠、展开、旋转、多窗口缩放都是一次环境变化，不是重建。
5. **每个平台都要实现**。暂时无法真机验证的平台也写实现，并在验收表标为"结构覆盖，未视觉验收"。

## 3. 环境模型

`Idiom` 与 `SizeClass` 已删除。`cove_core::Environment` 保持扁平，新增字段与已有的 `viewport`、`safe_area`、`keyboard` 并列：

```rust
pub struct Environment {
    pub viewport: Viewport,
    pub safe_area: Insets,
    pub input: InputCapabilities,
    pub platform: Platform,                 // MacOS | Windows | Linux | IOS | IPadOS | Android | Web
    pub design_language: DesignLanguage,    // Cupertino | Material | Fluent | Adwaita
    pub device_class: DeviceClass,          // Phone | Foldable(Book | Flip | DualScreen) | Tablet | Desktop
    pub posture: Posture,                   // Flat | Tabletop | Book
    pub display_features: Vec<DisplayFeature>,
    pub text_scale: TextScale,
    pub keyboard: Keyboard,
    pub reduces_motion: bool,
    pub translucent_window: bool,
}

pub struct DisplayFeature {
    pub kind: DisplayFeatureKind,           // Fold | Hinge | Cutout
    pub bounds: Rect,                       // 窗口逻辑坐标，物理左右
    pub orientation: FeatureOrientation,    // Horizontal | Vertical
    pub occludes: bool,                     // 区域内不可绘制
    pub separates: bool,                    // 把窗口分成两段
}

impl Environment {
    pub fn width_class(&self) -> WidthClass;
    pub fn height_class(&self) -> HeightClass;
    pub fn segments(&self) -> Vec<Rect>;     // 按 separates 的特征切分，上到下、左到右
    pub fn is_spanned(&self) -> bool;
    pub fn segment_for(&self, rect: Rect) -> Rect;
    pub fn occluded_regions(&self) -> impl Iterator<Item = Rect> + '_;
    pub fn minimum_control_extent(&self) -> f32; // 触摸：Material 48，其余 44；指针 28
}
```

没有拆成 `device` / `window` 两个子结构：`viewport`、`safe_area` 有上百处读取，嵌套只增加迁移面而不改变语义。

### 3.1 窗口分级

宽度改用 Material 3 的窗口分级，高度保留三档：

| 档位 | 宽度 (dp/pt) | 典型 |
|---|---|---|
| Compact | < 600 | 手机竖屏、折叠屏外屏、翻盖展开 |
| Medium | 600–839 | 书本式展开竖持、小平板竖屏、iPad 分屏 |
| Expanded | 840–1199 | 书本式展开横持、平板横屏 |
| Large | 1200–1599 | 大平板、桌面窄窗口 |
| ExtraLarge | ≥ 1600 | 桌面 |

高度：Compact < 480，Medium 480–899，Expanded ≥ 900。高度 Compact 时（手机横屏、Tabletop 的上半屏），导航栏收进 rail，sheet 改全屏。

### 3.2 惯例默认值

`DeviceClass` 与 `Platform` 只决定默认惯例：

| 设备 | 默认命中区 | hover | 返回 | 默认设计语言 |
|---|---|---|---|---|
| iOS Phone / Foldable | 44 | 否 | 左缘右滑 | Cupertino |
| iPadOS | 44（指针接入时 28） | 指针接入时 | 左缘右滑 | Cupertino |
| Android Phone / Foldable / Tablet | 48 | 否 | 系统返回 + predictive back | Material |
| macOS | 28 | 是 | ⌘[ / 工具栏按钮 | Cupertino（macOS 变体） |
| Windows | 32（平板模式 40） | 是 | Alt+← / 标题栏按钮 | Fluent |
| Linux | 32 | 是 | Alt+← | Adwaita |
| Web | 随 `pointer: coarse` | 随媒体查询 | 浏览器历史 | 随 UA 推断 |

折叠屏无论开合，`device_class` 恒为 `Foldable`，惯例不跳变；展开后的布局变化全部来自 `WindowClass`。

### 3.3 覆盖

`DeviceProfile` 与 `WindowProfile` 的每个字段都可以覆盖，用于：

- 品牌统一：应用强制 `design_language`，例如所有平台都用 Material。
- 局部覆盖：子树强制 `Compact`，例如在桌面上画一个手机预览。
- 验收：在 macOS 上渲染 Android 书本式折叠屏的 Tabletop 姿态（§11）。

实现：`EnvironmentOverride`（全部字段可选，`apply` / `merged`）、`DevicePreset`（§11.1 的预设）、`EnvironmentRequest`（DevTools 请求：`clear` → `preset` → 显式字段）。`COVE_PREVIEW_DEVICE=<预设>` 取代了原来的 `previews_phone()`。

## 4. 宿主数据源

| 平台 | `DeviceClass` | 窗口与姿态 | 显示特征 | 运行时变化 |
|---|---|---|---|---|
| Android | `PackageManager.hasSystemFeature`（铰链角度传感器）+ Jetpack WindowManager 是否上报 `FoldingFeature`；Book/Flip 由 fold 方向相对设备自然方向判定，`occlusionType == FULL` 判为 DualScreen | `WindowMetricsCalculator` 求窗口尺寸；`FoldingFeature.state`（`FLAT` / `HALF_OPENED`）与 `orientation` 求姿态 | `FoldingFeature.bounds`、`occlusionType`、`isSeparating` | `WindowInfoTracker.windowLayoutInfo` 流；Activity 声明 `screenSize|smallestScreenSize|screenLayout|orientation|density` 配置变更自理，保持现有的"Activity 保留应用"模型 |
| iOS / iPadOS | `SDL_IsTablet`（内部即 `UIDevice.userInterfaceIdiom`）区分 Phone 与 Pad，iPad 上报 `Platform::IPadOS`。可折叠 iPhone：先按 Phone 接入，待 UIKit 提供公开的姿态/铰链信息后上报 `Foldable` | `UIWindowScene` 几何与 trait collection 变化；Stage Manager、Split View、Slide Over 都表现为窗口尺寸变化 | 当前为空；接口已就绪，等 UIKit 有公开数据源时填充 | `viewWillTransition(to:)` / scene 几何回调 |
| macOS | Desktop | 窗口尺寸 | 无 | 窗口缩放、全屏、分屏 |
| Windows | Desktop；`GetSystemMetrics(SM_CONVERTIBLESLATEMODE)` 为平板模式时补 `TOUCH` | 窗口尺寸 | 无 | `WM_SETTINGCHANGE("ConvertibleSlateMode")` 更新 `input` |
| Linux | Desktop | 窗口尺寸 | 无 | 窗口缩放 |
| Web | 由 `pointer` / `hover` 媒体查询推断 | `innerWidth` / `innerHeight` | Viewport Segments API 与 Device Posture API（实验性，存在时使用） | `resize`、`change` 事件 |

注意：

- **外屏与内屏密度不同**。折叠屏开合时 `viewport.scale` 可能改变，Text 的字形图集与 Imaging 的缓存要按新 scale 失效重建。这是现有 scale 变化路径，折叠屏只是让它更频繁。
- Android 依赖：Gradle 运行时库引入 `androidx.window:window` 与 `window-java` 1.4.0（以及 `androidx.core`）。`FoldingState.java` 在 `onStart` / `onStop` 注册监听，合上（外屏不报告特征）时沿用 `SharedPreferences` 中上次见到的折叠类型，铰链角度传感器作为后备。
- JNI `surface(...)`：`idiom` 换成 `device`（0 手机、1 平板、2 书本式、3 翻盖、4 双屏）、`posture`（0 平放、1 桌面支架、2 书本）与 `float[] features`（每项 8 个数：kind、x、y、width、height、orientation、occludes、separates，逻辑坐标），Rust 侧由 `DeviceFacts::decode` 解码。
- Activity 在折叠/展开时仍按默认流程重建，应用状态由现有的 `onRetainNonConfigurationInstance` 保留；声明 `configChanges` 自理留待 P2 与 §6 一起做。

## 5. 语义组件

每个组件一个入口，解析规则内置。"折叠"一列指同一台设备在不同形态下的结果。

| 组件 | 桌面 | 手机 | 平板 | 折叠屏外屏 | 书本式展开（Flat） | Book 姿态 | Tabletop 姿态 |
|---|---|---|---|---|---|---|---|
| `ListDetail`（类 `NavigationSplitView`） | 侧栏 + 详情 | push 栈 | 分栏，竖屏侧栏可收起 | push 栈 | 分栏，**分界对齐折痕** | 左页列表、右页详情 | 上半屏详情、下半屏列表 |
| `AdaptiveTabs` | 侧栏或顶部 tab | 底部 tab bar | 侧栏 / rail | 底部 tab bar | rail | rail | 底部，位于下半屏 |
| `present_sheet` | 居中模态 | iOS page sheet + detents / Android bottom sheet | form sheet | 同手机 | **限制在单个区段内居中**，不跨折痕 | 同左 | 下半屏 |
| `Menu`、上下文菜单 | 下拉 / 右键 | 长按 | 长按，指针接入时右键 | 长按 | 长按，锚定在触发点所在区段 | 同左 | 同左 |
| `Toolbar(placement)` | 窗口工具栏 | 导航栏 + 底部工具栏 / Top app bar + 溢出菜单 | 导航栏 | 同手机 | 导航栏，次要操作展开 | 同左 | 操作移到下半屏 |
| `Picker` | 弹出菜单 | iOS 菜单或滚轮 / Android 下拉或对话框 | 菜单 | 同手机 | 菜单 | 同左 | 同左 |
| `List` | 行 hover、选中、多列 | insetGrouped + 左滑操作 / Material 列表 | 介于两者之间 | 同手机 | 同平板 | 同平板 | 同平板 |
| `MediaPlayer` | 浮动控件 | 全屏 + 叠加控件 | 同手机 | 同手机 | 同手机 | 同手机 | **上半屏画面、下半屏控件** |

### 5.1 跨铰链规则

所有布局原语遵守：

1. **不在 `occludes` 区域内放内容**。DualScreen 的铰链区域视为不可用插入区，与安全区同等处理。
2. **不让可交互元素或单行文本跨越 `separates` 的特征**。弹层、对话框、toast、菜单定位时先选区段，再在区段内定位。
3. **分栏边界吸附到折痕**。`ListDetail`、`SplitView`、`ResizableEditor` 在存在竖向分隔特征时，默认把分界放在特征上，拖动时在特征处吸附。
4. 全屏媒体、地图、画布可以跨越不遮挡的折痕（`occludes == false`），这是它们的默认行为。

### 5.2 新增原语

```rust
/// 按区段布局：一个区段时退化为 single，两个区段时分别放 first / second。
SegmentedLayout::new()
    .single(|| content())
    .split(|| primary(), || secondary())

/// 按姿态布局，未匹配的姿态回落到 flat。
PostureLayout::new()
    .flat(|| player())
    .tabletop(|| video(), || controls())
    .book(|| page_left(), || page_right())
```

`ListDetail`、`MediaPlayer` 等语义组件内部用这两个原语实现，应用也可以直接用。

### 5.3 组件形态

语义容器只决定结构；每个组件还要按平台改变自己的形态。组件分三类：

| 类别 | 组件 | 要求 |
|---|---|---|
| 通用 | Button、Toggle、Checkbox、Radio、Slider、RangeSlider、Rating、TextField、Select、Badge、Progress、Spinner、Avatar、Tag、Alert、EmptyState、Skeleton、Link、Stepper、Carousel、Chart、Image、Markdown | 跟随设计语言换样式（§7）；触摸时命中区 ≥ 44（Material 48）；不依赖 hover |
| 桌面惯用，手持需换形态 | Sidebar、Tree、DataTable、Table、PropertyGrid、SplitView、Workspace / Dock、StatusBar、Toolbar、Tooltip、HoverCard、ContextMenu、Command Palette、Breadcrumb、Kbd、SplitButton、Scrollbar、Pagination、Menu bar | 见下表 |
| 手持惯用，桌面需对应物 | BottomSheet、ActionSheet、SwipeList、RefreshableList、WheelPicker、MobileTabBar、HorizontalSwipe | 见下表 |

| 组件 | 手持形态 | 桌面形态 |
|---|---|---|
| Sidebar | 平台列表：Cupertino inset grouped（圆角分组、整行可点、尾部箭头），Material 列表（整宽行、前置图标）；不保留选中高亮 | 侧栏面板与选中高亮 |
| Tree | 逐层下钻（每层一个 push 页） | 展开折叠 |
| DataTable / Table | 卡片列表，或冻结首列横向滚动 | 表格 |
| PropertyGrid | 分组表单 | 两列网格 |
| SplitView / Workspace | `ListDetail` 或单栏 | 可拖动分栏 |
| Toolbar | 导航栏 + 底部工具栏 / 溢出菜单 | 窗口工具栏 |
| Tooltip / HoverCard | 不显示，或点按 popover | 悬停显示 |
| ContextMenu | 长按菜单或 action sheet | 右键 |
| Command Palette | 全屏搜索页 | 浮层 |
| Breadcrumb | 返回 + 标题 | 路径 |
| Kbd | 不显示 | 显示 |
| BottomSheet / ActionSheet | 底部弹出 | 居中对话框 / 锚定菜单 |
| SwipeList | 滑动操作 | 悬停显示的行内操作 + 右键 |
| RefreshableList | 下拉刷新 | 刷新命令 |
| WheelPicker | 滚轮 | 下拉选择 |

约定：

1. 形态由组件自己从环境（设计语言、设备类别、输入、宽度）决定，应用代码不分支。
2. 只适合桌面的呈现在节点上标记 `adaptation.desktop-only`，只适合手持的标记 `adaptation.handheld-only`，供审计识别（§11.3）。
3. 每个组件的用户文档写明两种形态。

## 6. 形态变化时的状态连续性

折叠、展开、旋转、多窗口缩放是一次 `Environment` 更新，组合树原地重算，不卸载。

### 6.1 导航状态是唯一真相

`ListDetail` 持有一个与形态无关的模型，栈与分栏只是它的两种投影：

```rust
pub struct ListDetailState<Id> {
    pub selection: Option<Id>,
    pub detail_path: Vec<Route>,
    /// 在单栏投影下，用户是否停留在详情上。
    pub detail_focused: bool,
}
```

| 变化 | 规则 |
|---|---|
| 单栏 → 分栏（展开） | 列表和详情同时可见；有 `selection` 时详情显示它，否则显示占位；`detail_path` 保留 |
| 分栏 → 单栏（合上） | `detail_focused` 为真时，栈顶是详情（含 `detail_path`）；否则停在列表并滚动到 `selection` |
| 单栏中返回 | `detail_focused = false`，`selection` 保留，展开后详情仍在 |

### 6.2 其他需要保留的状态

- 滚动位置：按内容锚点保留，不按像素偏移。窗口变宽后同一行仍在视口内。
- 文本输入：焦点、选区、IME 组合串不丢失；宿主重建输入连接后重新附着。
- 媒体播放不暂停；Tabletop 进出只换布局。
- 正在展示的 sheet / 菜单：重新解析呈现形态，例如 bottom sheet 变成居中对话框，内容状态保留；锚点失效时关闭菜单。

### 6.3 过渡动画

形态变化时，语义组件之间做共享元素过渡（列表行 ↔ 详情标题），其余部分交叉淡入。`reduces_motion` 时直接切换。

## 7. 设计语言与样式

- 每种 `DesignLanguage` 一套 token：字体族（SF / Roboto / Segoe UI Variable / Cantarell）、字号阶、圆角、间距、控件高度、阴影或分层方式、强调色规则、动效曲线。
- 每个控件一个样式协议，类似 `ButtonStyle` / `ToggleStyle`：`theme/styles/components` 按设计语言提供实现表，例如 `Toggle` 在 Cupertino、Material 下画开关，在桌面 Fluent、Adwaita 的表单里画复选框。
- 解析顺序：子树显式样式 > 应用强制的设计语言 > 平台默认。
- 浅色 / 深色仍是 `ColorScheme`，与设计语言正交。

## 8. 行为

| 行为 | Cupertino | Material | 桌面 |
|---|---|---|---|
| 滚动越界 | 回弹 | stretch overscroll | 无，滚轮按行，触控板惯性 |
| push 转场 | 右侧滑入 + 视差，边缘手势可交互返回 | 淡入 + 缩放，predictive back 预览 | 无或淡入 |
| 文本选择 | 放大镜 + 拖动柄 + 编辑菜单 | 拖动柄 + 浮动工具条 | 鼠标拖选、双击选词、右键菜单 |
| 次要操作 | 长按 | 长按 | 右键 |
| 触感 | 选择、冲击反馈 | 震动反馈常量 | 无（macOS 触控板力反馈可选） |
| 焦点环 | 键盘接入时显示 | 键盘接入时显示 | 始终可用 |

实现落点：[`scroll_physics_environment.rs`](../crates/compose/src/scrolling/scroll_physics_environment.rs) 按设计语言选择物理；`long_press.rs` 与上下文菜单统一成"次要操作"手势；`navigation_stack` 的转场由设计语言提供。

## 9. 原生桥接

自绘做不到符合惯例的部分直接调用系统：macOS 菜单栏、分享面板、文件与照片选择器、触感、无障碍树、IME、系统返回键、通知权限。契约放在 `cove-platform`，实现放在 `cove-shell` 或 `cove-hosts`，与现有屏幕取色的分层一致。每个平台都要有实现，确实不可能实现的返回 `Unsupported`，并在 UI 中隐藏入口。

## 10. 应用逃生口

```rust
EnvironmentReader::new(|env| match env.device.design_language {
    DesignLanguage::Cupertino => ios_specific(),
    _ => shared(),
})
```

只读，不鼓励使用；gallery 示例中不得出现。

## 11. 验证

### 11.1 设备配置

DevTools `set_ui_environment` 接受 `clear`、`preset` 与 `EnvironmentOverride` 的各字段；预设：

| 预设 | 平台 | 设备 | 窗口 | 姿态 / 特征 |
|---|---|---|---|---|
| `iphone` | IOS | Phone | 393×852 | Flat |
| `ipad-split` | IPadOS | Tablet | 694×1024 | Flat |
| `android-phone` | Android | Phone | 412×915 | Flat |
| `android-tablet` | Android | Tablet | 1280×800 | Flat |
| `fold-cover` | Android | Foldable(Book) | 360×800 | Flat |
| `fold-open` | Android | Foldable(Book) | 841×701 | Flat，竖向折痕不遮挡 |
| `fold-book` | Android | Foldable(Book) | 841×701 | Book |
| `fold-tabletop` | Android | Foldable(Book) | 701×841 | Tabletop，横向折痕 |
| `flip-open` | Android | Foldable(Flip) | 412×915 | Flat |
| `dual-screen` | Android | Foldable(DualScreen) | 1114×705 | 竖向铰链，遮挡 34 |
| `macos` / `windows` / `linux` | 对应平台 | Desktop | 1280×800 | — |

尺寸是验收用的典型值，不对应特定机型。

### 11.2 验收矩阵

- 每个 gallery 页面跑全部预设，截图进入快照测试。`Docs/gallery-audit.md` 增加"自适应"列，逐预设记录。
- 动态用例：在预设之间切换（`fold-cover → fold-open → fold-tabletop → fold-cover`），断言 §6 的状态保留，并用 `start_ui_trace` / `stop_ui_trace` 记录过渡帧耗时。
- 结构覆盖与真机验收分开记录。覆盖注入不验证宿主上报是否正确，宿主数据源要在 Android Studio 的折叠屏与可调尺寸模拟器上、以及有条件时在真机上单独验收。

### 11.3 适配审计

`examples/gallery/tests/adaptation_audit.rs` 在每个预设下渲染 Gallery 的每一页，逐条检查：

| 规则 | 判定 |
|---|---|
| `touch-target` | 触摸表面上，可交互节点（按钮、开关、链接等可访问角色且可命中）宽高都 ≥ `minimum_control_extent()` |
| `horizontal-overflow` | Compact 宽度下，可见节点不超出窗口右缘 |
| `desktop-only` | 手持设备上出现标记 `adaptation.desktop-only` 的节点 |
| `handheld-only` | 指针桌面上出现标记 `adaptation.handheld-only` 的节点 |

结果与 `examples/gallery/tests/adaptation_baseline.txt` 比较：新增违规让测试失败；基线里已不再出现的条目也让测试失败，要求删除。基线只减不增，是逐项修复的清单。`COVE_ADAPTATION_AUDIT_WRITE=1` 重写基线（仅在有意接受时使用）。

## 12. 分阶段

| 阶段 | 内容 | 完成标准 |
|---|---|---|
| P0 | §3 环境模型；§4 全部宿主数据源（含 Android `FoldingFeature`、iPad idiom 修正）；§3.3 覆盖与 DevTools `set_ui_environment` | 所有宿主上报完整 `Environment`；预设可在桌面渲染 |
| P1 | `SegmentedLayout`、`PostureLayout`；§5.1 跨铰链规则接入 overlay 定位与 split | 弹层、对话框、分栏在 `fold-open`、`dual-screen` 下不跨铰链 |
| P2 | `ListDetail`、`AdaptiveTabs`、`present_sheet`、`Menu`、`Toolbar` 语义入口，以及 §6 状态连续性 | 动态用例全部通过 |
| P3 | §8 行为：滚动物理、转场、返回手势、文本选择 | 各设计语言手感用例通过 |
| P4 | §7 设计语言样式表 | 每个控件在四种设计语言下都有截图 |
| P5 | §9 原生桥接 | 平台覆盖矩阵更新到 `crates/hosts/src/PLATFORM_COVERAGE.md` |

每个阶段发布时写 changelog，并在 `Website/src/content/components/` 更新受影响组件的文档。

## 13. 未决问题

1. ~~Android 宿主是否引入 `androidx.window`~~：已引入。
2. 可折叠 iPhone 的姿态与铰链信息何时、以何种形式由 UIKit 公开。现在只预留接口。
3. 宽度分级已改为 Material 3 五档；`Viewport::EXPANDED_MINIMUM_WIDTH` 从 1000 变为 840，`SidebarAdaptiveOptions` 与工作区布局随之在 840 进入双栏。
4. Web 的 Viewport Segments、Device Posture API 仍是实验性 API，可用性随浏览器变化，需要特性检测。

## 14. 实施状态

| 项 | 状态 | 位置 |
|---|---|---|
| 环境模型、窗口分级、区段计算 | 完成，结构测试 | `crates/core/src/{adaptive_environment,device_profile,display_features,environment_override}.rs`，`crates/core/tests/adaptive_environment.rs` |
| 组合读取器 | 完成 | `crates/composition/src/adaptive/viewport_traits.rs`（`PostureReader`、`DeviceClassReader`、`PlatformReader`、`DesignLanguageReader`） |
| Android 数据源 | 完成，**未编译验证**（本机无 Android SDK/JDK），未设备验收 | `FoldingState.java`、`SurfaceMetrics.java`、`jni.rs`、`surface/device_facts.rs` |
| iOS / iPadOS 数据源 | 完成，未设备验收 | `cove_shell_device_form` → `Sdl3PlatformHost::device_form` |
| Windows 平板模式 | 完成，未实机验收 | `native.c` 的 `SM_CONVERTIBLESLATEMODE` + `SM_DIGITIZER` |
| macOS / Linux | 完成 | 桌面，无显示特征 |
| Web | **缺口**：`cove-browser` 原型不发布 `Environment`，组件得到默认值（`Platform::Web`、Material） | — |
| 预设与覆盖 | 完成 | `COVE_PREVIEW_DEVICE`，DevTools `set_ui_environment`（桌面会调整窗口尺寸；Android 只替换事实） |
| 跨铰链规则（P1） | 完成：菜单、Popover、上下文菜单、Hover card 放进锚点所在区段；Dialog、Sheet、Toast 放进呈现区段（横向折痕取下半屏，纵向取 leading），遮罩仍覆盖窗口 | `crates/compose/src/primitives/overlays/portal/segments.rs`、`display_features::presentation_segment` |
| `SegmentedLayout`、`PostureLayout`（P1） | 完成；分栏边界吸附折痕（§5.1 第 3 条）留待 P2 的 `ListDetail` | `crates/compose/src/primitives/layout/segments.rs` |
| Android 实机链路 | 模拟器（Pixel 9 Pro Fold AVD）验证了 `FoldingFeature`：平放、桌面支架、书本、合上、旋转后设备类别保持 `Book`。渲染未验证：模拟器 SwiftShader 缺 `shaderDrawParameters`，gfxstream 在 `VkFramebuffer` 上崩溃 | `FoldingState.java` 的 `com.cove.android.window` 日志 |
| `ListDetail`（P2） | 完成：单栏 / 双栏 / 对齐折痕 / 桌面支架；§6.1 状态模型（`ListDetailState`）；列表与详情槽位固定，开合不重建 | `crates/compose/src/primitives/navigation/list_detail.rs` |
| `AdaptiveTabs`（P2） | 完成：底部 tab bar / rail / 顶部 tab 条，内容槽位固定 | `crates/compose/src/primitives/navigation/adaptive_tabs.rs` |
| 单栏转场 | 未做：单栏中列表与详情直接切换，没有 push 动画与边缘返回手势（`NavigationStack` 有，但会让槽位跨父节点而重建） | P2 后续 |
| `present_sheet`、`Menu`、`Toolbar` 语义入口 | 未开始 | P2 后续 |
| Gallery 外壳（P2） | 完成：`ListDetail` 承载导航与页面；600 起并排，折痕 / 铰链 / 桌面支架按 §5 | `examples/gallery/src/gallery_content/gallery_view.rs` |
| 组件形态（§5.3） | 进行中：审计与基线，Sidebar 手持列表 | §11.3 |
