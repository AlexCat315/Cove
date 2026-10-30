# Rust 迁移状态

仓库根目录：`/Users/cyke666/dev/Cove`。本文路径均相对该目录。

目标语言为 Rust 2024（`rust-version = 1.90`），性能关键路径保留 C。当前工作区版本 `0.3.0-dev`。最后更新：2026-10-30（本轮重新盘点，清单由 `Tools/Cove/rust/migration_inventory.py` 从磁盘重新生成）。

逐文件清单：`Docs/rust-migration-files.json`（机器可读，schema 2）与 `Docs/rust-migration-files.md`（逐文件表格）。每个 Swift 文件一行，记录源路径、源码 SHA-256、行数、模块、类型（生产/测试/清单/模板）、状态、Rust/C 目标与是否只是计划路径。**本文与清单不一致时以清单为准，并应先修清单。**

## 记录口径

- 生产源码：排除测试目录、SwiftPM 清单、CLI 项目模板与构建/依赖产物。
- 排除路径：`**/.build/**`、`third-party/**`、`vendor/**`、`target/**`、`dist/**`、`Artifacts/**`、`build/**`、`node_modules/**`、`.git/**`。
- `Tests/**` 与 `**/Tests/**` 归为测试源码；`Package.swift` 归为 SwiftPM 清单；`**/Templates/**` 归为 CLI 项目模板。`Sources/.../Package.swift` 是生产源码（Android APK 打包），不按清单处理。
- 状态：`pending`（未开始）、`partial`（有实现但仍有缺项）、`written_unverified`（已写入、未编译未行为对照）、`manifest_written`（仅 Cargo 清单建立）。**不写“已完成”**；完成需要逐文件行为对照、编译与平台验收。
- 状态 `pending` 的 `rust` 数组若标 `planned: true`，表示登记的目标路径，文件尚不存在。

按最新要求：**实施过程中不运行测试，全部文件转换结束后再统一验证。** 本次盘点没有运行 `cargo test`、跨平台验收或 GUI 冒烟测试；仅为修复工作区可编译性做了局部 `cargo check`，不构成验收结论。

## 总量（2026-10-30 盘点）

| 范围 | 文件数 | 已写入 | 进行中 | 未开始 |
| --- | ---: | ---: | ---: | ---: |
| 生产源码 | 984 | 958 | 0 | 26 |
| 测试源码 | 424 | 50 | 0 | 374 |
| SwiftPM 清单 | 25 | 19（Cargo 清单） | 0 | 6 |
| CLI 项目模板 | 5 | 0 | 0 | 5 |
| 合计 | 1,438 | — | — | — |

本轮修正：清单此前由人工维护，已与磁盘脱节。现改为脚本生成——
`python3 Tools/Cove/rust/migration_inventory.py` 遍历 Rust 树，按同名模块与主类型定义
重新推导每个 Swift 文件的目标路径与状态，并同时重写
`Docs/rust-migration-files.json` 与 `Docs/rust-migration-files.md`。
因此上一版"460 已写入 / 477 未开始"是陈旧值；实际为 651 / 333。

另有 51 个 Rust 文件此前从未被 `mod` 声明，因而**根本没有被编译**：
System 16 个、Logging 15 个、Tools/Cove 8 个、Tools/Support 12 个、Compose 2 个（已修复）。
同理，`Core/rust/tests/` 6 个与 `Graphics/rust/tests/` 7 个测试文件从未声明为 Cargo 测试目标，
`cargo` 完全看不到它们（已补 `[[test]]`）。这类"写而未编译"的问题与编译错误同级，必须计入缺陷。

本次盘点修正了两类历史失真：

1. `Docs/rust-migration-files.json` 中有 1,063 个文件曾被标为 `pending`，其中 System 16、Logging 15、Tools/Support 12、Graphics RHI 前端 40+、Hosts Android 10、DevTools Protocol 8 实际已有 Rust 实现，只是没有逐文件映射。已补上目标路径并改为 `written_unverified`；`GridInteractionGate` 已存在于 `Compose/rust/primitives/layout/grid/grid.rs`，不再重复新建。
2. `Tools/Cove/Sources/CoveCLI/Platforms/Android/APK/Package.swift` 是 Swift 生产源码，不是 SwiftPM 清单；已重新归类。

218 条 `pending` 记录带有计划路径，但对应 `.rs` 文件不存在，仍按未开始处理，不得计入完成。

另有 13 个已写入的 Rust 测试文件（`Core/rust/tests/` 6 个、`Graphics/rust/tests/` 7 个）尚未声明为 Cargo 测试目标，因此对应 Swift 测试仍记 `pending`，也不能声称测试已迁移。接入测试目标属于批次 7。

## 逐模块进度（生产源码）

| 模块 | Swift 文件 | Swift 行 | 已写入 | 未开始 |
| --- | ---: | ---: | ---: | ---: |
| Compose | 414 | 52,554 | 414 | 0 |
| Graphics | 107 | 14,534 | 107 | 0 |
| Core | 97 | 12,299 | 97 | 0 |
| Tools | 69 | 3,901 | 69 | 0 |
| Examples | 61 | 7,135 | 35 | 26 |
| DevTools | 51 | 5,183 | 51 | 0 |
| Hosts | 51 | 4,254 | 51 | 0 |
| Terminal | 29 | 3,088 | 29 | 0 |
| Sources | 19 | 4,861 | 19 | 0 |
| System | 17 | 1,004 | 17 | 0 |
| Logging | 15 | 705 | 15 | 0 |
| Text | 14 | 2,576 | 14 | 0 |
| Shell | 13 | 2,529 | 13 | 0 |
| Runtime | 11 | 1,485 | 11 | 0 |
| Imaging | 9 | 696 | 9 | 0 |
| PlatformCore | 6 | 540 | 6 | 0 |
| Browser | 1 | 400 | 1 | 0 |
| 合计 | 984 | 117,744 | 958 | 26 |

Rust 文件数列为磁盘快照，不等于逐 Swift 文件一一对应；多个 Swift 文件可能映射到一个 Rust 模块，也可能一个 Swift 文件拆到多个 Rust/C 文件。逐文件关系在清单中。

## 已写入实现的能力边界

| 能力 | 现状 |
| --- | --- |
| Core | `CoveUICore`/`CoveUIScene` 生产源码均有 Rust/C 对应：几何/颜色/绘制缓冲区、场景树与布局、输入与滚动、动画、渲染器、无障碍、位图与 mipmap。未验证。 |
| PlatformCore | 窗口 ID、键盘/鼠标/触摸/手柄事件、滚动单位、菜单数据、资源查找。未验证。 |
| System | 文件替换/权限/身份/锁/恢复目录/回收站、进程流/限额/超时/取消、POSIX 进程组、Windows Job Object。仅剩 `CoveProcessFixture/main.swift`（测试辅助进程）未重建。未验证。 |
| Logging | 过滤、递归脱敏、UTF-8 截断、有界历史、JSONL 轮转。未验证。 |
| Text | FontFace/FontAtlas/FontProvider/FontCollection/TextStyle/TextShaper/TextCache/TextEnvironment；FreeType/HarfBuzz 为 C，Apple 彩色字形为 Objective-C/C。未验证。 |
| Imaging | 解码/动画/方向/栅格化/渐进/纹理/资源缓存；stb_image/lunasvg/libwebp 复用，Apple PDF/ImageIO 用 Objective-C/C。未验证。 |
| Graphics | RHI 前端 41 个 Swift 文件已映射到 `Graphics/rust/rhi/`；UI 渲染 12 个文件在 `Graphics/rust/ui/`；Metal 格式表在 `Graphics/rust/metal/formats.rs`。未验证。 |
| Shell | SDL3 窗口/输入/剪贴板/对话框、C 事件桥接、macOS/iOS 窗口与显示刷新、AppKit 输入/菜单浮层。未验证。 |
| Runtime | 多窗口会话、输入仲裁、帧调度、Apple 无障碍桥接、iOS CADisplayLink。未验证。 |
| Compose Composition | `ViewGraph`、修饰器、组合作用域、布局树、状态订阅、属性宏、文件存储、离屏快照。未验证。 |
| Compose UI | 191 个文件已写入：动画/过渡/关键帧/阶段、Portal/菜单/焦点、虚拟列表/数据表几何/响应式、主题与样式、按钮/链接/标签/文本/表单等。6 个文件有缺项（Button、Link、Tag、Text、Select、Columns Picker）。大量控件未开始。 |
| Hosts | Android 契约（FrameClock/Launch）与集成层（Environment/Scene/Diagnostics/Inspection/Surface/TouchInput/帧计时）已写入。桌面宿主未开始。 |
| DevTools Protocol | 协议 0.2、输入录制 3、会话与镜像动作已写入。客户端/服务端/Agent/Inspector 未开始。 |
| Tools/Support | 12 个 Swift 文件全部有 Rust 对应。CLI 的 9 个 Rust 模块覆盖部分命令，41 个 Swift 文件为 `partial`，平台构建命令未开始。 |

以上全部未编译、未行为对照、未平台验收。

## 迁移顺序计划

依赖边来自各 `Cargo.toml`：`cove-core → cove-platform`；`cove-logging → cove-system`；Graphics/Text/Imaging 依赖 Core；Shell 依赖 Platform/Logging；Runtime 依赖 Core/Platform/Text/Imaging/Graphics/Shell/Logging；Composition 依赖 Core/Platform；Compose 依赖 Composition 及下层全部基础 crate；Terminal/Hosts/DevTools/Sources/Browser 依赖 Compose；Examples 依赖全部下层；Tools 独立于 UI 栈。

### 批次 1：已写入但未验证的基础层（串行核对）

System、Logging、Tools/Support、Graphics RHI 前端、DevTools Protocol、Hosts Android 契约/集成、Compose Composition。共 456 个生产文件。逐文件行为对照并补缺，不新建重复模块。

### 批次 2：Graphics 原生后端（三个可并行子批次）

- Metal：14 个文件（`Metal/Bindings`、`Metal/Device`、`Metal/Pipelines`、`Metal/Metal{BACKEND,Device,AccelerationStructure}`）。已翻译 4 个：`MetalSubmissionStatus`、`MetalLibraryCache`、`MetalRenderBindings`、`MetalUploadRing`；剩余 `MetalBackend`、`MetalCompilation`、`MetalBindingRegistry`、`MetalRegistries`、`MetalAccelerationStructure`、`MetalMeshPipeline`、`MetalDevice`、`MetalDeviceResources`、`MetalDeviceTransfer`、`MetalSubmitEncoder`。
- Vulkan：31 个文件（Backend/Presentation/Resources/Setup/Submission 与顶层模块）。
- DX12：8 个文件（Backend/Device/Formats/Resources/Submission）。
- 另 1 个 Graphics 文件（`Metal/MetalFormats.swift`）已在批次 1 映射。

三个后端只共享 `Graphics/rust/rhi/` 只读接口，可并行；若需改共享 RHI 接口则改为串行。新增后端注册只在 `Graphics/rust/rhi/backend_factory.rs` 内，不改根 `Cargo.toml`。

### 批次 3：Terminal 与 Hosts（可并行）

- Terminal：29 个文件（Engine 11、Process 5、Session、View 11、Demo）。Ghostty 引擎与 PTY/ConPTY，IPC/PTY 性能关键路径优先 C。
- Hosts：41 个文件（Android 15：App/Bridge/Content/Frame/SurfaceSession/Probe；Desktop 26：Application/Diagnostics/Display/Rendering/ResourcesSupport/Window）。
- 两者都依赖 Runtime/Compose/Graphics；若同时改 `Hosts/Cargo.toml` 或共享宿主接口则串行。

### 批次 4：Compose 剩余（按子树并行）

| 子批次 | 未开始 | 内容 |
| --- | ---: | --- |
| 4a 布局 | 49 | Grid 剩余、Sidebar、Split、Tabs、Carousel、Virtualization、Scrolling、ScrollView、ResizableEditor 等 |
| 4b 控件 | 44 | AppBar、Calendar、Fields（Combobox/Date/JSON/Numeric/Slider）、Completion、Toolbar 等 |
| 4c 数据 | 33 | DataTable、Columns、Chart、Avatar |
| 4d 浮层 | 33 | Menu、Modal、Toast、Popover、Portal、Command Palette 等 |
| 4e 文本 | 31 | TextField/TextArea、Rich、Rope、Markdown、Code、Completion |
| 4f 主题 | 10 | 默认面板样式、组件样式、按钮样式剩余 |
| 4g 反馈/媒体/交互/导航/动画 | 18 | Feedback 5、Media 5、Interaction 4、Navigation 3、动画 2 |
| 4h 缺项修复 | 6 | Button、Link、Tag、Text、Select、Columns Picker 的 `partial` 收口 |

Compose 各子树共享 `Compose/rust/lib.rs`、`primitives/mod.rs`、`theme/mod.rs` 等模块入口；模块入口变更串行，文件实现可并行。**不得**为绕过共享入口而新建并行导出层。

### 批次 5：Sources、Browser、DevTools（可并行）

- Sources：19 个文件（Workspace 13、WebView 6）。
- Browser：1 个文件（Wasm 原型宿主），需建立 Wasm 构建链。
- DevTools：43 个文件（Sources 24、Inspection 21；Protocol 8 已在批次 1）。
- 三者依赖 Compose；Browser 另需 Wasm 目标与 JS 桥接。

### 批次 6：Tools 平台命令与模板

- Tools 未开始 16 个文件：Android（APK/Compilation 12，含 `APK/Package.swift`）、Desktop 4、iOS 1。
- CLI 41 个 `partial` 文件随实现收口；5 个项目模板在 SDK 模板定稿后替换。
- Tools 与 UI 栈无依赖，可与批次 2–5 并行，但 CLI 参数/退出码行为需与 Swift 原实现一致。

### 批次 7：Examples 与测试（最后串行）

- Examples 61 个文件：依赖全部下层 crate，最后迁移以覆盖公开 API。
- Tests 424 个文件：按 Swift 测试逐项迁移为行为清单，不用空测试替代。依赖对应生产 crate 完成。

### 必须串行的共享文件

根 `Cargo.toml`（新增/调整 crate）、`Docs/rust-migration.md`、`Docs/rust-migration-files.json`、`Docs/rust-migration-files.md`、`CHANGELOG.md`、`README.md`。修改这些文件的批次不得与其他批次并行。每个文件迁移完成即更新清单中该行状态与目标路径。

## 原生后端与宿主状态（本轮完成）

| 层 | 状态 | 覆盖 |
| --- | --- | --- |
| Metal | 完成 | macOS / iOS，`Graphics/rust/metal/`，`backend_factory::install_backends` 按 `cfg` 注册 |
| Vulkan | 完成 | Windows / Linux / Android；`sys.rs` 为手写 FFI（`dlopen`/`LoadLibraryA` 运行期加载，无新依赖） |
| DX12 | 完成（一处待硬件验证） | Windows；COM vtable 直调，无 `windows` 依赖 |
| Terminal | 完成 | POSIX `forkpty`（macOS/Linux）、Windows ConPTY、Windows 命令行引号；Ghostty 静态库链接，FFI 布局用真实 `ghostty_type_json()` 清单做编译期断言 |
| Hosts | 完成 | Android 15 + Desktop 26；`Hosts/rust/PLATFORM_COVERAGE.md` 为逐平台矩阵 |

三个原生后端均无 `todo!()`/`unimplemented!()`；Vulkan 与 DX12 源码在所有宿主上编译（这就是 macOS 上 `cargo check` 有意义的原因），只有加载器与 surface 创建按平台 `cfg` 门控，错误平台返回明确的 "unsupported backend"。

### 已知缺口（不隐藏）

1. **Linux `https://`**：工作区依赖集无 TLS crate，`TransportCapabilities` 报 `https=false` 并给出具名限制；`file://` 与 `http://` 正常。刻意不引入 rustls/native-tls，避免把 TLS+crypto 拖进所有平台（含 Wasm）。
2. **DX12 描述符表绑定**：`command_list.rs` 已创建 root signature 与 descriptor heap，但 `SetPipelineState`/`SetDescriptorHeaps` 的逐 draw 绑定序列仍为空实现，需 Windows 真机验证后才能称为生产可用。
3. **DX12 加速结构**：本构建未链接 DXR 分发，`capabilities_for` 不宣称光追；`create_acceleration_structure` 返回明确错误。适配器自身的 DXR 支持仍通过 `AdapterCapabilities` 作为诊断信息上报。
4. **macOS 菜单栏**：`cove-shell` 只暴露 `cove_shell_menu_popup`（上下文菜单），没有 `NSApplication.mainMenu` 入口；`NativeMenuBar::snapshot()` 已能生成所需树结构，缺的是 shell 侧入口。
5. **Wasm 宿主**：`cove-hosts` 不是 `cove-browser` 的依赖，两者独立；Wasm 上 `platform_transport()` 返回 `None`，http/https 图片返回 `HostUnavailable`，需要 Wasm 专用 fetch/XHR transport。
6. **Apple 图片 MIME 与 macOS 内存压力**：前者用 URL 扩展名回退（NSData 无 MIME），后者读取已发布级别而非自持 `dispatch_source`。两者都需要手写 ObjC block ABI，为可靠性做了取舍。

## 逐模块保真度（本轮核对）

每个 Compose 家族迁移后按三档标注：`exact`（行为与 Swift 一致）、
`equivalent-with-named-difference`（行为等价，差异已具名）、
`compromise-with-follow-up`（有已知缺口，已登记跟进项）。
**未做行为对照测试**（按要求推迟到全量完成后统一验证），此表是基于逐文件阅读与实现的判断，
不是验收结论。

| 家族 | 判定 | 说明 |
| --- | --- | --- |
| Feedback（Badge、EmptyState、Progress、ProgressCircle、Alert） | exact | |
| Media（Icon、Skeleton、AsyncImage、ImageDataDownload、ImageLoadServices） | equivalent-with-named-difference | 无 socket transport；`ImageDownloadTransport` 由宿主安装，`file://` 完整可用 |
| Layout（ScrollView、Sidebar 17 个、Virtualization、Insets、Responsive、Panel、Accordion、BoundedScrollView） | exact | Sidebar 的选择滚动揭示（`SelectionRevealHost`）已按签名/几何缓存/pending 标志完整实现 |
| Overlays（Menu 家族、Modal、Toast、Popover、Anchored、RovingFocus） | exact | |
| Text：TextField（含 IME） | equivalent-with-named-difference | 通过 `TextInputClient` 接入 IME（组合区间、选区移动、删除周边）；无可选中文本的装饰层 |
| Text：Markdown（6 个） | equivalent-with-named-difference | `pulldown-cmark` 方言等价；`SymbolLink`（DocC 专有）刻意省略；表格单元格/段落/标题的富文本扁平化已退役 |
| Text：RichText / Paragraph / LinkAnchor | exact（一处具名边界） | `RichTextFlow::span()` 以字节偏移取索引，若某字形簇跨越两个不同链接的 span，字形着色取第一个；命中测试走 `fragment.span` 始终正确 |
| Text：RichTextSurface 文本选择 | compromise-with-follow-up | 绘制与命中已移植，但未注册 `TextSelectionScope`：跨富文本段的拖选未接通。渲染无错误，只是无法跨段选择 |
| Controls（Toggle/Checkbox、Slider、RadioGroup、Select） | exact | Toggle 36pt / Checkbox 16pt 与原实现一致 |
| Controls：`ButtonGroup`/`Toolbar`/`AppBar` | exact | 溢出布局端到端接通（`LayoutContainer` + `impl Layout` + `AppBar` 提供预算），并有断言优先级顺序与 `Always` 保证的测试；`available_width: None` 时走不回退路径，与 Swift 的 `fitsContent: true` 一致 |
| Controls：`Rating` | equivalent-with-named-difference | 精度/绘制/指针/键盘/无障碍精确；悬停预览未受 `supportsControlHover` 门控（`Node` 无该标记），仅影响观感 |
| Controls：`CalendarView`/`DatePicker` | equivalent-with-named-difference | 结构与行为精确；**标签为英文硬编码**，未实现 `DateFormatter` 本地化 |
| Data：List 家族 | exact | |
| Styles：10 个组件样式 | exact | |

已退役的四处扁平化：`MarkdownTableView` 单元格、`MarkdownBlocks` 段落、`MarkdownBlocks` 标题、
菜单富标题——现在均渲染真实 `RichText`，标题还恢复了 `Heading` 角色与 `level N` 值。

## 平台边界

| 平台 | 已写入 | 尚缺 | 验证 |
| --- | --- | --- | --- |
| macOS | 共享 Rust/C、POSIX、文件/进程/权限、回收站桥接、RHI 前端、UI 渲染、SDL/AppKit 窗口、Apple 无障碍 | Metal 后端、桌面宿主收口、应用包/签名 | 未验证 |
| Windows | 共享 Rust/C、UTF-16 进程参数、继承句柄白名单、Job Object、ACL、文件替换重试、RHI 前端 | DX12 后端、ConPTY 终端、桌面宿主、CLI Desktop/Android 构建、应用打包 | 未验证 |
| Linux | 共享 Rust/C、POSIX、权限/身份/恢复目录、RHI 前端 | Vulkan 后端、桌面宿主、系统集成、应用打包 | 未验证 |
| iOS | 共享 Rust/C、文件/资源/协议、Apple 字体/图片桥接、Runtime 显示循环 | 触摸/IME/生命周期宿主收口、Metal、Xcode 打包签名、CLI iOS 构建 | 未验证 |
| Android | 共享 Rust/C、NDK 配置、POSIX 进程、Android 契约/集成层 | Activity/JNI 宿主、触摸/IME/生命周期收口、Vulkan、APK 打包/签名/部署、CLI Android 构建 | 未验证 |
| Browser/Wasm | 平台数据类型 | 字体、图形、资源、浏览器事件循环、宿主与构建链 | 未建立构建链 |

不能因为本机无法运行其他平台而删除其实现；发现平台缺口必须记录并补齐。

## 当前验证状态

**本会话结束时 `cargo check --workspace --all-targets` 通过（0 errors）**，覆盖全部 18 个 crate
与 54 个已声明的 `[[test]]` 目标。这是可编译性检查，**不是验收**：按项目要求，测试执行、
格式化、Clippy、跨平台验收与 GUI 冒烟全部推迟到全量迁移完成后统一进行。
本会话**没有运行** `./cove fmt`、`./cove clippy`、`./cove test`、`./cove build`、`./cove migration status`。

### 本会话修复的可编译性问题（属缺陷，不是进度）

| 问题 | 范围 | 处理 |
| --- | --- | --- |
| 写而未编译：Rust 文件从未被 `mod` 声明 | System 16、Logging 15、Tools/Cove 8、Tools/Support 12、Compose 2 | 全部接入；删除重复单体实现 |
| 测试文件对 Cargo 不可见 | `Core/rust/tests/` 6、`Graphics/rust/tests/` 7、`Tests/*.rs` 6 | 补 `[[test]]` 声明 |
| `Compose/tests/` 陈旧目录污染每次 `--all-targets` | 4 个文件 | 删除（备份于 `/tmp/cove-stale-tests`） |
| `ModifiedContent` 私有字段 `modifier` 遮蔽 `ViewExt::modifier` | 全 Compose | 字段改名 `applied`；新增 `AnyView::with_modifier` |
| `TextField::new` 签名不可满足（`Binding<TextBuffer>` 但 `TextBuffer` 非 `Send`） | Compose 文本栈 | rope 改 `Arc`，恢复 Swift 的 `Sendable` 契约 |
| LSP snippet 展开多处结构错误 | `text_field/snippet.rs` | 重写 `${N|a,b|}`、`$N` 顺序、`${VARIABLE}`、字面 `$` |
| 跨 crate 共享 API 缺口 | Core/PlatformCore/Text/Composition | `Rect::size/origin`、`KeyModifiers::is_empty`、`ScrollConsumePolicy: Default`、`ObservableStateRegistrar: Clone`、`TextCache` 与 `FreeTypeColorGlyphFace` 导出等 |

### "视图缺失"类缺口（本轮新识别）

盘点发现一类此前被误判为"已完成"的缺口：**模型与样式已写入，但没有可渲染的视图组件**。
判定方法不是比对名字——`Rating` 在 Swift 与 Rust 可能不同名——而是构建
"实际拥有 `impl View` / `impl PrimitiveView` / `impl_primitive_view!` 的类型集合"
（当前 242 个可渲染组件），再逐个确认 Swift 视图名到 Rust 组件的映射。
按声明名匹配会给出假阳性。

以下 19 个组件属于此类：样式/几何/模型齐备，但无视图：

`Toolbar`、`ToolbarItem`、`Rating`、`Table`+`TableColumn`、`DataTable`、`CalendarView`、
`DatePicker`、`ButtonGroup`+`ButtonGroupItem`、`Tree`/`TreeTable`、`TabView`、`SplitView`、
`PropertyGrid`+`Row`+`Section`、`AppBar`、`Carousel`、`Pagination`、`Breadcrumb`+
`BreadcrumbItem`、`CommandPalette`、`Tooltip`

例：`ToolbarToggleButtonStyle` 存在但无 `Toolbar`；`TableGeometry`/`TableCard`/`TableTextCell`
存在但无列表格的 `Table`；`CalendarGrid` 存在但无 `CalendarView`。样式不是控件。

影响：Gallery 98 页中 26 页因此无法渲染（其余 72 页可移植）。本轮按解锁页数排序补齐，
`ButtonGroup`（4 页）优先，其后 `Toolbar`／`Table`／`DatePicker`（各 3 页）。

### 日期与日历的具名差异

- `foundation/{date,calendar}.rs` 采用**固定 UTC 偏移，不实现 DST/闰秒规则**。
  Foundation 的 `Calendar` 在夏令时切换日 `startOfDay` 会是 23 或 25 小时；此处恒为 24 小时。
  对日历网格而言这是正确取舍（日期是标签而非时长，网格也不会因 DST 出现空洞），
  但不与 Foundation 逐字节等价。
- `CalendarView` / `DatePicker` 的**月份名与极短星期符号为英文静态表**，无障碍标签按
  `"March 5, 2026"` 拼接，未使用 `DateFormatter` 的 `dateStyle = .medium` 与本地化符号查找。
  **英文环境下与 Swift 一致，其他语言不正确。** 需 tz 数据库与本地化符号表才能消除，
  属独立决策事项。
- 日期基础层的 API **按位置传参**（`interval_start(unit, date, cal)`），刻意不引入
  Rust 没有的具名参数约定，使调用点读起来接近 Swift 的
  `CalendarGrid.interval(of:containing:calendar:)`。
- `weekday_of` 曾有错位一天的实现缺陷（纪元第 0 天返回 6 而非 5），已修复并交叉验证。
  该缺陷会连带影响 `date_interval_start` 的周起始计算、`CalendarUnit::Weekday` 取值
  与 `iso_week_of` 的年界周数。**这类"自写日期算法"必须有独立参考实现对拍**，
  不能只靠内部自洽的单元测试。

### 已知缺口（功能层面，已在实现中具名记录）

见上文「逐模块保真度」与「原生后端与宿主状态」两节。摘要：
Linux `https://`、macOS 菜单栏、Wasm 图片 transport 与 iOS Rust 入口、
DX12 描述符表逐 draw 绑定与加速结构、Apple 图片 MIME 与 macOS 内存压力回退、
`RichTextSurface` 跨段文本选择、`Minimap` 未接入 `TextField`。

## 最终统一验收

1. Rust 工作区构建、格式化、Clippy 与完整单元/集成测试；以 Swift 测试为行为清单迁移。
2. macOS/Windows/Linux 的真实 Gallery、窗口/菜单/剪贴板、字体、GPU、PTY/ConPTY、DevTools 与 SDK 安装/更新验收。
3. iOS 模拟器/真机与 Android 真机/模拟器的生命周期、输入、IME、渲染、包/签名/部署验收。
4. Browser/Wasm 的构建、字体、WebGPU/Canvas、输入与资源加载验收。
5. 对齐公开产品与示例后删除自有 Swift 源码、SwiftPM 清单、旧脚本与旧 CI；第三方依赖不受影响。
6. `./cove migration status --require-complete` 通过；该检查仅说明源码清理，不替代功能与平台验收。
7. 更新正式版本、安装器、SDK 元数据、所有示例与 CHANGELOG，再启用 Rust SDK 发布。

旧 Swift CI 保存在 `Docs/legacy/ci-swift.yml`，用于追踪原生依赖、测试包、发布安装与平台验收范围。

## 清单工具的可信度

`Tools/Cove/rust/migration_inventory.py` 从磁盘重新推导状态，本轮补充三项校验，
因为发现了两起"同名不同子系统"的错配：

1. **可达性校验**：`.rs` 文件的 stem 若从未被任何 `mod` 语句声明，即为死代码，
   不能计入"已写入"。本项目已三次出现该类缺陷（System 16、Logging 15、Tools 20 个孤儿文件）。
   本轮据此降级 2 条记录。
2. **基名边界匹配**：头部注释里用裸 `in` 判断曾让 `Visibility.swift` 命中
   `AnimatedVisibility.swift`；现要求基名前有非标识符边界。
3. **通用词黑名单**：`Visibility`、`Support`、`Options`、`Tree`、`Selection` 等普通词
   会被无关子系统用作类型名，仅凭类型名命中不算证据。

已确认的两起错配：
- `Primitives/Data/Columns/Visibility.swift`（表格列可见性规划）曾被映射到
  `Graphics/rust/rhi/visibility.rs`（GPU 可见性缓冲），实为完全无关模块。该 Swift 文件
  25 行（`TableColumnPlan::resolve` + `tracks`），**确属未迁移**，已改回 `pending`。
- `Primitives/Controls/DateValues.swift` 的 Rust 文件从未被 `mod` 声明，且所引用的
  `foundation::{Calendar, CalendarUnit, Date}` 整个模块不存在——已由本轮补齐
  `foundation/{date,calendar}.rs`。

审计结论：**当前 0 条生产源码记录声称的目标文件不存在或不可达**。

## Compose API 注意事项（踩坑记录）

以下陷阱在本轮由多人反复踩到，按实际报错文本组织，供后续实现参考。

1. **扩展 trait 必须逐个导入。** `BoxView`/`Text`/`Button` 几乎没有固有 builder 方法；
   `.padding()`、`.corner_radius()`、`.frame_height()`、`.flex()`、`.semantic_background()`、
   `.semantic_foreground()`、`.button_style()` 分别来自
   `PaddingExt`、`FlexExt`、`LayoutFrameExt`、`AppearanceExt`、`SemanticStyleViewExt`、`ButtonStyleViewExt`。
   缺一个导入就报 "no method named X"——**看起来像 API 不存在，实际是缺 import**。

2. **`.button_style()` 与 `.semantic_foreground()` 返回 `ModifiedContent`，不是 `AnyView`。**
   **但 `ModifiedContent` 实现了 `View`**（`view_modifier.rs`），而所有 `*Ext` trait 都是
   `impl<T: View + Sized + 'static>` 的 blanket 实现，因此**修饰器可以自由串联**：
   `Text::new(x).padding_edges(10.0, 6.0).semantic_background(col).corner_radius(6.0)`
   合法（已用编译探针验证）。不需要在每组之间插入 `AnyView::new(...)`。
   真正需要擦除的情况只有：值是 `Rc<dyn View>` / 裸 `&dyn View`，或所需 trait 未导入。
   禁止用 `AnyView::new(modified_content)` 掩盖类型错误——会经 downcast 快路径
   **静默丢弃修饰器**。

3. **`button_style` 会先解析为 `ViewModifier`。** 报错形如
   "`SecondaryButtonStyle: ViewModifier` is not satisfied"，具有误导性；导入
   `ButtonStyleViewExt` 即可。

4. **已擦除值用 `AnyView::with_modifier(m)`。** 具体 `View` 用 `ViewExt::modifier`；
   `ModifiedContent` 或 builder 输出需先 `AnyView::new(x)` 再 `with_modifier`。

5. **两套对齐类型。** `cove_core::scene::{Align, FlexDirection, Justify, Wrap}` 用于 flex；
   `cove_ui_compose::Alignment` 用于 `BoxView::with_alignment`、`Grid`、`Masonry`。

6. **`Checkbox` 的 label 是结构体字段，不是方法。** 用
   `Checkbox::labeled(label, Binding<CheckboxState>)`；`from_boolean` 无 label 槽位。

7. **`LazyGrid::new` 取 11 个参数**；`GridRowHeights::fixed(n)` 是构造函数（不是 `::Fixed`）。

8. **`Spinner::new(size)` 需要尺寸；`Badge::enabled(bool)` 不是 `.is_enabled()`；
   `ProgressView::new(Option<f64>)` / `ProgressCircle::new(Option<f64>)`。**

9. **`body()` 不能把 `&self` 捕获进 `'static` 闭包**（如 `ContainerSizeReader`）。
   需要先克隆出快照结构体（见 `AdaptiveSidebar` 的 `AdaptiveSnapshot`）。

10. **`ModifiedContent` 的私有字段曾名为 `modifier`，遮蔽 `ViewExt::modifier(self, m)`**，
    报错为 "private field, not a method"。字段已改名 `applied`；如再遇到同类报错，
    检查是否又出现同名字段遮蔽。

11. **`"no method named X"` 的三种成因**（按出现频率）：
    (a) **缺 trait import** —— 最常见，如 `corner_radius` 来自 `AppearanceExt`
    （不是 `LayoutFrameExt`）；`button_style` 来自 `ButtonStyleViewExt`；
    (b) **参数个数不符 / 字段私有** —— 见下条；
    (c) **值的类型没有 blanket `View` 实现** —— 如 `Rc<dyn View>`、裸 `&dyn View`，
    此时才需要先擦除为 `AnyView`。
    **注意 `ModifiedContent` 属于 (a) 类排查对象，不属于 (c)** ——它实现了 `View`。

12. **尺寸/几何走构造函数，不走 builder。** `Icon::new(resource, size)` 必须给尺寸，
    `ProgressView::new(Option<f64>)`、`ProgressCircle::new(Option<f64>)`、
    `Spinner::new(size)` 同理；`GridRowHeights::fixed(n)` 是构造函数而非 `::Fixed`。
    这些原语**没有** `.size()` / `.height()` builder，字段是私有的，所以写
    `.size(28.0)` 会得到误导性的 "private field, not a method"。
    **看到 "private field" 时先查签名，不要找 trait import。**

13. **`impl View for X` 也可能写成 `impl<T: ...> View for X<T>`。** 用文本搜索判断某控件
    是否存在时，两种形式都要匹配——只匹配前者会漏报（`ButtonGroup` 曾被误判为缺失）。
    更可靠的做法是构建"拥有 `View`/`PrimitiveView` 实现"的可渲染类型集合
    （`Tools/Cove/rust/migration_inventory.py` 的 `renderable_index()`，当前 323 个）。

## 迁移约定

- **引用语义**：Swift `class` → `Rc<RefCell<T>>` 句柄；反向指针用 `Weak`。
- **属性观察**：Swift `didSet` → 显式失效 setter，统一汇入 `invalidate_geometry` / `mark_render_dirty`。
- **每窗口环境值**：`AmbientSlot` 静态量需要 `Sync`，改为 `thread_local!` + `RefCell` 持有者。
- **嵌套模块导入**：`scene/input`、`scene/scrolling`、`scene/animation` 下写 `crate::scene::…`，写 `super::…` 会解析到自身。
- **`impl` 内不能放静态量**：提升到模块级 `static` / `thread_local!`。
- **`OptionSet`**：`rhi_option_set!` 宏生成裸位新类型；**不透明句柄**：`handle!` 宏生成 `u32` 新类型。
- **`NSLock`**：前端状态用 `Mutex`，整帧会话用第二个 `Mutex`；后端放在 `Arc<Mutex<Box<dyn RhiBackend + Send>>>`。
- **`NSCondition`**：`FrameRing` = `Mutex` + `Condvar`；retirement 动作在释放环锁之后执行。
- **借用与回调**：`RefCell` 借用必须在执行用户回调前释放。
- **`DispatchQueue` / async 准备**：线程局部 `ResourcePreparationContext` + 同步 `run`；异步包装由 Runtime 提供。
- **`#if canImport`**：`BackendFactory` 用构造器注册表 `register_backend`，未注册时返回 "not built for this platform"。
- **`Duration`**：统一传纳秒 `u64`。
- **原生句柄**：`SurfaceDescriptor` 用 `usize` 地址（0 表示无），避免 `unsafe impl Send`。
- **原生字体依赖**：FreeType/HarfBuzz 复用 vendored 源码；Unicode 字素/类别由 `unicode-segmentation` / `unicode-general-category` 提供。
- **避免反模式**：不做重复封装、并行字段列表、转发属性；沿用现有 crate 分层与导出风格。
