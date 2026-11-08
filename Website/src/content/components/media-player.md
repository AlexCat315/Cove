# 音视频组件

`cove_ui_compose::primitives::media` 提供 `VideoPlayer`、`AudioPlayer`、`MediaTransport`、`VideoView`、`AudioLevelMeter` 与 `Waveform`。组件只依赖 `cove_media::PlaybackControl` 契约，不绑定具体后端：启用 `cove-media/ffmpeg` 时传入 `Player`，也可以传入应用自己实现的播放源。

```rust
use std::sync::Arc;
use cove_ui_compose::primitives::media::{AudioPlayer, VideoPlayer, cove_media::*};

let context = MediaContext::new(ContextOptions::default())?;
let player = context.create_player(PlayerOptions::default())?;
let request = player.open(MediaSource::file("clip.mp4"))?;
player.detach(request)?; // 结果完成即丢弃，不占 result_capacity

let control: Arc<dyn PlaybackControl> = Arc::new(player);
let video = VideoPlayer::new(control.clone());
let audio = AudioPlayer::new(control).title("Clip").subtitle("Artist");
```

## 组件

`VideoPlayer::new(control)` 是视频画面加下方控制条。`.shows_controls(false)` 只留画面，`.content_mode(ImageContentMode::Fill)` 裁切填满，`.corner_radius(r)` 设置圆角（默认 8）。

`AudioPlayer::new(control)` 是紧凑卡片：标题、副标题、实时电平与控制条。`.shows_meter(false)` 隐藏电平；控制源不提供 `audio_meter()` 时自动隐藏。

`MediaTransport::new(control)` 是独立控制条：播放/暂停、已播放时间、进度条、总时长、静音与音量。播放结束后点击播放会从头开始。拖动进度条时发送关键帧 seek 预览，松手发送一次精确 seek；seek 落地前进度条停在松手位置。没有时长或来源不可 seek 时进度条禁用。`.shows_volume(false)` 隐藏音量，`.enabled(false)` 整体禁用。

`VideoView::new(Some(output))` 只呈现 `VideoOutput` 的最新帧，按像素宽高比计算显示尺寸，默认 `Fit` 并以黑色填充留白（`.background(None)` 透明）。未收到帧前用 `.placeholder_aspect(16.0 / 9.0)` 参与布局。

`AudioLevelMeter::new(meter)` 每个声道一条电平条，-60 dBFS 到 0 dBFS，按 success / warning / error 分段着色；电平上升即时、下降 30 dB/s。`.active(false)` 停止逐帧刷新并回落到静音；减少动态效果时也不刷新。

`Waveform::new(peaks)` 绘制包络柱，`.progress(f)` 给已播放部分着 accent 色，`.on_seek(|fraction| ..)` 在按下与拖动时回报指针位置的比例。峰值通常来自 `MediaDecoder::read_audio(None)?.peaks(n)`，绘制时按列宽重新取样。

`use_playback(cx, &control)` 在任意 `View::body` 中返回控制源的最新 `PlayerSnapshot` 并在变化时重组，适合自定义控制界面。

## 线程与性能

控制源的变更回调运行在媒体线程上，组件只置位原子标记并向 UI 线程投递至多一个待处理刷新，因此重组频率等于后端上报频率，UI 线程从不等待播放器。视频帧同理：UI 线程落后时直接取最新帧，不排队。

每个 `VideoView` 用一个稳定的纹理 id 提交帧，渲染器把同 id 的新所有者视为流式纹理，就地更新同一个 GPU 纹理槽。RGBA8 帧在 `VideoStorage::shared_plane` 可借出时不做 CPU 拷贝，从 FFmpeg 缩放输出直达上传。

## 底层 API（`cove-media`）

始终可用（不需要 FFmpeg）：

- `PlaybackControl`：组件驱动的播放契约；`ChangeNotifier` / `ChangeListener` 供自定义实现发出变更通知。
- `VideoOutput::on_change`：帧到达回调；`VideoFrame::rgba_pixels()` 返回紧密排列的 RGBA8（可共享时零拷贝）；`SharedPlanes` 用于应用自产帧。
- `AudioMeter`：实时回调安全（`try_lock`，从不阻塞设备）的电平与采样历史，`record` / `levels` / `history`。
- `AudioBuffer`、`AudioFormat`、`audio_peaks`、`sample_stats`：PCM 与波形计算；样本统计内核在 AArch64（NEON）与 x86-64（SSE）上为内联汇编，其余目标使用标量实现。

启用 `ffmpeg` feature 后：

- `MediaDecoder`：在调用线程同步解复用与解码。`next_frame` / `next_video` / `next_audio` 逐帧读取，`frame_at(t)` 取缩略图，`read_audio(limit)` 解码为一个 PCM 缓冲；`DecoderOptions::video_size` 按比例缩放，`audio_format` 重采样。`cancellation_token()` 可从其他线程中止阻塞读取。
- `probe(source)`：读取容器、时长、码率、元数据（标题/艺术家等）与每条流的编码、分辨率、帧率、采样率、声道，不打开解码器。
- `audio_output_devices()` 枚举输出设备，`AudioOutput::Device(id)` 选择设备；设备消失时回退到默认设备。
- `AudioStream::open(options, render)`：应用渲染 PCM 到输出设备。设备流运行在独立线程，句柄在所有平台上都是 `Send + Sync`；默认暂停，暂停时输出静音。
- `Player::on_change`、`Player::audio_meter`、`Player::detach`，以及 `Player` 对 `PlaybackControl` 的实现。

共享 Rust 实现覆盖 macOS、Windows、Linux、iOS 与 Android；Wasm 目标不提供 FFmpeg 后端，但组件与 `PlaybackControl` 可用。自动化结构验证与原生视觉、交互验收分别记录在 Gallery 验收表中。
