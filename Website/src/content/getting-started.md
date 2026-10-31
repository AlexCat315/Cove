# 快速开始

## 1. 环境要求

- Rust（版本由 `rust-toolchain.toml` 固定）、CMake、C 编译器
- macOS 26 / iOS 26 及以上（Apple 平台）
- Windows 10+ x86_64（DirectX 12）或主流 Linux 发行版 x86_64（Vulkan）

## 2. 创建应用

```bash
cargo new my-app
cd my-app
cargo add cove-ui --git https://github.com/AlexCat315/Cove.git
cargo run
```

## 3. 目标平台

```bash
cargo run                                          # 当前桌面主机
cargo build --lib --target aarch64-linux-android   # Android + Vulkan
cargo build --lib --target aarch64-apple-ios       # iOS + Metal
```

## 下一步

- [架构概览](/docs/architecture)
- [视图与布局](/docs/views)
- [Cargo 工作流](/docs/cargo)
