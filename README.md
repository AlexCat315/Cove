# Cove

Rust 2024 跨平台 UI 工具包。工作区版本为 `0.3.0-dev`，尚未发布。

Cove 自有代码采用 [Apache-2.0](LICENSE)。依赖保留各自许可，见 [第三方声明](THIRD_PARTY_NOTICES.md)；启用原生媒体后的随包材料见 [媒体许可证与分发](Docs/media-licensing.md)。

需要 `rust-toolchain.toml` 固定的 Rust 工具链、C/C++ 编译器、CMake 和 Zig 0.17。首次构建前初始化原生依赖：

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
| `xtask/` | 应用打包、签名、安装、迁移审计与原生构建环境初始化 |
| `Templates/app/` | `cargo-generate` 应用模板 |
| `third-party/` | 原生依赖子模块；CMake 库编译到 crate `OUT_DIR`，FFmpeg 由 `ffmpeg-sys-next` 构建脚本编译到 `OUT_DIR` |

crate 使用 `src/`、`tests/`、`native/`、`resources/`；`build.rs` 位于 crate 根目录。性能关键的原生内核保留 C。

AI 调试可直接使用 [DevTools MCP](crates/devtools/README.md)。项目配置见 `.mcp.json`；启动入口为 `cargo run --quiet -p cove-devtools-cli -- mcp`，提供查询、交互、等待、截图和性能工具。

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked --no-fail-fast -- --test-threads=1
cargo xtask audit --json
```

媒体功能按 `ffmpeg` feature 选择，首次使用前初始化一次原生构建环境，之后沿用普通 Cargo 命令：

```sh
cargo cove setup --media
cargo test --locked -p cove-media --features ffmpeg --all-targets
```

`cargo cove setup --media --check` 只报告工具与 SDK 状态，`--dry-run` 只输出安装计划。

新建应用与打包：

```sh
cargo generate --git https://github.com/AlexCat315/Cove.git Templates/app --name my-app
cargo xtask package android --release --package cove-mobile-example
cargo xtask run ios --simulator --package cove-mobile-example
```

平台实现和验证范围见 [平台覆盖矩阵](crates/hosts/src/PLATFORM_COVERAGE.md)；测试通过不代表已完成设备验收。本地 `swift/` 参考树（不随仓库分发）保留迁移来源与逐文件映射。发布变更记入 [CHANGELOG.md](CHANGELOG.md)。
