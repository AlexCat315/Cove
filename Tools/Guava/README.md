# Guava SDK

`guava` 管理 SDK 安装、应用创建、构建、运行、测试和 DevTools。每个版本包含 CLI、框架源码、模板与原生库；源码开发入口为仓库根目录的 `guava` / `guava.cmd`。

| 平台 | 完整 SDK 目录 | 命令入口 |
| --- | --- | --- |
| macOS | `~/.guava/versions/VERSION` | `~/.local/bin/guava` |
| Linux | `$XDG_DATA_HOME/guava/versions/VERSION`，默认 `~/.local/share/guava` | `~/.local/bin/guava` |
| Windows | `%LOCALAPPDATA%\Guava\versions\VERSION` | `%LOCALAPPDATA%\Guava\bin\guava.cmd` |

安装器维护 PATH；`GUAVA_HOME` 指定安装根目录。`guava self update [VERSION]` 校验完整 SDK 并切换默认版本，保留旧版供钉版项目使用。

`guava sdk install VERSION|latest` 只安装共享 SDK，不修改项目版本。`guava sdk use VERSION|PATH` 选择已安装版本或开发源码，在应用目录执行时同时修改该项目的 SDK 版本。`guava.json` 的 `toolchain.sdkVersion` 支持精确版本或 `latest`；项目缺少精确版本时 CLI 自动安装。CLI 将命令交给该 SDK 自带的工具。

项目清单与 IDE 解析同一份全局 SDK，通过本地路径依赖使用框架。无需向项目克隆 GuavaUI。临时覆盖可用 `GUAVA_SDK_ROOT` 或项目内被忽略的 `.guava/sdk.json`，格式为 `{"path":"/absolute/path/GuavaUI"}`。

安装包包含主机平台原生库。iOS 和 Android 的原生库由 SDK 按需下载一次，保存在共享 SDK 的 `Artifacts/native/TARGET`，所有项目共用。Swift Android SDK 同样按需安装并校验。Xcode、Android SDK/NDK、JDK 和系统编译器由 `setup` / `doctor` 检查。

```sh
guava create app MyApp
guava run MyApp --mac
guava build MyApp --android --release
guava build MyApp --ios
guava test MyApp
guava devtools MyApp
guava doctor
```

桌面应用在对应系统上构建；Android 构建主机支持 macOS、Linux；iOS 需要 macOS 和完整 Xcode。Windows 的 C++ 工具环境由 CLI 自动加载。

构建和测试使用 SwiftPM 标准 `.build`，包含应用打包和开发会话；`guava clean` 清理该项目的生成文件，保留共享 SDK 与作者数据。不同编译目标和配置仍保留各自的必要产物。

单个 `vVERSION` 标签触发 SDK release，构建桌面及移动端原生库、合并校验清单、打包三个主机 SDK，全部验证后发布一个 Release。内部命令 `guava internal package-native` 和 `package-cli` 是同一流程的构建阶段。
