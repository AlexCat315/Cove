# Guava CLI

`guava` 管理应用创建、构建、运行、测试、SDK 选择和 DevTools。源码入口为仓库根目录的 `guava` / `guava.cmd`，正式版本从 GitHub Release 安装。

| 平台 | 安装目录 | 命令入口 |
| --- | --- | --- |
| macOS | `~/.guava/versions/VERSION` | `~/.local/bin/guava` 符号链接 |
| Linux | `$XDG_DATA_HOME/guava/versions/VERSION`，默认 `~/.local/share/guava` | `~/.local/bin/guava` 符号链接 |
| Windows | `%LOCALAPPDATA%\Guava\versions\VERSION` | `%LOCALAPPDATA%\Guava\bin\guava.cmd` |

安装器维护用户 PATH；`GUAVA_HOME` 可指定安装根目录。`guava self update [VERSION]` 校验归档、检查新程序启动，再切换命令入口。旧版本目录保留，运行中的程序继续使用其原有文件。

SDK 存放在安装目录的 `sdks/VERSION`。`guava sdk install VERSION` 下载并选择 SDK；`guava sdk use PATH` 选择本地源码。项目通过 `toolchain.sdkVersion` 固定版本；本地覆盖放在 `.guava/sdk.json` 或 `GUAVA_SDK_ROOT`。

```sh
guava --version
guava create app MyApp
guava run MyApp --mac
guava build MyApp --win --release
guava build MyApp --linux --release
guava test MyApp
guava devtools MyApp
guava doctor
```

桌面应用在对应操作系统上构建。Windows 编译环境由 CLI 从 Visual Studio C++ Build Tools 自动加载；Linux 使用系统编译器、Vulkan 开发包和图形驱动。`guava setup` 安装预编译原生依赖；源码版本可用 CMake、Ninja 和 Zig 构建它们。

CLI 发布使用 `guava internal package-cli`；原生依赖发布使用 `guava internal package-native`。
