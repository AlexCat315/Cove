# Cove

Rust 2024 跨平台 UI 工具包。工作区版本为 `0.3.0-dev`，尚未发布。

需要 `rust-toolchain.toml` 固定的 Rust 工具链、C/C++ 编译器、CMake 和 Zig 0.16。首次构建前初始化原生依赖：

```sh
git submodule update --init --recursive
cargo build --workspace --locked
cargo run --package cove-rust-counter --locked
```

## 工作区

| 目录 | 内容 |
| --- | --- |
| `src/` | `cove-ui` 公共入口 |
| `crates/` | Core、平台、绘制、文本、组件、宿主、终端、开发工具等库 |
| `examples/` | Gallery、Counter、移动示例 |
| `xtask/` | 应用打包、签名、安装与迁移审计 |
| `Templates/app/` | `cargo-generate` 应用模板 |
| `third-party/` | 原生依赖子模块，由 `build.rs` 编译到 `OUT_DIR` |

crate 使用 `src/`、`tests/`、`native/`、`resources/`；`build.rs` 位于 crate 根目录。性能关键的原生内核保留 C。

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked --no-fail-fast -- --test-threads=1
cargo xtask audit --json
```

新建应用与打包：

```sh
cargo generate --git https://github.com/AlexCat315/Cove.git Templates/app --name my-app
cargo xtask package android --release --package cove-mobile-example
cargo xtask run ios --simulator --package cove-mobile-example
```

平台实现和验证范围见 [迁移验证](Docs/rust-migration-validation.md)；测试通过不代表已消除其中列出的行为差异或完成设备验收。[逐文件映射](Docs/rust-migration-files.md)保留迁移来源及目标。发布变更记入 [CHANGELOG.md](CHANGELOG.md)。
