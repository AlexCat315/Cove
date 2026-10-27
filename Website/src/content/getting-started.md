# 快速开始

本指南带你在几分钟内安装 Cove SDK，并运行第一个跨平台应用。

## 1. 环境要求

- **Swift 6.4.1**（从源码构建应用时需要）
- macOS 26 / iOS 26 及以上（Apple 平台）
- Windows 10+ x86_64（DirectX 12）或主流 Linux 发行版 x86_64（Vulkan）

`cove` CLI 自带运行库，只执行 `cove --version` 无需先安装 Swift。

## 2. 安装 SDK

macOS / Linux：

```bash
curl -fsSL https://github.com/AlexCat315/Cove/releases/latest/download/install.sh -o install-cove.sh
sh install-cove.sh
```

Windows PowerShell：

```powershell
Invoke-WebRequest https://github.com/AlexCat315/Cove/releases/latest/download/install.ps1 -OutFile install-cove.ps1
./install-cove.ps1
```

安装器会自动配置用户 `PATH`，安装完成后请打开一个新终端。

## 3. 验证安装

```bash
cove --version
cove setup
```

`cove setup` 会检查 Swift 工具链、平台编译器与图形后端，并给出缺失项的修复建议。

## 4. 创建第一个应用

```bash
cove create app MyApp
cove run MyApp
```

CLI 会生成一个共享工程，包含桌面、Android 与 iOS 的宿主配置，以及一份可运行的示例视图。

## 5. 选择目标平台

```bash
cove run MyApp --mac          # macOS + Metal
cove run MyApp --android      # Android + Vulkan
cove build MyApp --ios        # iOS + Metal
```

## 下一步

- 阅读 [架构概览](/docs/architecture)，理解 Cove 的分层设计。
- 学习 [视图与布局](/docs/views)，掌握声明式 UI 写法。
- 查阅 [cove CLI 手册](/docs/cli)，了解全部命令。
