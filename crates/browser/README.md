# Cove Browser

Rust WASI reactor，默认通过 Cove RHI 的 WebGPU 后端运行共享 DrawList 渲染器。浏览器负责异步申请设备和完成回调。没有 WebGPU 时使用 Canvas2D。

三平台通用的准备流程（macOS / Linux / Windows）。`cargo cove setup --wasm` 会下载并按 SHA-256 校验固定版本 wasi-sdk 34，确认可用的原生 Ninja，并把路径写入 Cove 的 setup 状态；构建脚本读取同一份状态，无需手动 export 环境变量、手工修改 SDK 或指定 CMake 生成器。

```sh
cargo cove setup --wasm
rustup target add wasm32-wasip1
cd crates/browser
npm ci
npm run build
npm run serve
```

`COVE_WASI_SDK_PATH` 仍可显式覆盖（例如使用系统自带的 SDK）。`cove-native-build` 会为 wasm target 强制 Ninja 生成器并生成工具链包装文件，避免 Windows 上默认 Visual Studio 生成器把 C/C++ 依赖编译成 x86。

默认监听 `127.0.0.1:8080`。`COVE_WASM_CONFIGURATION=release` 生成优化版本；`COVE_WASM_OUTPUT` 可调整输出目录。字体和许可证位于 [`../text/fonts`](../text/fonts/README.md)。

默认打开 Counter。用 `?component=scrollView` 或 `?component=textFields` 打开 Gallery 对应页面。画布内可用滚轮和触控板滚动；点击可编辑字段后可使用浏览器键盘和输入法。

```sh
npm run verify
npm run test:wasm
```

`verify` 检查前端脚本语法。`test:wasm` 实例化构建后的模块，验证 Counter 和 Gallery 的输入路由及状态变化。浏览器绘制、GPU、系统键盘和 IME 仍需实机验证。

用 `cargo run -p cove-devtools-cli -- serve --web-root crates/browser/dist` 启动控制器，再打开终端打印的 URL，将路径改为 `/app/` 并保留配对片段。Inspector、MCP 和 CLI 随即可以选择 WASM 运行时。浏览器截图与操作需要配对 WebDriver 会话，见 [DevTools](../devtools/README.md)。

UI 着色器来自 `crates/graphics/resources/Shaders/Slang`。修改源码后，在仓库根目录运行 `node Tools/Shaders/build-native-ui.mjs --wgsl`，使用 `cargo cove setup --shaders` 配置的 Slang 2026.19 重新生成浏览器产物。
