# cove CLI 手册

`cove` 是贯穿整个开发循环的命令行工具：环境检查、工程创建、运行、测试、构建与 SDK 版本管理。

## 常用命令速览

```bash
cove setup                 # 检查工具链与图形后端
cove create app <Name>     # 创建跨平台应用工程
cove run <Name>            # 构建并运行
cove test [path]           # 运行测试
cove build <Name>          # 构建（不运行）
cove clean                 # 清理构建产物
```

## 目标平台选择

```bash
cove run MyApp --mac          # macOS + Metal
cove run MyApp --android      # Android + Vulkan
cove build MyApp --ios        # iOS + Metal
cove run MyApp --ios --simulator UDID   # 指定模拟器
cove run MyApp --device ID --team TEAM_ID  # iOS 真机
```

## SDK 版本管理

应用的 `cove.json` 用 `toolchain.sdkVersion` 固定完整 SDK 版本。

```bash
cove sdk install VERSION      # 安装指定版本
cove sdk install latest       # 安装最新版本
cove sdk use VERSION          # 为当前项目选择版本
cove sdk use PATH             # 使用本地开发源码
```

所有项目与 IDE 通过本地路径引用全局 `versions/VERSION` 中的同一份 SDK，不再单独下载 CoveUI 与原生库。精确版本缺失时，CLI 会自动安装。

## 自更新

```bash
cove self update           # 更新到最新
cove self update VERSION   # 更新到指定版本
```

自更新会校验发布包的 SHA-256，再切换到独立版本目录，旧版本可随时回退。通过其他包管理器（如 Homebrew）安装的 CLI，应使用该包管理器更新。

## 从源码开发

```bash
git clone --recurse-submodules https://github.com/AlexCat315/Cove.git
cd Cove
./cove setup
./cove test . --no-parallel
./cove test Tools/Cove
```

## 工作站命令（Diffuse）

当你在 [Diffuse](https://github.com/AlexCat315/Diffuse) 仓库中时，`cove` 还提供工作站相关命令：

```bash
cove studio native-bootstrap          # 构建 FFmpeg / OpenEXR / Wasmtime 原生依赖
cove studio native-bootstrap --force  # 强制重编译
cove internal build-package Engine    # 按包独立构建
```

## 全局选项

```bash
cove --version               # 查看版本
cove --help                  # 查看帮助
COVE_SDK_ROOT=/path cove run MyApp   # 临时指定 SDK 路径
```
