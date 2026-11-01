# Cove Browser

Rust WASI reactor，使用 WebGPU；不可用时回退 Canvas2D。导出 C ABI，供 `web/app.js` 调用。

需要 Rust 的 `wasm32-wasip1` target、wasi-sdk 和 Node.js。`COVE_WASI_SDK_PATH` 指向 wasi-sdk 安装目录。

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
