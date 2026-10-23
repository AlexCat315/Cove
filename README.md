# GuavaUI

Swift 跨平台 UI 框架与 `guava` CLI。应用共享声明式视图、布局、文本和图像管线，通过 NativeRHI 使用 Metal、DirectX 12 或 Vulkan。

预编译 CLI 首批提供 macOS arm64、Windows x86_64、Linux x86_64。构建应用需要 Swift 6.4.1 和对应平台的编译工具。

## 安装 CLI

macOS / Linux：

```sh
curl -fsSL https://github.com/AlexCat315/GuavaUI/releases/latest/download/install.sh -o install-guava.sh
sh install-guava.sh
```

Windows PowerShell：

```powershell
Invoke-WebRequest https://github.com/AlexCat315/GuavaUI/releases/latest/download/install.ps1 -OutFile install-guava.ps1
./install-guava.ps1
```

安装器配置用户 PATH；安装后打开新终端。CLI 自带运行库，执行 `guava --version` 无需先安装 Swift。

```sh
guava sdk install 0.1.0
guava setup
guava create app MyApp
guava run MyApp
guava self update
```

`guava self update [VERSION]` 校验发布包 SHA-256，再切换到独立版本目录。通过其他包管理器安装的 CLI 使用该包管理器更新。

## SDK 版本

应用的 `guava.json` 使用 `toolchain.sdkVersion` 固定版本，SwiftPM 依赖使用同一精确版本：

```swift
.package(url: "https://github.com/AlexCat315/GuavaUI.git", exact: "0.1.0")
```

本地联调可使用 `GUAVA_SDK_ROOT` 或项目内被忽略的 `.guava/sdk.json`，其格式为 `{"path":"/absolute/path/GuavaUI"}`。CLI 更新和项目 SDK 版本分别管理。

## 源码开发

```sh
git clone --recurse-submodules https://github.com/AlexCat315/GuavaUI.git
cd GuavaUI
./guava setup
./guava test . --no-parallel
./guava test Tools/Guava
```

`Runtime` 管理 NativeRHI、窗口与输入宿主及原生图像解码；`PlatformCore` 管理平台值；`Sources`、`Portable`、`Text` 管理 UI。场景、物理、脚本 HUD 和编辑器集成由 GuavaStudio 管理。GPU 生产的图像通过 `TextureImage` 进入现有图像绘制和资源持有流程；离屏视图使用 `OffscreenView` 返回绘制快照。

## 发布

1. 更新 `Tools/Guava/Sources/GuavaCLI/SDK.json` 的版本，运行 **Native SDK release**。
2. 将成功构建的 `native.json` 写入 `Artifacts/native.json` 并提交，等待三平台 CI 通过。
3. 推送对应的 `vVERSION` 标签。CLI 发布流程测试并打包三个平台，全部成功后发布安装器、程序与校验和。

原生依赖单独使用 `native-vVERSION-rREVISION` 预发布标签，SDK 与 CLI 使用 `vVERSION` 正式标签。已发布标签与资源保持不可变。
