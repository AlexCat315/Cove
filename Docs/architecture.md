# 能力分层与包边界

> 本文记录保留中的 Swift 0.2.x 架构，作为 Rust 迁移的功能与边界验收依据。当前 Rust 工作区状态见 [rust-migration.md](rust-migration.md)，本页的 Swift 构建命令不再适用于当前 `cove` 入口。

每个能力拥有自己的源码、target 声明与测试。部署需求附着在能力及其实现上；目录和依赖方向不再由「能否脱离原生环境运行」决定。根 `Package.swift` 只定义应用入口与扩展，不重新声明下层 target。

## 依赖方向

| 层 | 所有者 | 公开产品 | 职责 | 实现需求 |
|---|---|---|---|---|
| L0 | `Core/` | `CoveUICore` | 状态、观察、布局、场景图、动画、DrawList、帧快照与资源持有契约 | 布局为 Taffy（纯 Rust），见下文 |
| L1 | `Text/` | `CoveUIText` | 字体发现、字体实例、整形、字形光栅化、图集、文本环境与缓存 | FreeType、HarfBuzz；平台字体发现/彩色字形适配 |
| L2 | `Imaging/` | `CoveUIImaging` | 位图/动画解码、矢量栅格化、方向、图片缓存和图片所有权接口 | lunasvg、plutovg、webp、sharpyuv；可选平台格式 |
| L3 | `Graphics/` | `CoveUIGraphics` | RHI、图形设备/资源、DrawList GPU 渲染、纹理驻留与图集上传 | Metal、D3D12、Vulkan |
| L4 | `Shell/` | `CoveUIShell` | 窗口、事件循环、输入、剪贴板、光标、显示刷新 | SDL3 与平台窗口 API |
| L5 | `Compose/` | `CoveUICompose` | 声明式视图、控件、主题、布局修饰符、离屏组合 | Swift 源码；使用 Core/Text/Imaging 的公开接口 |
| L6 | `Runtime/` | `CoveUIRuntime` | 组装 Core/Text/Imaging/Graphics/Shell/Compose，适配原生宿主与帧调度 | 由所选后端继承 |
| L7 | `DevTools/` | `CoveUIDevTools`、`CoveUIAgent`、探针程序 | 协议、检查、服务端、运行时采样、Agent 适配 | 协议与检查不需要 GPU；socket 实现使用 NIO |
| L8 | `Hosts/`、`Browser/`、CLI | 根包的 `CoveUI` 等应用产品 | 桌面/Android/浏览器/CLI 入口，选择实现、组装应用 | 各宿主自身需求 |

`System`、`Logging`、`PlatformCore` 是共用支撑能力，target 只在各自的 manifest 中声明。浏览器当前使用 WebGPU/Canvas 原型；WebGL 尚未作为 Graphics 的通用 RHI 实现发布。

依赖为有向无环图。层号表达允许的依赖方向，不要求每一层依次穿过所有中间层：

```text
Core ← Text
Core ← Imaging
Core + Text + Imaging ← Graphics
PlatformCore + Logging ← Shell
Core + Composition + Text + Imaging ← Compose
Core + Text + Imaging + Graphics + Shell + Compose ← Runtime
Core + Protocol ← Inspection
Runtime + Inspection + Protocol ← DevTools
Runtime + Compose + DevTools ← Hosts / CoveUI
Protocol ← CLI
Core + Composition + Text + Inspection ← Browser
```

`Compose` 不依赖 Runtime、Graphics 或 Shell。声明光标使用 Core 导出的平台值，不导入 SDL 窗口实现。文本测量、整形缓存和图集查询属于 Text；GPU 上传属于 Graphics。离屏视图输出 Core 的 `DrawListSnapshot`，无需拥有 GPU 渲染器。

## API 表面与内部模块

一个 product 对应一个支持的消费入口，不对应每个 Swift/C target。

- Core 的 `CoveUICore` 产品包含场景入口；`CoveUIScene` 是内部模块划分，不再单独发布 product。
- Graphics 以一个产品提供 RHI 和 DrawList 渲染；`NativeRHI` 不再从根包重复发布。C/平台桥接模块不作为 UI 产品发布。
- `CFreeType`、`CHarfBuzz` 只由 Text 声明与导入。公开字体 API 使用 `FontFace`、`FontAxis`、`ScriptTag` 等 Swift 类型，不泄漏 FT/HB 原生类型。
- `TextureImageBacking` 由 Imaging 定义；Graphics 用适配器持有纹理资源并实现该契约，并保留 `TextureImage(texture:)`。Compose 只持有图片，不接触 `TextureResource`。渲染器拒绝其他后端的 backing。
- `package` 访问不跨 SwiftPM 包。需要被上层调用的场景语义查询是下层的明确公开接口。
- `import CoveUI` 继续作为主要应用入口。内部模块旧 product 的移除属于此次允许的破坏性迁移，不增加旧依赖图的兼容壳。

以下窄入口有单独的 manifest，它们表达可独立使用的抽象：

| 入口 | 产品 | 用途 |
|---|---|---|
| `Compose/Composition` | `CoveUIComposition` | View/ViewBuilder/ViewGraph、宏、基础组合；不引入字体或图片实现 |
| `DevTools/Protocol` | `CoveUIDevToolsProtocol` | 消息、会话与诊断值契约；CLI 不链接 socket、窗口、字体或 GPU |
| `DevTools/Inspection` | `CoveUIInspection` | 场景/语义查询与 Agent 工具契约；Browser 和无窗口检查使用 |
| `Agent` | `cove-agent` | 工具、skills、MCP 与传输；不依赖 UI，Diffuse 等应用直接使用 |
| `Http` | `cove-http` | 通用 HTTP 客户端；图片传输与应用网络请求共用 |

这些入口不复制 target。完整 Compose/DevTools 包依赖它们。跨能力测试集中在 `Tests/Headless`，生产包不会为了测试反向依赖上层包。

## 部署标注

原生依赖是 `third-party/` 下固定版本的 git 子模块。`cove-native-build` 在各 crate 的 `build.rs` 中用 CMake 将其编译进 `OUT_DIR` 并静态链接；libghostty-vt 使用 Ghostty `v1.4.0` 源码，由 Zig 0.17.0 的 `zig build` 编译为静态库。不使用预编译产物目录或环境变量覆盖。

部署标签描述完整依赖闭包，而非文件使用的语言。`Compose` 使用 Swift 实现，不意味着使用其完整图片和文本功能时没有原生库依赖。CLI 只选择协议闭包，Browser 原型选择基础组合、检查和 Wasm 字体实现，原生应用选择完整 Runtime。

## Core 布局

`LayoutNode`、`FlexProps` 和自定义布局由 `cove-core` 内的 Taffy（纯 Rust）实现，语义沿用 Swift 版的 Yoga：默认列方向、`flex-shrink: 0`、无自动最小尺寸、布局时解析 logical 边。Taffy 通过底层 trait 直接遍历 `LayoutNode` 树，每个节点自带样式、缓存和父节点相对的结果；自定义布局的探测与放置见 `crates/core/src/scene/layout_node/custom.rs`。Core 没有原生布局依赖。

Taffy 启用 Flexbox、CSS Grid 与 `calc`。`CssGrid` 直接参与保留布局树；普通 `Grid` 通过自定义布局测量契约接入 Taffy 求解，列显隐策略仍归 Compose，`LazyGrid` 保留虚拟化。尺寸表达式与接口见 [布局](layout.md)。

## 检查与验收

`python3 Tools/Architecture/check.py` 检查每个 target 的唯一所有者、生产包依赖方向、源码 import 边界、跨能力原生桥接导入和内部 product 回流。CI 在构建前执行它。

构建与测试直接使用 Cargo：

```sh
cargo test --workspace --no-run
cargo test --workspace -- --test-threads=1
cargo test -p cove-core
cargo test -p cove-text
cargo test -p cove-devtools
cargo test -p cove-graphics
```

测试包不再由根 manifest 重复声明，因此根包测试和能力包测试均需在 CI 中执行。平台图形/窗口后端仍需各平台验收；本机通过不代表 Android、Wasm、Windows 与 Linux 全部通过。

## 后续能力设计

[音视频与 GStreamer 集成设计](media-gstreamer.md)记录 Rust 媒体能力的公共接口、可选模块、RHI/宿主适配、平台工作项和验证门禁；该文档为设计草案，不代表媒体接口已实现。
