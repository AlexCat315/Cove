# Cove

Swift 跨平台 UI 框架与 `cove` CLI。应用共享声明式视图、布局、文本和图像管线，通过 NativeRHI 使用 Metal、DirectX 12 或 Vulkan。

SDK 安装包提供 macOS arm64、Windows x86_64、Linux x86_64。构建应用需要 Swift 6.4.1 和对应平台的编译工具。Apple 平台最低版本为 macOS 26 和 iOS 26。

## 安装 SDK

macOS / Linux：

```sh
curl -fsSL https://github.com/AlexCat315/Cove/releases/latest/download/install.sh -o install-cove.sh
sh install-cove.sh
```

Windows PowerShell：

```powershell
Invoke-WebRequest https://github.com/AlexCat315/Cove/releases/latest/download/install.ps1 -OutFile install-cove.ps1
./install-cove.ps1
```

安装器配置用户 PATH；安装后打开新终端。SDK 包含 CLI、框架源码和主机原生库，CLI 自带运行库，执行 `cove --version` 无需先安装 Swift。

```sh
cove setup
cove create app MyApp
cove run MyApp
cove self update
```

`cove self update [VERSION]` 校验发布包 SHA-256，再切换到独立版本目录。通过其他包管理器安装的 CLI 使用该包管理器更新。

## SDK 版本

应用的 `cove.json` 用 `toolchain.sdkVersion` 固定完整 SDK 版本。所有项目及 IDE 通过本地路径引用全局 `versions/VERSION` 中的同一份 SDK，不再单独下载 CoveUI 与原生库。

`cove sdk install VERSION|latest` 安装 SDK；`cove sdk use VERSION|PATH` 选择版本或本地开发源码。`latest` 使用本地最新版本，精确版本缺失时由 CLI 自动安装。项目使用对应 SDK 自带的 CLI。

主机原生库随安装包提供；Android 与 iOS 产物按目标下载到同一共享 SDK。构建与测试共用 SwiftPM 标准 `.build`。详见 [SDK 工具](Tools/Cove/README.md)。

## 源码开发

```sh
git clone --recurse-submodules https://github.com/AlexCat315/Cove.git
cd Cove
./cove setup
./cove test . --no-parallel
./cove test Tools/Cove
```

`System` 提供独立的 `CoveFileSystem`、`CoveProcess` 产品，供应用、CLI 和 Studio 共用，不依赖 SDL 或图形环境；接口语义和移动端支持见 [CoveSystem](System/README.md)。可直接运行 `./cove test System --no-parallel`。

框架按 Core、Text、Imaging、Graphics、Shell、Compose、Runtime、DevTools、Hosts 分层。各能力包独立声明 target，根包保留应用 API；依赖规则、部署需求及 Core 的 Yoga 迁移项见 [架构](Docs/architecture.md)。

## 发布

1. 更新 SDK 版本、示例钉版并写入 CHANGELOG.md，或运行 `Tools/Release/release.sh`。
2. 推送 `vVERSION` 标签。SDK release 自动构建桌面、Android 和 iOS 原生库，测试并打包各主机完整 SDK。
3. 所有构建通过后，在同一个 Release 发布安装包、目标平台原生库、清单与校验和。无需单独的 Native Release 或手工提交原生库清单。
