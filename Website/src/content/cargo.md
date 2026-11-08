# Cargo 工作流

Cove 是一组 Cargo crate。构建、运行、测试用 Cargo；打包、签名、装机用应用工程自带的 `cargo xtask`（由 `cove-package` 提供）。工具链由 `rust-toolchain.toml` 固定。

## 创建工程

```bash
cargo install cargo-generate
cargo generate --git https://github.com/AlexCat315/Cove.git Templates/app --name my-app
```

生成的工程包含 `src/`（共享 UI，`cove_ui::application!` 声明根视图）、`Assets/`、`Platforms/`（Android Gradle、macOS/Windows/Linux 平台文件）与 `xtask/`。应用 ID、显示名与构建号写在 `Cargo.toml` 的 `[package.metadata.cove]`。

## 开发

```bash
cargo run
cargo test
COVE_DEVTOOLS=1 cargo run
```

## 打包

```bash
cargo xtask package                     # 当前桌面主机：.app / 可执行目录
cargo xtask package android --release   # APK
cargo xtask package ios --simulator     # 模拟器 .app
cargo xtask run ios --simulator
cargo xtask run android --device SERIAL
cargo xtask devices android
```

| 平台 | 产物 | 需要 |
| --- | --- | --- |
| macOS | `<Name>.app`，ad-hoc 签名；`--identity` 用开发者证书并开启 hardened runtime | Xcode 命令行工具 |
| Windows | 可执行目录，`mt.exe` 嵌入 `Application.manifest` | Windows SDK |
| Linux | 可执行目录与 `.desktop` | — |
| Android | `cdylib` → `libCoveAndroidApp.so`，Gradle 出 APK；Release 默认用调试密钥，`--keystore --key-alias --store-password-env --key-password-env` 指定发布密钥 | NDK r30（`ANDROID_NDK_HOME`）、Android SDK 36、JDK 21 |
| iOS | Rust 二进制直接组成 `.app`，无需 Xcode 工程；真机需 `--identity` 与 `--provisioning-profile` | Xcode |

桌面产物需在对应主机上构建。产物默认位于 `target/cove/<platform>/<mode>/`，`--output` 可指定目录。Android Studio 直接打开 `Platforms/Android`，每次构建由 Gradle 调用 `cargo xtask android-prepare`。

## 从源码开发 Cove

```bash
git clone --recurse-submodules https://github.com/AlexCat315/Cove.git
cd Cove
cargo test --workspace -- --test-threads=1
cargo xtask run --package cove-examples
cargo xtask audit --json
```
