# Third-party notices

Cove 自有代码按 [Apache License 2.0](LICENSE) 提供。第三方源码、生成的绑定、静态库及随包资源保留各自的版权和许可证；根目录的许可证不替代这些条款。

## 可选原生媒体

`cove-media` 的 `ffmpeg` feature 通过 `ffmpeg-sys-next 9.0.1` 从 FFmpeg `release/9.0` 源码静态编译。构建启用 `avcodec`、`avfilter`、`avformat`、`swresample`、`swscale`，关闭 `gpl`、`version3`、`nonfree`，不自动探测外部库。Rust 依赖由 `Cargo.lock` 固定。

| 组件 | 许可证及原文 |
| --- | --- |
| FFmpeg 9.0（libavcodec、libavfilter、libavformat、libavutil、libswresample、libswscale） | LGPL-2.1-or-later，不启用 GPL/version3/nonfree 组件；原文为源码树中的 `COPYING.LGPLv2.1`、`COPYING.LGPLv3` 及逐文件版权声明 |
| `ffmpeg-sys-next` 9.0.1 | WTFPL；绑定在构建时生成 |
| `cpal` 0.18.2 及其平台后端 | Apache-2.0；各平台使用 CoreAudio、WASAPI、ALSA 或 AAudio 等系统 API |

FFmpeg 源码与原文在首次构建时下载到 Cargo 的 `OUT_DIR`，不保存在本仓库。分发静态链接 FFmpeg 的二进制时，须同时提供对应源码、构建参数与重新链接材料；单独附带本文件或“关于”页面不能替代这些材料。具体操作见 [媒体分发说明](Docs/media-licensing.md)。

## 其他组件

上述媒体清单不覆盖 Cove 的其他原生库、字体、着色器工具链、浏览器、终端及最终应用自行加入的依赖。它们的原始声明随各自的源码提供；分发方须按最终链接和随包资源补齐声明。Apple、Windows、Android 等平台 SDK 和操作系统库适用其自身条款，不因 Cove 的 Apache-2.0 许可而改变。
