# Changelog

每个 Cove SDK 版本对应一个 `vX.Y.Z` 标签，以 Cargo git 依赖形式提供。

## Unreleased

- **平台自适应 P0：平台、设备类别、折叠姿态与铰链进入环境（2026-11-09，破坏性）**：
  - 设计：新增 `Docs/adaptive-platform.md`（四个正交维度、语义组件、跨铰链规则、形态变化时的状态连续性、分阶段计划）；本条为其中 P0。
  - `cove-core`：删除 `Idiom` 与 `SizeClass`。`Environment` 新增 `platform`（`MacOS | Windows | Linux | IOS | IPadOS | Android | Web`）、`design_language`（`Cupertino | Material | Fluent | Adwaita`）、`device_class`（`Phone | Foldable(Book | Flip | DualScreen) | Tablet | Desktop`）、`posture`（`Flat | Tabletop | Book`）、`display_features`（折痕/铰链的窗口坐标、遮挡、分隔），以及 `segments()`、`is_spanned()`、`segment_for()`、`occluded_regions()`；`display_features::confine` 把弹层移入区段。宽度改为 Material 3 五档 `WidthClass`（600/840/1200/1600，可比较），高度为 `HeightClass`（480/900）；`Viewport::EXPANDED_MINIMUM_WIDTH` 由 1000 变为 840，侧栏与工作区在 840 进入双栏。触摸命中区按设计语言：Material 48，其余 44。新增 `EnvironmentOverride`、`DevicePreset`（13 个预设）、`EnvironmentRequest`，`Size::new`；`Insets`、`InputCapabilities` 可序列化。
  - `cove-composition`：新增 `PostureReader`、`DeviceClassReader`、`PlatformReader`、`DesignLanguageReader`；`WidthClassReader` / `HeightClassReader` 改为传 `WidthClass` / `HeightClass`。`uses_sheet` 改用 `device_class.is_handheld()`，折叠屏与手机一致。
  - Android：运行时库依赖 `androidx.window:window` / `window-java` 1.4.0 与 `androidx.core` 1.13.1。新增 `FoldingState.java`（`WindowInfoTracker` 监听 `FoldingFeature`，按显示自然方向区分书本式/翻盖，`OcclusionType.FULL` 为双屏；合上时沿用 `SharedPreferences` 记住的类型，铰链角度传感器作后备）。JNI `MainActivity.surface` / `cove_android_surface` 用 `device`、`posture`、`float[] features` 取代 `idiom`，Rust 侧 `DeviceFacts::decode` 解码并限长。`SurfaceState` 分离宿主上报与 DevTools 覆盖。
  - iOS / Windows：`cove-shell` 新增 `DeviceForm` 与 `PlatformShell::device_form`（`SDL_IsTablet`；Windows 平板模式且有内置触摸屏时为 `TouchComputer`）。iPad 不再被当作手机，上报 `Tablet` 与 `Platform::IPadOS`；Windows 平板模式在 `input` 中加入 `TOUCH`。
  - 预览与 DevTools：`COVE_PREVIEW_DEVICE=<预设>` 取代 `=phone`；新增协议命令 `window.environment`、能力 `environment` 与 MCP 工具 `set_ui_environment`（`clear` → `preset` → 显式字段，桌面按预设调整窗口尺寸，返回下一帧的环境与区段）。
  - 文档：`Website/src/content/components/adaptive.md`（平台自适应）登记到「工具链」组；更新 `mobile.md`、`crates/devtools/README.md`、`crates/hosts/src/PLATFORM_COVERAGE.md`。
  - 未验证：本机无 Android SDK/JDK，`FoldingState.java` 与 Gradle 依赖**未编译**；Android 折叠数据、iPad 判定与 Windows 平板模式均未做设备验收。已知缺口：`cove-browser` 原型不发布 `Environment`，Web 无姿态数据源；组件尚未按区段定位弹层或按姿态重排（P1/P2）。

- **移动端能力补齐与文档修复（2026-11-09）**：
  - 键盘：`Shell` 新增 `show_soft_keyboard` / `hide_soft_keyboard` / `keyboard_height`（默认无副作用，桌面与测试 fake 不受影响）；`cove_shell` 的 C 桥用 `SDL_StartTextInput/StopTextInput` 显隐软键盘；`apple.m` 在 iOS 观察 `UIKeyboardWillShow/WillHide` 缓存键盘高度并唤醒事件循环；`Sdl3PlatformHost::keyboard_height()` 让 `Environment.keyboard` 每帧刷新。Android 原有 `WindowInsets.Type.ime()` 链路保持不变，两端现在共享同一套组件层 API。
  - 存储：`FileAppStorageStore::default_path()` 拆出可测的 `base_for` 策略。iOS 写入沙盒 `Library/Application Support`，Android 由 `createApplication` 的 JNI 入口显式注入 `COVE_APP_STORAGE_DIR`（`resources` 的父目录，即应用私有 `filesDir`），也可回退到 `PACKAGE_RESOURCE_BUNDLE_PATH` 的父目录，不再写 `$HOME/.local/share`；桌面策略不变。
  - 进程服务：`cove-system` 新增 `process_execution_available()` / `require_process_execution()`，`ProcessSession::new`、`ProcessSession::spawn`、`NativeProcess::start` 在 iOS/Android/tvOS/watchOS/visionOS 统一返回 `ProcessError::Unavailable`（聚合层为 `io::ErrorKind::Unsupported`），此前 README 声称的行为并无代码。
  - 打包：`xtask/src/apple.rs` 抽出 `ios_entries`，iOS `Info.plist` 补 `UISupportedInterfaceOrientations` 与 `~ipad` 变体（四种方向），桌面 bundle 不受影响。
  - 文档：新增 `Website/src/content/components/mobile.md` 并注册导航；修正 `README.md`、`Website/src/content/cargo.md` 的过时包名/脚本、`AGENTS.md` 残缺引用、`Docs/architecture.md` 悬空链接、`crates/hosts/src/PLATFORM_COVERAGE.md` 的 DevTools 状态、`crates/system/README.md` 的 Swift API 名、`editor_interaction_tests.rs` 的矛盾注释与三个平台 README；`Docs/gallery-audit.md` 记录结构验证与原生验收边界。
  - 测试：`app_storage` 目录策略 5 项、进程守卫 2 项、iOS plist 键 1 项、软键盘默认实现 1 项，均为纯 Rust 结构测试。
  - 未验证：iOS/Android 键盘、IME、触摸、存储、方向与进程行为均未做设备验收。已知缺口（iOS 独立宿主与 `UITextInput`、原生 IME 组合预览、Android 通用剪贴板 JNI、多指手势、SAF/权限、前后台通知、原生 WebView、Android https OpenSSL）已在平台矩阵中列明，未以空实现充当完成。

- **CI 门禁失败修复（2026-11-08）**：
  - 统一 `cargo fmt --all` 的 5 处格式差异；`crates/graphics/src/dx12/pipeline.rs` 的 `formats` 只在 Windows 分支使用，导入改为 `#[cfg(target_os = "windows")]`，消除非 Windows 上的 `unused_imports`（clippy `-D warnings`）。
  - `cargo cove setup --check` 要求 macOS SDK 26+，desktop 矩阵由 `macos-15` 改为 `macos-26`；media-windows 补上固定版本的 Zig，避免 `setup --media` 报告 `zig` 未就绪。
  - `cove-native-build`：Windows `sdk::prepare` 原先直接用 vcvars 的 PATH 覆盖 `record_pinned` 写入的 Zig/Slang 目录；改为合并 PATH 并新增回归测试。
  - `cove-media`：`read_packets` 在 EOF 后提前返回，遗留的延后包再也不会入队，`queues_empty` 永假、短片段停在 `Playing`；现在优先重试延后包，队列满时立即停止读包以免覆盖唯一延后槽位。`pump` 在一个 tick 内连续发布所有到期帧，慢速 runner 不再被限制为每 tick 一帧（700 帧 / 200fps 的读前测试此前在 CI 稳定超时 15 秒）。
  - 本地验收：`fmt --check`、全 workspace `clippy -D warnings`、`test --workspace --test-threads=1`、`-p cove-media --features ffmpeg --all-targets`、`cargo cove setup --check`、`cargo xtask audit --json` 全部通过；Windows / Linux 分支由 CI 实机验收。

- **环境准备收敛到 `cargo cove setup`（2026-11-08）**：
  - `cove-native-build` 的 `BASE` 清单补上 `zig`（最低 0.17.0）。它是 `crates/terminal/build.rs` 每次构建必需的工具，此前 `setup --check` 会误报 `ready: true` 而 `cargo build` 仍失败。缺失时下载 Zig 0.17.0 官方归档，SHA-256 校验后解到 Cove 缓存并把绝对路径写入 setup state；`build.rs` 经 `cove_native_build::setup::setting("COVE_ZIG")` 读取，开发者无需手工 export。macOS / Linux / Windows 的 x86_64 与 aarch64 六组归档均有固定校验值。
  - 新增 `--shaders`：同样下载安装 Slang 2026.19（六组归档、固定 SHA-256）；Windows 额外从 Windows SDK 复制 `dxcompiler.dll` / `dxil.dll` 到 `slangc` 目录以启用 `-target dxil`。`Tools/Shaders/build-native-ui.mjs` 在 `SLANGC` 未设置时读取 setup state。macOS / Linux 会尝试复用 Slang 归档自带的 DXC 运行库，缺失时在报告里明确标注 DXIL 不可用，不伪造实现。
  - 新增子模块检测：按 `.gitmodules` 检查各 checkout 非空，缺失时自动 `git submodule update --init --recursive`；`--check` / `--dry-run` 只报告不改动。报告新增 `submodules`、`shaders` 字段，`ready` 纳入子模块以及（仅 `--shaders` 时）shader 工具链。
  - 单一来源：CI desktop job 增加 `cargo cove setup --check` 门禁并断言 `ZIG_VERSION` 与 `setup.rs` 一致；`setup-toolchains` 的 Zig 版本、`setup-slang` 的 Slang URL 与 SHA-256 都从 `setup.rs` 派生，不再硬编码。
  - `github.com/.../releases/download` 边缘返回 504 时自动回退到 `api.github.com` 的 release asset 端点（带 `Accept: application/octet-stream`），两个来源都要通过同一 SHA-256；CI 的 `setup-slang` 同样带回退。修正 `slangc -version` 输出在 stderr、Rust 侧只读 stdout 导致版本误判的问题。
  - 结构覆盖：六宿主/架构归档齐备与校验值格式、回退端点、Zig 属于必需工具、版本字符串解析、安装计划字段、解压后二进制定位。macOS 本机已端到端跑通 `cargo cove setup --shaders --check`（`ready: true`）；Windows DXIL 复制与 Linux 安装分支仍需在对应宿主实机验收。

- **弹出层定位在布局回写中先于内容确定；嵌套子菜单不再跳位（2026-11-08）**：
  - 根因：布局回写对每个节点先写子孙、后运行 `layout_did_update`；portal 槽位靠这个钩子把求得的位置翻转/夹紧进窗口，因此槽位里的菜单行在自己的钩子里读到的是父面板未夹紧的原始位置。行用它锚定下一级子菜单，而行重建时（`synchronize_submenu`）又用夹紧后的位置，于是父面板一旦被翻转或夹紧，下一级子菜单就在两个锚点之间交替——横向差一次翻转，纵向差夹紧量。重新弹出时的第一帧同样落在未夹紧的位置。
  - 修复：`Node` 新增 `FrameResolver` / `set_frame_resolver` / `resolve_layout_frame`，`ViewGraph` 与 `LayoutPass` 的回写在祖先就位后、子孙写回前调用它。portal 槽位改用解析器按实时登记项（位置、`position_for_size`、锚点、视口边距）计算最终 frame，钩子只刷新尺寸约束；不再使用构建时捕获的旧位置。`PortalStore::entry`、`window_frame_in_parent` 公开。
  - 测试：`a_nested_branch_follows_its_clamped_parent_on_every_pass`（三级菜单贴窗口右下角，修复前第三级请求落在 (842, 386)，屏幕上的行在 (450, 366)）；`an_open_submenu_stays_beside_its_row_in_a_flipped_menu`；`rebuilding_the_settings_menu_keeps_an_open_submenu_in_place`（播放器占满窗口、设置子菜单翻转夹紧，打开及之后每帧重建都与行对齐且不移动）；core `descendants_see_a_resolved_frame_within_the_same_pass`。compose 全量 1720 项通过。

- **视频播放器改为画面内控制、弹幕、全屏与画中画；本地/网络来源（2026-11-08，破坏性）**：
  - `VideoPlayer` 重写为主流形态：进度条与控制栏叠在画面底部渐变上，播放时自动隐藏、指针活动时显示；中央大播放按钮与加载指示；单击播放/暂停、双击全屏；键盘 空格/K、←/→、↑/↓、F、M、D、P、Esc。新增 `MediaProgressBar`（悬停加粗、悬停时间、拖动预览、松手精确 seek）、`PlayerSettings`、`PlayerPresentation`、`PlayerActions`、`send_danmaku`。设置菜单：倍速、画面模式、弹幕不透明度/速度/字号/区域、音轨。旧的“画面 + 下方控制条”`VideoPlayer` 移除（破坏性）；`MediaTransport` 仍供 `AudioPlayer` 使用。
  - 弹幕：`DanmakuFeed`（时间排序、`parse_bilibili_xml`）、`DanmakuComment`（滚动/顶部/底部、自有描边）、`DanmakuSettings`、`DanmakuScheduler`（纯调度：车道分配、防追尾、满屏丢弃、seek 不涌出）、`DanmakuLayer`。位置只由媒体时间决定，暂停静止、倍速同步。
  - 全屏与画中画：`Shell` 新增 `set_window_fullscreen` / `is_window_fullscreen` / `set_window_always_on_top`（默认空实现；SDL3 实现，macOS 用 AppKit `toggleFullScreen` 与浮动层级并可跨 Space/全屏应用；线程化宿主经 `PlatformCommand::Fullscreen/AlwaysOnTop` 转发，`WindowSnapshot` 新增 `fullscreen`）。compose 新增 `core::WindowPresentation` / `WindowPresenter`，桌面宿主安装实现（全屏 = 主窗口全屏 + 覆盖窗口的 portal；画中画 = 置顶辅助窗口）。无宿主支持时全屏仅覆盖窗口、画中画在窗口内浮动；系统层面退出全屏或关闭画中画窗口会回到页面内。
  - 来源：新增 `HttpInput`（`cove-http` 原生 TLS 上的字节范围请求，`206` 可 seek），`http(s)://`（非 `.m3u8`）URI 在 `Player` 与 `MediaDecoder` 中自动经它读取——内置 FFmpeg 不含 TLS，此前 `https` 无法播放。`PlaybackControl` 新增 `select_tracks`（默认不支持，`Player` 实现）。
  - 控制栏闪烁：播放中指针停在控制栏上时，自动隐藏把控制栏移出节点树，指针下的命中目标随之改变并被当作移动，控制栏又显示、又因手势层的离开事件再次隐藏，循环闪烁。现在控制栏始终挂载，只切换不透明度与子树命中（`ChromeVisibility`）；只有真实的指针位移才算活动；隐藏只由空闲计时决定。新增回归测试。
  - 设置菜单消失：播放中把指针移向向上展开的设置菜单时，指针离开控制栏且按钮上的移动不计为活动，空闲计时到点便隐藏控制栏，菜单随之消失。`MenuButton` 新增 `on_open_change`（原生菜单与弹出菜单两条路径都通知，选中菜单项关闭时也通知）；播放器在菜单打开期间不自动隐藏，关闭后重新计时；控制栏上的真实指针位移也计为活动。
  - 二级菜单上下闪烁：播放器主体订阅完整快照，播放中每 50ms 的位置上报都会重建整层控件，包括已打开的设置菜单；子菜单因此被反复重新呈现，在“行原位”与“贴窗口边缘夹紧”两个位置间交替。现在播放器主体只订阅不含位置的稳定快照，进度条、时间与弹幕改为各自订阅位置的子视图；悬停/拖动状态也移到独立的 `Timeline` 信号。新增 `use_playback_select`、`steady_snapshot`，`SignalSender::update_if` / `set_if_changed`（跨线程写入只在值变化时通知）。回归测试断言菜单打开期间 20 次位置上报不触发 portal 重新呈现（改回旧订阅时为 24 次）。
  - 修复（既有渲染缺陷，崩溃）：设备的绑定集缓存有容量上限（1024）并按 LRU 淘汰，但 UI 纹理槽、图集与背景模糊（玻璃）通道把绑定集句柄长期缓存，从不再次访问缓存，于是它们总是最先被淘汰；之后用到已销毁的句柄即校验失败 `RHI: layout mismatch — binding set must match the active pipeline's declared set layout`，应用退出。玻璃菜单打开期间每帧为新的可绘制纹理创建绑定集，很快填满缓存，因此打开设置菜单一段时间后必现。改为每次向设备取用（命中即刷新 LRU，正在使用的集合不会被淘汰），移除失效的句柄缓存；新增 `Device::is_binding_set_live`。`binding_set_eviction_tests` 覆盖（保留句柄的实现会以同一错误失败）。
  - 布局：`LayoutNode::set_aspect_ratio`（Taffy `aspect_ratio`），播放器按宽度保持 16:9。图标新增 `picture-in-picture`、`danmaku`、`danmaku-off`、`play-fill`；`Scancode::M`。`Waveform` 默认按峰值归一化（`.normalized(false)` 关闭）。
  - 修复（既有引擎缺陷）：解复用路由条件写反——包所属流已选中但队列已满时直接丢弃，未选中流的包反而被延后；视频队列 512 包上限约 17 秒，读到前面后丢包，画面在约 17.5 秒处冻结直到下一个关键帧（音频同理会出现缺口）。改为未选中/不可解码流丢弃、已满的选中流延后，另一选中流饥饿时允许有界超出（4 倍），避免交错死锁；延后包仍满时不再空转。
  - Gallery：Media Player 页改为本地文件（原生打开面板、拖放到播放器/音频卡片）与网络 URL 两种来源，视频同名 `.xml` 弹幕自动加载，可输入发送弹幕（6 种颜色）或导入 XML；音频在后台解码波形与标签。播放需 `--features media-ffmpeg`（未开启时页面给出命令）。不再随仓库附带样片：上一轮的 `sample.mp4` 移到 `crates/media/tests/fixtures/` 仅作真实编解码测试夹具。
  - 测试：compose `media_player_tests` 新增 5 项（画面内控制与快捷键、全屏/画中画窗口内回退、弹幕渲染与开关、进度条拖动预览与松手提交、隐藏后静止指针不闪回、菜单打开期间不隐藏、位置上报不重呈菜单），共 13 项；`danmaku` 单元测试 4 项；`cove-media` 新增 HTTP 端到端与超队列读前两项（共 11 项），`HttpInput` 单元测试 2 项。

- **音视频组件与底层媒体 API（2026-11-08）**：
  - 组件：`cove_ui_compose::primitives::media` 新增 `VideoPlayer`、`AudioPlayer`、`MediaTransport`（播放/暂停、拖动 seek、时间、静音与音量）、`VideoView`、`AudioLevelMeter` 与 `Waveform`，以及 `use_playback(cx, &control)`。组件只依赖新的 `cove_media::PlaybackControl` 契约，`cove-ui-compose` 依赖不带 FFmpeg 的 `cove-media`。媒体线程的变更只置位原子标记并向 UI 线程投递至多一个刷新；视频帧以每个视图一个稳定纹理 id 提交，渲染器就地更新同一 GPU 纹理槽。
  - 底层 API（始终可用）：`PlaybackControl`、`ChangeNotifier` / `ChangeListener`、`VideoOutput::on_change`、`VideoStorage::shared_plane`（默认 `None`）、`SharedPlanes`、`VideoFrame::rgba_pixels`、`AudioFormat`、`AudioBuffer`、`AudioPeak` / `audio_peaks`、`AudioMeter`（实时回调安全）、`AudioLevels`、`AudioDeviceInfo`、`MediaInfo` / `StreamInfo` / `StreamDetails`、`DecoderOptions`、`sample_stats` / `channel_stats`。样本统计内核在 AArch64（NEON）与 x86-64（SSE）上为内联汇编，其他目标为标量实现。
  - 底层 API（`ffmpeg` feature）：`MediaDecoder`（`next_frame`/`next_video`/`next_audio`，同步解复用/解码、按比例缩放、重采样、`frame_at`、`read_audio`、可跨线程取消）、`probe`、`audio_output_devices`、`AudioStream`（应用渲染 PCM，句柄在所有平台为 `Send + Sync`）、`Player::on_change` / `audio_meter` / `detach`，`Player` 实现 `PlaybackControl`；分离的请求不再计入 `result_capacity`。
  - 破坏性：`AudioOutput` 新增 `Device(String)` 并去掉 `Copy`；`FrameStorage::new` 接受 `impl Into<Arc<[u8]>>`，帧数据由 FFmpeg 直接写入共享分配，展示与上传不再拷贝；`VideoPresenter` 优先使用共享平面。
  - 修复（既有缺陷）：`from_micros` 把微秒乘以 1 000 000，容器时长被放大 1000 倍（`PlayerSnapshot::duration` 受影响）；声道布局未指定的音频（如普通 WAV）第二帧起重采样报 “Input changed” 导致播放失败；文件末尾在包队列未清空时就向解码器发送排空信号，剩余包永远不被解码，短片段停在 `Playing` 不会 `Ended`；设备音频时钟在 flush 后不复位，暂停或结束后 seek 时位置仍报告旧的设备时间。
  - 渲染：`Bitmap::shared_pixels`；`cove_imaging::allocate_texture_id`；UI 纹理存储把同 id 的新所有者视为流式纹理，移出图集并原地更新专用纹理槽，旧所有者先释放也不丢槽，CPU 像素上传不再复制。
  - Gallery：新增 Media Player 页（Media 分类）。默认用基于公开底层 API 的合成源驱动；`--features media-ffmpeg` 时播放 `examples/gallery/resources/media/sample.mp4`（生成的测试图案与正弦音，169 KB），波形来自 `MediaDecoder`。
  - 测试：`cove-media` 新增 `decoder` 集成测试 9 项（内存 WAV/Y4M 与 Gallery 的 H.264/AAC 样片：探测、重采样、缩放、seek、取消、分离请求与通知、`AudioStream`、播放到结束、结束后 seek）与内核、电平、通知单元测试；compose 新增 `media_player_tests` 6 项，全量 1710 项通过。x86-64 内核只做了编译验证。
  - 文档：新增 `media-player.md` 并登记导航；更新 `Docs/media-ffmpeg.md` 与 `crates/media/README.md`。

- **列表重设计：选中与悬停、ListRow API、内嵌盒与滚动条（2026-11-07，破坏性外观与 API）**：
  - 选中：`ListRowStyleConfiguration` 新增 `is_emphasized`。列表持有键盘焦点时选中为强调色带 + 浅色墨水，否则为中性灰带（`surface_pressed`）并保持原墨水，与 macOS 一致；焦点经 `FocusChain::current_focus` 的响应式读取驱动，焦点移动时行自动重组。此前选中行永远是强调色，且离开选中后前景色从未清除。
  - 悬停：此前行宿主把悬停画成整行高的背景，样式又把选中画成上下各内缩 4pt 的带，两者几何不一致——悬停带比选中带高、贴到相邻的选中行。现在悬停与按压由宿主实时绘制，几何与选中带相同（内缩 `spacing.xs`、圆角 `radius.md`），不重组行；已选中的行不叠加悬停。
  - 行上下文：新增 `ListRowContext`（`is_selected` / `is_emphasized` / `is_enabled` / `is_on_accent()`）与 `ListRowContextReader`，默认行样式与 SwipeList 都发布它，内容据此自动取墨水。
  - `ListRow` 新增 `icon` / `icon_tint`、`subtitle`、`value`、`badge`、`accessory(ListRowAccessory::{Disclosure, Checkmark, Custom})`、`destructive`、`disabled`；全部部件按所在的带取色，调用方不再按 `RowState::is_selected` 手选颜色（Gallery 的 List 故事此前在强调带上画强调色图标）。
  - 内嵌盒：内边距改为取自选中外观（左右 `horizontal_inset` = `spacing.sm`，上下 `vertical_inset` = `spacing.xs`），选中带四周与盒边距离一致；SwipeList 也放进同一个盒子，行内容补上 12pt 前导内边距与分隔线对齐，选中行上方的分隔线不再画。
  - 滚动条：虚拟化列表（List / SectionedList / SwipeList）改用覆盖式滚动条，不再保留 12pt 尾部车道——此前车道让选中带左边 4pt、右边 16pt。ReorderableList 仍保留车道给尾部拖动手柄，并把盒子内边距移到外层包装，修复拖动预览比源行偏移盒内边距的问题。
  - Scrollbar：新增 `always_visible(bool)`；Gallery Scrollbar 页此前只有一个静止时隐藏的滑块、没有任何内容，现在与一个 600pt 文档共享偏移，滑块常显。
  - 测试：新增 `selection_takes_the_accent_only_while_the_list_has_focus`；选中带测量改为相对内嵌盒，列表/分节/滑动列表与移动端测试的几何断言改为从主题取内边距；主题交叉淡化与样式集成测试改用中性选中带。
  - 文档：新增 `list.md` 并注册导航；`scrollbar.md` 补 `always_visible`。

- **Alert 视觉重做与 API 完善（2026-11-07）**：
  - 图标：状态标记改为 SF Symbols 式实心符号——勾、叉、感叹号与信息标记从实心圆形 / 三角形中挖空（`check-circle-fill`、`x-circle-fill`、`info-circle-fill`、`alert-triangle-fill`、`circle-fill`），16pt 下仍是一块清晰记号；新增 `StatusTone::filled_icon()`，原有描边 `icon()` 保持不变。
  - 外观：`Soft` 底色按 10% 混入 tone 色并压一条 18% 同色发丝边，`Outline` 描边降到 35%；标题与说明按 2pt 配对、操作行单独留 8pt；关闭按钮改用中性灰而不是 tone 色；圆角为 `radius.md`。
  - API（破坏性）：`AlertTone` 更名为 `AlertVariant`；`with_actions` 由链式 `.actions(view)` 取代；新增 `.enabled(bool)`（60% 不透明度、语义禁用、关闭失活）、`.dismiss_label(...)`、`AlertMetrics` 密度（Mini/Small、Regular、Large 的图标、内边距与间距统一随 `ControlSizeEnvironment` 缩放）。
  - Gallery：Alert 页按 Swift 段落重排为 Actions & dismissal、Inline feedback，并保留 Variants、Density；dismiss → Show alert 与 Disabled 交互经 DevTools 后置条件验证，浅色 / 深色截图目检通过。用户文档：`alert.md`。

- **半透明侧栏、正文卡片、指针与 CI（2026-11-07）**：
  - 窗口背景：`WindowOptions` / `WindowConfiguration` 新增 `backdrop: WindowBackdrop`（`Opaque` / `Sidebar`），`WindowHandle::backdrop()` 与 `WindowSnapshot` 报告平台实际应用的值。macOS 在内容视图最底层插入 `NSVisualEffectView`（`Sidebar` 材质、窗口后方混合、常驻激活态），窗口与 `CAMetalLayer` 设为非不透明，主窗口清屏色改为 `[0,0,0,0]`——UI 的直通 alpha 混合在透明目标上输出的正是预乘结果，无需改着色器。`Environment` 新增 `translucent_window`（破坏性：完整列出字段的字面量需补上）；`Sidebar` 在其为真时不画填充与发丝线。Windows / Linux 返回不透明：DXGI 与 Vulkan 交换链以不透明方式创建，Mica 与合成器模糊无法透出，需要 DirectComposition / 带 alpha 的交换链后再接入。
  - 标题栏与外观：首版只把材质放进内容视图，标准标题栏区域没有任何背景、完全透明（能看到窗口后面的其他应用）；且材质跟随系统外观，系统深色时浅色主题的侧栏是深灰底、文字几乎看不清。现在启用材质的窗口改为全尺寸内容视图 + 透明标题栏，材质一直铺到顶边，标题栏高度经 `contentLayoutRect` 作为顶部安全区上报，UI 从其下方开始，拖动与标题仍由系统处理。`Shell` 新增 `set_window_appearance(WindowAppearance)`（含平台线程命令 `SetAppearance`），桌面宿主每帧按根主题背景亮度在变化时同步：macOS 设置 `NSWindow.appearance`（Aqua / DarkAqua），Windows 设置 `DWMWA_USE_IMMERSIVE_DARK_MODE` 让标题栏随主题；Linux 窗口装饰属于合成器，无可移植开关，保持系统外观。
  - 材质可读性与侧栏排版：外观同步此前从未生效——`surface_theme` 只向下找 12 层 / 512 个节点，桌面宿主在应用根外包了 Surface、文字缩放与浮层等多层提供者，找不到主题；新增 `provided_theme_within(node, depth, budget)`，桌面宿主用 48 层 / 2048 节点。UI 文字不是 AppKit 的 vibrant 文字，纯模糊在窗口背后内容较暗时会让文字发灰；宿主在半透明窗口的根节点上铺一层轻薄的主题色（`window_backdrop_tint`：深色为 `surface_variant` 18%，浅色不加——浅色材质本身会把背后内容提亮到接近白色，再叠浅色只会盖掉透明感）。材质状态改为常驻 `Active`，窗口不在前台时也保持半透明，每个应用都自动获得，材质仍明显透出。背景应用后立即刷新窗口状态，首帧即拿到标题栏顶部安全区——此前首几帧顶部为 0，侧栏品牌行与红绿灯和窗口标题重叠；启用材质后隐藏窗口标题（侧栏品牌已给出应用名）。
  - 浮层坐标：`PortalPlacement::fit` 在窗口坐标系里求位置，但结果被直接写成相对宿主的 frame；浮层宿主（`LayerRoot`）位于安全区内时，偏移被加了两次——启用标题栏顶部安全区后 Select 下拉菜单向下错位 32pt（手机预览的顶部安全区同样受影响）。新增 `set_window_frame`，宿主与注册表两处放置都先减去宿主的绝对原点；注册表的“位置是否变化”也改为比较绝对坐标。
  - Gallery：窗口请求侧栏材质；正文页放进圆角卡片（主题 `lg` 圆角、内容底色、`low` 阴影、0.5pt 分隔色边，距窗口上/右/下 8pt），不透明窗口下卡片外为 `surface_variant`。
  - 指针：文字选择根节点此前把 I 形光标设在整个根上，根下没有自己光标的控件（Workspace 标签、按钮）与空白处都显示 I 形光标。`Node` 新增 `set_cursor_region` / `cursor_at`，指针按窗口坐标判断节点光标是否生效；选择根只在 `text_under` 命中已注册段落时显示 I 形光标（控件内的文字本就不注册），其余位置为箭头。新增 `a_cursor_region_limits_where_the_cursor_shows`。
  - CI（macOS）：macOS-15 runner 首次真正运行 Metal 测试，Apple Paravirtual GPU 在分配不受支持的资源时直接断言（`IOGPUMetalResource initWithResource`）而不是返回 nil，设备创建中的间接命令缓冲探测让每个 GPU 测试进程 SIGABRT。半虚拟化设备现只暴露核心光栅与计算路径（关闭间接绘制、光追与 mesh 探测），`adapter_capabilities` 与能力一致。
  - CI（Lints）：`cove-text` 中 Apple 专用的 `take_string` 加平台门控、`font_provider` 两处 `needless_return`；`cove-system` 路径测试在 Windows 下的未使用导入与对 `io::Result` 的 `==`；SDL 3.4 移除 `SDL_HINT_WINDOWS_DPI_AWARENESS`（SDL3 默认即 per-monitor-v2），删去该调用。
  - CI（Windows 媒体）：FFmpeg 的 MSVC configure 在 `sh` 下运行，job 此前既没有 MSVC 开发环境也没有 MSYS2 的 make / nasm，`cl.exe` 无法生成可执行文件；改为加载 `msvc-dev-cmd`、把 `C:\msys64\usr\bin` 加入 PATH、移除 coreutils 的 `link.exe` 并固定 MSVC 链接器。
  - 未验证：macOS 半虚拟化 GPU 与 Windows 媒体修复需等 CI 结果；系统材质的实际合成效果未截图（本机无屏幕录制权限），DevTools 只能看到 UI 自身像素。

- **Workspace 外观与槽位拆分修复、CI 修复（2026-11-07）**：
  - Workspace：槽位内拆分此前只记录 `fraction`、从未用于布局，两侧都按 100% 填充，第二个面板被挤出容器（Gallery 中 Preview 面板完全不可见）；现按 `fraction` 以 flex 份额分配，分隔条保留与区域间一致的 6pt `gutter`（此前为 1pt）。标签栏选中标签改为胶囊，并以 `SlidingSelection` 在标签间滑动；新增 `SlidingSelectionHost` 修饰符，可在不改变布局的前提下把现有节点设为滑动选中宿主（用于滚动标签行）。工作区根节点自带 `surface_variant` 画布，侧轨与外壳槽位不再自绘白底。标签行不再显示横向滚动条（此前浮在标签上方遮挡内容），仍可用滚轮与触控板滚动，溢出标签由尾部菜单提供。
  - 滚动条与文字选择：滚动条此前只是滚动视图上方的绘制，命中测试看不到它——指针移到滚动条上会取到下方可选文字的 I 形光标，按下也可能落到文字选择上。`Node` 新增 `set_chrome_hit_region` / `chrome_contains`：节点声明自己绘制在子节点之上的“铬区”，命中测试在该区域内以节点本身为目标、不再访问子节点，按下只交给该节点处理，指针恢复为箭头。ScrollView 把有溢出方向的滚动条通道声明为铬区（`Hidden` 与触屏除外）。`hit_tester` 新增用例覆盖铬区优先于其覆盖的子节点。
  - TextField：`clearable` 的清除控件改为仅在获得焦点且有内容时出现，并画成系统样式（灰色实心圆 + 白色 ×）；此前静止时也常驻一个裸 ×，不符合 macOS 输入框外观。尾部预留宽度不随焦点变化。
  - 动画泄漏：节点离开树之后（例如拆除时释放指针捕获或悬停导致按钮重绘），`animatable_set_with` / `animatable_set_optional` 仍会在其上启动新动画，而该节点的动画注册表已随卸载清空，控制器永远留在调度器里。现在已脱离树的节点直接写入终值、不注册动画。`unmounting_the_story_clears_its_scheduler` 因此恢复通过。
  - 开关拇指改为临界阻尼弹簧：不回弹，但中途反向时保留速度（`switch_value_changes_immediately_while_the_thumb_travels`）。
  - 测试（main 上已失败的 8 项）：内嵌列表盒的 4pt 侧边内缩与 `surface` 底色同步到 `list_tree_tests`、`sectioned_list_tests`、`mobile_presentation_tests`；Gallery 动画用例在断言调度器空闲前让被点按钮的 0.15s 松手反馈结束。
  - ScrollView：`ScrollbarGutter` 新增 `Hidden`——不绘制滚动条、不接收指针，内容仍可滚动；Workspace 标签行、TabView 滚动标签条与 Toolbar 命令行改用它。
  - Gallery：补上唯一缺失的 Workspace 页（Dock workspace：左侧 Files / Search / History 三标签、中部 Editor 与 Preview 上下拆分、面板最小/首选/最大尺寸、保存/恢复/重置布局），删除不再使用的 `StoryDispatch::Pending` 及占位视图；`serde_json` 移入 Gallery 正式依赖。
  - CI：修复各桌面平台 Formatting 与 Lints——`cove-logging` 与 `cove-system` 中仅在 Linux/Windows 编译的 `needless_return`、`collapsible_if`、Windows 下未使用的 `lexical`；`accessibility.m` 改用 `@"AXHeading"`，在 macOS 15 SDK 上不再因缺少 `NSAccessibilityHeadingRole` 编译失败。
  - CI（Linux 媒体）：手写 Vulkan 绑定中 30 个常量与头文件不符——`VkPhysicalDeviceFeatures2` 等 20 个 `sType`（深度模板/动态状态互换、时间线信号量、缓冲区地址、加速结构、mesh、draw parameters 等），以及访问掩码、混合因子、边框颜色、深度格式、解析模式与加速结构枚举；驱动按错误类型读取结构，导致 `vkGetPhysicalDeviceFeatures2` 校验错误与 `media-rhi` presentation 测试失败。全部按 `vulkan_core.h` 改正，新增 `graphics/tests/vulkan_constants.rs` 以文本方式比对绑定与头文件（任意宿主可运行；回退修复时失败）。
  - CI（Windows 媒体）：`windows_environment()` 用 `Command::arg` 传 `cmd /s /c` 命令行，Windows 下内层引号被转义为 `\"`，vcvarsall 从未执行，于是即使已装 Build Tools 也会转去调用 Visual Studio 安装器并在 runner 上失败；改用 `raw_arg` 原样传入。
  - 未覆盖：在 macOS 上交叉运行的 Linux / Windows clippy 只覆盖不依赖 CMake 原生库的 crate（`cove-logging`、`cove-system`、`cove-native-build` 已通过），其余 crate 的平台专属 lint 仍以 CI 结果为准；Vulkan 修复未在 Linux 实机运行 `media-rhi` 测试验证。

- **Stepper 外观重做与自适应堆叠修复（2026-11-07，破坏性外观）**：
  - 外观：标记改为按状态的实心圆盘（完成 success + 勾、失败 error + 叉、进行中 accent + 指示器、等待描边空心 + 序号），选中不再在整行背后画矩形，只强调标记与标题；连接线搬进该步自己的单元格，横向跨过到下一个标记的真实间距（1pt 发丝线），纵向竖轨从标记底边连到下一个标记顶边无断口；连接线颜色取它离开的那一步的结果（success/error/中性）。状态与交互切换使用主题 fast 动效。
  - 修复：`FlexModifier` 只在 `basis` 为 `Some` 时写回 flex basis，`None` 保留节点旧样式；自适应 Stepper 复用同一批节点切换主轴后，宽屏分支留下的 `basis 0` 让堆叠的每一步高度塌为 0、三步重叠在一起。改为完整表达作者意图（`None` 显式写回 auto）——该缺陷属于共享修饰符，任何按分支切换主轴的自适应组件都可能命中。
  - 验证：新增 `stepper_appearance_tests` 6 项（跨距、竖轨连续、连接线随结果着色、标记填充、选中不改底色、堆叠分支不塌陷），逐项变异验证；compose 全量 1702 项通过 1697，5 项既有失败（switch 动效、list 选择与行槽、sticky headers、移动端分节行内缩）在干净 HEAD 同样失败，与本轮无关。
  - 文档：新增 `stepper.md` 并注册导航，`gallery-audit.md` 更新 Stepper 验收记录。

- **macOS 风格组件库与弹簧动效（2026-11-07，破坏性外观）**：
  - 主题：浅色改为 macOS 窗口结构——内容区白色、侧栏与分组区 `surface_variant`（`#F2F2F4`）、按压 `#E3E3E8`；深色内容 `#1E1E1E`、分组 `#262628`、控件 `#2C2C2E`。强调色 `#0071E3`（深色 `#0B70E0`，白字均 ≥4.5:1）。圆角 0/4/6/8/10 → 0/6/8/12/16；阴影改为宽而淡的扩散阴影，浮层阴影保持宽光晕 + 紧接触阴影。`display`/`title`/`headline` 按 SF 规律收紧字距（34/26/17pt），正文及以下保持字体自身字距——负字距会让列表行与表格单元的小数宽度越过容器。
  - 材质：新增 `theme.materials`（`chrome`/`content`/`floating`）与 `.glass(GlassKind)`；`floating_surface`、overlay `FloatingSurface`、搜索建议列表与通知横幅改用 `floating` 磨砂材质（半透明底 + 背景模糊 + 发丝边）。
  - 按钮：`Button` 默认样式由 `PrimaryButtonStyle` 改为 `SecondaryButtonStyle`（macOS 推按钮：白底、发丝边、接触阴影），只有视图标记的默认动作使用强调色填充；按钮改为胶囊，纯图标按钮无边框（按 ghost 处理），工具栏内默认 ghost。按下即时压缩到 0.96，松手按欠阻尼弹簧回弹。SplitButton 改为一个胶囊外框 + 两个无边框分区；MenuButton 新增 `.ghost()`。
  - 输入：文本框聚焦时外圈 4pt 强调色光晕；Select 与 DatePicker 触发器改为 macOS 弹出按钮（胶囊 + 上下双箭头）；SearchField 为胶囊。复选框为带接触阴影的白色凹槽，勾按路径逐段画出；单选选中为强调色实心圆 + 白点弹簧弹出；开关关闭轨道改为浅系统灰，拇指弹簧行程、按住拉伸 5pt、柔和投影。Slider 为 4pt 灰轨 + 26×18 胶囊拇指。
  - 选择与导航：新增 `SlidingSelection` / `install_sliding_selection` / `SlidingSelectionMark`——一组行共用一个选中底并以弹簧滑动，位置相对宿主测量、滚动不追赶。Sidebar（中性灰圆角选中、强调色图标、30pt 紧凑行、贴边全高 + 内容侧发丝线）、ButtonGroup 与 TabView（灰色胶囊槽 + 白色滑动拇指，分段条贴合内容宽度）都基于它。
  - 列表与表格：List / SectionedList / 纵向 ReorderableList 为 macOS 内嵌列表盒（`surface` 圆角盒 + 侧边内缩），分隔线两端对称内缩、选中行上方与末行不画；DataTable 斑马行改为一级填充。日历日期改为圆形。
  - 反馈与浮层：Alert soft 变体为语调淡染底、无描边；进度条与环形进度用 `surface_pressed` 轨道；滚动条为 macOS 叠加滚动条（静止只有细胶囊拇指，悬停/拖动时加宽并出现轨道）；对话框圆角 18 + 发丝边、去掉页头页脚分隔线；面板、Accordion、代码块、Carousel、StatusBar、InsetGroup 改用分组填充。
  - 动效：`MotionScale` 0.12/0.28/0.45s，新增 `Easing::RESPONSIVE` / `Easing::FLUID`；浮层进入为欠阻尼弹簧（0.38s，从 0.9 弹出），退出 0.14s；`snappy`/`bouncy` 允许过冲。新增 `hover_lift()` 与 `cascade_in()`。Gallery 页面切换以弹簧淡入上浮，Dashboard 数字用 `NumericText` 滚动。
  - 修复：节点自身阴影此前被自身裁剪一并裁掉（两种渲染器都在阴影前压入裁剪），裁剪型表面（文本框等）的阴影/光晕不可见，现先画阴影再裁剪；`SelectionAppearance::fill` 合成顺序颠倒导致选中行的悬停/按压层被盖住；TabView 标题以 `flex-basis: 0` 布局、所有标签标题宽度塌为 0（原版即存在），改为自然宽度可收缩截断；列表行分隔线复用时残留。
  - 测试：按新默认值更新——默认按钮断言改为推按钮外观，专测主按钮行为的用例显式使用 `PrimaryButtonStyle`；动效时长、浮层 reveal 0.9 / 退出 0.14s、半径阶梯、侧栏 30pt 行与中性选中、侧栏图标强调色、面板分组填充、浮层材质、Tab 截断等断言同步收敛。
  - 文档：重写 `theme.md`，新增 `motion.md` 并注册导航；`sidebar.md`、`slider.md` 补外观说明。

- **菜单密度、分节样式与浮层动效苹果化（2026-11-07，破坏性外观）**：
  - 菜单行默认下限 30 → 20pt，实际单行高度改由 `natural_row_height` 一处结算：label 行高（默认 18pt）加上下各 3pt 留白，默认字号下为 24pt，仍不低于 `minimum_row_height()` 这个 CompositionLocal（触屏表面抬到 44pt，宿主可自行要求更高，例如 44pt 选择器）。高亮带同时从 `accent_muted` 软染色改为系统菜单的满饱和 `accent` + `on_accent` 墨色，按压态用 `accent_pressed`，圆角降到 `radius.sm`；悬停与键盘高亮仍是即时切换、不做淡入淡出。行首留白、固定列宽与列间距提为 `ROW_HORIZONTAL_PADDING`（8pt）、`ROW_LANE_WIDTH`（12pt）、`ROW_LANE_SPACING`（6pt）三个共享度量，标题、勾选列、图标列与分节标题都按同一套列宽落位。
  - 面板行预算修复：`MenuWindowBounds` 原先按 `max(行下限, button_height)` 结算，行降到 20pt 后 5 行仍按 32pt/行预留 160pt 视口，实际显示约 7 行——`max_visible_rows` 只剩占位、不再限制可见行数。改为与行自身同一份 `natural_row_height`，并让 `apply_node` 与 `layout_did_update` 回调经同一个 `budgeted_height` 解析 `max_height`：先前一处用作者值、一处用测量值，同一轮布局里 `max_height` 在 160 与 388 之间来回改，多行换行的用例会以「行 114pt / 视口 160pt」失败。`button_scroll_view_tests::nested_menu_consumes_boundary_wheel_before_parent_scroll_view` 里写死的 160pt 断言改为断言契约本身（视口 = 5 × 行高，±1pt）。
  - 分节样式：`MenuEntry::label` 不是行、不带列，改为借用行实际预留的列宽，把标题落在与行标题同一列；`MenuEntry::separator` 用 `separator_view()` 给出 1pt 细线加上下各 5pt（合计 11pt 分节带），TouchMenu 走同一个入口，两套呈现的分节断裂一致（此前触控菜单的分隔线没有任何空隙）。`section_header_and_separator_take_their_own_chrome` 覆盖两者：去掉行首留白时标题墨位为 22 而非 30，去掉分节空隙时间隔只剩 3pt。
  - 连带修掉一类绘制缺陷：节点的底色铺满整个 box，因此写在 `Divider` 自身上的留白不会让出空隙、只会把细线画成色带。Markdown 主题分隔线（`---`）此前正是 8pt 灰带，现在空隙移到外层容器、线体保持 1pt；`thematic_break_keeps_its_air_outside_the_hairline` 在没有修复时以 `8.0 != 1.0` 失败。菜单分隔线按同一规则构造，避免落入同一个陷阱。
  - 浮层关闭不再重放进入曲线：`animated_visibility` 此前无论进出都用 `SemanticMotionRef::entering()`，主题的 `SurfaceMotion.exit = 0.09` 与 `SemanticMotionRef::exiting()` 一直是死值。现按方向非对称取用——进入 0.2s 配 0.94 缩放的触发角 reveal，关闭 0.09s。新增 `surface_reveal_tests::menu_exit_uses_the_short_surface_exit_not_the_entrance`，改回单一曲线后 0.12s 仍有动画在跑、用失败。
  - 指针形状：菜单行此前对可激活项给手型、禁用项给禁用符号；系统菜单在整个面板内保持箭头，改为不覆写指针、继承面板形状。
  - 同一轮样式收拢（承接前序改动）：Select 触发器打开态按系统 popup button 处理——chevron 始终朝下、由按压底（`surface_pressed`）而非焦点环解释「菜单正挂在下面」，`focused` 仅在未打开且真正聚焦时成立；`FloatingSurfaceElevation::Panel` 的环境阴影由 `SurfaceShadow::default()` 提到 `elevation.floating.ambient`，菜单圆角从 group 层级降到 control 层级（6pt）。
  - 清理：删除 `MENU_WINDOW_BOUNDS`（`"__menu.rowMetrics"`）——行度量迁到 CompositionLocal 与 `natural_row_height` 后仓库内已无任何读写方。
  - 验证：compose 全量 1693 项通过（本轮新增 `section_header_and_separator_take_their_own_chrome`、`thematic_break_keeps_its_air_outside_the_hairline`、`menu_exit_uses_the_short_surface_exit_not_the_entrance`，`design_system_tests`、`row_density_tests`、`row_slot` 三处随 30 → 20 密度更新的断言同步收敛为 24pt / 26pt 字号 / 44pt 触控下限）。DevTools 真机实测（Menu 页 `Actions` 菜单，1280×720）：面板 x289、行 x293（面板内缩 4pt）、标题墨位 x301（行首 8pt）、行高 24、行距 25、分节处间隔 13（11pt 分节带 + 两处 1pt 栈间距）、180pt 作者宽度生效；Escape 关闭后菜单项查询命中 0 条、不残留 portal。**逐像素目检本轮未完成**：本会话的 `screencapture` 返回 `could not create image from display`（无屏幕录制权限），因此只做了数值级验收，明暗两主题的视觉判定仍记为待办。
  - 已知缺口与后续判断：submenu / Tooltip / HoverCard 目前没有进入过渡——若给它们补上，关闭也会变成动画，与现有「关闭后不留 portal」的断言及 macOS 子菜单即时收合的观感冲突，需连同断言一起重做，本轮不动；浮层仍用不透明 `surface_floating` 而非 vibrancy/backdrop blur，这是 `floating_surface_tests` 明确断言的现状（`backdrop_blur == 0.0`），改它等于换材质，需要单独立项。
  - 文档：`menu.md` 新增《行密度与分节》《动效》两节；`Docs/gallery-audit.md` 的 Menu / MenuButton / Markdown 行改记本轮实测数值与「像素验收待补」。

- **Tree 悬停主干按单层整组加深、命中区与悬停依赖收敛（2026-11-06）**：
  - 悬停加深的语义改为「当前行悬挂的那一列主干 + 共用它的整组兄弟」（破坏性外观）：鼠标停在某一行，只有它上一层的那一列由 0.36 提到 0.62，并从该列经过的每一行带上生效——指针上方的同组兄弟、下方仍在该组内的行都一起变深；祖先各列、其它分组的列、指针行自身的横向连接段之外的更深层列保持默认墨色；`Elbow` 的弯头只加深指针所在行那一行，同组兄弟的弯头不变。判定按祖先路径（`TreeGuideHover` 注册表记录 `(行号, 路径, 层)`，参考线在绘制阶段询问该列是否为悬停层），跨行失效仍落在参考线节点自身，指针移动不排入 Tree 重算；悬停行被折叠或过滤移出树时该列立刻回到默认墨色（`retain_active`）。此前「整条上游路径逐层加深」与「在祖先中线把高亮交给下一层」两种写法都没有对应约定，已按编辑器的 active indent guide 与文件树的常见行为统一；根层行不悬挂任何主干，悬停不加深参考线。`tree_guide_hover_deepens_the_path_to_the_hovered_row` 覆盖两种悬停层与折叠后的残留，新增 `tree_guide_hover_deepens_only_the_trunk_a_row_hangs_from` 覆盖跨兄弟主干与逐行弯头；把 `deepens` 的层级判定去掉、改成只认悬停行自身路径、或让弯头不看指针行，三个用例分别失败。
  - `hovered` 状态此前没有接入响应式系统：它不在 `Tree::dynamic_properties()` 里，因此写入不会使任何读取方失效——后果是自定义 `TreeRowStyle` 从来没收到过真实的 `is_hovered`。现补上该字段，同时把 `TreeRow::body` 里无条件的 `hovered.get()` 读取改成闭包 `hover_snapshot`：默认样式在绘制阶段读行节点上的悬停标记、不订阅状态，只有存在 `TreeSlots::trailing` 或样式声明 `requires_hover_recompose()` 时 `children()` 才读取它、宿主 `hover_action` 才被调用。默认树的指针移动因此不再具备触发整树重算的路径。新增 `a_custom_tree_row_style_receives_the_hovered_row` 覆盖自定义样式确实收到悬停行。
  - 拖拽命中区不再向左扩张：`TreeDragRegistry::hit` 原先在行的绝对 frame 外再留一段 `ROW_GUTTER`，而行本身已铺满整行插槽（缩进留白与展开控件都在 frame 内），同一份留白被算了两遍，树左侧的空白也会被误判为命中。改为只用行 frame 判定；`tree_drag_resolves_the_indent_gutter_inside_the_row_frame` 断言行 frame 的 x 一致、落在行左侧 30pt 的树外留白不产生放置，落在 frame 内即解析为该行。
  - 验证：compose 全量 1690 项通过。DevTools 真机逐像素验收（基线指针在树外 900,150）：`Straight` 悬停 Fill Light 时二级列 x312 在 460..507 由 202 → 164（Key Light 整带加 Fill Light 到中线收束），祖先主干 x296 保持 202 → 202；悬停 Lights 时 x296 在 430..567 连续一段 202 → 164，覆盖到末子 Camera 的中线，x312 不变。`Elbow` 悬停 Fill Light：主干 202 → 164 跨兄弟带，本行弯头 208/201/202 → 177/159/169，Key Light 的弯头只有 JPEG 噪声级变化；换悬停 Key Light 则加深的是它自己的弯头 206 → 159，Fill Light 弯头回到噪声级。深色主题同页复核（84 → 112 主干、80/88/87 → 109/113/117 本行弯头、祖先列 84 → 84）。明暗主题均成立。
  - 附带修复：`ToggleButtonStyle` 的文档示例从 ` ```ignore ` 代码块改为可编译的 doctest，示例本身也从 Swift 风格的 `Button(isSelected:action:)` 更正为实际 Rust API（`Button::new` + `.selected(...)` + `.button_style(toggle())`）。此前 `cargo test` 的 doctest 目标恒有一条 ignored 记录，现为 2 passed / 0 ignored。

- **Tree 参考线落位、拖拽投影与悬停底带修复（2026-11-06）**：
  - 参考线不再整体下移半行：`TreeGuideOverlay` 的布局表面此前没有子节点、高度为 0，被 `ZStack(alignment: .leading)` 按垂直居中摆到行的中线上，而绘制钩子把 `origin.y` 当行顶使用，于是 `Straight` 样式每行只画下半段、行间接缝不断开，末子的收束线段也掉到行下方悬空。表面改为与行等高后，相邻行的竖线接成一条连续线，`Elbow` 的横向连接与收束点回到行中心。`tree_guide_columns_are_drawn_from_the_row_top` 断言行自己的参考线节点与行同 y、同高；移除高度设置后该用例以 `45.0 != 30.0` 失败。
  - 拖拽预览的祖先列不再错位一层：`reparent::guide_flags` 原先把深度 0 祖先的兄弟状态也放进列表（长度 `depth`），而展平投影与绘制循环按 `level` 索引、约定根层不占列（长度 `depth - 1`），所以投影生效时最外列读到的是不存在的根层、父级真正的续接位被丢弃。`guide_flags` 改为按祖先深度写入 `column = ancestor_depth - 1`，跳过根层，祖先深度缺位时该列断开；`tree_reparent_tests` 的既有断言随之收敛，并新增 `guide_flags_index_columns_by_ancestor_depth` 覆盖三层嵌套。
  - 悬停与键盘光标的底色不再画两遍、也不再压住层级留白：行节点此前用 `radius.sm` 铺满整行（含缩进与展开控件），自定义 `TreeRowStyle` 或 `TreeSlots::trailing` 场景下样式又用 `radius.md` 在内容区铺第二层，内容区因此比留白区深一档、出现接缝，且高亮左边界与 accent 选中带不一致。新增 `TreeRowStateLayer`，占据行内容插槽并在绘制阶段读取行的悬停标记，行节点自身保持无底色；样式自行接管悬停时该层让位。悬停仍不排入 Tree 重算。`tree_row_hover_paints_one_band_in_the_content_slot` 与改写后的 `default_tree_row_hover_does_not_recompose_tree` 共同断言只有一层底带、它比行窄、与行等高，且离开后消失。
  - 拖动中的源行连同其参考线一起降低不透明度（此前只有行变淡，参考线仍按满墨绘制）；`Dashed` / `Dotted` 的分段相位改为按绝对 y 排布，跨行不再每行重新起相。
  - 验证：compose 全量 1687 项通过（`list_tree_tests` 新增 `tree_guide_columns_are_drawn_from_the_row_top`、`tree_row_hover_paints_one_band_in_the_content_slot`，`tree_reparent_tests` 新增 `guide_flags_index_columns_by_ancestor_depth`）。DevTools 真机验收：`Straight` 下最外列逐像素接成一条 431→597 的连续线、末子收束正好停在行中心；`Elbow` 的横向连接与竖段落在行中心；`Dashed` 在跨 6 行的同一列上保持 6pt 周期、`Dotted` 保持 4pt 周期，行界处不重新起相；悬停行底带左边界随缩进与选中带同步（深度 1 为 x=365、深度 2 为 x=381，差值正好是一个 16pt 缩进），不再出现第二层底色；拖拽预览中祖先列按新层级落位，被拖动子树的参考线墨色与标签同步变淡（202 → 243），松手后回到满墨；明暗主题均成立。
  - 文档：`tree.md` 的《样式与插槽》补上底带几何与参考线列索引约定。

- **CI 构建修复（2026-11-05）**：
  - `cove-logging` 在 unix 目标给 `native/logging/Router.c` 补上 `_POSIX_C_SOURCE=200809L`、`_DEFAULT_SOURCE`、`_DARWIN_C_SOURCE`，写法与 `cove-system`、`cove-http` 一致。Linux 用严格 `-std=c11` 编译时，`O_CLOEXEC` 与 `flock` 的声明被特性测试宏隐藏，构建脚本直接报 `undeclared identifier`；macOS、iOS、Android 原本就能编译。已用 `zig cc -target x86_64-linux-gnu` 复现修复前的同一条错误并确认修复后通过，其余仓库自有原生 C 源文件也用同一三元组逐个编译验证。
  - Android 构建补上 `CXX_aarch64_linux_android`：`crates/imaging/build.rs` 的图像解码桥是 C++ 的 `cc::Build`，而 `cc 1.10` 不读 `ANDROID_NDK_HOME`，缺少该变量时只会去找 NDK 中并不存在的 `aarch64-linux-android-clang++`，于是 `cove-imaging` 构建失败（三个 CMake 依赖自身编译通过）。CI 与 Release 的 `Configure NDK` 步骤同时设置；CI 还把 `ANDROID_NDK_HOME`/`ANDROID_NDK_ROOT` 钉到 r30，此前 runner 预装的 27.3.13750724 会被 CMake 取用，与仓库钉住的版本不一致。
  - 删除 CI 的 `Headless counter example` 步骤：`examples/counter` 已在 `3212c158` 连同 `zz_shaping_probe` 一起删除，步骤仍指向不存在的 manifest，`cargo run` 会以 `manifest path ... does not exist` 失败；headless 冒烟覆盖由 `cargo test --workspace` 与 `cargo xtask audit` 承担。

- **Tree 拖拽实时重排、参考线样式与几何 API（2026-11-05，破坏性）**：
  - `TreeDragOperations` 改为实时重排：投影只改位移不改数据，展平顺序、布局与命中仍由宿主模型决定，每行按 `(投影槽位 - 自身索引) * (row_height + row_spacing)` 位移，源行降到 0.38 不透明度，其余行用弹簧跟随，松手后模型落位与拖拽中绘制的位置逐行一致、不再二次滑动。删除放置插入线，`Before` / `Inside` / `After` 由行高上下 28% 判定。
  - 位移改为作用在裁剪槽上：新增 `ROW_SHIFT_ANCHOR` 标记与 `ReorderRowShift::anchor`，`FixedVirtualStack` 的 `RowSlot` 在 `update_node` 登记自己为锚点。虚拟化 Tree 的行 `frame` 是父级相对坐标，直接位移会把行推出唯一可见区域导致整行消失；ReorderableList 用自己的绝对定位槽因此未受影响。
  - 键盘移动与指针共用一份契约：`Alt+↑`/`Alt+↓` 走兄弟 `Before`/`After`，`Alt+Shift+←` 出到父节点之后，`Alt+Shift+→` 进入上一个兄弟，全部经 `can_drag`/`can_drop`/`on_drop` 上报；被拒绝的组合退回普通方向键导航。`TreeRow` 的按键处理先交给会话的 Alt 感知路径，`HOME`/`END` 等其余按键回落同一入口，此前它无条件吞掉四个方向键导致 Alt 移动失效。
  - `Inside` 放置自动展开目标分支（`TreeActions::reveal`），避免移入折叠节点的行被隐藏。
  - 参考线拆成 `TreeGuideStyle::{None, Straight, Elbow, Dashed, Dotted}` 与 `guide_width`（pt，不随字号缩放）、`guide_position`（0.0 到 1.0，默认 0.5）；`show_line(bool)` 保留为 `Straight`/`None` 简写。边距新增 `leading_padding`、`trailing_padding` 链式方法，并补齐缺失的 `indentation`，`TreeLayout` 几何字段全部可链式配置。
  - 参考线默认加粗（破坏性）：`guide_width` 默认 1.0 → 1.5pt，基础墨色由 `on_surface_muted` 的 0.24 提到 0.36，1pt 细线在高分屏上几乎看不清的问题随之消失。悬停某一行时只加深这一行自己绘制的参考线（含 `Elbow` 横向连接段）到 0.62，其余行保持默认；加深在绘制阶段读取行的悬停标记，行把参考线节点登记在 `__tree_row_guides` 上并由悬停回调逐个 `mark_render_dirty`，因此不触发行重算。
  - Tree 行改为可聚焦（`set_is_focusable`），点击行即可用键盘；`SelectionKeyHost` 记录自己是否持有子树焦点，并在焦点行随收起/过滤离开输入图后把焦点收回容器，`Alt+←` 收起全树后的快捷键不再失效。
  - 文档与示例：`tree.md` 重写《拖拽》并补键盘移动、《虚拟滚动》改为完整几何 API 与链式示例（含 `guide_width` 新默认值）、参考线一节列出五种样式与悬停加深、《可访问性》补焦点回收；Gallery Tree 页新增 `None`/`Straight`/`Elbow`/`Dashed`/`Dotted` 样式条与 `Indent`/`Lead` 两个滑杆，状态行同时报告拖放结果，目录副标题同步。
  - 验证：`tree_reparent_tests` 10 项、`list_tree_tests` 新增 15 项（含 `tree_drag_paints_projected_rows_inside_their_slots` 断言每行仍在其裁剪槽内、`tree_drag_reveals_the_branch_an_inside_drop_creates`、`alt_arrow_on_a_focused_row_moves_the_branch`、`collapse_all_keeps_the_tree_on_the_keyboard`、`tree_guide_hover_deepens_only_the_hovered_row`（断言只有悬停行的参考线墨色变深、失效标记只落在该行的参考线节点上，且不排入重算），均验证过移除修复后失败）、`tree_behavior` 的 Alt 移动契约用例，compose 全量 1684 项通过。DevTools 真机验收覆盖三种放置位置、四种键盘移动、展开/收起全部与焦点回收，并在明暗主题下目检五种参考线、两个滑杆、拖动中的投影，以及悬停行参考线加深（`Straight` 与 `Elbow` 两种样式，其余行墨色不变）。
  - 已知缺口：Tree 拖到视口边缘不自动滚动；键盘重排立即改结构，不走弹簧动画；减弱动效只有代码路径，未做设备级目检；Gallery 的 Reset example 不会把 Tree 模型恢复到初始层级（列表页同样如此，属既有行为）。

- **ReorderableList 拖拽实时重排与动效统一（2026-11-05，破坏性）**：
  - 拖拽改为实时重排，删除插入线：被拖动行的槽位跟随落点移动，它越过的每一行反向让出一个步长，拖动过程中画面就是松手后的顺序。移除 `ReorderableListModel::visible_boundary`、`drop_indicator_y` 与 `reorder-drop-indicator` 节点；`ReorderRowShift` 改为挂在行槽上的弹簧位移资源，新增 `slide_to`/`set_offset`/`clear`/`target_offset`，`live_offset` 给出投影位移。
  - 落位不再二次滑动：投影已经把每行放到落点，释放只清除投影并回调 `on_move`，任何行都不会再滑第二次；浮起副本仍用弹簧落到目标槽位，占位行透明度在落位过程中回到满值。Escape 取消时投影弹回、副本飞回原槽位。
  - 落位动画跨父级重建：新增 `session.ensure_begun`/`resume_settle` 与预览节点的会话 token，`scroll`/`shifts`/`settle` 进入 `dynamic_properties`，重建后弹簧从上一次的位置继续，孤儿会话自动退役且不会改写新会话状态。
  - 新增 `.axis(ScrollAxis::Horizontal)`：滚动、拖拽、实时重排与落位动画整体转到横轴，键盘左/右方向键与上/下等效，`Both` 按纵向处理。
  - 通用拖放预览（`Draggable`/`DropTarget`）加入 `DragPreviewFollower` 弹簧跟随与淡入淡出，减弱动效时贴住指针。
  - 阈值收敛为 `primitives::interaction::DRAG_THRESHOLD`（6pt）与 `crossed`/`crossed_along_axis`，`Draggable`/`DropTarget`、`Tree`、`SwipeList`、`DataTable`、`WheelPicker` 共用；删除 `SwipeListState` 内联的 4pt 死区。
  - 文档与示例：`reorderable-list.md` 改写拖拽段落并新增《轴向》，`drag-and-drop.md` 补《预览跟随与淡入淡出》《阈值统一》，`gallery.md` 目录行同步；Gallery Reordering 页新增横轴卡片条与顺序读数。
  - 验证：`reorder_model` 与 `reorderable_list_tests` 共 34 项通过（新增 `drag_reflows_the_rows_live`、横轴指针/键盘回归，原插入线断言改为位移与绘制顺序断言）；compose 全量 1654 项通过。DevTools 真机指针拖拽已在明暗两套主题下目检纵轴与横轴：拖动中的实时重排无空隙无重叠、浮起副本跟随指针、松手后各行停在投影位置不再二次滑动，顺序读数与画面一致。

- **媒体后端迁移到 FFmpeg（2026-11-05，破坏性）**：
  - `cove-media` 与 `cove-media-rhi` 的媒体 feature 由 `gst` 改为 `ffmpeg`。播放器、解复用、解码、重采样与 RGBA8 转换改为 `ffmpeg-sys-next 9.0.1`（静态源码构建，LGPL profile），音频设备输出改为 `cpal`；Wasm 不提供媒体后端。
  - 引擎按播放器拆分为泵线程与控制线程：所有 `AV*` 句柄只属于泵线程，控制线程负责命令路由、请求终态与操作期限；`abort` 通过 FFmpeg 中断回调中止阻塞 I/O，显式关闭可以打断卡住的打开。
  - 文件、绝对 URI、`Arc<[u8]>` 和自定义 `MediaInput` 统一走 `avformat` 与自定义 AVIO；音频主时钟来自 cpal 回调，无设备或静音输出时退回单调墙钟。
  - 删除 `third-party/gstreamer` 子模块、GStreamer 构建入口、`media notices` 命令、原生媒体缓存树、pkg-config/`GST_PLUGIN*` 环境注入和对应许可证收集器；`cargo cove setup --media` 改为检查 `make`、`nasm`、`clang`（libclang）以及 Linux 的 pkg-config/ALSA 开发包。
  - 文档改为 [Docs/media-ffmpeg.md](Docs/media-ffmpeg.md)，许可证说明改为 FFmpeg 静态链接义务；`Tools/MediaSmoke` 改为 FFmpeg 解码能力与控制层冒烟测试。
  - 验证范围：macOS 本机 media 测试与 RHI 上传测试；Linux、Windows、iOS、Android 已实现但未设备验收。

- **媒体一次性初始化与 Cargo 日常入口（2026-11-05）**：
  - `cargo cove setup --media` 成为唯一前置命令：检查并按平台补齐 CMake、Ninja、Python/Meson、pkg-config、Bison/Flex 以及编译器与平台 SDK（macOS 走 Homebrew 与 Command Line Tools，Windows 走 winget 与 MSYS2 并自动恢复 `vcvars` 环境，Linux 走发行版包管理器；Android 按需安装 cmdline-tools、platform/build-tools、固定 SHA-256 的 NDK 与 JDK 21）。`--check`、`--dry-run`、`--target` 用于诊断与交叉准备。
  - 原生 GStreamer 构建移入官方 `*-sys` crate 的构建脚本：按目标身份哈希使用用户级缓存目录、并发加锁避免重复构建、隔离 pkg-config 后写入 Cargo 链接参数。应用侧不需要 `[patch]`、额外环境变量，也不需要先跑构建示例；日常入口恢复为 `cargo build` / `cargo test` / `cargo run`。
  - 删除 `crates/native-build/examples/gstreamer.rs`，旧的“先构建原生依赖”入口退出正式流程；第三方许可证声明改由 `cargo cove media notices [--target <triple>]` 生成并打印输出目录。
  - setup 完成 Android 准备后写入项目 `.cargo/config.toml` 的目标链接器与 `CC_/CXX_/AR_/ANDROID_*/JAVA_HOME`，只补缺失项、不覆盖项目已有配置，并在 JSON 报告里用 `cargo_config_updated` 指出被修改的文件。
  - 补记 `third-party/gstreamer` 的子模块 gitlink（`1b2a95a1b727…`，即 1.28.8），使递归克隆与 CI 能真正取到源码；本地缺失时构建脚本回退到按该 revision 执行 `git fetch`。
  - CI 新增 `media`（macos-26、ubuntu-24.04）与 `media-windows`（windows-2025，暂标 `continue-on-error`）任务：setup → `cargo test --locked -p cove-media --features gst` → `cove-media-rhi` → 仓库外消费者构建/测试/运行 → `cargo cove media notices`。
  - 验证范围：macOS 已实测 setup 检查、缓存复用、`cove-media`（12 项）、`cove-media-rhi`（3 项）与仓库外下游应用的 `cargo build` / `cargo test` / `cargo run`；Linux、Windows、iOS、Android 只有实现，尚无本机验收。

- **媒体许可证声明（2026-11-05）**：补齐 GStreamer 及原生传递依赖的许可证原文、Rust 上游补充许可和静态 LGPL 分发说明；源码构建自动生成按目标收集的第三方通知、版本、许可和构建记录，遇到缺失原文或未登记项目时报错。固定 profile 显式关闭 GPL 选项；产品级对应源码、应用重新链接材料及其他平台发行验收仍按分发说明准备。

- **原生媒体基础（2026-11-04）**：新增可选 `cove-media` / `cove-media-rhi`，提供 GStreamer Player、自有管线端点、有界状态/请求与帧订阅、合作取消和保留帧的 CPU→RHI 上传。固定 GStreamer 1.28.8 源码，加入隔离 pkg-config 的静态构建入口与五平台配方。macOS 已验证真实媒体控制、Metal 像素读回和旧纹理生命周期；VideoView、Browser、硬件共享与其他平台设备验收仍待完成。该后端已在下方 Unreleased 记录中被 FFmpeg 取代。

- **List / Tree / SwipeList 排版修复与 Gallery 卡片化（2026-11-04，界面）**：
  - `ListRow`：图标/标题/尾部槽之间补上 Swift 的 8pt 间距；标题不再硬编码 `on_surface`，选中行由行样式统一改墨为 `on_accent`；副标题改为继承行墨并降低透明度，避免强调色带上的深色副标题。选中行尾部 chevron 也继承行墨。
  - `List`：行宿主裁剪到固定虚拟槽，内容超出行高时不再渗入相邻行。
  - `Tree`：行内容与带缩进引导的 ZStack 都撑满行槽，选中带不再收缩到标签宽度；叶子行用真实固定槽保留缩进与 disclosure 栏，不再因 `EmptyView` 无布局而贴左；行样式外层负责 flex，选中带覆盖完整内容区。
  - `SwipeList`：行内容显式 100% 宽；动作条只在 `reveal < -0.5` 时挂载，闭合行不再把 Archive/Delete 画在标题上。
  - Gallery：所有 `StorySection` 从“背景 + 1px 线框”改为 `surface_raised` 卡片，`radius.xl`、低 elevation 阴影、20pt 内边距和 40% 边框；删除未使用的重复 `GalleryStorySection`。
  - Gallery 列表示例对齐 Swift：List 使用 52pt 两行行高、文件图标、默认选中的强调行；SwipeList 使用 52pt 两行行内容、圆形标记、2pt 行距与 260pt 视口；RefreshableList 视口改为 264pt 以完整显示 6 行；Tree 改为仅显示节点标题，补回 Search/Expand all/Collapse all 工具栏和 260pt 宽可清除搜索框。
  - 回归：`list_row_tests` 6 项（新增 8pt 间距断言）、`list_tree_tests` 15 项（新增叶子缩进与选中带宽度断言）、`swipe_list_tests` 7 项（新增闭合行无动作条、内容从左侧开始断言）通过。

- **偏移容器中的 TextField 清除按钮几何修复（2026-11-04，修复）**：
  - `TextField` 的清除图标与命中区在绘制时重复叠加了节点的绝对原点：`edge` 已包含 `origin.x`，代码又加了一次 `absolute_origin().x`。字段不在窗口左边缘时（如 Gallery 侧栏搜索框），图标会整体右移并贴边裁切。
  - 现在图标与命中区统一使用已包含原点的绝对 `start`；新增 `the_clear_affordance_stays_inside_an_offset_field` 回归测试，把字段放在 40pt 内边距容器中并断言清除图标的所有顶点仍位于字段边界内。
  - 这是通用 TextField 几何问题，不是 Gallery 的用法错误；`Node::update_draw` 文档补充了“绘制回调收到的 origin 已是绝对坐标，不要再叠加 `absolute_origin()`”的约定。

- **DragUpload 默认外观重做（2026-11-04，界面）**：
  - `FileDropZone` 自带安静状态外观：`surface_sunken` 填充、`radius.lg` 圆角与虚线描边，不再只在拖拽悬停时才有框体。拖拽悬停/接受为强调色实线，拒绝为错误色实线，禁用为分隔线色；虚线沿圆角轮廓连续分段绘制。
  - `FileDropPrompt` 改为图标芯片 + headline 标题 + caption 类型说明 + 次级 “Browse files” 按钮的居中层级，类型与数量只在说明行出现一次；禁用时文字与芯片同步降级。图标在 44pt 芯片内真正居中，按钮的顶部间距移到独立容器，不扩大命中区域。
  - `DragUpload` 不再常驻空进度条与禁用取消按钮。进度条、百分比与取消按钮只在 `Preparing`/`Uploading` 出现；`Succeeded`/`Failed` 显示带色调的图标状态条，错误文案限制两行。
  - Gallery 的 Drag Upload 示例把过滤说明与诊断收为 caption 行，用户文档 `drag-upload.md` 补充默认外观说明。

- **拖入批次与类型语义修复（2026-11-04，修复）**：
  - **修复 `maximum_files` 失效**：`DropPhase::File` 原先每条事件立刻投递一次，而 SDL 每个文件发一条 `DROP_FILE`，多文件拖入因此被拆成多次投递，数量上限只看到 1 个文件。现在 `Begin` 开启批次、`File` 累积、`Complete` 一次性投递；新增 `FileDropRegistry::begin/stage/finish/cancel` 与 `has_staged`。中断的拖入不会把残留路径带进下一次。
  - 目录语义：`DroppedFile` 新增 `is_directory`；新增 `DroppedFile::inspected` 用 `fs::metadata` 判定目录与大小，读不到的路径按文件处理。`FileDropZone` 新增 `allows_directories`（默认 `false`），文件夹默认被显式拒绝而不是被当成空文件读取。
  - 扩展名语义：新增 `DroppedFile::extension()`。`file_kind` 不再把前导点当扩展名，因此 `.gitignore`、`.env` 与 `README` 都没有类型：空过滤放行，非空过滤明确拒绝。
  - Content-Type：新增 `primitives/upload/content_type.rs`，按扩展名查表覆盖图像、文档、音视频、字体与压缩包（zip/tar/gz/7z/rar 等），未知回落 `application/octet-stream`；只查表不嗅探，拖入阶段不读字节。`FileDropPrompt` 对空过滤显示 “Any file”，不再省略该行。
  - Gallery 的 `part()` 改用该表，删除手写 match；新增 “Any type”（空过滤）与 “Archives”（仅压缩包）两段示例。
  - 覆盖：`file_drop_tests` 增至 18 项（批次单次投递、超批整批拒绝、批次清除、取消、目录拒绝/放行）与 `content_type` 6 项单元测试；DevTools 注入 `begin → file×3 → complete` 验证整批只触发一次上传。真实 OS 多选拖入、文件夹拖入与原生打开面板仍未实机验收。

- **系统文件拖入与上传（2026-11-04）**：
  - `cove-platform` 新增 `ExternalDropEvent` 与 `DropPhase::{Begin, File, Text, Position, Complete}`，作为 `InputEvent::ExternalDrop` 交付。此前 shell/runtime 完全没有系统拖入通道：`cove_shell_poll` 只解码键鼠与手柄，SDL3 已支持的 `SDL_EVENT_DROP_*` 被丢弃。
  - 原生桥接入拖放：`CoveShellEvent` 新增 `aux` 承载拖拽来源，drop 坐标按 `coordinate_scale` 单独换算（此前只有指针事件缩放），`event_free` 一并释放。`sdl3_shell::decode_event` 解码 kind 25…29；无窗口归属的 drop 路由到主窗口，修复 iOS 以 null window 交付文档时被 `windows.contains_key` 丢弃的问题。解码逻辑抽为公开的 `external_drop` 以便无窗口断言。
  - 新增 `FileDropZone`、`FileDropRegistry`、`DroppedFile`：节点命中与顶层优先、扩展名与数量过滤、`accepts` 谓词、进入/离开高亮（接受/拒绝两种描边）。
  - 新增 `primitives/upload`：`multipart` 编码（单次拷贝、文件名与参数转义、编码前超限拒绝）、`UploadTransport`/`UploadTransportHolder` 接缝、`UploadState` 五态与 `DragUpload`（拖放区 + 状态机 + 进度 + 取消）。上传在 worker 线程读取与发送，经 `cove_core::work::spawn_then` 回 UI 线程写 `State`。
  - core 新增 `FilePicker`/`FilePickerFilter` ambient（照 `ColorSampler`），为触屏与键盘提供与拖入等价的浏览入口；未安装时按钮禁用而非打开空面板。
  - 宿主：新增 `application_services/upload.rs` 的 `HttpUploadTransport` 并在 `install_platform_services()` 安装；桌面运行时把 `InputEvent::ExternalDrop` 路由到 per-window 注册表并安装 `FilePicker`；`ApplicationServices` 补 `FilePicker` 的 capture/restore。删除 `composition.rs` 中无任何调用者的旧 `AssetDropRegistry`（`set_active_target` 从未被调用），改用 `FileDropRegistry`。
  - `cove-http` 仍以 `Vec<u8>` 发送 body，因此上传有 `maximum_bytes`（默认 64 MiB）并显式报 `PayloadTooLarge`；流式 body 列为后续项。
  - 边界：Wasm 无 HTTP 后端时 holder 为空，`DragUpload` 保持可用并报告 `HostUnavailable`；Android SDL 后端不上报拖放、iOS 仅“用…打开”触发，两端的正式入口是浏览按钮。
  - Gallery 新增 “Drag Upload” 页（注入离线假 transport），同步 catalog/routing/icons 与用户文档 `drag-upload.md`。结构测试：`file_drop_tests` 13 项、`external_drop_tests` 6 项、upload 与 hosts transport 单测。**原生 OS 拖入与原生打开面板交互尚未实机验收**。

- **屏幕取色：契约/后端分层与全异步回调（2026-11-04，破坏性）**：
  - `cove-platform` 只放契约：`ScreenColor` 之外新增 `ScreenColorBackend`、`ScreenColorSample`、`ScreenColorCompletion`；取消、不支持、失败不再折叠成一个 `None`。`Shell::sample_screen_color`、平台桥消息与宿主注入统一改用契约类型。
  - 真正的取色全部下沉到 `cove-shell/src/color_sampler/`，按窗口系统分后端：macOS `NSColorSampler`（`objc2-app-kit` + `block2`，从 `apple.m` 迁到 Rust）；Windows Win32 屏幕 DC `GetPixel`（系统没有桌面取色 API，WinUI/Chromium 的 Eyedropper 只能读自身窗口内容）；Linux X11 `XGetImage` 截屏采样（运行时 `dlopen` 加载 `libX11.so.6`，按 XImage 真实通道掩码解码像素）与 XDG `org.freedesktop.portal.Screenshot.PickColor`（自带最小 D-Bus 线协议客户端，无新增 crate）；iOS/Android/Wasm 明确回报 `Unsupported`。
  - **修复阻塞平台线程**：Windows 按键轮询与 `GetPixel`、X11 指针/按键轮询与 `XGetImage`、D-Bus 会话全部移到工作线程；结果经 `CompletionSink` 入队并唤醒事件循环，由 `take_deferred_callbacks()` 回到平台线程回调，恰好一次。删除原生 `cove_shell_sample_color` FFI 及 C/ObjC 实现，`build.rs` 不再探测 X11 头或链接 `-lX11`。
  - 取色器是模态的：新请求取消上一个并回调 `Cancelled`；`Sdl3Shell::shutdown()` 取消所有在途请求。宿主按 `screen_color_backend()` 决定是否安装 ambient，不支持的平台不显示吸管。
  - 结构覆盖：契约、异步队列/唤醒/去重/取消、Win32 `COLORREF`、X11 掩码像素、D-Bus 编解码与假总线完整 `PickColor` 流程。Windows/Linux 后端在本机用带桩契约的临时 harness 完成类型检查；原生实机取色交互尚未验收。

- **补齐 Table 轻量表格（2026-11-04）**：新增 `Table`（`primitives/data/lists/table.rs`），在普通 `Vec<Record>` 上绘制固定表头与虚拟化行，排序同步完成，宽度/排序/可见性/单选由挂载会话持有；`TableOptions` 的 `selection` 沿用 `Selection<Id>`，与 `List` 一致。与 `DataTable` 的分界是数据生命周期与排序成本，而非外观；两者共用 `TableColumn`、`TableColumnLayout`、`TableLayout`、`TableSort`、`ColumnVisibility`、`TableColumnPlan`。`TreeTable` 仍是首列换成层级列的 `DataTable`，`ChartDataTable` 是 `Chart` 的无障碍镜像，均非并列选项。
  - 列宽拖拽手柄从 `data_table/view.rs` 提到 `columns/resize_grip.rs`，改为只依赖 `Binding<f32>` + `TableColumnLayout`，`DataTable` 通过适配器传入原行为，两边共用指针拖动、方向键与辅助功能设置；顺带补上此前缺失的悬停与按压缩放反馈之外的绘制逻辑（竖线改为按节点高度绘制）。
  - 排序语义抽为 `sorted_order()` 共享：相等比较在两个方向都保留源序；`Table` 按列 id 在所有已授权列中查找排序，隐藏被排序列不会丢失排序。
  - 列可见性轨道换算去重为 `columns::picker_tracks`，`data_table/view.rs` 与 `table_presentation.rs` 不再各留一份。
  - `TableColumns` 收敛为 `pub(crate)`：它此前被导出但没有任何外部使用者。
  - Gallery 的 Table 页此前路由到 `AdaptiveTableSample::new(options, true)`，即实际渲染 `DataTable`；现改为新增的 `TableStory`（10,000 行库存表 + 空数据段，`gallery-table`），`AdaptiveTableSample` 按 `data` 分支真正在两种表之间选择。
  - 回归覆盖排序循环、相等保留源序、隐藏排序列、列宽约束、窄宽丢列、空数据保留表头、重复行 id 拒绝与选中绑定。用户文档 `table.md` 新增。

- **ColorField 色轮与取色器完善（2026-11-04）**：
  - 新增 `ColorWheel`，提供环样式（色相环 + 内嵌饱和度/明度方块）与盘样式（角度=色相、半径=饱和度）；拖动与方向键实时写回绑定并保留透明度；HSV 转换与几何映射独立到 `color_wheel_model`。**修复拖动**：指针点与圆心原先分属局部/窗口两套坐标，节点不在原点时角度全错；现在统一为窗口坐标，浮层内拖动正确（新增浮层内拖动的回归测试）。
  - `cove_core` 新增公开的逐顶点着色原语 `DrawList::add_disk_shaded(center, radius, shade)` 与 `add_shaded_annulus(center, inner, outer, shade)`，按位置逐顶点求色并按半径做一像素羽化。
  - `ColorField` 浮层新增透明度棋盘（触发器与预览，alpha 可见）与预设色板（`shows_swatches`/`swatches`）；`shows_wheel(false)` 可收起色轮。`Button` 新增 `accessibility_text`，供非文本标签（色块）提供可访问名。
  - R/G/B/A 数值改为可编辑输入（0–255）；文本输入支持 HEX/HSL 切换（`ColorFormat`、`format_color`、`parse_color`、`hsl_from_rgb`、`rgb_from_hsl`、`parse_hsl`）；透明度滑块改为棋盘上的透明→不透明渐变轨道；提交的颜色自动进入“最近颜色”色板（`recent`/`recent_colors`，去重上限 8）。
  - `ColorField` 新增 `on_sample(handler)` 吸管钩子：宿主提供屏幕取色，控件显示吸管按钮并在取色完成时回调（控件本身不访问屏幕；平台取色实现由宿主提供）。
  - 色轮属于取色器家族，不是通用 Slider 的类型。Gallery 新增环/盘两个示例，用户文档新增 `color-wheel.md` 并更新 `color-field.md`。

- **图表对齐 Swift（2026-11-04）**：
  - 轴文字真正绘制：`ChartPaint::axis_labels` 按 Swift `axisLabels` 的 gutter 规则输出刻度与标题并接入 `ChartPlot` 绘制流程；`ChartText` 接入真实 `TextEnvironment` 测量，仅在没有环境时退回字素近似，修复轴宽、绘图区与标签抽稀一直按伪造字宽计算的问题。
  - 覆盖层按种类分级：callout 卡片（`input_radius`、色点、body-strong 数值）、annotation 描边药丸、donut 中心标题、其余 caption；callout 行距改为 2pt；覆盖层整体从可访问性树隐藏，避免与图表读数重复播报。
  - 图例项补齐可见内容、`Return`/`Space`/`KeypadEnter` 键盘激活（忽略重复）、透明空心色块与焦点环；隐藏色块改用 `clear`，不再覆盖图表自身背景。
  - 交互判定恢复 Swift 语义：仅绑定 `viewport.range` 或 `selection.range` 的图表也可命中、聚焦与滚轮缩放；Alt-drag 平移改用真实修饰键；wheel/motion 注册改回 `control` 路由，不再被全局滚动兜底误触发；卸载时释放指针捕获与焦点。
  - 数据动画修复：减少动态效果不再提前返回（此前会永久冻结采样），`ChartConfiguration.animation` 真正生效而不再被丢弃。
  - 几何缓存签名补齐 guides（阈值/带状区/标记）、每序列绘制样式、每序列值轴侧、完整分类标签与 caption 字号：编辑阈值、切换右轴或改变曲线样式会重建几何，而不是沿用旧位置。
  - 修复 `selection.value_text` 从未被读取（自定义格式化静默失效）、`Viewport::default()` 把最小缩放比例归零（1% 下限退化为 0.01%）、`Chart` 绘图区 basis 置 0 导致无约束父容器下塌陷到 24pt；`ChartDataTable` 单元格恢复内容自适应宽度。
  - Gallery 图表页补齐 Swift 全部 13 个段落：新增 Large series；Data table 改为真正渲染 `ChartDataTable`；Explore 接入 label/viewport/brush/animation（三个按钮此前无效）；新增 Stacked/Percent 复选框并让 Percent 模式可用；Bars/Baseline/Dual axis/Log/Histogram/Horizontal/Scatter/Donut/Trend 改用 Swift 数据集、标记、颜色、轴标题与高度；调试名恢复 Swift 命名并去掉兜底分支。

- **字段 API 收敛（2026-11-04，破坏性）**：输入框外壳从一处解析——新增 `InputChromeState` 与 `InputAppearance::{background_for,border_for,border_width_for}`，`TextField`、数值前缀、`Select`、`TimeInput` 与 JSON 编辑器共用同一份填充/边框/圆角，不再各自手写。新增可独立使用的 `Field` 外壳（与 `Form` 同一套标签、帮助、错误与可访问性布局），Gallery 字段页由临时的 `field_row` 改用 `Field`。数值配置收敛为 `NumberFieldOptions`：`Vec3Field` 不再复刻 `NumberField` 的 builder，改为 `.options(NumberFieldOptions)`，删除 `.step/.size/.with_format/.enabled/.decimals/.mixed_value_label`；`NumberField` 增加 `.options(...)`，删除未使用的 `decimals`。向量专属的约束、混合轴、最小轴宽与剪贴板仍留在 `Vec3Field`。Gallery 目录由 106 页收敛为 102 页：`Numeric` 合并 `NumberField`+`Vec3Field`，`Text Fields` 合并 `TextField`+`SearchField`+`TextArea`+`JsonField`，并删除对应页变量、catalog 条目、图标与 dispatch。用户文档：`field.md` 新增，`number-field.md`、`vec3-field.md` 更新。

- **Gallery 控件对齐（2026-11-04）**：按本地 Swift 恢复 Toolbar 全部命令组及纵向溢出、完整 Text Editor 与其状态/诊断/标记/补全，重新对齐 TextArea、NumberField、Vec3Field、ColorField、Select、SplitButton 的初始值、布局和操作；修复数值前缀输入框外观、向量步长与平台剪贴板、颜色浮层主题/草稿和 SplitButton 单一 Tab 停靠点。同步九页 Rust 文档及验收记录。

- **自定义布局测量（2026-11-04）**：普通尺寸提议保留内容固有宽度，操作组单独测量收窄后的换行高度，避免长按钮重叠及网格、Markdown 代码块横向滚动回归。

- **实际字体样式继承（2026-11-04）**：CJK 回退继承主字体实际字重与斜体；Apple 复制 CoreText traits，其他平台读取 OpenType 表与 variation 坐标，补充命名 Bold 主字体和 TTC 元数据回归。

- **时间线与代码行号栏（2026-11-04）**：恢复时间线主题间距并消除图标重复缩放，组折叠与 zoom 即时更新；行号控件与正文共用几何，真实指针及输入框快捷键可操作，文本重基保留折叠状态。

- **资源准备所有权（2026-11-04）**：所有后端共享的资源准备流程自动登记分配与缓存 lease，取消或失败按逆序释放并遵守 GPU 帧退休；回归直接验证失效 Metal binding 不会提交工作。

- **Swift 行为回归（2026-11-03）**：修正 reader 读取追踪、范围选择会话、菜单样式标题、图片占位尺寸、滚动条动画及公共滚动锚点；补齐向量字段布局、时间线标识、浮层焦点和代码行号控件。工作区控制器通知保留视图树，尺寸约束、缩放、RTL 标签导航和可反转离场动画使用共享 Rust 实现。CJK 回退按字重和斜体匹配家族，终端选区立即使缓存帧失效。迁移断言改为验证预期结果，组件文档与 Gallery 验收记录同步更新。

- **统一 Text API（2026-11-03，破坏性）**：
  - 删除 `RichText` 组件及导出，不保留兼容别名；普通文字使用 `Text::new`，样式片段使用 `Text::runs`，`TextRun` / `TextTraits` 替代带 Rich 前缀的片段类型。
  - 两种内容由同一个 `Text` 节点处理，统一字体、颜色、对齐、行数、截断和选择配置；切换纯文本与样式片段保留节点与有效选区。构建方法统一为 `alignment` / `color`，正文用 `plain_text()` 读取。
  - 菜单、Markdown、表格、Gallery 和测试迁移到新入口；带样式片段支持完整字素截断。链接锚点改为行内语义覆盖层，不再在段落后额外占行，并把焦点回传同一 Text 节点。
  - 普通文字与样式片段复用拖选、复制、Shift 扩选与触控长按，跨链接拖选不打开链接；正文与输入框显示选区两端短竖线：起点向下、终点向上，圆头细线贴合选区边缘。
  - Website 合并为 Text 用户文档与新 API 示例；Gallery 的富文本页改为 Text。

- **默认桌面主题与选区外观（2026-11-03）**：
  - 浅色使用暖白与石墨强调色，深色使用分层炭灰与浅灰强调色；降低装饰色饱和度，圆角调整为 0/4/6/8/12，输入底色使用 `surface_sunken`。
  - 正文、带样式的 Text 和输入框共用轻微圆角选区与两端短竖线；跨段落/反向选择只标记整个选区的两个端点。修复输入框重复降低选区透明度，选择期间隐藏插入光标。
  - Text 样式片段补齐共享选区、复制、Shift 扩选、统一选区与 display-only；拖选链接不触发打开。选择根作用域持有于挂载节点，避免重建丢失选择。
  - Website 增加默认主题 Rust 文档并更新文本选择说明。系统鼠标指针与应用插入光标分开验收；浏览器亦出现的指针异常仍待系统恢复后复核。

- **TextFieldStyle 状态接入与 TextBuffer 字素索引（2026-11-03）**：
  - 完成响应式样式宿主：焦点、hover、IME 组合/提交/取消更新 `is_focused` / `is_editing`，不依赖绘制；字段自身样式优先于环境样式，默认样式负责外观，输入节点保留选区与历史。禁用清除焦点与组合，失焦取消未提交组合。
  - rope 叶节点按扩展字素簇建表与切分；编辑接缝修复组合重音、ZWJ emoji、国旗重配和 CRLF，超长字素保持完整；光标停靠、正文拖选、密码遮罩、代码缩进、snippet 和补全同步使用字素索引。
  - SDL 组合事件的标量偏移映射为字段字素偏移；Windows SDL、Android 与浏览器入口先统一 UTF-16 位置为标量位置。平台 UTF-16 坐标使用 rope 查找并对齐完整字素。长度限制允许不增加字素数的组合插入。
  - Gallery 增加编辑高亮与 Editor 样式展示，所有字段计数使用 `character_count` 并注册独立状态；Website 补充 TextFieldStyle / TextBuffer 用户文档。
  - 破坏性：所有字符索引改为字素，`character_at` 返回 `Option<String>`；UTF-8、UTF-16 及 parser 坐标保留各自单位。

- **TextField 行号、横向滚动与覆盖层（2026-11-03）**：
  - 行号栏接入排版：三位数固定列加 24pt 间距，`line_number_color` / `line_number_gutter_color` 分别控制数字与栏底，行号不随横向滚动移动，文档列在位数变化时保持不动。
  - 绘制阶段按光标揭示双向滚动并发布 `__textfield_document_layout`；横向滚轮与纵向共用 30pt 步进，溢出字段独占滚轮（含边界），聚焦且溢出时声明 `WheelRoutingPriority::PreferFocused`，hover 时绘制滚动条覆盖层。
  - Escape 在 `on_cancel` 存在时被消费并把焦点交还最近的焦点祖先；焦点请求选中当前值；Control-Home/End 到文档首尾；单行字段 Shift-Return 插入换行、垂直字段 Return 换行 / Primary-Return 提交。
  - 更新阶段也发布滚动界限，嵌套字段在首次绘制前就能独占滚轮；修复滚动界限为零时滚轮被外层吞掉的回归。
  - 测试改回验证预期行为：`cargo test -p cove-ui-compose --test compose` 1545 项通过，`cargo test -p cove-examples` 全部通过；`Docs/gallery-audit.md` 与用户文档同步更新，剩余项为 `TextFieldStyle` 状态接入、IME 区域重发布、组合预览缓存、字素计数与双击选词。
- **TextField 迁移收尾（2026-11-03）**：
  - 编辑事务历史接入字段：相邻输入与连续删除各合并为一步，粘贴/IME 提交/选区替换各为独立一步；撤销连选区一并恢复，新编辑清空重做分支，外部改写绑定的文本清空两侧栈。菜单与本字段快捷键（Primary-Z、Primary-Shift-Z、Primary-Y）读取同一份历史，不再把 Primary-Z 漏给外层快捷键宿主。
  - 补上多行测量与滚轮：垂直字段按行盒增高到 `max_visible_lines`，显式换行同样让默认字段增高；溢出后滚轮按 30pt 一格移动视口，未溢出时继续传给外层滚动视图。滚动界限在更新阶段发布，嵌套字段在首次绘制前就知道自己能否滚动。
  - 补上词/行/文档级光标移动：Option（Windows 为 Ctrl）按词移动与删除，Command-Left/Right 到行首行尾，Command-Up/Down 到文档首尾，Command-Backspace/Delete 删除到行首行尾，Up/Down 逐显示行保持列，PageUp/PageDown 移动一个视口，Home/End 停在显示行的首尾并保留软换行行尾亲和。
  - 补上逐段文字样式：`syntax_color_at_utf8_offset` 按字形着色，占位文本不参与；前后缀、两端色块、字数计数按测得宽度参与排版、命中与绘制，`max_length` 同时约束输入、粘贴与 IME 提交。
  - 平台输入的删除与组合提交进入撤销历史并遵守 `max_length`；修复工作区无法编译的半成品改动（重复的选区删除、未定义的按键分发、缺失的字体类型、隐式默认值）。
  - 测试从 缺失行为 断言改为验证预期行为，`cargo test -p cove-ui-compose --test compose` 1545 项通过，`cargo test -p cove-examples` 全部通过；`Docs/gallery-audit.md` 与 TextField 用户文档同步更新，注明行号栏配色、横向滚动、`TextFieldStyle` 状态、Escape 回调与 hover 滚动条仍未完成。

- **Gallery 首轮逐项验收与组件文档（2026-11-03）**：
  - 修复 TextField 缺失原生输入区域 resolver，导致点击搜索后普通文字无法输入、删除键仍有效、光标不闪烁；共享 Rust 实现补齐焦点状态、IME anchor、光标闪烁、搜索图标和清空绑定，Gallery 侧栏改用 SearchField。平台选区/删除/组合提交写回绑定，UTF-16 不拆开字素；补上点击定位、拖选、多次点击、全选/复制/剪切/异步粘贴和及时重绘。
  - Slider 的阴影只绘制在滑块上，不再给整个轨道加灰底；Slider / RangeSlider 共享密度与清晰轮廓，RangeSlider 增加局部阴影、独立键盘焦点环、端点可访问边界、点击端点焦点和 RTL 输入/绘制映射。两种控件分别保持单值与双端点类型。
  - Rating 绘制列表保留 SVG 图片，修复评分方块；复用已挂载会话，正确更新只读/禁用输入标志；密度与自定义示例各自持有真实状态，恢复十颗星、清空与窄窗口横向滚动。
  - Slider 页补上刻度、垂直方向与编辑状态；RangeSlider 页补上步长、最小间距与禁用示例。文本选择按完整字素扩展，公共 prelude 导出 TextBuffer 和 TextSelectionViewExt。
  - Website 新增 Gallery、Slider、RangeSlider、Rating、SearchField、TextField、文本选择的 Rust 用户文档；`Docs/gallery-audit.md` 记录 106 页队列、212 个宽屏/窄屏几何场景及未完成的逐页视觉/功能验收，明确已有 GAP 测试不能替代能力验收。
  - 破坏性：`SliderStyleConfiguration` 增加 `size: ControlSize`；自定义构造需提供大小。

- **DevTools MCP 可直接使用（2026-11-03）**：
  - 新增独立 Rust `cove-devtools-cli` / `cove-devtools mcp` stdio 服务及项目 `.mcp.json`，启动前可发现工具和指南，支持端口、WebSocket 地址与认证会话文件；持久连接后台读取，失败不重放写操作。
  - 补齐语义选择器点击、填值、键盘、拖拽/滚动、条件等待、截图、窗口调整、选择、临时样式和日志工具，以及跨调用 CPU trace、分页、比较和帧耗时采集；同一套工具可由 CLI 调用。
  - MCP 截图返回有大小限制的图片 content，不再作为 base64 文字；节点默认紧凑输出，显式值读取保留截断标记，选择器歧义和无效输入在交付前报错。
  - 修复角色筛选大小写、组合控件标签、诊断参数校验和时间线停止时丢失尾部 span；宿主区分输入、可访问性动作和图片能力，桌面/Android 共用适配器在延迟安装输入回调后更新能力声明。
  - 替换过时 Swift 接入文档，补充 AI 使用指引与真实认证 WebSocket、stdio、交互、截图和 trace 回归验证。破坏性：`AgentToolResult` 增加 `images` 字段，直接构造时请改用构造器或补齐字段；无效协议输入和越界诊断参数不再被当作成功请求。

- **Swift 迁移遗留与 Gallery 覆盖补齐（2026-11-02）**：
  - `ChartOverlayLabel` 从 attachment 字符串改为真实文字排版：标题、副标题与每行键值都按主题渲染，callout 与 annotation 的卡片边框和圆角随之生效。
  - 删除 `workspace/view.rs` 中重复移植的 tab bar、rail、split divider 与布局节点子树；`DRAG_THRESHOLD` 只保留 `drag.rs` 一处定义。
  - Gallery 占位页替换为真实组件：Popover、Modal、Disclosure、Dialog、AlertDialog、Sheet、BottomSheet、CommandPalette、DataTable、VirtualStack、SwipeList、RefreshableList、DragAndDrop、SectionedList、NavigationRail、Accordion、Breadcrumb、Pagination、ButtonGroup、Label、ContextMenu。ContextMenu 页改为真实的右键菜单演示并与 Menu 页分开。
  - `PlacementExample` 每个放置方向一个真实 `Popover` 触发器；懒加载网格、Masonry 样例和 `Divider` 接入 Layout 页。
  - TimePicker 的 "Now" 读取系统时钟，WheelPicker 补上 Reset 与已提交值读数；侧栏示例重新绑定草稿输入框；TextField 页删除被丢弃的重复控件。


- **Compose 组件 API（破坏性）**。应用可导入 `cove_ui::prelude::*`，组件在所属分组中显式导出。使用旧实现模块路径的代码请改从分组导入。
  - `DatePicker`、`DateRangePicker` 和 `CalendarView` 通过 `new(binding)` 直接读写选中日期。`Image` 的构造器接收逻辑尺寸 `Size`，颜色和渲染模式通过 builder 设置。
  - `Spinner::new()`、`Divider::new()` 和 `ZStack::new(content)` 采用默认外观。`Sheet`、`BottomSheet`、`Dialog`、`Tooltip` 使用 `configure` 增量修改选项。
  - 标签页、侧栏、分段按钮、单选组、步骤条、表单、折叠面板、表格等集合组件使用 `try_new` 报告重复 ID。`AppBar::try_commands` 一次检查主命令和其他动作。`DataTableModel::replace_rows` 返回校验结果，失败时保留旧数据。
  - `LayoutThatFits::new(first).or(fallback)` 要求至少一个布局。`TreeTable::try_new` 要求首列。`LazyGrid::try_new` 使用 `LazyGridOptions` 并报告无效列数或行高数量。`Grid::try_new(columns, content)` 在布局前校验命名列。
  - 属性表格行圆角跟随当前主题的 `radius.sm`。

- **下拉菜单、侧边栏、默认主题与 DevTools 帧回读（2026-11-02，破坏性）**：
  - 修复下拉点击无反应、切页才出现：主窗口的 portal store 现在在挂载前就绪，`PortalHost` 与弹层使用同一个
    store；`Select`、`ColorField` 的展开状态归各自 scope 持有，不再共享，也不会被其他面板带开。
  - 菜单高亮改为响应式，跟随指针和方向键，同一时间只有一行高亮；高亮改用浅色强调色，文字颜色不再反转。
    `Select::width` 现在决定触发器宽度，弹层至少同宽并随内容变宽。
  - 侧边栏行撑满并有统一边距，hover 和选中区域覆盖整行；header、footer、分组标题共用一列；选中行也响应 hover。
  - 破坏性：折叠按钮移到顶部，新增 `AdaptiveSidebar::brand` 槽位与其同行；`Sidebar` 的 header、footer 增加边距，
    `SidebarPresentation::gutter` 给出其值。
  - 默认主题更柔和：弱化 surface 与边框对比度，圆角阶梯调为 0/6/8/12/16。
  - DevTools 补齐 Swift 的 host 接线：`mirror.start`/`stop` 驱动帧回读（含 Android），mirror 期间持续出帧，
    并接上 `mirror.activate`、`window.resize`、状态观察、失效日志与渲染树；空闲 host 收到请求时会被唤醒；
    检查器 overlay 与拦截改用实际的 `SceneInspector`。破坏性：`attach_frame_tap` 增加 reset 参数，
    `set_state_registry` 改为 `set_state_source`，`DiagnosticsCallbacks` 增加字段。
- **主线程、UI 线程、渲染线程分离（2026-11-02，破坏性）**：
  - 桌面端主线程只做平台泵，组合、布局和绘制列表运行在新的 `cove-ui` 线程。`application!` 生成的 `run()`
    改走 `ApplicationLaunch::launch`，声明在 UI 线程上构建；`COVE_SINGLE_THREAD=1` 退回单线程。iOS 和 Android
    的 UI 仍在系统主线程上，平台调用走直通路径。
  - 新增 `cove_runtime::platform`：可合并的调用进每窗口槽位，有序调用走 FIFO，读取来自发布的窗口快照，
    结果作为消息回投。任何一侧都不等待另一侧；UI 线程无响应时，主线程在退出请求后 5 秒放弃它。
  - 帧管线改为最多 2 帧在飞：`ThreadedWindowRenderer::submit` 立即返回，无空位时不录制；结果和错误由
    `take_presented` 取回。`close` 最多等 2 秒。Android 的 GPU 工作移到每个 surface 的渲染线程，渲染线程
    持有自己的 `ANativeWindow` 引用。
  - 无障碍改为快照提交，取值读快照，动作排队回 UI 线程。原生菜单和剪贴板读取改为回调：
    `NativeMenuPresenter::present` 和 `ClipboardHolder::request_read`。
  - 窗口状态每轮事件泵读取一次并缓存，不再每次读取都调用原生接口。Debug 构建的原生入口断言主线程。
  - 新增 `PlatformThreadWebViewHost`：UI 线程上组合的 `WebView` 必须用它包裹平台 host，否则视图不会创建。
    新增 `PlatformThread::post` 和 `ThreadPoster`。
  - 修复：`wake` 推送失败或与轮询竞争时丢失唤醒；首帧之后录制列表丢失 `geometry_antialiasing` 与
    `clip_revision`，导致抗锯齿模式隔帧错误、图层缓存反复失效；DevTools 收到的是回收后的空绘制列表；
    macOS 14 以下重复 `CVDisplayLinkStart` 被当作失败；`par_map` 等待时可能把图片解码等独立任务拉到调用线程，
    现在只协助 `par_map` 分片，且任何退出路径都等齐所有分片；iOS 入口从未进入 `SDL_RunApp`，且
    `run` 返回后立刻释放 GPU。
  - 破坏性：`AppWindowRenderer` 只在渲染线程使用，host 持有具体类型 `ThreadedWindowRenderer`；
    `native_menu::install` 增加 presenter 参数；`DiagnosticsSession::presented` 在移交帧时调用，
    不再等呈现完成。
- **compose 集成测试合并为单一二进制并清零失败（2026-11-02）**：
  - `crates/compose/tests` 的 248 个文件合并为 `--test compose`（`tests/main.rs`，`autotests = false`），
    只链接一次、共享一个进程；新增测试文件需在 `main.rs` 声明，漏声明会被 `every_test_file_is_declared` 拦下。
  - 修复翻译遗漏：文本选择成员在节点卸载时未注销；虚拟列表锚点丢失行号与内缩导致行高变化时滚动跳动；
    Slider 拖动被外层触摸滚动抢走捕获；Carousel 翻页按钮缺少无障碍标签；Split 紧凑选择器长标题溢出标签；
    Toolbar 溢出入口消失时焦点未交还第一个命令。
  - `work::wait_idle` 只等待，不再把任务拉到调用线程执行。
  - 将已实现行为对应的过时 GAP 断言改回 Swift 预期。
- **Gallery 演示段回填第一批（2026-11-02）**：
  - Chart 页从 1 段恢复到 Swift 的 12 段：趋势（面积+线、阈值、走廊、标注）、分组/堆叠柱、零线负值、双轴、
    对数轴、直方图、大数据量缩放与刷选、散点、环形、迷你图、横向柱、数据表。
  - Button 补齐「图标与提示」「长标题」「尺寸」；Rating 补齐整星、半星、十分制、只读与禁用、密度、双向；
    Carousel 补齐自动播放、多页、无循环无箭头、空集合；TextField 补齐密码与只读、装饰与上限、Unicode；
    Slider 补齐步进与禁用；Icon 补齐语义尺寸与语义着色；Toggle、StatusBar、Link 各补齐到 Swift 的段数。
  - 演示段总数 115 → 136（Swift 为 195），其余单段页面待继续回填。
- **进程级字体标识与并行文字排版（2026-11-02，破坏性）**：
  - 新增 `font_registry`：字体 ID 由文件、字面、可变轴、字号和栅格缩放决定，全进程唯一，任何线程排出的
    字形都指向同一字体。字体文件只读一次并跨线程共享，FreeType 字面按线程打开。
  - 同一字体文件不再按字重和字号重复读取：冷挂载一页的字体数据从 55 MB 降到 8 MB，页面冷挂载从约
    41 ms 降到约 31 ms。
  - `TextEnvironment::request_shape` 排队待排版文字，下一次未命中缓存时整批处理：字体回退留在调用线程，
    HarfBuzz 在工作线程运行，平台彩色字体仍在加载线程排版。`Text` 标签已接入。
  - `work::par_map` 改为动态领取任务，慢核或晚启动的线程不再拖慢整批。
  - 破坏性：`FontProvider::new` 去掉 `id_base`，`TextFontResolver::new` 去掉 `provider_id_block_size`。
- **清理过时 GAP 断言与测试夹具（2026-11-02）**：
  - 将按钮按下激活、抽屉键盘输入、紧凑对话框尺寸和坐标、侧栏开启与身份保留、虚拟列表滚动、
    自动尺寸约束及冻结列对齐等过时预期改为正常回归检查；同步更正相关迁移记录。
  - 测试固定语言并在退出时恢复，窗口调整同步实际布局宽度；补齐 Disclosure 测试包装的状态订阅，
    连续滚动复用同一事件分发器，并显式使用点单位。触屏菜单按公开语义查找，不再依赖桌面私有标记。
  - 修复菜单尺寸修饰器覆盖滚动布局回调、放大文字后的菜单视口预算、冻结列滚轮路由、侧栏和工具栏
    焦点交接；列表标题按剩余槽位宽度布局，富文本按内容测量并响应字体修饰，Markdown 标题和代码块
    分别使用标题字级与保留换行的横向滚动布局。
- **本地化落地：目录、Calendar 与无障碍文案（2026-11-02，破坏性）**：
  - 新增 `crates/compose/resources/Locales/en.json` 与 `zh-Hans.json`，以 `include_str!` 编入 crate。
    `LocalizedStringKey::resolved()` 现在真的能查到译文：先查调用方指定的 bundle，再查内置目录，再查
    可执行文件旁的资源文件，最后回退到英文目录与 key 本身。此前机制齐全但仓库内没有任何语言资源，
    `resolved()` 永远返回 key 原文。
  - 新增 `i18n::tr(key)`、`i18n::format_template(template, values)` 与线程级 `set_locale_override`；
    场景测试统一固定 `en`，标签断言不再依赖 `LANG`。
  - `calendar` 模块导出 `month_name`、`month_abbreviation`、`weekday_symbol`、`month_title`、
    `long_date`、`medium_date`，月份与星期名走目录：中文渲染「2026年3月」与「日一二三四五六」，英文
    保持「March 2024」与「S M T W T F S」。`date_picker` 与 `date_range` 里三份重复的月份名表改为
    调用它们。
  - 修复 `CalendarPeriodCell` 无障碍名重复年份（「March 2024 2024」）；`calendar_modes_tests` 中四处
    记录该缺陷的断言同步改为正确值。
  - 30 个文件里 38 处 `label:` / `help:` 英文字面量改为 `i18n::tr()`，覆盖侧栏、工具栏、分栏、滚动条、
    图表、数据表格、进度、评分等；`TimePicker` 的 `Hour` / `Minute` / `Second` / `AM or PM` 轮名同样
    走目录，`Rating` 的「4.5 out of 5」改为模板键 `rating.out_of`。
  - 新增 `localization_tests`：分别以 `zh-Hans` 与 `en` 挂载月历，断言标题与星期行确实按语言渲染。
- **时间控件补全约束与本地化（2026-11-02，破坏性）**：
  - 修复 `TimePicker::current()` 只把小时夹进 `[minimum, maximum]` 的缺陷：`minimum = 09:30` 时滚轮停在
    09:00，一提交却跳到 09:30。夹取改为按总秒数进行，`commit()` 与滚轮位置共用
    `TimePickerOptions::clamped()`，两者不再可能不一致。
  - `TimePickerOptions` 新增 `period_labels: Option<(String, String)>`，12 小时制的 AM/PM 可换成
    「上午/下午」；`periods()` 与半日轮的无障碍名称都走它（默认仍为 `AM`/`PM` 与 `AM or PM`）。
  - `TimePickerOptions` 新增 `disabled_ranges: Vec<(TimeOfDay, TimeOfDay)>`（半开区间），把底层
    `WheelPickerOption::is_enabled` 透到时间轮上：整小时被覆盖时该小时行置灰，只有部分被覆盖时置灰
    对应的分钟行；12 小时制的一行同时代表上午与下午，两段都被覆盖才置灰。反序与空区间在 `validate()`
    中归一化或丢弃。
  - `TimeFieldOptions` 新增 `hour_cycle` 与 `period_labels`，支持 `hh:mm AM/PM` 的显示与解析（裸
    `HH:MM` 仍按 24 小时制解读，粘贴 `21:30` 不会被折到上午）；占位符随制式变化。
  - 两个时间控件的默认分钟步长统一为 `date_values::DEFAULT_MINUTE_STEP`（5 分钟）；`TimeField` 此前
    是 1，仅影响方向键步进。
- **Gallery 逐页验收：98 页中 97 页渲染真实控件（2026-11-02，破坏性）**：
  - 路由表 `StoryDispatch` 从 58 个分支扩到 97 个；`MigrationPending` 更名为 `StorySource`，只记录每页的 Swift
    来源文件，删除了与仓库不符的「缺失控件」列——`Slider`、`TextField`、`Combobox`、`PropertyGrid`、`TabItem`、
    `Tooltip`、`CarouselPage` 等此前被列为缺失的控件都早已存在于 `cove-ui-compose`。`Workspace` 是唯一仍无故事的页面。
  - 新增 39 个真实交互故事：`SliderStory`、`RangeSliderStory`、`TextFieldStory`、`SearchFieldStory`、`TextAreaStory`、
    `NumberFieldStory`、`Vec3FieldStory`、`ColorFieldStory`、`SelectStory`、`JsonFieldStory`、`AssetRefStory`、
    `ListStory`、`ReorderableStory`、`ScrollbarStory`、`TimelineStory`、`IconStory`、`MenuButtonStory`、`SplitButtonStory`、
    `KeyCapStory`、`LinkStory`、`PanelStory`、`SplitViewStory`、`ScrollViewStory`、`BoundedScrollStory`、`PropertyGridStory`、
    `ZStackStory`、`StatusBarStory`、`CarouselStory`、`DatePickerStory`、`DateRangeStory`、`TimeFieldStory`；
    `Theme`、`Layout`、`Table`、`Radio`、`Form`、`Combobox`、`Notification`、`Tooltip` 此前已有故事但未接入路由，现已接通。
  - 空壳故事补齐控件与状态：`TabsStory` 渲染 `TabView`（内容页、分段、可关闭文档）、`TreeStory` 渲染带搜索与多选的
    `Tree`、`NavigationStackStory` 渲染真实页栈、`TimePickerStory` 渲染 `TimePicker`、`ComboboxStory` 同时接入本地与
    远程数据源、`NotificationStory` 挂载 `NotificationHost`、`TooltipStory` 附着真实气泡、`FormStory` 使用 `Form` 与校验。
  - 修复 `Button::title` 不设置无障碍名称的问题：标题按钮现在以可见标题作为可访问名称（此前进度条/触发器被读作空标签）。
  - 修复 `PlacementExample` 的触发器闭包捕获构建时下标，导致第一次点击后循环不再前进。
  - 修复 `AnimationDemoNote` 未实现 `body` 导致 Animation 页在文本选择外壳下 panic。
  - 以真实行为替换固定旧缺陷的断言：Tree/List 页的「尚无故事」、Animation 页的 panic、侧栏内容面板位置、
    百分比内边距（现按包含块计算）、虚拟栈 `scrollToIndex` 与抽屉方向键焦点。
  - 修复 `drawer_tests` 的 `Shared<V>` 未转发 `dynamic_properties`，造成关闭状态不触发视图重组的测试缺陷；
    关闭、焦点作用域释放与摘要更新断言现已通过，并补充真实 Gallery 路径的关闭验证。此前对
    `ModalFocusResource` 卸载时机的归因已在 `Docs/rust-migration-validation.md` 更正。
- **绘制遍历去掉逐节点分配（2026-11-02）**：
  - `DrawListResources` 改为写时复制的共享块：合成时追加缓存图层只共享其资源列表（O(块数)），
    去重延后到遍历时（渲染线程每帧一次），指针哈希替代 SipHash；克隆缓存图层列表不再复制哈希集合。
    此前每帧把所有缓存图层的资源逐个重新哈希进帧集合，是缓存帧最主要的开销。
  - `RenderTree::paint_plan` 返回共享的 `Rc<PaintPlan>`；图层渲染器的裁剪栈改为共享 `Rc<[Clip]>`，
    子节点判断改用 `has_children`，不再每个节点克隆子节点列表与裁剪栈。
  - 新增被忽略的 `draw_recording_benchmark`。测量结论：无变化帧约 0.04–0.07 ms（根图层整体复用），
    并行化绘制列表构建与命令缓冲编码没有可观收益，暂不实现。
- **线程化：工作线程池、后台图片解码、独立渲染线程（2026-11-02，破坏性）**：
  - 新增 `cove_core::work`：进程级工作线程池（核数减一），`spawn`、`spawn_then`（结果回投发起线程的
    收件箱，在下一帧开头由 `drain_scene_invalidations` 统一应用）、`ui_channel`/`UiSender`（跨线程向
    UI 线程回调）、`par_map`（按序保持的切片数据并行，调用线程参与处理，可在工作线程内嵌套）以及
    `wait_idle`。
  - `AsyncImage` 的下载与解码改在工作线程执行，同一缓存键的并发请求共用一次解码；渐进预览经
    `UiSender` 回投。`ImageLoadServices::decode` 改为 `Send + Sync` 的 `ImageDecode`，新增
    `ImageDecodeExecution`（注入的服务默认内联执行）。`AsyncImageSession` 变为可观察，并在作用域内
    跨父视图重建保留，后台结果到达后会重组内容。
  - `ImageDownloadTransport` 要求 `Send + Sync`，`ImageDownloadTransportHolder` 改为进程级（`Arc`）。
  - 矢量图在尺寸或密度变化后改在后台重新栅格化，期间继续绘制上一份栅格。
  - 每个桌面窗口的呈现移到独立渲染线程（`ThreadedWindowRenderer`）：UI 线程交出一帧 `DrawList`
    后即可开始下一帧，最多一帧在途，缓冲区循环复用；配置、缩放、关闭与字形图集上传按通道顺序执行。
    `AppWindowRenderer::draw` 改为接收 `&mut DrawList`，新增 `upload_font_atlas`；
    `NativeDrawListRenderer` 的共享状态改为 `Arc<Mutex>`，可跨线程移动。设置 `COVE_SYNC_RENDER=1`
    可回到同线程呈现。
  - 测试场景的 `settle` 与宿主循环一致，先等待后台任务并排空收件箱。
- **作用域状态迁移与交互修复（2026-11-02，破坏性）**：
  - `CalendarView`、`Combobox`、`NavigationStack`、`Sidebar` 及 Gallery 日历示例改用 `cx.state`；
    父视图重建保留浏览月份、搜索会话、导航过渡和本地展开状态。删除 `Sidebar::local_expanded` 与日历示例的旧状态字段。
  - 日历翻页、面板切换会立即重绘；复用日期节点时刷新语义与焦点，支持 PageDown 换月后回车提交，
    修复年份翻页的借用冲突与禁用后的残留焦点。
  - 修复 Popconfirm 打开后再次点击触发器无法关闭；气泡布局与绘制保留指定的溢出策略，并按可用侧边空间收窄。
  - 侧栏补齐方向键展开、折叠和父子焦点移动，RTL 正确镜像；无布局节点的输入宿主继承祖先方向。
    行高落实最小值，roving focus 定位到行内实际可聚焦控件。
  - 关闭的滑动行操作从无障碍树与命中测试中隐藏，滑动不再误触操作按钮。
  - 接通 Combobox 远程数据源、防抖、取消和过期结果过滤；原生平台使用工作线程与 UI 收件箱，
    Wasm 在场景防抖时钟到期后执行同步查询回调。导航过渡期间的路径变更保护恢复生效，返回手势使用实际页面宽度。
  - 以实际键盘、指针和无障碍行为替换已修复缺陷的 GAP 断言；剩余旧状态调用与后续批次见 `Docs/state-migration.md`。

- **Taffy 布局能力与 Yoga 残留清理（2026-11-02，破坏性）**：
  - 删除不再参与构建的 `crates/core/native/yoga` 桥接与头文件；对齐转换函数改为
    `layout_justify`、`layout_align`、`layout_values`，官网、示例和开发工具文档同步更新布局引擎名称。
  - 启用 Taffy `grid` 和 `calc`。`LayoutStyleValue` 支持百分比与逻辑点组合、上下限，以及
    `min-content`、`max-content`、`fit-content` 尺寸；表达式贯通宽高、flex basis、定位、边距、间距和最小/最大约束。
    `FrameDimension` 统一为该类型，新增 `FrameValues` 与 `LayoutValuesExt`；最小/最大约束改用统一尺寸值。
  - 新增 `GridStyle` 与 `CssGrid`：支持 `fr`、`minmax`、重复轨道、命名线、自动放置与跨格。
    普通 `Grid` 改用 Taffy 计算轨道与放置，保留命名列显隐和子视图身份；溢出轨道正确计入滚动范围。
    `LazyGrid` 保留虚拟化与范围计算，固定行高单元格填满分配列宽，修复空内容宽度为零。
  - 内容最小尺寸通过独立测量模式传递，文本按不可换行片段计算；根节点计算方向传递到自定义布局，
    RTL 切换会重新放置。更新仍断言旧布局缺陷的回归测试。接口与边界见 `Docs/layout.md`。
- **Ghostty 源码更新至 v1.4.0（2026-11-02）**：
  - `third-party/ghostty` 固定到官方 `v1.4.0`（`83fb95624b0439ee660c2679b43b38c8b63769cd`），
    `cove-terminal` 使用该版本附带的 libghostty-vt；VT 库上游版本仍为 `0.1.0-dev`。
  - 源码构建使用 Zig 0.17.0，显式关闭 xcframework 与 macOS app 输出，只构建供 Rust 静态链接的 VT 库；Windows 链接新版 `ghostty-vt-static.lib`，各平台 CI 工具链同步更新。
  - 新增与实际链接库 ABI 清单对照的测试，校验所有已用 FFI 结构体的大小、对齐和字段偏移。

- **状态模型改为作用域持有的 Signal（2026-11-01，破坏性）**：
  - `View::body` 改为 `fn body(&self, cx: &Scope) -> AnyView`。状态用 `cx.state(|| 初值)` 创建，归视图的
    组合作用域所有：视图在树中身份不变，状态就保留；作用域卸载时随之释放。此前状态放在视图值里，
    父视图每次重建都会新建 `State`，只能靠手写 `dynamic_properties` 按名字拷回，72 个视图漏写、
    多处在 `body()` 内 `State::new`，导致状态重置或永不重组。
  - 新增 `cove_core::reactive`：`Signal<T>` 是 `Copy`、`!Send` 的句柄，`get`/`with` 自动订阅当前观察者，
    `set`/`update` 使订阅者失效；另有 `batch`、`untrack`、`set_if_changed`、`track`，以及跨线程写入用的
    `SignalSender`（投递到场景线程收件箱）。热路径无锁。
  - `Binding` 改为单线程（`Rc`），不再要求 `Send + Sync`；`Signal` 可直接转为 `Binding`，经 Binding
    读取同样被追踪。由此修复 `Binding` 驱动的视图无法重组：`Popover`、`MenuButton`、`SplitButton`、
    `Popconfirm`、资源选择器等此前都打不开。
  - 组件的协调过程在其观察者下执行：组件直接物化的原语在 `update_node` 中读取 Binding 也会订阅该组件。
  - `cx.remember` 保存非响应式的作用域值；`cx.keyed_state` 按键区分列表项状态。
  - 修饰后视图的槽位标识包含修饰符类型与内容标识（对应 Swift 的 `ModifiedContent<Content, Modifier>`）。
    此前所有修饰后视图共用一个标识，列表结构变化时会把标题节点原地更新成行内容。
  - 已迁移：`MenuButton`、`Popover`、`ContextMenu`、`Modal`、`Popconfirm`、触屏 `HoverCard`、`TouchMenu`、
    `PortalHost`、`NotificationHost`、`CommandPalette`、`SwipeRow`、`TabOverflowStrip`、`MobileTabBar`、
    `ResizableEditor`、`ResizableTextArea`、资源选择器。旧 `State`/`DynamicField` 暂时保留，其余视图分批迁移后删除。
- **补齐 Swift 行为（2026-11-01）**：
  - `Popover` 增加 `trigger_style`、`on_key` 透传、打开时 `z_index` 提升，以及触屏 sheet 分支
    （`AdaptiveOverlayHost`/`TouchOverlay`）；触发器的无障碍描述改用 composition local。
  - `MenuButton` 不再嵌套两层按钮，并在安装原生菜单时使用系统菜单；`ContextMenu` 补上原生菜单、
    触屏长按、触屏 sheet 与弹出过渡，新增 `ContextMenuExt`。
  - `ToastController` 恢复可观察性：发布、关闭、暂停、到期都会使读取方重组，此前提示卡片从不出现。
- **布局引擎改为 Taffy，布局树重构（2026-11-01，破坏性）**：
  - `LayoutNode` 改为目录模块（`style`、`engine`、`custom`）。手写 flex 实现替换为 Taffy 0.15 的底层
    trait，直接挂在 `LayoutNode` 树上：每个节点自带样式、布局缓存和结果，脏标记只清理所在路径，
    重新布局只访问脏路径。此前没有测量缓存，嵌套测量按深度指数增长。
  - 与 Yoga 语义对齐：默认列方向、`flex-shrink: 0`、`align-content: flex-start`、无自动最小尺寸、
    RTL 由引擎镜像；logical `start`/`end` 在布局时按生效方向解析并优先于 `left`/`right`；
    `StyleEdges::set` 不再接受方向参数。
  - `LayoutNode::frame()` 改为父节点相对坐标（Yoga 约定），`ViewGraph` 原样写回。此前子节点被放到祖父
    坐标系、写回时再减去父原点，而自定义布局放置的是局部坐标，两套坐标混用导致侧栏行错位、
    自定义布局子节点偏移。
  - 自定义布局按 Swift `CustomLayout.swift` 移植：对 flexbox 是测量叶子，子节点在 flex-start 列式
    probe 中布局，每个子节点缓存 4 个 proposal，放置在 flex 之后、仅在 bounds 或方向变化时执行。
  - 最小尺寸下限改存 `LayoutNodeData::minimum_size_floor`（原 `__minimumSizeFloor` attachment）。
- **修复 gallery 白屏与外壳布局（2026-11-01）**：
  - `LayerRoot` 改回 Swift 的单节点结构（`[content, portal layer]` 两个子节点）。此前返回双元素
    `TupleView`，外层修饰符同时作用到 portal 层，背景色把整个窗口盖成白色。
  - `AdaptiveSidebar` 按 Swift 补全外壳：rowReverse 内容与停靠栏、紧凑模式触发条、调宽手柄、绝对定位
    的遮罩与导航面板、边缘滑动；新增 `header`、`footer`、`collapse` builder（文档已提到但未实现）。
  - gallery 外壳按 Swift 使用 `SafeArea`、`LayerRoot`、`AdaptiveSidebar` 与 `.appearance(_, .circular())`；
    删除只写节点背景、不传递主题的 `WorkspaceAppearanceModifier`。

- **原生依赖改为 Cargo 构建脚本直接编译（2026-11-01，破坏性）**：
  - 新增 `cove-native-build`（`crates/native-build`），用 `cmake` crate 把 `third-party/` 子模块编译进各
    crate 的 `OUT_DIR` 并静态链接，统一处理 macOS/iOS 部署目标、Android NDK、wasi-sdk 与 MSVC 运行时。
  - `cove-text`（FreeType、HarfBuzz）、`cove-shell`（SDL3）、`cove-imaging`（plutovg、lunasvg、libwebp）
    不再读取 `vendor/`、`Native/vendor` 的 artifact bundle；lunasvg 使用独立的 plutovg 子模块。
    `cove-terminal` 始终以 `zig build` 编译 libghostty-vt。
  - 删除 `COVE_IMAGE_ARTIFACT_ROOT`、`COVE_GHOSTTY_VT_ROOT`、`COVE_GHOSTTY_ARTIFACT_ROOT`，以及 SwiftPM
    时代的 CMake 暂存工程、artifact bundle 清单与 modulemap 模板（`cmake/`、`Native/`、
    `third-party/cmake`、各 crate 的 `native/CMakeLists.txt`）。
  - 子模块统一在 `third-party/`：`sdl3` 由 `Native/third-party/sdl3` 移入，新增 `lunasvg` v3.5.0、
    `plutovg` v1.3.3、`libwebp` v1.6.0，均为浅克隆。Android runtime 的 SDL Java 源路径随之更新。

- **删除 Swift 源码，修复迁移遗留缺陷（2026-11-01，破坏性）**：
  - 按 git 路径白名单删除 1,369 个 Swift 文件（`.swift`、`Package.swift`、`Package.resolved` 与 4 个
    `module.modulemap`）及只解析 Swift 包的 `Tools/Architecture`、`Tools/Migration`；
    `Docs/rust-migration-files.json` 保留原路径与 SHA-256。`release.yml` 改用 `cargo xtask audit`。
  - `List` 改回 Swift 的 `FixedVirtualStack` 虚拟化：此前逐行克隆整份 `ids`（20 万行约 160 GB）并挂载
    全部行。`List::ids` 改为 `Rc<[Id]>`，`List`/`VirtualStack` 的 `Id` 需实现 `Hash`；键盘入口改用
    `SelectionKeyHost`；`anchor`/`cursor` 声明为状态，父视图重建后不再丢失。
  - `FixedVirtualStack` 按 Swift 重新移植：写回 viewport、首尾占位、偏移写入共享窗口、按行 id 保持身份、
    每个栈值新的 generation；`visible_range` 在零高度时仍返回首行。
  - `.flex(grow)` 与 Swift 一致带 `shrink: 1`（此前只设 grow，滚动容器随内容无限增高）；删除
    `FlexGrowModifier`。
  - `Node::reorder_children`、`LayoutNode::reorder_children` 的成员校验由 O(n²) 改为哈希集合。
  - 修复目录重排误改：`HTTP/1.1` 请求行与 WebSocket 握手、`text/markdown` MIME、CI `http.version`；
    `examples/gallery`、`examples/counter` 的 git 大小写；xtask Android 字体与 runtime 路径；CI counter
    清单路径。
  - 测试 `GlobalTestLock` 改为同线程可重入，修复嵌套加锁导致的死锁；新增 `clippy.toml`。

- **继续测试迁移并整理 Cargo 工作区（2026-11-01，破坏性）**：
  - 核对第三批 65 个测试目标，补入盘点遗漏的设计系统套件；修复 ScrollView、Tooltip 与
    DataTable 测试的 Rust API 对接错误。
  - crate 统一放入 `crates/`，使用 `src/`、`tests/`、`native/`、`resources/` 与根 `build.rs`；
    示例放入 `examples/gallery/`，打包工具放入 `xtask/`，集成测试由 Cargo 自动发现。
  - 新增 Rust `cargo xtask audit`，保留删除前 1,348 个 Swift 文件的校验值与映射，检查实际
    src/C/Cargo 目标和目录；验证记录见 `Docs/rust-migration-validation.md`。
  - 浏览器构建、调试配置与 CI 改用 Cargo；浏览器 JS 按 Rust 分配器 ABI 传入释放长度。
    移除依赖旧 Swift CLI 的浏览器命令及外部 Diffuse 的 Swift shader-sdk CI 作业。



- **新增 `cove-agent`（`Agent/`），agent/MCP 层脱离 UI 栈**：
  - `cove-devtools` 的 `agent/`、`agent_transport.rs` 与 `inspection/tools.rs` 移入，只依赖 serde；
    `cove-devtools` 以 `pub use cove_agent as agent` 与 `agent_transport`、`inspection::tools` 保留原路径。
  - 恢复 Swift 版行为：`McpServer::with_provider` 每次 `tools/list`/`tools/call` 重新读取工具并声明
    `listChanged: true`；`ToolExecutor` 与新的 `AgentModel` 改为 `Arc<dyn Fn + Send + Sync>`，
    工具箱可交给 stdio/TCP 服务线程（破坏性：原 `Rc` 执行器需改为 `Arc`；`McpServer::tools`
    改为返回 `Result<Vec<AgentTool>, _>`）。
  - 新增 `notification()`、`transport::LineWriter` 与 `serve_lines`，宿主可在响应之间写出
    `notifications/tools/list_changed`。
- **新增 `cove-http`（`Http/`），通用阻塞式 HTTP 客户端，响应体流式读取**：
  - 任意方法、请求头与请求体；非 2xx 作为响应返回；重定向策略统一（303 及 POST 的 301/302 改为 GET，
    跨源去掉 `Authorization`/`Cookie`，拒绝 https→http）；请求头注入与非法 URL 在发出前拒绝。
  - macOS/iOS 用 NSURLSession（`native/apple.m`，带背压）；Windows 用 WinHTTP；Linux、Android 与其他
    Unix 用 HTTP/1.1 + OpenSSL（`native/openssl.c` 运行时加载 `libssl` 1.1.1–4.x，校验证书链与主机名，
    读取系统信任库，含 Android `cacerts`）。
  - `cove-hosts` 的图片传输改为 `cove-http` 之上的薄适配：Linux 支持 `https://`；Apple 返回真实
    `Content-Type`；Android 首次有图片传输。`TransportCapabilities::PLATFORM` 改为运行时的
    `TransportCapabilities::platform()`（破坏性）。
  - Android 没有系统 OpenSSL：`cargo xtask android-prepare` 从 `COVE_OPENSSL_ANDROID_DIR` 打包
    `libssl.so`/`libcrypto.so`，未设置时 https 报告不可用。

- **应用模板与打包改为 Cargo 生态方式**：
  - `Templates/app` 是 `cargo-generate` 模板：共享 UI（`src/`）、`Assets/`、`Platforms/`
    （Android Gradle、macOS/Windows/Linux 平台文件）与 `xtask/`。应用身份写在
    `[package.metadata.cove]`。
  - 新增 `cove-package`（`Tools/Package`），以 `cargo xtask package|run|devices|android-prepare`
    调用：macOS `.app` 组装与 codesign；Windows 目录与 `mt.exe` 清单嵌入；Linux 目录与 `.desktop`；
    Android 以 `cargo rustc --crate-type cdylib` 产出 `libCoveAndroidApp.so`、暂存资源后由 Gradle
    出 APK（Release 默认调试密钥，可指定发布密钥），ADB 安装启动；iOS 直接把 Rust 二进制组成
    `.app`（Shell 经 `SDL_RunApp` 进入 UIKit，不再需要 Xcode 工程与 `main.swift`），模拟器 ad-hoc
    签名、真机按描述文件签名，`simctl`/`devicectl` 安装启动。
  - 新增 `cove_ui::application!(root)`：生成桌面/iOS 的 `run()` 与 Android 的 `JNI_OnLoad`，
    后者向 Android 宿主注册同一根视图（此前 `HostedScene` 只有 `UnlinkedContent`，APK 无法承载应用）。
  - Android JNI 胶水 `Hosts/Android/Sources/CAndroidHost/{Host,Input}.c` 迁移为
    `Hosts/rust/android/jni.rs`：`cdylib` 只导出 Rust 符号，静态库里的 C `Java_*` 符号会被隐藏。
  - `Examples/Mobile` 成为 Cargo 包 `cove-mobile-example`，删除其 Xcode 工程与 `main.swift`。
- **首次为移动目标完整编译并链接，修复其中暴露的缺陷**（此前 Android/iOS 路径从未被构建过）：
  - `Terminal` 的 `libghostty-vt` 缺少 Android 与 iOS 的 Zig 目标映射。
  - `Shell` 移动端 `PlatformApplication::run` 的泛型推断失败（`T` 回退为 `!`），无法编译。
  - Android 日志绑定把 `__android_log_write` 声明为 4 个参数（实为 3 个）。
  - `Imaging`/`Text` 在 Android 上以显式未版本化 sysroot 路径静态链接 `c++_static`：既与 `Shell`
    的 `c++_shared` 混用两套 C++ 运行库，又使链接器选中 NDK r30 自带 Rust std 的 `libc.a`，导致
    `rust_eh_personality` 重复定义。统一改为 `c++_shared`。
  - `Shell` 在 iOS 上缺少 `CoreMotion` 框架（SDL 传感器后端需要）。
  - `cove-ui-addons` 的 WebKit 宿主声称支持 iOS，实际只基于 AppKit（Swift 原版同样只有 AppKit）；
    改为仅 macOS 编译。**iOS（UIKit `WKWebView`）与 Android 的 WebView 宿主仍缺失。**
  - 验证：Android `.so` 导出全部 19 个 `Java_*` 原生方法与 `JNI_OnLoad`，Gradle 产出 APK；
    iOS 模拟器 `.app` 构建并签名。两者都未在设备或模拟器上启动。
- **移除 Cove CLI（破坏性）**：删除 `Tools/Cove`（`cove-cli` 及其 Swift 原版、项目模板、
  安装脚本）、仅被它使用的 `Tools/Support`（`cove-tool-support`），以及根目录 `cove` /
  `cove.cmd`。构建、运行、测试、交叉编译直接使用 Cargo；原生依赖由各 crate 的 `build.rs`
  构建。迁移盘点改为 `python3 Tools/Migration/inventory.py [--status [--json]
  [--require-complete]]`。CI 与发布流程改为 Cargo；SDK 版本唯一来源改为根 `Cargo.toml`
  的 `[workspace.package] version`，发布只产出 tag 与 Release 说明，不再打包原生预编译库。
  随之删除仅服务 Swift CI 的 `Tools/Testing/Windows.ps1` 与 `.github/actions/build-cache`。
  清单删除 75 个已不存在的 Swift 行：生产源码 915/915 已写入，测试待迁移 294/411。
- **测试迁移启动（2026-10-31）**：新写 5 个集成测试并在 `Compose/Cargo.toml` 注册
  `[[test]]`——`select_enumeration`、`offscreen_reuse`、`scratch_geometry`、
  `recomposition_paint`、`chrome_component`，经 `cargo check -p cove-ui-compose --tests`
  验证 0 error。重新生成清单后，测试已写入 118/424、剩余 306；另 15 个早已落盘但旧
  JSON 漏标的 `[[test]]` 目标被正确置为 `written_unverified`（目标文件均真实存在）。
  迁移期仍不运行 `cargo test`，待全部转换后统一验证。
- **生产源码迁移完成**：984 个 Swift 生产文件中 983 个已有 Rust/C 实现，唯一例外是
  `NSNumber+Boolean.swift`（Objective-C 的 `NSNumber` 布尔扩展，Rust 无对等物且不需要）。
  SwiftPM 清单 25/25 已建立 Cargo 清单，CLI 项目模板 5/5 已改写为 Cargo 工程。
- **补齐此前虚报为“已完成”的 69 个生产文件**。旧清单按文件名相似度映射，只要目标 `.rs`
  存在即记完成——`DataTable.swift` 落到无关的 `lists/table.rs`、`SplitView.swift` 落到
  25 行的 `view.rs`，因而得出“984/984”的假结论。清单生成器改为**符号校验**：索引全部
  Rust 声明（含 `handle!`/`payload!`/`identifier!` 宏调用与 `pub use ... as` 改名两端），
  逐文件核对 Swift 主类型名是否真实存在；对不上先按符号重定位，仍找不到则置未开始并清空
  错误路径。本轮据此重写：`DataTable`、`TreeTable`、`Tree`、`Chart`、`ChartDataTable`、
  `PropertyGrid` 全家、`SectionedList`、`SwipeList`、`ReorderableList`、`Combobox`、
  `SearchField`、`RangeSlider`、`ColorField`、`JsonField`、`TimeField`、`TimeInput`、
  `TimePicker`、`Vec3Field`、`AssetRefField`、`DateRangePicker/Calendar`、`SplitButton`、
  `MenuButton`、`CompletionPopover`、`SplitView`、`ResizableEditor`、`DisclosureGroup`、
  `NavigationRail`、`Tooltip`、`CommandPalette`、`ContextMenu`、`OverlayHost`、
  `ShortcutHost`、`Sheet`、`ToolbarMetrics`、`UIIcons`、默认主题预设、`WorkspaceView`、
  `ImmersiveWindowTitleBar`、`GalleryView`、DevTools 协议/诊断/传输类型，以及
  `NativeAppFrameCapture` 的真实 GPU 离屏渲染与回读。
- **修复布局引擎两处真实缺陷**（此前被测试以“已知差异”绕过，属于掩盖而非修复）：
  - `align_self`/`align_items` 交叉轴改为对**容器**求解。原实现取行内最高子项，导致
    100 高容器内 30 高子项居中落在 `y=0`（Yoga 为 `y=35`）。
  - **widget 级 `layout_direction` 从未生效**：`ViewGraph::compute_layout` 硬编码
    `Direction::Ltr`，`measure`/`arrange` 全程透传同一参数，从不读 `style.direction`。
    后果是所有 RTL 行排序无效，且 `Edge::End` 在无父节点时就把物理边烘死。现改为逐节点
    `effective_direction()` 解析，`Edge::End` 在布局期按生效方向解析。
- **修复清单生成器三处判定缺陷**：测试行被 `derive()` 提前标记导致后续映射遍历跳过；
  脚本自己清空的行被当成“操作者纠正”永久跳过；`CacheTests`→`caches`、
  `DrawListResourceTests`→`draw_list_resources` 这类复数目标名漏匹配。
- **CLI 模板改为 Rust 工程**：新增 `Cargo.toml.in`、`src/{main,app,resources}.rs` 与
  `tests/resources.rs`（可编译验证），`cove new` 不再拷贝 Swift 源码与 SwiftPM 清单。
- 以下缺陷一并修复：DevTools 诊断记录误用 `parentId`/`nodeId`/`missingIds`（Swift 与
  `DevTools/Client/*.js` 读的是 `parentID`/`nodeID`/`missingIDs`）；`capture_presentation`
  忽略模态作用域导致被遮控件仍报 `input_enabled: true`；`Tools::skills()` 返回空库；
  滑块 `shows_ticks` 从未绘制；`NumericTextFormat` 只认 ASCII 数字且把微小负数渲染成
  `-0`；`Tree` 搜索过滤不展开祖先；`NativeAppFrameCapture::reset()` 泄漏显存。

- **迁移清单此前虚报完成度，已改为符号校验并修正**。旧清单按文件名相似度映射 Rust 文件，只要目标 `.rs` 存在即记“已写入”，于是 `DataTable.swift` 被记到无关的 `lists/table.rs`、`SplitView.swift` 被记到 25 行的 `view.rs`，得出“984/984 全部完成”的假结论。`Tools/Cove/rust/migration_inventory.py` 改为索引全部 Rust 声明（含宏调用与 `pub use ... as` 改名两端），逐文件核对 Swift 主类型名是否真实存在；对不上时先按符号重定位，仍找不到则置 `pending` 并清空错误路径。按新口径，当年**实际有 69 个生产文件没有实现**。
- **补齐 35 个此前完全缺失的视图与类型**（本轮）：
  - Compose 数据：`DataTable`（含 model/selection/view，保留 20 万行虚拟化）、`TreeTable`、`Tree`、`Chart`、`ChartDataTable`、`PropertyGrid` 全家、`SectionedList`、`SwipeList`、`ReorderableList`。
  - Compose 控件：`Combobox`、`SearchField`、`RangeSlider`、`ColorField`、`JsonField`、`TimeField`、`TimeInput`、`TimePicker`、`Vec3Field`、`AssetRefField`、`DateRangePicker/Calendar`、`SplitButton`、`MenuButton`、`CompletionPopover`。
  - Compose 布局/浮层：`SplitView`（含分割条拖拽与键盘）、`ResizableEditor`、`DisclosureGroup`、`NavigationRail`、`Tooltip`（悬停/焦点触发、仲裁、Escape 关闭）、`CommandPalette`（排名过滤、翻页、回车执行）、`ContextMenu`、`OverlayHost`、`ShortcutHost`、`Sheet`、`GridInteractionHost`、`ToolbarMetrics`、`UIIcons`、`DefaultDarkTheme`/`DefaultLightTheme`。
  - 宿主与工作区：`WorkspaceView`（2651 行，含四个具名分割、标签拖拽重排、浮动层、紧凑呈现）、`ImmersiveWindowTitleBar`、`WindowDragRegion`、`WindowControlStrip/Button`、`GalleryView`。
  - DevTools：`DevToolsCodec`、`InspectionValidation`、`NativeAppFrameCapture`（真实 GPU 离屏渲染 + 回读）、`UIAccessibilityActions`、`DevServerConfigurationError::UnsupportedHost`。
  - Core：`WorkspacePanelDescriptor`、`WorkspacePanelRegistry`、`WorkspaceThemeEnvironment`。
- **修复布局引擎两处偏离 Yoga 的真实缺陷**（此前被测试以“已知差异”绕过，属掩盖而非修复）：
  - `align_self`/`align_items` 的交叉轴居中改为对**容器**交叉轴求解。原实现取行内最高子项，导致 100 高容器内 30 高子项居中落在 `y=0` 而非 `y=35`。
  - `direction: Rtl` 现在真正镜像主轴排布。原实现的 `arrange_flex` 只看 `flex_direction.is_reverse()`，`direction` 传入后未参与主轴放置，RTL 行仍从左往右；现与 Yoga 一致（首个子项落在右边缘）。
- **修复 DevTools 三处真实缺陷**：诊断记录曾输出 `parentId`/`nodeId`/`missingIds`，而 Swift 与 `DevTools/Client/*.js` 读的是 `parentID`/`nodeID`/`missingIDs`，JS 客户端一直静默取不到字段；`capture_presentation` 未考虑场景模态作用域，被模态遮住的控件仍报 `input_enabled: true`；`Tools::skills()` 返回空库，Swift 返回内置 ui-diagnostics 指南。
- **修复控件三处真实缺陷**：滑块 `shows_ticks` 从未绘制（`paint::ticks()` 完整但 `render()` 未调用）；`NumericTextFormat` 用 `char::to_digit` 只认 ASCII，阿拉伯-印度数字等 `Nd` 数字被拒（Swift `NumberFormatter` 接受），改为 77 个 `Nd` 区块表；微小负数渲染成 `-0` 而 Swift 为 `0`。
- **修复 `Tree` 搜索过滤不展开祖先**：命中位于折叠分支时不可达，改为与 Swift `filterAndAutoExpand` 一致（`is_expanded = explicitly_expanded || child_subtree_matches`），经实际运行验证行数/深度/路径键。
- **修复 `NativeAppFrameCapture::reset()` 泄漏显存**：原来只清 extent，纹理仍留在闭包里；现在 capture 与 source 共享目标槽，`reset` 真正释放。
- **补齐清单生成器的两处判定缺陷**：`derive()` 会把测试行提前置为 `written_unverified`，使后续测试映射遍直接跳过（新迁移的测试套件永远不显示）；脚本自己清空的 `pending` 行会被当成“操作者纠正”而永久跳过，无法再被符号恢复。

仍未开始：34 个生产文件（2,380 行，主要是 Gutter 家族、Graphics 原生后端补件、Swift 宏插件）、345 个测试文件、6 个 Cargo 清单、5 个 CLI 模板。

- **修复 `agent_transport` 回环测试的死锁**：`loopback_server_round_trips_one_line` 用 `read_to_end` 等待 EOF，但传输层是**持久连接的行协议**——`serve` 回完一行后继续读下一行、永不关闭连接，于是客户端等 EOF、服务端等输入，`cargo test --workspace` 永久挂起（用户报告的 `cove_sample.txt` 采样即为此）。测试改为先 `shutdown(Write)` 半关闭写端再读。

死锁解除后暴露出 5 个被它掩盖的真实缺陷：

- **SHA-1 轮常量错位**：压缩函数写成 `c = b; b = a.rotate_left(30)`，而标准是 `c = b.rotate_left(30); b = a`。摘要"看起来合理"但**任何公开测试向量都不过**，导致 WebSocket 握手的 `Sec-WebSocket-Accept` 永久错误（RFC 6455 示例令牌断言失败）。修正后 FIPS 与 RFC 向量均通过。
- **`E_FAIL` 常量错值**：写成 `-2_147_467_263`，实际是 `0x80004001`；`E_FAIL` 应为 `0x80004005`（`-2_147_467_259`）。所有 HRESULT 错误信息都在报告错误的码。
- **DX12 内联存储尺寸未随指针宽度调整**：`D3D12_SHADER_BYTECODE` 是两个机器字（32 位 8 字节 / 64 位 16 字节），但 `read_from`/`write_into` 硬编码 8 字节缓冲。64 位下 `transmute_copy` 的"目标大于源"是**未定义行为**，测试直接 abort。
- **JPEG 负系数位型溢出且公式错误**：`((1 << size) - 1) + value` 在 `size == 32` 时溢出，且负值应以 `value - 1` 的 `size` 位补码表示。修正为 `(value - 1) & mask`。
- **Vulkan 实例记录字段布局不符规范**：`VkAccelerationStructureInstanceKHR` 把 `instanceCustomIndex`、`mask` 写成独立 `u32`，而规范要求 `customIndex:24|mask:8` 打包进同一字、`offset:24|flags:8` 打包进下一字。原布局是 72 字节（含隐式填充），GPU 会读到错位的地址。

另修复 4 处被同一批测试掩盖的实现缺陷：

- **`FrameTap` 首帧恒被丢弃**：`last_capture_at` 初始化为 `0.0`，与全新时钟相等，`now - 0 < interval` 使每次会话的第一帧都不发送。改为 `-inf` 哨兵表示"从未捕获"。
- **JSON-RPC 解析错误码违规**：请求体不是 JSON 时返回 `-32600`（Invalid Request），规范要求 `-32700`（Parse error）。
- **`tick`/`record_input` 的 `RefCell` 重入 panic**：`if let Some(x) = self.state.borrow().field` 会让不可变借用存活到整个分支体，分支里调用需要 `borrow_mut` 的 `settle()` 即 panic（滚动动画 11 项测试全挂）；`record_input` 则在持有 `borrow_mut` 时调用内部 `borrow` 的 `position()`。均改为先取出标量/拷贝再借用。
- **`focus_next` 不显示焦点环**：键盘遍历沿用了上一次的可见性，Swift 的 `focus(_:visible:)` 默认 `true`，因此指针操作后改用键盘时会看不到焦点指示。

另修复布局引擎两处真实缺陷（见上一条），并修正 9 处测试自身的错误预期，均经交叉核对 Swift 实现或规范确认：`LogQuery::default()` 的 `Info` 下限、`Weak` 控制器被提前释放、命中测试忽略子节点 frame 偏移、滚动偏移符号、解析抗锯齿描边的半像素外扩、非图层节点被期望拥有缓存列表、`ancestors` 反转后取错端点、4×4 图集的 gutters 容量、`ChunkUploadAllocator` 的 64 KiB 下限与碎片保留语义。

- **开始统一测试与验证**（迁移完成后首次全量运行）。`cove-core` 从 10 项失败修至 **103/103 全通过**，`cove-system`（含进程契约 5 项）与 `cove-logging` 亦全部通过。修复的真实缺陷：

- **布局引擎 `gap` 从未生效**：`line_main` 计算时计入了间距，但逐子项推进的游标没有加上 `gap`，导致设置间距只是缩小内容而不移动任何元素，尾部元素压在起始边上。
- **`align_items: Stretch` 下容器宽度塌缩为 0**：交叉轴测量只取最宽子项，未认领被提议的交叉轴范围，使自动宽度的列布局宽度为 0。修正时限定为"仅当存在自动交叉轴尺寸的子项"，否则显式尺寸的行仍应贴合内容（该限制由 `grow_shares_free_space` 测试守住）。
- **`cove-system` 进程契约测试引用了从未创建的 fixture 二进制**：`contracts.rs` 的 5 项测试全部依赖 `cove_process_fixture`，但仓库中没有该程序，测试始终失败。新增 `System/rust/bin/cove_process_fixture.rs`（`output`/`interactive`/`environment`/`sleep`/`arguments` 五种模式），并修正测试对 `target/<profile>/deps` 与 `target/<profile>/` 的路径推断。
- **`LogRuntime` 的 `&self` 方法必然 panic**：`clone_handle` 从 thread-local 表里按指针找回自身 `Arc`，但构造函数从未登记，且工作线程与本线程的 thread-local 互不相通。改为在值内保存 `Weak<Self>` 自引用，任意线程可用；`register_runtime` 保留为幂等空函数以免破坏公开导出。
- **`LogQuery::default()` 隐含 `Info` 下限**：日志管道测试用默认查询读取 Trace/Debug 记录必然为空，改为显式 `minimum: Level::Trace`。

- 另修正 5 处测试自身的错误预期（均经交叉核对 Swift 实现确认为测试写错而非实现错误）：`Weak` 控制器在测试中被提前释放、命中测试忽略子节点自身 frame 偏移、滚动偏移符号、分析抗锯齿描边的半像素外扩、非图层节点被期望拥有缓存列表。

- **生产源码迁移全部完成：984/984（100%）**。本轮补齐最后两个文件：`DataStories`（19 项测试）与 `AdaptiveExamples`（13 项测试）。至此 17 个模块全部完成，`Docs/rust-migration-files.json` 中无任何 `pending` 条目，每条均有 Rust 目标。

- 新增 `DataStories`（19 项测试，已运行通过）：**树搜索在命中时保留其祖先**（"retains matching ancestors"——命中项与其路径可见，非命中的子节点被过滤，否则宽泛查询会倾倒整棵子树）、刷新阈值 64pt 且进行中不可重入、虚拟栈跳转到末行而非越界、滑行动作日志与未知行/未知动作的拒绝。

- 新增 `AdaptiveExamples`（13 项测试，已运行通过）：`supports(page)` 与 `sampleHeight` 两张手工表的**一致性**才是真正的不变量——测试断言二者互为补集、支持集无重复、且每个受支持页要么有确定高度要么明确按内容自适应。该测试发现 `ALL_PAGES` 漏掉了 `Vec3Field`（枚举存在但表里没有），已补入。

- 新增 `NavigationStories`（22 项测试，已运行通过）：一个 Swift 文件支撑十一个画廊页，全部接入 `StoryDispatch`。覆盖折叠面板的单/多展开模式（**关闭多选会回落到首项**，避免与刚选的模式矛盾）、禁用段不可切换、分页窗口的省略号与首尾页恒显、翻页在两端夹取、空数据集不暴露页码、标签独立移除与重置、对话框取消不提交、警示对话框仅在确认时执行、**底页重新打开回到初始停靠位**、拖拽越过阈值才提升/关闭、导航栈 push/pop 时根草稿保留。

- 修正一处自我豁免：我最初把这些故事中的六个标为"纯展示、无需移植"，但读源码后发现标签移除、对话框确认/取消、快捷键平台格式化等都有真实状态机，因此逐个补齐而非以注释绕过。

- 新增 `TextEditorStory`（21 项测试，已运行通过）：移植了故事私有的字节级 Swift 词法分析器——行注释止于换行前、字符串遵守反斜杠转义（含末尾反斜杠不越界）、大写标识符归为类型、数字与关键字着色、超 300 KB 不分析、`token_at` 二分查找；并移植了括号折叠扫描，**字符串与注释内的括号不参与折叠**、单行括号对不可折叠、嵌套块按起始行排序。**`KEYWORDS` 在 Swift 原文中 `self` 排在 `Self` 之前，并非字节序**，直接照抄会让 `binary_search` 失效；已重新排序并加测试断言该列表必须有序。生产源码达 981/984（99.7%）。

- 新增 `WebStory`（15 项测试，已运行通过）：状态行的优先级（失败带描述 > 加载中带目标标题 > 就绪/已加载兜底）、旧错误描述在已重新加载时被相位而非描述决定、后退/前进跟随宿主历史、重载常可用而停止仅在加载中、本地样例为内联 HTML 且与 Swift 页面一致。**Swift 侧此文件是 `#if os(macOS) && canImport(WebKit)` 并在他处显示"不支持"提示；Rust 侧无此门控**——`Sources/rust/webview` 已有 Apple/Windows/Linux/Android 四个宿主，因此各平台都渲染真实页面而非占位提示。

- 新增 `OverlayStories`（15 项测试，已运行通过）：一个 Swift 文件支撑六个画廊页（Menu/ContextMenu/Popover/Modal/Disclosure/Tabs），均已接入 `StoryDispatch`。覆盖菜单树深度（Settings → Appearance → Accent color 三层）、标签与分隔符不计为命令、100 行菜单中每第 10 行为分组子菜单、菜单 id 全局唯一、弹层与对话框的草稿保留/取消语义、披露组折叠后草稿仍在、关闭文档时**回落到相邻标签而非首个**、全部关闭后选择清空且可重置。首次断言把"顶层行数 > 叶子数"写反（子菜单是一行却含多个叶子），已修正为实际关系。生产源码达 980/984（99.6%）。

- 新增 `ProductStories`（20 项测试，已运行通过）：一个 Swift 文件内的五个独立故事——单选组（禁用项拒绝选择且被方向键跳过）、表单校验（空白名不满足 `required`、邮箱需同时含 `@` 与 `.`、提交失败同时报出两个字段）、本地 Combobox（不可用项不可选）、模拟机场目录（按代码与名称大小写不敏感匹配、空查询返回全部 12 条、`offline` 查询**故意失败**以便演练重试路径、无匹配返回空而非错误）、Toast 队列上限与计数。生产源码达 978/984（99.4%）。

- 新增 `SidebarStory`（12 项测试，已运行通过）：两个分组共 7 个目标、禁用目标仍列出但键盘跳过、详情面板在无选中时显示提示而非任意行、6 个目标的图标 slug 互不重复、图标/徽标开关、插槽行使用 36pt accessory 车道、收藏与抽屉开关。已接入 `StoryDispatch`（并移除了此前误放在同一页面的重复匹配分支）。

- 新增 `DataTableStory`（22 项测试，已运行通过）：200,000 行 × 33 列的确定性行生成（数量按 17 步进并在 250 处回绕、指标列由行 id 推导）、加载后清空选择并聚焦首行、每次跳转移递增请求号、名称必填与数量非负整数校验、数量按数值而非文本排序、树展开时仅在**所有祖先均展开**时显示（折叠隐藏整棵子树而非仅第一层）、千分位分组与状态行文案。生产源码达 977/984（99.3%）。

- 新增移动端 `SettingsScreen`（9 项测试，已运行通过）：三个分组（General 两个开关 + 外观折行；Accessibility 一个开关；无标题的 Sign out 破坏性行）、开关写回 `MobileModel`、外观选择仅接受 System/Light/Dark 三项且选择后关闭弹层、未提供的外观值被拒绝且不改动模型。

- 新增 `ImageStory`（12 项测试，已运行通过）：四种图片来源（bundled/remote/missing/empty）各自的 URL 解析与 URL 去空白、`empty` 不可重试且请求号不变、三种加载相位到占位面的映射——`Empty` 与 `Failure` 共用布局但文案不同（`No image` + 引导 vs `Image unavailable` + 错误描述）。已接入 `StoryDispatch`。生产源码达 974/984（99.0%）。

- 新增 `PlacementExample`（11 项测试，已运行通过）：12 种摆放与 `PopoverPlacement::ALL` 一一对应、紧凑镜像三向分支（leading 贴右、trailing 贴左、居中折半，且触发器超宽时夹到边缘不出现负坐标）、上/侧/下三档垂直位置、触发器宽度受视口上限约束。该组件是 Popconfirm 页的子组件而非独立页，故不接入 `StoryDispatch`。生产源码达 973/984（98.9%）。

- 新增 `CommandStory`（11 项测试，已运行通过）：8 条命令分 4 组、仅 `Deploy to Device` 禁用；文档承诺的 `GO`/`go f`/`appearance` 三种查询均按真实排序模型命中；禁用命令不执行也不入最近列表；最近命令新前旧后并封顶 3 条、重复运行不产生重复项。**测试发现我的实现把 `showsRecent` 传反了**（应为查询为空时显示最近项），据 Swift 源码 `showsRecentCommands && trimmedQuery.isEmpty` 修正。生产源码达 972/984（98.8%）。

- **补齐 `WheelPicker`/`WheelPickerOption` 整块缺失**：`Primitives/Controls/WheelPicker.swift` 此前在 Rust 侧完全没有对应类型，`StoryDispatch` 的阻塞登记里也标着 `WheelPickerOption` 待实现。现实现为 `Compose/rust/primitives/controls/fields/wheel_picker.rs`（12 项测试，已运行通过）：按值而非标签识别选项、禁用行可见但不可提交、步进双向跳过禁用行且两端不环绕、`Home`/`End` 跳到首个/末个**可用**行、可见窗口居中并在末端整体回滑。首次实现的回滑逻辑只退一步导致窗口从 7..11 而非 6..11 开始，由测试发现并改为整体重算。

- 新增 `WheelPickerStory`（11 项测试，已运行通过）：时/分/半天三档时间轮与 0…10 音量轮（10 禁用）、`Now`/`Reset` 的 24→12 小时折叠、分钟按 5 取整向下吸附、`09:05`/`12:00` 双位补零。生产源码达 971/984（98.7%）。

- 新增 `MarkdownStory`（9 项测试，已运行通过）：三份文档（完整发布说明 / 精简安装片段 / 可编辑草稿）均按声明结构解析；测试断言了 ` ```swift `、表格、`[x]`/`[ ]` 任务项、引用、粗体、删除线与中文段落确实出现在源文档中。草稿实际解析为 **3** 个块（标题、描述段落、列表）——我最初的预期写成 2，用一个临时探针打印真实块结构后修正了测试而非实现；探针文件已删除。生产源码达 970/984（98.6%）。

- 新增 `TimePickerStory`（10 项测试，已运行通过）：12 小时制下午夜与正午均显示 `12`、11:59 仍为 AM、13:45 为 `01:45 PM`；24 小时制零填充无标记；秒数开关的字段顺序；清空后显示 `No time`。已接入 `StoryDispatch`。生产源码达 969/984（98.5%）。

- 新增 `DrawerStory` 与 `RailStory`（`navigation_layout_stories`，12 项测试，已运行通过）：抽屉 20 个目标中跳过不可用项、`closesOnSelection` 决定选择后是否关闭、未选中时标题为 `none`；导航栏 72pt（有标签）/56pt（无标签）两档宽度、跳过禁用目标、`999` 等长徽标不被截断。已接入 `StoryDispatch`。生产源码达 968/984（98.4%）。

- 新增 `ToolbarStory`（6 项测试，已运行通过）：`Always` 命令在极窄宽度下仍在栏内、`Overflow` 命令**无论宽度都进菜单**（由测试确认的真实语义，非猜测）、自动优先级在空间不足时按低优先级先移出且平局保留源序、Run/Pause 标题随播放态翻转。已接入 `StoryDispatch`。生产源码达 966/984（98.2%）。

- 公开 `MenuItem::perform`：与 `ActionSheetAction::perform` 对应，使调用方可从快捷键或测试触发已声明命令。

- 新增 `ActionSheetStory`（10 项测试，已运行通过）：禁用命令仍渲染但其图标保留且被方向键跳过、`export` 由复选框门控、`delete` 标记为 destructive、首条可用命令跳过被禁用的首项、长列表按声明数量生成且 id 唯一。已接入 `StoryDispatch`。生产源码达 965/984（98.1%）。

- 公开 `ActionSheetAction::perform`：此前只有私有 `perform`，调用方无法从键盘快捷键或命令面板触发已声明的命令。

- 新增 `AvatarStory`（11 项测试，已运行通过）：图片源切换（`bundled`/`remote`/`missing`/`initials`，无图片源直接显示首字母）、`AvatarGroupGeometry` 的 limit/overflow/overlap 组合（6 人限 3 时显示 3 个头像 + 1 个溢出标记、限 0 时仅剩标记、`Hidden` 完全抑制标记、0.3 重叠使步进为 22.4pt）、Unicode 首字母用例含空名回落。已接入 `StoryDispatch`。生产源码达 964/984（98.0%）。

- **Gallery 故事页继续收口**：新增 `data_stories`（`ChartExplorationStory` 缩放/重置/换数据与笔刷说明、`AdaptiveTableSample` 自适应列可见性）、`navigation_stories`（`AppBarStory` 长/短标题、文本/图标主操作、溢出命令与结果记录）、`sidebar_stories`（`ResizableSidebarExample` 宽度重置、`NestedSidebarExample` 分组展开/收起保留选中项、叶子可选性）。共 25 项测试（已运行通过），并把 chart、appBar、sidebar 三页接入 `StoryDispatch`。生产源码达 963/984（97.9%）。

- 补齐 `AdaptiveSidebar` 缺失的构造入口：此前该类型有 `View` 实现但**没有 `new`**，因此任何调用方都无法构造；现提供 `new(title, selection, sections)` 与 `content`/`presentation`/`width`/`expanded`/`options` 构建器，并在构造时校验 section ID 唯一（与 `Sidebar` 一致）。

- **Examples（Gallery）开始收口**：新增 `media_stories`（`SkeletonStory` 占位切换、`RatingSample` 评分条）与 `overlay_stories`（`PopconfirmStory` 确认记录、`HoverCardStory` 跟随切换），共 7 项测试（已运行通过），并把 5 个页面接入 `StoryDispatch`（skeleton、rating、popconfirm、hoverCard）。生产源码达 958/984（97.4%），仅余 26 个 Gallery 故事页。

- **Compose 生产源码全部迁移完成（414/414）**，全工作区生产源码达 953/984（96.8%）。本轮补齐最后一批控件模型（共 59 项测试，均已运行通过）：`AssetRef` 类别匹配与选择器过滤（9 项，关键字分组按优先级取首个命中）、`DataTableRowHost`/`CellHostUpdate` 与编辑器失焦判定（11 项，选中优先于斑马纹、更新未变即跳过重绘、冻结泳道按 group 播报）、`CommandSearch` 排序与投影（22 项：标题前缀 > 词首 > 包含 > 关键字 > 子序列 > 上下文，禁用项靠后、平局保持源序、分组仅在多组时出现、箭头环绕与翻页夹取、`i64::MIN` 模运算不溢出、最近项去重与前插封顶）。

- 修复 `normalize` 的变音折叠缺失：此前仅剥离 NFD 组合符号，预组合的 Latin-1（如 `é` = U+00E9）不会折叠为 `cafe`，导致带重音的命令标题无法用小写无重音查询命中。补上 Latin-1 补集显式映射后，带重音标题与无重音查询可正确匹配。

- 新增 Compose 字段模型（共 48 项测试，均已运行通过）：`Vec3Codec`（13 项：`1, 2, 3`／`(1 2 3)`／`Vector3(1, 2, 3)` 均可解析，非有限值拒绝，写入按轴夹取且只回写变化轴）、`JsonFieldValidation`（11 项：空白提交为 `{}` 且判为可接受，格式化为排序键）、`parse_hex`/`hex_string`（9 项：含 `#` 与大小写均可，8 位末对为 alpha，往返受 8 位量化限制）、`ComboboxSearchSession`（15 项：短于最小长度不发请求、同查询不重复请求、**仅最新代次可发布**、取消回 Idle 清查询但保留已载结果、重复值去重保留首个）。

- 新增 `DataTableEditingState`（11 项测试，已运行通过）：草稿生命周期与 Tab 遍历。被拒绝的提交**保持草稿打开并记录错误**（不丢弃用户输入），因此切换到另一单元格时会先中止而非吞掉非法值；行/列消失时草稿直接丢弃；Tab 跳过不可编辑列，走到末尾即结束。行身份用 id 而非索引，因为显示顺序可能在编辑期间改变。

- 新增 `TimeInputState`（14 项测试，已运行通过）：12/24 小时制显示、分段步进（12 小时制内步进不跨半天，与 Swift 一致）、数字缓冲（高位数字立即完成并进段、低位等待第二位）、AM/PM 首字母仅在异于当前半天时翻转、失焦清空该段缓冲。**移植中两处测试预期先写错后按 Swift 语义修正**：`set` 对时/分各自独立取模因而分钟越过 59 只回绕不向小时进位；分钟缓冲在单数字完成后仍保留，故紧随的下一数字构成超范围值并被忽略。行为与 Swift 一致，是预期错误而非实现错误。

- 新增 `DataTableRowMetrics` 与 `DataTableSortRequest`（9 项测试，已运行通过）：固定行高走算术、可变行高走前缀和索引，视图窗口/滚动范围/定位共用同一真源；排序在**两个方向**都以源索引兜底相等比较，因此降序不会反转并列项——这是"稳定"与"仅确定性"的区别；过期投影按模型修订号与排序命令双重拒绝。

- 补充 Compose 控件模型（含单元测试，均已实际运行通过）：`ViewThatFits`（阈值/intrinsic 双模式）、`LayerRoot`+`PortalHost`+`LayerRoot` portal 层、`DataTableColumnOrder`（8 项测试）、`ReorderableList` 拖拽模型（13 项测试）、`SearchFieldState`（8 项测试）、`DateRangePreset`/`DateRangeRules`（12 项测试，覆盖闰年、跨年、反向区间归一化、含端点的天数计算与 `can_complete` 双向校验）。`DateRangePreset::range` 的反向区间归一化在首次实现中写错（先重赋值 `lower` 再比较），由测试发现并修正。

- 生产源码迁移达 935/984（95%），**15 个模块全部完成**：Graphics、Core、Tools、DevTools、Hosts、Terminal、Sources、System、Logging、Text、Shell、Runtime、Imaging、PlatformCore、Browser。剩余为 Compose 18 个与 Examples 31 个；后者的 31 个 Gallery 故事页依赖前者尚未落地的控件。

- 补齐本轮 Compose 缺口：`ViewThatFits`（阈值与 intrinsic 两种选择模式，非活动分支保持挂载但不占位/不绘制/不接收输入，并在隐藏时释放焦点与指针捕获）、`LayerRoot`（portal 层 z-index 30000，高于标题栏上限 20000）、`PortalHost`（按首帧解析并保留窗口 store 归属，不在每次更新时重新解析，避免跨窗口迁移观察者）、`DataTableColumnOrder`（顺序应用/移动/拖拽目标计算）、`ReorderableList` 模型（拖拽会话、放置边界与去移除后的目标索引、键盘步进夹取、保留挂载行）。

- 修复 `PlatformEventBridge` 克隆语义：此前 `Clone` 会复制订阅表，导致克隆体收不到经原件发布的事件；改为共享 `Arc` 存储，与 Swift 的引用类型语义一致。

- 迁移清单工具修正可达性判定：`mod` 声明必须位于该文件的**同级模块**（`agent/skills.rs` 由 `agent/mod.rs` 的 `mod skills;` 引入），且 crate 根包含 `main.rs`（`Tools/Cove` 是二进制 crate）。此前判定过宽（把 `date_values.rs` 误认为可达）与过窄（把 `Tools` 30 条误降级）并存。另引入 `verified_by: "operator"` 标记人工按符号核对的映射，脚本不再覆盖。

- 修复 `cove_ui_compose` 未导出 `modifiers` 模块导致的可见性缺口：`FrameDimension` 等修饰器类型此前无法从 crate 外部引用（`pub use modifiers::layout::FrameDimension;` 已补，纯增量）。同时纠正一处流传的误判——**`ModifiedContent` 实现了 `View`，所有 `*Ext` trait 均为 blanket 实现，修饰器可自由串联**，无需在每组之间插入 `AnyView::new(...)`；已用编译探针验证并写入迁移文档。"no method named X" 的真实成因是缺 trait import、参数不符/字段私有、或值本身无 `View` 实现，三者已分列记录。

- 补齐 Gallery 侧视图缺失的控件：`Toolbar` 溢出布局接通（`LayoutContainer` + `impl Layout` + `AppBar` 供给可用宽度，并有断言优先级顺序与 `Always` 保证的测试）、`AppBar`、`ButtonGroup`、`Table`/`TableColumn`、`Calendar`/`DatePicker`、`Rating`、`Breadcrumb`、`Pagination`。`PaginationModel::items` 的 `start == 3` / `end == total-2` 邻页特例已按原实现移植并单测。

- 修复 `foundation/date.rs::weekday_of` 的错位一天缺陷：原式 `(epoch_day.rem_euclid(7) + 5).rem_euclid(7) + 1` 对纪元第 0 天返回 6，而正确值为 5（1970-01-01 为周四，Sunday=1 记法）。所有星期被报成下一天。改为 `((epoch_day + 4).rem_euclid(7) + 1)`。影响面：`Calendar::date_interval_start` 的 `leading = (weekday - first_weekday).rem_euclid(7)` 因此对每个日期都算错，导致 `CalendarView` 的周粒度选区错位、`CalendarUnit::Weekday` 取值偏移一天、`iso_week_of` 在年界返回错误周数。已用 Python `datetime.isocalendar()` 交叉验证修复后结果：2024-12-30 → 2025-W01、2021-01-01 → 2020-W53、2025/2026-01-01 → W01 全部一致。

- 新增日期/日历基础层：`foundation/date.rs`（`Date` 以纪元秒表示，`DateParts` 墙钟时间，Howard Hinnant 民历换算双向，`iso_week_of` 返回 ISO 周年+周数，圣诞周归属按"含该周周四的年份"处理）与 `foundation/calendar.rs`（固定 UTC 偏移 + `first_weekday`，`component`/`date`/`start_of_day`/`date_by_adding`/`date_interval_start`，月相加按目标月末夹取，周起始用 `rem_euclid`）。工作区无 `time`/`chrono`/`jiff`，故自行实现而非引入依赖。**具名差异：固定偏移、无 DST 规则**——对日历网格是正确取舍（日期是标签不是时长），但不与 Foundation 逐字节等价。

- 补齐此前被误判完成的控件：`DateValues`（`DateAvailability`/`CalendarGrid`，其 Rust 文件从未被 `mod` 声明且所引用的 `foundation` 日期模块不存在）、`Calendar`（period/navigation 模型 + `CalendarView`）、`DatePicker`、`Table`/`TableColumn`、`Toolbar`、`ButtonGroup`、`Rating` 等一批"模型与样式齐备但无视图"的组件。`CalendarView`/`DatePicker` 的**标签为英文硬编码**，未实现 `DateFormatter` 的本地化符号查找——英文下与 Swift 一致，其他语言不正确，已登记为已知缺口。

- `Checkbox::labeled_from_boolean(label, Binding<bool>)` 新增：`from_boolean` 内部会转成三态、没有 label 槽位，而三态路径有 `labeled`，造成布尔路径无法带标签。`Toggle` 本就有 `labeled(_:is_on:)`，此项补齐唯一的不对称。另修复 `masonry` 模块缺少 `pub use`（`primitives::layout::masonry::Masonry` 与 `primitives::layout::Masonry` 现均可用），与其余布局模块保持一致。

- 迁移清单工具增加三项可信度校验并修正两起错配：`.rs` 文件的 stem 若未被任何 `mod` 声明（死代码）不计入已写入；头部注释的基名匹配要求非标识符边界（避免 `Visibility.swift` 命中 `AnimatedVisibility.swift`）；普通英文词（`Visibility`/`Support`/`Options` 等）不作跨子系统匹配依据。据此确认 `Primitives/Data/Columns/Visibility.swift`（表格列可见性规划，25 行）未迁移——此前被错映射到 `Graphics/rust/rhi/visibility.rs`（GPU 可见性缓冲）；`Primitives/Controls/DateValues.swift` 亦为孤儿文件且引用了不存在的 `foundation` 日期模块。审计后当前 0 条生产记录指向不存在或不可达的目标。

- `TextBuffer` 恢复 `Send + Sync`（对齐 Swift 的 `Sendable`）：rope 由 `Rc` 改为 `Arc`，共享空叶子从 `thread_local!` 提升为 `OnceLock` 静态量，`TextBuffer::eq` 改用 `Arc::ptr_eq`。此前 `TextField::new(placeholder, Binding<TextBuffer>)` 的签名不可满足——`Binding` 要求闭包 `Send + Sync`，而 `TextBuffer` 基于 `Rc`——**全仓库零调用点**证明该控件从未可构造。修复后 `TextField` 可用，未使用任何 `unsafe impl`，`Send`/`Sync` 由数据结构本身满足（已用编译期断言验证后删除该断言）。`SharedRope` 保持 `Rc<RefCell<_>>`：实时编辑会话本就是 UI 线程状态，只有缓冲区**值**需要可共享。

- 修复 LSP snippet 展开的多处结构缺陷（`Compose/rust/primitives/text/text_field/snippet.rs`）：`${N|a,b|}` 现扫描整个候选列表并在首个逗号处记住首选，随后跳过 `|}`，转义按对消费；`$N` 在检查 `{` **之前**读取，修正纯 `$1`/`$2` 制表位的顺序（此即最常见路径）；`${VARIABLE}` 无默认值时不再可能输出多余 `$`；`$` 后非数字/`{`/标识符时按字面 `$` 输出。此前版本在单候选与多候选下都会错位跳过字符。

- 修复跨越多个团队的同一类缺陷（`.modifier()` 陷阱）于源头：`ModifiedContent` 的私有字段 `modifier` 遮蔽了 `ViewExt::modifier(self, m)`，报错形如 "private field, not a method"，本会话内至少 12 次误修。字段改名为 `applied`；新增 `AnyView::with_modifier(self, m)` 与 `IdentifiedView::with_modifier(self, m)`，使已擦除的值可直接链式附加修饰器。约定：具体 `View` 用 `ViewExt::modifier`，已擦除值用 `with_modifier`，禁止用 `AnyView::new(modified_content)` 掩盖类型错误（会经 downcast 快路径静默丢弃修饰器）。

- 删除 `Compose/tests/` 目录下 4 个陈旧测试文件（`theme.rs`、`theme_environment.rs`、`composition_local.rs`、`semantic_motion.rs`）。它们由 Cargo 默认自动发现路径编译，但写于早期接口（`Rect.origin` 作为字段、`ViewGraph(tree:recomposer:)` 元组构造），从未通过编译，且使 `--all-targets` 每次必然失败，污染全部验证。

- 新增 `pulldown-cmark` 工作区依赖用于 Markdown 解析。Swift 侧使用 swift-markdown（cmark-gfm），手写 CommonMark 子集会在嵌套列表、惰性续行、引用式链接、HTML 块与实体处理上与 cmark-gfm 分歧。`pulldown-cmark` 关闭默认特性后仅引入 `bitflags`/`memchr`/`pulldown-cmark-escape`/`unicase` 四个纯 Rust 小依赖，无构建脚本，全平台可用（含 Wasm）。

- 开始迁移 Swift 测试为 Rust 行为测试（424 个测试源文件）。`Tools/Cove/rust/migration_inventory.py` 现在按 `[[test]]` 目标名匹配测试条目，不再把它们误记为未开始。已迁移：System 6 个、Logging 流水线、Core 场景动画 6 个、`Node` 变换/资源/不透明度/无障碍几何、`Animation` 值类型、`PlatformCore` 滚动输入、`Text` 彩色字形图集，以及 Compose 侧的 `TestSupport` 等价物（`GlobalTestLock` 串行化进程级 holder、`TestScene` 私有 holder、`test_text_environment`、按 `LayoutDebugAttachmentKey::DEBUG_NAME` 读回调试名）。

- `--all-targets` 全工作区曾达到 18 个 crate 全绿（含测试目标）。此后 Compose 的 68 个未开始文件仍在持续写入，属预期状态。

- 迁移盘点改为脚本生成，并与磁盘对齐：新增 `Tools/Cove/rust/migration_inventory.py`，遍历 Rust 树按同名模块与主类型定义重新推导每个 Swift 文件的目标路径与状态，同时重写 `Docs/rust-migration-files.json` 与 `Docs/rust-migration-files.md`。生产源码实际为 651 → 802 / 984 已写入，上一版"460 已写入 / 477 未开始"是人工维护造成的陈旧值。

- 修复"写而未编译"缺陷（与编译错误同级）：System 16 个、Logging 15 个、Tools/Cove 8 个、Tools/Support 12 个、Compose 2 个 Rust 文件此前从未被 `mod` 声明；`Core/rust/tests/` 6 个与 `Graphics/rust/tests/` 7 个测试文件从未声明为 Cargo 测试目标，`cargo` 完全看不到它们。System 与 Logging 现按 Swift 包结构重挂模块，并删除重复的单体实现（Logging 511 行 `lib.rs`、System 合并版 process），关键对外 API 保持可用（`LogRuntime::log` 五参形式、`LogQuery.text: Option<String>`、`MetadataProvider` 改为 newtype、`Access::Inherit` 单一类型）。

- 新增 `Logging/rust/build.rs` 编译 `Sources/CCoveLog/Router.c`，`LogSystem::bootstrap` 不再因 `cove_log_set_handler` 未定义而在链接期失败。

- Graphics 三个原生后端全部完成：Metal（macOS/iOS）、Vulkan（Windows/Linux/Android，`vulkan/sys.rs` 为手写 FFI，运行期 `dlopen`/`LoadLibraryA` 加载，无新依赖）、DX12（Windows，COM vtable 直调，无 `windows` 依赖）。`backend_factory::install_backends` 按 `cfg` 注册。DX12 加速结构未链接 DXR 分发，能力探测如实不宣称光追；`command_list` 的描述符表逐 draw 绑定仍待 Windows 真机验证。

- Terminal 完成：POSIX `forkpty`（macOS/Linux）、Windows ConPTY、Windows 命令行引号；链接 vendored Ghostty 静态库，FFI 布局由真实 `ghostty_type_json()` 清单生成并加编译期尺寸断言。会话线程改用 `Arc` 邮箱 + `LocalInvalidationHandle` 桥接替代 Swift 的 `NSLock` + `DispatchQueue.main`。

- Hosts 完成：Android 15 + Desktop 26 个文件，49 个 Rust 文件；`install_platform_services()` 在桌面 `Application::run` 与 Android `cove_android_create_application` 两处启动路径调用，使远端图片加载真正生效。逐平台矩阵与 7 项已知缺口记录在 `Hosts/rust/PLATFORM_COVERAGE.md`。

- Sources（WebView/Workspace）与 DevTools 完成：WKWebView（objc2 + `define_class!` 导航代理）、WebView2（手写 COM vtable）、WebKitGTK（dlopen GTK3/GTK4）三个真实后端；DevTools 客户端/服务端/Agent/Inspector 全部落地，`std::net` 实现 RFC 6455 传输，`pump()` 每帧驱动。

- Browser 宿主落地：`Browser/rust/prototype/` 按 Swift `main.swift` 的 `@_cdecl` ABI 复刻全部导出（`cove_render`/`cove_font_*`/`cove_pointer`/`cove_key`/`cove_text`/`cove_units`/`cove_snapshot`/`cove_dispatch` 等），帧载荷与 DevTools 会话流程一致；单线程 reactor 用 `thread_local!` 承载 Swift 的 `nonisolated(unsafe)` 全局。

- Tools CLI 与 Support 完成：`main.rs` 此前完全没有 `mod` 声明，导致 8 个模块全部是死代码，且 5 个被引用的模块不存在。现已接入 23 个命令分发、`--version`/`--help`、SDK 重执行引导，并以真实二进制核对退出码与 stderr 文本；Android/Desktop/iOS 平台构建命令为真实实现。

- 修复一批共享 API 缺口：`Rect::size()`/`Rect::origin()`、`KeyModifiers::is_empty()`、`ScrollConsumePolicy: Default`（`whenScrollable`，与 `ScrollView` 默认一致）、`ObservableStateRegistrar: Clone`、`scene::scrolling` 重导出、Compose 根导出 `State`/`LayoutNode`/`ScrollView` 系列。`ModifiedContent` 的固有方法 `modifier()` 曾遮蔽 `ViewExt::modifier(_:)`，改名为 `modifier_ref()`。

- 已知缺口：Linux `https://` 图片加载（无 TLS crate，如实上报 `https=false`）；macOS 菜单栏（`cove-shell` 只有上下文菜单入口）；Wasm 图片 transport（需专用 fetch/XHR）；Apple 图片 MIME 与 macOS 内存压力读取采用回退实现。

- 全量迁移尚未完成：Compose 仍有 82 个文件未开始，Examples 46 个、Tests 424 个待迁移。所有 Rust 代码仍未做行为对照、跨平台验收与 GUI 冒烟；按要求统一验证放在全部转换之后。

- 翻译 Metal 后端第一批：`MetalSubmissionStatus`、`MetalLibraryCache`、`MetalRenderBindings`、`MetalUploadRing` 落地到 `Graphics/rust/metal/`，并在 `Graphics/rust/lib.rs` 中以 Apple 目标 cfg 声明模块。设备、注册表、加速结构、管线、传输与提交编码器尚未翻译，未编译、未测试。

- 修复 Rust 工作区可编译性：为 `Compose/CompositionMacros` 补上 proc-macro 的 `path`，修正 `cove-composition` 的导出与类型错误（`ModifierReplay` 导出、场景几何重导出、`DynamicField: Clone`、slot tag 包装、`&dyn View`/`&AnyView` 混用、`ViewScope::uninstall` 签名），并为 `Graphics/rust/ui/draw_geometry.rs` 的语句位 `unsafe` 块加括号。仅为局部 `cargo check`，不构成验收。

- 迁移盘点改为逐文件映射：新增 `Docs/rust-migration-files.md`（1,438 行）并升级 `Docs/rust-migration-files.json` 为 schema 2。补回 System、Logging、Tools/Support、Graphics RHI 前端、Hosts Android 契约/集成、DevTools Protocol 的既有 Rust 映射，纠正 Android `APK/Package.swift` 的清单误分类，并确认 ZStack 与 ChartOverlay 已实现。盘点未编译、未测试。

- 继续逐文件迁移 Compose：加入触发式/重复关键帧轨道、离散阶段序列、数字文本插值、可动画缩放/偏移变换和进度值动画资源，保留阶段动画、最终无障碍值、减弱动态效果和节点卸载清理。本批尚未编译或测试。

- 继续迁移 Compose 基础控件：加入 Portal 宽高约束与放置算法、模态焦点作用域、活动路由出口、虚拟列表窗口状态、数据表列几何、集合控件键盘边界和编辑器高度约束。本批尚未编译或测试。

- 继续迁移 Compose 布局：加入 `LayoutThatFits` 候选选择和响应式动作组的水平/堆叠排列，复用同一组子视图与状态。本批尚未编译或测试。

- 继续迁移 Compose 菜单基础：加入不依赖具体渲染器的菜单/子菜单/动作描述符和快捷键关联。本批尚未编译或测试。

- 继续迁移 Compose 动画：实现 `Transition` 插入/移除效果、带退出保留和快速反转的 `AnimatedVisibility`，并接入 `TransitionView` 与 Portal；支持主题动画、减少动态效果、分组延迟和布局折叠。本批未编译或测试。

- 继续迁移 Compose：新增通用的动画值修饰器、线程安全布局帧捕获、主题驱动浮层表面修饰器和资源包定位；ViewGraph 布局回写会逐次触发节点布局回调。改写列可见性优先级规划，加入列选择菜单模型；接入表单、字段标签、表单验证、键帽控件和键盘快捷键值，并从 Compose API 导出动画上下文函数。列选择器和 Select 的下拉/菜单呈现仍待迁移。本批代码尚未编译或测试。

- 继续迁移 Compose：补齐颜色/渐变/透明度/阴影/边框/字体/字距与 Padding/Flex/Frame/绝对定位修饰器，以及无障碍、光标、焦点请求、命中测试和悬停修饰器；新增 StatusTone；接入按钮交互/无障碍主机、节点级样式刷新、图标缩放和生命周期加载指示器；将 Link 和 Tag 接入 Rust 控件模块。Button/Link 提示呈现、Link 交互动画和 Text 仍有缺项，尚未编译或测试。

- 继续迁移 Compose：增加统一动画时钟的 Rust `PetWorld`、窗口坐标精灵绘制、布局锚点注册与 `PetScene` 组合；接入媒体、头像、图表、文本辅助、布局和主题模块，并写入纯文字和分隔线的基础绘制/测量。文字选择与截断提示仍未完成，移植尚未验证，待全量迁移后统一测试。

- 继续迁移 Compose：补齐 Rust `ViewGraph` 的局部状态字段读取跟踪，并将 `@TrackStateReads` 编译器宏改为 Rust 属性宏；迁移与宏展开均留待全量完成后的统一验证。

- 继续迁移 Graphics UI 渲染层与 Runtime：写入纹理驻留/图集、背景模糊/揭幕、分条 MSAA、多窗口事件与帧调度、iOS 显示循环及 Apple 无障碍桥接；这些实现仍未编译或测试，原生 GPU 后端与上层控件迁移继续进行。

- 继续迁移 Shell：写入 Rust SDL3 窗口/输入/剪贴板/对话框，C 事件桥接，以及 macOS/iOS 原生窗口、精确滚动和显示刷新桥接；尚未统一验证。

- 继续逐文件迁移 Text/Imaging：加入 Rust 字体发现、FreeType/HarfBuzz 整形与图集、CoreText 彩色字形、图片/动画解码、EXIF 方向和资源缓存；新增带源码指纹的迁移清单。按要求留待全量转换后统一测试。

- 开始 Swift → Rust 2024 迁移，新增 `0.3.0-dev` Cargo 工作区；`cove` / `cove.cmd` 改用 Rust CLI，不再调用 Swift 编译器。
- 加入 Core 几何/状态/绘制缓冲区/弹簧/位图/字号缩放、PlatformCore 输入与资源契约、System 文件与进程服务、Logging 管线和 DevTools 协议的 Rust 实现；绘制热路径继续使用 C，mipmap 面积过滤增加共享 C 实现。
- Rust DevTools 协议改为 0.2，输入录制格式为 3；CLI 增加 Cargo 构建/运行/检查/格式化/测试/清理与迁移盘点。新增无窗口 RustCounter 示例及桌面/移动共享基础层 CI 配置。
- 全量迁移尚未完成：Swift UI、原生 GPU/窗口/移动宿主、终端、WebView、Workspace、Gallery 与 SDK 发布等实现保留并列入 `Docs/rust-migration.md`；尚未统一测试，不宣称功能或平台验收完成。

## 0.2.13 — 2026-10-29

- 增加 `RouteOutlet` 活动路由组合边界；Gallery 将页面状态读取移入内容区，Sidebar 按输入代次和行状态复用，切页只更新受影响的目标行。
- `VirtualStack` 的固定、可变和估算行高统一支持 `preload` 视口比例，默认额外准备约 30% 内容；缓冲窗口内滚动不触发组合，补充窗口只创建新进入的行，保留原有 `overscan` 最小行数与跳转/缩放/数据更新行为。
- 增加显式 `MemoizedView` 输入复用边界，局部状态和环境变化仍可刷新；不改变普通视图对不透明回调的保守更新规则。

- 修复文本拖选导致整窗卡顿：按场景几何版本缓存段落顺序与坐标，使用跨平台 C 内核排序和命中扫描，绘制直接查找选区；拖选只刷新选区发生变化的段落，复制不再逐段重复排序。
- 渲染树改用缓存的扁平绘制计划与子树范围，缓存命中、裁剪可直接跳过整段，纯结构节点只传递变换；补齐动态 zIndex 与结构变化的失效处理。
- 顶点平移、索引重定位、透明度与绘制边界扫描采用 C 内核，大缓冲区按不重叠区间并行处理；滚动复用不再重复裁剪已验证安全的圆角几何。共享实现覆盖 macOS、iOS、Linux、Windows 与 Android。
- 节点脏标记、资源卸载、渲染镜像构建/拆除及帧末清理改为迭代，避免深层树递归栈溢出。

## 0.2.12 — 2026-10-29

- 修复 Linux/Windows 原生菜单测试：PNG 图标按 CoreGraphics/ImageIO 编码能力验证，未提供编码器的平台仍验证图标为空；统一实现与测试的平台条件。

## 0.2.11 — 2026-10-29

- 修复 0.2.10 发布验证：Compose 按所属包查找资源，线程局部存储析构回调处理空指针并消除 Windows 的 UUID 名称冲突，Android 的 Float 数学运算使用明确重载，iOS 不再引用 AppKit 菜单适配器。
- 修复后续 Linux CI：文本选择几何测试仅依赖 Foundation，移除已删除工具包的测试与缓存入口。
- 修复 Windows 的 HarfBuzz 枚举桥接，保留公开 `ScriptTag` 的 UInt32 语义，兼容原生枚举底层的有符号整数。

## 0.2.10 — 2026-10-29

- 按能力拆分 Core、Text、Imaging、Graphics、Shell、Compose、Runtime、DevTools 与 Hosts，移除 Portable 与根 manifest 对下层 target 的重复声明；根包收拢为应用与扩展产品，内部模块产品改为从所属能力入口消费。
- Compose 去除 Runtime/窗口/GPU 依赖；字体、文本环境和缓存统一归 Text，图片解码与资源所有权归 Imaging，GPU 渲染和上传归 Graphics。保留 `import CoveUI` 与 `TextureImage(texture:)` 应用入口；CLI、Browser 使用窄协议/组合/检查依赖。修复字号切换后 HarfBuzz 缓存未同步和映射字体逐字节复制的问题。新增架构边界检查；Core 的 Yoga 布局依赖仍明确保留，尚未达到全量纯 Swift。

- 文本选择补齐跨组件一致性与触摸细节：`RichText`/`Markdown` 与 `Text` 共用同一套端点手柄、放大镜与命中测试（`TextSelectionChrome`），手指或笔可拖动手柄精修选区；`RichTextFlow.caretRect(at:)` 提供手柄定位。
- 选择区内的 `Text` 不再进入 Tab 焦点序列，点击按钮、列表行或表格单元格中的标签时不再抢走控件焦点与点击；`Text`/`RichText` 在控件内部自动退出选择。
- 键盘可扩展选区：`Shift+←/→` 按字素步进，`Shift+Home/End` 扩到段首段尾，可跨段落延伸。
- 触摸长按选词后焦点落到被选中的标签或段落，复制快捷键可直接作用于该选区。
- 选择菜单的 Look Up / Share 仅在宿主提供动作时出现，新增 `textSelectionActions(lookup:share:)`；`TextSelectionScope` 的 `hasSelection`、`selectedText`、`copy()`、`selectAll()`、`clear()` 与 `TextSelectionRoot` 初始化器公开，供宿主接入。
- 文本选择改为默认开启：`Runtime` 与辅助窗口安装根视图时统一包一层 `textSelection()`，普通 `Text` 无需在页面里逐个声明即可拖选与复制；子树上用 `.textSelection(false)` 重新关掉。
- 选择区只接管落在自身文字上的按压（`TextSelectionScope.textUnder`，`_ContextMenuHost.isInterested`），整页或整窗的选择区不再遮挡控件自己的右键菜单与触摸长按。
- 触摸屏的选择菜单以底部表单呈现（`TouchPresentation.usesSheet`），复制、全选在手指长按后即可用，不再依赖硬件键盘。
- 选择区对布局完全透明：只挂一个不占布局节点的 `SurfaceReader`，菜单宿主仍是布局根；底部表单作为宿主的兄弟节点而非包裹层。此前用 `Box` 包裹会让 `SurfaceRoot` 的 `flex(1, basis: 0)` 失去确定高度，整个窗口高度塌陷为 0（Gallery 的 `gallery-shell` 由 820 变 0，所有分区叠在一起）。
- 修复 `Animation.initialVelocity` 的访问级别：`CoveUICompose` 的 `staggered` 需要读取它，否则干净构建失败。
- `HorizontalSwipe` 的分页手势改用 `.scroll` 角色（原为默认 `.control`）。它包住整页时，页内每个标签都会被判为「控件内的文字」而退出选择，导致整屏文本一个都选不中；分页与滚动一样属于手势容器，不该占用控件角色。
- 选择能力按整个 Gallery 目录验收，而非抽样单页：`GalleryContentTests` 逐页挂载真实应用根，断言每个叶子标签要么可按下、要么被控件合法拥有（`Badge`/`Tag`/`KeyCap` 这类合并语义的容器不计入），并逐页跑真实的「按下→拖动→松开→Cmd+C」序列校验剪贴板。任一页出现「整页不可选」都会直接指名吞掉手势的祖先节点。

## 0.2.9 — 2026-10-27

## 0.2.8 — 2026-10-27

- 增加 `Device.prepareResources` 和四类资源的异步创建 API，提供阶段/步骤/编译进度、Task 取消和失败清理；Metal、Vulkan、DX12 在昂贵编译期间释放设备资源锁，已预热的 UI 可继续渲染。
- 着色器按内容共享并计数引用，取消或失败只释放本次持有的资源；并发创建同一管线时回收重复对象，保护编译期间的着色器输入生命周期。
- 内置 UI 打包 macOS、iOS 与模拟器的预编译 Metal 库，按源指纹失效回退；新增跨平台着色器产物生成流程。GPU 管线仍需在目标设备上创建，本版本未提供跨进程驱动缓存。
- 包含未成功发布的 0.2.7 的全部内存与效果修复：跨后端 16 MiB MSAA 工作区、磨砂前缀重绘、同通道揭幕、Vulkan 临时附件与空块释放、DX12 分配报告和离线 UI DXIL。
- 修复 DX12 多采样颜色纹理的无效资源标记，以及发布说明中的旧 CI 跳过标记阻断后续版本工作流的问题；支持手动恢复不可变标签的发布。

## 0.2.7 — 2026-10-27

- 统一窗口渲染的 MSAA 生命周期：磨砂重绘已解析的前缀，揭幕在最终通道合成，跨通道只保留单采样图像；取消效果帧保留整屏多采样附件的路径。
- 为需要显存的 Metal、Vulkan 和 DX12 后端提供 16 MiB MSAA 工作区预算，通过保持原始视口与样本位置的分条渲染限制大窗口分配；支持真正 memoryless 的硬件继续使用片上附件。
- Vulkan 临时附件使用 TRANSIENT_ATTACHMENT 并优先选择 lazy 内存；空内存块在资源退役后释放。DX12 增加纹理分配报告。
- 修复安装版 Windows 测试诊断脚本查找 CLI 的路径。
- 补齐 UI 与磨砂/揭幕的 DXIL 离线产物及固定 Slang 工具链生成入口；修正着色器编译与缓存生命周期文档。
- 磨砂捕获按实际尺寸复用纹理，修复较小的后续面板沿用较大纹理时的采样范围错误。

## 0.2.6 — 2026-10-27

- 保持默认 4x MSAA，普通窗口在支持的 Apple GPU 上改用临时附件；磨砂与揭幕自动保留持久样本。固定场景的输出逐字节一致，M1 的 4K 附件分配由 129.5 MiB 降至 0 MiB 持久 backing，2x 场景 GPU 耗时下降约 32%；DevTools 分开显示逻辑容量和后端 MSAA 分配，其他后端保留安全回退。

- iOS 构建由 destination 选择应用 SDK，避免全局 `-sdk` 把宿主宏插件误编译为模拟器程序；补齐字段追踪对延迟绘制、布局回调和未读取状态失效信号的保守处理。

## 0.2.5 — 2026-10-27

- 新增 `@TrackStateReads` 视图宏，自动推导普通值类型 `@State` 的字段读取依赖；按实际读取字段刷新，重建时替换依赖，整值读取、引用与不透明值保守失效，并在 DevTools 中记录字段名称。
- Metal 接入可选的命令缓冲 GPU 时间戳，完成 1x 解析覆盖率与 4x MSAA 的固定场景评估：M1 上 1x 降低 UI GPU 耗时，但 4x 显著改善自定义三角形边缘，因此桌面默认继续保留 4x；报告包含像素误差、目标内存与截图，测量入口只在显式启用时执行。

## 0.2.4 — 2026-10-27

- 修复 Windows 安装器读取 GitHub 校验清单的方式：下载后明确按 UTF-8 文本逐行解析，兼容二进制响应类型，并避免覆盖 PowerShell 自动变量。
- 包含 Cove 0.2 系列的完整改名、子树复用、抗锯齿、mip／LOD、跨平台内存压力和 DevTools 修复。

## 0.2.3 — 2026-10-27

- 完整发布 Cove 0.2 系列 SDK：包含改名、子树复用、覆盖率抗锯齿、mip／LOD、内存压力与 DevTools 接线修复。
- 修复解析抗锯齿在 Android 上的浮点数学重载与类型推导，采用 Swift 原生开方和取整，并在整数转换前限制细分数量。

## 0.2.2 — 2026-10-27

- 完整发布 Cove 改名后的 SDK，包含 0.2 系列的子树复用、抗锯齿、mip／LOD、内存压力和 DevTools 修复。
- Android 发布明确选择已安装的 NDK r30，避免 runner 继承的旧 NDK 环境变量使原生库打包失败。
- 发行脚本等待真实的 GitHub workflow ID，不再把空列表显示为 `null`。

## 0.2.1 — 2026-10-27

- 首次发布完整 Cove SDK：同步模块、CLI、清单、安装目录与发布包名称，包含值状态子树复用、覆盖率抗锯齿、mip／LOD 和跨平台内存压力修复。
- 修复 Linux／Android 原生构建中 SVG、WebP 静态依赖的链接顺序，CLI 自动配置所需选项。
- 浏览器 DevTools 与内置 Agent 共用内存数据源，Android 提供文本／图片缓存统计；断开连接后清除旧快照。
- 深层 UI 交互回归在应用主 Actor 上执行，避免 macOS CI 后台工作线程栈溢出；改名后的 CLI 使用物理路径，防止旧路径链接产生重复编译缓存。

## 0.2.0 — 2026-10-27

- 项目迁移为 Cove：框架模块采用 `Cove*`，CLI 为 `cove`，清单与本地目录采用 `cove.json`／`.cove`；安装器、发布包、Android 标识与仓库链接同步改名。此版本需要更新旧项目的导入与配置。

- `@State` 父视图更新时自动跳过值输入相等的子 scope，保留原状态和回调；闭包、可变引用及不透明输入保守重组，环境变化仍强制刷新。
- 补齐描边、路径、圆环与圆角遮罩的覆盖率抗锯齿；`geometryAntialiasing = .analytic` 可按窗口使用 1x 目标，默认仍保留 4x MSAA。离屏 HUD 与 Android 使用解析遮罩。
- 图像生成带 alpha 权重的 mip 金字塔，计入图片缓存预算，并按实际显示像素选择 LOD；GPU 图像可携带由生产者生成的独立分辨率层。
- 增加 Windows 低内存通知、Linux 主机／cgroup 压力监听和 Android `onTrimMemory` 清理；DevTools 提供 `query_ui_memory` 与性能页内存估算。
- 显式本地 SDK 覆盖可跨版本调试；SDK 包补齐 Inspector 客户端及资源文档。Android 发布不再请求已下架的 `tools` 包。

## 0.1.14 — 2026-10-26

- 修复发布引导：源码仓库的旧原生库清单不再阻止新版本从源码构建；已安装 SDK 缺少库时明确报错，避免回退到不存在的依赖源码。

- 窗口隐藏、最小化或被遮挡时停止呈现，等待在途工作完成后释放该窗口的 swapchain、MSAA 与 backdrop 目标；恢复时按最新尺寸和 VSync 设置重建，辅助窗口互不影响。
- 重组性能统计不再触发重画；接入 Swift Observation，按实际读取属性的视图失效，父视图不会因子视图的读取而订阅同一属性。
- 文本 shaping/layout 缓存增加 LRU 淘汰、条目数与估算字节双限；提供应用内存配置、窗口资源快照与主动清理，接入 macOS 内存压力及 iOS 内存警告。
- 主窗口与辅助窗口可分别覆盖采样数和清屏色；未覆盖时继承应用配置，默认画质保持不变。
- 多窗口分别持有 reveal/history 与 blur 临时状态，共享字体、图片驻留和已预热管线；隐藏窗口同时释放特效缓冲，避免窗口之间的呈现状态相互覆盖。
- 离屏视图在内容、尺寸和像素密度不变时复用几何快照；图集补丁只发送一次，主动清理后可重新生成。

## 0.1.13 — 2026-10-26

- SDK CLI 增加 `guava task` 和 `guava.workspace.json`：项目提供 Swift 任务库，SDK 负责依赖排序、按需构建、参数传递及构建前钩子，不需要项目包装脚本或独立 CLI。
- 工作区可声明 `dependencyUpdates: "branches"`，本地 build/run 自动检查并刷新分支依赖；CI 保持锁文件，离线检查保留已解析版本。

## 0.1.12 — 2026-10-26

- 安装包包含完整 SDK 与主机原生库，按版本安装到全局 `versions/VERSION`；项目、示例和 IDE 引用同一份源码与库，不再回退到各项目单独检出 GuavaUI。
- CLI 自动调用项目对应 SDK 的工具；安装 SDK 不再改写项目版本，`sdk use` 负责选择版本。
- Guava 构建、测试与清理使用 SwiftPM 标准 `.build`，移除 `.guava/build/swift` 这套独立编译缓存。
- 将原生构建与 SDK 发布收拢为一个工作流、一个 Release；原生库按目标拆包并由 SDK 自动准备，覆盖桌面、Android ARM64、iOS 真机和 ARM64 模拟器。
- Android 构建自动安装匹配且校验过的 Swift Android SDK；平台工具由 setup/doctor 检查。
- 并发项目通过系统文件锁复用同一份 SDK 安装与原生库准备；修复旧安装记录路径末尾斜杠导致升级被误判为其他安装的问题。

## 0.1.11 — 2026-10-26

- `guava.json` 的 `toolchain.sdkVersion` 支持写 `latest`：构建时浮动到本地已安装的最高版本 SDK（离线可用，不触发网络），用 `guava sdk install latest` 拉取新发布并刷新；`guava sdk install latest` 在精确钉版项目里仍会把解析后的具体版本写回清单（npm 语义），无 `GUAVA_SDK_ROOT` 的 SwiftPM 回退路径改用 `from: 0.0.0` 解析最高发布标签，Android Gradle 回退同样支持 latest。
- `guava sdk install latest` 解析 GitHub Latest 指针时去掉仓库地址末尾的 `.git`，此前拼接出 `.git/releases/latest` 会 404。
- `guava self update` 不再调用 `api.github.com`（未认证每小时 60 次配额，超限即失败），改为跟随 `github.com/repos/latest` 的重定向解析最新版本；下载类错误现在打印完整的 HTTP 状态而不是通用的 `ToolError` 提示。
- `guava setup --ios` / `--android` 显式检查 CMake 与 Ninja（移动端 C 依赖从源码交叉编译）；移除 iOS 提示中并不存在的 Rust 工具链要求。
- `Tools/Release/release.sh` 推送后只确认 release workflow 已触发并输出 run 链接，不再轮询等待 CI 结果；移除 `--no-ci`、`--ci-reruns`、`--ci-timeout` 选项。

## 0.1.10 — 2026-10-26

## 0.1.9 — 2026-10-26

- DevTools Inspector 自动打开时先打开会话目录内的 `inspector.html` 跳转页：`open`、`xdg-open` 与 `rundll32` 会丢弃 `file://` URL 的查询参数，客户端此前只能回退到默认 9229 端口并停留在 Disconnected；跳转页在浏览器内完成导航，动态端口与一次性 token 完整保留，打开即自动连接。`guava devtools` 附加到现存会话时同样生成该跳转页。

## 0.1.8 — 2026-10-26

- Windows 子进程以反斜杠路径启动，参数只在需要时加引号：`cmd.exe` 会扫描整条命令行，曾把 `C:/…/cmd.exe` 里的 `/cmd.exe` 当作 `/c` 开关，现在能执行命令并返回真实退出码。
- 文件与管道错误改用 `NSPOSIXErrorDomain`：Android 的 Foundation 没有 `POSIXErrorCode`，`System` 包恢复在 Android 上编译。

### 触摸与移动端

- `NavigationStack` 在触摸表面支持从前缘拖动返回：顶层页面跟随手指移开，下层页面以视差露出；松手时超过宽度三分之一或快速轻扫则返回，否则回弹。从右到左布局从右缘开始。
- `Dialog` 在手机上改为底部弹层：带拖动柄、贴合内容高度、避开 Home 指示条与键盘，去掉关闭按钮和分隔线；`AlertDialog` 仍居中显示。
- `HoverCard` 在无法悬停的触摸表面改为点按标签打开底部弹层，内容不再只能靠悬停到达。
- 标签栏的“更多标签”与列可见性菜单在触摸表面改用底部弹层，与 `Popover`、上下文菜单共用 `AdaptiveOverlayHost`。
- `SectionedList` 在触摸表面点按行即激活（与 `List` 一致），分组标题与行内容对齐并贴近所属行。
- 修复减弱动态效果时 `NavigationStack` 推入页面后无法再返回：没有过渡动画时不再等待过渡结束。
- `Examples/Mobile` 的根视图放入 `LayerRoot`，底部弹层、对话框与菜单在移动端可以正常呈现；项目详情的“分享”改为打开对话框。

## 0.1.7 — 2026-10-25

- 新增可独立构建的 `System` 包，提供 `GuavaFileSystem` 与 `GuavaProcess`，不依赖 SDL、窗口或渲染器；桌面与移动端共享文件、路径、权限语义，不支持的进程能力明确报错。
- 文件写入统一使用同目录暂存与替换，失败时保留旧内容；Windows 仅对临时共享锁进行有限重试，私密文件使用当前用户 ACL。
- 进程服务统一流式读取、输出限额、超时与取消。超时覆盖子进程退出后的管道排空，Windows 使用 Job Object 回收后代，读取句柄由读取线程关闭。
- CLI、应用存储、DevTools 与技能目录使用共享系统能力，移除重复的 Windows 文件写入与进程读取实现。
- 下载文件先复制到目标目录暂存，再校验和发布，避免 CI 临时目录与工作区跨卷时替换失败；下载失败保留原文件。
- `guava test` 支持仅构建、跳过构建、分开运行 XCTest/Swift Testing、测试事件文件与总超时；CI 增加系统契约测试、构建缓存和 Windows 超时进程转储。
- 上下文菜单保留编辑器的弹性高度，触摸滑动列表与长按菜单协作处理指针捕获，允许垂直滚动接管手势。

## 0.1.6 — 2026-10-25

## 0.1.5 — 2026-10-25

### 触摸与移动端

- 菜单在触摸表面改用独立呈现：`MenuPresentationHost` 决定菜单怎样呈现，`TouchMenu` 负责触摸下的导航与项激活，`TouchOverlay` 承担模态呈现；`Popover` 在触摸输入下改走 `TouchOverlay`，不再套用指针式弹出层。
- 新增 `LongPress` 手势与 `MobileBar` 标签栏。标签栏去掉顶部的分隔线，由自身表面标出边缘，背景延伸到 Home 指示条下方。
- 列表行区分指针与触摸：触摸行铺满容器宽度、最小高度 44pt；`ListRowStyle` 随之调整内边距与选中样式。
- 按钮按触摸输入调整尺寸与内边距，触摸表面上的拖动柄按 `minimumControlExtent` 取宽度。

### Android 与表面度量

- `UIEnvironment` 新增 `idiom`、`textScale` 与 `keyboardHeight`，与安全区分开；新增 `TextScale` 读取系统辅助功能中的文字缩放。
- `AndroidApplication` 新增 `updateScale` 与 `surfaceMetrics`，`SurfaceState` 统一持有逻辑尺寸、密度、惯用语、安全区、键盘高度与系统栏图标样式；`AndroidContentSession` 改用 `SurfaceState`。
- Android 的 `windowSoftInputMode` 改为 `adjustResize`，键盘弹出时内容按 `keyboardHeight` 收缩，模板与示例同步更新。

### 控件与表面

- 边框改用半透明墨色（`border`、`borderStrong`、`divider`），不再使用不透明灰；分隔线改为 0.5pt 发丝线，用来确认已看到的边界，而不是画出实线。浅色与深色主题都按此调整。
- `.secondary` 按钮、`StatusButtonStyle` 与开关改用 `onSurface` 的半透明墨色填充，不再叠加表面色与描边；开关滑块在开关两态下都是白色圆片，在浮起表面上不再与卡片同色而消失。
- 新增 `ListGroup`：可带标题与脚注，行位于同一圆角表面上，分隔线从行内容处起始，触摸表面使用更大的圆角。
- 侧栏可折叠：`AdaptiveSidebar` 新增 `isCollapsed` 绑定与 `collapseLabel` / `expandLabel`。宽度足够时仍可折成 72pt 轨道，被窗口强制成轨道时按钮改为展开抽屉，两种情形不再混淆。
- 侧栏拖动柄静止时不绘制，只在悬停或聚焦时出现，边界交给表面差异表达。
- 按钮的 `tooltip` 只重复可见标题时不再弹出，文本仍作为辅助技术的帮助；`AccessibilityTree.visibleText` 提供控件实际显示的文本。
- 示例库的页面改为窗口表面上的单块软面板（`surfaceSunken` 加圆角），去掉头尾与工具栏之间的分隔线。

### 工具与示例

- 新增 `GUAVA_PREVIEW_DEVICE=phone`：桌面构建以 390×844 窗口、触摸输入与刘海屏安全区运行，无需模拟器即可检查和调整移动端布局。
- 新增 `Tools/Release/release.sh`：升版本、提交、打标签、推送并等待发布工作流；网络步骤带退避重试，失败的工作流先重跑失败任务。
- 新增 `Examples/Mobile` 示例，共享业务代码位于 `Sources/AppContent`，覆盖 iOS、Android 与 macOS 目标。

## 0.1.4 — 2026-10-25

- 图形管线改用 `PipelineStore` 缓存：图形、计算与网格管线按描述复用，`Device` 通过它创建与销毁管线，并统计创建耗时与缓存命中。`NativeDrawListRenderer` 在配置变化后保留管线状态。
- 修复 Windows 上的原子安装写入：替换文件时按类型选择替换标志，`guava self update` 不再留下半写的可执行文件。
- CLI 子进程的标准错误改用专用读取器排空至 EOF，安装器与终端不再因管道未读完而卡住。
- 跨平台安装器修复后重新发布 SDK 与 CLI。

## 0.1.3 — 2026-10-24

- 图表支持堆叠与缩放，新增 `ChartOrientation`；视口可拖动与刷选，数据变化使用可中断的过渡动画。
- 修复 Android 的数学库与 SDL 链接，修正 Vulkan 动态描述符布局。
- 保留层滚动时，画家局部的裁剪保持与内容对齐。
- 恢复 DevTools 的独立依赖，并校验开发者客户端可独立启动。

## 0.1.2 — 2026-10-24

- 图表新增缩放与堆叠，补齐对应测试。
- 修复 CLI 在 Windows 上查找工具链的问题。

## 0.1.1 — 2026-10-24

- 修复 Windows 上读取 `PATH` 时区分大小写的问题，CLI 能正确找到已安装的工具。

## 0.1.0 — 2026-10-24

- 首个发布：预编译 `guava` CLI 与运行库，提供 macOS arm64、Windows x86_64 与 Linux x86_64 三个平台。
- `install.sh` 与 `install.ps1` 安装器写入用户 `PATH`；CLI 自带运行库，`guava --version` 不需要先装 Swift。
- 提供 `guava create`、`run`、`build`、`test`、`clean`、`devtools`、`sdk install` 与 `self update`。
