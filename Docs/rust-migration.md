# Rust 迁移状态

目标语言为 Rust 2024（`rust-version = 1.90`），性能关键内核可保留 C。当前工作区版本 `0.3.0-dev`。

最后更新：2026-10-29。

## 记录口径与当前工作

按最新要求：**实施过程中不运行测试，全部文件转换结束后再统一测试。** 本次继续迁移不会运行 `cargo test`、编译测试程序、跨平台验收或 GUI 冒烟测试。历史编译记录只说明当时的工作区状态，不能作为后续修改或其他平台的验证结论。

文中“已写入”只表示有对应 Rust/C 实现，**不表示通过编译、行为覆盖或平台验收**。Swift 文件暂留作逐文件对照，因此其数量是“仍保留的源文件”，不能直接作为尚未翻译文件的数量。旧表格中的“完成”按这一口径理解，最终完成还需逐文件对照和统一验收。

逐文件清单保存在 [rust-migration-files.json](rust-migration-files.json)，记录原始路径、源码 SHA-256、对应 Rust/C 路径及状态。状态含义：`pending` 尚未建立实现映射；`partial` 对应实现仍有缺项；`written_unverified` 已写入、未验证；`manifest_written` 已建立 Cargo 清单。清单仍在补充，不用文件名对应代替行为审阅。

当前顺序：迁移记录 → Text/Imaging 生产源码（已写入）→ Shell/Runtime 与 Graphics 各原生后端、UI 渲染 → Compose/各宿主、工具和示例 → 全部测试迁移与最终统一验证。所有原有平台都保留在迁移范围内。

## 总量

| 指标 | 数值 |
| --- | --- |
| 仍保留的 Swift 文件（历史快照） | 1,436 |
| 仍保留的 Swift 行数（历史快照） | 183,294 |
| 已写入 Rust 文件（持续变化，见逐文件清单） | 159 |
| 已写入 Rust 行数 | 25,686 |
| 已迁移 crate | `cove-core`、`cove-graphics`（RHI 前端）、`cove-platform`、`cove-system`、`cove-logging`、`cove-devtools-protocol`、`cove-cli`、`cove-ui`、`cove-rust-counter` |

## 逐模块进度

`rust` 列统计 `*/rust/**/*.rs`，不含构建脚本。

| 模块 | Swift 文件 | Swift 行 | Rust 文件 | Rust 行 | 状态 |
| --- | --- | --- | --- | --- | --- |
| Core | 104 | 12,548 | 92 | 12,696 | 已写入，未统一验证 |
| PlatformCore | 8 | 581 | 2 | 425 | 已写入，未统一验证 |
| System | 24 | 1,452 | 5 | 1,154 | 已写入，待逐文件行为对照 |
| Logging | 19 | 906 | 1 | 511 | 基础管线已写入；上下文与异步行为待对照 |
| Graphics | 144 | 18,365 | 30 | 9,771 | RHI 前端完成；三个平台后端与 `CoveUIGraphics` 未开始 |
| Text | 17 | 2,803 | 16 | 1,002 | 生产源码已写入；未编译/测试 |
| Imaging | 10 | 718 | 11 | 415 | 生产源码已写入；未编译/测试 |
| Compose | 416 | 52,590 | — | — | 未开始 |
| Runtime | 12 | 1,501 | — | — | 未开始 |
| Shell | 14 | 2,584 | 14 | 472 | SDL 窗口/输入/对话框与 Apple 桥接已写入；未验证 |
| Terminal | 38 | 4,030 | — | — | 未开始 |
| Hosts | 55 | 4,349 | — | — | 未开始 |
| Browser | 2 | 419 | — | — | 未开始 |
| Sources | 19 | 4,861 | — | — | 未开始 |
| Tools | 88 | 4,800 | — | — | 未开始 |
| DevTools | 57 | 5,910 | — | — | 未开始 |
| Examples | 84 | 9,723 | — | — | 未开始 |
| Tests | 325 | 55,154 | — | — | 未开始 |

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

`Graphics/rust/lib.rs` 目前只导出 `rhi`。

### 其余基础 crate

`cove-platform`（键盘/IME/指针/滚动/游戏手柄/窗口事件/菜单/资源目录）、`cove-system`（权限与身份、锁、同卷原子替换、恢复目录、回收站、进程环境、双流读取、限额/超时/取消、POSIX 进程组与 Windows Job Object）、`cove-logging`（级别过滤、递归脱敏、UTF-8 限长、有界历史、JSONL 轮转）、`cove-devtools-protocol`（树/布局/状态/时间线协议、订阅会话、状态差异、输入录制）、`cove-cli`（build/run/test/check/fmt/clippy/clean、doctor/setup、迁移盘点）。


### `cove-text` 与 `cove-imaging`（本次继续转换）

Text 的 FontFace、FontAtlas、FontProvider、FontCollection、TextStyle、TextShaper、TextCache、TextEnvironment、DrawList+Text、彩色字形源和图集都有 Rust 对应文件。FreeType/HarfBuzz 调用、位图复制/缩放为 C；macOS/iOS 的 CoreText 功能使用 Objective-C/C 桥接，没有新增 Swift。字体数据和原生句柄绑定生命周期，图集缓存按字体/字号/密度区分。

Imaging 的 ImageDecoder、Animation、Orientation、Rasterization、Progressive、TextureImage、BundleImageResource、ImageAssetRegistry 和 Exports 已写入。复用原有 stb_image/lunasvg/libwebp 解码，方向重排使用 C，macOS/iOS 的 PDF/ImageIO 路径使用 Objective-C/C。加入动画帧预算、源像素预算、缓存 LRU、共享 mipmap 与全进程纹理 ID。其他平台保留可移植解码路径；原有 Apple 专用格式能力边界没有冒充全平台支持。

以上均未编译和测试；Cargo.lock、原生库交叉链接、宿主集成及统一验收尚未完成。Browser/Wasm 的 C 运行时与宿主接入仍待实现，不能把桌面原生桥接记作浏览器完成。

### `cove-shell`（继续转换中）

PlatformShell、SDL3Shell、ApplicationEntry、Cursor、SDL3Clipboard、日志接入、NativeMenuPopover、显示刷新 inbox/Apple 驱动、AppKitInput 已写入 Rust/C/Objective-C 对应实现。SDL 事件转换保留窗口路由、关闭拦截、IME、触摸、游戏手柄、滚轮单位/阶段，文件对话框结果通过队列回到应用线程。窗口关闭后使保留的句柄失效，防止访问已销毁原生窗口。

macOS/iOS 使用原生 CAMetalLayer；Windows、X11、Wayland 和 Android 原生窗口引用已实现。AppKit 菜单仍遵循原版 macOS 能力边界；其他平台的 Compose 菜单呈现器待上层迁移。显示刷新在无原生回调的 SDL 平台上显式报告不可用，Runtime 必须保留原有截止时间回退。

Shell 代码尚未编译/测试；SDL 链接、各平台宿主生命周期、菜单与窗口集成仍在最终对照范围内。接下来继续 Graphics 的 UI 纹理、资源驻留与绘制路径。

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

- **Graphics 平台后端**：`Metal`（15 文件 / 2,712 行）、`Vulkan`（31 / 6,010）、`DX12`（8 / 500）；`CoveUIGraphics`（12 / 1,757，含 `NativeUIBackdropPass` 531、`NativeDrawListRenderer` 473、`NativeUITextureStore` 379）。
- **Text**：生产源码已逐文件写入。剩余项为测试迁移、原生依赖跨平台构建接入与最终行为对照；上层 Compose 文本选择仍待迁移。
- **Imaging**：生产源码已逐文件写入。剩余项为图形纹理上传接入与最终行为对照。
- **Compose**：视图图、控件、主题、组合与重组（416 文件，最大单块）。
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
