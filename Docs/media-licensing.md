# 原生媒体的许可证与分发

日期：2026-11-05。适用范围：`cove-media` / `cove-media-rhi` 的 `ffmpeg` feature。Cove 自有代码沿用 Apache-2.0；FFmpeg、`cpal` 及其他第三方代码保留各自许可。

## 构建 profile

`cove-media` 通过 `ffmpeg-sys-next = 9.0.1` 的 `build` feature 从 FFmpeg `release/9.0` 源码静态编译以下库：`libavcodec`、`libavfilter`、`libavformat`、`libavutil`、`libswresample`、`libswscale`。

构建脚本固定传入 `--disable-autodetect`、`--disable-programs`、`--disable-doc`，并且未启用 `build-license-gpl`、`build-license-version3`、`build-license-nonfree`，因此：

- FFmpeg 产物按 LGPL-2.1-or-later 分发，不包含 GPL 编码器（x264/x265 等）或 nonfree 组件。
- 不链接系统或第三方的 FFmpeg 依赖；实际启用的外部库由 `ffbuild/config.mak` 的 `EXTRALIBS` 决定，当前为空。
- `cpal` 在 macOS/iOS 使用系统 CoreAudio，Windows 使用 WASAPI，Linux 使用 ALSA，Android 使用 AAudio/Oboe；这些系统 API 不引入额外随包库。

## 构建机依赖

普通用户安装已打包的应用，所需媒体库、许可证与运行时随应用交付，不需要 Rust、FFmpeg 开发 SDK、`make`、`nasm` 或 `clang`。源码构建者需要一次：

```sh
cargo cove setup --media
cargo build --locked -p cove-media --features ffmpeg
```

setup 按平台安装 C/C++ 编译器与 SDK、`make` ≥ 3.81、`nasm` ≥ 2.13、`clang`（bindgen 需要 libclang）；Linux 另需 `pkg-config` 与 ALSA 开发包。FFmpeg 源码在首次构建时由 Cargo 构建脚本下载到 `OUT_DIR`，之后增量复用。

## 静态链接 LGPL 的义务

FFmpeg 以静态库链接进应用，分发二进制时应按 LGPL 2.1（及实际适用版本）提供：

1. **对应源码**：FFmpeg `release/9.0` 的完整源码、实际使用的 `configure` 参数，以及本仓库对构建脚本或 FFmpeg 的任何本地修改。不要只提供版本号、官网链接或 `.a` 文件。
2. **构建材料**：`crates/media/Cargo.toml`、`Cargo.lock`、`cove-native-build` 的 setup 逻辑、工具链版本与目标三元组，足以复现相同的静态库。
3. **重新链接材料**：提供应用源码及依赖，或按 LGPL 要求提供可与修改后的 FFmpeg 重新链接的应用目标文件、必要数据与操作说明。Rust LTO 产物或单独的最终可执行文件不应未经验证就当作完整方案。
4. **许可条款**：保留 FFmpeg 的版权、许可证与免责声明，并允许接收者为自身使用修改库、为调试修改而逆向工程；应用 EULA 不得抵消这些权利。

建议的发行流程：

- 在源码构建后保留 `OUT_DIR/ffmpeg-9.0` 源码树与 `ffbuild/config.mak`，随发行材料归档并记录校验值。
- 提供 `cargo vendor --locked --versioned-dirs <vendor-dir>` 的 Rust 依赖，以及 Cargo source 配置与锁文件。
- 在隔离目录中用这些材料重建一次，确认不下载未提供的源码；修改一个 FFmpeg 可见行为后重新链接应用并运行，作为验收。
- 签名、商店条款与安装方式不能限制接收者行使 LGPL 权利。

## 随包声明

Cove 自有代码全文见仓库根目录 `LICENSE`。FFmpeg 的 `COPYING.LGPLv2.1`、`COPYING.LGPLv3` 与逐文件版权声明随 FFmpeg 源码一起取得；Rust 依赖的许可证来自 Cargo 源码包，无法从源码包取得全文的补充文本保存在 [LICENSES/media](../LICENSES/media/README.md)。

仓库不自动收集 FFmpeg 许可证原文；发布前按上述清单人工核对并在应用内提供“关于/许可证”入口。没有运行时 UI 的库、CLI 和 SDK 同样保留这些文件。

## 平台验收

macOS 已在本机完成源码构建与测试；Linux、Windows、iOS、Android 的 FFmpeg 构建路径已实现，设备与商店分发验收仍未完成。交叉构建须在 setup 中准备对应 SDK/NDK，并使用与实际构建一致的目标三元组。
