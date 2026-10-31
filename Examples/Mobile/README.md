# Cove 移动示例

共享内容在 `Examples/rust/mobile`（`cove-examples` crate）。

```sh
cargo test -p cove-examples
COVE_DEVTOOLS=1 cargo run   # 在宿主应用内启用 DevTools
```

`Platforms/Android` 与 `Platforms/iOS` 仍是 Swift 时期的原生工程：Gradle 与 Xcode 尚未改为调用 Cargo 产出 `cdylib` / `staticlib`，当前不能打包 APK 或 `.app`。
