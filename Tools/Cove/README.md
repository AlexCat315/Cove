# Cove CLI

源码入口为根目录 `cove` / `cove.cmd`，由 Cargo 编译 `cove-cli`。不调用 Swift，也不回退到旧 CLI。

CLI 只做 Cargo 做不到或做不安稳的事：脚手架、平台打包、设备部署、DevTools 会话。构建本身交给 Cargo。

```sh
./cove fmt . --fix
./cove clippy . --all-targets --locked
./cove check . --package cove-ui --target aarch64-linux-android --locked
./cove test . --no-parallel --locked
./cove build . --release
./cove run Examples/RustCounter --locked
./cove migration status --json
```

## 命令分层

**透传 Cargo**：`build` / `run` / `test` / `fmt` / `clippy` / `check` / `clean`。统一入口，跨平台同一套参数，不重写构建逻辑。`--locked`、`--offline`、`--all-targets`、`--package`、`--target` 直接传给 Cargo。

`fmt` 默认带 `--check`（CI 用），`--fix` 写回。`clippy` 固定 `-D warnings`。`test --build-only` 等价 `cargo build --tests`，`--filter` 是 Cargo 的测试名子串，`--no-parallel` 传 `--test-threads=1`。`--` 之后的参数传给测试程序或运行中的应用。

**脚手架**：`cove create app <dir> [--platforms mac,win,linux,android,ios]` 生成 Cargo 工程（`Cargo.toml`、`src/{main,app,resources}.rs`、`Assets/`、`Platforms/`）与 `cove.json`。Cargo 没有模板能力，这是它的核心价值。

**平台打包与部署**：`--android` 交叉编译后用 Gradle 出 APK 并签名；`--ios` 走 xcodebuild 出 `.app` 并安装到模拟器或真机；桌面产出 `.app`／可执行目录并做 codesign。Cargo 交叉编译只给二进制，不产 APK/IPA、不做签名、不做安装。

**DevTools**：`cove devtools` 起 Inspector 会话，或把 `list/query/click/activate/reveal/input/capture/resize/timings` 作为结构化客户端动作发给运行中的应用。

**环境检查**：`cove doctor` 报 Rust/Cargo、C 编译器和平台 SDK 是否在位。只检查，不安装——工具链版本由 `rust-toolchain.toml` 钉住，依赖由 Cargo 管。

**迁移盘点**：`cove migration status [--json] [--require-complete]` 统计 `Docs/rust-migration-files.json` 里仍未迁移的生产源码，`--require-complete` 在仍有缺口时返回非零。

## 不再提供

`setup`、`sdk install/use/list`、`self update` 已删除。工具链版本由 `rust-toolchain.toml` 决定，SDK 版本由 `Cargo.toml` 的路径依赖决定，CLI 升级交给安装它的包管理器。

`cove task` 与 `cove.workspace.json` 仍按 SwiftPM 时期的声明格式工作，迁移到 Cargo 宿主后尚未完成验证，视为不可用。

详见 [迁移清单](../../Docs/rust-migration.md)。
