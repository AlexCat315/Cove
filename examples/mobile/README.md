# Cove 移动示例

共享内容在 `examples/gallery/src/mobile`；本 crate（`cove-mobile-example`）用 `cove_ui::application!` 把它接到各平台宿主。

```sh
cargo run -p cove-mobile-example
cargo xtask package android --package cove-mobile-example
cargo xtask run ios --simulator --package cove-mobile-example
cargo xtask package mac --package cove-mobile-example
```

`Platforms/Android` 可直接用 Android Studio 打开（先运行一次 `cargo xtask android-prepare --package cove-mobile-example`）。
