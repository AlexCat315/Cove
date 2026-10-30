# Rust 迁移状态

目标语言为 Rust 2024（`rust-version = 1.90`），性能关键内核可保留 C。当前工作区版本 `0.3.0-dev`。

最后更新：2026-10-30。

## 记录口径与当前工作

按最新要求：**实施过程中不运行测试，全部文件转换结束后再统一测试。** 本次继续迁移不会运行 `cargo test`、编译测试程序、跨平台验收或 GUI 冒烟测试。历史编译记录只说明当时的工作区状态，不能作为后续修改或其他平台的验证结论。

文中“已写入”只表示有对应 Rust/C 实现，**不表示通过编译、行为覆盖或平台验收**。Swift 文件暂留作逐文件对照，因此其数量是“仍保留的源文件”，不能直接作为尚未翻译文件的数量。旧表格中的“完成”按这一口径理解，最终完成还需逐文件对照和统一验收。

逐文件清单保存在 [rust-migration-files.json](rust-migration-files.json)，记录原始路径、源码 SHA-256、对应 Rust/C 路径及状态。状态含义：`pending` 尚未建立实现映射；`partial` 对应实现仍有缺项；`written_unverified` 已写入、未验证；`manifest_written` 已建立 Cargo 清单。清单仍在补充，不用文件名对应代替行为审阅。

当前顺序：迁移记录 → Text/Imaging 生产源码（已写入）→ Shell/Runtime 与 Graphics UI 渲染（已写入，原生 GPU 后端继续迁移）→ Compose 基础层与上层控件 → Terminal、Hosts、Browser、Tools、DevTools、Sources、Examples → 测试源码迁移 → 全部实现完成后统一验证。所有原有平台都保留在迁移范围内。

## 总量

| 指标 | 数值 |
| --- | --- |
| Swift 源文件（含测试、模板；逐文件清单快照） | 1,438 项 |
| Swift 源码行数（同一快照） | 183,470 |
| 逐文件清单：已写入、未验证 | 311 项 |
| 逐文件清单：有缺项 | 1 项 |
| 逐文件清单：待迁移或待建立映射 | 1,107 项 |
| Cargo 清单映射 | 19 项 |
| Rust 文件与行数（已跟踪文件快照，含测试源码） | 468 个 / 66,138 行 |
| 已接入 Cargo 工作区的能力 | Core、PlatformCore、System、Logging、Graphics、Text、Imaging、Shell、Runtime、Compose、Terminal、Hosts、DevTools、Sources、Tools、Browser、Examples |

清单状态只表示目标路径已存在，不等于行为完整或平台可用。Compose 的映射覆盖仍在扩展；剩余项包含大量上层 UI、平台宿主、工具和测试源码。

## 逐模块进度快照

下表统计生产源码目录，不含测试源码；Rust 文件数来自磁盘快照。逐文件映射状态见 JSON 清单。

| 模块 | Swift 文件 | Swift 行 | Rust 文件 | Rust 行 | 状态 |
| --- | ---: | ---: | ---: | ---: | --- |
| Core | 97 | 12,301 | 93 | 12,807 | 对应实现已写入；本次补齐局部失效桥；未验证 |
| PlatformCore | 6 | 541 | 2 | 425 | 实现已写入；未验证 |
| System | 17 | 1,004 | 21 | 2,956 | 文件映射和实现待逐项复核 |
| Logging | 15 | 705 | 16 | 2,701 | 文件映射和异步行为待复核 |
| Graphics | 107 | 14,534 | 44 | 10,644 | RHI 前端与 UI 渲染已写入；原生 GPU 后端待迁移 |
| Text | 14 | 2,576 | 16 | 1,002 | 生产源码已写入；未验证 |
| Imaging | 9 | 697 | 11 | 415 | 生产源码已写入；未验证 |
| Compose | 383 | 49,814 | 202 | 24,619 | Composition 基础层、静态文字绘制、部分控件与宠物场景已写入；大量控件待迁移 |
| Runtime | 11 | 1,485 | 12 | 328 | 多窗口/调度/宿主桥接已写入；未验证 |
| Shell | 13 | 2,529 | 14 | 483 | SDL 与 Apple 桥接已写入；未验证 |
| Terminal | 29 | 3,088 | 1 | 6 | 待迁移 |
| Browser | 1 | 400 | 1 | 2 | 待迁移 |
| Tools | 73 | 3,951 | — | — | 待迁移 |
| DevTools | 32 | 3,069 | 1 | 8 | 待迁移 |

## 已写入的 crate

### `cove-core`（对应实现已写入，待统一验证）

`CoveUICore` 与 `CoveUIScene` 的全部内容：几何/颜色/仿射变换、共享绘制缓冲区与 C 内核、位图与 mipmap、状态订阅与读取追踪、场景树与布局引擎（含 Flexbox 与自定义布局桥接）、渲染树镜像、无障碍树、命中测试与输入（指针捕获、焦点链、文本输入、滚轮手势、事件分发）、滚动（物理、回弹、弹簧、速度追踪、揭示）、动画（插值、曲线、可动画属性、写入）、重组器、四类渲染器（`NodeRenderer`、`LayerAwareNodeRenderer`、`RenderCulling`、`SurfacePainting`）、`Recomposer`、`Exports`。

### `cove-graphics`（NativeRHI 前端）

`Graphics/rust/rhi/` 下的模块：

| 文件 | 对应 Swift |
| --- | --- |
| `error.rs` `handles.rs` `types.rs` `module.rs` | `RHIError` `Device/Handles` `RHITypes` `NativeRHI` |
| `descriptors.rs` `commands.rs` | `Device/Descriptors`、`Resources/MeshPipeline`、`Resources/TextureBufferUpload`、`Device/Commands` |
| `capabilities.rs` `visibility.rs` `specialization.rs` `presentation.rs` `pipeline_interface.rs` `queue.rs` `resources.rs` | `Device/Capabilities`、`Resources/*`、`Device/Queue` |
| `state_tracker.rs` `access_tracker.rs` `validation.rs` `caches.rs` `shader_artifact.rs` `pipeline_validation.rs` | `Validation/*` |
| `submission.rs` `submission_planner.rs` `pipeline_cache.rs` `resource_preparation.rs` | `Submission/*`、`Device/PipelineCache`、`Device/ResourcePreparation` |
| `device.rs` `resource_ownership.rs` `backend.rs` `backend_factory.rs` | `Device/Device`、`Resources/{Swapchain,Texture}Resource`、`RHIBackend`、`BackendFactory` |
| `testing.rs` | 测试用空后端（Swift 无对应文件） |

`Graphics/rust/lib.rs` 同时导出 `rhi` 和 `ui`；平台后端仍在迁移范围内。

### 其余基础 crate

`cove-platform`（键盘/IME/指针/滚动/游戏手柄/窗口事件/菜单/资源目录）、`cove-system`（权限与身份、锁、同卷原子替换、恢复目录、回收站、进程环境、双流读取、限额/超时/取消、POSIX 进程组与 Windows Job Object）、`cove-logging`（级别过滤、递归脱敏、UTF-8 限长、有界历史、JSONL 轮转）、`cove-devtools-protocol`（树/布局/状态/时间线协议、订阅会话、状态差异、输入录制）、`cove-cli`（build/run/test/check/fmt/clippy/clean、doctor/setup、迁移盘点）。


### `cove-text` 与 `cove-imaging`（本次继续转换）

Text 的 FontFace、FontAtlas、FontProvider、FontCollection、TextStyle、TextShaper、TextCache、TextEnvironment、DrawList+Text、彩色字形源和图集都有 Rust 对应文件。FreeType/HarfBuzz 调用、位图复制/缩放为 C；macOS/iOS 的 CoreText 功能使用 Objective-C/C 桥接，没有新增 Swift。字体数据和原生句柄绑定生命周期，图集缓存按字体/字号/密度区分。

Imaging 的 ImageDecoder、Animation、Orientation、Rasterization、Progressive、TextureImage、BundleImageResource、ImageAssetRegistry 和 Exports 已写入。复用原有 stb_image/lunasvg/libwebp 解码，方向重排使用 C，macOS/iOS 的 PDF/ImageIO 路径使用 Objective-C/C。加入动画帧预算、源像素预算、缓存 LRU、共享 mipmap 与全进程纹理 ID。其他平台保留可移植解码路径；原有 Apple 专用格式能力边界没有冒充全平台支持。

以上均未编译和测试；Cargo.lock、原生库交叉链接、宿主集成及统一验收尚未完成。Browser/Wasm 的 C 运行时与宿主接入仍待实现，不能把桌面原生桥接记作浏览器完成。

### `cove-shell`（继续转换中）

PlatformShell、SDL3Shell、ApplicationEntry、Cursor、SDL3Clipboard、日志接入、NativeMenuPopover、显示刷新 inbox/Apple 驱动、AppKitInput 已写入 Rust/C/Objective-C 对应实现。SDL 事件转换保留窗口路由、关闭拦截、IME、触摸、游戏手柄、滚轮单位/阶段，文件对话框结果通过队列回到应用线程。窗口关闭后使保留的句柄失效，防止访问已销毁原生窗口。

macOS/iOS 使用原生 CAMetalLayer；Windows、X11、Wayland 和 Android 原生窗口引用已实现。AppKit 菜单仍遵循原版 macOS 能力边界；其他平台的 Compose 菜单呈现器待上层迁移。显示刷新在无原生回调的 SDL 平台上显式报告不可用，Runtime 必须保留原有截止时间回退。

Shell 代码尚未编译/测试；SDL 链接、各平台宿主生命周期、菜单与窗口集成仍在最终对照范围内。接下来继续迁移上层 Compose 控件及原生图形后端。

## 本次继续迁移的最新快照

以下统计只计算自有 Rust 源码，含新写入文件与构建脚本，不代表功能覆盖率：

| 范围 | Rust 文件 | Rust 行数 | 当前边界 |
| --- | --- | --- | --- |
| Text | 16 | 1,002 | 生产源码已写入；未验证 |
| Imaging | 11 | 415 | 生产源码已写入；未验证 |
| Shell | 14 | 483 | 窗口/输入/原生桥接已写入；未验证 |
| Graphics | 44 | 10,644 | 前端与 UI 层已写入；原生 GPU 后端未完成 |
| Runtime | 12 | 328 | 会话/帧调度/Apple 无障碍桥接已写入；未验证 |
| Compose（UI + Composition + 宏） | 202 | 24,619 | 基础层、静态文字绘制、部分控件和宠物场景已写入；上层控件待迁移 |

Graphics 新增 UI 视口、几何缓存验证、着色器资源载入、纹理所有权/弱驻留、带边缘填充的图片图集、失败录制重试上传、背景模糊、揭幕及有限预算的分条 MSAA。几何扫描、UV 重映射和图集填充使用 C。RHI 的帧回收改为 GPU 回调只更新完成状态，设备 API 线程执行销毁回调，并补充空闲等待的通知。

Runtime 新增多窗口会话、原生/注入输入仲裁、对话框回调回到主线程、事件驱动/显示刷新/固定帧率、绝对截止时间回退、光标刷新、隐藏窗口释放通知、iOS CADisplayLink 生命周期，以及 macOS/iOS 无障碍树和动作桥接。同步补回 Core 的 WindowRevealContext 接口。新实现全部尚未验证。

Compose/Composition 的 ViewGraph、视图包装、修饰器、组合作用域、布局树和状态订阅已写入。新增 Rust 属性宏 `#[track_state_reads(field, ...)]`，在视图 `body` 中把 `self.state.get().field` 改写为字段读取追踪，并将 `DynamicField` 注册接入失效队列；普通视图仍通过本地订阅失效。另写入文件存储、可观察值包装、字符串目录解析、离屏快照和宠物运动、精灵绘制、锚点及场景组合。为媒体、头像、图表、文本辅助、布局与主题中已有的 Rust 文件补上模块入口。纯文字和分隔线已写入基础 Rust 实现；选择交互、完整截断提示及部分样式细节仍缺，因此该源文件标记为 `partial`。Compose 清单中仍有 265 项待映射和 1 项有缺项，其中包含待迁移实现和测试。全量项目尚未完成，Swift 源码继续作为行为对照保留。

## 迁移约定

这些是移植过程中确定的规则，后续文件应保持一致。

- **引用语义**：Swift `class` → `Rc<RefCell<T>>` 句柄；反向指针用 `Weak`。
- **属性观察**：Swift `didSet` → 显式失效 setter，统一汇入 `invalidate_geometry` / `mark_render_dirty`。
- **每窗口环境值**：`AmbientSlot` 静态量需要 `Sync`，改为 `thread_local!` + `RefCell` 持有者。
- **嵌套模块导入**：`scene/input`、`scene/scrolling`、`scene/animation` 下必须写 `crate::scene::…`，写 `super::…` 会解析到自身。
- **`impl` 内不能放静态量**：提升到模块级 `static` / `thread_local!`。
- **`OptionSet`**：`rhi_option_set!` 宏生成裸位新类型；**不透明句柄**：`handle!` 宏生成 `u32` 新类型。
- **`NSLock`**：前端状态用 `Mutex`，整帧会话用第二个 `Mutex`；后端放在 `Arc<Mutex<Box<dyn RhiBackend + Send>>>` 中，让 GPU 完成线程上的延迟销毁回调可以取得它。
- **`NSCondition`**：`FrameRing` = `Mutex` + `Condvar`；retirement 动作在释放环锁之后执行，避免销毁回调与环锁互锁。
- **借用与回调**：`RefCell` 借用必须在执行用户回调前释放（dispatcher、recomposer、animator、`FocusChain::focus`）。
- **`DispatchQueue` / async 准备**：改为线程局部 `ResourcePreparationContext` + 同步 `run`；异步包装留给尚未迁移的 Runtime crate。
- **`#if canImport`**：`BackendFactory` 改为构造器注册表（`register_backend`）。未注册时返回 "not built for this platform"，不引用不存在的符号。
- **`Duration`**：统一传纳秒 `u64`。
- **原生句柄**：`SurfaceDescriptor` 用 `usize` 地址（0 表示无），避免 `unsafe impl Send`。
- **原生字体依赖**：FreeType/HarfBuzz 复用 vendored 源码；Unicode 字素和类别由 `unicode-segmentation` / `unicode-general-category` 提供。

## 尚未迁移

- **Graphics 平台后端**：`Metal`（15 文件 / 2,712 行）、`Vulkan`（31 / 6,010）、`DX12`（8 / 500）；`CoveUIGraphics` 的 12 个生产文件已写入 `Graphics/rust/ui/`，待接入实际 GPU 后统一验证。
- **Text**：生产源码已逐文件写入。剩余项为测试迁移、原生依赖跨平台构建接入与最终行为对照；上层 Compose 文本选择仍待迁移。
- **Imaging**：生产源码已逐文件写入。剩余项为图形纹理上传接入与最终行为对照。
- **Compose**：视图图、控件、主题、组合与重组（383 文件，最大单块）。
- **Runtime / Shell / Terminal**：应用运行时、SDL 窗口与事件循环、Ghostty 引擎与 PTY/ConPTY。
- **Hosts / Browser**：桌面、Android、iOS 宿主与 WebGPU/Canvas 宿主。
- **Tools / DevTools / Sources / Examples**：CLI 之外的工具、Inspection/Agent/Server/Inspector、SDK 安装器、示例应用。
- **Tests**：325 文件，作为行为清单迁移。

Swift 源码没有被删除，也没有用空 Rust 模块替代。新 CLI 不会调用 Swift 编译器；对尚无 `Cargo.toml` 的目录会直接说明未迁移。

## 平台边界

| 平台 | 已写入 | 尚缺 | 验证 |
| --- | --- | --- | --- |
| macOS | 共享 Rust/C、POSIX、权限/锁/身份、回收站桥接、RHI 前端 | Metal 后端、原生窗口、宿主、应用包与签名 | 最新实现未验证 |
| Windows | 共享 Rust/C、UTF-16 进程参数、继承句柄白名单、Job Object、ACL、文件替换重试、RHI 前端 | DX12 后端、原生窗口、ConPTY UI、应用打包 | 最新实现未验证 |
| Linux | 共享 Rust/C、POSIX、权限/锁/身份、恢复目录、RHI 前端 | Vulkan 后端、原生窗口、系统集成、应用打包 | 最新实现未验证 |
| iOS | 共享 Rust/C、文件/资源/协议；进程 API 由系统能力决定 | 触摸/IME/生命周期宿主、Metal、Xcode 打包签名 | 最新实现未验证 |
| Android | 共享 Rust/C、NDK 配置、文件/资源/协议、POSIX 进程调用 | Activity/JNI、触摸/IME/生命周期、Vulkan、APK 签名部署 | 最新实现未验证 |
| Browser/Wasm | 平台数据类型 | 字体、图形、资源、浏览器事件循环与宿主 | 未建立构建链 |

不能因为本机无法运行其他平台而删除其实现。

## 当前验证状态

以下为先前文档保留的历史记录，本次继续迁移没有重新运行；它不代表所有目标平台已经检查，也不能保证后续实现的编译或行为正确：

```
cargo check --workspace --tests   # 通过（仅剩若干 dead_code / deprecated 警告）
```

尚未运行 `cargo test`、`cargo fmt --check`、`cargo clippy`。

后续统一验证放在全量翻译完成之后，实施阶段不通过分批测试扩大这里的通过范围。

## 最终统一验收

以下要求适用于全部实现结束之后，不是当前已通过的检查。

1. Rust 工作区构建、格式化、Clippy 与完整单元/集成测试；以原有 Swift 测试为行为清单迁移，不用空测试或只验证实现细节的测试代替。
2. macOS/Windows/Linux 的真实 Gallery、窗口/菜单/剪贴板、字体、GPU、PTY/ConPTY、DevTools 与 SDK 安装/更新验收。
3. iOS 模拟器/真机与 Android 真机/模拟器的生命周期、输入、IME、渲染、包/签名/部署验收。
4. Browser/Wasm 的构建、字体、WebGPU/Canvas、输入与资源加载验收。
5. 对齐所有公开产品与示例后删除自有 Swift 源码、SwiftPM 清单、旧脚本与旧 CI；保留第三方依赖原有语言不影响 Rust 主项目目标。
6. `./cove migration status --require-complete` 通过；该检查仅说明源码清理，不替代功能和平台验收。
7. 更新正式版本、安装器、SDK 元数据、所有示例与 CHANGELOG，再启用 Rust SDK 发布。

旧 Swift CI 保存在 [legacy/ci-swift.yml](legacy/ci-swift.yml)，用于追踪原生依赖、测试包、发布安装与平台验收范围。
