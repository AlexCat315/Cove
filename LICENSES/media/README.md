# 媒体许可证材料

本目录只保留无法从 Cargo 注册表源码包直接取得全文的 Rust 依赖补充声明（`rust-supplements/` 与 `rust-supplements.json`），例如只链接通用 MIT 条款而未附全文的 objc2 版本。记录包含 crate 版本、上游 revision 与原文来源，升级依赖后应重新核对。

`cove-media` 的 FFmpeg 后端由 `ffmpeg-sys-next` 在构建时从 `release/9.0` 源码静态编译，FFmpeg 自身的 `COPYING.LGPLv2.1`、`COPYING.LGPLv3` 与各文件版权声明位于该源码树中，不在此重复保存。分发二进制时须随版本提供对应源码与重新链接材料，见 [Docs/media-licensing.md](../../Docs/media-licensing.md)。

Rust 依赖的许可证从 Cargo 实际解析的源码包复制其顶层 LICENSE / LICENCE / COPYING / NOTICE / COPYRIGHT / UNLICENSE 文件；不要删除双许可中的任一分支，也不要只保留补充文件而忽略 crate 自带声明。
