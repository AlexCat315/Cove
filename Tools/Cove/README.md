# Cove CLI（Rust 迁移中）

源码入口为根目录 `cove` / `cove.cmd`，由 Cargo 编译 `cove-cli`。不调用 Swift，也不回退到旧 CLI。

```sh
./cove build . --locked
./cove run Examples/RustCounter
./cove check Core
./cove fmt . --fix
./cove clippy . --all-targets --locked
./cove migration status --json
```

在全部实现完成后执行 `./cove test . --no-parallel --locked`。`test --build-only` 只生成测试程序，`--filter` 传递测试名称筛选，`--` 后的参数传给测试程序或运行中的应用。

`build` / `check` / `run` / `test` 支持 `--release`、`--target TRIPLE`、`--package NAME`、`--locked`、`--offline`；构建命令支持 `--all-targets`。`run --bin NAME` 选择二进制程序。

`doctor` 检查 Rust/Cargo/C 编译器。`setup [--target TRIPLE]` 安装 Rust 的 rustfmt、clippy 和可选编译目标。跨平台构建仍需要对应 C 编译器及 SDK。

`migration status` 只盘点项目自己的 Swift/Rust 源码，排除依赖与构建目录；`--require-complete` 在仍有 Swift 源码时返回状态码 2。即使 Swift 文件已清空，仍须完成独立的平台和功能验收，不能只靠文件数宣布迁移完成。

尚未迁移的目录会明确报错。应用创建、SDK 安装/切换、自更新、原生应用打包、设备部署、项目任务和 DevTools 客户端尚待迁移，旧 Swift 文档不能作为新 CLI 的功能承诺。详见 [完整迁移清单](../../Docs/rust-migration.md)。
