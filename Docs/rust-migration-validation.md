# Rust 迁移验证

日期：2026-11-02。版本：`0.3.0-dev`（未发布）。

第三批分组文档的 65 个目标已存在；另补入未登记的 `DesignSystemTests.swift` 对应套件。冻结清单保留 914 个生产源文件、411 个测试及辅助文件、23 个 SwiftPM 清单的原路径、行数、SHA-256 与目标。源文件删除后不再按名称猜测或删除历史行。

## 目录

- 根包：`src/`、`tests/`。
- 库：`crates/<name>/src/`、`tests/`、`native/`、`resources/`，构建脚本为根 `build.rs`。
- 示例：`examples/gallery/`、`examples/counter/`、`examples/mobile/`。
- 打包及审计：`xtask/`；执行 `cargo xtask audit --json`。
- 原生依赖子模块统一在 `third-party/`，由 `cove-native-build` 编译到各 crate 的 `OUT_DIR`；其中的第三方 Swift 文件不属于 Cove 源码清理范围。

## 验证记录

最终目录验证进行中。原目录基线完成编译并运行了 754 个通过的用例后，主动停止以进行目录迁移；该轮不是全量通过结论。

原生依赖构建（2026-11-01，macOS arm64 主机）：`cove-text`、`cove-shell`、`cove-imaging`、`cove-terminal`
从 `third-party/` 子模块编译通过，四个 crate 的测试全部通过；`aarch64-linux-android`（NDK r30）与
`aarch64-apple-ios-sim` 交叉编译通过（仅 rlib，未链接 APK/应用，未上设备）。Windows、Linux 未在本机验证。

## 已知限制

测试目标存在、通过本机测试和达到 Swift 行为等价是三个不同的结论。既有测试中存在明确记录的 `GAP` / `Named differences`：例如自适应侧栏的布局、模态焦点及动画，Table/DataTable 的字号缩放，富文本选择及编辑行为。测试固定这些差异，不代表差异已经修复。

## Gallery 页面验收（2026-11-02）

98 个目录页面中 97 个已渲染真实控件，1 个仍无故事：

| 项 | 状态 |
| --- | --- |
| `StoryDispatch` 分支 | 97 / 98；`Workspace` 仍为 `Pending` |
| 「缺失控件」清单 | 已删除：`Slider`、`TextField`、`Combobox`、`Tree`、`TabItem`、`Tooltip`、`PropertyGrid`、`CarouselPage`、`Breadcrumb`、`Pagination`、`Popconfirm`、`CommandPalette`、`HoverCard`、`AppBar`、`Rating`、`WheelPickerOption`、`TimeInput`、`DateRangePicker` 全部存在于 `crates/compose` |
| 故事来源记录 | `StorySource::entries()` 只记录每页的 Swift 文件，不再声称控件缺失 |

同批修复的行为：`Button::title` 现在以可见标题作为可访问名称；`PlacementExample` 的触发器循环不再停在下标 1；
`AnimationDemoNote` 补上 `body`；`Tree`、`Tabs`、`NavigationStack`、`TimePicker`、`Combobox`、`Notification`、`Tooltip`、
`Form` 由文字外壳改为真实控件。

抽屉失败用例已修正（2026-11-02）：此前归因为 `ModalFocusResource` 卸载时机不正确，实际原因是
`drawer_tests` 的 `Shared<V>` 包装只转发 `body`，没有转发 `dynamic_properties`。回车修改了故事状态，
但没有触发重组，抽屉视图仍挂载，因而焦点作用域也仍存在。补齐测试包装的状态订阅转发后，原有关闭、
焦点作用域释放与摘要更新断言通过，无需修改生产焦点资源。真实 Gallery 路径另验证回车选中第 9 项、
移除对话框、释放作用域并恢复开启按钮焦点；`drawer_tests` 三项全部通过。其他 `GAP` 与 `Workspace` 缺少故事仍保留。

组件测试的旧 GAP 断言已清理（2026-11-02）：`navigation_drawer_tests::keyboard_and_selection`
验证 Space、Return 与小键盘 Enter 在按下时开启，释放不重复激活，并继续验证关闭、作用域释放与焦点恢复；
该目标 8 项通过。此前“Space 按下不打开”的失败属于测试预期过时，现已删除该限制记录。

本批还核查组件与 Gallery 的其余测试，更新过时预期并修正夹具的状态订阅、窗口宽度、语言和滚动事件会话。

### 演示段回填（2026-11-02）

页面数与 Swift 一致（98 / 98），但每页的 `StorySection` 演示段长期少于 Swift：Swift 195 段，回填前 115 段。

已按 Swift 结构补齐到同等段数的页面：

| 页面 | 段数 | 内容 |
| --- | --- | --- |
| Chart | 12 | 趋势、分组/堆叠柱、零线负值、双轴、对数轴、直方图、大数据量、散点、环形、迷你图、横向柱、数据表 |
| Rating | 6 | 整星、半星、十分制与替换符号、只读与禁用、密度与外观、双向 |
| Carousel | 5 | 分页、自动播放、多页、无循环无箭头、空集合 |
| Button | 5 | 变体、图标与提示、状态、长标题、尺寸 |
| TextField | 4 | 输入与清除、密码与只读、装饰与上限、Unicode 编辑 |
| StatusBar | 3 | 窗口页脚、单槽位、长分支名 |
| Link | 3 | 正文内、悬停下划线、从不下划线 |
| Slider | 3 | 连续值、步进、禁用 |
| Icon | 3 | 语义尺寸、语义着色、目录 |
| Toggle | 2 | 设置、禁用 |

演示段总数 115 → 136。仍少于 Swift 的页面：视口与适配组（10 → 3）、Menu（3 → 1）、Stepper（3 → 2）、
TimePicker / Timeline / Panel / Split / ZStack / Web / Table / Tag（2 → 1）、Avatar（5 → 4）。
这些页面已有真实控件与一段可交互的演示，缺的是同页的多变体、禁用态和边界示例。
菜单窗口尺寸修饰器覆盖 ScrollView 布局回调的问题已修复，`menu_hierarchy_tests` 12 项通过。
工作区完整复测结果以本批最终验证记录为准；仍有准确记录的行为差异不能计作已修复。

本批更正的三处过时断言：`inspection_tests` 的百分比内边距（实际按包含块计算，即 CSS 语义，断言仍按自身宽度）、
`advanced_interaction_tests` 的虚拟栈 `scrollToIndex`（目标行其实已经挂载）、`drawer_tests` 的方向键焦点推进
（实际已前进到下一个目的地）。

iOS 的 UIKit WebView 与 Android WebView 原生宿主在本次任务开始时已缺失；现有 macOS、Windows、Linux WebView 实现必须保留。所有平台实现与硬件验收应单独判断，不能把配置存在或交叉编译通过写成设备验收通过。
