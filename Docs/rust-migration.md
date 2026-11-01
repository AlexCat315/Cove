# Rust 迁移状态

2026-11-01；版本 `0.3.0-dev`，未发布。

工作区已统一为 Rust 2024 / Cargo 目录。库位于 `crates/`，源码、集成测试、原生代码与资源分别位于 `src/`、`tests/`、`native/`、`resources/`；构建脚本位于 crate 根目录。示例位于 `examples/`，打包与审计工具位于 `xtask/`。

[逐文件 JSON 清单](rust-migration-files.json)保留 1,348 个源文件的原路径、SHA-256、行数和实际目标。它是来源记录，不是行为等价证明；删除源文件不会删掉对应行。`cargo xtask audit --json` 检查所有目标仍存在、Cargo 入口使用标准目录，且自有源码中没有 Swift 或 SwiftPM 清单。

[第三批分组](test-migration-partition-3.md)列出的 65 个测试目标已写入；补入了原分组遗漏的设计系统套件。[验证报告](rust-migration-validation.md)记录实际执行结果及未消除的行为差异。当前没有任何平台仅凭路径存在就被标为验收通过。
