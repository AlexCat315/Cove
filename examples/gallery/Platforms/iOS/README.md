# iOS Gallery

复用 Gallery 内容与 Application 宿主。iOS 由 `cargo xtask package ios --simulator --package cove-examples` / `cargo xtask run ios --simulator --package cove-examples` 直接打包 Rust 二进制为 `.app`，无需 Xcode 工程；目录内残留的 `.xcodeproj` 是 Swift 时代遗留，不参与构建。

启动参数 `--component button` 等可直接选择组件故事。UI 的布局跟随 surface 尺寸，输入使用触摸密度。
