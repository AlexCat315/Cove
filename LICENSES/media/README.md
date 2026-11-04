# 媒体许可证原文

`native/` 保存当前 macOS 源码构建所用上游许可与署名材料的逐字副本，以及 Windows fallback 固定版本的 MIT 原文。`native.json` 记录五个平台共用的收集规则；构建器按 Meson 实际启用的子项目复制原文，不会把整个仓库中的插件都列为已链接。

来源为 GStreamer revision `1b2a95a1b72787cda146b46df9605b168e5b683b` 及其 wraps 下载的源码。`native/<component>/` 下的路径对应 `native.json` 中该组件源码目录内的路径。GL headers 和 proxy-libintl 的 `*.notice.txt` 是对应 C 文件中包含版权、许可、免责条款的完整注释，未翻译或改写。

`rust-supplements.json` 为发布包缺少许可证全文的 Rust crate 提供补充文件，记录 crate 版本、上游源码 revision 和原文来源。构建器核对版本、声明与 `.cargo_vcs_info.json`；不匹配时停止，防止升级后沿用旧通知。其他 Rust crate 从 Cargo 实际解析的源码包复制全部顶层 LICENSE / LICENCE / COPYING / NOTICE / COPYRIGHT / UNLICENSE 文件和许可证目录，不把 `AND` 条款删掉或只保留双许可的一半。

objc2 系列的这些已锁定 revision 只有链接到 MIT 通用条款的 `LICENSE.md`，没有 MIT 全文；这里同时保留各 revision 的原始声明，以及上游 revision `8a5abacd90d5403fc0463459a35826f892b9142e` 后来明确提供的 `LICENSE-MIT.txt` 和版权。两者的独立来源在补充记录的 `text_sources` 字段标明，未给旧源码伪造版权年份。taffy 全文来自该 crate 对应的精确上游 revision。

生成包中的 `inventory.json` 保留每个 crate 的完整许可证表达式；对补充记录显式选择 MIT 的 crate，同时保留上游提供的其他可选许可声明。`source-records/` 保存 wraps 和补丁；这些记录不等于完整对应源码。源码、应用重新链接材料及分发要求见 [Docs/media-licensing.md](../../Docs/media-licensing.md)。

升级 GStreamer/wraps 后应同步这些原文及规则；增加未登记的 Meson 子项目、缺失许可证文件或出现没有全文的 Rust crate 时，构建会报错。生成器不把 `--dependencies` 的所有探测结果当作实际链接依赖；原始探测和目标记录一并保留供最终产品核对。
