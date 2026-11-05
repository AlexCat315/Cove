# cove-media

第三方许可证包由原生构建器写入安装前缀的 `share/licenses/cove-media/`。静态 LGPL 的源码与应用重新链接材料见 [许可证与分发](../../Docs/media-licensing.md)。

Rust 2024 媒体控制与保留帧端点。原生播放器通过可选的 `gst` feature 使用官方 GStreamer 绑定；本 crate 不依赖 Compose、Graphics 或窗口系统。

本批实现的是原生控制、应用管线接入和 CPU 帧展示基线。`VideoView`、Browser 后端、宿主音频策略和硬件表面共享仍按 [实施文档](../../Docs/media-gstreamer.md) 后续阶段推进，没有提供占位接口。

## 构建与运行

原生版本固定为 GStreamer 1.28.8，revision `1b2a95a1b72787cda146b46df9605b168e5b683b`。Rust 绑定固定在工作区与 Cargo.lock 中。

首次使用只初始化一次构建环境：`cargo cove setup --media` 检查并按需补齐 Git、C/C++ 编译器与平台 SDK、Python、Meson、Ninja、pkg-config、Bison ≥ 2.4、Flex，并把验证过的工具路径写入用户缓存；`--check` 只报告，`--dry-run` 只输出安装计划。macOS、Windows、Linux 均有实现，交叉目标用 `--target <triple>` 追加平台 SDK 与链接器配置。

```sh
cargo cove setup --media
```

在 Cove 仓库之外开发应用时，先在 Cove 根目录执行 `cargo install --path xtask --bin cargo-cove` 取得 `cargo cove` 子命令；仓库内的 `cargo cove` 来自 `.cargo/config.toml` 别名，只在 Cove 工作目录可用。

之后是日常 Cargo 入口，不需要额外构建步骤、环境变量或 `[patch]`：

```sh
cargo test -p cove-media --features gst --all-targets
cargo test -p cove-media-rhi --features gst
cargo run -p cove-media --features gst --example headless
```

固定的官方 `*-sys` 绑定在构建脚本里调用 `cove-native-build`：首次请求时按目标完成源码准备、Meson 构建、静态链接和许可证收集，产物位于用户缓存的 `Cove/native/<指纹>/`（macOS 为 `~/Library/Caches/Cove/native/<指纹>/`，含 `configure.log`、`compile.log`、`install.log`）。`third-party/gstreamer` 子模块未初始化时按固定 revision 自行获取；并发 sys crate 由构建锁串行化，只有清单指纹改变才重建。

构建器只把本次构建的 pkg-config 目录暴露给 sys crate，选择静态链接；固定 profile 关闭文件插件注册表扫描，在构建器之外启动也不会扫描本机 SDK，显式静态注册仍可用。构建清单改变时 pkg-config 路径指纹也改变，避免 Cargo 复用旧的原生链接结果。

默认原生 profile 选定 core/app/playback、转换、测试源、AVI/WAV/MP4/Matroska 容器与平台音频/解码插件。容器插件不等于具备其所有编解码器；Linux 基础 profile 尚未包含 H.264/AAC 软件解码器，gst-libav/网络协议也未加入。应用通过 `has_element` 和实际协商结果查询能力，缺失插件以错误返回。

交叉构建在应用侧仍用 `cargo build --target <triple>`，先执行 `cargo cove setup --media --target <triple>` 准备 SDK：iOS 自动使用 Xcode SDK，Android 由 setup 安装 NDK 并写入目标链接器与 `CC_/CXX_/AR_` 变量（arm64/x86_64，API 26），其他交叉工具链通过 `COVE_CROSS_FILE_<TARGET>`（`-` 换成 `_`）提供 Meson cross 文件。Android 的链接器写入当前项目的 `.cargo/config.toml`，只补缺失项、不覆盖项目已有配置；该文件被 Cove 仓库跟踪，因此 setup 只在实际发生写入时于 JSON 报告的 `cargo_config_updated` 字段给出路径。生成配方不代表相应设备已验收。Android 在初始化前须通过 `gst::android::install_environment` 提供 JavaVM、application/class-loader 全局引用和资源所有者；静态链接的 JNI 提供函数供 androidmedia 使用。

## 播放控制

```rust,no_run
use cove_media::*;
use std::time::Duration;

let context = MediaContext::new(ContextOptions::default())?;
let player = context.create_player(PlayerOptions::default())?;
let request = player.open(MediaSource::file("clip.mov"))?;
// 仅用于非 UI 线程；UI 通过 result/events_since 观察。
let outcome = player.wait_result(request, Duration::from_secs(35))?;
player.take_result(request)?;
if let CommandOutcome::Failed(error) = outcome { return Err(error); }
let output = player.video_output();
# Ok::<(), MediaError>(())
```

提交成功只返回 `RequestId`。`result`/`wait_result` 查询终态，`take_result` 确认消费并释放请求容量；未确认的结果计入 `result_capacity`，达到容量后提交返回 `Busy`。`wait_result` 的等待超时不会取消命令；`operation_timeout` 是后端操作期限，触发时停止会话并发布失败。

`events_since(0)` 返回保留的事件；后续传回 `next_cursor`。`missed > 0` 表示有事件被覆盖，应刷新 `snapshot` 并查询未确认的请求。状态快照事件包含 EOS、错误和 Closed，命令终态独立保留。事件序号和状态 revision 在切源时持续递增。

seek 完成需要匹配 flush/segment 序号后的新 generation 样本，不以任意 `ASYNC_DONE` 作为确认。连续 seek 采用最新请求优先，被替代请求有 `Superseded` 终态。未知 PTS 保留为 `None`，仅在报告命令实际位置时可查询管线位置。rate 范围为 0.1–16，音量范围为 0–1；音量可以在打开前设置并跨切源保留。

可 seek 来源的轨道切换会重建管线输出并回到原位置预滚，确认选择、新样本与播放意图恢复后才完成；该基线允许短暂的音频间断。不可 seek 来源仅在播放态走带序号的 SELECT_STREAMS，直播矩阵尚待验收。正在切换或 seek 时收到另一条不兼容操作会返回 Busy。切源及输出重建不会强制释放已保留的旧帧。

文件、绝对 URI、`Arc<[u8]>` 和自定义 `MediaInput` 都可作为来源。自定义输入必须报告真实的字节寻址能力并合作取消，阻塞读取应定期检查 `CancellationToken`。普通播放与静音 headless 输出使用同一控制契约。

## 自有管线与视频展示

`cove_media::gst` 再导出同一套官方绑定；`GstVideoEndpoint::element()` 可连接应用拥有的管线，`with_caps` 可请求其他 raw 格式/内存。端点不控制或关闭应用管线。离线逐帧消费使用官方 appsink/appsrc 及其背压配置；最新帧展示端点会覆盖尚未展示的旧帧。

`VideoOutput::subscribe` 提供容量为 1 的合并唤醒，收到后读取一致的 `snapshot`。帧 generation、sequence、PTS/DTS、布局、色彩与完整 caps 随 `Arc<VideoFrame>` 保留；`SampleStorage::sample` 可取完整 Gst 元数据。关闭或切源拒绝迟到帧，已有帧仍持有原样本。CPU map/copy 是显式操作，硬件内存可能拒绝映射或发生下载。

`cove-media-rhi::VideoPresenter` 在 RHI 所有者线程将 RGBA8/BGRA8 上传为 `Arc<TextureImage>`，适用于现有 Metal/D3D12/Vulkan 后端。它遵循每帧实际 stride，并保留旧图像资源以支持旧视图快照；统计报告上传帧数和源平面字节数（含行填充，不是 GPU 总线测量）。PQ/HLG 明确报错，要求先做 tone mapping。未实现外部表面导入，不承诺零拷贝、HDR 或任意 YUV 直接显示。

`close` 拒绝后续控制操作，重复关闭有效；最后一个 Player 句柄析构发起非阻塞关闭。`shutdown`/`wait_closed` 用于显式宿主停机，不能在 UI 或 streaming 回调中阻塞等待。
