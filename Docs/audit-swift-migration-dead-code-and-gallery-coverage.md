# 审计报告：Unused 警告 / Swift 迁移遗留 / Gallery 覆盖缺口

- 日期：2026-11-02
- 范围：`crates/compose`、`crates/addons`、`examples/gallery`
- 方法：`cargo check --workspace --all-targets` 全量警告提取 + 逐项源码验证
- 状态：审计结论保留为记录；§5.3 占位页、§3.1 缺陷 5、§6 孤立状态与 §2 警告已按 §8 顺序修复，已在对应段落就地标注。**AdaptiveControlExamples 不再移植**（见 §5.3 末条）。

---

## 目录

1. [总体结论](#1-总体结论)
2. [编译警告全量清单](#2-编译警告全量清单)
3. [未使用字段/方法逐项定性](#3-未使用字段方法逐项定性)
   - 3.1 确认的真实缺陷
   - 3.2 无害遗留（重复移植）
4. [被 `#[expect(dead_code)]` 压制的未接线功能](#4-被-expectdead_code-压制的未接线功能)
5. [Gallery 组件覆盖缺口](#5-gallery-组件覆盖缺口)
6. [Gallery 已展示内容的质量问题](#6-gallery-已展示内容的质量问题)
7. [当前工作区构建状态](#7-当前工作区构建状态)
8. [修复优先级建议](#8-修复优先级建议)
9. [附录：审计方法](#附录审计方法)

---

## 1. 总体结论

原始怀疑（"未使用字段 = Swift 迁移时 Rust 没有正确使用"）**部分成立，且实际情况比表面警告更严重**。警告背后混杂三类问题：

| 类别 | 性质 | 数量 | 是否可见于警告 |
|---|---|---|---|
| **真缺陷** | 配置字段可写、行为读取缺失，功能静默失效 | 5 处确认 | 部分可见 |
| **无害遗留** | 同一逻辑的重复移植副本，现役路径正常 | ~10 处 | 可见 |
| **隐藏缺口** | `#[expect(dead_code)]` 主动压制的 "ported from Swift but not wired in yet" | 16 处 | **不可见** |

Gallery 方面：

- **≥20 个目录页只渲染标题文字**（占位 story，组件本体缺席）；
- **4 个 story 已导出但路由不可达**；
- **40+ 个公开组件完全没有 Gallery 展示**；
- 已渲染的 story 中大量 **孤立 State**（按钮点了没反应）；
- **当前工作区 Gallery 测试编译不过**（4 个错误），直接证明"迁移后 API 变了、调用方没跟上"。

---

## 2. 编译警告全量清单

`cargo check --workspace --all-targets`（Rust 侧，剔除 C/ObjC 第三方噪音后）：

### crates/addons（22 条）

| 位置 | 内容 |
|---|---|
| `crates/addons/src/workspace/drag.rs:21` | constant `DRAG_THRESHOLD` never used |
| `crates/addons/src/workspace/view.rs:32/33/34` | constants `TAB_PRESS_KEY` / `FLOATING_DRAG_KEY` / `FLOATING_REDOCK_KEY` never used |
| `crates/addons/src/workspace/view.rs:66/71` | functions `resolve_theme` / `debug_name` never used |
| `crates/addons/src/workspace/view.rs:957` | function `tab_group_view` never used |
| `crates/addons/src/workspace/view.rs:1018` | method `build` never used |
| `crates/addons/src/workspace/view.rs:1113` | associated items `new`, `region`, `write` never used |
| `crates/addons/src/workspace/view.rs:1517` | method `edge` never used |
| `crates/addons/tests/workspace_support/mod.rs`（11 条） | `make_document` / `make_bare_document` / `workspace_root` / `controller_with_fixture` / `collect` / `panel` / `group` / `group_node` / `debug_name` / `mutex`+`locked` / 多个 associated items never used |

### crates/compose（14 条）

| 位置 | 内容 |
|---|---|
| `.../controls/fields/slider/host.rs:56` | method `snap` never used |
| `.../layout/carousel/autoplay.rs:100/121/178/301` | `invalidate_progress`、多个 `new`、`configure`/`stop`/`contains_focus` never used |
| `.../overlays/anchored/anchoring.rs:184` | methods `width` / `key_handler` never used |
| `.../text/markdown/markdown_view.rs:265` | method `with_weight` never used |
| `.../data/chart/axis.rs:143/145/146` | value assigned to `left`/`top`/`bottom` never read（unused_assignments） |
| `crates/compose/tests/{app_storage,scale,nested_scroll_routing,timeline_group,wheel_picker}_tests.rs` | 测试内零散 never-used 项 |

### examples/gallery（6 条）

| 位置 | 内容 |
|---|---|
| `.../stories/sidebar_stories.rs:32/118` | field `draft` never read ×2 |
| `.../gallery_view.rs` | unused import / unused variable ×3 |
| `.../stories/field_stories.rs:295` | unused variable `field`（**构造的 TextField 被丢弃，见 §6.2**） |
| `.../tests/support/mod.rs`（19 条） | 测试辅助函数 never used |

### 原生侧（C/ObjC，非本次重点但记录在案）

- `cove-shell`：ObjC `native/apple.m` 7 处 `CVDisplayLink*` API 在 macOS 15 已废弃（应迁移到 `NSView.displayLink`）；`native.c:87` unused variable `props`。
- `cove-text`：`native.c:63/75/186` 三处 `-Wtautological-constant-out-of-range-compare`（`SIZE_MAX/sizeof(...)` 与 `FT_UInt`/`unsigned int` 比较恒真/恒假——溢出保护对 32 位字段实际无效，值得单独排查）。
- `cove-imaging`：`document->width() > INT_MAX` 比较的宏展开 note。

---

## 3. 未使用字段/方法逐项定性

判定标准：**公开 builder/配置 API 写入了字段，但全代码库无任何行为路径读取它** → 用户可配置、功能静默失效 = 真缺陷。

### 3.1 确认的真实缺陷（P0/P1）

#### 缺陷 1：AsyncImage 重试机制完全无效 ⚠️ P0

- 写入：`crates/compose/src/primitives/media/async_image.rs:306-307`（`pub fn retry_id(...)`）
- 读取：**全代码库零读取**（已 grep 验证；`request()` 构造 `ImageLoadRequest` 时只用 url/width/height/policy，见 async_image.rs:321-328）
- 影响链：`AvatarAppearance.retry_id`（`data/avatar/avatar.rs:72`，:289 处传入）和 Gallery 的 retry 按钮（`examples/.../image_story.rs:230`）都在调用它。
- 结论：**重试 token 永远无法击穿加载缓存，"重试"按钮实质是空操作。**

#### 缺陷 2：Carousel 自动播放整条链路未挂载 ⚠️ P0

- 用户 API：`Carousel::autoplay()`（`layout/carousel/mod.rs:178`）接受 interval/loop 等配置。
- 唯一能启动时钟的 `CarouselAutoplayHost`（`layout/carousel/autoplay.rs:108`，`advance` 逻辑在 :152-160、:257-280）**在 src 中从未被构造**（grep 验证：仅定义 + 测试引用）。
- 现状：`running_interval()`（mod.rs:223-228）读 interval 只用于渲染一个永远不会动的进度点；hover 暂停（autoplay.rs:268）无效果。
- 佐证：测试套件已经知道并"钉死"了这个缺陷——`crates/compose/tests/carousel_tests.rs:15-25` 注释称其为 "production gap"，`rotation_never_advances_because_no_clock_mounts`（:593-616）断言 `active_count()==0`。
- 结论：**autoplay 配置全部接受、引擎从未上弦。§2 中 autoplay.rs 的 8 条 never-used 警告全部由此派生。**

#### 缺陷 3：Markdown 表格内链接点击无效

- 写入：`crates/compose/src/primitives/text/markdown/table_block.rs:158-159`（`MarkdownTableView::on_open_link`）
- 读取：单元格渲染时 `RichText::new(self.runs.clone())` 从不传递 handler（table_block.rs:286）；`markdown_view.rs:126` 认真地做了 Markdown links → table links 的接线，但接的是死 setter。
- 对照：非表格链接经 `rich/link_anchor.rs:49` 正常工作。
- 结论：**表格内链接静止不可点。**

#### 缺陷 4：Workspace 浮窗"拖回中心重停靠"手势缺失

- `FLOATING_REDOCK_KEY`（`crates/addons/src/workspace/view.rs:34`）全库无引用。
- 现役实现 `workspace_view.rs` 的 Up 处理器直接丢弃 controller（`let _ = (&pointer_window, &pointer_controller);`，workspace_view.rs:2840），**但文档注释（:2784-2785）声称 "releasing redocks it onto the center group"**。
- 缓解：仅有一个手动 redock 按钮（workspace_view.rs:2656-2661）。
- 结论：**注释与实现矛盾——Swift 原型的释放-重停靠手势在 Rust 迁移中没有落地，不是简单死常量。**

#### 缺陷 5：Chart overlay 标注文字是空壳

- `ChartOverlayItem` 的 title/subtitle/rows 只写入节点 attachment `"chart.label.title"/"chart.label.subtitle"/"chart.label.rows"`（`overlays/anchored/overlay.rs:236-246`），**全代码库无任何读取方**（唯一 grep 命中即写入处）。
- 源码注释自认："strings are exposed through attachments until `Text` lands"。
- 结论：**图表 callout/annotation 渲染出外壳却没有文字。已知意图性 stub，但对使用者是静默 no-op。**

### 3.2 无害遗留（重复移植，功能正常；建议清理以防漂移）

| 项 | 定性 | 证据 |
|---|---|---|
| `slider/host.rs:56 snap` | 同一 snap 数学存在 3 份副本：`SliderHost::snap`（死）、`Snapshot::snap`（:417，活）、`ValueWriter::snap`（:453，活）。拖拽（:304/:345/:489）、键盘（:396）、无障碍（:169-183）全部走活副本。**步进吸附实际生效。** | slider/host.rs |
| `anchoring.rs:184 width/key_handler` | 只是便捷 setter 没人调用；底层字段经 `width_opt`（popover.rs:409-412 → anchored.rs:319/:381 消费）和 `host.key_handler` 直接赋值（popover.rs:261 → anchored.rs:335/:347-349 消费）完整接线。`Popover::width/on_key` 端到端正常。 | popover.rs, anchoring.rs |
| workspace `view.rs` 的 `tab_group_view`/`edge`/`build`/`resolve_theme`/`debug_name`/`TAB_PRESS_KEY`/`FLOATING_DRAG_KEY` | `view.rs` 是早期移植副本，现役 UI 在 `workspace_view.rs`（mod.rs:48 导出，全部测试引用它）。这些项在现役文件中都有等价物且在使用：`TAB_PRESS_KEY` ↔ workspace_view.rs:60（:2096-2185 使用）、`FLOATING_DRAG_KEY` ↔ :61/:2821-2861、`edge()` ↔ :2492/:2500、`WorkspaceLayoutNodeView` ↔ :1211/:1037。`view.rs` 的 pub 辅助（`rail_plan` 等）仍在用，死的是私有 tab-drag 子树（view.rs:591-682）。 | view.rs / workspace_view.rs |
| `drag.rs:21 DRAG_THRESHOLD` | 现役 `workspace_view.rs:64` 自己重新声明 `const DRAG_THRESHOLD: f32 = 4.0`（:2179 使用），拖拽阈值实际生效。**但同一常量双处定义 + 测试 `drag_threshold_is_four_points`（drag.rs:546）只是重言断言，存在漂移风险。** | drag.rs, workspace_view.rs |
| `chart/axis.rs:143-146` | `unused_assignments` 误报级别：`let mut left/... = 0.0` 初始化值确实不读，但计算值被 clamp 后正确喂进绘图矩形（:203-212）。`right` 未被标记因为 Horizontal 分支合法依赖其 0.0。**轴留白数学正确。** | axis.rs |
| `sidebar_stories.rs:32/118 draft` | example 的 `State<String>` 初始化后（:41/:129）无任何引用。移植时抄了"保留草稿状态"却丢掉了消费它的 TextField 绑定。纯展示层死状态，无公开 API 影响。对照正确接线：`markdown_story.rs:93-98`、`navigation_stories_file.rs:1038-1043`（:1066-1083 渲染绑定）。 | sidebar_stories.rs |
| `addon tests/workspace_support`、gallery `tests/support` 的 never-used 函数 | 测试脚手架超集，各测试二进制只编译用到的子集；无产品影响。 | — |

---

## 4. 被 `#[expect(dead_code)]` 压制的未接线功能

**这是比可见警告更大的隐患**——它们不产生任何警告，全部自我标注为：

```rust
#[expect(dead_code, reason = "ported from Swift but not wired in yet")]
```

即：代码从 Swift 移植过来了，但从未接入行为路径。共 16 处，全部位于 `crates/compose/src`：

| # | 位置 | 内容 | 用户可感知影响 |
|---|---|---|---|
| 1 | `overlays/menu/touch_menu.rs:300` | `TouchMenuRow.is_tab_stop` | 触摸菜单行按作者意图应可键盘聚焦，实际不可 |
| 2 | `layout/sidebar/resize_handle.rs:90` | `ResizeHandle::write` | 键盘驱动侧栏缩放存在代码但从输入路径不可达 |
| 3 | `layout/sidebar/sidebar.rs:392` | Sidebar（非 adaptive 变体）内部件 | 见 §5：整个非 adaptive Sidebar 无展示 |
| 4 | `layout/sidebar/sidebar.rs:479` | 同上 | 同上 |
| 5 | `layout/sidebar/adaptive_sidebar.rs:107` | `AdaptiveSidebar::set_collapsed` | **外部折叠绑定被静默忽略**；宿主传 `is_collapsed` 期望程序化控制时无效（仅用户拖拽会回写） |
| 6 | `layout/virtualization/variable_stack.rs:22` | `VariableStack.viewport` | **变高虚拟化忽略视口信息**——每 pass 捕获 `ScrollViewport` 但从不咨询 |
| 7 | `layout/scroll_view.rs:145` | `ScrollView.track_start` | 滚动条几何字段保存不读；thumb 数学只用 track_length/thumb_length，起点偏移场景可能偏差 |
| 8 | `layout/grid/lazy_grid.rs:606` | `LazyGridCell.key` | **单元格身份 key 保存但从不参与复用/diff**——按 key 稳定复用的语义无法兑现 |
| 9 | `navigation/stepper/stepper.rs:83` | `Stepper::select` | "忽略禁用 id"规则只活在死代码里，选择仅经绑定路径流动 |
| 10 | `controls/fields/slider/host.rs:65` | `SliderHost::increment` | 死副本；活的增量逻辑在 Snapshot 路径（见 §3.2） |
| 11 | `text/markdown/code_block.rs:210` | 代码块内部件 | 代码块相关能力未接线 |
| 12 | `data/timeline.rs:255` | `is_collapsed` 助手 | 逻辑在 :315-333 内联重复了一份——**助手与内联副本漂移风险** |
| 13 | `data/timeline.rs:267` | `toggle_collapsed` 助手 | 同上 |
| 14 | `data/timeline.rs:290` | `zoom_factor` 助手 | 内联重复在 :304 |
| 15 | `data/data_table/view.rs:2057` | 编辑器 `column_id` | **格内编辑器记住编辑哪列却从不读取——按列定向提交无法工作** |
| 16 | `media/icon.rs:97` | `Icon::resolved_fill` 助手 | `self.fill` 在 :111 内联读取，填充配置实际生效；重复遗留 |

分类小结：

- **真缺陷（承诺的行为没接上）**：#1、#2、#5、#6、#8、#15，以及 #9 的禁用项规则
- **重复/漂移风险**：#12、#13、#14、#16、#10
- **待接入的半成品**：#3、#4、#11、#7

---

## 5. Gallery 组件覆盖缺口

判定口径：组件类型在 `routing.rs` 可达 story 的 `body()` 中被真实构造 = 已覆盖。

### 5.1 已有专门 story 的组件（56 页，抽样列举）

Button / Checkbox / Toggle / Slider / RangeSlider / TextField / NumberField / Vec3Field / ColorField / JsonField / SearchField / Select / AssetRefField / Rating / Skeleton / Combobox / Form / RadioGroup / Tooltip / NotificationHost / DatePicker / DateRangePicker / TimeField / SplitButton / MenuButton / KeyCap / Link / Icon / TimePicker / WheelPicker / CalendarView / AppBar / Toolbar / ActionSheet / HoverCard / Popconfirm / TabView / Accordion* / Tag / NavigationStack / Stepper / NavigationDrawer / AdaptiveSidebar / Avatar 家族 / Image / AsyncImage / Chart / Table(自适应样例) / Tree / List / ReorderableList / Scrollbar / PropertyGrid / Timeline / Alert / Badge / EmptyState / ProgressView / Spinner / Panel / SplitView / ScrollView / ZStack / StatusBar / Carousel* / Grid / Masonry* / RichText / MarkdownView 等。
（* 号见 5.3：页面存在但 story 为占位/部分占位）

### 5.2 仅在其他 story 中间隙出现的组件

- **ButtonGroup / ButtonGroupItem** — 只在 overview.rs 仪表盘出现；其专属页是占位（见 5.3）
- **ContextMenu（`.context_menu()`）** — 只在 sidebar_stories.rs 的示例行内出现一次
- **MenuEntry / MenuItem / MenuSubmenu** — 作为 MenuButton/Toolbar/AppBar 的数据类型出现
- **RefreshOptions** — data_stories_file.rs 的 RefreshListStory import 了但从不渲染可刷新列表
- **SafeArea / LayerRoot / ViewThatFits** — 仅 Gallery 外壳（gallery_view.rs）使用，无 story

### 5.3 有目录页、但 story 只渲染标题文字的占位页（≥20 个）⚠️

路径均相对 `examples/gallery/src/gallery_content/stories/`：

| 页面 | 证据 | 备注 |
|---|---|---|
| Modal | `overlay_stories_file.rs:496-504` 只有 StorySection + 状态 Text | 无 Modal 组件、无打开按钮；`ModalStory.is_open`（:452）成死状态 |
| Popover | `overlay_stories_file.rs:429-437` 一行 Text | `is_open`（:393）只写不读 |
| Disclosure | `overlay_stories_file.rs:552-560` | 无 DisclosureGroup；`is_expanded` 未用 |
| CommandPalette | `command_story.rs:143-181` 2 按钮 + caption | 组件从未挂到 `is_presented` |
| DataTable | `data_table_story.rs:399-415` 只有状态 Text | `selection/is_frozen/order/expanded` 全部只服务于未测辅助函数 |
| VirtualStack | `data_stories_file.rs:369-377` 一行 Text | 无 VirtualStack、无 scroll-to 按钮 |
| SwipeList | `data_stories_file.rs:457-466` 日志 Text | 无 SwipeList |
| RefreshableList | `data_stories_file.rs:533-542` 计数 Text | `options()`（:519）从未挂到任何列表 |
| DragAndDrop | `data_stories_file.rs:601-613` caption | 无 Draggable/DropTarget；`drop_into_a/b` 不可达 |
| SectionedList | `data_stories_file.rs:654-662` "3 sections" 文字 | 无 SectionedList |
| Accordion | `navigation_stories_file.rs:178-186` 仅 caption | **`accordion()` 在 :135-146 已构建真组件，`body()` 却没调用** |
| Breadcrumb | `navigation_stories_file.rs:235-243` | 只 import 了 model 类型 |
| Pagination | `navigation_stories_file.rs:303-311` | `items()` 未用 |
| ButtonGroup | `navigation_stories_file.rs:377-386` | `mode_segments()/category_segments()` 未用 |
| Dialog | `navigation_stories_file.rs:702-711` | `is_open,is_saved`（:649-651）死状态 |
| AlertDialog | `navigation_stories_file.rs:768-780` | `is_open`（:725）死状态 |
| Label | `navigation_stories_file.rs:826-838` | 无 FieldLabel/Tag/KeyCap，尽管 catalog 文案承诺（catalog.rs:758-762） |
| Sheet | `navigation_stories_file.rs:903-912` | `is_open,setting`（:863-865）死状态 |
| BottomSheet | `navigation_stories_file.rs:983-992` | `is_open,notifications`（:932-934）死状态 |
| NavigationRail | `navigation_layout_stories.rs:318-340` | 只有一个 "Show labels" Checkbox；`items()/presentation()` 从不渲染；`selection`（:244）死状态 |
| ContextMenu | `routing.rs:243-245` 路由到 MenuStory | 与 Menu 页内容完全相同，无右键目标演示 |

另：**PlacementExample**（placement_example.rs:185-221）当时是占位且不可达——已修复：每个放置方向一个真实 `Popover`（带 `trigger_description` 可访问名），并接入 Popover 页；`tests/gallery_tests.rs` 的 GAP 断言翻转为 12 个触发器逐个校验。

**AdaptiveControlExamples（已删除，不再移植）**。Swift 原件 `Stories/Foundation/AdaptiveExamples.swift`（551 行）由两部分组成：一层外壳（把页面塞进 390pt / 720pt 定宽容器，用 `.surfaceEnvironment(...)` 覆写 viewport 与输入能力）和一份 `AdaptiveControlSample` 大 switch（为 97 个页面各手写一份最小样例）。Rust 侧只有外壳的数据表（`SUPPORTED`、`PREVIEW_WIDTHS`、`frame_height`、`captions`）与一行 caption 的 `body`，从未被任何页面路由。**决定**：定宽预览外壳不进入 Rust 画廊——它复刻的是 Swift 宿主窗口的伪环境，而 Rust 每个页面已有真实 story，再套一层固定宽度需要嵌套 `LayerRoot` 才能容纳 portal，收益不足以支付这部分复杂度。因此删除整个 `adaptive_examples.rs`（含 `GalleryPage::has_adaptive_sample` / `adaptive_sample_height` / `adaptive_debug_name` 扩展方法与相关属性测试），`tests/split_tests.rs` 重写为直接挂载 `SplitViewStory`（断言三个分割视图、作者尺寸与夹紧区间），`tests/rail_tests.rs` 去掉 adaptive 断言并改为断言真实 `navigation-rail`。

### 5.4 完全无 Gallery 覆盖的公开组件

路径均相对 `crates/compose/src/primitives/`：

**controls/**
- CompletionPopover — `controls/completion_popover.rs`
- FieldLabel / FieldLabelOptions — `controls/field_label.rs`（Label 页为占位）
- MenuButton 结构体本身（仅 MenuButtonTitle 被构建）— `controls/menu_button.rs`
- ToolbarGroup、ToolbarSeparator — `controls/toolbar/mod.rs`
- TimeInput（+ time_input_model）— `controls/fields/time_input.rs`
- DateRangeCalendar — `controls/fields/date_range.rs`
- FormLayout / label_placement / label_scope — `controls/form/`

**data/**
- DataTable / DataTableModel / DataTableOptions — `data/data_table/`（页面占位）
- TreeTable / TreeTableColumn / TreeTableRow — `data/tree_table.rs`
- TableColumns — `data/columns/table_presentation.rs`
- SectionedList / ListSection / StickyHeader — `data/lists/sectioned_list.rs`（占位）
- SwipeList / SwipeAction — `data/lists/swipe_list.rs`（占位）
- ListGroup — `data/lists/inset_group.rs`（仅 mobile/ demo 使用，不在 gallery stories）
- ChartLegend / ChartOverlay / ChartMorph — `data/chart/`（Chart 已覆盖，子部件未直接演示）

**feedback/**
- ProgressValue — `feedback/progress_value.rs`

**interaction/**
- Draggable / DropTarget / DragSession / DragScope — `interaction/drag_and_drop.rs`（占位）
- HorizontalSwipe — `interaction/horizontal_swipe.rs`
- EdgeAutoScroll — `interaction/edge_auto_scroll.rs`

**layout/**
- DisclosureGroup — `layout/disclosure_group.rs`（占位）
- Breadcrumb / Pagination  widget — `layout/navigation_components.rs`（占位）
- NavigationRail / SidebarRail — `layout/navigation_rail.rs`、`layout/sidebar/rail.rs`（占位/缺席）
- Sidebar（非 adaptive 变体）— `layout/sidebar/sidebar.rs`
- Overlay / OverlayLayout — `layout/overlay.rs`
- SurfaceRoot — `layout/insets/surface_root.rs`
- ViewThatFits / ViewThatFitsMode — `layout/responsive/view_that_fits.rs`（仅外壳）
- ActionGroupLayout — `layout/responsive/action_group.rs`
- ResizableEditor、ResizableTextArea — `layout/`
- ScrollReveal — `layout/scroll_reveal.rs`
- HorizontalScrollOrigin — `layout/horizontal_scroll_origin.rs`
- MobileTabBar / MobileTabStyle — `layout/mobile_tab_bar.rs`
- SplitCompactPresentation / SplitPaneSelector / SplitPane — `layout/split_compact.rs`
- TabOverflowLayout / TabOverflowStrip — `layout/tab_overflow.rs`
- SplitHandle（独立使用）— `layout/split/handle.rs`
- EditorHeightHandle / EditorHeightLimits — `layout/resize/`
- VirtualStack / FixedVirtualStack / EstimatedVirtualStack / VariableVirtualStack / VirtualWindow — `layout/virtualization/`（占位）
- SafeArea — `layout/insets/safe_area.rs`（仅外壳）

**media/**
- UIIcons — `media/ui_icons.rs`

**navigation/**
- RouteOutlet — `navigation/route_outlet.rs`

**overlays/**
- Modal / ModalOptions / ModalBackdrop — `overlays/modal/modal.rs`（占位）
- Dialog / AlertDialog — `overlays/modal/dialog.rs`（占位）
- Sheet / SheetDragHandle — `overlays/modal/sheet.rs`（占位）
- BottomSheet — `overlays/modal/bottom_sheet.rs`（占位）
- Menu（浮动菜单表面）— `overlays/menu/menu.rs`
- Popover / PopoverSizing — `overlays/popover.rs`（占位）
- CommandPalette — `overlays/command_palette.rs`（占位）
- FocusScope、PortalHost、OverlayHost、AssetDropTarget、TouchMenu、RovingFocusHost、ShortcutHost、SelectionKeyHost、TouchPresentation — `overlays/` 各文件

**text/**
- Divider — `text/text.rs`（**任何 story 都从未构造过**）
- TextGutterControls 及 code chrome 家族 — `text/code/`
- TextSelectionRoot — `text/rich/text_selection.rs`
- MarkdownCodeBlock / MarkdownListView / MarkdownTableView — `text/markdown/`（仅经 MarkdownView 间接渲染）
- SingleLineText — `text/single_line.rs`

**columns/**
- ColumnVisibilityPickerModel / ColumnPickerEntry — `columns/picker.rs`
- ColumnVisibilityPlan / ColumnVisibilityTrack — `columns/mod.rs`

**已导出但路由不可达的 story（4 个）** —— 已处理：
- `AdaptiveControlExamples`：删除（不移植，见 §5.3 末条）。
- `PlacementExample`：成为真实组件并接入 Popover 页。
- `LazyGridStory`、`MasonrySample`：接入 Layout 页（懒加载网格、Masonry 样例两节）。

---

## 6. Gallery 已展示内容的质量问题

### 6.1 孤立 State：交互"点了没反应"

以下 story 的 State 字段被 setter 写入，但 `body()` 中无任何消费者——对应页面按钮全部失效：

- `CommandStory.is_custom_presented` — command_story.rs:55
- `PopoverStory.is_open` — overlay_stories_file.rs:393（仅 :410 写）
- `ModalStory.is_open/name` — overlay_stories_file.rs:452-453
- `DialogStory.is_open,is_saved` — navigation_stories_file.rs:649-651
- `AlertDialogStory.is_open` — :725；`SheetStory.is_open,setting` — :863-865；`BottomSheetStory.is_open,notifications` — :932-934
- `TagStory.filters` — :447（只渲染了 `tags`）
- `RailStory.selection` — navigation_layout_stories.rs:244
- `DataTableStory.selection/is_frozen/order/expanded` — data_table_story.rs

### 6.2 头条组件被构造后丢弃

`stories/field_stories.rs:295`：

```rust
let field = AnyView::new(TextField::new("Project name", self.text.binding()) ... )
```

`field` 从未加入返回的视图树（编译器警告 unused variable）——**TextField 页的头条演示不渲染**。

### 6.3 演示逻辑错误

- **TimePicker "Now" 按钮写死 9:30**（`DEFAULT_HOUR/DEFAULT_MINUTE`）而非当前时间 — time_picker_story.rs:135-137
- **WheelPicker story（当前工作区正在修改）**：`hour_wheel/minute_wheel/period_wheel/volume_wheel`（:129-143）构建了 body 从不使用的 `WheelPickerModel` 副本；`reset()`（:107）注释声称"Reset button does this"但 **UI 里没有 Reset 按钮**；`set_time_24h`（:118）同样不可达。组件 API 用法本身（`WheelPicker::new(binding, options)`，:153 ↔ wheel_picker.rs:205）无误。

### 6.4 健壮性

20+ 处 story body 使用 `try_new(...).expect(...)`（overlay_stories_file.rs:694/709/754、stepper.rs:125/198/239、layout.rs:319/343/432/454、product_stories.rs:301/447/531 等）。当前因 id 静态唯一而安全，但任何改动可能让 Gallery 整页 panic。src/ 中无 `.unwrap()` 泄漏（测试模块除外）。

---

## 7. 当前工作区构建状态

`cargo check --workspace --all-targets`：**通过**（exit 0，警告见 §2）。

`cargo test -p cove-examples`（examples/gallery）：**测试套件编译失败，4 个错误**：

1. `E0432` — `data_stories.rs:25`：`Table`、`TableOptions` 在 `cove_ui_compose::primitives::data::data_table::table_column` 中已不存在
2. `E0308` — `data_list_stories.rs:104`：`.on_activate(move |id| ...)` 期望 `usize`，实得 `&usize`
3. `E0560` — `data_stories_file.rs:251`：`TreeSelection` 无字段 `primary`（现有：`ids`/`keys`/`expanded_keys`）
4. `E0560` — `data_stories_file.rs:252`：`TreeSelection` 无字段 `multiple`

这 4 处正是"组件 API 迁移变更后，调用方（story）未同步"的直接实证。另有 12 条编译警告（unused `ViewExt` import ×5 等）。

---

## 8. 修复优先级建议

| 优先级 | 项目 | 理由 |
|---|---|---|
| **P0** | AsyncImage `retry_id` 接入加载缓存键（缺陷 1） | 用户可见功能完全失效 |
| **P0** | 挂载 `CarouselAutoplayHost`，接通 autoplay（缺陷 2），并翻转 carousel_tests.rs 中钉死缺陷的断言 | 公开 API 静默 no-op |
| **P0** | 修复 examples/gallery 测试编译错误（§7） | 工作区当前不可测 |
| **P1** | Markdown 表格链接 handler 接线（缺陷 3） | 用户可感知的交互缺失 |
| **P1** | 决策：实现 workspace_view 释放-重停靠手势，或修正错误注释并删除死 key（缺陷 4） | 注释与实现矛盾 |
| **P1** | 逐一处理 16 处 `expect(dead_code, "not wired in yet")`：接线或删除（重点 §4 中标"真缺陷"的 6 处：touch_menu tab-stop、resize 键盘、set_collapsed、variable_stack viewport、lazy_grid key、data_table column_id） | 不产生警告、最易被再次忽略 |
| **P1** | Gallery 占位页替换为真实组件渲染（§5.3 的 20 页；Accordion 最省力——`accordion()` 已写好，body 调用即可）；修复 §6.1 孤立 State 与 §6.2 丢弃的 TextField、§6.3 Now 按钮 | Gallery 是组件质量的门面 |
| **P2** | ~~Chart overlay label 文字渲染落地（缺陷 5）~~（已完成） | 已知意图性 stub |
| **P2** | 清理重复移植：删除 `view.rs` 死子树、slider snap/increment 三副本合一、`DRAG_THRESHOLD` 单一定义、Timeline 助手与内联副本合一 | 消除漂移风险，顺带消掉大部分现存警告 |
| **P2** | ~~路由接入不可达 story；ContextMenu 页改为真实右键演示~~（已完成：PlacementExample 接入 Popover 页、懒加载网格/Masonry/Divider 接入 Layout 页、ContextMenu 独立演示；AdaptiveControlExamples 判定不移植并删除） | 覆盖缺口 |
| **P3** | story body 中 `expect()` 的系统性替换；原生侧 CVDisplayLink 废弃 API 与 cove-text 恒假比较警告单独立项 | 健壮性/跨平台 |

---

## 附录：审计方法

1. `cargo check --workspace --all-targets --message-format=short` 提取全部 Rust 警告，按 `never read|never used` 过滤并定位到 file:line。
2. 对每条警告：读取源码，追踪字段写入点（构造/builder/setter）与读取点（渲染、命中、提交、无障碍），区分"仅 Debug 读取""重复副本""真实无消费"。
3. `grep -rn "expect(dead_code" crates/compose/src` 量化被压制的死代码及其 reason 字符串。
4. 组件覆盖：以 `crates/compose/src/primitives/` 公开导出为基准，与 `examples/gallery` 的 `catalog.rs` 页面清单 + `routing.rs` dispatch arm + story `body()` 实际构造类型三方比对。
5. 质量抽查：逐文件读取 story body，核对 State 读写闭环、构造值是否入树、演示逻辑正确性；`cargo test -p cove-examples` 验证测试套件状态。

*本审计为只读验证，未修改任何代码。*
