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
let video = VideoPlayer::new(control.clone()).title("Clip").danmaku(DanmakuFeed::new());
let audio = AudioPlayer::new(control).title("Clip").subtitle("Artist");
```

## 组件

`VideoPlayer::new(control)` 是主流播放器形态：进度条与控制栏叠在画面底部的渐变上，播放时静止 `autohide` 秒（默认 2.5）后隐藏，指针移动或按键立即显示，暂停时常显。进度条静止时细、悬停变粗并显示拇指与悬停时间，按下拖动时发送关键帧 seek 预览，松手发送一次精确 seek。控制栏依次为播放/暂停、静音与音量、`已播放 / 总时长`、弹幕开关、设置、画中画、全屏；暂停或结束时画面中央显示大播放按钮，打开或 seek 中显示加载指示。单击画面切换播放，双击切换全屏。未收到帧前按 16:9 布局。

- `.title(...)`：控制显示时出现在画面顶部。
- `.danmaku(feed)`：启用弹幕层、弹幕开关与设置中的弹幕选项。
- `.settings(PlayerSettings { content_mode, danmaku })`：初始设置，之后由观众在设置菜单中修改。
- `.allows_fullscreen(false)`、`.allows_picture_in_picture(false)`：隐藏对应按钮。
- 键盘（点击画面获得焦点后）：空格 / K 播放暂停，←/→ 前后 5 秒，↑/↓ 音量，F 全屏，M 静音，D 弹幕，P 画中画，Esc 回到页面内。

设置菜单包含倍速（0.5×–2×）、画面（Fit / Fill / Stretch）、弹幕（不透明度、速度、字号、显示区域），来源有多条音轨时还有音轨选择（`PlaybackControl::select_tracks`）。

全屏把同一个画面放进覆盖整个窗口的 portal，并通过 `WindowPresentation` 请求宿主把窗口切到全屏（macOS 为独立 Space）；在系统层面退出全屏也会回到页面内。画中画通过 `WindowPresentation::open_floating` 打开一个置顶的小窗口（macOS 可跨 Space 与全屏应用），关闭该窗口即回到页面内；没有宿主支持时（移动端、测试）画面在当前窗口右下角浮动。播放器原位置显示提示与“Return”按钮。

`AudioPlayer::new(control)` 是紧凑卡片：标题、副标题、实时电平与控制条。`.shows_meter(false)` 隐藏电平；控制源不提供 `audio_meter()` 时自动隐藏。

`MediaTransport::new(control)` 是独立控制条（`AudioPlayer` 使用）。`MediaProgressBar::new(position, duration)` 是视频内的进度条，可单独使用：`.on_scrub(|seconds, committed| ..)`、`.on_hover(|seconds| ..)`、`.buffered(fraction)`。

`VideoView::new(Some(output))` 只呈现 `VideoOutput` 的最新帧，按像素宽高比计算显示尺寸，默认 `Fit` 并以黑色填充留白（`.background(None)` 透明）。

`AudioLevelMeter::new(meter)` 每个声道一条电平条，-60 dBFS 到 0 dBFS，按 success / warning / error 分段着色；电平上升即时、下降 30 dB/s；`.active(false)` 时立即归零。

`Waveform::new(peaks)` 绘制包络柱，默认按最响一列归一化（`.normalized(false)` 保持原始幅度），`.progress(f)` 着色已播放部分，`.on_seek(|fraction| ..)` 在按下与拖动时回报比例。峰值通常来自 `MediaDecoder::read_audio(None)?.peaks(n)`。

`use_playback(cx, &control)` 在任意 `View::body` 中返回控制源的最新 `PlayerSnapshot` 并在变化时重组；播放中位置每 50ms 上报一次，只依赖状态、倍速、音量、音轨的视图应改用 `use_playback_select(cx, &control, steady_snapshot)` 或自定义投影，只在投影变化时重组，避免把菜单等浮层随位置一起重建。

## 弹幕

`DanmakuFeed` 按时间保存 `DanmakuComment { time, text, color, mode: Scroll | Top | Bottom, is_own }`；`push` 返回 id，`DanmakuFeed::parse_bilibili_xml` 导入 Bilibili XML（`<d p="时间,模式,字号,颜色,...">`，模式 1–3 滚动、4 底部、5 顶部）。`send_danmaku(&feed, control, text, color)` 在当前播放位置发送一条带描边的自有弹幕。

弹幕在画面上的位置只由媒体时间决定：暂停时静止，seek 后从新位置开始（跳过的弹幕不会一次性涌出），倍速播放时同步加快。滚动弹幕按车道分配，前一条完全进入画面且不会被追尾时才复用车道；显示区域满时丢弃而不重叠。顶部/底部弹幕停留 4 秒。`DanmakuScheduler` 是不含 UI 状态的调度器，可单独测试或用于自定义渲染。

## 来源

`MediaSource::file(path)`、`MediaSource::uri(url)`、`MediaSource::bytes(..)` 或自定义 `MediaInput`。`http://` 与 `https://`（`.m3u8` 播放列表除外）经 `HttpInput` 读取：使用 `cove-http` 的原生 TLS（Foundation、WinHTTP、其他平台 OpenSSL），服务器返回 `206` 时可 seek，`200` 时顺序读取。`HttpInput::new(url).header(..)` 可附加鉴权头后作为 `MediaSource::Input` 传入。本地文件可用 `FilePicker`（原生打开面板）或 `FileDropZone`（拖放）获取。

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
- `Player::on_change`、`Player::audio_meter`、`Player::detach`，以及 `Player` 对 `PlaybackControl` 的实现（含 `select_tracks`）。
- `HttpInput`：HTTP(S) 字节范围输入。

窗口能力（`cove_ui_compose::core::WindowPresentation`）：`set_fullscreen`、`is_fullscreen`、`open_floating(FloatingWindowRequest)`、`close_floating`、`is_floating_open`。桌面宿主通过 `Shell::set_window_fullscreen` / `set_window_always_on_top` 实现（SDL3，macOS 使用 AppKit 全屏与浮动窗口层级）；其他宿主可安装自己的 `WindowPresenter`。

共享 Rust 实现覆盖 macOS、Windows、Linux、iOS 与 Android；Wasm 目标不提供 FFmpeg 后端，但组件与 `PlaybackControl` 可用。自动化结构验证与原生视觉、交互验收分别记录在 Gallery 验收表中。
