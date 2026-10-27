# 架构概览

Cove 采用清晰的分层架构，把跨平台差异收敛到最底层，向上提供统一的 Swift API。

## 分层总览

```
┌─────────────────────────────────────────────┐
│  应用层 (你的 App / Diffuse 工作站)          │
├─────────────────────────────────────────────┤
│  Sources / Portable / Text   声明式 UI 层    │
│  · 视图树  · 布局  · 文本  · 图像  · 组件     │
├─────────────────────────────────────────────┤
│  Runtime  窗口 / 输入 / NativeRHI / 图像解码 │
├─────────────────────────────────────────────┤
│  PlatformCore  平台值与类型桥接              │
├─────────────────────────────────────────────┤
│  System  文件系统 / 进程 (无图形依赖)        │
├─────────────────────────────────────────────┤
│  Metal · DirectX 12 · Vulkan   原生 GPU 后端 │
└─────────────────────────────────────────────┘
```

## System

`System` 提供独立的 `CoveFileSystem` 与 `CoveProcess` 产品，供应用、CLI 与工作站共用，不依赖 SDL 或任何图形环境。这意味着即使在无头服务器上，也能使用文件与进程能力。

## Runtime

`Runtime` 负责：

- **NativeRHI**：统一的 GPU 抽象，对接 Metal / DX12 / Vulkan
- **窗口与输入宿主**：基于 SDL3 的跨平台窗口、鼠标键盘与触摸事件
- **原生图像解码**：把解码后的位图上传为 GPU 纹理

## UI 层

`Sources`、`Portable` 与 `Text` 共同构成 UI 层：

- 声明式视图与视图组合（`CoveUI` / `CoveUICompose`）
- 基于 Yoga 的 Flexbox 布局
- FreeType 光栅化 + HarfBuzz 文本塑形
- 图像管线：GPU 生产的图像通过 `TextureImage` 进入绘制与资源持有流程
- 离屏视图通过 `OffscreenView` 返回绘制快照

## SDK 版本模型

应用的 `cove.json` 使用 `toolchain.sdkVersion` 固定完整 SDK 版本。所有项目与 IDE 通过本地路径引用全局 `versions/VERSION` 中的同一份 SDK，不再单独下载 CoveUI 与原生库。

```bash
cove sdk install latest      # 安装最新 SDK
cove sdk use 1.2.0           # 为当前项目选择版本
```

## 与 Diffuse 的边界

场景、三维渲染、物理、脚本 HUD 与编辑器集成由 [Diffuse](https://github.com/AlexCat315/Diffuse) 管理；Cove 只负责 UI、NativeRHI、平台服务与 CLI。两者组合，构成从底层 UI 到创作工作站的完整工具链。
