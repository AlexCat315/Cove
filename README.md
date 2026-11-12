# Cove

Rust 2024 跨平台 UI 工具包。工作区版本为 `0.3.0-dev`，尚未发布。

Cove 自有代码采用 [Apache-2.0](LICENSE)。依赖保留各自许可，见 [第三方声明](THIRD_PARTY_NOTICES.md)；启用原生媒体后的随包材料见 [媒体许可证与分发](Docs/media-licensing.md)。

需要 `rust-toolchain.toml` 固定的 Rust 工具链。`cargo cove setup` 检测并按平台补齐 C/C++ 编译器、CMake、Ninja、Zig 0.17，初始化 `third-party/` 子模块，并把校验过的工具路径写入 Cove 缓存状态供各 `build.rs` 读取。构建浏览器 / Wasm 目标时追加 `--wasm`，它会按平台下载并校验 wasi-sdk 34、锁定原生 Ninja，CMake 生成器与编译器路径由 `cove-native-build` 统一处理：

```sh
cargo cove setup
cargo build --workspace --locked
cargo run -p cove-examples --bin gallery --locked
```

**安装与环境变量的作用域**：Zig / Slang 是真实下载到本机 Cove 缓存（`%LOCALAPPDATA%\Cove\tools` 或 `~/Library/Caches/Cove/tools`）的固定版本归档，经 SHA-256 校验后解压；编译器、CMake、Ninja 等则按平台通过 Homebrew / WinGet / 包管理器安装，Windows 还会恢复 `vcvars`。但 **setup 默认不修改用户机器或终端的环境变量**：它把绝对路径写进 Cove 自己的状态文件，`build.rs` 通过 `cove-native-build` 把该状态注入子进程，所以普通 `cargo build` 无需任何 `export` 就能找到工具。想在新开的终端里直接敲 `zig` / `slangc`，二选一：

```sh
# 仅当前终端生效，零残留（Windows Git Bash 用 --shell posix，PowerShell 用 --shell powershell）
eval "$(cargo cove env --shell posix)"

# 或持久写入用户级 PATH（macOS/Linux 写带标记的 shell profile 块）；重开终端生效
cargo cove setup --persist-path
cargo cove setup --unpersist-path   # 撤销，只移除 Cove 自己的条目
```

`--persist-path` 是显式的 opt-in：默认静默改动 PATH 会把项目专属的工具版本带进机器上所有其他 shell。Windows 实现直接读写注册表原始值，保留 `REG_EXPAND_SZ` 与未展开的 `%VAR%`；结果无变化时不写入。

改动 `crates/graphics/resources/Shaders/Slang/*.slang` 时追加 `cargo cove setup --shaders`，它会安装固定版本 Slang 2026.19；Windows 把 Windows SDK 的 `dxcompiler.dll` / `dxil.dll` 放到 `slangc` 旁边，macOS / Linux 尝试复用 Slang 归档自带的 DXC 运行库，没有就在报告里标明 DXIL 不可用。`Tools/Shaders/build-native-ui.mjs` 直接读取该状态中的 `SLANGC`，无需手工 export。

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

Wasm 目标还需要 `rustup target add wasm32-wasip1`，随后在 `crates/browser` 执行 `npm ci && npm run build && npm run serve`；详见 [浏览器宿主说明](crates/browser/README.md)。`cargo cove setup --check` 只报告工具、SDK、子模块、shader 与 wasm 工具链状态，CI 也用同一条命令校验清单；`--dry-run` 只输出安装计划（含 PATH 持久化计划，不会执行）。`--media` 追加 FFmpeg 工具，`--shaders` 追加 Slang/DXIL，`--wasm` 追加 wasi-sdk / 原生 Ninja（macOS、Linux、Windows 的 x86_64 与 aarch64 归档均有固定校验值）。`cargo cove env` 打印可直接 eval 的环境变量语句，`--persist-path` / `--unpersist-path` 管理用户级 PATH。所有报告都带 `shell` 与 `user_path` 字段，明确说明构建脚本与交互终端各自的环境来源。

新建应用与打包：

```sh
cargo generate --git https://github.com/AlexCat315/Cove.git Templates/app --name my-app
cargo xtask package android --release --package cove-examples
cargo xtask run ios --simulator --package cove-examples
```

平台实现和验证范围见 [平台覆盖矩阵](crates/hosts/src/PLATFORM_COVERAGE.md)；测试通过不代表已完成设备验收。本地 `swift/` 参考树（不随仓库分发）保留迁移来源与逐文件映射。发布变更记入 [CHANGELOG.md](CHANGELOG.md)。
