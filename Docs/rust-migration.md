# Rust 迁移状态

目标语言为 Rust 2024（`rust-version = 1.90`），性能关键内核可保留 C。当前工作区版本 `0.3.0-dev`。

最后更新：2026-10-29。

## 总量

| 指标 | 数值 |
| --- | --- |
| 待迁移 Swift 文件 | 1,436 |
| 待迁移 Swift 行数 | 183,294 |
| 已写入 Rust 文件 | 130 |
| 已写入 Rust 行数 | 25,686 |
| 已迁移 crate | `cove-core`、`cove-graphics`（RHI 前端）、`cove-platform`、`cove-system`、`cove-logging`、`cove-devtools-protocol`、`cove-cli`、`cove-ui`、`cove-rust-counter` |

## 逐模块进度

`rust` 列统计 `*/rust/**/*.rs`，不含构建脚本。

| 模块 | Swift 文件 | Swift 行 | Rust 文件 | Rust 行 | 状态 |
| --- | --- | --- | --- | --- | --- |
| Core | 104 | 12,548 | 92 | 12,696 | 完成 |
| PlatformCore | 8 | 581 | 2 | 425 | 完成 |
| System | 24 | 1,452 | 5 | 1,154 | 完成 |
| Logging | 19 | 906 | 1 | 511 | 完成 |
| Graphics | 144 | 18,365 | 30 | 9,771 | RHI 前端完成；三个平台后端与 `CoveUIGraphics` 未开始 |
| Text | 17 | 2,803 | — | — | 未开始 |
| Imaging | 10 | 718 | — | — | 未开始 |
| Compose | 416 | 52,590 | — | — | 未开始 |
| Runtime | 12 | 1,501 | — | — | 未开始 |
| Shell | 14 | 2,584 | — | — | 未开始 |
| Terminal | 38 | 4,030 | — | — | 未开始 |
| Hosts | 55 | 4,349 | — | — | 未开始 |
| Browser | 2 | 419 | — | — | 未开始 |
| Sources | 19 | 4,861 | — | — | 未开始 |
| Tools | 88 | 4,800 | — | — | 未开始 |
| DevTools | 57 | 5,910 | — | — | 未开始 |
| Examples | 84 | 9,723 | — | — | 未开始 |
| Tests | 325 | 55,154 | — | — | 未开始 |

## 已完成的 crate

### `cove-core`（104 个 Swift 文件 → 92 个 Rust 文件）

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
- **新增外部 crate**：暂不引入；freetype/harfbuzz 为 vendored 依赖，留给 Text/Graphics 阶段。

## 尚未迁移

- **Graphics 平台后端**：`Metal`（15 文件 / 2,712 行）、`Vulkan`（31 / 6,010）、`DX12`（8 / 500）；`CoveUIGraphics`（12 / 1,757，含 `NativeUIBackdropPass` 531、`NativeDrawListRenderer` 473、`NativeUITextureStore` 379）。
- **Text**：字体发现、FreeType/HarfBuzz 整形、栅格化、彩色字形、图集、选择与文本布局。
- **Imaging**：位图/动画/SVG 解码、缓存、方向、纹理 backing。
- **Compose**：视图图、控件、主题、组合与重组（416 文件，最大单块）。
- **Runtime / Shell / Terminal**：应用运行时、SDL 窗口与事件循环、Ghostty 引擎与 PTY/ConPTY。
- **Hosts / Browser**：桌面、Android、iOS 宿主与 WebGPU/Canvas 宿主。
- **Tools / DevTools / Sources / Examples**：CLI 之外的工具、Inspection/Agent/Server/Inspector、SDK 安装器、示例应用。
- **Tests**：325 文件，作为行为清单迁移。

Swift 源码没有被删除，也没有用空 Rust 模块替代。新 CLI 不会调用 Swift 编译器；对尚无 `Cargo.toml` 的目录会直接说明未迁移。

## 平台边界

| 平台 | 已写入 | 尚缺 | 验证 |
| --- | --- | --- | --- |
| macOS | 共享 Rust/C、POSIX、权限/锁/身份、回收站桥接、RHI 前端 | Metal 后端、原生窗口、宿主、应用包与签名 | 仅 `cargo check` |
| Windows | 共享 Rust/C、UTF-16 进程参数、继承句柄白名单、Job Object、ACL、文件替换重试、RHI 前端 | DX12 后端、原生窗口、ConPTY UI、应用打包 | 仅 `cargo check` |
| Linux | 共享 Rust/C、POSIX、权限/锁/身份、恢复目录、RHI 前端 | Vulkan 后端、原生窗口、系统集成、应用打包 | 仅 `cargo check` |
| iOS | 共享 Rust/C、文件/资源/协议；进程 API 由系统能力决定 | 触摸/IME/生命周期宿主、Metal、Xcode 打包签名 | 仅 `cargo check` |
| Android | 共享 Rust/C、NDK 配置、文件/资源/协议、POSIX 进程调用 | Activity/JNI、触摸/IME/生命周期、Vulkan、APK 签名部署 | 仅 `cargo check` |
| Browser/Wasm | 平台数据类型 | 字体、图形、资源、浏览器事件循环与宿主 | 未建立构建链 |

不能因为本机无法运行其他平台而删除其实现。

## 当前验证状态

按用户要求，实施阶段不运行测试，只做 `cargo check` 保证编译安全：

```
cargo check --workspace --tests   # 通过（仅剩若干 dead_code / deprecated 警告）
```

尚未运行 `cargo test`、`cargo fmt --check`、`cargo clippy`。

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
