# Cargo 工作流

Cove 是一组 Cargo crate，构建、运行、测试全部用 Cargo，没有单独的 CLI。工具链由仓库的 `rust-toolchain.toml` 固定。

## 依赖

```toml
[dependencies]
cove-ui = { git = "https://github.com/AlexCat315/Cove.git", tag = "vX.Y.Z" }
```

本地开发源码用 `path = "/path/to/Cove"` 替代 `git`。

## 常用命令

```bash
cargo run                                   # 当前主机
cargo test --workspace -- --test-threads=1  # 测试
cargo build --release
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

## 交叉编译

```bash
rustup target add aarch64-linux-android aarch64-apple-ios aarch64-apple-ios-sim
cargo build --lib --target aarch64-linux-android   # 需要 ANDROID_NDK_HOME（NDK r30）
cargo build --lib --target aarch64-apple-ios
```

原生依赖（SDL3、FreeType、HarfBuzz 等）由各 crate 的 `build.rs` 通过 CMake 构建，需要 CMake 与 C 编译器。

## 从源码开发

```bash
git clone --recurse-submodules https://github.com/AlexCat315/Cove.git
cd Cove
cargo test --workspace -- --test-threads=1
python3 Tools/Migration/inventory.py --status
```
