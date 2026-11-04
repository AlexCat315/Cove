# Swift 行为断言核对

更新：2026-11-04。以本任务开始时的工作区为基准，匹配到 107 行独立的 GAP 冒号文本：40 行断言/诊断字符串、66 行说明，另 1 行是合法的 `PopoverGeometry::GAP` 常量。常量不属于缺失行为标记。说明与断言按同一行为合并核对，旧前缀均已移除。

本表只记录本任务中的原始标记及其关联回归；其他迁移审计不据此宣告完成。生产实现使用 Rust 2024；测试结构与原生视觉、键盘、IME 验收分别记录。

## Compose

| Rust 测试文件 | Swift 对照测试 | 结果 |
| --- | --- | --- |
| [date_range_rules_tests.rs](../crates/compose/tests/date_range_rules_tests.rs) | [DateRangeRulesTests.swift](../swift/Tests/CoveUIComposeTests/DateRangeRulesTests.swift) | 范围会话保留 pending start；同步日格启用状态，拒绝超过最大天数的第二次选择。 |
| [route_and_preload_tests.rs](../crates/compose/tests/route_and_preload_tests.rs) | [RouteAndPreloadTests.swift](../swift/Tests/CoveUIComposeTests/RouteAndPreloadTests.swift) | 绑定读取在 outlet 所属作用域登记，选择变化更新目的地且不重建应用壳。 |
| [async_image_tests.rs](../crates/compose/tests/async_image_tests.rs) | [AsyncImageTests.swift](../swift/Tests/CoveUIComposeTests/AsyncImageTests.swift) | 占位内容填满声明的图片框。 |
| [button_scroll_view_tests.rs](../crates/compose/tests/button_scroll_view_tests.rs) | [ButtonScrollViewTests.swift](../swift/Tests/CoveUIComposeTests/ButtonScrollViewTests.swift) | hover 进入与离开通过显式动画上下文渐变。 |
| [theme_cross_fade_component_tests.rs](../crates/compose/tests/theme_cross_fade_component_tests.rs) | [ThemeCrossFadeComponentTests.swift](../swift/Tests/CoveUIComposeTests/ThemeCrossFadeComponentTests.swift) | TabView 通过语义背景属性渐变，t=0 保留旧颜色，完成后到达新颜色。 |
| [navigation_drawer_tests.rs](../crates/compose/tests/navigation_drawer_tests.rs) | [NavigationDrawerTests.swift](../swift/Tests/CoveUIComposeTests/Adaptive/NavigationDrawerTests.swift) | 保留已有 leading 动画实现，将首帧断言翻转为从屏幕外进入。 |
| [narrow_reflow_tests.rs](../crates/compose/tests/narrow_reflow_tests.rs) | [NarrowReflowTests.swift](../swift/Tests/CoveUIComposeTests/Adaptive/NarrowReflowTests.swift) | Vec3 按缩放后的最小轴宽布局；窄屏依次堆叠 X/Y/Z，宽屏同排；按 StaticText 标签检查。 |
| [scrollbar_tests.rs](../crates/compose/tests/scrollbar_tests.rs) | [ScrollbarTests.swift](../swift/Tests/CoveUIComposeTests/ScrollbarTests.swift) | hover 渐入；滚动立即 flash，延迟 0.7 秒后用 0.3 秒渐出。 |
| [scroll_offset_binding_tests.rs](../crates/compose/tests/scroll_offset_binding_tests.rs) | [ScrollOffsetBindingTests.swift](../swift/Tests/CoveUIComposeTests/ScrollOffsetBindingTests.swift) | 新增公共 scroll_anchor 修饰器并用它验证前插时的视口锚定。 |
| [toolbar_overflow_tests.rs](../crates/compose/tests/toolbar_overflow_tests.rs) | [ToolbarOverflowTests.swift](../swift/Tests/CoveUIComposeTests/Adaptive/ToolbarOverflowTests.swift) | 保留已有按空间分配的生产修复，验证 Share project 在 390pt 进入溢出菜单。 |
| [sidebar_resize_tests.rs](../crates/compose/tests/sidebar_resize_tests.rs) | [SidebarResizeTests.swift](../swift/Tests/CoveUIComposeTests/Adaptive/SidebarResizeTests.swift) | 使用实际应用壳验证焦点从消失的 resize lane 交给当前目的地或紧凑入口。 |
| [vertical_toolbar_tests.rs](../crates/compose/tests/vertical_toolbar_tests.rs) | [VerticalToolbarTests.swift](../swift/Tests/CoveUIComposeTests/VerticalToolbarTests.swift) | 保留已有纵向容量与绑定实现，修正未指定尺寸的溢出按钮为非零目标，实际点击打开菜单。 |
| [view_that_fits_tests.rs](../crates/compose/tests/view_that_fits_tests.rs) | [ViewThatFitsTests.swift](../swift/Tests/CoveUIComposeTests/ViewThatFitsTests.swift) | Swift 源码也在候选首次激活时才挂载；保留符合源码的惰性行为并清理错误差异说明。 |
| [observable_state_tracking_tests.rs](../crates/compose/tests/observable_state_tracking_tests.rs) | [ObservableStateTrackingTests.swift](../swift/Tests/CoveUIComposeTests/ObservableStateTrackingTests.swift) | 观察作用域覆盖 reader 闭包及其 materialization；每次重组重新收集依赖。 |
| [markdown_tests.rs](../crates/compose/tests/markdown_tests.rs) | [MarkdownTests.swift](../swift/Tests/CoveUIComposeTests/MarkdownTests.swift) | 旧说明已过时；保留已有 selection/clipboard 预期断言，清理前缀说明。 |
| [gutter_tests.rs](../crates/compose/tests/gutter_tests.rs) | [GutterTests.swift](../swift/Tests/CoveUIComposeTests/CodeEditing/GutterTests.swift) | 实际输入框挂载 TextGutterControls，控件与正文共用 gutter 几何；真实点击断点按钮、从输入框派发 F9/折叠快捷键；文本重基同步折叠区间与标记，仅遍历视口内行。 |
| [adaptive_timeline_tests.rs](../crates/compose/tests/adaptive_timeline_tests.rs) | [TimelineTests.swift](../swift/Tests/CoveUIComposeTests/Adaptive/TimelineTests.swift) | 事件按 id 保留行身份；恢复主题间距并去除图标重复缩放。无 metadata 行与标题仅相差主题间距，使用 Swift 的精确几何断言；组折叠使用绑定或保留的局部状态，zoom 根据最新 factor 更新。 |
| [sidebar_hierarchy_tests.rs](../crates/compose/tests/sidebar_hierarchy_tests.rs) | [SidebarHierarchyTests.swift](../swift/Tests/CoveUIComposeTests/Adaptive/SidebarHierarchyTests.swift) | 跨紧凑展示保留导航节点；移除焦点行时按原祖先路径恢复；移动选择时展开新路径。 |
| [product_interaction_regression_tests.rs](../crates/compose/tests/product_interaction_regression_tests.rs) | [ProductInteractionRegressionTests.swift](../swift/Tests/CoveUIComposeTests/ProductInteractionRegressionTests.swift) | 命令结果过滤禁用项；HoverCard 注册焦点 accessory、真实卡片 hover 与 Escape 路由。 |
| [menu_content_tests.rs](../crates/compose/tests/menu_content_tests.rs) | [MenuContentTests.swift](../swift/Tests/CoveUIComposeTests/MenuContentTests.swift) | styled title 的 run 链接被移除，整行为激活目标。 |
| [advanced_interaction_tests.rs](../crates/compose/tests/advanced_interaction_tests.rs) | [AdvancedInteractionTests.swift](../swift/Tests/CoveUIComposeTests/AdvancedInteractionTests.swift) | 生产代码及断言已验证 Primary-Z 本地 undo 与 paste 前选区恢复，清理过期说明。 |

生产对照包括 `ViewScope.swift` / `ViewGraph.swift`、`Primitives/Controls/Fields/DateRange*`、`Primitives/Layout/ScrollView`、`Primitives/Layout/Sidebar`、`Primitives/Layout/Tabs`、`Primitives/Overlays`、`Primitives/Text/Code` 与 `Primitives/Data/Timeline.swift`。Reader 闭包与 Binding 读取统一登记到正在观察的 Rust 作用域。

关联的 `timeline_group_tests.rs` 同步翻转旧的“折叠后仍显示”和“缩放读取旧值”断言，验证绑定更新及未绑定的局部折叠状态。

## Workspace

下列原始 19 条说明均对应 [WorkspaceViewTests.swift](../swift/Tests/CoveUIWorkspaceTests/WorkspaceViewTests.swift) 和 [WorkspaceView.swift](../swift/Sources/CoveUIWorkspace/WorkspaceView.swift)。Rust 回归位于 [workspace_view_tests.rs](../crates/addons/tests/workspace_view_tests.rs)。

| 原始位置对应的测试 | 预期与实现 |
| --- | --- |
| `collapsed_rail_activates_the_clicked_panel` | 为 rail 恢复按钮提供稳定名称；通过真实点击选择并恢复目标面板。 |
| `workspace_theme_controls_tab_bar_height` | 继承调用方主题，52pt tab bar 实际应用到布局。 |
| `top_slot_uses_preferred_content_height_and_bottom_chrome_remains_fixed` | 使用面板首选高度并保留底部 chrome。 |
| `workspace_panel_content_is_clipped_to_its_region` | 面板视口实际裁剪。 |
| `collapsing_left_right_then_bottom_restores_in_place` | 逐次点击各 rail 恢复面板，状态栏保持原位。 |
| `panel_regions_scroll_only_content_that_overflows` | 实际内容滚动节点具有稳定检查名称。 |
| `dragging_tab_shows_drop_indicator_before_release` | 命名实际 drop overlay，验证拖动期间存在指示器。 |
| `tab_strip_exposes_tab_list_panel_and_position_and_mirrors_arrows_in_rtl（2 条）` | 一项 tab stop；原生布局方向进入键盘路由，RTL 镜像箭头。 |
| `tab_arrow_command_mapping` | 提供 workspace_tab_command，验证 Home/End 和方向映射。 |
| `panel_sizing_applies_to_outer_pointer_keyboard_and_accessibility_and_restores（3 条）` | 指针、键盘、辅助功能共用首选/最小/最大尺寸与真实可用范围；测量不改写文档。 |
| `cramped_workspace_shares_minimums_without_horizontal_overflow` | 断点取宿主 SurfaceReader 尺寸，狭窄内容按共享约束分配。 |
| `top_region_uses_the_shared_height_contract_and_keyboard` | 上侧区域同样使用首选高度及键盘约束。 |
| `two_hundred_percent_tabs_keep_controls_reachable_and_reveal_keyboard_selection` | 字号缩放高度，真实 End 导航触发滚动 reveal。 |
| `closing_last_panel_animates_the_vacated_region_and_keeps_the_document_closed` | 文档立即关闭；离场内容保留绘制，尺寸逐帧回收。 |
| `animated_collapse_retains_paint_disables_input_and_reclaims_space` | 真实 scheduler 驱动折叠，离场节点不接收输入。 |
| `animated_collapse_reversal_keeps_the_same_panel_and_restores_layout` | 离场中反转保留面板身份并恢复布局。 |

## 其他组件与夹具说明

| 原始文件 | Swift 对照与处理 |
| --- | --- |
| `examples/gallery/tests/adaptive_shell_tests.rs` | 对照 `swift/Examples/Tests/GalleryContentTests/AdaptiveShellTests.swift`；Gallery size 和 sidebar-item 检查标识实际存在。 |
| `examples/gallery/tests/app_bar_tests.rs` | 对照 `AppBarTests.swift` 及 `Stories/Navigation/NavigationStories.swift`；共享 story 包装器转发动态属性，点击后检查实际结果文本和标题。 |
| `examples/gallery/tests/drawer_tests.rs` | 对照 `DrawerTests.swift` 及 `Stories/Layout/SidebarStory.swift`；实际入口、共享 inline sidebar、320pt 高度填充的可点击抽屉；点击目的地关闭并恢复入口焦点。 |
| `crates/text/tests/fallback_style_tests.rs` | 对照 `swift/Text/Sources/CoveUIText/FontProvider.swift` 及 `FallbackStyleTests.swift`；Apple 家族描述符继承主字体实际 CoreText traits；其他平台匹配样式文件，并读取主字体 OpenType 字重、斜体及当前可变字体轴；regular/bold CJK 与命名 Bold 主字体回归均验证样式继承。 |
| `crates/terminal/tests/view_tests.rs` | 对照 `swift/Terminal/Tests/CoveUITerminalTests/ViewTests.swift` 与 TerminalSession；select/clear_selection 立即使缓存帧失效，验证选中单元格。其余 work queue/clipboard/font/图安装差异说明清理前缀。 |
| `crates/graphics/tests/metal_binding_lifetime_tests.rs` | 对照 `swift/Graphics/Tests/NativeRHITests/Metal/MetalBindingLifetimeTests.swift`；生产 encoder 生命周期已符合 Swift，测试改为直接提交已失效的 backend binding，检查精确错误和未调用 completion。 |
| `crates/graphics/tests/resource_preparation_tests.rs` | 对照 `swift/Graphics/Sources/NativeRHI/Device/{Device,ResourcePreparation}.swift`；所有资源类型自动 acquired/released，取消或失败按逆序退还本任务 lease，并走共享帧退休队列；检查早期 buffer 已被销毁。 |
| `crates/graphics/tests/{native_gpu_contract,native_multisample,shader_specialization}_tests.rs` | Slang fixture 路径是运行测试的依赖条件；保留条件和错误检查，移除无关前缀。 |
| `crates/runtime/tests/native_accessibility_tests.rs` | AppKit 桥接可达性与测试调用方式差异，不是钉住错误输出的断言；清理前缀，保留辅助功能行为检查。 |
| `crates/shell/tests/display_refresh_rate_tests.rs` | Swift helper 在 Rust 测试内联；清理无关前缀。 |
| `crates/hosts/tests/session_tests.rs` | Android holder 的可观察身份检查保留，清理命名差异前缀。 |
| `crates/terminal/tests/{session,windows_session}_tests.rs` | 同步队列和 MainActor 的夹具差异；保留现有会话断言，清理前缀。 |
| `crates/compose/src/theme/styles/components/default_divider_style.rs` | 语义样式 wrapper 的实现差异不要求递归调用 legacy Divider；清理过期说明。 |

## 关联布局回归

完整工作区回归还检查 `GridColumnsTests.swift::scrolling` 与 `MarkdownTests.swift::codeScrolls`：普通自定义布局的软尺寸提议保留固定列及长代码行的固有宽度；`ActionGroupLayout` 显式测量约束后的按钮高度，使堆叠测量与放置一致。网格、Markdown、Alert 和横向滚动场景分别定向验证。

## 验证

完整工作区测试、网站构建与前缀扫描的最终结果见 [gallery-audit.md](gallery-audit.md#2026-11-03-swift-行为断言清理)。

最终结果：工作区命令退出码 0，3534 项通过、0 失败、5 项原有忽略。网站构建、格式检查和前缀扫描通过。详细原生验收状态见 Gallery 表。
