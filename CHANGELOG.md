# Changelog

每个 Cove SDK 版本对应一个 `vX.Y.Z` 标签，统一包含 CLI、框架源码、模板与各目标平台的原生预编译库。

## Unreleased

- **生产源码迁移完成**：984 个 Swift 生产文件中 983 个已有 Rust/C 实现，唯一例外是
  `NSNumber+Boolean.swift`（Objective-C 的 `NSNumber` 布尔扩展，Rust 无对等物且不需要）。
  SwiftPM 清单 25/25 已建立 Cargo 清单，CLI 项目模板 5/5 已改写为 Cargo 工程。
- **补齐此前虚报为“已完成”的 69 个生产文件**。旧清单按文件名相似度映射，只要目标 `.rs`
  存在即记完成——`DataTable.swift` 落到无关的 `lists/table.rs`、`SplitView.swift` 落到
  25 行的 `view.rs`，因而得出“984/984”的假结论。清单生成器改为**符号校验**：索引全部
  Rust 声明（含 `handle!`/`payload!`/`identifier!` 宏调用与 `pub use ... as` 改名两端），
  逐文件核对 Swift 主类型名是否真实存在；对不上先按符号重定位，仍找不到则置未开始并清空
  错误路径。本轮据此重写：`DataTable`、`TreeTable`、`Tree`、`Chart`、`ChartDataTable`、
  `PropertyGrid` 全家、`SectionedList`、`SwipeList`、`ReorderableList`、`Combobox`、
  `SearchField`、`RangeSlider`、`ColorField`、`JsonField`、`TimeField`、`TimeInput`、
  `TimePicker`、`Vec3Field`、`AssetRefField`、`DateRangePicker/Calendar`、`SplitButton`、
  `MenuButton`、`CompletionPopover`、`SplitView`、`ResizableEditor`、`DisclosureGroup`、
  `NavigationRail`、`Tooltip`、`CommandPalette`、`ContextMenu`、`OverlayHost`、
  `ShortcutHost`、`Sheet`、`ToolbarMetrics`、`UIIcons`、默认主题预设、`WorkspaceView`、
  `ImmersiveWindowTitleBar`、`GalleryView`、DevTools 协议/诊断/传输类型，以及
  `NativeAppFrameCapture` 的真实 GPU 离屏渲染与回读。
- **修复布局引擎两处真实缺陷**（此前被测试以“已知差异”绕过，属于掩盖而非修复）：
  - `align_self`/`align_items` 交叉轴改为对**容器**求解。原实现取行内最高子项，导致
    100 高容器内 30 高子项居中落在 `y=0`（Yoga 为 `y=35`）。
  - **widget 级 `layout_direction` 从未生效**：`ViewGraph::compute_layout` 硬编码
    `Direction::Ltr`，`measure`/`arrange` 全程透传同一参数，从不读 `style.direction`。
    后果是所有 RTL 行排序无效，且 `Edge::End` 在无父节点时就把物理边烘死。现改为逐节点
    `effective_direction()` 解析，`Edge::End` 在布局期按生效方向解析。
- **修复清单生成器三处判定缺陷**：测试行被 `derive()` 提前标记导致后续映射遍历跳过；
  脚本自己清空的行被当成“操作者纠正”永久跳过；`CacheTests`→`caches`、
  `DrawListResourceTests`→`draw_list_resources` 这类复数目标名漏匹配。
- **CLI 模板改为 Rust 工程**：新增 `Cargo.toml.in`、`src/{main,app,resources}.rs` 与
  `tests/resources.rs`（可编译验证），`cove new` 不再拷贝 Swift 源码与 SwiftPM 清单。
- 以下缺陷一并修复：DevTools 诊断记录误用 `parentId`/`nodeId`/`missingIds`（Swift 与
  `DevTools/Client/*.js` 读的是 `parentID`/`nodeID`/`missingIDs`）；`capture_presentation`
  忽略模态作用域导致被遮控件仍报 `input_enabled: true`；`Tools::skills()` 返回空库；
  滑块 `shows_ticks` 从未绘制；`NumericTextFormat` 只认 ASCII 数字且把微小负数渲染成
  `-0`；`Tree` 搜索过滤不展开祖先；`NativeAppFrameCapture::reset()` 泄漏显存。

- **迁移清单此前虚报完成度，已改为符号校验并修正**。旧清单按文件名相似度映射 Rust 文件，只要目标 `.rs` 存在即记“已写入”，于是 `DataTable.swift` 被记到无关的 `lists/table.rs`、`SplitView.swift` 被记到 25 行的 `view.rs`，得出“984/984 全部完成”的假结论。`Tools/Cove/rust/migration_inventory.py` 改为索引全部 Rust 声明（含宏调用与 `pub use ... as` 改名两端），逐文件核对 Swift 主类型名是否真实存在；对不上时先按符号重定位，仍找不到则置 `pending` 并清空错误路径。按新口径，当年**实际有 69 个生产文件没有实现**。
- **补齐 35 个此前完全缺失的视图与类型**（本轮）：
  - Compose 数据：`DataTable`（含 model/selection/view，保留 20 万行虚拟化）、`TreeTable`、`Tree`、`Chart`、`ChartDataTable`、`PropertyGrid` 全家、`SectionedList`、`SwipeList`、`ReorderableList`。
  - Compose 控件：`Combobox`、`SearchField`、`RangeSlider`、`ColorField`、`JsonField`、`TimeField`、`TimeInput`、`TimePicker`、`Vec3Field`、`AssetRefField`、`DateRangePicker/Calendar`、`SplitButton`、`MenuButton`、`CompletionPopover`。
  - Compose 布局/浮层：`SplitView`（含分割条拖拽与键盘）、`ResizableEditor`、`DisclosureGroup`、`NavigationRail`、`Tooltip`（悬停/焦点触发、仲裁、Escape 关闭）、`CommandPalette`（排名过滤、翻页、回车执行）、`ContextMenu`、`OverlayHost`、`ShortcutHost`、`Sheet`、`GridInteractionHost`、`ToolbarMetrics`、`UIIcons`、`DefaultDarkTheme`/`DefaultLightTheme`。
  - 宿主与工作区：`WorkspaceView`（2651 行，含四个具名分割、标签拖拽重排、浮动层、紧凑呈现）、`ImmersiveWindowTitleBar`、`WindowDragRegion`、`WindowControlStrip/Button`、`GalleryView`。
  - DevTools：`DevToolsCodec`、`InspectionValidation`、`NativeAppFrameCapture`（真实 GPU 离屏渲染 + 回读）、`UIAccessibilityActions`、`DevServerConfigurationError::UnsupportedHost`。
  - Core：`WorkspacePanelDescriptor`、`WorkspacePanelRegistry`、`WorkspaceThemeEnvironment`。
- **修复布局引擎两处偏离 Yoga 的真实缺陷**（此前被测试以“已知差异”绕过，属掩盖而非修复）：
  - `align_self`/`align_items` 的交叉轴居中改为对**容器**交叉轴求解。原实现取行内最高子项，导致 100 高容器内 30 高子项居中落在 `y=0` 而非 `y=35`。
  - `direction: Rtl` 现在真正镜像主轴排布。原实现的 `arrange_flex` 只看 `flex_direction.is_reverse()`，`direction` 传入后未参与主轴放置，RTL 行仍从左往右；现与 Yoga 一致（首个子项落在右边缘）。
- **修复 DevTools 三处真实缺陷**：诊断记录曾输出 `parentId`/`nodeId`/`missingIds`，而 Swift 与 `DevTools/Client/*.js` 读的是 `parentID`/`nodeID`/`missingIDs`，JS 客户端一直静默取不到字段；`capture_presentation` 未考虑场景模态作用域，被模态遮住的控件仍报 `input_enabled: true`；`Tools::skills()` 返回空库，Swift 返回内置 ui-diagnostics 指南。
- **修复控件三处真实缺陷**：滑块 `shows_ticks` 从未绘制（`paint::ticks()` 完整但 `render()` 未调用）；`NumericTextFormat` 用 `char::to_digit` 只认 ASCII，阿拉伯-印度数字等 `Nd` 数字被拒（Swift `NumberFormatter` 接受），改为 77 个 `Nd` 区块表；微小负数渲染成 `-0` 而 Swift 为 `0`。
- **修复 `Tree` 搜索过滤不展开祖先**：命中位于折叠分支时不可达，改为与 Swift `filterAndAutoExpand` 一致（`is_expanded = explicitly_expanded || child_subtree_matches`），经实际运行验证行数/深度/路径键。
- **修复 `NativeAppFrameCapture::reset()` 泄漏显存**：原来只清 extent，纹理仍留在闭包里；现在 capture 与 source 共享目标槽，`reset` 真正释放。
- **补齐清单生成器的两处判定缺陷**：`derive()` 会把测试行提前置为 `written_unverified`，使后续测试映射遍直接跳过（新迁移的测试套件永远不显示）；脚本自己清空的 `pending` 行会被当成“操作者纠正”而永久跳过，无法再被符号恢复。

仍未开始：34 个生产文件（2,380 行，主要是 Gutter 家族、Graphics 原生后端补件、Swift 宏插件）、345 个测试文件、6 个 Cargo 清单、5 个 CLI 模板。

- **修复 `agent_transport` 回环测试的死锁**：`loopback_server_round_trips_one_line` 用 `read_to_end` 等待 EOF，但传输层是**持久连接的行协议**——`serve` 回完一行后继续读下一行、永不关闭连接，于是客户端等 EOF、服务端等输入，`cargo test --workspace` 永久挂起（用户报告的 `cove_sample.txt` 采样即为此）。测试改为先 `shutdown(Write)` 半关闭写端再读。

死锁解除后暴露出 5 个被它掩盖的真实缺陷：

- **SHA-1 轮常量错位**：压缩函数写成 `c = b; b = a.rotate_left(30)`，而标准是 `c = b.rotate_left(30); b = a`。摘要"看起来合理"但**任何公开测试向量都不过**，导致 WebSocket 握手的 `Sec-WebSocket-Accept` 永久错误（RFC 6455 示例令牌断言失败）。修正后 FIPS 与 RFC 向量均通过。
- **`E_FAIL` 常量错值**：写成 `-2_147_467_263`，实际是 `0x80004001`；`E_FAIL` 应为 `0x80004005`（`-2_147_467_259`）。所有 HRESULT 错误信息都在报告错误的码。
- **DX12 内联存储尺寸未随指针宽度调整**：`D3D12_SHADER_BYTECODE` 是两个机器字（32 位 8 字节 / 64 位 16 字节），但 `read_from`/`write_into` 硬编码 8 字节缓冲。64 位下 `transmute_copy` 的"目标大于源"是**未定义行为**，测试直接 abort。
- **JPEG 负系数位型溢出且公式错误**：`((1 << size) - 1) + value` 在 `size == 32` 时溢出，且负值应以 `value - 1` 的 `size` 位补码表示。修正为 `(value - 1) & mask`。
- **Vulkan 实例记录字段布局不符规范**：`VkAccelerationStructureInstanceKHR` 把 `instanceCustomIndex`、`mask` 写成独立 `u32`，而规范要求 `customIndex:24|mask:8` 打包进同一字、`offset:24|flags:8` 打包进下一字。原布局是 72 字节（含隐式填充），GPU 会读到错位的地址。

另修复 4 处被同一批测试掩盖的实现缺陷：

- **`FrameTap` 首帧恒被丢弃**：`last_capture_at` 初始化为 `0.0`，与全新时钟相等，`now - 0 < interval` 使每次会话的第一帧都不发送。改为 `-inf` 哨兵表示"从未捕获"。
- **JSON-RPC 解析错误码违规**：请求体不是 JSON 时返回 `-32600`（Invalid Request），规范要求 `-32700`（Parse error）。
- **`tick`/`record_input` 的 `RefCell` 重入 panic**：`if let Some(x) = self.state.borrow().field` 会让不可变借用存活到整个分支体，分支里调用需要 `borrow_mut` 的 `settle()` 即 panic（滚动动画 11 项测试全挂）；`record_input` 则在持有 `borrow_mut` 时调用内部 `borrow` 的 `position()`。均改为先取出标量/拷贝再借用。
- **`focus_next` 不显示焦点环**：键盘遍历沿用了上一次的可见性，Swift 的 `focus(_:visible:)` 默认 `true`，因此指针操作后改用键盘时会看不到焦点指示。

另修复布局引擎两处真实缺陷（见上一条），并修正 9 处测试自身的错误预期，均经交叉核对 Swift 实现或规范确认：`LogQuery::default()` 的 `Info` 下限、`Weak` 控制器被提前释放、命中测试忽略子节点 frame 偏移、滚动偏移符号、解析抗锯齿描边的半像素外扩、非图层节点被期望拥有缓存列表、`ancestors` 反转后取错端点、4×4 图集的 gutters 容量、`ChunkUploadAllocator` 的 64 KiB 下限与碎片保留语义。

- **开始统一测试与验证**（迁移完成后首次全量运行）。`cove-core` 从 10 项失败修至 **103/103 全通过**，`cove-system`（含进程契约 5 项）与 `cove-logging` 亦全部通过。修复的真实缺陷：

- **布局引擎 `gap` 从未生效**：`line_main` 计算时计入了间距，但逐子项推进的游标没有加上 `gap`，导致设置间距只是缩小内容而不移动任何元素，尾部元素压在起始边上。
- **`align_items: Stretch` 下容器宽度塌缩为 0**：交叉轴测量只取最宽子项，未认领被提议的交叉轴范围，使自动宽度的列布局宽度为 0。修正时限定为"仅当存在自动交叉轴尺寸的子项"，否则显式尺寸的行仍应贴合内容（该限制由 `grow_shares_free_space` 测试守住）。
- **`cove-system` 进程契约测试引用了从未创建的 fixture 二进制**：`contracts.rs` 的 5 项测试全部依赖 `cove_process_fixture`，但仓库中没有该程序，测试始终失败。新增 `System/rust/bin/cove_process_fixture.rs`（`output`/`interactive`/`environment`/`sleep`/`arguments` 五种模式），并修正测试对 `target/<profile>/deps` 与 `target/<profile>/` 的路径推断。
- **`LogRuntime` 的 `&self` 方法必然 panic**：`clone_handle` 从 thread-local 表里按指针找回自身 `Arc`，但构造函数从未登记，且工作线程与本线程的 thread-local 互不相通。改为在值内保存 `Weak<Self>` 自引用，任意线程可用；`register_runtime` 保留为幂等空函数以免破坏公开导出。
- **`LogQuery::default()` 隐含 `Info` 下限**：日志管道测试用默认查询读取 Trace/Debug 记录必然为空，改为显式 `minimum: Level::Trace`。

- 另修正 5 处测试自身的错误预期（均经交叉核对 Swift 实现确认为测试写错而非实现错误）：`Weak` 控制器在测试中被提前释放、命中测试忽略子节点自身 frame 偏移、滚动偏移符号、分析抗锯齿描边的半像素外扩、非图层节点被期望拥有缓存列表。

- **生产源码迁移全部完成：984/984（100%）**。本轮补齐最后两个文件：`DataStories`（19 项测试）与 `AdaptiveExamples`（13 项测试）。至此 17 个模块全部完成，`Docs/rust-migration-files.json` 中无任何 `pending` 条目，每条均有 Rust 目标。

- 新增 `DataStories`（19 项测试，已运行通过）：**树搜索在命中时保留其祖先**（"retains matching ancestors"——命中项与其路径可见，非命中的子节点被过滤，否则宽泛查询会倾倒整棵子树）、刷新阈值 64pt 且进行中不可重入、虚拟栈跳转到末行而非越界、滑行动作日志与未知行/未知动作的拒绝。

- 新增 `AdaptiveExamples`（13 项测试，已运行通过）：`supports(page)` 与 `sampleHeight` 两张手工表的**一致性**才是真正的不变量——测试断言二者互为补集、支持集无重复、且每个受支持页要么有确定高度要么明确按内容自适应。该测试发现 `ALL_PAGES` 漏掉了 `Vec3Field`（枚举存在但表里没有），已补入。

- 新增 `NavigationStories`（22 项测试，已运行通过）：一个 Swift 文件支撑十一个画廊页，全部接入 `StoryDispatch`。覆盖折叠面板的单/多展开模式（**关闭多选会回落到首项**，避免与刚选的模式矛盾）、禁用段不可切换、分页窗口的省略号与首尾页恒显、翻页在两端夹取、空数据集不暴露页码、标签独立移除与重置、对话框取消不提交、警示对话框仅在确认时执行、**底页重新打开回到初始停靠位**、拖拽越过阈值才提升/关闭、导航栈 push/pop 时根草稿保留。

- 修正一处自我豁免：我最初把这些故事中的六个标为"纯展示、无需移植"，但读源码后发现标签移除、对话框确认/取消、快捷键平台格式化等都有真实状态机，因此逐个补齐而非以注释绕过。

- 新增 `TextEditorStory`（21 项测试，已运行通过）：移植了故事私有的字节级 Swift 词法分析器——行注释止于换行前、字符串遵守反斜杠转义（含末尾反斜杠不越界）、大写标识符归为类型、数字与关键字着色、超 300 KB 不分析、`token_at` 二分查找；并移植了括号折叠扫描，**字符串与注释内的括号不参与折叠**、单行括号对不可折叠、嵌套块按起始行排序。**`KEYWORDS` 在 Swift 原文中 `self` 排在 `Self` 之前，并非字节序**，直接照抄会让 `binary_search` 失效；已重新排序并加测试断言该列表必须有序。生产源码达 981/984（99.7%）。

- 新增 `WebStory`（15 项测试，已运行通过）：状态行的优先级（失败带描述 > 加载中带目标标题 > 就绪/已加载兜底）、旧错误描述在已重新加载时被相位而非描述决定、后退/前进跟随宿主历史、重载常可用而停止仅在加载中、本地样例为内联 HTML 且与 Swift 页面一致。**Swift 侧此文件是 `#if os(macOS) && canImport(WebKit)` 并在他处显示"不支持"提示；Rust 侧无此门控**——`Sources/rust/webview` 已有 Apple/Windows/Linux/Android 四个宿主，因此各平台都渲染真实页面而非占位提示。

- 新增 `OverlayStories`（15 项测试，已运行通过）：一个 Swift 文件支撑六个画廊页（Menu/ContextMenu/Popover/Modal/Disclosure/Tabs），均已接入 `StoryDispatch`。覆盖菜单树深度（Settings → Appearance → Accent color 三层）、标签与分隔符不计为命令、100 行菜单中每第 10 行为分组子菜单、菜单 id 全局唯一、弹层与对话框的草稿保留/取消语义、披露组折叠后草稿仍在、关闭文档时**回落到相邻标签而非首个**、全部关闭后选择清空且可重置。首次断言把"顶层行数 > 叶子数"写反（子菜单是一行却含多个叶子），已修正为实际关系。生产源码达 980/984（99.6%）。

- 新增 `ProductStories`（20 项测试，已运行通过）：一个 Swift 文件内的五个独立故事——单选组（禁用项拒绝选择且被方向键跳过）、表单校验（空白名不满足 `required`、邮箱需同时含 `@` 与 `.`、提交失败同时报出两个字段）、本地 Combobox（不可用项不可选）、模拟机场目录（按代码与名称大小写不敏感匹配、空查询返回全部 12 条、`offline` 查询**故意失败**以便演练重试路径、无匹配返回空而非错误）、Toast 队列上限与计数。生产源码达 978/984（99.4%）。

- 新增 `SidebarStory`（12 项测试，已运行通过）：两个分组共 7 个目标、禁用目标仍列出但键盘跳过、详情面板在无选中时显示提示而非任意行、6 个目标的图标 slug 互不重复、图标/徽标开关、插槽行使用 36pt accessory 车道、收藏与抽屉开关。已接入 `StoryDispatch`（并移除了此前误放在同一页面的重复匹配分支）。

- 新增 `DataTableStory`（22 项测试，已运行通过）：200,000 行 × 33 列的确定性行生成（数量按 17 步进并在 250 处回绕、指标列由行 id 推导）、加载后清空选择并聚焦首行、每次跳转移递增请求号、名称必填与数量非负整数校验、数量按数值而非文本排序、树展开时仅在**所有祖先均展开**时显示（折叠隐藏整棵子树而非仅第一层）、千分位分组与状态行文案。生产源码达 977/984（99.3%）。

- 新增移动端 `SettingsScreen`（9 项测试，已运行通过）：三个分组（General 两个开关 + 外观折行；Accessibility 一个开关；无标题的 Sign out 破坏性行）、开关写回 `MobileModel`、外观选择仅接受 System/Light/Dark 三项且选择后关闭弹层、未提供的外观值被拒绝且不改动模型。

- 新增 `ImageStory`（12 项测试，已运行通过）：四种图片来源（bundled/remote/missing/empty）各自的 URL 解析与 URL 去空白、`empty` 不可重试且请求号不变、三种加载相位到占位面的映射——`Empty` 与 `Failure` 共用布局但文案不同（`No image` + 引导 vs `Image unavailable` + 错误描述）。已接入 `StoryDispatch`。生产源码达 974/984（99.0%）。

- 新增 `PlacementExample`（11 项测试，已运行通过）：12 种摆放与 `PopoverPlacement::ALL` 一一对应、紧凑镜像三向分支（leading 贴右、trailing 贴左、居中折半，且触发器超宽时夹到边缘不出现负坐标）、上/侧/下三档垂直位置、触发器宽度受视口上限约束。该组件是 Popconfirm 页的子组件而非独立页，故不接入 `StoryDispatch`。生产源码达 973/984（98.9%）。

- 新增 `CommandStory`（11 项测试，已运行通过）：8 条命令分 4 组、仅 `Deploy to Device` 禁用；文档承诺的 `GO`/`go f`/`appearance` 三种查询均按真实排序模型命中；禁用命令不执行也不入最近列表；最近命令新前旧后并封顶 3 条、重复运行不产生重复项。**测试发现我的实现把 `showsRecent` 传反了**（应为查询为空时显示最近项），据 Swift 源码 `showsRecentCommands && trimmedQuery.isEmpty` 修正。生产源码达 972/984（98.8%）。

- **补齐 `WheelPicker`/`WheelPickerOption` 整块缺失**：`Primitives/Controls/WheelPicker.swift` 此前在 Rust 侧完全没有对应类型，`StoryDispatch` 的阻塞登记里也标着 `WheelPickerOption` 待实现。现实现为 `Compose/rust/primitives/controls/fields/wheel_picker.rs`（12 项测试，已运行通过）：按值而非标签识别选项、禁用行可见但不可提交、步进双向跳过禁用行且两端不环绕、`Home`/`End` 跳到首个/末个**可用**行、可见窗口居中并在末端整体回滑。首次实现的回滑逻辑只退一步导致窗口从 7..11 而非 6..11 开始，由测试发现并改为整体重算。

- 新增 `WheelPickerStory`（11 项测试，已运行通过）：时/分/半天三档时间轮与 0…10 音量轮（10 禁用）、`Now`/`Reset` 的 24→12 小时折叠、分钟按 5 取整向下吸附、`09:05`/`12:00` 双位补零。生产源码达 971/984（98.7%）。

- 新增 `MarkdownStory`（9 项测试，已运行通过）：三份文档（完整发布说明 / 精简安装片段 / 可编辑草稿）均按声明结构解析；测试断言了 ` ```swift `、表格、`[x]`/`[ ]` 任务项、引用、粗体、删除线与中文段落确实出现在源文档中。草稿实际解析为 **3** 个块（标题、描述段落、列表）——我最初的预期写成 2，用一个临时探针打印真实块结构后修正了测试而非实现；探针文件已删除。生产源码达 970/984（98.6%）。

- 新增 `TimePickerStory`（10 项测试，已运行通过）：12 小时制下午夜与正午均显示 `12`、11:59 仍为 AM、13:45 为 `01:45 PM`；24 小时制零填充无标记；秒数开关的字段顺序；清空后显示 `No time`。已接入 `StoryDispatch`。生产源码达 969/984（98.5%）。

- 新增 `DrawerStory` 与 `RailStory`（`navigation_layout_stories`，12 项测试，已运行通过）：抽屉 20 个目标中跳过不可用项、`closesOnSelection` 决定选择后是否关闭、未选中时标题为 `none`；导航栏 72pt（有标签）/56pt（无标签）两档宽度、跳过禁用目标、`999` 等长徽标不被截断。已接入 `StoryDispatch`。生产源码达 968/984（98.4%）。

- 新增 `ToolbarStory`（6 项测试，已运行通过）：`Always` 命令在极窄宽度下仍在栏内、`Overflow` 命令**无论宽度都进菜单**（由测试确认的真实语义，非猜测）、自动优先级在空间不足时按低优先级先移出且平局保留源序、Run/Pause 标题随播放态翻转。已接入 `StoryDispatch`。生产源码达 966/984（98.2%）。

- 公开 `MenuItem::perform`：与 `ActionSheetAction::perform` 对应，使调用方可从快捷键或测试触发已声明命令。

- 新增 `ActionSheetStory`（10 项测试，已运行通过）：禁用命令仍渲染但其图标保留且被方向键跳过、`export` 由复选框门控、`delete` 标记为 destructive、首条可用命令跳过被禁用的首项、长列表按声明数量生成且 id 唯一。已接入 `StoryDispatch`。生产源码达 965/984（98.1%）。

- 公开 `ActionSheetAction::perform`：此前只有私有 `perform`，调用方无法从键盘快捷键或命令面板触发已声明的命令。

- 新增 `AvatarStory`（11 项测试，已运行通过）：图片源切换（`bundled`/`remote`/`missing`/`initials`，无图片源直接显示首字母）、`AvatarGroupGeometry` 的 limit/overflow/overlap 组合（6 人限 3 时显示 3 个头像 + 1 个溢出标记、限 0 时仅剩标记、`Hidden` 完全抑制标记、0.3 重叠使步进为 22.4pt）、Unicode 首字母用例含空名回落。已接入 `StoryDispatch`。生产源码达 964/984（98.0%）。

- **Gallery 故事页继续收口**：新增 `data_stories`（`ChartExplorationStory` 缩放/重置/换数据与笔刷说明、`AdaptiveTableSample` 自适应列可见性）、`navigation_stories`（`AppBarStory` 长/短标题、文本/图标主操作、溢出命令与结果记录）、`sidebar_stories`（`ResizableSidebarExample` 宽度重置、`NestedSidebarExample` 分组展开/收起保留选中项、叶子可选性）。共 25 项测试（已运行通过），并把 chart、appBar、sidebar 三页接入 `StoryDispatch`。生产源码达 963/984（97.9%）。

- 补齐 `AdaptiveSidebar` 缺失的构造入口：此前该类型有 `View` 实现但**没有 `new`**，因此任何调用方都无法构造；现提供 `new(title, selection, sections)` 与 `content`/`presentation`/`width`/`expanded`/`options` 构建器，并在构造时校验 section ID 唯一（与 `Sidebar` 一致）。

- **Examples（Gallery）开始收口**：新增 `media_stories`（`SkeletonStory` 占位切换、`RatingSample` 评分条）与 `overlay_stories`（`PopconfirmStory` 确认记录、`HoverCardStory` 跟随切换），共 7 项测试（已运行通过），并把 5 个页面接入 `StoryDispatch`（skeleton、rating、popconfirm、hoverCard）。生产源码达 958/984（97.4%），仅余 26 个 Gallery 故事页。

- **Compose 生产源码全部迁移完成（414/414）**，全工作区生产源码达 953/984（96.8%）。本轮补齐最后一批控件模型（共 59 项测试，均已运行通过）：`AssetRef` 类别匹配与选择器过滤（9 项，关键字分组按优先级取首个命中）、`DataTableRowHost`/`CellHostUpdate` 与编辑器失焦判定（11 项，选中优先于斑马纹、更新未变即跳过重绘、冻结泳道按 group 播报）、`CommandSearch` 排序与投影（22 项：标题前缀 > 词首 > 包含 > 关键字 > 子序列 > 上下文，禁用项靠后、平局保持源序、分组仅在多组时出现、箭头环绕与翻页夹取、`i64::MIN` 模运算不溢出、最近项去重与前插封顶）。

- 修复 `normalize` 的变音折叠缺失：此前仅剥离 NFD 组合符号，预组合的 Latin-1（如 `é` = U+00E9）不会折叠为 `cafe`，导致带重音的命令标题无法用小写无重音查询命中。补上 Latin-1 补集显式映射后，带重音标题与无重音查询可正确匹配。

- 新增 Compose 字段模型（共 48 项测试，均已运行通过）：`Vec3Codec`（13 项：`1, 2, 3`／`(1 2 3)`／`Vector3(1, 2, 3)` 均可解析，非有限值拒绝，写入按轴夹取且只回写变化轴）、`JsonFieldValidation`（11 项：空白提交为 `{}` 且判为可接受，格式化为排序键）、`parse_hex`/`hex_string`（9 项：含 `#` 与大小写均可，8 位末对为 alpha，往返受 8 位量化限制）、`ComboboxSearchSession`（15 项：短于最小长度不发请求、同查询不重复请求、**仅最新代次可发布**、取消回 Idle 清查询但保留已载结果、重复值去重保留首个）。

- 新增 `DataTableEditingState`（11 项测试，已运行通过）：草稿生命周期与 Tab 遍历。被拒绝的提交**保持草稿打开并记录错误**（不丢弃用户输入），因此切换到另一单元格时会先中止而非吞掉非法值；行/列消失时草稿直接丢弃；Tab 跳过不可编辑列，走到末尾即结束。行身份用 id 而非索引，因为显示顺序可能在编辑期间改变。

- 新增 `TimeInputState`（14 项测试，已运行通过）：12/24 小时制显示、分段步进（12 小时制内步进不跨半天，与 Swift 一致）、数字缓冲（高位数字立即完成并进段、低位等待第二位）、AM/PM 首字母仅在异于当前半天时翻转、失焦清空该段缓冲。**移植中两处测试预期先写错后按 Swift 语义修正**：`set` 对时/分各自独立取模因而分钟越过 59 只回绕不向小时进位；分钟缓冲在单数字完成后仍保留，故紧随的下一数字构成超范围值并被忽略。行为与 Swift 一致，是预期错误而非实现错误。

- 新增 `DataTableRowMetrics` 与 `DataTableSortRequest`（9 项测试，已运行通过）：固定行高走算术、可变行高走前缀和索引，视图窗口/滚动范围/定位共用同一真源；排序在**两个方向**都以源索引兜底相等比较，因此降序不会反转并列项——这是"稳定"与"仅确定性"的区别；过期投影按模型修订号与排序命令双重拒绝。

- 补充 Compose 控件模型（含单元测试，均已实际运行通过）：`ViewThatFits`（阈值/intrinsic 双模式）、`LayerRoot`+`PortalHost`+`LayerRoot` portal 层、`DataTableColumnOrder`（8 项测试）、`ReorderableList` 拖拽模型（13 项测试）、`SearchFieldState`（8 项测试）、`DateRangePreset`/`DateRangeRules`（12 项测试，覆盖闰年、跨年、反向区间归一化、含端点的天数计算与 `can_complete` 双向校验）。`DateRangePreset::range` 的反向区间归一化在首次实现中写错（先重赋值 `lower` 再比较），由测试发现并修正。

- 生产源码迁移达 935/984（95%），**15 个模块全部完成**：Graphics、Core、Tools、DevTools、Hosts、Terminal、Sources、System、Logging、Text、Shell、Runtime、Imaging、PlatformCore、Browser。剩余为 Compose 18 个与 Examples 31 个；后者的 31 个 Gallery 故事页依赖前者尚未落地的控件。

- 补齐本轮 Compose 缺口：`ViewThatFits`（阈值与 intrinsic 两种选择模式，非活动分支保持挂载但不占位/不绘制/不接收输入，并在隐藏时释放焦点与指针捕获）、`LayerRoot`（portal 层 z-index 30000，高于标题栏上限 20000）、`PortalHost`（按首帧解析并保留窗口 store 归属，不在每次更新时重新解析，避免跨窗口迁移观察者）、`DataTableColumnOrder`（顺序应用/移动/拖拽目标计算）、`ReorderableList` 模型（拖拽会话、放置边界与去移除后的目标索引、键盘步进夹取、保留挂载行）。

- 修复 `PlatformEventBridge` 克隆语义：此前 `Clone` 会复制订阅表，导致克隆体收不到经原件发布的事件；改为共享 `Arc` 存储，与 Swift 的引用类型语义一致。

- 迁移清单工具修正可达性判定：`mod` 声明必须位于该文件的**同级模块**（`agent/skills.rs` 由 `agent/mod.rs` 的 `mod skills;` 引入），且 crate 根包含 `main.rs`（`Tools/Cove` 是二进制 crate）。此前判定过宽（把 `date_values.rs` 误认为可达）与过窄（把 `Tools` 30 条误降级）并存。另引入 `verified_by: "operator"` 标记人工按符号核对的映射，脚本不再覆盖。

- 修复 `cove_ui_compose` 未导出 `modifiers` 模块导致的可见性缺口：`FrameDimension` 等修饰器类型此前无法从 crate 外部引用（`pub use modifiers::layout::FrameDimension;` 已补，纯增量）。同时纠正一处流传的误判——**`ModifiedContent` 实现了 `View`，所有 `*Ext` trait 均为 blanket 实现，修饰器可自由串联**，无需在每组之间插入 `AnyView::new(...)`；已用编译探针验证并写入迁移文档。"no method named X" 的真实成因是缺 trait import、参数不符/字段私有、或值本身无 `View` 实现，三者已分列记录。

- 补齐 Gallery 侧视图缺失的控件：`Toolbar` 溢出布局接通（`LayoutContainer` + `impl Layout` + `AppBar` 供给可用宽度，并有断言优先级顺序与 `Always` 保证的测试）、`AppBar`、`ButtonGroup`、`Table`/`TableColumn`、`Calendar`/`DatePicker`、`Rating`、`Breadcrumb`、`Pagination`。`PaginationModel::items` 的 `start == 3` / `end == total-2` 邻页特例已按原实现移植并单测。

- 修复 `foundation/date.rs::weekday_of` 的错位一天缺陷：原式 `(epoch_day.rem_euclid(7) + 5).rem_euclid(7) + 1` 对纪元第 0 天返回 6，而正确值为 5（1970-01-01 为周四，Sunday=1 记法）。所有星期被报成下一天。改为 `((epoch_day + 4).rem_euclid(7) + 1)`。影响面：`Calendar::date_interval_start` 的 `leading = (weekday - first_weekday).rem_euclid(7)` 因此对每个日期都算错，导致 `CalendarView` 的周粒度选区错位、`CalendarUnit::Weekday` 取值偏移一天、`iso_week_of` 在年界返回错误周数。已用 Python `datetime.isocalendar()` 交叉验证修复后结果：2024-12-30 → 2025-W01、2021-01-01 → 2020-W53、2025/2026-01-01 → W01 全部一致。

- 新增日期/日历基础层：`foundation/date.rs`（`Date` 以纪元秒表示，`DateParts` 墙钟时间，Howard Hinnant 民历换算双向，`iso_week_of` 返回 ISO 周年+周数，圣诞周归属按"含该周周四的年份"处理）与 `foundation/calendar.rs`（固定 UTC 偏移 + `first_weekday`，`component`/`date`/`start_of_day`/`date_by_adding`/`date_interval_start`，月相加按目标月末夹取，周起始用 `rem_euclid`）。工作区无 `time`/`chrono`/`jiff`，故自行实现而非引入依赖。**具名差异：固定偏移、无 DST 规则**——对日历网格是正确取舍（日期是标签不是时长），但不与 Foundation 逐字节等价。

- 补齐此前被误判完成的控件：`DateValues`（`DateAvailability`/`CalendarGrid`，其 Rust 文件从未被 `mod` 声明且所引用的 `foundation` 日期模块不存在）、`Calendar`（period/navigation 模型 + `CalendarView`）、`DatePicker`、`Table`/`TableColumn`、`Toolbar`、`ButtonGroup`、`Rating` 等一批"模型与样式齐备但无视图"的组件。`CalendarView`/`DatePicker` 的**标签为英文硬编码**，未实现 `DateFormatter` 的本地化符号查找——英文下与 Swift 一致，其他语言不正确，已登记为已知缺口。

- `Checkbox::labeled_from_boolean(label, Binding<bool>)` 新增：`from_boolean` 内部会转成三态、没有 label 槽位，而三态路径有 `labeled`，造成布尔路径无法带标签。`Toggle` 本就有 `labeled(_:is_on:)`，此项补齐唯一的不对称。另修复 `masonry` 模块缺少 `pub use`（`primitives::layout::masonry::Masonry` 与 `primitives::layout::Masonry` 现均可用），与其余布局模块保持一致。

- 迁移清单工具增加三项可信度校验并修正两起错配：`.rs` 文件的 stem 若未被任何 `mod` 声明（死代码）不计入已写入；头部注释的基名匹配要求非标识符边界（避免 `Visibility.swift` 命中 `AnimatedVisibility.swift`）；普通英文词（`Visibility`/`Support`/`Options` 等）不作跨子系统匹配依据。据此确认 `Primitives/Data/Columns/Visibility.swift`（表格列可见性规划，25 行）未迁移——此前被错映射到 `Graphics/rust/rhi/visibility.rs`（GPU 可见性缓冲）；`Primitives/Controls/DateValues.swift` 亦为孤儿文件且引用了不存在的 `foundation` 日期模块。审计后当前 0 条生产记录指向不存在或不可达的目标。

- `TextBuffer` 恢复 `Send + Sync`（对齐 Swift 的 `Sendable`）：rope 由 `Rc` 改为 `Arc`，共享空叶子从 `thread_local!` 提升为 `OnceLock` 静态量，`TextBuffer::eq` 改用 `Arc::ptr_eq`。此前 `TextField::new(placeholder, Binding<TextBuffer>)` 的签名不可满足——`Binding` 要求闭包 `Send + Sync`，而 `TextBuffer` 基于 `Rc`——**全仓库零调用点**证明该控件从未可构造。修复后 `TextField` 可用，未使用任何 `unsafe impl`，`Send`/`Sync` 由数据结构本身满足（已用编译期断言验证后删除该断言）。`SharedRope` 保持 `Rc<RefCell<_>>`：实时编辑会话本就是 UI 线程状态，只有缓冲区**值**需要可共享。

- 修复 LSP snippet 展开的多处结构缺陷（`Compose/rust/primitives/text/text_field/snippet.rs`）：`${N|a,b|}` 现扫描整个候选列表并在首个逗号处记住首选，随后跳过 `|}`，转义按对消费；`$N` 在检查 `{` **之前**读取，修正纯 `$1`/`$2` 制表位的顺序（此即最常见路径）；`${VARIABLE}` 无默认值时不再可能输出多余 `$`；`$` 后非数字/`{`/标识符时按字面 `$` 输出。此前版本在单候选与多候选下都会错位跳过字符。

- 修复跨越多个团队的同一类缺陷（`.modifier()` 陷阱）于源头：`ModifiedContent` 的私有字段 `modifier` 遮蔽了 `ViewExt::modifier(self, m)`，报错形如 "private field, not a method"，本会话内至少 12 次误修。字段改名为 `applied`；新增 `AnyView::with_modifier(self, m)` 与 `IdentifiedView::with_modifier(self, m)`，使已擦除的值可直接链式附加修饰器。约定：具体 `View` 用 `ViewExt::modifier`，已擦除值用 `with_modifier`，禁止用 `AnyView::new(modified_content)` 掩盖类型错误（会经 downcast 快路径静默丢弃修饰器）。

- 删除 `Compose/tests/` 目录下 4 个陈旧测试文件（`theme.rs`、`theme_environment.rs`、`composition_local.rs`、`semantic_motion.rs`）。它们由 Cargo 默认自动发现路径编译，但写于早期接口（`Rect.origin` 作为字段、`ViewGraph(tree:recomposer:)` 元组构造），从未通过编译，且使 `--all-targets` 每次必然失败，污染全部验证。

- 新增 `pulldown-cmark` 工作区依赖用于 Markdown 解析。Swift 侧使用 swift-markdown（cmark-gfm），手写 CommonMark 子集会在嵌套列表、惰性续行、引用式链接、HTML 块与实体处理上与 cmark-gfm 分歧。`pulldown-cmark` 关闭默认特性后仅引入 `bitflags`/`memchr`/`pulldown-cmark-escape`/`unicase` 四个纯 Rust 小依赖，无构建脚本，全平台可用（含 Wasm）。

- 开始迁移 Swift 测试为 Rust 行为测试（424 个测试源文件）。`Tools/Cove/rust/migration_inventory.py` 现在按 `[[test]]` 目标名匹配测试条目，不再把它们误记为未开始。已迁移：System 6 个、Logging 流水线、Core 场景动画 6 个、`Node` 变换/资源/不透明度/无障碍几何、`Animation` 值类型、`PlatformCore` 滚动输入、`Text` 彩色字形图集，以及 Compose 侧的 `TestSupport` 等价物（`GlobalTestLock` 串行化进程级 holder、`TestScene` 私有 holder、`test_text_environment`、按 `LayoutDebugAttachmentKey::DEBUG_NAME` 读回调试名）。

- `--all-targets` 全工作区曾达到 18 个 crate 全绿（含测试目标）。此后 Compose 的 68 个未开始文件仍在持续写入，属预期状态。

- 迁移盘点改为脚本生成，并与磁盘对齐：新增 `Tools/Cove/rust/migration_inventory.py`，遍历 Rust 树按同名模块与主类型定义重新推导每个 Swift 文件的目标路径与状态，同时重写 `Docs/rust-migration-files.json` 与 `Docs/rust-migration-files.md`。生产源码实际为 651 → 802 / 984 已写入，上一版"460 已写入 / 477 未开始"是人工维护造成的陈旧值。

- 修复"写而未编译"缺陷（与编译错误同级）：System 16 个、Logging 15 个、Tools/Cove 8 个、Tools/Support 12 个、Compose 2 个 Rust 文件此前从未被 `mod` 声明；`Core/rust/tests/` 6 个与 `Graphics/rust/tests/` 7 个测试文件从未声明为 Cargo 测试目标，`cargo` 完全看不到它们。System 与 Logging 现按 Swift 包结构重挂模块，并删除重复的单体实现（Logging 511 行 `lib.rs`、System 合并版 process），关键对外 API 保持可用（`LogRuntime::log` 五参形式、`LogQuery.text: Option<String>`、`MetadataProvider` 改为 newtype、`Access::Inherit` 单一类型）。

- 新增 `Logging/rust/build.rs` 编译 `Sources/CCoveLog/Router.c`，`LogSystem::bootstrap` 不再因 `cove_log_set_handler` 未定义而在链接期失败。

- Graphics 三个原生后端全部完成：Metal（macOS/iOS）、Vulkan（Windows/Linux/Android，`vulkan/sys.rs` 为手写 FFI，运行期 `dlopen`/`LoadLibraryA` 加载，无新依赖）、DX12（Windows，COM vtable 直调，无 `windows` 依赖）。`backend_factory::install_backends` 按 `cfg` 注册。DX12 加速结构未链接 DXR 分发，能力探测如实不宣称光追；`command_list` 的描述符表逐 draw 绑定仍待 Windows 真机验证。

- Terminal 完成：POSIX `forkpty`（macOS/Linux）、Windows ConPTY、Windows 命令行引号；链接 vendored Ghostty 静态库，FFI 布局由真实 `ghostty_type_json()` 清单生成并加编译期尺寸断言。会话线程改用 `Arc` 邮箱 + `LocalInvalidationHandle` 桥接替代 Swift 的 `NSLock` + `DispatchQueue.main`。

- Hosts 完成：Android 15 + Desktop 26 个文件，49 个 Rust 文件；`install_platform_services()` 在桌面 `Application::run` 与 Android `cove_android_create_application` 两处启动路径调用，使远端图片加载真正生效。逐平台矩阵与 7 项已知缺口记录在 `Hosts/rust/PLATFORM_COVERAGE.md`。

- Sources（WebView/Workspace）与 DevTools 完成：WKWebView（objc2 + `define_class!` 导航代理）、WebView2（手写 COM vtable）、WebKitGTK（dlopen GTK3/GTK4）三个真实后端；DevTools 客户端/服务端/Agent/Inspector 全部落地，`std::net` 实现 RFC 6455 传输，`pump()` 每帧驱动。

- Browser 宿主落地：`Browser/rust/prototype/` 按 Swift `main.swift` 的 `@_cdecl` ABI 复刻全部导出（`cove_render`/`cove_font_*`/`cove_pointer`/`cove_key`/`cove_text`/`cove_units`/`cove_snapshot`/`cove_dispatch` 等），帧载荷与 DevTools 会话流程一致；单线程 reactor 用 `thread_local!` 承载 Swift 的 `nonisolated(unsafe)` 全局。

- Tools CLI 与 Support 完成：`main.rs` 此前完全没有 `mod` 声明，导致 8 个模块全部是死代码，且 5 个被引用的模块不存在。现已接入 23 个命令分发、`--version`/`--help`、SDK 重执行引导，并以真实二进制核对退出码与 stderr 文本；Android/Desktop/iOS 平台构建命令为真实实现。

- 修复一批共享 API 缺口：`Rect::size()`/`Rect::origin()`、`KeyModifiers::is_empty()`、`ScrollConsumePolicy: Default`（`whenScrollable`，与 `ScrollView` 默认一致）、`ObservableStateRegistrar: Clone`、`scene::scrolling` 重导出、Compose 根导出 `State`/`LayoutNode`/`ScrollView` 系列。`ModifiedContent` 的固有方法 `modifier()` 曾遮蔽 `ViewExt::modifier(_:)`，改名为 `modifier_ref()`。

- 已知缺口：Linux `https://` 图片加载（无 TLS crate，如实上报 `https=false`）；macOS 菜单栏（`cove-shell` 只有上下文菜单入口）；Wasm 图片 transport（需专用 fetch/XHR）；Apple 图片 MIME 与 macOS 内存压力读取采用回退实现。

- 全量迁移尚未完成：Compose 仍有 82 个文件未开始，Examples 46 个、Tests 424 个待迁移。所有 Rust 代码仍未做行为对照、跨平台验收与 GUI 冒烟；按要求统一验证放在全部转换之后。

- 翻译 Metal 后端第一批：`MetalSubmissionStatus`、`MetalLibraryCache`、`MetalRenderBindings`、`MetalUploadRing` 落地到 `Graphics/rust/metal/`，并在 `Graphics/rust/lib.rs` 中以 Apple 目标 cfg 声明模块。设备、注册表、加速结构、管线、传输与提交编码器尚未翻译，未编译、未测试。

- 修复 Rust 工作区可编译性：为 `Compose/CompositionMacros` 补上 proc-macro 的 `path`，修正 `cove-composition` 的导出与类型错误（`ModifierReplay` 导出、场景几何重导出、`DynamicField: Clone`、slot tag 包装、`&dyn View`/`&AnyView` 混用、`ViewScope::uninstall` 签名），并为 `Graphics/rust/ui/draw_geometry.rs` 的语句位 `unsafe` 块加括号。仅为局部 `cargo check`，不构成验收。

- 迁移盘点改为逐文件映射：新增 `Docs/rust-migration-files.md`（1,438 行）并升级 `Docs/rust-migration-files.json` 为 schema 2。补回 System、Logging、Tools/Support、Graphics RHI 前端、Hosts Android 契约/集成、DevTools Protocol 的既有 Rust 映射，纠正 Android `APK/Package.swift` 的清单误分类，并确认 ZStack 与 ChartOverlay 已实现。盘点未编译、未测试。

- 继续逐文件迁移 Compose：加入触发式/重复关键帧轨道、离散阶段序列、数字文本插值、可动画缩放/偏移变换和进度值动画资源，保留阶段动画、最终无障碍值、减弱动态效果和节点卸载清理。本批尚未编译或测试。

- 继续迁移 Compose 基础控件：加入 Portal 宽高约束与放置算法、模态焦点作用域、活动路由出口、虚拟列表窗口状态、数据表列几何、集合控件键盘边界和编辑器高度约束。本批尚未编译或测试。

- 继续迁移 Compose 布局：加入 `LayoutThatFits` 候选选择和响应式动作组的水平/堆叠排列，复用同一组子视图与状态。本批尚未编译或测试。

- 继续迁移 Compose 菜单基础：加入不依赖具体渲染器的菜单/子菜单/动作描述符和快捷键关联。本批尚未编译或测试。

- 继续迁移 Compose 动画：实现 `Transition` 插入/移除效果、带退出保留和快速反转的 `AnimatedVisibility`，并接入 `TransitionView` 与 Portal；支持主题动画、减少动态效果、分组延迟和布局折叠。本批未编译或测试。

- 继续迁移 Compose：新增通用的动画值修饰器、线程安全布局帧捕获、主题驱动浮层表面修饰器和资源包定位；ViewGraph 布局回写会逐次触发节点布局回调。改写列可见性优先级规划，加入列选择菜单模型；接入表单、字段标签、表单验证、键帽控件和键盘快捷键值，并从 Compose API 导出动画上下文函数。列选择器和 Select 的下拉/菜单呈现仍待迁移。本批代码尚未编译或测试。

- 继续迁移 Compose：补齐颜色/渐变/透明度/阴影/边框/字体/字距与 Padding/Flex/Frame/绝对定位修饰器，以及无障碍、光标、焦点请求、命中测试和悬停修饰器；新增 StatusTone；接入按钮交互/无障碍主机、节点级样式刷新、图标缩放和生命周期加载指示器；将 Link 和 Tag 接入 Rust 控件模块。Button/Link 提示呈现、Link 交互动画和 Text 仍有缺项，尚未编译或测试。

- 继续迁移 Compose：增加统一动画时钟的 Rust `PetWorld`、窗口坐标精灵绘制、布局锚点注册与 `PetScene` 组合；接入媒体、头像、图表、文本辅助、布局和主题模块，并写入纯文字和分隔线的基础绘制/测量。文字选择与截断提示仍未完成，移植尚未验证，待全量迁移后统一测试。

- 继续迁移 Compose：补齐 Rust `ViewGraph` 的局部状态字段读取跟踪，并将 `@TrackStateReads` 编译器宏改为 Rust 属性宏；迁移与宏展开均留待全量完成后的统一验证。

- 继续迁移 Graphics UI 渲染层与 Runtime：写入纹理驻留/图集、背景模糊/揭幕、分条 MSAA、多窗口事件与帧调度、iOS 显示循环及 Apple 无障碍桥接；这些实现仍未编译或测试，原生 GPU 后端与上层控件迁移继续进行。

- 继续迁移 Shell：写入 Rust SDL3 窗口/输入/剪贴板/对话框，C 事件桥接，以及 macOS/iOS 原生窗口、精确滚动和显示刷新桥接；尚未统一验证。

- 继续逐文件迁移 Text/Imaging：加入 Rust 字体发现、FreeType/HarfBuzz 整形与图集、CoreText 彩色字形、图片/动画解码、EXIF 方向和资源缓存；新增带源码指纹的迁移清单。按要求留待全量转换后统一测试。

- 开始 Swift → Rust 2024 迁移，新增 `0.3.0-dev` Cargo 工作区；`cove` / `cove.cmd` 改用 Rust CLI，不再调用 Swift 编译器。
- 加入 Core 几何/状态/绘制缓冲区/弹簧/位图/字号缩放、PlatformCore 输入与资源契约、System 文件与进程服务、Logging 管线和 DevTools 协议的 Rust 实现；绘制热路径继续使用 C，mipmap 面积过滤增加共享 C 实现。
- Rust DevTools 协议改为 0.2，输入录制格式为 3；CLI 增加 Cargo 构建/运行/检查/格式化/测试/清理与迁移盘点。新增无窗口 RustCounter 示例及桌面/移动共享基础层 CI 配置。
- 全量迁移尚未完成：Swift UI、原生 GPU/窗口/移动宿主、终端、WebView、Workspace、Gallery 与 SDK 发布等实现保留并列入 `Docs/rust-migration.md`；尚未统一测试，不宣称功能或平台验收完成。

## 0.2.13 — 2026-10-29

- 增加 `RouteOutlet` 活动路由组合边界；Gallery 将页面状态读取移入内容区，Sidebar 按输入代次和行状态复用，切页只更新受影响的目标行。
- `VirtualStack` 的固定、可变和估算行高统一支持 `preload` 视口比例，默认额外准备约 30% 内容；缓冲窗口内滚动不触发组合，补充窗口只创建新进入的行，保留原有 `overscan` 最小行数与跳转/缩放/数据更新行为。
- 增加显式 `MemoizedView` 输入复用边界，局部状态和环境变化仍可刷新；不改变普通视图对不透明回调的保守更新规则。

- 修复文本拖选导致整窗卡顿：按场景几何版本缓存段落顺序与坐标，使用跨平台 C 内核排序和命中扫描，绘制直接查找选区；拖选只刷新选区发生变化的段落，复制不再逐段重复排序。
- 渲染树改用缓存的扁平绘制计划与子树范围，缓存命中、裁剪可直接跳过整段，纯结构节点只传递变换；补齐动态 zIndex 与结构变化的失效处理。
- 顶点平移、索引重定位、透明度与绘制边界扫描采用 C 内核，大缓冲区按不重叠区间并行处理；滚动复用不再重复裁剪已验证安全的圆角几何。共享实现覆盖 macOS、iOS、Linux、Windows 与 Android。
- 节点脏标记、资源卸载、渲染镜像构建/拆除及帧末清理改为迭代，避免深层树递归栈溢出。

## 0.2.12 — 2026-10-29

- 修复 Linux/Windows 原生菜单测试：PNG 图标按 CoreGraphics/ImageIO 编码能力验证，未提供编码器的平台仍验证图标为空；统一实现与测试的平台条件。

## 0.2.11 — 2026-10-29

- 修复 0.2.10 发布验证：Compose 按所属包查找资源，线程局部存储析构回调处理空指针并消除 Windows 的 UUID 名称冲突，Android 的 Float 数学运算使用明确重载，iOS 不再引用 AppKit 菜单适配器。
- 修复后续 Linux CI：文本选择几何测试仅依赖 Foundation，移除已删除工具包的测试与缓存入口。
- 修复 Windows 的 HarfBuzz 枚举桥接，保留公开 `ScriptTag` 的 UInt32 语义，兼容原生枚举底层的有符号整数。

## 0.2.10 — 2026-10-29

- 按能力拆分 Core、Text、Imaging、Graphics、Shell、Compose、Runtime、DevTools 与 Hosts，移除 Portable 与根 manifest 对下层 target 的重复声明；根包收拢为应用与扩展产品，内部模块产品改为从所属能力入口消费。
- Compose 去除 Runtime/窗口/GPU 依赖；字体、文本环境和缓存统一归 Text，图片解码与资源所有权归 Imaging，GPU 渲染和上传归 Graphics。保留 `import CoveUI` 与 `TextureImage(texture:)` 应用入口；CLI、Browser 使用窄协议/组合/检查依赖。修复字号切换后 HarfBuzz 缓存未同步和映射字体逐字节复制的问题。新增架构边界检查；Core 的 Yoga 布局依赖仍明确保留，尚未达到全量纯 Swift。

- 文本选择补齐跨组件一致性与触摸细节：`RichText`/`Markdown` 与 `Text` 共用同一套端点手柄、放大镜与命中测试（`TextSelectionChrome`），手指或笔可拖动手柄精修选区；`RichTextFlow.caretRect(at:)` 提供手柄定位。
- 选择区内的 `Text` 不再进入 Tab 焦点序列，点击按钮、列表行或表格单元格中的标签时不再抢走控件焦点与点击；`Text`/`RichText` 在控件内部自动退出选择。
- 键盘可扩展选区：`Shift+←/→` 按字素步进，`Shift+Home/End` 扩到段首段尾，可跨段落延伸。
- 触摸长按选词后焦点落到被选中的标签或段落，复制快捷键可直接作用于该选区。
- 选择菜单的 Look Up / Share 仅在宿主提供动作时出现，新增 `textSelectionActions(lookup:share:)`；`TextSelectionScope` 的 `hasSelection`、`selectedText`、`copy()`、`selectAll()`、`clear()` 与 `TextSelectionRoot` 初始化器公开，供宿主接入。
- 文本选择改为默认开启：`Runtime` 与辅助窗口安装根视图时统一包一层 `textSelection()`，普通 `Text` 无需在页面里逐个声明即可拖选与复制；子树上用 `.textSelection(false)` 重新关掉。
- 选择区只接管落在自身文字上的按压（`TextSelectionScope.textUnder`，`_ContextMenuHost.isInterested`），整页或整窗的选择区不再遮挡控件自己的右键菜单与触摸长按。
- 触摸屏的选择菜单以底部表单呈现（`TouchPresentation.usesSheet`），复制、全选在手指长按后即可用，不再依赖硬件键盘。
- 选择区对布局完全透明：只挂一个不占布局节点的 `SurfaceReader`，菜单宿主仍是布局根；底部表单作为宿主的兄弟节点而非包裹层。此前用 `Box` 包裹会让 `SurfaceRoot` 的 `flex(1, basis: 0)` 失去确定高度，整个窗口高度塌陷为 0（Gallery 的 `gallery-shell` 由 820 变 0，所有分区叠在一起）。
- 修复 `Animation.initialVelocity` 的访问级别：`CoveUICompose` 的 `staggered` 需要读取它，否则干净构建失败。
- `HorizontalSwipe` 的分页手势改用 `.scroll` 角色（原为默认 `.control`）。它包住整页时，页内每个标签都会被判为「控件内的文字」而退出选择，导致整屏文本一个都选不中；分页与滚动一样属于手势容器，不该占用控件角色。
- 选择能力按整个 Gallery 目录验收，而非抽样单页：`GalleryContentTests` 逐页挂载真实应用根，断言每个叶子标签要么可按下、要么被控件合法拥有（`Badge`/`Tag`/`KeyCap` 这类合并语义的容器不计入），并逐页跑真实的「按下→拖动→松开→Cmd+C」序列校验剪贴板。任一页出现「整页不可选」都会直接指名吞掉手势的祖先节点。

## 0.2.9 — 2026-10-27

## 0.2.8 — 2026-10-27

- 增加 `Device.prepareResources` 和四类资源的异步创建 API，提供阶段/步骤/编译进度、Task 取消和失败清理；Metal、Vulkan、DX12 在昂贵编译期间释放设备资源锁，已预热的 UI 可继续渲染。
- 着色器按内容共享并计数引用，取消或失败只释放本次持有的资源；并发创建同一管线时回收重复对象，保护编译期间的着色器输入生命周期。
- 内置 UI 打包 macOS、iOS 与模拟器的预编译 Metal 库，按源指纹失效回退；新增跨平台着色器产物生成流程。GPU 管线仍需在目标设备上创建，本版本未提供跨进程驱动缓存。
- 包含未成功发布的 0.2.7 的全部内存与效果修复：跨后端 16 MiB MSAA 工作区、磨砂前缀重绘、同通道揭幕、Vulkan 临时附件与空块释放、DX12 分配报告和离线 UI DXIL。
- 修复 DX12 多采样颜色纹理的无效资源标记，以及发布说明中的旧 CI 跳过标记阻断后续版本工作流的问题；支持手动恢复不可变标签的发布。

## 0.2.7 — 2026-10-27

- 统一窗口渲染的 MSAA 生命周期：磨砂重绘已解析的前缀，揭幕在最终通道合成，跨通道只保留单采样图像；取消效果帧保留整屏多采样附件的路径。
- 为需要显存的 Metal、Vulkan 和 DX12 后端提供 16 MiB MSAA 工作区预算，通过保持原始视口与样本位置的分条渲染限制大窗口分配；支持真正 memoryless 的硬件继续使用片上附件。
- Vulkan 临时附件使用 TRANSIENT_ATTACHMENT 并优先选择 lazy 内存；空内存块在资源退役后释放。DX12 增加纹理分配报告。
- 修复安装版 Windows 测试诊断脚本查找 CLI 的路径。
- 补齐 UI 与磨砂/揭幕的 DXIL 离线产物及固定 Slang 工具链生成入口；修正着色器编译与缓存生命周期文档。
- 磨砂捕获按实际尺寸复用纹理，修复较小的后续面板沿用较大纹理时的采样范围错误。

## 0.2.6 — 2026-10-27

- 保持默认 4x MSAA，普通窗口在支持的 Apple GPU 上改用临时附件；磨砂与揭幕自动保留持久样本。固定场景的输出逐字节一致，M1 的 4K 附件分配由 129.5 MiB 降至 0 MiB 持久 backing，2x 场景 GPU 耗时下降约 32%；DevTools 分开显示逻辑容量和后端 MSAA 分配，其他后端保留安全回退。

- iOS 构建由 destination 选择应用 SDK，避免全局 `-sdk` 把宿主宏插件误编译为模拟器程序；补齐字段追踪对延迟绘制、布局回调和未读取状态失效信号的保守处理。

## 0.2.5 — 2026-10-27

- 新增 `@TrackStateReads` 视图宏，自动推导普通值类型 `@State` 的字段读取依赖；按实际读取字段刷新，重建时替换依赖，整值读取、引用与不透明值保守失效，并在 DevTools 中记录字段名称。
- Metal 接入可选的命令缓冲 GPU 时间戳，完成 1x 解析覆盖率与 4x MSAA 的固定场景评估：M1 上 1x 降低 UI GPU 耗时，但 4x 显著改善自定义三角形边缘，因此桌面默认继续保留 4x；报告包含像素误差、目标内存与截图，测量入口只在显式启用时执行。

## 0.2.4 — 2026-10-27

- 修复 Windows 安装器读取 GitHub 校验清单的方式：下载后明确按 UTF-8 文本逐行解析，兼容二进制响应类型，并避免覆盖 PowerShell 自动变量。
- 包含 Cove 0.2 系列的完整改名、子树复用、抗锯齿、mip／LOD、跨平台内存压力和 DevTools 修复。

## 0.2.3 — 2026-10-27

- 完整发布 Cove 0.2 系列 SDK：包含改名、子树复用、覆盖率抗锯齿、mip／LOD、内存压力与 DevTools 接线修复。
- 修复解析抗锯齿在 Android 上的浮点数学重载与类型推导，采用 Swift 原生开方和取整，并在整数转换前限制细分数量。

## 0.2.2 — 2026-10-27

- 完整发布 Cove 改名后的 SDK，包含 0.2 系列的子树复用、抗锯齿、mip／LOD、内存压力和 DevTools 修复。
- Android 发布明确选择已安装的 NDK r30，避免 runner 继承的旧 NDK 环境变量使原生库打包失败。
- 发行脚本等待真实的 GitHub workflow ID，不再把空列表显示为 `null`。

## 0.2.1 — 2026-10-27

- 首次发布完整 Cove SDK：同步模块、CLI、清单、安装目录与发布包名称，包含值状态子树复用、覆盖率抗锯齿、mip／LOD 和跨平台内存压力修复。
- 修复 Linux／Android 原生构建中 SVG、WebP 静态依赖的链接顺序，CLI 自动配置所需选项。
- 浏览器 DevTools 与内置 Agent 共用内存数据源，Android 提供文本／图片缓存统计；断开连接后清除旧快照。
- 深层 UI 交互回归在应用主 Actor 上执行，避免 macOS CI 后台工作线程栈溢出；改名后的 CLI 使用物理路径，防止旧路径链接产生重复编译缓存。

## 0.2.0 — 2026-10-27

- 项目迁移为 Cove：框架模块采用 `Cove*`，CLI 为 `cove`，清单与本地目录采用 `cove.json`／`.cove`；安装器、发布包、Android 标识与仓库链接同步改名。此版本需要更新旧项目的导入与配置。

- `@State` 父视图更新时自动跳过值输入相等的子 scope，保留原状态和回调；闭包、可变引用及不透明输入保守重组，环境变化仍强制刷新。
- 补齐描边、路径、圆环与圆角遮罩的覆盖率抗锯齿；`geometryAntialiasing = .analytic` 可按窗口使用 1x 目标，默认仍保留 4x MSAA。离屏 HUD 与 Android 使用解析遮罩。
- 图像生成带 alpha 权重的 mip 金字塔，计入图片缓存预算，并按实际显示像素选择 LOD；GPU 图像可携带由生产者生成的独立分辨率层。
- 增加 Windows 低内存通知、Linux 主机／cgroup 压力监听和 Android `onTrimMemory` 清理；DevTools 提供 `query_ui_memory` 与性能页内存估算。
- 显式本地 SDK 覆盖可跨版本调试；SDK 包补齐 Inspector 客户端及资源文档。Android 发布不再请求已下架的 `tools` 包。

## 0.1.14 — 2026-10-26

- 修复发布引导：源码仓库的旧原生库清单不再阻止新版本从源码构建；已安装 SDK 缺少库时明确报错，避免回退到不存在的依赖源码。

- 窗口隐藏、最小化或被遮挡时停止呈现，等待在途工作完成后释放该窗口的 swapchain、MSAA 与 backdrop 目标；恢复时按最新尺寸和 VSync 设置重建，辅助窗口互不影响。
- 重组性能统计不再触发重画；接入 Swift Observation，按实际读取属性的视图失效，父视图不会因子视图的读取而订阅同一属性。
- 文本 shaping/layout 缓存增加 LRU 淘汰、条目数与估算字节双限；提供应用内存配置、窗口资源快照与主动清理，接入 macOS 内存压力及 iOS 内存警告。
- 主窗口与辅助窗口可分别覆盖采样数和清屏色；未覆盖时继承应用配置，默认画质保持不变。
- 多窗口分别持有 reveal/history 与 blur 临时状态，共享字体、图片驻留和已预热管线；隐藏窗口同时释放特效缓冲，避免窗口之间的呈现状态相互覆盖。
- 离屏视图在内容、尺寸和像素密度不变时复用几何快照；图集补丁只发送一次，主动清理后可重新生成。

## 0.1.13 — 2026-10-26

- SDK CLI 增加 `guava task` 和 `guava.workspace.json`：项目提供 Swift 任务库，SDK 负责依赖排序、按需构建、参数传递及构建前钩子，不需要项目包装脚本或独立 CLI。
- 工作区可声明 `dependencyUpdates: "branches"`，本地 build/run 自动检查并刷新分支依赖；CI 保持锁文件，离线检查保留已解析版本。

## 0.1.12 — 2026-10-26

- 安装包包含完整 SDK 与主机原生库，按版本安装到全局 `versions/VERSION`；项目、示例和 IDE 引用同一份源码与库，不再回退到各项目单独检出 GuavaUI。
- CLI 自动调用项目对应 SDK 的工具；安装 SDK 不再改写项目版本，`sdk use` 负责选择版本。
- Guava 构建、测试与清理使用 SwiftPM 标准 `.build`，移除 `.guava/build/swift` 这套独立编译缓存。
- 将原生构建与 SDK 发布收拢为一个工作流、一个 Release；原生库按目标拆包并由 SDK 自动准备，覆盖桌面、Android ARM64、iOS 真机和 ARM64 模拟器。
- Android 构建自动安装匹配且校验过的 Swift Android SDK；平台工具由 setup/doctor 检查。
- 并发项目通过系统文件锁复用同一份 SDK 安装与原生库准备；修复旧安装记录路径末尾斜杠导致升级被误判为其他安装的问题。

## 0.1.11 — 2026-10-26

- `guava.json` 的 `toolchain.sdkVersion` 支持写 `latest`：构建时浮动到本地已安装的最高版本 SDK（离线可用，不触发网络），用 `guava sdk install latest` 拉取新发布并刷新；`guava sdk install latest` 在精确钉版项目里仍会把解析后的具体版本写回清单（npm 语义），无 `GUAVA_SDK_ROOT` 的 SwiftPM 回退路径改用 `from: 0.0.0` 解析最高发布标签，Android Gradle 回退同样支持 latest。
- `guava sdk install latest` 解析 GitHub Latest 指针时去掉仓库地址末尾的 `.git`，此前拼接出 `.git/releases/latest` 会 404。
- `guava self update` 不再调用 `api.github.com`（未认证每小时 60 次配额，超限即失败），改为跟随 `github.com/repos/latest` 的重定向解析最新版本；下载类错误现在打印完整的 HTTP 状态而不是通用的 `ToolError` 提示。
- `guava setup --ios` / `--android` 显式检查 CMake 与 Ninja（移动端 C 依赖从源码交叉编译）；移除 iOS 提示中并不存在的 Rust 工具链要求。
- `Tools/Release/release.sh` 推送后只确认 release workflow 已触发并输出 run 链接，不再轮询等待 CI 结果；移除 `--no-ci`、`--ci-reruns`、`--ci-timeout` 选项。

## 0.1.10 — 2026-10-26

## 0.1.9 — 2026-10-26

- DevTools Inspector 自动打开时先打开会话目录内的 `inspector.html` 跳转页：`open`、`xdg-open` 与 `rundll32` 会丢弃 `file://` URL 的查询参数，客户端此前只能回退到默认 9229 端口并停留在 Disconnected；跳转页在浏览器内完成导航，动态端口与一次性 token 完整保留，打开即自动连接。`guava devtools` 附加到现存会话时同样生成该跳转页。

## 0.1.8 — 2026-10-26

- Windows 子进程以反斜杠路径启动，参数只在需要时加引号：`cmd.exe` 会扫描整条命令行，曾把 `C:/…/cmd.exe` 里的 `/cmd.exe` 当作 `/c` 开关，现在能执行命令并返回真实退出码。
- 文件与管道错误改用 `NSPOSIXErrorDomain`：Android 的 Foundation 没有 `POSIXErrorCode`，`System` 包恢复在 Android 上编译。

### 触摸与移动端

- `NavigationStack` 在触摸表面支持从前缘拖动返回：顶层页面跟随手指移开，下层页面以视差露出；松手时超过宽度三分之一或快速轻扫则返回，否则回弹。从右到左布局从右缘开始。
- `Dialog` 在手机上改为底部弹层：带拖动柄、贴合内容高度、避开 Home 指示条与键盘，去掉关闭按钮和分隔线；`AlertDialog` 仍居中显示。
- `HoverCard` 在无法悬停的触摸表面改为点按标签打开底部弹层，内容不再只能靠悬停到达。
- 标签栏的“更多标签”与列可见性菜单在触摸表面改用底部弹层，与 `Popover`、上下文菜单共用 `AdaptiveOverlayHost`。
- `SectionedList` 在触摸表面点按行即激活（与 `List` 一致），分组标题与行内容对齐并贴近所属行。
- 修复减弱动态效果时 `NavigationStack` 推入页面后无法再返回：没有过渡动画时不再等待过渡结束。
- `Examples/Mobile` 的根视图放入 `LayerRoot`，底部弹层、对话框与菜单在移动端可以正常呈现；项目详情的“分享”改为打开对话框。

## 0.1.7 — 2026-10-25

- 新增可独立构建的 `System` 包，提供 `GuavaFileSystem` 与 `GuavaProcess`，不依赖 SDL、窗口或渲染器；桌面与移动端共享文件、路径、权限语义，不支持的进程能力明确报错。
- 文件写入统一使用同目录暂存与替换，失败时保留旧内容；Windows 仅对临时共享锁进行有限重试，私密文件使用当前用户 ACL。
- 进程服务统一流式读取、输出限额、超时与取消。超时覆盖子进程退出后的管道排空，Windows 使用 Job Object 回收后代，读取句柄由读取线程关闭。
- CLI、应用存储、DevTools 与技能目录使用共享系统能力，移除重复的 Windows 文件写入与进程读取实现。
- 下载文件先复制到目标目录暂存，再校验和发布，避免 CI 临时目录与工作区跨卷时替换失败；下载失败保留原文件。
- `guava test` 支持仅构建、跳过构建、分开运行 XCTest/Swift Testing、测试事件文件与总超时；CI 增加系统契约测试、构建缓存和 Windows 超时进程转储。
- 上下文菜单保留编辑器的弹性高度，触摸滑动列表与长按菜单协作处理指针捕获，允许垂直滚动接管手势。

## 0.1.6 — 2026-10-25

## 0.1.5 — 2026-10-25

### 触摸与移动端

- 菜单在触摸表面改用独立呈现：`MenuPresentationHost` 决定菜单怎样呈现，`TouchMenu` 负责触摸下的导航与项激活，`TouchOverlay` 承担模态呈现；`Popover` 在触摸输入下改走 `TouchOverlay`，不再套用指针式弹出层。
- 新增 `LongPress` 手势与 `MobileBar` 标签栏。标签栏去掉顶部的分隔线，由自身表面标出边缘，背景延伸到 Home 指示条下方。
- 列表行区分指针与触摸：触摸行铺满容器宽度、最小高度 44pt；`ListRowStyle` 随之调整内边距与选中样式。
- 按钮按触摸输入调整尺寸与内边距，触摸表面上的拖动柄按 `minimumControlExtent` 取宽度。

### Android 与表面度量

- `UIEnvironment` 新增 `idiom`、`textScale` 与 `keyboardHeight`，与安全区分开；新增 `TextScale` 读取系统辅助功能中的文字缩放。
- `AndroidApplication` 新增 `updateScale` 与 `surfaceMetrics`，`SurfaceState` 统一持有逻辑尺寸、密度、惯用语、安全区、键盘高度与系统栏图标样式；`AndroidContentSession` 改用 `SurfaceState`。
- Android 的 `windowSoftInputMode` 改为 `adjustResize`，键盘弹出时内容按 `keyboardHeight` 收缩，模板与示例同步更新。

### 控件与表面

- 边框改用半透明墨色（`border`、`borderStrong`、`divider`），不再使用不透明灰；分隔线改为 0.5pt 发丝线，用来确认已看到的边界，而不是画出实线。浅色与深色主题都按此调整。
- `.secondary` 按钮、`StatusButtonStyle` 与开关改用 `onSurface` 的半透明墨色填充，不再叠加表面色与描边；开关滑块在开关两态下都是白色圆片，在浮起表面上不再与卡片同色而消失。
- 新增 `ListGroup`：可带标题与脚注，行位于同一圆角表面上，分隔线从行内容处起始，触摸表面使用更大的圆角。
- 侧栏可折叠：`AdaptiveSidebar` 新增 `isCollapsed` 绑定与 `collapseLabel` / `expandLabel`。宽度足够时仍可折成 72pt 轨道，被窗口强制成轨道时按钮改为展开抽屉，两种情形不再混淆。
- 侧栏拖动柄静止时不绘制，只在悬停或聚焦时出现，边界交给表面差异表达。
- 按钮的 `tooltip` 只重复可见标题时不再弹出，文本仍作为辅助技术的帮助；`AccessibilityTree.visibleText` 提供控件实际显示的文本。
- 示例库的页面改为窗口表面上的单块软面板（`surfaceSunken` 加圆角），去掉头尾与工具栏之间的分隔线。

### 工具与示例

- 新增 `GUAVA_PREVIEW_DEVICE=phone`：桌面构建以 390×844 窗口、触摸输入与刘海屏安全区运行，无需模拟器即可检查和调整移动端布局。
- 新增 `Tools/Release/release.sh`：升版本、提交、打标签、推送并等待发布工作流；网络步骤带退避重试，失败的工作流先重跑失败任务。
- 新增 `Examples/Mobile` 示例，共享业务代码位于 `Sources/AppContent`，覆盖 iOS、Android 与 macOS 目标。

## 0.1.4 — 2026-10-25

- 图形管线改用 `PipelineStore` 缓存：图形、计算与网格管线按描述复用，`Device` 通过它创建与销毁管线，并统计创建耗时与缓存命中。`NativeDrawListRenderer` 在配置变化后保留管线状态。
- 修复 Windows 上的原子安装写入：替换文件时按类型选择替换标志，`guava self update` 不再留下半写的可执行文件。
- CLI 子进程的标准错误改用专用读取器排空至 EOF，安装器与终端不再因管道未读完而卡住。
- 跨平台安装器修复后重新发布 SDK 与 CLI。

## 0.1.3 — 2026-10-24

- 图表支持堆叠与缩放，新增 `ChartOrientation`；视口可拖动与刷选，数据变化使用可中断的过渡动画。
- 修复 Android 的数学库与 SDL 链接，修正 Vulkan 动态描述符布局。
- 保留层滚动时，画家局部的裁剪保持与内容对齐。
- 恢复 DevTools 的独立依赖，并校验开发者客户端可独立启动。

## 0.1.2 — 2026-10-24

- 图表新增缩放与堆叠，补齐对应测试。
- 修复 CLI 在 Windows 上查找工具链的问题。

## 0.1.1 — 2026-10-24

- 修复 Windows 上读取 `PATH` 时区分大小写的问题，CLI 能正确找到已安装的工具。

## 0.1.0 — 2026-10-24

- 首个发布：预编译 `guava` CLI 与运行库，提供 macOS arm64、Windows x86_64 与 Linux x86_64 三个平台。
- `install.sh` 与 `install.ps1` 安装器写入用户 `PATH`；CLI 自带运行库，`guava --version` 不需要先装 Swift。
- 提供 `guava create`、`run`、`build`、`test`、`clean`、`devtools`、`sdk install` 与 `self update`。
