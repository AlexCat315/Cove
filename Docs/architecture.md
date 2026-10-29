# 能力分层与包边界

> 本文记录保留中的 Swift 0.2.x 架构，作为 Rust 迁移的功能与边界验收依据。当前 Rust 工作区状态见 [rust-migration.md](rust-migration.md)，本页的 Swift 构建命令不再适用于当前 `cove` 入口。

每个能力拥有自己的源码、target 声明与测试。部署需求附着在能力及其实现上；目录和依赖方向不再由「能否脱离原生环境运行」决定。根 `Package.swift` 只定义应用入口与扩展，不重新声明下层 target。

## 依赖方向

| 层 | 所有者 | 公开产品 | 职责 | 实现需求 |
|---|---|---|---|---|
| L0 | `Core/` | `CoveUICore` | 状态、观察、布局、场景图、动画、DrawList、帧快照与资源持有契约 | 基础值与绘制数据为 Swift；当前场景布局仍依赖 Yoga，见下文 |
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

三个窄入口有单独的 manifest，它们表达可独立使用的抽象：

| 入口 | 产品 | 用途 |
|---|---|---|
| `Compose/Composition` | `CoveUIComposition` | View/ViewBuilder/ViewGraph、宏、基础组合；不引入字体或图片实现 |
| `DevTools/Protocol` | `CoveUIDevToolsProtocol` | 消息、会话与诊断值契约；CLI 不链接 socket、窗口、字体或 GPU |
| `DevTools/Inspection` | `CoveUIInspection` | 场景/语义查询与 Agent 工具契约；Browser 和无窗口检查使用 |

这三个入口不复制 target。完整 Compose/DevTools 包依赖它们。跨能力测试集中在 `Tests/Headless`，生产包不会为了测试反向依赖上层包。

## 部署标注

`Native/third-party` 与 `Native/vendor` 是原生构建输入、缓存和发布产物，不是框架层。Text 的产物暂沿用根 `vendor`。Android/iOS 的产物根目录仍由 CLI 传入；各能力包统一将路径解释为相对于 SDK 根目录，再解析到自身位置。SDK 打包必须包括所有能力 manifest 和源码。

部署标签描述完整依赖闭包，而非文件使用的语言。`Compose` 使用 Swift 实现，不意味着使用其完整图片和文本功能时没有原生库依赖。CLI 只选择协议闭包，Browser 原型选择基础组合、检查和 Wasm 字体实现，原生应用选择完整 Runtime。

## Core 的纯 Swift 要求

当前 `LayoutNode`、`FlexProps` 和自定义布局仍使用 Yoga。此次迁移保持布局语义和应用 API，将 Yoga 留在 Core 内部，不把这一现状标成「纯 Swift Core」。基础状态、几何和 DrawList target 本身没有原生依赖。

严格实现题设的纯 Swift L0，尚需替换 Yoga 布局实现，或让 Core 定义布局后端契约、由宿主明确注入后端。后者必须覆盖节点所有权、脏标记、约束/百分比/RTL、文字测量、自定义布局的探测与放置，且 Core 自身仍须有可用的 Swift 默认后端；只把 Yoga 改名或放到另一个目录不满足要求。应以现有场景布局测试及 Gallery 几何行为作为替换验收，不能用缺少布局能力的空实现获得「无原生依赖」。

## 检查与验收

`python3 Tools/Architecture/check.py` 检查每个 target 的唯一所有者、生产包依赖方向、源码 import 边界、跨能力原生桥接导入和内部 product 回流。CI 在构建前执行它。

构建与测试统一通过 Cove CLI：

```sh
./cove test . --build-only
./cove test . --skip-build --no-parallel
./cove test Core --no-parallel
./cove test Text --no-parallel
./cove test Tests/Headless --no-parallel
./cove test DevTools --no-parallel
./cove test Graphics --no-parallel
./cove test Tools/Cove --no-parallel
```

测试包不再由根 manifest 重复声明，因此根包测试和能力包测试均需在 CI 中执行。平台图形/窗口后端仍需各平台验收；本机通过不代表 Android、Wasm、Windows 与 Linux 全部通过。
