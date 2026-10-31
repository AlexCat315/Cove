# Cove

跨平台 UI 框架，正在从 Swift 全量迁移到 Rust 2024。当前 Rust 工作区是迁移中的基础层，**还不能替代原生 UI SDK**。原有 Swift 源码保留用于逐项迁移与行为对照。构建、运行与测试使用 Cargo；打包、签名与装机使用 `cargo xtask`（`Tools/Package`）。

## 源码开发

需要 Rust 1.99.1 和 C 编译器；工具链由 `rust-toolchain.toml` 固定。`Cargo.lock` 固定依赖解析。

```sh
cargo build --workspace
cargo run --manifest-path Examples/RustCounter/Cargo.toml
python3 Tools/Migration/inventory.py --status
```

新建应用工程与打包：

```sh
cargo generate --git https://github.com/AlexCat315/Cove.git Templates/app --name my-app
cargo xtask package android --release --package cove-mobile-example
cargo xtask run ios --simulator --package cove-mobile-example
```

详见 [Cargo 工作流](Website/src/content/cargo.md)。

实现完成后统一验证：

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked -- --test-threads=1
```

## 当前迁移范围

| 能力 | Rust 实现 |
| --- | --- |
| Core | 几何、颜色、共享绘制缓冲区、C 几何内核、位图与 C mipmap、状态订阅、弹簧、字号缩放 |
| PlatformCore | 窗口 ID、键盘/鼠标/触摸/游戏手柄事件、滚动单位、菜单数据、资源查找 |
| System | 原子文件替换、权限、文件身份与锁、恢复目录与 macOS 回收站、进程流/限额/超时/取消、Windows Job Object |
| Logging | 过滤、递归脱敏、有界历史、UTF-8 截断、JSONL 文件轮转 |
| DevTools/Protocol | 消息、树/状态/时间线数据、检查请求校验、会话、输入录制；Rust 协议 0.2 |
| Agent | JSON Schema 子集、工具与工具循环、skills、MCP 服务端（动态工具）、stdio/本地 TCP 传输；不依赖 UI |
| Http | 阻塞式 HTTP 客户端、流式响应体：NSURLSession、WinHTTP、HTTP/1.1 + OpenSSL |

`RustCounter` 是无窗口状态示例。Gallery、原生 UI Runtime、完整 Core 场景与布局、Compose、Text、Imaging、Graphics、Shell、终端、WebView、Workspace、浏览器及移动宿主、SDK 安装/打包/发布仍待迁移。详见 [迁移清单](Docs/rust-migration.md)。

macOS、Windows、Linux、iOS、Android 的共享基础层有对应平台代码，尚未完成统一验证。平台配置或代码存在不代表平台验收通过。

## 发布状态

上一个 Swift SDK 为 0.2.13。Rust 工作区版本为 `0.3.0-dev`，尚未发布；发布以 git tag 形式提供 Cargo 依赖，版本取自根 `Cargo.toml` 的 `[workspace.package]`。发布变更记入 [CHANGELOG.md](CHANGELOG.md)。
