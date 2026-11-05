# cove-media

Rust 2024 媒体控制与保留帧端点。原生播放器由可选的 `ffmpeg` feature 提供；本 crate 不依赖 Compose、Graphics 或窗口系统。完整设计、平台状态与已知边界见 [FFmpeg 集成文档](../../Docs/media-ffmpeg.md)，许可证与分发材料见 [媒体许可证与分发](../../Docs/media-licensing.md)。

## 构建与运行

FFmpeg 9.0 由 `ffmpeg-sys-next` 的构建脚本从源码静态编译到 Cargo 的 `OUT_DIR`，保持 LGPL profile（`gpl`、`version3`、`nonfree` 关闭），不写入共享前缀，也不需要 pkg-config 或额外环境变量。首次构建需要网络获取 FFmpeg 源码。

首次使用只初始化一次构建环境：`cargo cove setup --media` 检查并按需补齐 Git、CMake、Ninja、C/C++ 编译器与平台 SDK，以及 `make`、`nasm`、`clang`（bindgen 使用 libclang）；Linux 额外安装 pkg-config 与 ALSA 开发包。`--check` 只报告，`--dry-run` 只输出安装计划。macOS、Windows、Linux 均有实现，交叉目标用 `--target <triple>` 追加平台 SDK 与链接器配置。

```sh
cargo cove setup --media
cargo test -p cove-media --features ffmpeg --all-targets
cargo test -p cove-media-rhi --features ffmpeg
```

在 Cove 仓库之外开发应用时，先在 Cove 根目录执行 `cargo install --path xtask --bin cargo-cove` 取得 `cargo cove` 子命令；仓库内的 `cargo cove` 来自 `.cargo/config.toml` 别名，只在 Cove 工作目录可用。

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

seek 完成需要位置之后的新解码帧，不以任意解码器状态作为确认；连续 seek 采用最新请求优先，被替代请求有 `Superseded` 终态。未知 PTS 保留为 `None`。rate 范围为 0.1–16，音量范围为 0–1；音量可以在打开前设置并跨切源保留。

可 seek 来源的轨道切换会回到原位置重新预滚，确认新选择与播放意图恢复后才完成。正在切换或 seek 时收到另一条不兼容操作会返回 `Busy`。切源及输出重建不会强制释放已保留的旧帧。

文件、绝对 URI、`Arc<[u8]>` 和自定义 `MediaInput` 都可作为来源。自定义输入必须报告真实的字节寻址能力并合作取消，阻塞读取应定期检查 `CancellationToken`。普通播放与静音输出使用同一控制契约。

## 视频展示

`VideoOutput::subscribe` 提供容量为 1 的合并唤醒，收到后读取一致的 `snapshot`。帧 generation、sequence、PTS/DTS、布局与色彩信息随 `Arc<VideoFrame>` 保留；`FrameStorage` 提供 RGBA8 平面字节与行填充。关闭或切源拒绝迟到帧，已有帧仍持有原数据。

`cove-media-rhi::VideoPresenter` 在 RHI 所有者线程将 RGBA8 上传为 `Arc<TextureImage>`，适用于现有 Metal/D3D12/Vulkan 后端。它遵循每帧实际 stride，并保留旧图像资源以支持旧视图快照；统计报告上传帧数和源平面字节数（含行填充，不是 GPU 总线测量）。PQ/HLG 明确报错，要求先做 tone mapping。未实现外部表面导入，不承诺零拷贝、HDR 或任意 YUV 直接显示。

`close` 拒绝后续控制操作，重复关闭有效；最后一个 Player 句柄析构发起非阻塞关闭。`shutdown`/`wait_closed` 用于显式宿主停机，不能在 UI 或 streaming 回调中阻塞等待。
