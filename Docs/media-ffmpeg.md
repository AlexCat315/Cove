# Cove 音视频：FFmpeg 集成

适用范围：`cove-media` 与 `cove-media-rhi` 的 `ffmpeg` feature。Cove 自有代码采用 Apache-2.0；FFmpeg 与 Rust 依赖保留各自许可，分发材料见 [媒体许可证与分发](media-licensing.md)。

## 1. 边界

- 核心控制与保留帧输出不依赖 UI、GPU 或窗口系统；`cove-media` 的默认构建不引入 FFmpeg。
- 启用 `ffmpeg` 后，播放、解复用、解码、重采样与像素转换全部由 FFmpeg 完成；音频设备输出使用 `cpal`。
- `cove-media-rhi` 只负责把 CPU 帧上传为 Cove RHI 纹理，不包含解码逻辑。
- Wasm 目标不编译媒体后端（`ffmpeg-sys-next` 与 `cpal` 均不适用于 `wasm32`），相关 API 在 `cfg` 层排除。

## 2. 依赖

| 组件 | 版本 | 说明 |
| --- | --- | --- |
| `ffmpeg-sys-next` | `=9.0.1` | features：`build`、`avcodec`、`avfilter`、`avformat`、`swresample`、`swscale`；`build` 在构建脚本内从源码编译静态 FFmpeg |
| `cpal` | `0.18.2` | 无默认 features；提供系统默认音频设备输出 |
| `libc` | `0.2` | 自定义 AVIO 与 errno 映射 |

FFmpeg 构建保持 `--disable-autodetect`、`--disable-programs`、`--disable-doc`，并关闭 `gpl`、`version3`、`nonfree`，因此产物为 LGPL-2.1-or-later 的静态库集合，不包含 GPL 编码器或外部库。库文件位于 Cargo 的 `OUT_DIR`，不写入共享前缀，也没有 pkg-config 或环境变量注入。

首次构建需要联网：构建脚本以 `git clone --depth=1 -b release/9.0` 获取 FFmpeg 源码到 `OUT_DIR`，之后 `configure`、`make`、`make install` 都在同一目录完成。

## 3. 模块

```
crates/media/src/
  engine/
    mod.rs      Abort、初始化、版本与解码器查询、错误文本
    io.rs       自定义 AVIO：内存/自定义 MediaInput 与取消
    session.rs  解复用、解码、时钟、seek、轨道选择、帧发布
    audio.rs    cpal 输出、采样队列、主时钟
    video.rs    swscale RGBA8 转换与 FrameStorage
    pump.rs     每播放器一条泵线程与命令枚举
    decoder.rs  MediaDecoder：调用线程上的同步解复用/解码与 probe
  native.rs     MediaContext / Player 控制层、请求期限与关闭
  ffmpeg.rs     原始 FFmpeg 绑定与 FrameStorage 再导出
```

所有原始 `AVFormatContext`、`AVCodecContext`、`AVFrame`、`AVPacket`、`SwrContext`、`SwsContext` 只在泵线程内创建和释放。控制线程只负责路由命令、保存请求结果并执行操作期限，不直接触碰 FFmpeg 句柄。

## 4. 公共接口

`MediaContext::new(ContextOptions)` 初始化后端并启动控制线程；`create_player(PlayerOptions)` 创建独立播放器；`has_decoder(name)` 查询编解码器；`backend_version()` 返回 `av_version_info()`；`shutdown()` 与 `wait_closed(timeout)` 用于宿主停机。

`Player` 的提交型方法返回 `RequestId`，不阻塞 UI：

- `open(MediaSource)`：文件、绝对 URI、`Arc<[u8]>` 或自定义 `MediaInput`。
- `play`、`pause`、`seek(MediaTime)`、`select_tracks`、`set_rate`、`set_volume`、`close`。
- `result`、`take_result`、`wait_result` 查询并消费请求终态；未消费结果计入 `result_capacity`，达到容量后提交返回 `Busy`。
- `events_since(cursor)` 返回保留事件与下一个游标；`snapshot()` 返回一致的状态快照。
- `video_output()` 返回 `VideoOutput`，订阅容量为 1 的最新帧唤醒，或用 `on_change` 注册回调。
- `on_change(callback)` 在快照变化与命令完成后于媒体线程回调；`audio_meter()` 返回实际送入设备的电平与采样历史；`detach(request)` 让结果完成即丢弃，分离的请求不计入 `result_capacity`。
- `Player` 实现 `PlaybackControl`，可直接交给 `VideoPlayer` / `AudioPlayer`。

同步底层接口：

- `MediaDecoder::open(source, DecoderOptions)` 在调用线程解复用与解码；`next_frame`/`next_video`/`next_audio`、`frame_at`、`read_audio`、`seek`；`video_size` 按比例缩放，`audio_format` 重采样为交错 `f32`；`cancellation_token()` 可从其他线程中止阻塞读取。`probe(source)` 只读描述。
- `audio_output_devices()` 枚举默认音频主机的输出设备，`AudioOutput::Device(id)` 选择设备，不可用时回退默认设备。
- `AudioStream::open(AudioStreamOptions, render)` 播放应用渲染的 PCM；cpal 流在专用线程上创建与持有。

`operation_timeout`（默认 30 秒）是后端操作期限，不是 `wait_result` 的等待超时。期限触发时控制层中止泵线程、把状态置为 `Failed` 并以 `Timeout` 完成未决请求；显式 `close` 则立即中止操作并进入 `Closed`，因此卡住的打开也能被关闭。

## 5. 时间与播放

- 时间统一为 `MediaTime`（微秒），来自流的 `time_base` 与 `AVFrame` 的 PTS/DTS。未知 PTS 保持 `None`。
- 音频主时钟由 cpal 回调按已播放采样推进；没有可用设备或 `AudioOutput::Silent` 时使用单调墙钟。
- seek 使用 `av_seek_frame` 加解码器 `flush_buffers`，完成条件是位置之后的新帧，而不是任意解码器状态。
- rate 范围 0.1–16，音量 0–1；音量可在打开前设置并跨切源保留。
- 视频帧在泵线程完成 `sws_scale` 到 RGBA8，按实际 stride 发布；`FrameStorage` 暴露平面字节与行填充。HDR PQ/HLG 帧由 RHI 上传层拒绝，要求先做 tone mapping。

## 6. 构建环境

```sh
cargo cove setup --media
cargo test --locked -p cove-media --features ffmpeg --all-targets
cargo test --locked -p cove-media-rhi --features ffmpeg
```

`cargo cove setup --media` 检查并按需安装：

- 通用：Git、CMake、Ninja、C/C++ 编译器与平台 SDK。
- FFmpeg：`make` ≥ 3.81、`nasm` ≥ 2.13、`clang`（bindgen 使用其 libclang）。
- Linux：`pkg-config`、ALSA 开发包（`cpal` 的 ALSA 后端需要），以及 `clang`/`libclang`。
- Windows：MSYS2 的 `make`、`nasm`、`clang`，以及 `sh.exe`。
- Apple：完整 Xcode 仅在 iOS 目标需要；macOS 使用 Command Line Tools。

`--check` 只报告，`--dry-run` 只输出安装计划。Android 目标由 setup 写入 NDK 编译器与 `CC_/CXX_/AR_`；`ffmpeg-sys-next` 要求这些变量指明交叉工具链（与 `cargo-ndk` 提供的变量一致）。

## 7. 平台状态

| 平台 | 实现 | 验证 |
| --- | --- | --- |
| macOS | `videotoolbox` 未启用；软件解码与 CoreAudio 输出 | 本机编译与测试 |
| iOS | 交叉编译通过 `ffmpeg-sys-next` 的 Apple sysroot 路径 | 仅实现，未设备验收 |
| Windows | MSVC + MSYS2 构建 FFmpeg，WASAPI 输出 | CI 任务，暂标 `continue-on-error` |
| Android | NDK 交叉编译，`AAudio`/`cpal` 输出 | 仅实现，未设备验收 |
| Linux | 软件解码，ALSA 输出 | CI 任务 |
| Wasm | 不提供媒体后端 | 不适用 |

## 8. 已知边界

- 尚未启用硬件解码（VideoToolbox/MediaCodec/D3D11VA/VAAPI）与零拷贝外部表面导入；所有帧先下载为 CPU 数据。
- 未实现播放列表、字幕轨渲染与音轨混音；播放中切换输出设备需要重新创建 `Player`。
- 帧格式只输出 RGBA8；YUV 直接展示与 HDR 元数据传递不在当前范围。帧数据由 `sws_scale` 直接写入共享分配，`VideoView` 与 `VideoPresenter` 借用该分配上传，不做 CPU 拷贝。
- 网络协议由 FFmpeg 自带实现启用，但未对每个协议做端到端验收。
