# iOS Gallery

复用 Gallery 内容与 Application 宿主。Xcode 工程尚未改为链接 Cargo 产出的 `staticlib`，当前不能直接构建；共享代码可用 `cargo build --lib --target aarch64-apple-ios-sim` 检查。

启动参数 `--component button` 等可直接选择组件故事。UI 的布局跟随 surface 尺寸，输入使用触摸密度。
