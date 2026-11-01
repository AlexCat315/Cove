# Rust 迁移验证

日期：2026-11-01。版本：`0.3.0-dev`（未发布）。

第三批分组文档的 65 个目标已存在；另补入未登记的 `DesignSystemTests.swift` 对应套件。冻结清单保留 914 个生产源文件、411 个测试及辅助文件、23 个 SwiftPM 清单的原路径、行数、SHA-256 与目标。源文件删除后不再按名称猜测或删除历史行。

## 目录

- 根包：`src/`、`tests/`。
- 库：`crates/<name>/src/`、`tests/`、`native/`、`resources/`，构建脚本为根 `build.rs`。
- 示例：`examples/gallery/`、`examples/counter/`、`examples/mobile/`。
- 打包及审计：`xtask/`；执行 `cargo xtask audit --json`。
- 原生依赖子模块统一在 `third-party/`，由 `cove-native-build` 编译到各 crate 的 `OUT_DIR`；其中的第三方 Swift 文件不属于 Cove 源码清理范围。

## 验证记录

最终目录验证进行中。原目录基线完成编译并运行了 754 个通过的用例后，主动停止以进行目录迁移；该轮不是全量通过结论。

原生依赖构建（2026-11-01，macOS arm64 主机）：`cove-text`、`cove-shell`、`cove-imaging`、`cove-terminal`
从 `third-party/` 子模块编译通过，四个 crate 的测试全部通过；`aarch64-linux-android`（NDK r30）与
`aarch64-apple-ios-sim` 交叉编译通过（仅 rlib，未链接 APK/应用，未上设备）。Windows、Linux 未在本机验证。

## 已知限制

测试目标存在、通过本机测试和达到 Swift 行为等价是三个不同的结论。既有测试中存在明确记录的 `GAP` / `Named differences`：例如自适应侧栏的布局、模态焦点及动画，Table/DataTable 的字号缩放，富文本选择及编辑行为。测试固定这些差异，不代表差异已经修复。

iOS 的 UIKit WebView 与 Android WebView 原生宿主在本次任务开始时已缺失；现有 macOS、Windows、Linux WebView 实现必须保留。所有平台实现与硬件验收应单独判断，不能把配置存在或交叉编译通过写成设备验收通过。
