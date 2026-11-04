# Third-party notices

Cove 自有代码按 [Apache License 2.0](LICENSE) 提供。第三方源码、生成的绑定、静态库及随包资源保留各自的版权和许可证；根目录的许可证不替代这些条款。

## 可选原生媒体

固定源码：GStreamer **1.28.8**，revision `1b2a95a1b72787cda146b46df9605b168e5b683b`。原生传递依赖由该 revision 的 Meson wraps 固定；Rust 依赖由 `Cargo.lock` 固定。以下为当前 profile 的声明，实际构建清单随目标平台生成。

| 组件 | 许可证及原文 |
| --- | --- |
| GStreamer core、base、good | LGPL-2.1-or-later；[core](LICENSES/media/native/gstreamer/COPYING)、[base](LICENSES/media/native/gst-plugins-base/COPYING)、[good](LICENSES/media/native/gst-plugins-good/COPYING) |
| GStreamer bad、codecparsers | [LGPL-2.1-or-later](LICENSES/media/native/gst-plugins-bad/COPYING)，内嵌 dboolhuff 保留 [BSD 条款和专利授权](LICENSES/media/native/gst-plugins-bad/gst-libs/gst/codecparsers/) |
| GLib 2.86.3、GVDB | [LGPL-2.1-or-later](LICENSES/media/native/glib/COPYING)，附带 [GLib 各文件的其他许可证](LICENSES/media/native/glib/LICENSES/)；[GVDB](LICENSES/media/native/gvdb/COPYING) |
| proxy-libintl 0.5（Meson 版本字段为 `1`） | [LGPL-2.0-or-later](LICENSES/media/native/proxy-libintl/COPYING) |
| PCRE2 10.47、SLJIT | [BSD-3-Clause WITH PCRE2-exception，另含 SLJIT 的 BSD 条款](LICENSES/media/native/pcre2/LICENCE.md) |
| libffi（Meson 3.2.9999，wrap 3.2.9999.5） | [MIT](LICENSES/media/native/libffi/LICENSE) |
| zlib 1.3.1 | [Zlib](LICENSES/media/native/zlib/LICENSE) |
| Khronos GL headers 2019.1.0 | [原始版权、许可和免责注释](LICENSES/media/native/gl-headers/) |
| Windows DirectX-Headers、DirectXMath（启用对应 fallback 时） | MIT；[DirectX-Headers](LICENSES/media/native/directx-headers/LICENSE)、[DirectXMath](LICENSES/media/native/directxmath/LICENSE)，构建时从实际启用的源码目录收集 |
| 官方 GStreamer Rust 绑定、GLib Rust 绑定及传递依赖 | 按每个 crate 的声明收集全文；并非统一采用 LGPL。包括 MIT / Apache-2.0、Unicode-3.0、LLVM exception 等附加条款 |

完整原文及来源规则见 [LICENSES/media](LICENSES/media/README.md)。`glib/LICENSES/` 中保留了上游测试、工具等代码的许可证文本；目录内存在某种许可证，不表示当前应用链接了对应代码。

构建器自动在 `<native-out>/install/share/licenses/cove-media/` 写入第三方声明、逐组件原文、版本、Cargo 校验信息、Meson 项目和构建记录。应将该目录随使用媒体功能的二进制一起分发。

静态链接 LGPL 还须提供对应源码及重新链接所需材料；单独附带本文件或“关于”页面不能替代这些材料。具体操作见 [媒体分发说明](Docs/media-licensing.md)。当前 profile 显式关闭 GPL 选项、ugly 和 gst-libav；增加插件或外部库后须更新实际清单，不能按插件分组名称推断许可证。

## 其他组件

上述媒体清单不覆盖 Cove 的其他原生库、字体、着色器工具链、浏览器、终端及最终应用自行加入的依赖。它们的原始声明随各自的源码提供；分发方须按最终链接和随包资源补齐声明。Apple、Windows、Android 等平台 SDK 和操作系统库适用其自身条款，不因 Cove 的 Apache-2.0 许可而改变。
