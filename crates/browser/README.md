# Cove Browser

Rust WASI reactor，默认通过 Cove RHI 的 WebGPU 后端运行共享 DrawList 渲染器。浏览器负责异步申请设备和完成回调。没有 WebGPU 时使用 Canvas2D。

需要 Rust 的 `wasm32-wasip1` target、wasi-sdk 34 和 Node.js。`COVE_WASI_SDK_PATH` 指向 wasi-sdk 安装目录。

```sh
rustup target add wasm32-wasip1
npm ci
npm run build
npm run serve
```

默认监听 `127.0.0.1:8080`。`COVE_WASM_CONFIGURATION=release` 生成优化版本；`COVE_WASM_OUTPUT` 可调整输出目录。字体和许可证位于 [`../text/fonts`](../text/fonts/README.md)。

```sh
npm run verify
```

此命令检查前端脚本语法；浏览器端的渲染、输入和 GPU 行为需在浏览器中验证。

UI 着色器来自 `crates/graphics/resources/Shaders/Slang`。修改源码后，在仓库根目录运行 `node Tools/Shaders/build-native-ui.mjs --wgsl`，使用 `cargo cove setup --shaders` 配置的 Slang 2026.19 重新生成浏览器产物。
