# Rust 迁移状态

整个项目的目标语言已改为 Rust 2024，性能关键内核保留 C。当前工作区版本是 `0.3.0-dev`；这不是已完成全量功能迁移或已发布的 Rust SDK。

## 已写入的实现

| crate | 范围 | 待补内容 |
| --- | --- | --- |
| `cove-core` | 几何与颜色、共享绘制缓冲区、C 变换/索引/透明度/边界内核、位图与 C 面积过滤 mipmap、可中断弹簧、状态订阅、字号缩放 | 场景树、布局、精确状态读取追踪、完整绘制指令与资源、裁剪/阴影/渐变、输入与渲染镜像 |
| `cove-platform` | 键盘、IME、指针、滚动、游戏手柄、窗口事件、菜单数据、资源目录查找 | 与各原生宿主的映射及资源打包验收 |
| `cove-system` | 文件权限与身份、锁、同卷原子替换、恢复目录、macOS 回收站、显式进程环境、双流读取、限额/超时/取消、POSIX 进程组与 Windows Job Object | 完整旧接口行为对照、移动系统权限边界验收 |
| `cove-logging` | 级别与最长模块前缀过滤、递归脱敏、UTF-8 限长、共享有界历史、JSONL 轮转 | 异步写入队列、任务上下文、跨会话保留、C 日志路由接入、环境配置 |
| `cove-devtools-protocol` | 树/布局/状态/时间线/镜像数据协议、检查请求验证、订阅会话、状态差异、输入录制与有界校验 | 网络传输、运行时适配、客户端与 Inspector |
| `cove-cli` | Cargo build/run/test/check/fmt/clippy/clean、doctor/setup、迁移盘点 | create、SDK 安装/钉版/切换、自更新、项目任务、平台打包/签名/部署、DevTools 客户端 |
| `cove-ui` | 上述基础 crate 的消费入口 | 声明式 UI 与原生运行时入口 |
| `cove-rust-counter` | 无窗口状态、Unicode 便签与 checkpoint 示例 | 真实跨平台 Counter UI |

Rust 输入事件采用 Rust 的序列化表示；DevTools 协议版本改为 `cove-devtools/0.2`，录制格式为 3。旧协议 0.1 / 录制格式 2 不宣称兼容，Inspector 与宿主须一起迁移。

## 尚未迁移的能力

- Core 场景/布局、Compose/Composition 的视图图、控件、主题、组合与重组。
- Text 的字体发现、FreeType/HarfBuzz 整形、栅格化、彩色字形、图集、选择与文本布局。
- Imaging 的位图/动画/SVG 解码、缓存、方向、纹理 backing。
- Graphics 的资源与提交规划、验证、Metal/D3D12/Vulkan 后端、GPU DrawList 渲染、图集/图片上传、磨砂与揭幕效果。
- Shell 的 SDL 窗口、事件循环、输入映射、剪贴板、光标、系统菜单与显示时序。
- Runtime、桌面/Android/iOS 宿主、Browser WebGPU/Canvas 宿主。
- Terminal 的 Ghostty 引擎、POSIX PTY、Windows ConPTY、终端交互与绘制。
- WebView、Workspace、DevTools Inspection/Agent/Server/Inspector。
- Gallery、Mobile、原 Counter 及其现有行为测试。
- SDK 安装器、原生依赖准备、移动应用打包/签名/部署、发布工具与发布工作流。

这些 Swift 源文件没有被删除，也没有用空 Rust 模块替代。新 CLI 不会调用旧 Swift 编译器；对尚无 `Cargo.toml` 的目录会直接说明未迁移。因而旧 Gallery 和应用打包目前不能通过新的入口运行。

## 平台边界

| 平台 | 已写入的基础实现 | 尚缺的完整应用实现 | 验证状态 |
| --- | --- | --- | --- |
| macOS | 共享 Rust/C、POSIX 进程组、权限/锁/身份、Foundation 回收站桥接 | 原生窗口、Metal、宿主、应用包与签名 | 未统一验证 |
| Windows | 共享 Rust/C、UTF-16 进程参数、继承句柄白名单、Job Object、ACL、文件替换重试 | 原生窗口、D3D12、ConPTY UI、应用打包 | 未统一验证 |
| Linux | 共享 Rust/C、POSIX 进程组、权限/锁/身份、恢复目录 | 原生窗口、Vulkan、系统集成、应用打包 | 未统一验证 |
| iOS | 共享 Rust/C、文件/资源/协议；进程 API 由系统能力决定，不能据可编译宣称可运行子进程 | 触摸/IME/生命周期宿主、Metal、Xcode 打包签名 | 未统一验证 |
| Android | 共享 Rust/C、NDK 编译配置、文件/资源/协议与 POSIX 进程调用 | Activity/JNI、触摸/IME/生命周期、Vulkan、APK 签名部署 | 未统一验证 |
| Browser/Wasm | 目前仅平台数据类型适合无宿主使用；未建立完整浏览器构建链 | 字体、图形、资源、浏览器事件循环与宿主 | 未统一验证 |

不能因为本机无法运行其他平台而删除其实现。现阶段尚缺的 Rust 平台能力已在上表与能力清单中明确列出。CI 中的移动任务只检查共享基础层，不表示整个移动应用已经实现。

## 最终统一验收

按用户要求，实施阶段不分批运行测试；已启动的首次测试流程在工具链准备阶段被停止，未得到测试结果。下面是全部实现结束后的验收要求，不是当前已通过的检查。

1. Rust 工作区构建、格式化、Clippy 与完整单元/集成测试；以原有 Swift 测试为行为清单迁移，不用空测试或只验证实现细节的测试代替。
2. macOS/Windows/Linux 的真实 Gallery、窗口/菜单/剪贴板、字体、GPU、PTY/ConPTY、DevTools 与 SDK 安装/更新验收。
3. iOS 模拟器/真机与 Android 真机/模拟器的生命周期、输入、IME、渲染、包/签名/部署验收。
4. Browser/Wasm 的构建、字体、WebGPU/Canvas、输入与资源加载验收。
5. 对齐所有公开产品与示例后删除自有 Swift 源码、SwiftPM 清单、旧脚本与旧 CI；保留第三方依赖原有语言不影响 Rust 主项目目标。
6. `./cove migration status --require-complete` 通过；该检查仅说明源码清理，不替代功能和平台验收。
7. 更新正式版本、安装器、SDK 元数据、所有示例与 CHANGELOG，再启用 Rust SDK 发布。

旧 Swift CI 保存在 [legacy/ci-swift.yml](legacy/ci-swift.yml)，用于追踪原生依赖、测试包、发布安装与平台验收范围。
