# 快速开始

## 1. 环境要求

- Rust（版本由 `rust-toolchain.toml` 固定）、CMake、C 编译器
- macOS 26 / iOS 26 及以上（Apple 平台）
- Windows 10+ x86_64（DirectX 12）或主流 Linux 发行版 x86_64（Vulkan）

## 2. 创建应用

```bash
cargo install cargo-generate
cargo generate --git https://github.com/AlexCat315/Cove.git Templates/app --name my-app
cd my-app
cargo run
```

## 3. 打包

```bash
cargo xtask package                     # 当前桌面主机
cargo xtask package android --release   # Android + Vulkan
cargo xtask run ios --simulator         # iOS + Metal
```

## 下一步

- [架构概览](/docs/architecture)
- [视图与布局](/docs/views)
- [Cargo 工作流](/docs/cargo)
